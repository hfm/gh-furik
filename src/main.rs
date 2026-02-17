mod formatter;
mod github;
use crate::github::prelude::*;
use clap::Parser;

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
        value_name = "HOST",
        default_value = "github.com",
        help = "Target GitHub hostname",
        env = "GH_HOST"
    )]
    hostname: String,
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

    let client = Client::new(&hostname).await?;
    let items = fetch_all_events(&client, from, to).await?;
    let output = crate::formatter::format_markdown(&hostname, &items, compact);

    print!("{output}");

    Ok(())
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
