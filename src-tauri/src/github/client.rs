// Not yet wired into any Tauri command (that lands later in Phase 2), so
// clippy would otherwise flag these as dead code under `-D warnings`.
#![allow(dead_code)]

use serde::Deserialize;

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

    pub async fn list_accessible_repos(&self) -> Result<Vec<RepoSummary>, GithubError> {
        let mut repos = Vec::new();
        let mut page = 1;

        loop {
            let path = format!(
                "/user/repos?per_page=100&page={}&affiliation=owner,collaborator,organization_member",
                page
            );
            let response = self.request(reqwest::Method::GET, &path).send().await?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(GithubError::Api { status, body });
            }

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
}
