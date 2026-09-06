mod closed_issues;
mod closed_pull_requests;
mod fetch;
mod issue_comments;
mod opened_issues;
mod opened_pull_requests;
mod pull_request_reviews;
mod queries;
mod types;
mod viewer;

pub use types::EventItem;
#[cfg(test)]
pub use types::EventKind;

pub(crate) use closed_issues::query_closed_issues;
pub(crate) use closed_pull_requests::query_closed_pull_requests;
pub(crate) use issue_comments::query_issue_comments;
pub(crate) use opened_issues::query_opened_issues;
pub(crate) use opened_pull_requests::query_opened_pull_requests;
pub(crate) use pull_request_reviews::query_pull_request_review_contributions;
pub(crate) use viewer::query_viewer_login;

#[cfg(test)]
mod test_support {
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, method, path},
    };

    pub(super) async fn mock_response(
        server: &MockServer,
        payload: serde_json::Value,
        response: serde_json::Value,
    ) {
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_partial_json(payload))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(1)
            .mount(server)
            .await;
    }

    pub(super) fn client(server: &MockServer) -> octocrab::Octocrab {
        octocrab::Octocrab::builder()
            .base_uri(server.uri())
            .unwrap()
            .build()
            .unwrap()
    }
}
