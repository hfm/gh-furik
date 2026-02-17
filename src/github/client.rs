use anyhow::Context;

pub struct Client {
    octocrab: octocrab::Octocrab,
    viewer_login: String,
}

impl Client {
    pub async fn new(host: &str) -> anyhow::Result<Self> {
        let token = fetch_token(host)?;
        let octocrab = build_github_client(host, token)?;
        let viewer_login = super::graphql::query_viewer_login(&octocrab).await?;
        Ok(Self {
            octocrab,
            viewer_login,
        })
    }

    pub(crate) fn octocrab(&self) -> &octocrab::Octocrab {
        &self.octocrab
    }

    pub(crate) fn viewer_login(&self) -> &str {
        &self.viewer_login
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

fn fetch_token(host: &str) -> anyhow::Result<String> {
    if let Some(token) = token_from_env(host) {
        return Ok(token);
    }
    if let Some(token) = token_from_gh(host)? {
        return Ok(token);
    }

    anyhow::bail!(
        "token for {host} not found. Please set `GH_TOKEN` or log in with `gh auth login`."
    );
}

fn token_from_env(host: &str) -> Option<String> {
    let keys = if host.eq_ignore_ascii_case("github.com") {
        ["GH_TOKEN", "GITHUB_TOKEN"]
    } else {
        ["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"]
    };

    for key in keys {
        if let Ok(token) = std::env::var(key) {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    None
}

fn token_from_gh(host: &str) -> anyhow::Result<Option<String>> {
    let output = match std::process::Command::new("gh")
        .args(["auth", "token", "--secure-storage", "--hostname", host])
        .output()
    {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("failed to execute `gh auth token`"),
    };

    if !output.status.success() {
        return Ok(None);
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if token.is_empty() { None } else { Some(token) })
}

#[cfg(test)]
mod tests {
    use super::fetch_token;
    use temp_env::with_vars;

    #[test]
    fn token_prefers_gh_token() {
        with_vars(
            [
                ("GH_TOKEN", Some("gh-token")),
                ("GITHUB_TOKEN", Some("github-token")),
            ],
            || {
                let token = fetch_token("github.com").unwrap();
                assert_eq!(token, "gh-token");
            },
        );
    }

    #[test]
    fn fetch_token_env_differs_by_host() {
        with_vars(
            [
                ("GH_TOKEN", Some("gh-token")),
                ("GH_ENTERPRISE_TOKEN", Some("ghe-token")),
            ],
            || {
                let github_token = fetch_token("github.com").unwrap();
                assert_eq!(github_token, "gh-token");

                let ghe_token = fetch_token("ghe.example.com").unwrap();
                assert_eq!(ghe_token, "ghe-token");
            },
        );
    }

    #[test]
    fn fetch_token_skips_empty_github_token_vars() {
        with_vars(
            [
                ("GH_TOKEN", Some("")),
                ("GITHUB_TOKEN", Some("github-token")),
            ],
            || {
                let token = fetch_token("github.com").unwrap();
                assert_eq!(token, "github-token");
            },
        );
    }

    #[test]
    fn fetch_token_skips_empty_enterprise_token_vars() {
        with_vars(
            [
                ("GH_ENTERPRISE_TOKEN", Some("")),
                ("GITHUB_ENTERPRISE_TOKEN", Some("ghe-token")),
            ],
            || {
                let token = fetch_token("ghe.example.com").unwrap();
                assert_eq!(token, "ghe-token");
            },
        );
    }
}
