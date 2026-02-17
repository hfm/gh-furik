use super::fetch::{fetch_paginated_json, in_range, parse_datetime};
use super::queries::QueryKind;
use super::types::*;
use valq::query_value;

pub(crate) async fn query_issue_comments(
    client: &octocrab::Octocrab,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
) -> anyhow::Result<Vec<EventItem>> {
    let stop_from = from;
    fetch_paginated_json(
        client,
        QueryKind::IssueComments,
        move |node| {
            let created_at = parse_datetime(
                query_value!(node["createdAt"] -> str).expect("issue comment missing createdAt"),
            )?;
            if !in_range(created_at, from, to) {
                return Ok(None);
            }
            let url = query_value!(node.url -> str).expect("issue comment missing url");
            let body = query_value!(node.body -> str).expect("issue comment missing body");
            let title = query_value!(node.issue.title -> str).expect("issue missing title");
            let subject_url = query_value!(node.issue.url -> str).expect("issue missing url");
            let repository = query_value!(node.issue.repository["nameWithOwner"] -> str)
                .expect("issue missing repository nameWithOwner");

            Ok(Some(EventItem {
                kind: EventKind::IssueComment,
                created_at,
                url: url.to_string(),
                body: Some(body.to_string()),
                repository: repository.to_string(),
                subject_title: title.to_string(),
                subject_url: subject_url.to_string(),
            }))
        },
        move |node| {
            let updated_at = parse_datetime(
                query_value!(node["updatedAt"] -> str).expect("issue comment missing updatedAt"),
            )?;
            Ok(match stop_from {
                Some(from) => updated_at.date_naive() < from,
                None => false,
            })
        },
    )
    .await
}
