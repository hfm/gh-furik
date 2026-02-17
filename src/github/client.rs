use super::EventItem;
use super::auth::fetch_token;
use super::graphql;
use anyhow::Context;
use chrono::NaiveDate;

pub struct Client {
    octocrab: octocrab::Octocrab,
}

impl Client {
    pub fn new(host: &str) -> anyhow::Result<Self> {
        let token = fetch_token(host)?;
        let octocrab = build_github_client(host, token)?;
        Ok(Self { octocrab })
    }

    pub async fn query_viewer_login(&self) -> anyhow::Result<String> {
        graphql::query_viewer_login(&self.octocrab).await
    }

    pub async fn query_issue_comments(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> anyhow::Result<Vec<EventItem>> {
        graphql::query_issue_comments(&self.octocrab, Some(from), Some(to)).await
    }

    pub async fn query_pull_request_review_contributions(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> anyhow::Result<Vec<EventItem>> {
        graphql::query_pull_request_review_contributions(&self.octocrab, Some(from), Some(to)).await
    }

    pub async fn query_opened_issues(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> anyhow::Result<Vec<EventItem>> {
        graphql::query_opened_issues(&self.octocrab, Some(from), Some(to)).await
    }

    pub async fn query_opened_pull_requests(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> anyhow::Result<Vec<EventItem>> {
        graphql::query_opened_pull_requests(&self.octocrab, Some(from), Some(to)).await
    }

    pub async fn query_closed_issues(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        viewer_login: &str,
    ) -> anyhow::Result<Vec<EventItem>> {
        graphql::query_closed_issues(&self.octocrab, Some(from), Some(to), viewer_login).await
    }

    pub async fn query_closed_pull_requests(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        viewer_login: &str,
    ) -> anyhow::Result<Vec<EventItem>> {
        graphql::query_closed_pull_requests(&self.octocrab, Some(from), Some(to), viewer_login)
            .await
    }
}

fn build_github_client(host: &str, token: String) -> anyhow::Result<octocrab::Octocrab> {
    let client = octocrab::Octocrab::builder()
        .base_uri(api_base_url(host))
        .context("failed to set base URI")?
        .personal_token(token)
        .build()?;
    Ok(client)
}

fn api_base_url(host: &str) -> String {
    if host.eq_ignore_ascii_case("github.com") {
        "https://api.github.com".to_string()
    } else {
        format!("https://{host}/api")
    }
}
