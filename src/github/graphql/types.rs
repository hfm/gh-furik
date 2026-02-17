#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    IssueOpened,
    IssueClosed,
    IssueComment,
    PullRequestOpened,
    PullRequestClosed,
    PullRequestMerged,
    PullRequestReview,
    PullRequestReviewComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventItem {
    pub kind: EventKind,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub url: String,
    pub body: Option<String>,
    pub repository: String,
    pub subject_title: String,
    pub subject_url: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GraphqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GraphqlError {
    pub message: String,
}
