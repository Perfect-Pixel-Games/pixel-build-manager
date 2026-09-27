use super::cache::{self, LatestChannel};
use super::download::{download_with_progress, DownloadError};
use super::extract::{extract_archive, ExtractError};
use crate::github::client::{GithubClient, GithubError, ReleaseAsset, ReleaseSummary};
use std::path::Path;

const SEPARATORS: [char; 3] = ['.', '_', '-'];

/// Mirrors the frontend's `configTemplate()` in `BuildBrowser.tsx`: strips
/// the release's own tag out of an asset name, so a project whose CI embeds
/// the version in the artifact filename (e.g. Tauri/Electron installers)
/// still collapses to the one stable config identity `ticked_configs` is
/// keyed by, rather than registering a new "config" every release. This
/// port exists because a latest-mode sync must resolve that same identity
/// with no frontend involved (it can run entirely from a background poll).
fn config_template(release_tag: &str, asset_name: &str) -> String {
    if release_tag.is_empty() {
        return asset_name.to_string();
    }
    let stripped = asset_name.replace(release_tag, "");

    let mut collapsed = String::with_capacity(stripped.len());
    let mut previous: Option<char> = None;
    for c in stripped.chars() {
        if SEPARATORS.contains(&c) && previous == Some(c) {
            continue;
        }
        collapsed.push(c);
        previous = Some(c);
    }

    collapsed
        .trim_matches(|c| SEPARATORS.contains(&c))
        .to_string()
}

/// Picks the most recently published release matching `channel` (release ==
/// non-prerelease, prerelease == prerelease) out of `releases`. Sorts by
/// `published_at` explicitly rather than trusting the GitHub API's response
/// ordering. Returns `None` if no release in that channel exists yet (e.g.
/// a project with no prerelease published so far).
fn resolve_latest_release(
    releases: &[ReleaseSummary],
    channel: LatestChannel,
) -> Option<&ReleaseSummary> {
    let want_prerelease = matches!(channel, LatestChannel::Prerelease);
    let mut matching: Vec<&ReleaseSummary> = releases
        .iter()
        .filter(|release| release.prerelease == want_prerelease)
        .collect();
    matching.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    matching.into_iter().next()
}

#[derive(Debug, thiserror::Error)]
pub enum LatestSyncError {
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Extract(#[from] ExtractError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatestSyncOutcome {
    /// The resolved release/prerelease hasn't changed since the last sync;
    /// nothing was downloaded.
    UpToDate,
    /// No build configs are ticked for this project, so there's nothing to
    /// sync regardless of whether a new release exists.
    NothingTicked,
    /// No release/prerelease exists yet in this channel at all.
    NoMatchingRelease,
    /// A new release/prerelease was found and its ticked configs were
    /// (re-)synced.
    Synced { tag: String },
}

fn find_asset_for_ticked_config<'a>(
    release: &'a ReleaseSummary,
    ticked_config: &str,
) -> Option<&'a ReleaseAsset> {
    release
        .assets
        .iter()
        .find(|asset| config_template(&release.tag_name, &asset.name) == ticked_config)
}

/// Checks whether `channel`'s latest release has changed since the last
/// sync, and if so (re-)syncs every ticked config's matching asset from it
/// into `latest/<channel>/<config_name>/`, replacing whatever was there.
/// Downloads land transiently in the project's shared `cache/` dir (reusing
/// its existing collision-safe, asset-id-keyed path) and are deleted
/// immediately after a successful extraction, so they never linger there --
/// only the extracted `latest/` contents persist, and only one version at a
/// time.
#[allow(clippy::too_many_arguments)]
pub async fn check_and_sync_latest<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    client: &GithubClient,
    owner: &str,
    repo: &str,
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
    ticked_configs: &[String],
    mut on_progress: F,
) -> Result<LatestSyncOutcome, LatestSyncError> {
    if ticked_configs.is_empty() {
        return Ok(LatestSyncOutcome::NothingTicked);
    }

    let releases = client.list_releases(owner, repo).await?;
    let Some(release) = resolve_latest_release(&releases, channel) else {
        return Ok(LatestSyncOutcome::NoMatchingRelease);
    };

    let already_synced = cache::read_latest_synced_tag(workspace_root, project_key, channel)
        .as_deref()
        == Some(release.tag_name.as_str());
    if already_synced {
        return Ok(LatestSyncOutcome::UpToDate);
    }

    let channel_dir = cache::latest_channel_dir(workspace_root, project_key, channel);
    if channel_dir.exists() {
        std::fs::remove_dir_all(&channel_dir)?;
    }
    std::fs::create_dir_all(cache::cache_dir(workspace_root, project_key))?;

    for ticked in ticked_configs {
        let Some(asset) = find_asset_for_ticked_config(release, ticked) else {
            continue;
        };
        let archive_path =
            cache::cached_asset_path(workspace_root, project_key, asset.id, &asset.name);
        let download_url = client.asset_download_url(owner, repo, asset.id);
        download_with_progress(
            http,
            &download_url,
            client.token(),
            &archive_path,
            asset.size,
            &mut on_progress,
        )
        .await?;

        let target = cache::latest_config_dir(workspace_root, project_key, channel, ticked);
        extract_archive(&archive_path, &target)?;
        std::fs::remove_file(&archive_path).ok();
    }

    cache::write_latest_synced_tag(workspace_root, project_key, channel, &release.tag_name)?;

    Ok(LatestSyncOutcome::Synced {
        tag: release.tag_name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn release(id: u64, tag: &str, prerelease: bool, published_at: Option<&str>) -> ReleaseSummary {
        ReleaseSummary {
            id,
            tag_name: tag.to_string(),
            name: None,
            prerelease,
            published_at: published_at.map(|s| s.to_string()),
            assets: Vec::new(),
        }
    }

    #[test]
    fn config_template_strips_the_release_tag_and_collapses_separators() {
        assert_eq!(
            config_template("0.0.3", "pixel-build-manager_0.0.3_x64-setup.exe"),
            "pixel-build-manager_x64-setup.exe"
        );
    }

    #[test]
    fn config_template_is_the_identity_when_the_tag_is_not_in_the_name() {
        assert_eq!(
            config_template("0.2.14", "last-beacon-windows-x64-shipping.tar.gz"),
            "last-beacon-windows-x64-shipping.tar.gz"
        );
    }

    #[test]
    fn resolve_latest_release_picks_the_most_recently_published_match_for_the_channel() {
        let releases = vec![
            release(1, "0.2.13", false, Some("2026-08-01T00:00:00Z")),
            release(2, "0.2.14", false, Some("2026-09-01T00:00:00Z")),
            release(3, "0.3.0-rc1", true, Some("2026-09-10T00:00:00Z")),
        ];

        let latest_release = resolve_latest_release(&releases, LatestChannel::Release).unwrap();
        let latest_prerelease =
            resolve_latest_release(&releases, LatestChannel::Prerelease).unwrap();

        assert_eq!(latest_release.tag_name, "0.2.14");
        assert_eq!(latest_prerelease.tag_name, "0.3.0-rc1");
    }

    #[test]
    fn resolve_latest_release_returns_none_when_no_release_matches_the_channel() {
        let releases = vec![release(1, "0.2.14", false, Some("2026-09-01T00:00:00Z"))];

        assert!(resolve_latest_release(&releases, LatestChannel::Prerelease).is_none());
    }

    fn build_test_zip_bytes() -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            writer.start_file("game.exe", options).unwrap();
            writer.write_all(b"binary-contents").unwrap();
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }

    async fn mount_releases(server: &MockServer, body: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn syncs_the_latest_release_s_ticked_configs_and_writes_the_marker() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        mount_releases(
            &server,
            serde_json::json!([
                {
                    "id": 1, "tag_name": "0.2.14", "name": null, "prerelease": false,
                    "published_at": "2026-09-01T00:00:00Z",
                    "assets": [
                        { "id": 10, "name": "shipping.zip", "size": zip_bytes.len(), "browser_download_url": "https://example.com/a" }
                    ]
                }
            ]),
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/releases/assets/10"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone()))
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let client = GithubClient::with_base_url("test-token".to_string(), server.uri());
        let http = reqwest::Client::new();

        let outcome = check_and_sync_latest(
            &http,
            &client,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            &["shipping.zip".to_string()],
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(
            outcome,
            LatestSyncOutcome::Synced {
                tag: "0.2.14".to_string()
            }
        );
        let target = cache::latest_config_dir(
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            "shipping.zip",
        );
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
            "binary-contents"
        );
        assert_eq!(
            cache::read_latest_synced_tag(workspace.path(), "org/repo", LatestChannel::Release),
            Some("0.2.14".to_string())
        );
    }

    #[tokio::test]
    async fn a_second_check_with_the_same_latest_release_skips_downloading_again() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        mount_releases(
            &server,
            serde_json::json!([
                {
                    "id": 1, "tag_name": "0.2.14", "name": null, "prerelease": false,
                    "published_at": "2026-09-01T00:00:00Z",
                    "assets": [
                        { "id": 10, "name": "shipping.zip", "size": zip_bytes.len(), "browser_download_url": "https://example.com/a" }
                    ]
                }
            ]),
        )
        .await;
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/releases/assets/10"))
            .respond_with(move |_: &wiremock::Request| {
                call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone())
            })
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let client = GithubClient::with_base_url("test-token".to_string(), server.uri());
        let http = reqwest::Client::new();
        let ticked = vec!["shipping.zip".to_string()];

        check_and_sync_latest(
            &http,
            &client,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            &ticked,
            |_, _| {},
        )
        .await
        .unwrap();
        let outcome = check_and_sync_latest(
            &http,
            &client,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            &ticked,
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, LatestSyncOutcome::UpToDate);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_new_latest_release_replaces_the_previous_channel_contents() {
        let server = MockServer::start().await;
        let workspace = tempfile::tempdir().unwrap();
        let client = GithubClient::with_base_url("test-token".to_string(), server.uri());
        let http = reqwest::Client::new();
        let ticked = vec!["shipping.zip".to_string()];

        let first_zip = build_test_zip_bytes();
        mount_releases(
            &server,
            serde_json::json!([
                { "id": 1, "tag_name": "0.2.13", "name": null, "prerelease": false, "published_at": "2026-08-01T00:00:00Z",
                  "assets": [{ "id": 10, "name": "shipping.zip", "size": first_zip.len(), "browser_download_url": "https://example.com/a" }] }
            ]),
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/releases/assets/10"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(first_zip.clone()))
            .mount(&server)
            .await;

        check_and_sync_latest(
            &http,
            &client,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            &ticked,
            |_, _| {},
        )
        .await
        .unwrap();
        let target = cache::latest_config_dir(
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            "shipping.zip",
        );
        assert!(target.join("game.exe").exists());
        // A stray leftover file that must not survive the next sync's clear.
        std::fs::write(target.join("stale.txt"), b"old").unwrap();

        // A newer release appears -- a fresh mock server stands in for the
        // next poll's release list (wiremock mocks can't be swapped in
        // place on a live server).
        let server2 = MockServer::start().await;
        let client2 = GithubClient::with_base_url("test-token".to_string(), server2.uri());
        let second_zip = build_test_zip_bytes();
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 2, "tag_name": "0.2.14", "name": null, "prerelease": false, "published_at": "2026-09-01T00:00:00Z",
                  "assets": [{ "id": 20, "name": "shipping.zip", "size": second_zip.len(), "browser_download_url": "https://example.com/b" }] }
            ])))
            .mount(&server2)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/releases/assets/20"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(second_zip.clone()))
            .mount(&server2)
            .await;

        let outcome = check_and_sync_latest(
            &http,
            &client2,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            &ticked,
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(
            outcome,
            LatestSyncOutcome::Synced {
                tag: "0.2.14".to_string()
            }
        );
        assert!(
            !target.join("stale.txt").exists(),
            "the channel dir must be cleared before re-extracting"
        );
        assert!(target.join("game.exe").exists());
        assert_eq!(
            cache::read_latest_synced_tag(workspace.path(), "org/repo", LatestChannel::Release),
            Some("0.2.14".to_string())
        );
    }

    #[tokio::test]
    async fn empty_ticked_configs_is_a_no_op_and_makes_no_network_call() {
        let server = MockServer::start().await;
        let workspace = tempfile::tempdir().unwrap();
        let client = GithubClient::with_base_url("test-token".to_string(), server.uri());
        let http = reqwest::Client::new();

        let outcome = check_and_sync_latest(
            &http,
            &client,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Release,
            &[],
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, LatestSyncOutcome::NothingTicked);
    }

    #[tokio::test]
    async fn no_prerelease_yet_resolves_to_no_matching_release() {
        let server = MockServer::start().await;
        mount_releases(
            &server,
            serde_json::json!([
                { "id": 1, "tag_name": "0.2.14", "name": null, "prerelease": false, "published_at": "2026-09-01T00:00:00Z", "assets": [] }
            ]),
        )
        .await;
        let workspace = tempfile::tempdir().unwrap();
        let client = GithubClient::with_base_url("test-token".to_string(), server.uri());
        let http = reqwest::Client::new();

        let outcome = check_and_sync_latest(
            &http,
            &client,
            "org",
            "repo",
            workspace.path(),
            "org/repo",
            LatestChannel::Prerelease,
            &["shipping.zip".to_string()],
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, LatestSyncOutcome::NoMatchingRelease);
    }
}
