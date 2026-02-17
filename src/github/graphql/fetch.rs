use anyhow::Context;
use chrono::TimeZone;
use std::time::Duration;

use valq::query_value;

use super::queries::{QueryKind, SEARCH_COUNT_QUERY, SEARCH_QUERY};
use super::types::*;

pub(super) const MAX_PAGES: usize = 1000;
const SEARCH_LIMIT: i32 = 1000;
const SEARCH_RETRIES: usize = 3;

pub(super) fn event_items_from_search_node(
    node: &serde_json::Value,
    viewer_login: &str,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
) -> Vec<EventItem> {
    let mut items = Vec::new();

    let typename = query_value!(node["__typename"] -> str).expect("search node missing __typename");
    let url = query_value!(node.url -> str).expect("search node missing url");
    let title = query_value!(node.title -> str).expect("search node missing title");
    let repository = query_value!(node.repository["nameWithOwner"] -> str)
        .expect("search node missing repository nameWithOwner");

    let timeline_nodes = query_value!(node.timelineItems.nodes -> array);

    if let Some(nodes) = timeline_nodes {
        for node in nodes.iter().filter(|node| !node.is_null()) {
            let event_type =
                query_value!(node["__typename"] -> str).expect("timeline node missing __typename");
            let actor_login = query_value!(node.actor.login -> str);
            if !actor_matches(actor_login, viewer_login) {
                continue;
            }
            let created_at = parse_datetime(
                query_value!(node["createdAt"] -> str).expect("timeline node missing createdAt"),
            )
            .expect("timeline node invalid createdAt");
            if !in_range(created_at, from, to) {
                continue;
            }

            match event_type {
                "ClosedEvent" => {
                    let kind = if typename == "Issue" {
                        EventKind::IssueClosed
                    } else {
                        EventKind::PullRequestClosed
                    };
                    items.push(EventItem {
                        kind,
                        created_at,
                        url: url.to_string(),
                        body: None,
                        repository: repository.to_string(),
                        subject_title: title.to_string(),
                        subject_url: url.to_string(),
                    });
                }
                "MergedEvent" if typename == "PullRequest" => {
                    items.push(EventItem {
                        kind: EventKind::PullRequestMerged,
                        created_at,
                        url: url.to_string(),
                        body: None,
                        repository: repository.to_string(),
                        subject_title: title.to_string(),
                        subject_url: url.to_string(),
                    });
                }
                _ => {}
            }
        }
    }

    items
}

fn actor_matches(actor_login: Option<&str>, viewer_login: &str) -> bool {
    actor_login
        .map(|actor_login| actor_login == viewer_login)
        .unwrap_or(false)
}

pub(super) async fn fetch_search_nodes_range(
    client: &octocrab::Octocrab,
    query_base: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut ranges = Vec::new();
    ranges.push((from, to));

    let mut out = Vec::new();
    while let Some((start, end)) = ranges.pop() {
        if start > end {
            continue;
        }

        let query = search_query(query_base, start, end);
        let count = fetch_search_count(client, &query).await?;
        if count == 0 {
            continue;
        }

        if count > SEARCH_LIMIT {
            if start == end {
                eprintln!("Too many results from={} to={} count={}", start, end, count);
                out.extend(fetch_search_nodes(client, &query).await?);
                continue;
            }

            let mid = midpoint_date(start, end);
            if let Some(next_day) = mid.succ_opt() {
                ranges.push((next_day, end));
            }
            ranges.push((start, mid));
            continue;
        }

        out.extend(fetch_search_nodes(client, &query).await?);
    }

    Ok(out)
}

async fn graphql_with_retry<T>(
    client: &octocrab::Octocrab,
    payload: &serde_json::Value,
    context: &'static str,
) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    for attempt in 1..=SEARCH_RETRIES {
        match client.graphql::<T>(payload).await {
            Ok(resp) => return Ok(resp),
            Err(err) => {
                if attempt == SEARCH_RETRIES {
                    return Err(anyhow::Error::new(err)).context(context);
                }
                let backoff = 200u64.saturating_mul(1 << (attempt - 1));
                eprintln!(
                    "search query failed; retrying attempt={} error={}",
                    attempt, err
                );
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }
        }
    }

    unreachable!("SEARCH_RETRIES must be >= 1");
}

async fn fetch_search_count(client: &octocrab::Octocrab, query: &str) -> anyhow::Result<i32> {
    let payload = serde_json::json!({
        "query": SEARCH_COUNT_QUERY,
        "variables": { "query": query },
    });

    let resp = graphql_with_retry::<GraphqlResponse<serde_json::Value>>(
        client,
        &payload,
        "GraphQL search query failed",
    )
    .await?;

    let data = graphql_data(resp)?;
    let issue_count = data
        .get("search")
        .and_then(|search| search.get("issueCount"))
        .and_then(|count| count.as_i64())
        .expect("search response missing issueCount");
    Ok(issue_count as i32)
}

async fn fetch_search_nodes(
    client: &octocrab::Octocrab,
    query: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut after: Option<String> = None;
    let mut out = Vec::new();

    for _ in 0..MAX_PAGES {
        let payload = serde_json::json!({
            "query": SEARCH_QUERY,
            "variables": { "query": query, "after": after.clone() },
        });

        let resp = graphql_with_retry::<GraphqlResponse<serde_json::Value>>(
            client,
            &payload,
            "GraphQL search query failed",
        )
        .await?;

        let data = graphql_data(resp)?;
        let search = data.get("search").expect("search response missing search");
        if let Some(nodes) = search.get("nodes").and_then(|nodes| nodes.as_array()) {
            out.extend(nodes.iter().filter(|node| !node.is_null()).cloned());
        }

        let page_info = search
            .get("pageInfo")
            .expect("search response missing pageInfo");
        let has_next_page = page_info
            .get("hasNextPage")
            .and_then(|value| value.as_bool())
            .expect("search response missing pageInfo.hasNextPage");
        let end_cursor = page_info
            .get("endCursor")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        if !has_next_page {
            break;
        }
        after = end_cursor;
        if after.is_none() {
            break;
        }
    }

    Ok(out)
}

fn search_query(query_base: &str, from: chrono::NaiveDate, to: chrono::NaiveDate) -> String {
    format!(
        "{query_base} involves:@me closed:{}..{}",
        from.format("%Y-%m-%d"),
        to.format("%Y-%m-%d")
    )
}

fn midpoint_date(from: chrono::NaiveDate, to: chrono::NaiveDate) -> chrono::NaiveDate {
    let days = (to - from).num_days();
    from + chrono::Duration::days(days / 2)
}

pub(super) fn normalize_range(
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
) -> (chrono::NaiveDate, chrono::NaiveDate) {
    let start = from.unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let end = to.unwrap_or_else(|| chrono::Utc::now().date_naive());
    (start, end)
}

pub(super) fn issue_since(from: Option<chrono::NaiveDate>) -> Option<String> {
    let from = from?;
    let start = from.and_hms_opt(0, 0, 0)?;
    Some(chrono::Utc.from_utc_datetime(&start).to_rfc3339())
}

pub(super) fn graphql_data<T>(resp: GraphqlResponse<T>) -> anyhow::Result<T> {
    if let Some(errors) = resp.errors {
        let msg = errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!("GraphQL returned errors: {msg}");
    }
    resp.data.context("GraphQL response missing data")
}

pub(super) fn in_range(
    dt: chrono::DateTime<chrono::Utc>,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
) -> bool {
    let date = dt.date_naive();
    if let Some(from) = from
        && date < from
    {
        return false;
    }
    if let Some(to) = to
        && date > to
    {
        return false;
    }
    true
}

pub(super) fn parse_datetime(value: &str) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
    let dt = chrono::DateTime::parse_from_rfc3339(value).context("invalid datetime")?;
    Ok(dt.with_timezone(&chrono::Utc))
}

pub(super) async fn fetch_paginated_json<F, S>(
    client: &octocrab::Octocrab,
    query: QueryKind,
    map: F,
    should_stop: S,
) -> anyhow::Result<Vec<EventItem>>
where
    F: Fn(&serde_json::Value) -> anyhow::Result<Option<EventItem>> + Copy,
    S: Fn(&serde_json::Value) -> anyhow::Result<bool> + Copy,
{
    let mut after: Option<String> = None;
    let mut out = Vec::new();

    for _ in 0..MAX_PAGES {
        let payload = serde_json::json!({
            "query": query.as_str(),
            "variables": query.variables(after.clone()),
        });

        let resp = client
            .graphql::<GraphqlResponse<serde_json::Value>>(&payload)
            .await
            .context("GraphQL query failed")?;

        let data = graphql_data(resp)?;
        let connection = data
            .get("viewer")
            .and_then(|viewer| viewer.get(query.connection_field()))
            .expect("GraphQL response missing connection");
        let page_info = connection
            .get("pageInfo")
            .expect("GraphQL response missing pageInfo");
        let has_next_page = page_info
            .get("hasNextPage")
            .and_then(|value| value.as_bool())
            .expect("GraphQL response missing pageInfo.hasNextPage");
        let end_cursor = page_info
            .get("endCursor")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        if let Some(nodes) = connection.get("nodes").and_then(|value| value.as_array()) {
            for node in nodes.iter().filter(|node| !node.is_null()) {
                if should_stop(node)? {
                    return Ok(out);
                }
                if let Some(item) = map(node)? {
                    out.push(item);
                }
            }
        }

        if !has_next_page {
            break;
        }
        after = end_cursor;
        if after.is_none() {
            break;
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    fn dt(date: &str) -> chrono::DateTime<chrono::Utc> {
        Utc.with_ymd_and_hms(
            date[0..4].parse().unwrap(),
            date[5..7].parse().unwrap(),
            date[8..10].parse().unwrap(),
            0,
            0,
            0,
        )
        .unwrap()
    }

    #[test]
    fn in_range_allows_open_bounds() {
        assert!(in_range(dt("2025-01-02"), None, None));
        assert!(in_range(
            dt("2025-01-02"),
            Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            None
        ));
        assert!(in_range(
            dt("2025-01-02"),
            None,
            Some(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap())
        ));
    }

    #[test]
    fn in_range_rejects_outside_bounds() {
        assert!(!in_range(
            dt("2024-12-31"),
            Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            None
        ));
        assert!(!in_range(
            dt("2025-02-01"),
            None,
            Some(NaiveDate::from_ymd_opt(2025, 1, 31).unwrap())
        ));
    }
}
