//! Parsing and resolving user-supplied GitHub repository references, so a
//! user can bind any public repo with releases -- not just ones they're a
//! member of.

use super::client::{GithubClient, GithubError, RepoSummary};

/// Extracts `(owner, repo)` from a GitHub repository URL.
///
/// Accepts the forms people typically paste: with or without a scheme,
/// `www.`, a trailing slash, a `.git` suffix, deeper paths such as
/// `/releases` or `/tree/main`, and query strings or fragments. The bare
/// `owner/repo` shorthand is accepted too.
pub fn parse_github_repo_url(input: &str) -> Result<(String, String), String> {
    let invalid = || {
        format!(
            "\"{}\" is not a GitHub repository URL (expected https://github.com/owner/repo)",
            input.trim()
        )
    };

    let trimmed = input.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let without_fragment = without_scheme.split(['?', '#']).next().unwrap_or("");

    let mut segments = without_fragment.split('/').filter(|s| !s.is_empty());
    let first = segments.next().ok_or_else(invalid)?;
    let host_given = first.contains('.');
    let owner = if host_given {
        let host = first.to_ascii_lowercase();
        if host != "github.com" && host != "www.github.com" {
            return Err(invalid());
        }
        segments.next().ok_or_else(invalid)?
    } else {
        first
    };
    let repo = segments.next().ok_or_else(invalid)?;
    // Shorthand must be exactly `owner/repo`; a full URL may go deeper.
    if !host_given && segments.next().is_some() {
        return Err(invalid());
    }
    let repo = repo.strip_suffix(".git").unwrap_or(repo);

    let valid_owner =
        !owner.is_empty() && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let valid_repo = !repo.is_empty()
        && repo != "."
        && repo != ".."
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !valid_owner || !valid_repo {
        return Err(invalid());
    }

    Ok((owner.to_string(), repo.to_string()))
}

/// Resolves a pasted GitHub URL to a repo that can be bound: it must exist,
/// be visible to the token (every public repo is), and have at least one
/// release. Errors are phrased for display to the user.
pub async fn resolve_bindable_repo(
    client: &GithubClient,
    url: &str,
) -> Result<RepoSummary, String> {
    let (owner, repo) = parse_github_repo_url(url)?;

    let summary = match client.get_repo(&owner, &repo).await {
        Ok(summary) => summary,
        // GitHub answers 404 for private repos the token can't see, so the
        // two cases are indistinguishable here.
        Err(GithubError::Api { status: 404, .. }) => {
            return Err(format!(
                "Repository {owner}/{repo} was not found. Check the URL, and that the repository is public."
            ))
        }
        Err(e) => return Err(e.to_string()),
    };

    let has_releases = client
        .has_any_release(&summary.owner.login, &summary.name)
        .await
        .map_err(|e| e.to_string())?;
    if !has_releases {
        return Err(format!(
            "Repository {} has no releases to pull builds from.",
            summary.full_name
        ));
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn mock_repo(server: &MockServer, full_name: &str, releases: serde_json::Value) {
        let (owner, name) = full_name.split_once('/').unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/repos/{full_name}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "name": name, "full_name": full_name, "owner": { "login": owner } }
            )))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{full_name}/releases")))
            .respond_with(ResponseTemplate::new(200).set_body_json(releases))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn resolve_bindable_repo_returns_a_repo_with_releases() {
        let server = MockServer::start().await;
        mock_repo(
            &server,
            "some-org/public-game",
            serde_json::json!([
                { "id": 1, "tag_name": "1.0", "name": null, "prerelease": false, "published_at": null, "assets": [] }
            ]),
        )
        .await;
        let client = GithubClient::with_base_url("token123".to_string(), server.uri());

        let repo =
            resolve_bindable_repo(&client, "https://github.com/some-org/public-game/releases")
                .await
                .unwrap();

        assert_eq!(repo.full_name, "some-org/public-game");
    }

    #[tokio::test]
    async fn resolve_bindable_repo_rejects_a_repo_without_releases() {
        let server = MockServer::start().await;
        mock_repo(&server, "some-org/no-releases", serde_json::json!([])).await;
        let client = GithubClient::with_base_url("token123".to_string(), server.uri());

        let err = resolve_bindable_repo(&client, "https://github.com/some-org/no-releases")
            .await
            .unwrap_err();

        assert!(err.contains("has no releases"), "{err}");
    }

    #[tokio::test]
    async fn resolve_bindable_repo_reports_a_missing_repo_clearly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/nobody/nothing"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&server)
            .await;
        let client = GithubClient::with_base_url("token123".to_string(), server.uri());

        let err = resolve_bindable_repo(&client, "https://github.com/nobody/nothing")
            .await
            .unwrap_err();

        assert!(err.contains("nobody/nothing was not found"), "{err}");
    }

    #[tokio::test]
    async fn resolve_bindable_repo_rejects_an_unparseable_url_without_calling_github() {
        let client =
            GithubClient::with_base_url("token123".to_string(), "http://127.0.0.1:1".to_string());

        let err = resolve_bindable_repo(&client, "https://gitlab.com/owner/repo")
            .await
            .unwrap_err();

        assert!(err.contains("not a GitHub repository URL"), "{err}");
    }

    fn parsed(input: &str) -> (String, String) {
        parse_github_repo_url(input).unwrap()
    }

    fn pair(owner: &str, repo: &str) -> (String, String) {
        (owner.to_string(), repo.to_string())
    }

    #[test]
    fn parses_a_plain_https_url() {
        assert_eq!(
            parsed("https://github.com/owner/repo"),
            pair("owner", "repo")
        );
    }

    #[test]
    fn tolerates_common_url_variations() {
        for input in [
            "http://github.com/owner/repo",
            "https://www.github.com/owner/repo",
            "https://GitHub.com/owner/repo",
            "github.com/owner/repo",
            "https://github.com/owner/repo/",
            "https://github.com/owner/repo.git",
            "https://github.com/owner/repo/releases",
            "https://github.com/owner/repo/tree/main/src",
            "https://github.com/owner/repo?tab=readme#install",
            "  https://github.com/owner/repo  ",
        ] {
            assert_eq!(parsed(input), pair("owner", "repo"), "input: {input}");
        }
    }

    #[test]
    fn accepts_owner_repo_shorthand() {
        assert_eq!(parsed("owner/repo"), pair("owner", "repo"));
    }

    #[test]
    fn keeps_dots_underscores_and_dashes_in_repo_names() {
        assert_eq!(
            parsed("https://github.com/my-org/my_game.v2"),
            pair("my-org", "my_game.v2")
        );
    }

    #[test]
    fn rejects_non_github_hosts() {
        assert!(parse_github_repo_url("https://gitlab.com/owner/repo").is_err());
        assert!(parse_github_repo_url("https://github.com.evil.io/owner/repo").is_err());
    }

    #[test]
    fn rejects_urls_missing_the_repo_segment() {
        assert!(parse_github_repo_url("").is_err());
        assert!(parse_github_repo_url("https://github.com").is_err());
        assert!(parse_github_repo_url("https://github.com/owner").is_err());
        assert!(parse_github_repo_url("owner").is_err());
    }

    #[test]
    fn rejects_shorthand_with_extra_segments() {
        assert!(parse_github_repo_url("owner/repo/extra").is_err());
    }

    #[test]
    fn rejects_characters_github_does_not_allow() {
        assert!(parse_github_repo_url("https://github.com/own er/repo").is_err());
        assert!(parse_github_repo_url("https://github.com/owner/re%70o").is_err());
        assert!(parse_github_repo_url("https://github.com/owner/..").is_err());
    }
}
