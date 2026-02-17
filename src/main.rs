mod formatter;
mod github;
use crate::github::prelude::*;
use clap::Parser;
use futures::future::try_join_all;

#[derive(clap::Parser, Debug)]
#[command(version, about = "GitHub activity digger")]
struct Cli {
    #[arg(
        long,
        value_parser = parse_date,
        value_name = "YYYY-MM-DD",
        help = "Start date",
        default_value_t = today()
    )]
    from: chrono::NaiveDate,
    #[arg(
        long,
        value_parser = parse_date,
        value_name = "YYYY-MM-DD",
        help = "End date",
        default_value_t = today()
    )]
    to: chrono::NaiveDate,
    #[arg(
        long,
        value_name = "HOST[,HOST...]",
        value_delimiter = ',',
        value_parser = parse_hostname,
        default_value = "github.com",
        help = "Target GitHub hostname",
        env = "GH_HOST"
    )]
    hostname: Vec<String>,
    #[arg(short, long, help = "Use compact list output")]
    compact: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli {
        from,
        to,
        hostname,
        compact,
    } = Cli::parse();

    let results = try_join_all(
        hostname
            .into_iter()
            .map(|host| fetch_events_for_host(host, from, to)),
    )
    .await?;
    let output = format_host_outputs(&results, compact);

    print!("{output}");

    Ok(())
}

async fn fetch_events_for_host(
    hostname: String,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<(String, Vec<EventItem>)> {
    let client = Client::new(&hostname).await?;
    let items = fetch_all_events(&client, from, to).await?;
    Ok((hostname, items))
}

async fn fetch_all_events(
    client: &Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<EventItem>> {
    let (
        issue_comments,
        review_contributions,
        opened_issues,
        opened_prs,
        closed_issues,
        closed_prs,
    ) = tokio::try_join!(
        query_issue_comments(client, from, to),
        query_pull_request_review_contributions(client, from, to),
        query_opened_issues(client, from, to),
        query_opened_pull_requests(client, from, to),
        query_closed_issues(client, from, to),
        query_closed_pull_requests(client, from, to),
    )?;

    let items: Vec<_> = [
        issue_comments,
        review_contributions,
        opened_issues,
        opened_prs,
        closed_issues,
        closed_prs,
    ]
    .into_iter()
    .flatten()
    .collect();

    Ok(items)
}

fn parse_date(input: &str) -> anyhow::Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")
}

fn today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

fn parse_hostname(input: &str) -> Result<String, String> {
    let host = input.trim();
    if host.is_empty() {
        return Err("hostname must not be empty".to_string());
    }
    Ok(host.to_string())
}

fn format_host_outputs(results: &[(String, Vec<EventItem>)], compact: bool) -> String {
    let sections: Vec<String> = results
        .iter()
        .map(|(hostname, items)| {
            crate::formatter::format_markdown(hostname, items, compact)
                .trim_end_matches('\n')
                .to_string()
        })
        .collect();
    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hostname_single() {
        let host = parse_hostname("github.com").unwrap();
        assert_eq!(host, "github.com");
    }

    #[test]
    fn parse_hostname_trims_spaces() {
        let host = parse_hostname(" ghe.example.com ").unwrap();
        assert_eq!(host, "ghe.example.com");
    }

    #[test]
    fn parse_hostname_rejects_empty_input() {
        let error = parse_hostname(" ").unwrap_err();
        assert!(error.contains("hostname must not be empty"));
    }

    #[test]
    fn format_host_outputs_preserves_input_order() {
        let output = format_host_outputs(
            &[
                ("github.com".to_string(), vec![]),
                ("ghe.example.com".to_string(), vec![]),
            ],
            false,
        );

        let github_index = output.find("# github.com").unwrap();
        let ghe_index = output.find("# ghe.example.com").unwrap();
        assert!(github_index < ghe_index);
    }

    #[test]
    fn format_host_outputs_has_single_blank_line_between_hosts() {
        let output = format_host_outputs(
            &[
                ("github.com".to_string(), vec![]),
                ("ghe.example.com".to_string(), vec![]),
            ],
            false,
        );

        assert!(output.contains("_No activity found._\n\n# ghe.example.com"));
        assert!(!output.contains("_No activity found._\n\n\n# ghe.example.com"));
    }
}
