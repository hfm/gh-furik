use super::fetch::{SearchSpec, event_items_from_search_node, fetch_search_nodes_range};
use super::queries::SEARCH_QUERY;
use super::types::{EventItem, EventKind};

pub(crate) async fn query_closed_issues(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    if from > to {
        return Ok(Vec::new());
    }

    let spec = SearchSpec {
        query_base: "is:issue involves:@me",
        date_field: "closed",
        query_suffix: None,
        document: SEARCH_QUERY,
        variables: serde_json::json!({}),
    };
    let nodes = fetch_search_nodes_range(client.octocrab(), &spec, from, to).await?;

    Ok(nodes
        .into_iter()
        .flat_map(|node| event_items_from_search_node(&node, client.viewer_login(), from, to))
        .filter(|item| matches!(item.kind, EventKind::IssueClosed))
        .collect())
}
