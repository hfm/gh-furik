use crate::github::{EventItem, EventKind};

const COMMENT_PREVIEW_MAX_LEN: usize = 80;

pub fn format_markdown(host: &str, items: &[EventItem]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {host}\n\n"));

    if items.is_empty() {
        out.push_str("_No activity found._\n");
        return out;
    }

    let mut sorted: Vec<&EventItem> = items.iter().collect();
    sorted.sort_by(|a, b| {
        a.repository
            .cmp(&b.repository)
            .then(a.subject_url.cmp(&b.subject_url))
            .then(b.created_at.cmp(&a.created_at))
    });

    let mut current_repo: Option<&str> = None;
    let mut current_subject: Option<&str> = None;

    for item in sorted {
        if current_repo != Some(item.repository.as_str()) {
            current_repo = Some(item.repository.as_str());
            current_subject = None;
            if !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&format!("## {}\n\n", item.repository));
        }

        if current_subject != Some(item.subject_url.as_str()) {
            current_subject = Some(item.subject_url.as_str());
            if !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&format!(
                "### {} {}\n\n",
                item.subject_title, item.subject_url
            ));
        }

        let date = item.created_at.date_naive();
        out.push_str(&format!(
            "- {date} ({})\n  - {}: {}\n",
            kind_label(&item.kind),
            action_label(&item.kind),
            item.url
        ));

        if let Some(body) = item.body.as_ref()
            && let Some(line) = first_line_preview(body)
        {
            out.push_str("  > ");
            out.push_str(&line);
            out.push('\n');
        }
    }

    out
}

fn first_line_preview(body: &str) -> Option<String> {
    let line = body.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    if line.chars().count() <= COMMENT_PREVIEW_MAX_LEN {
        return Some(line.to_string());
    }
    let mut out: String = line.chars().take(COMMENT_PREVIEW_MAX_LEN).collect();
    out.push_str("...");
    Some(out)
}

fn kind_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::IssueComment => "IssueComment",
        EventKind::PullRequestReviewComment => "PullRequestReviewComment",
        EventKind::PullRequestReview => "PullRequestReview",
        EventKind::IssueOpened => "IssueOpened",
        EventKind::PullRequestOpened => "PullRequestOpened",
        EventKind::IssueClosed => "IssueClosed",
        EventKind::PullRequestClosed => "PullRequestClosed",
        EventKind::PullRequestMerged => "PullRequestMerged",
    }
}

fn action_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::IssueComment
        | EventKind::PullRequestReviewComment
        | EventKind::PullRequestReview => "Comment",
        EventKind::IssueOpened | EventKind::PullRequestOpened => "Opened",
        EventKind::IssueClosed | EventKind::PullRequestClosed => "Closed",
        EventKind::PullRequestMerged => "Merged",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_item() -> EventItem {
        EventItem {
            kind: EventKind::IssueComment,
            created_at: chrono::Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            url: "https://example.test/comment/1".to_string(),
            body: Some("hello\nworld".to_string()),
            repository: "o/r".to_string(),
            subject_title: "Issue A".to_string(),
            subject_url: "https://example.test/issue/1".to_string(),
        }
    }

    #[test]
    fn format_markdown_empty() {
        let out = format_markdown("github.com", &[]);
        assert!(out.contains("_No activity found._"));
    }

    #[test]
    fn format_markdown_single_item() {
        let item = sample_item();
        let out = format_markdown("github.com", &[item]);
        assert!(out.contains("# github.com"));
        assert!(out.contains("## o/r"));
        assert!(out.contains("### Issue A https://example.test/issue/1"));
        assert!(out.contains("Comment: https://example.test/comment/1"));
        assert!(out.contains("> hello"));
        assert!(!out.contains("> world"));
    }
}
