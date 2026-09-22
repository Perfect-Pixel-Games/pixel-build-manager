use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RepoSummary {
    pub name: String,
    pub full_name: String,
    pub owner: RepoOwner,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RepoOwner {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseSummary {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("github api error ({status}): {body}")]
    Api { status: u16, body: String },
}

#[derive(Debug, Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl GithubClient {
    pub fn new(token: String) -> Self {
        Self::with_base_url(token, "https://api.github.com".to_string())
    }

    pub fn with_base_url(token: String, base_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            token,
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// The authenticated asset-content API URL for downloading a release
    /// asset's bytes. Deliberately not `asset.browser_download_url` -- that
    /// URL only works for an interactive browser session (or a fully public
    /// repo); for a private repo it 404s without one, since GitHub masks
    /// private-resource existence rather than returning 401/403.
    pub fn asset_download_url(&self, owner: &str, repo: &str, asset_id: u64) -> String {
        format!(
            "{}/repos/{}/{}/releases/assets/{}",
            self.base_url, owner, repo, asset_id
        )
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "pixel-build-manager")
    }

    async fn ensure_success(response: reqwest::Response) -> Result<reqwest::Response, GithubError> {
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api { status, body });
        }
        Ok(response)
    }

    pub async fn list_accessible_repos(&self) -> Result<Vec<RepoSummary>, GithubError> {
        let mut repos = Vec::new();
        let mut page = 1;

        loop {
            let path = format!(
                "/user/repos?per_page=100&page={}&affiliation=owner,collaborator,organization_member",
                page
            );
            let response = self.request(reqwest::Method::GET, &path).send().await?;
            let response = Self::ensure_success(response).await?;

            let batch: Vec<RepoSummary> = response.json().await?;
            let is_last_page = batch.len() < 100;
            repos.extend(batch);

            if is_last_page {
                break;
            }
            page += 1;
        }

        Ok(repos)
    }

    /// Lists accessible repos, filtered down to only those that have
    /// published at least one release (including pre-releases). Repos are
    /// checked concurrently. A repo whose release check itself fails (e.g. a
    /// transient error) is excluded rather than failing the whole listing --
    /// one flaky repo shouldn't block browsing the rest.
    pub async fn list_accessible_repos_with_releases(
        &self,
    ) -> Result<Vec<RepoSummary>, GithubError> {
        let repos = self.list_accessible_repos().await?;

        let has_releases = futures_util::future::join_all(
            repos
                .iter()
                .map(|repo| self.has_any_release(&repo.owner.login, &repo.name)),
        )
        .await;

        Ok(repos
            .into_iter()
            .zip(has_releases)
            .filter_map(|(repo, has_releases)| has_releases.unwrap_or(false).then_some(repo))
            .collect())
    }

    pub async fn has_any_release(&self, owner: &str, repo: &str) -> Result<bool, GithubError> {
        let path = format!("/repos/{}/{}/releases?per_page=1", owner, repo);
        let response = self.request(reqwest::Method::GET, &path).send().await?;
        let response = Self::ensure_success(response).await?;
        let releases: Vec<serde_json::Value> = response.json().await?;
        Ok(!releases.is_empty())
    }

    pub async fn list_releases(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<ReleaseSummary>, GithubError> {
        let path = format!("/repos/{}/{}/releases?per_page=100", owner, repo);
        let response = self.request(reqwest::Method::GET, &path).send().await?;
        let response = Self::ensure_success(response).await?;

        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn list_accessible_repos_returns_parsed_repos() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/repos"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "last-beacon", "full_name": "pixel-perfect/last-beacon", "owner": { "login": "pixel-perfect" } }
            ])))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());
        let repos = client.list_accessible_repos().await.unwrap();

        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "pixel-perfect/last-beacon");
    }

    #[tokio::test]
    async fn list_accessible_repos_paginates_until_a_short_page() {
        let server = MockServer::start().await;
        let full_page: Vec<_> = (0..100)
            .map(|i| {
                serde_json::json!({
                    "name": format!("repo-{i}"),
                    "full_name": format!("org/repo-{i}"),
                    "owner": { "login": "org" }
                })
            })
            .collect();
        Mock::given(method("GET"))
            .and(path("/user/repos"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(full_page))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user/repos"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "last-one", "full_name": "org/last-one", "owner": { "login": "org" } }
            ])))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());
        let repos = client.list_accessible_repos().await.unwrap();

        assert_eq!(repos.len(), 101);
        assert_eq!(repos[100].full_name, "org/last-one");
    }

    #[tokio::test]
    async fn list_accessible_repos_maps_api_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/repos"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Bad credentials"))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());
        let result = client.list_accessible_repos().await;

        assert!(matches!(result, Err(GithubError::Api { status: 401, .. })));
    }

    #[tokio::test]
    async fn list_releases_returns_parsed_releases_with_assets() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/pixel-perfect/last-beacon/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": 1,
                    "tag_name": "0.2.14",
                    "name": "LastBeacon 0.2.14",
                    "prerelease": false,
                    "published_at": "2026-09-01T00:00:00Z",
                    "assets": [
                        {
                            "id": 10,
                            "name": "last-beacon-windows-x64-shipping.tar.gz",
                            "size": 12345,
                            "browser_download_url": "https://example.com/shipping.tar.gz"
                        }
                    ]
                }
            ])))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());
        let releases = client
            .list_releases("pixel-perfect", "last-beacon")
            .await
            .unwrap();

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].tag_name, "0.2.14");
        assert_eq!(
            releases[0].assets[0].name,
            "last-beacon-windows-x64-shipping.tar.gz"
        );
    }

    #[tokio::test]
    async fn has_any_release_is_true_when_the_repo_has_at_least_one_release() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/pixel-perfect/last-beacon/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 1, "tag_name": "0.2.14", "name": null, "prerelease": false, "published_at": null, "assets": [] }
            ])))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());

        assert!(client
            .has_any_release("pixel-perfect", "last-beacon")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn has_any_release_is_false_when_the_repo_has_no_releases() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/pixel-perfect/empty-repo/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());

        assert!(!client
            .has_any_release("pixel-perfect", "empty-repo")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn list_accessible_repos_with_releases_filters_out_repos_without_any_release() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/repos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "last-beacon", "full_name": "pixel-perfect/last-beacon", "owner": { "login": "pixel-perfect" } },
                { "name": "empty-repo", "full_name": "pixel-perfect/empty-repo", "owner": { "login": "pixel-perfect" } }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/pixel-perfect/last-beacon/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 1, "tag_name": "0.2.14", "name": null, "prerelease": false, "published_at": null, "assets": [] }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/pixel-perfect/empty-repo/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());
        let repos = client.list_accessible_repos_with_releases().await.unwrap();

        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "pixel-perfect/last-beacon");
    }

    #[tokio::test]
    async fn list_accessible_repos_with_releases_excludes_a_repo_whose_release_check_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/repos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "flaky-repo", "full_name": "pixel-perfect/flaky-repo", "owner": { "login": "pixel-perfect" } }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/pixel-perfect/flaky-repo/releases"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let client = GithubClient::with_base_url("token123".to_string(), server.uri());
        let repos = client.list_accessible_repos_with_releases().await.unwrap();

        assert!(repos.is_empty());
    }

    #[test]
    fn asset_download_url_points_at_the_authenticated_asset_content_api() {
        let client = GithubClient::with_base_url(
            "token123".to_string(),
            "https://api.github.com".to_string(),
        );

        let url = client.asset_download_url("pixel-perfect", "last-beacon", 42);

        assert_eq!(
            url,
            "https://api.github.com/repos/pixel-perfect/last-beacon/releases/assets/42"
        );
    }

    #[test]
    fn token_exposes_the_client_s_bearer_token() {
        let client = GithubClient::with_base_url("token123".to_string(), "irrelevant".to_string());

        assert_eq!(client.token(), "token123");
    }
}
