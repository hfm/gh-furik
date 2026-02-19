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
        move |node| opened_pull_request_event_from_node(node, from, to),
        move |node| {
            let created_at = parse_datetime(
                query_value!(node["createdAt"] -> str).expect("pull request missing createdAt"),
            )?;
            Ok(created_at.date_naive() < from)
        },
    )
    .await
}

fn opened_pull_request_event_from_node(
    node: &serde_json::Value,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Option<EventItem>> {
    let created_at = parse_datetime(
        query_value!(node["createdAt"] -> str).expect("pull request missing createdAt"),
    )?;
    if !in_range(created_at, from, to) {
        return Ok(None);
    }
    let url = query_value!(node.url -> str).expect("pull request missing url");
    let title = query_value!(node.title -> str).expect("pull request missing title");
    let body = query_value!(node.body -> str).map(str::to_string);
    let repository = query_value!(node.repository["nameWithOwner"] -> str)
        .expect("pull request missing repository nameWithOwner");

    Ok(Some(EventItem {
        kind: EventKind::PullRequestOpened,
        created_at,
        url: url.to_string(),
        body,
        repository: repository.to_string(),
        subject_title: title.to_string(),
        subject_url: url.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opened_pull_request_event_includes_body() {
        let node = serde_json::json!({
            "createdAt": "2025-01-10T00:00:00Z",
            "url": "https://example.test/pull/1",
            "title": "PR A",
            "body": "first line\nsecond line",
            "repository": { "nameWithOwner": "o/r" }
        });

        let event = opened_pull_request_event_from_node(
            &node,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(event.kind, EventKind::PullRequestOpened);
        assert_eq!(event.body.as_deref(), Some("first line\nsecond line"));
    }
}
