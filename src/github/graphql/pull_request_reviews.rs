use super::fetch::{MAX_PAGES, graphql_data, in_range, parse_datetime};
use super::queries::REVIEW_CONTRIBUTIONS_QUERY;
use super::types::*;
use anyhow::Context;
use chrono::TimeZone;
use valq::query_value;

pub(crate) async fn query_pull_request_review_contributions(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if from > to {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut current = from;
    while current <= to {
        let candidate = current + chrono::Duration::days(364);
        let chunk_end = if candidate > to { to } else { candidate };
        out.extend(
            query_pull_request_review_contributions_range(client, current, chunk_end).await?,
        );
        let Some(next) = chunk_end.succ_opt() else {
            break;
        };
        current = next;
    }

    Ok(out)
}

async fn query_pull_request_review_contributions_range(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    let from_dt = from.and_hms_opt(0, 0, 0).context("invalid start of day")?;
    let to_dt = to.and_hms_opt(23, 59, 59).context("invalid end of day")?;
    let from_dt = chrono::Utc.from_utc_datetime(&from_dt);
    let to_dt = chrono::Utc.from_utc_datetime(&to_dt);
    let mut after: Option<String> = None;
    let mut out = Vec::new();

    for _ in 0..MAX_PAGES {
        let payload = serde_json::json!({
            "query": REVIEW_CONTRIBUTIONS_QUERY,
            "variables": { "from": from_dt, "to": to_dt, "after": after.clone() },
        });

        let resp = client
            .octocrab()
            .graphql::<GraphqlResponse<serde_json::Value>>(&payload)
            .await
            .context("GraphQL review contributions query failed")?;

        let data = graphql_data(resp)?;
        let connection = data
            .get("viewer")
            .and_then(|viewer| viewer.get("contributionsCollection"))
            .and_then(|collection| collection.get("pullRequestReviewContributions"))
            .expect("review contributions response missing connection");

        if let Some(nodes) = connection.get("nodes").and_then(|nodes| nodes.as_array()) {
            for node in nodes.iter().filter(|node| !node.is_null()) {
                let occurred_at = parse_datetime(
                    query_value!(node["occurredAt"] -> str)
                        .expect("review contribution missing occurredAt"),
                )?;

                let Some(review) = query_value!(node.pullRequestReview) else {
                    continue;
                };

                let review_url = query_value!(review.url -> str).expect("review missing url");
                let review_body = query_value!(review.body -> str).map(str::to_string);
                let pull_request =
                    query_value!(review.pullRequest).expect("review missing pullRequest");
                let subject_title =
                    query_value!(pull_request.title -> str).expect("pull request missing title");
                let subject_url =
                    query_value!(pull_request.url -> str).expect("pull request missing url");
                let repository = query_value!(pull_request.repository["nameWithOwner"] -> str)
                    .expect("pull request missing repository nameWithOwner");

                if in_range(occurred_at, from, to) {
                    out.push(EventItem {
                        kind: EventKind::PullRequestReview,
                        created_at: occurred_at,
                        url: review_url.to_string(),
                        body: review_body.clone(),
                        repository: repository.to_string(),
                        subject_title: subject_title.to_string(),
                        subject_url: subject_url.to_string(),
                    });
                }

                if let Some(comments) = query_value!(review.comments.nodes -> array) {
                    for comment in comments.iter() {
                        let created_at = parse_datetime(
                            query_value!(comment["createdAt"] -> str)
                                .expect("review comment missing createdAt"),
                        )?;
                        if !in_range(created_at, from, to) {
                            continue;
                        }
                        let comment_url =
                            query_value!(comment.url -> str).expect("review comment missing url");
                        let comment_body =
                            query_value!(comment.body -> str).expect("review comment missing body");
                        out.push(EventItem {
                            kind: EventKind::PullRequestReviewComment,
                            created_at,
                            url: comment_url.to_string(),
                            body: Some(comment_body.to_string()),
                            repository: repository.to_string(),
                            subject_title: subject_title.to_string(),
                            subject_url: subject_url.to_string(),
                        });
                    }
                }
            }
        }

        let page_info = connection
            .get("pageInfo")
            .expect("review contributions response missing pageInfo");
        let has_next_page = page_info
            .get("hasNextPage")
            .and_then(|value| value.as_bool())
            .expect("review contributions response missing pageInfo.hasNextPage");
        let end_cursor = page_info
            .get("endCursor")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        if !has_next_page {
            break;
        }
        after = end_cursor;
        if after.is_none() {
            break;
        }
    }

    Ok(out)
}
