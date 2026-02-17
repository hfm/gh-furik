use super::fetch::graphql_data;
use super::types::GraphqlResponse;
use anyhow::Context;
use valq::query_value;

pub(crate) async fn query_viewer_login(client: &octocrab::Octocrab) -> anyhow::Result<String> {
    let payload = serde_json::json!({ "query": "query { viewer { login } }" });

    let resp: GraphqlResponse<serde_json::Value> = client
        .graphql(&payload)
        .await
        .context("GraphQL viewer query failed")?;

    let data = graphql_data(resp)?;
    let login = query_value!(data.viewer.login -> str).expect("viewer response missing login");
    Ok(login.to_string())
}
