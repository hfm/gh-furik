use anyhow::Context;
use valq::query_value;

pub(crate) async fn query_viewer_login(client: &octocrab::Octocrab) -> anyhow::Result<String> {
    let payload = serde_json::json!({ "query": "query { viewer { login } }" });

    let data: serde_json::Value = client
        .graphql(&payload)
        .await
        .context("GraphQL viewer query failed")?;

    let login = query_value!(data.viewer.login -> str).expect("viewer response missing login");
    Ok(login.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::graphql::test_support::{client, mock_response};
    use serde_json::json;
    use wiremock::MockServer;

    #[tokio::test]
    async fn viewer_login_reads_graphql_data() {
        let server = MockServer::start().await;
        mock_response(
            &server,
            json!({"query": "query { viewer { login } }"}),
            json!({"data": {"viewer": {"login": "me"}}}),
        )
        .await;
        assert_eq!(query_viewer_login(&client(&server)).await.unwrap(), "me");
    }
}
