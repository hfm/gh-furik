fn token_from_env(host: &str) -> Option<String> {
    let keys = if host.eq_ignore_ascii_case("github.com") {
        ["GH_TOKEN", "GITHUB_TOKEN"]
    } else {
        ["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"]
    };

    for key in keys {
        if let Ok(token) = std::env::var(key) {
            return Some(token);
        }
    }

    None
}

fn token_from_gh(host: &str) -> Option<String> {
    let output = std::process::Command::new("gh")
        .args(["auth", "token", "--secure-storage", "--hostname", host])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

pub(super) fn fetch_token(host: &str) -> anyhow::Result<String> {
    if let Some(token) = token_from_env(host) {
        return Ok(token);
    }
    if let Some(token) = token_from_gh(host) {
        return Ok(token);
    }

    anyhow::bail!("token for {host} not found.");
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
}
