mod formatter;
mod github;
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli { from, to, hostname } = Cli::parse();

    let client = crate::github::Client::new(&hostname)?;
    let items = fetch_all_events(&client, from, to).await?;
    let output = crate::formatter::format_markdown(&hostname, &items);

    print!("{output}");

    Ok(())
}

async fn fetch_all_events(
    client: &crate::github::Client,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> anyhow::Result<Vec<crate::github::EventItem>> {
    let viewer_login = client.query_viewer_login().await?;

    let (
        issue_comments,
        review_contributions,
        opened_issues,
        opened_prs,
        closed_issues,
        closed_prs,
    ) = tokio::try_join!(
        client.query_issue_comments(from, to),
        client.query_pull_request_review_contributions(from, to),
        client.query_opened_issues(from, to),
        client.query_opened_pull_requests(from, to),
        client.query_closed_issues(from, to, &viewer_login),
        client.query_closed_pull_requests(from, to, &viewer_login),
    )?;

    let mut items: Vec<_> = [
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
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(items)
}

fn parse_date(input: &str) -> anyhow::Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")
}

fn today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}
