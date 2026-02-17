use super::fetch::{event_items_from_search_node, fetch_search_nodes_range};
use super::types::{EventItem, EventKind};
use std::collections::HashSet;

pub(crate) async fn query_closed_pull_requests(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if from > to {
        return Ok(Vec::new());
    }

    let nodes = fetch_search_nodes_range(client.octocrab(), "is:pr", from, to).await?;

    let items: Vec<_> = nodes
        .into_iter()
        .flat_map(|node| event_items_from_search_node(&node, client.viewer_login(), from, to))
        .filter(|item| {
            matches!(
                item.kind,
                EventKind::PullRequestClosed | EventKind::PullRequestMerged
            )
        })
        .collect();

    Ok(filter_out_closed_when_merged(items))
}

fn filter_out_closed_when_merged(items: Vec<EventItem>) -> Vec<EventItem> {
    let merged_subjects: HashSet<_> = items
        .iter()
        .filter(|item| item.kind == EventKind::PullRequestMerged)
        .map(|item| item.subject_url.clone())
        .collect();

    items
        .into_iter()
        .filter(|item| {
            !(item.kind == EventKind::PullRequestClosed
                && merged_subjects.contains(item.subject_url.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn event(kind: EventKind, subject_url: &str) -> EventItem {
        EventItem {
            kind,
            created_at: chrono::Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            url: subject_url.to_string(),
            body: None,
            repository: "owner/repo".to_string(),
            subject_title: "Sample PR".to_string(),
            subject_url: subject_url.to_string(),
        }
    }

    #[test]
    fn drops_closed_when_merged_exists_for_same_pr() {
        let items = vec![
            event(EventKind::PullRequestClosed, "https://example.test/pull/1"),
            event(EventKind::PullRequestMerged, "https://example.test/pull/1"),
        ];

        let actual = filter_out_closed_when_merged(items);

        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].kind, EventKind::PullRequestMerged);
    }

    #[test]
    fn keeps_closed_when_no_merged_exists_for_same_pr() {
        let items = vec![
            event(EventKind::PullRequestClosed, "https://example.test/pull/1"),
            event(EventKind::PullRequestMerged, "https://example.test/pull/2"),
        ];

        let actual = filter_out_closed_when_merged(items);

        assert_eq!(actual.len(), 2);
        assert!(
            actual
                .iter()
                .any(|item| item.kind == EventKind::PullRequestClosed
                    && item.subject_url == "https://example.test/pull/1")
        );
    }
}
