mod client;
mod graphql;

pub use client::Client;
pub use graphql::EventItem;
#[cfg(test)]
pub use graphql::EventKind;
pub(crate) use graphql::{
    query_closed_issues, query_closed_pull_requests, query_issue_comments, query_opened_issues,
    query_opened_pull_requests, query_pull_request_review_contributions,
};

pub(crate) mod prelude {
    pub use super::Client;
    pub use super::EventItem;
    pub(crate) use super::{
        query_closed_issues, query_closed_pull_requests, query_issue_comments, query_opened_issues,
        query_opened_pull_requests, query_pull_request_review_contributions,
    };
}
