use super::fetch::{
    MAX_PAGES, SearchSpec, fetch_search_nodes_range, graphql_with_retry, in_range, parse_datetime,
};
use super::queries::{
    PULL_REQUEST_REVIEWS_QUERY, REVIEW_COMMENTS_QUERY, REVIEWED_PULL_REQUESTS_QUERY,
};
use super::types::*;
use anyhow::Context;
use valq::query_value;

#[derive(Clone)]
struct ReviewSubject {
    title: String,
    url: String,
    repository: String,
}

pub(crate) async fn query_pull_request_review_contributions(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if from > to {
        return Ok(Vec::new());
    }

    let spec = SearchSpec {
        query_base: "is:pr reviewed-by:@me",
        date_field: "updated",
        query_suffix: Some(format!("created:<={to}")),
        document: REVIEWED_PULL_REQUESTS_QUERY,
        variables: serde_json::json!({}),
    };
    // A PR keeps receiving updates after the review, so the search window must extend to today.
    let search_to = to.max(chrono::Utc::now().date_naive());
    let nodes = fetch_search_nodes_range(client.octocrab(), &spec, from, search_to).await?;

    let mut out = Vec::new();
    let mut review_tasks = tokio::task::JoinSet::new();
    for node in nodes {
        let octocrab = client.octocrab().clone();
        let viewer_login = client.viewer_login().to_string();
        review_tasks.spawn(async move {
            fetch_review_events(&octocrab, &viewer_login, &node, from, to).await
        });
        if review_tasks.len() >= 4 {
            collect_event_task(&mut review_tasks, &mut out).await?;
        }
    }
    while !review_tasks.is_empty() {
        collect_event_task(&mut review_tasks, &mut out).await?;
    }
    Ok(out)
}

fn event_item_from_review(
    review: &serde_json::Value,
    subject_title: &str,
    subject_url: &str,
    repository: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Option<EventItem>> {
    let Some(submitted_at) = query_value!(review["submittedAt"] -> str) else {
        return Ok(None);
    };
    let submitted_at = parse_datetime(submitted_at)?;
    if !in_range(submitted_at, from, to) {
        return Ok(None);
    }

    Ok(Some(EventItem {
        kind: EventKind::PullRequestReview,
        created_at: submitted_at,
        url: query_value!(review.url -> str)
            .expect("review missing url")
            .to_string(),
        body: query_value!(review.body -> str).map(str::to_string),
        repository: repository.to_string(),
        subject_title: subject_title.to_string(),
        subject_url: subject_url.to_string(),
    }))
}

fn event_item_from_review_comment(
    comment: &serde_json::Value,
    subject_title: &str,
    subject_url: &str,
    repository: &str,
    viewer_login: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Option<EventItem>> {
    if query_value!(comment.author.login -> str) != Some(viewer_login) {
        return Ok(None);
    }
    let created_at = parse_datetime(
        query_value!(comment["createdAt"] -> str).expect("review comment missing createdAt"),
    )?;
    if !in_range(created_at, from, to) {
        return Ok(None);
    }

    Ok(Some(EventItem {
        kind: EventKind::PullRequestReviewComment,
        created_at,
        url: query_value!(comment.url -> str)
            .expect("review comment missing url")
            .to_string(),
        body: Some(
            query_value!(comment.body -> str)
                .expect("review comment missing body")
                .to_string(),
        ),
        repository: repository.to_string(),
        subject_title: subject_title.to_string(),
        subject_url: subject_url.to_string(),
    }))
}

async fn fetch_review_events(
    client: &octocrab::Octocrab,
    viewer_login: &str,
    pull_request: &serde_json::Value,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    let pull_request_id = query_value!(pull_request.id -> str).expect("pull request missing id");
    let subject = ReviewSubject {
        title: query_value!(pull_request.title -> str)
            .expect("pull request missing title")
            .to_string(),
        url: query_value!(pull_request.url -> str)
            .expect("pull request missing url")
            .to_string(),
        repository: query_value!(pull_request.repository["nameWithOwner"] -> str)
            .expect("pull request missing repository nameWithOwner")
            .to_string(),
    };
    let mut out = Vec::new();
    let mut comment_tasks = tokio::task::JoinSet::new();

    let mut after: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let payload = serde_json::json!({
            "query": PULL_REQUEST_REVIEWS_QUERY,
            "variables": {
                "id": pull_request_id,
                "author": viewer_login,
                "after": after,
            },
        });
        let data = graphql_with_retry::<serde_json::Value>(
            client,
            &payload,
            "GraphQL pull request reviews query failed",
        )
        .await?;
        let reviews = data
            .get("node")
            .and_then(|node| node.get("reviews"))
            .context("pull request reviews response missing connection")?;

        if let Some(nodes) = reviews.get("nodes").and_then(|nodes| nodes.as_array()) {
            for review in nodes.iter().filter(|review| !review.is_null()) {
                if let Some(item) = event_item_from_review(
                    review,
                    &subject.title,
                    &subject.url,
                    &subject.repository,
                    from,
                    to,
                )? {
                    out.push(item);
                }
                let octocrab = client.clone();
                let viewer_login = viewer_login.to_string();
                let review = review.clone();
                let subject = subject.clone();
                comment_tasks.spawn(async move {
                    fetch_review_comments(&octocrab, &viewer_login, &review, &subject, from, to)
                        .await
                });
                if comment_tasks.len() >= 8 {
                    collect_event_task(&mut comment_tasks, &mut out).await?;
                }
            }
        }
        after = next_cursor(reviews)?;
        if after.is_none() {
            break;
        }
    }
    while !comment_tasks.is_empty() {
        collect_event_task(&mut comment_tasks, &mut out).await?;
    }

    Ok(out)
}

async fn fetch_review_comments(
    client: &octocrab::Octocrab,
    viewer_login: &str,
    review: &serde_json::Value,
    subject: &ReviewSubject,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if query_value!(review["submittedAt"] -> str).is_none() {
        return Ok(Vec::new());
    }
    let review_id = query_value!(review.id -> str).expect("review missing id");
    let mut after: Option<String> = None;
    let mut out = Vec::new();

    for _ in 0..MAX_PAGES {
        let payload = serde_json::json!({
            "query": REVIEW_COMMENTS_QUERY,
            "variables": { "id": review_id, "after": after },
        });
        let data = graphql_with_retry::<serde_json::Value>(
            client,
            &payload,
            "GraphQL review comments query failed",
        )
        .await?;
        let comments = data
            .get("node")
            .and_then(|node| node.get("comments"))
            .context("review comments response missing connection")?;

        if let Some(nodes) = comments.get("nodes").and_then(|nodes| nodes.as_array()) {
            for comment in nodes.iter().filter(|comment| !comment.is_null()) {
                if let Some(item) = event_item_from_review_comment(
                    comment,
                    &subject.title,
                    &subject.url,
                    &subject.repository,
                    viewer_login,
                    from,
                    to,
                )? {
                    out.push(item);
                }
            }
        }
        after = next_cursor(comments)?;
        if after.is_none() {
            break;
        }
    }

    Ok(out)
}

async fn collect_event_task(
    tasks: &mut tokio::task::JoinSet<anyhow::Result<Vec<EventItem>>>,
    out: &mut Vec<EventItem>,
) -> anyhow::Result<()> {
    let items = tasks
        .join_next()
        .await
        .context("review comment task missing")?
        .context("review comment task failed")??;
    out.extend(items);
    Ok(())
}

fn next_cursor(connection: &serde_json::Value) -> anyhow::Result<Option<String>> {
    let page_info = connection
        .get("pageInfo")
        .context("GraphQL connection missing pageInfo")?;
    let has_next_page = page_info
        .get("hasNextPage")
        .and_then(|value| value.as_bool())
        .context("GraphQL connection missing pageInfo.hasNextPage")?;
    if !has_next_page {
        return Ok(None);
    }

    Ok(page_info
        .get("endCursor")
        .and_then(|value| value.as_str())
        .map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn emits_review_within_range() {
        let review = serde_json::json!({
            "state": "APPROVED",
            "submittedAt": "2025-01-02T10:00:00Z",
            "url": "https://example.test/pull/1#pullrequestreview-1",
            "body": "LGTM"
        });

        let item = event_item_from_review(
            &review,
            "Sample PR",
            "https://example.test/pull/1",
            "owner/repo",
            date("2025-01-01"),
            date("2025-01-31"),
        )
        .unwrap()
        .unwrap();

        assert_eq!(item.kind, EventKind::PullRequestReview);
        assert_eq!(item.body.as_deref(), Some("LGTM"));
    }

    #[test]
    fn emits_only_viewer_review_comments_within_range() {
        let viewer_comment = serde_json::json!({
            "createdAt": "2025-01-02T09:00:00Z",
            "url": "https://example.test/pull/1#discussion_r1",
            "body": "nit",
            "author": { "login": "viewer" }
        });
        let other_comment = serde_json::json!({
            "createdAt": "2025-01-02T09:30:00Z",
            "url": "https://example.test/pull/1#discussion_r2",
            "body": "reply",
            "author": { "login": "someone-else" }
        });

        let item = event_item_from_review_comment(
            &viewer_comment,
            "Sample PR",
            "https://example.test/pull/1",
            "owner/repo",
            "viewer",
            date("2025-01-01"),
            date("2025-01-31"),
        )
        .unwrap()
        .unwrap();

        assert_eq!(item.kind, EventKind::PullRequestReviewComment);
        assert_eq!(item.url, "https://example.test/pull/1#discussion_r1");
        assert!(
            event_item_from_review_comment(
                &other_comment,
                "Sample PR",
                "https://example.test/pull/1",
                "owner/repo",
                "viewer",
                date("2025-01-01"),
                date("2025-01-31"),
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn returns_cursor_only_when_another_page_exists() {
        let more = serde_json::json!({
            "pageInfo": { "hasNextPage": true, "endCursor": "cursor-1" }
        });
        let done = serde_json::json!({
            "pageInfo": { "hasNextPage": false, "endCursor": "cursor-1" }
        });

        assert_eq!(next_cursor(&more).unwrap().as_deref(), Some("cursor-1"));
        assert_eq!(next_cursor(&done).unwrap(), None);
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::github::graphql::test_support::{client, mock_response};
    use serde_json::json;
    use wiremock::MockServer;

    #[tokio::test]
    async fn reviews_and_comments_read_graphql_data() {
        let server = MockServer::start().await;
        mock_response(
            &server,
            json!({
                "query": PULL_REQUEST_REVIEWS_QUERY,
                "variables": {
                    "id": "pr",
                    "author": "me",
                    "after": null
                }
            }),
            json!({
                "data": {
                    "node": {
                        "reviews": {
                            "nodes": [{
                                "id": "review",
                                "submittedAt": "2025-01-01T00:00:00Z",
                                "url": "https://example.test/review",
                                "body": "Approved"
                            }],
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            }
                        }
                    }
                }
            }),
        )
        .await;
        mock_response(
            &server,
            json!({
                "query": REVIEW_COMMENTS_QUERY,
                "variables": {
                    "id": "review",
                    "after": null
                }
            }),
            json!({
                "data": {
                    "node": {
                        "comments": {
                            "nodes": [{
                                "author": {
                                    "login": "me"
                                },
                                "createdAt": "2025-01-01T00:00:00Z",
                                "url": "https://example.test/comment",
                                "body": "Looks good"
                            }],
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            }
                        }
                    }
                }
            }),
        )
        .await;
        let date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let items = fetch_review_events(
            &client(&server),
            "me",
            &json!({
                "id": "pr",
                "title": "Change",
                "url": "https://example.test/pr",
                "repository": {
                    "nameWithOwner": "o/r"
                }
            }),
            date,
            date,
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, EventKind::PullRequestReview);
        assert_eq!(items[0].body.as_deref(), Some("Approved"));
        assert_eq!(items[1].kind, EventKind::PullRequestReviewComment);
        assert_eq!(items[1].body.as_deref(), Some("Looks good"));
    }
}
