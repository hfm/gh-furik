use super::fetch::{event_items_from_search_node, fetch_search_nodes_range};
use super::types::{EventItem, EventKind};

pub(crate) async fn query_closed_pull_requests(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if from > to {
        return Ok(Vec::new());
    }

    let nodes = fetch_search_nodes_range(client.octocrab(), "is:pr", from, to).await?;

    Ok(nodes
        .into_iter()
        .flat_map(|node| event_items_from_search_node(&node, client.viewer_login(), from, to))
        .filter(|item| {
            matches!(
                item.kind,
                EventKind::PullRequestClosed | EventKind::PullRequestMerged
            )
        })
        .collect())
}
