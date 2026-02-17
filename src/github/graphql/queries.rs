use serde_json::Value;

pub(crate) const ISSUE_COMMENTS_QUERY: &str = include_str!("queries/issue_comments.graphql");
pub(crate) const OPENED_ISSUES_SINCE_QUERY: &str =
    include_str!("queries/opened_issues_since.graphql");
pub(crate) const OPENED_PULL_REQUESTS_QUERY: &str =
    include_str!("queries/opened_pull_requests.graphql");

pub(crate) const REVIEW_CONTRIBUTIONS_QUERY: &str =
    include_str!("queries/review_contributions.graphql");
pub(crate) const SEARCH_QUERY: &str = include_str!("queries/search.graphql");
pub(crate) const SEARCH_COUNT_QUERY: &str = include_str!("queries/search_count.graphql");

pub(crate) enum QueryKind {
    IssueComments,
    OpenedIssues { since: String },
    OpenedPullRequests,
}

impl QueryKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            QueryKind::IssueComments => ISSUE_COMMENTS_QUERY,
            QueryKind::OpenedIssues { .. } => OPENED_ISSUES_SINCE_QUERY,
            QueryKind::OpenedPullRequests => OPENED_PULL_REQUESTS_QUERY,
        }
    }

    pub(crate) fn variables(&self, after: Option<String>) -> Value {
        match self {
            QueryKind::OpenedIssues { since } => {
                serde_json::json!({ "after": after, "since": since })
            }
            _ => serde_json::json!({ "after": after }),
        }
    }

    pub(crate) fn connection_field(&self) -> &'static str {
        match self {
            QueryKind::IssueComments => "issueComments",
            QueryKind::OpenedIssues { .. } => "issues",
            QueryKind::OpenedPullRequests => "pullRequests",
        }
    }
}
