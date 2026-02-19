use crate::github::EventItem;

const COMMENT_PREVIEW_MAX_LEN: usize = 80;

pub fn format_markdown(host: &str, items: &[EventItem], compact: bool) -> String {
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
            .then(a.created_at.cmp(&b.created_at))
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
            if !compact && !out.ends_with("\n\n") {
                out.push('\n');
            }
            if compact {
                out.push_str(&format!("- {} {}\n", item.subject_title, item.subject_url));
            } else {
                out.push_str(&format!(
                    "### {} {}\n\n",
                    item.subject_title, item.subject_url
                ));
            }
        }

        let date = item.created_at.date_naive();
        let action_label = item.kind.action_label();
        if compact {
            if should_include_event_url(action_label) {
                out.push_str(&format!("  - {date} {} {}\n", action_label, item.url));
            } else {
                out.push_str(&format!("  - {date} {}\n", action_label));
            }
        } else if should_include_event_url(action_label) {
            out.push_str(&format!("- {date} {} {}\n", action_label, item.url));
        } else {
            out.push_str(&format!("- {date} {}\n", action_label));
        }

        if let Some(body) = item.body.as_ref()
            && let Some(preview) = body_preview(
                body,
                preview_line_limit(action_label),
                if compact { "    > " } else { "  > " },
            )
        {
            out.push_str(&preview);
            out.push('\n');
        }
    }

    out
}

fn body_preview(body: &str, max_lines: usize, line_prefix: &str) -> Option<String> {
    if max_lines == 0 {
        return None;
    }

    let mut lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(max_lines + 1)
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return None;
    }

    let has_more = lines.len() > max_lines;
    if has_more {
        lines.truncate(max_lines);
    }

    let last_index = lines.len() - 1;
    for (index, line) in lines.iter_mut().enumerate() {
        let is_last = index == last_index;
        if line.chars().count() > COMMENT_PREVIEW_MAX_LEN {
            let mut out: String = line.chars().take(COMMENT_PREVIEW_MAX_LEN).collect();
            out.push_str("...");
            *line = out;
        } else if has_more && is_last {
            line.push_str(" ...");
        }
    }

    Some(
        lines
            .into_iter()
            .map(|line| format!("{line_prefix}{line}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn preview_line_limit(action_label: &str) -> usize {
    if action_label == "Opened" { 3 } else { 1 }
}

fn should_include_event_url(action_label: &str) -> bool {
    !matches!(action_label, "Opened" | "Closed" | "Merged")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::EventKind;
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
        let out = format_markdown("github.com", &[], false);
        assert!(out.contains("_No activity found._"));
    }

    #[test]
    fn format_markdown_single_item() {
        let item = sample_item();
        let out = format_markdown("github.com", &[item], false);
        assert!(out.contains("# github.com"));
        assert!(out.contains("## o/r"));
        assert!(out.contains("### Issue A https://example.test/issue/1"));
        assert!(out.contains("- 2025-01-01 Comment https://example.test/comment/1"));
        assert!(out.contains("> hello"));
        assert!(!out.contains("> world"));
    }

    #[test]
    fn format_markdown_compact_single_item() {
        let item = sample_item();
        let out = format_markdown("github.com", &[item], true);
        assert!(out.contains("# github.com"));
        assert!(out.contains("## o/r"));
        assert!(out.contains("- Issue A https://example.test/issue/1"));
        assert!(out.contains("  - 2025-01-01 Comment https://example.test/comment/1"));
        assert!(out.contains("    > hello"));
        assert!(!out.contains("> world"));
    }

    #[test]
    fn format_markdown_merged_event_omits_event_url() {
        let item = EventItem {
            kind: EventKind::PullRequestMerged,
            created_at: chrono::Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(),
            url: "https://example.test/pr-event/1".to_string(),
            body: None,
            repository: "o/r".to_string(),
            subject_title: "PR A".to_string(),
            subject_url: "https://example.test/pull/1".to_string(),
        };
        let out = format_markdown("github.com", &[item], false);

        assert!(out.contains("### PR A https://example.test/pull/1"));
        assert!(out.contains("- 2025-01-02 Merged\n"));
        assert!(!out.contains("- 2025-01-02 Merged https://example.test/pr-event/1"));
    }

    #[test]
    fn format_markdown_compact_merged_event_omits_event_url() {
        let item = EventItem {
            kind: EventKind::PullRequestMerged,
            created_at: chrono::Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(),
            url: "https://example.test/pr-event/1".to_string(),
            body: None,
            repository: "o/r".to_string(),
            subject_title: "PR A".to_string(),
            subject_url: "https://example.test/pull/1".to_string(),
        };
        let out = format_markdown("github.com", &[item], true);

        assert!(out.contains("- PR A https://example.test/pull/1"));
        assert!(out.contains("  - 2025-01-02 Merged\n"));
        assert!(!out.contains("  - 2025-01-02 Merged https://example.test/pr-event/1"));
    }

    #[test]
    fn format_markdown_opened_pr_shows_body_preview_without_event_url() {
        let item = EventItem {
            kind: EventKind::PullRequestOpened,
            created_at: chrono::Utc.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap(),
            url: "https://example.test/pr-event/2".to_string(),
            body: Some(
                "description line 1\ndescription line 2\ndescription line 3\ndescription line 4"
                    .to_string(),
            ),
            repository: "o/r".to_string(),
            subject_title: "PR B".to_string(),
            subject_url: "https://example.test/pull/2".to_string(),
        };
        let out = format_markdown("github.com", &[item], false);

        assert!(out.contains("- 2025-01-03 Opened\n"));
        assert!(!out.contains("- 2025-01-03 Opened https://example.test/pr-event/2"));
        assert!(out.contains("  > description line 1"));
        assert!(out.contains("  > description line 2"));
        assert!(out.contains("  > description line 3 ..."));
        assert!(!out.contains("description line 4"));
    }

    #[test]
    fn format_markdown_compact_opened_pr_keeps_preview_indentation_for_all_lines() {
        let item = EventItem {
            kind: EventKind::PullRequestOpened,
            created_at: chrono::Utc.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap(),
            url: "https://example.test/pr-event/2".to_string(),
            body: Some("line 1\nline 2\nline 3".to_string()),
            repository: "o/r".to_string(),
            subject_title: "PR B".to_string(),
            subject_url: "https://example.test/pull/2".to_string(),
        };
        let out = format_markdown("github.com", &[item], true);

        assert!(out.contains("    > line 1\n    > line 2\n    > line 3"));
        assert!(!out.contains("\n  > line 2"));
    }
}
