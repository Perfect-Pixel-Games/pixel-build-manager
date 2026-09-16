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
}
