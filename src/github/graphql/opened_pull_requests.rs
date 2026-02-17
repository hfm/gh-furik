use super::fetch::{fetch_paginated_json, in_range, parse_datetime};
use super::queries::QueryKind;
use super::types::*;
use valq::query_value;

pub(crate) async fn query_opened_pull_requests(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    fetch_paginated_json(
        client.octocrab(),
        QueryKind::OpenedPullRequests,
        move |node| {
            let created_at = parse_datetime(
                query_value!(node["createdAt"] -> str).expect("pull request missing createdAt"),
            )?;
            if !in_range(created_at, from, to) {
                return Ok(None);
            }
            let url = query_value!(node.url -> str).expect("pull request missing url");
            let title = query_value!(node.title -> str).expect("pull request missing title");
            let repository = query_value!(node.repository["nameWithOwner"] -> str)
                .expect("pull request missing repository nameWithOwner");

            Ok(Some(EventItem {
                kind: EventKind::PullRequestOpened,
                created_at,
                url: url.to_string(),
                body: None,
                repository: repository.to_string(),
                subject_title: title.to_string(),
                subject_url: url.to_string(),
            }))
        },
        move |node| {
            let created_at = parse_datetime(
                query_value!(node["createdAt"] -> str).expect("pull request missing createdAt"),
            )?;
            Ok(created_at.date_naive() < from)
        },
    )
    .await
}
