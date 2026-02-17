use super::fetch::{event_items_from_search_node, fetch_search_nodes_range, normalize_range};
use super::types::{EventItem, EventKind};

pub(crate) async fn query_closed_issues(
    client: &octocrab::Octocrab,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
    viewer_login: &str,
) -> anyhow::Result<Vec<EventItem>> {
    let (range_from, range_to) = normalize_range(from, to);
    if range_from > range_to {
        return Ok(Vec::new());
    }

    let nodes = fetch_search_nodes_range(client, "is:issue", range_from, range_to).await?;

    Ok(nodes
        .into_iter()
        .flat_map(|node| event_items_from_search_node(&node, viewer_login, from, to))
        .filter(|item| matches!(item.kind, EventKind::IssueClosed))
        .collect())
}
