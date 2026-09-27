use crate::github::client::{GithubClient, GithubError};
use crate::updater::channel::Channel;

const OWNER: &str = "Perfect-Pixel-Games";
const REPO: &str = "pixel-build-manager";

#[derive(Debug, thiserror::Error)]
pub enum EndpointError {
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error("no prerelease found for {0}/{1}")]
    NoPrereleaseFound(String, String),
}

/// Resolves the `latest.json` manifest URL to check for this app's own
/// updates, filtered to the given channel. Release channel uses GitHub's
/// `latest` alias (which natively means "newest non-prerelease, non-draft
/// release" -- free channel filtering, no extra API call). Prerelease
/// channel has no equivalent alias, so it looks up the release list and
/// takes the first one flagged as a prerelease (the list is newest-first).
pub async fn resolve_update_endpoint(
    channel: Channel,
    client: &GithubClient,
) -> Result<String, EndpointError> {
    match channel {
        Channel::Release => Ok(format!(
            "https://github.com/{OWNER}/{REPO}/releases/latest/download/latest.json"
        )),
        Channel::Prerelease => {
            let releases = client.list_releases(OWNER, REPO).await?;
            let tag = releases
                .into_iter()
                .find(|release| release.prerelease)
                .map(|release| release.tag_name)
                .ok_or_else(|| {
                    EndpointError::NoPrereleaseFound(OWNER.to_string(), REPO.to_string())
                })?;
            Ok(format!(
                "https://github.com/{OWNER}/{REPO}/releases/download/{tag}/latest.json"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn release_channel_resolves_to_the_latest_release_alias() {
        let client = GithubClient::anonymous_with_base_url("unused".to_string());

        let url = resolve_update_endpoint(Channel::Release, &client)
            .await
            .unwrap();

        assert_eq!(
            url,
            "https://github.com/Perfect-Pixel-Games/pixel-build-manager/releases/latest/download/latest.json"
        );
    }

    #[tokio::test]
    async fn prerelease_channel_resolves_to_the_first_prerelease_tag() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/repos/Perfect-Pixel-Games/pixel-build-manager/releases",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 2, "tag_name": "0.4.3", "name": null, "prerelease": true, "published_at": null, "assets": [] },
                { "id": 1, "tag_name": "0.4.0", "name": null, "prerelease": false, "published_at": null, "assets": [] }
            ])))
            .mount(&server)
            .await;
        let client = GithubClient::anonymous_with_base_url(server.uri());

        let url = resolve_update_endpoint(Channel::Prerelease, &client)
            .await
            .unwrap();

        assert_eq!(
            url,
            "https://github.com/Perfect-Pixel-Games/pixel-build-manager/releases/download/0.4.3/latest.json"
        );
    }

    #[tokio::test]
    async fn prerelease_channel_errors_when_no_prerelease_exists() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/repos/Perfect-Pixel-Games/pixel-build-manager/releases",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 1, "tag_name": "0.4.0", "name": null, "prerelease": false, "published_at": null, "assets": [] }
            ])))
            .mount(&server)
            .await;
        let client = GithubClient::anonymous_with_base_url(server.uri());

        let result = resolve_update_endpoint(Channel::Prerelease, &client).await;

        assert!(matches!(
            result,
            Err(EndpointError::NoPrereleaseFound(_, _))
        ));
    }
}
