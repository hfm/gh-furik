use super::fetch::{
    MAX_PAGES, SearchSpec, fetch_search_nodes_range, graphql_data, graphql_with_retry, in_range,
    parse_datetime,
};
use super::queries::{
    PULL_REQUEST_REVIEWS_QUERY, REVIEW_COMMENTS_QUERY, REVIEWED_PULL_REQUESTS_QUERY,
};
use super::types::*;
use anyhow::Context;
use valq::query_value;

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
        variables: serde_json::json!({ "author": client.viewer_login() }),
    };
    // A PR keeps receiving updates after the review, so the search window must extend to today.
    let search_to = to.max(chrono::Utc::now().date_naive());
    let nodes = fetch_search_nodes_range(client.octocrab(), &spec, from, search_to).await?;

    let mut out = Vec::new();
    for node in &nodes {
        out.extend(event_items_from_reviewed_pull_request(node, from, to)?);
        out.extend(fetch_additional_review_events(client, node, from, to).await?);
    }
    Ok(out)
}

fn event_items_from_reviewed_pull_request(
    node: &serde_json::Value,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    let mut out = Vec::new();

    let subject_title = query_value!(node.title -> str).expect("pull request missing title");
    let subject_url = query_value!(node.url -> str).expect("pull request missing url");
    let repository = query_value!(node.repository["nameWithOwner"] -> str)
        .expect("pull request missing repository nameWithOwner");

    let Some(reviews) = query_value!(node.reviews.nodes -> array) else {
        return Ok(out);
    };

    for review in reviews.iter().filter(|review| !review.is_null()) {
        out.extend(event_items_from_review(
            review,
            subject_title,
            subject_url,
            repository,
            from,
            to,
        )?);
    }

    Ok(out)
}

fn event_items_from_review(
    review: &serde_json::Value,
    subject_title: &str,
    subject_url: &str,
    repository: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    let Some(submitted_at) = query_value!(review["submittedAt"] -> str) else {
        return Ok(Vec::new());
    };
    let submitted_at = parse_datetime(submitted_at)?;
    let mut out = Vec::new();

    if in_range(submitted_at, from, to) {
        out.push(EventItem {
            kind: EventKind::PullRequestReview,
            created_at: submitted_at,
            url: query_value!(review.url -> str)
                .expect("review missing url")
                .to_string(),
            body: query_value!(review.body -> str).map(str::to_string),
            repository: repository.to_string(),
            subject_title: subject_title.to_string(),
            subject_url: subject_url.to_string(),
        });
    }

    if let Some(comments) = query_value!(review.comments.nodes -> array) {
        for comment in comments.iter().filter(|comment| !comment.is_null()) {
            if let Some(item) = event_item_from_review_comment(
                comment,
                subject_title,
                subject_url,
                repository,
                from,
                to,
            )? {
                out.push(item);
            }
        }
    }

    Ok(out)
}

fn event_item_from_review_comment(
    comment: &serde_json::Value,
    subject_title: &str,
    subject_url: &str,
    repository: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Option<EventItem>> {
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

async fn fetch_additional_review_events(
    client: &crate::github::Client,
    pull_request: &serde_json::Value,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    let pull_request_id = query_value!(pull_request.id -> str).expect("pull request missing id");
    let subject_title =
        query_value!(pull_request.title -> str).expect("pull request missing title");
    let subject_url = query_value!(pull_request.url -> str).expect("pull request missing url");
    let repository = query_value!(pull_request.repository["nameWithOwner"] -> str)
        .expect("pull request missing repository nameWithOwner");
    let mut out = Vec::new();

    if let Some(reviews) = query_value!(pull_request.reviews.nodes -> array) {
        for review in reviews.iter().filter(|review| !review.is_null()) {
            out.extend(
                fetch_additional_review_comments(
                    client,
                    review,
                    subject_title,
                    subject_url,
                    repository,
                    from,
                    to,
                )
                .await?,
            );
        }
    }

    let mut after = next_cursor(
        pull_request
            .get("reviews")
            .context("pull request missing reviews")?,
    )?;
    for _ in 0..MAX_PAGES {
        let Some(cursor) = after.take() else {
            break;
        };
        let payload = serde_json::json!({
            "query": PULL_REQUEST_REVIEWS_QUERY,
            "variables": {
                "id": pull_request_id,
                "author": client.viewer_login(),
                "after": cursor,
            },
        });
        let response = graphql_with_retry::<GraphqlResponse<serde_json::Value>>(
            client.octocrab(),
            &payload,
            "GraphQL pull request reviews query failed",
        )
        .await?;
        let data = graphql_data(response)?;
        let reviews = data
            .get("node")
            .and_then(|node| node.get("reviews"))
            .context("pull request reviews response missing connection")?;

        if let Some(nodes) = reviews.get("nodes").and_then(|nodes| nodes.as_array()) {
            for review in nodes.iter().filter(|review| !review.is_null()) {
                out.extend(event_items_from_review(
                    review,
                    subject_title,
                    subject_url,
                    repository,
                    from,
                    to,
                )?);
                out.extend(
                    fetch_additional_review_comments(
                        client,
                        review,
                        subject_title,
                        subject_url,
                        repository,
                        from,
                        to,
                    )
                    .await?,
                );
            }
        }
        after = next_cursor(reviews)?;
    }

    Ok(out)
}

async fn fetch_additional_review_comments(
    client: &crate::github::Client,
    review: &serde_json::Value,
    subject_title: &str,
    subject_url: &str,
    repository: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if query_value!(review["submittedAt"] -> str).is_none() {
        return Ok(Vec::new());
    }
    let review_id = query_value!(review.id -> str).expect("review missing id");
    let comments = review
        .get("comments")
        .context("review missing comments connection")?;
    let mut after = next_cursor(comments)?;
    let mut out = Vec::new();

    for _ in 0..MAX_PAGES {
        let Some(cursor) = after.take() else {
            break;
        };
        let payload = serde_json::json!({
            "query": REVIEW_COMMENTS_QUERY,
            "variables": { "id": review_id, "after": cursor },
        });
        let response = graphql_with_retry::<GraphqlResponse<serde_json::Value>>(
            client.octocrab(),
            &payload,
            "GraphQL review comments query failed",
        )
        .await?;
        let data = graphql_data(response)?;
        let comments = data
            .get("node")
            .and_then(|node| node.get("comments"))
            .context("review comments response missing connection")?;

        if let Some(nodes) = comments.get("nodes").and_then(|nodes| nodes.as_array()) {
            for comment in nodes.iter().filter(|comment| !comment.is_null()) {
                if let Some(item) = event_item_from_review_comment(
                    comment,
                    subject_title,
                    subject_url,
                    repository,
                    from,
                    to,
                )? {
                    out.push(item);
                }
            }
        }
        after = next_cursor(comments)?;
    }

    Ok(out)
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

    fn pull_request(reviews: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "__typename": "PullRequest",
            "url": "https://example.test/pull/1",
            "title": "Sample PR",
            "repository": { "nameWithOwner": "owner/repo" },
            "reviews": { "nodes": reviews },
        })
    }

    #[test]
    fn emits_review_and_comments_within_range() {
        let node = pull_request(serde_json::json!([{
            "state": "APPROVED",
            "submittedAt": "2025-01-02T10:00:00Z",
            "url": "https://example.test/pull/1#pullrequestreview-1",
            "body": "LGTM",
            "comments": { "nodes": [
                { "createdAt": "2025-01-02T09:00:00Z", "url": "https://example.test/pull/1#discussion_r1", "body": "nit" },
                { "createdAt": "2025-02-01T09:00:00Z", "url": "https://example.test/pull/1#discussion_r2", "body": "late" }
            ] }
        }]));

        let items =
            event_items_from_reviewed_pull_request(&node, date("2025-01-01"), date("2025-01-31"))
                .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, EventKind::PullRequestReview);
        assert_eq!(items[0].body.as_deref(), Some("LGTM"));
        assert_eq!(items[1].kind, EventKind::PullRequestReviewComment);
        assert_eq!(items[1].url, "https://example.test/pull/1#discussion_r1");
    }

    #[test]
    fn skips_reviews_outside_range_and_pending_reviews() {
        let node = pull_request(serde_json::json!([
            {
                "state": "APPROVED",
                "submittedAt": "2024-12-31T23:00:00Z",
                "url": "https://example.test/pull/1#pullrequestreview-1",
                "body": "",
                "comments": { "nodes": [] }
            },
            {
                "state": "PENDING",
                "submittedAt": null,
                "url": "https://example.test/pull/1#pullrequestreview-2",
                "body": "",
                "comments": { "nodes": [] }
            }
        ]));

        let items =
            event_items_from_reviewed_pull_request(&node, date("2025-01-01"), date("2025-01-31"))
                .unwrap();

        assert!(items.is_empty());
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
