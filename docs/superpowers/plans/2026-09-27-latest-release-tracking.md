# Latest Release/Prerelease Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a bound project auto-track "latest release" or "latest prerelease" instead of a manually-picked build, syncing ticked configs into a dedicated non-accumulating directory whenever a new one ships.

**Architecture:** A new `SyncMode` setting per project (`Manual` | `LatestRelease` | `LatestPrerelease`) drives a background poll (every 5 minutes) plus an immediate on-toggle check, both funneled through one Rust orchestration function (`sync::latest::check_and_sync_latest`) that resolves the newest matching release, compares it against a marker file, and — on change — wipes and re-extracts a dedicated `latest/<channel>/` directory. Two new Rust-emitted events (`latest-sync-progress`, `latest-sync-finished`) let the frontend show backend-initiated syncs in the existing `BusyOverlay`, via a new `useLatestSync` hook that's independent of the existing manual-mode `useSync`.

**Tech Stack:** Rust (Tauri backend, `reqwest`, `wiremock` for tests), React + TypeScript (Vitest + Testing Library for tests).

**Reference spec:** `docs/superpowers/specs/2026-09-27-latest-tracking-design.md`

---

## Task 1: `SyncMode` setting

**Files:**
- Modify: `src-tauri/src/settings/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests inside the existing `#[cfg(test)] mod tests` block in `src-tauri/src/settings/mod.rs` (append after `theme_serializes_as_lowercase_strings`):

```rust
    #[test]
    fn default_sync_mode_is_manual() {
        assert_eq!(ProjectSettings::default().sync_mode, SyncMode::Manual);
    }

    #[test]
    fn sync_mode_serializes_as_snake_case_strings() {
        assert_eq!(serde_json::to_string(&SyncMode::Manual).unwrap(), "\"manual\"");
        assert_eq!(
            serde_json::to_string(&SyncMode::LatestRelease).unwrap(),
            "\"latest_release\""
        );
        assert_eq!(
            serde_json::to_string(&SyncMode::LatestPrerelease).unwrap(),
            "\"latest_prerelease\""
        );
    }

    #[test]
    fn set_sync_mode_creates_entry_if_missing() {
        let mut settings = Settings::default();

        settings.set_sync_mode("org/repo", SyncMode::LatestRelease);

        assert_eq!(settings.projects["org/repo"].sync_mode, SyncMode::LatestRelease);
    }

    #[test]
    fn deserializing_settings_without_a_sync_mode_field_defaults_to_manual() {
        let json = r#"{"projects":{"org/repo":{"favorite":false,"selected_release_tag":null,"ticked_configs":[]}}}"#;

        let settings: Settings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.projects["org/repo"].sync_mode, SyncMode::Manual);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `src-tauri`): `cargo test settings::`
Expected: compile error — `SyncMode` not found, `set_sync_mode` not found.

- [ ] **Step 3: Implement `SyncMode` and wire it into `ProjectSettings`/`Settings`**

Add the enum right after the existing `Theme` enum in `src-tauri/src/settings/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    #[default]
    Manual,
    LatestRelease,
    LatestPrerelease,
}
```

Update `ProjectSettings` to add the new field:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub favorite: bool,
    pub selected_release_tag: Option<String>,
    #[serde(default)]
    pub ticked_configs: Vec<String>,
    #[serde(default)]
    pub sync_mode: SyncMode,
}
```

Add the setter method right after `set_ticked_configs`:

```rust
    pub fn set_sync_mode(&mut self, project_key: &str, mode: SyncMode) {
        self.projects.entry(project_key.to_string()).or_default().sync_mode = mode;
    }
```

Fix the pre-existing round-trip test so it still compiles with the new field — in the `save_then_load_round_trips` test, change:

```rust
            ProjectSettings {
                favorite: true,
                selected_release_tag: Some("0.2.14".to_string()),
                ticked_configs: vec!["shipping.zip".to_string()],
            },
```

to:

```rust
            ProjectSettings {
                favorite: true,
                selected_release_tag: Some("0.2.14".to_string()),
                ticked_configs: vec!["shipping.zip".to_string()],
                sync_mode: SyncMode::LatestRelease,
            },
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test settings::`
Expected: PASS, all tests including the pre-existing ones.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/settings/mod.rs
git commit -m "Add SyncMode setting for per-project latest-release tracking"
```

---

## Task 2: Latest-channel storage layout in `cache.rs`

**Files:**
- Modify: `src-tauri/src/sync/cache.rs`

- [ ] **Step 1: Write the failing tests**

Add `use serde::{Deserialize, Serialize};` to the top of `src-tauri/src/sync/cache.rs` (alongside the existing `use std::fs;` etc. imports).

Append these tests inside the existing `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn latest_channel_dir_nests_under_a_dedicated_latest_folder() {
        let root = Path::new("D:\\Builds");

        assert_eq!(
            latest_channel_dir(root, "org/repo", LatestChannel::Release),
            PathBuf::from("D:\\Builds\\org\\repo\\latest\\release")
        );
        assert_eq!(
            latest_channel_dir(root, "org/repo", LatestChannel::Prerelease),
            PathBuf::from("D:\\Builds\\org\\repo\\latest\\prerelease")
        );
    }

    #[test]
    fn latest_config_dir_sanitizes_the_config_name() {
        let root = Path::new("D:\\Builds");

        let dir = latest_config_dir(root, "org/repo", LatestChannel::Release, "..");

        assert_eq!(dir, PathBuf::from("D:\\Builds\\org\\repo\\latest\\release\\_"));
    }

    #[test]
    fn read_latest_synced_tag_returns_none_when_nothing_has_synced_yet() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release),
            None
        );
    }

    #[test]
    fn write_then_read_latest_synced_tag_round_trips() {
        let dir = tempfile::tempdir().unwrap();

        write_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release, "0.2.14").unwrap();

        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release),
            Some("0.2.14".to_string())
        );
    }

    #[test]
    fn the_two_channels_have_independent_synced_tags() {
        let dir = tempfile::tempdir().unwrap();

        write_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release, "0.2.14").unwrap();
        write_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Prerelease, "0.3.0-rc1").unwrap();

        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release),
            Some("0.2.14".to_string())
        );
        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Prerelease),
            Some("0.3.0-rc1".to_string())
        );
    }

    #[test]
    fn list_synced_latest_configs_returns_empty_when_nothing_has_synced_yet() {
        let dir = tempfile::tempdir().unwrap();

        let configs = list_synced_latest_configs(dir.path(), "org/repo", LatestChannel::Release).unwrap();

        assert!(configs.is_empty());
    }

    #[test]
    fn list_synced_latest_configs_lists_extracted_configs_and_excludes_the_marker_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(latest_config_dir(root, "org/repo", LatestChannel::Release, "shipping.zip")).unwrap();
        write_latest_synced_tag(root, "org/repo", LatestChannel::Release, "0.2.14").unwrap();

        let configs = list_synced_latest_configs(root, "org/repo", LatestChannel::Release).unwrap();

        assert_eq!(configs, vec!["shipping.zip".to_string()]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test sync::cache::`
Expected: compile error — `LatestChannel`, `latest_channel_dir`, etc. not found.

- [ ] **Step 3: Implement the latest-channel storage functions**

Add this near the top of `src-tauri/src/sync/cache.rs`, after the existing imports:

```rust
/// Which of a project's two auto-tracked channels a latest-mode sync
/// targets. Modeled as an enum (not a raw string) so an invalid value from
/// the frontend fails Tauri's argument deserialization instead of ever
/// reaching a directory-path computation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LatestChannel {
    Release,
    Prerelease,
}

impl LatestChannel {
    fn dir_name(self) -> &'static str {
        match self {
            LatestChannel::Release => "release",
            LatestChannel::Prerelease => "prerelease",
        }
    }
}

const LATEST_SYNCED_TAG_FILE: &str = ".synced_tag";
```

Add these functions after `list_synced_configs` (still before the `#[cfg(test)]` block):

```rust
/// The extraction root for one project's auto-tracked channel. Sibling to
/// `cache/` and `builds/` -- deliberately its own top-level directory rather
/// than living under `builds/<tag>/`, since its whole contents get wiped and
/// replaced on every new release rather than accumulating per-tag like
/// manual-mode builds do.
pub fn latest_channel_dir(workspace_root: &Path, project_key: &str, channel: LatestChannel) -> PathBuf {
    project_dir(workspace_root, project_key)
        .join("latest")
        .join(channel.dir_name())
}

/// The extraction directory for one ticked config within a latest channel.
/// Keyed by the *ticked config's stable template name*, not the release
/// asset's literal file name -- unlike manual mode's `build_config_dir`,
/// this must stay the same path across releases even for a project whose
/// asset names embed the version, since the whole point is one stable
/// location that gets overwritten in place.
pub fn latest_config_dir(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
    config_name: &str,
) -> PathBuf {
    latest_channel_dir(workspace_root, project_key, channel).join(sanitize_path_component(config_name))
}

/// Reads which release tag is currently extracted into `channel`'s
/// directory, if any. Colocating this marker with the data it describes
/// (rather than in `settings.json`) means it's naturally reset whenever the
/// channel dir is cleared, with no separate bookkeeping to keep in sync.
pub fn read_latest_synced_tag(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
) -> Option<String> {
    let marker = latest_channel_dir(workspace_root, project_key, channel).join(LATEST_SYNCED_TAG_FILE);
    fs::read_to_string(marker).ok()
}

pub fn write_latest_synced_tag(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
    tag: &str,
) -> io::Result<()> {
    let dir = latest_channel_dir(workspace_root, project_key, channel);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(LATEST_SYNCED_TAG_FILE), tag)
}

/// Lists the build-config names currently extracted for `channel`, mirroring
/// `list_synced_configs`. The marker file above is a regular file, so the
/// existing `is_dir()` filter already excludes it with no extra logic.
pub fn list_synced_latest_configs(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
) -> io::Result<Vec<String>> {
    let dir = latest_channel_dir(workspace_root, project_key, channel);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut configs = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                configs.push(name.to_string());
            }
        }
    }
    Ok(configs)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test sync::cache::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync/cache.rs
git commit -m "Add dedicated latest/<channel>/ storage layout and marker file"
```

---

## Task 3: Config-template matching and latest-release resolution (pure logic)

**Files:**
- Create: `src-tauri/src/sync/latest.rs`
- Modify: `src-tauri/src/sync/mod.rs`

- [ ] **Step 1: Register the new module**

In `src-tauri/src/sync/mod.rs`, add the new module:

```rust
pub mod cache;
pub mod download;
pub mod extract;
pub mod latest;
pub mod launch;
pub mod orchestrator;
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/sync/latest.rs` with just the test module for now:

```rust
use super::cache::LatestChannel;
use crate::github::client::ReleaseSummary;

#[cfg(test)]
mod tests {
    use super::*;

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
        let latest_prerelease = resolve_latest_release(&releases, LatestChannel::Prerelease).unwrap();

        assert_eq!(latest_release.tag_name, "0.2.14");
        assert_eq!(latest_prerelease.tag_name, "0.3.0-rc1");
    }

    #[test]
    fn resolve_latest_release_returns_none_when_no_release_matches_the_channel() {
        let releases = vec![release(1, "0.2.14", false, Some("2026-09-01T00:00:00Z"))];

        assert!(resolve_latest_release(&releases, LatestChannel::Prerelease).is_none());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test sync::latest::`
Expected: compile error — `config_template`, `resolve_latest_release` not found.

- [ ] **Step 4: Implement `config_template` and `resolve_latest_release`**

Add this above the `#[cfg(test)]` block in `src-tauri/src/sync/latest.rs`:

```rust
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

    collapsed.trim_matches(|c| SEPARATORS.contains(&c)).to_string()
}

/// Picks the most recently published release matching `channel` (release ==
/// non-prerelease, prerelease == prerelease) out of `releases`. Sorts by
/// `published_at` explicitly rather than trusting the GitHub API's response
/// ordering. Returns `None` if no release in that channel exists yet (e.g.
/// a project with no prerelease published so far).
fn resolve_latest_release(releases: &[ReleaseSummary], channel: LatestChannel) -> Option<&ReleaseSummary> {
    let want_prerelease = matches!(channel, LatestChannel::Prerelease);
    let mut matching: Vec<&ReleaseSummary> = releases
        .iter()
        .filter(|release| release.prerelease == want_prerelease)
        .collect();
    matching.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    matching.into_iter().next()
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test sync::latest::`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/sync/latest.rs src-tauri/src/sync/mod.rs
git commit -m "Add config-template matching and latest-release resolution logic"
```

---

## Task 4: `check_and_sync_latest` orchestration

**Files:**
- Modify: `src-tauri/src/sync/latest.rs`

- [ ] **Step 1: Write the failing tests**

Update the top of `src-tauri/src/sync/latest.rs` to add the new imports needed:

```rust
use super::cache::{self, LatestChannel};
use super::download::{download_with_progress, DownloadError};
use super::extract::{extract_archive, ExtractError};
use crate::github::client::{GithubClient, GithubError, ReleaseAsset, ReleaseSummary};
use std::path::Path;
```

Append these tests inside the existing `#[cfg(test)] mod tests` block (they need `wiremock`, `tempfile`, and `zip`, all already dev-dependencies):

```rust
    use std::io::Write;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

        assert_eq!(outcome, LatestSyncOutcome::Synced { tag: "0.2.14".to_string() });
        let target = cache::latest_config_dir(workspace.path(), "org/repo", LatestChannel::Release, "shipping.zip");
        assert_eq!(std::fs::read_to_string(target.join("game.exe")).unwrap(), "binary-contents");
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

        check_and_sync_latest(&http, &client, "org", "repo", workspace.path(), "org/repo", LatestChannel::Release, &ticked, |_, _| {})
            .await
            .unwrap();
        let outcome = check_and_sync_latest(&http, &client, "org", "repo", workspace.path(), "org/repo", LatestChannel::Release, &ticked, |_, _| {})
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

        check_and_sync_latest(&http, &client, "org", "repo", workspace.path(), "org/repo", LatestChannel::Release, &ticked, |_, _| {})
            .await
            .unwrap();
        let target = cache::latest_config_dir(workspace.path(), "org/repo", LatestChannel::Release, "shipping.zip");
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

        let outcome = check_and_sync_latest(&http, &client2, "org", "repo", workspace.path(), "org/repo", LatestChannel::Release, &ticked, |_, _| {})
            .await
            .unwrap();

        assert_eq!(outcome, LatestSyncOutcome::Synced { tag: "0.2.14".to_string() });
        assert!(!target.join("stale.txt").exists(), "the channel dir must be cleared before re-extracting");
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

        let outcome = check_and_sync_latest(&http, &client, "org", "repo", workspace.path(), "org/repo", LatestChannel::Release, &[], |_, _| {})
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

        let outcome = check_and_sync_latest(&http, &client, "org", "repo", workspace.path(), "org/repo", LatestChannel::Prerelease, &["shipping.zip".to_string()], |_, _| {})
            .await
            .unwrap();

        assert_eq!(outcome, LatestSyncOutcome::NoMatchingRelease);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test sync::latest::`
Expected: compile error — `LatestSyncOutcome`, `check_and_sync_latest` not found.

- [ ] **Step 3: Implement `check_and_sync_latest`**

Add this above the `#[cfg(test)]` block in `src-tauri/src/sync/latest.rs`, after `resolve_latest_release`:

```rust
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

    let already_synced = cache::read_latest_synced_tag(workspace_root, project_key, channel).as_deref()
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
        let archive_path = cache::cached_asset_path(workspace_root, project_key, asset.id, &asset.name);
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test sync::latest::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync/latest.rs
git commit -m "Add check_and_sync_latest orchestration for latest-mode syncing"
```

---

## Task 5: Background poller and Tauri command surface

**Files:**
- Create: `src-tauri/src/latest_poller.rs`
- Modify: `src-tauri/src/lib.rs`

No automated tests in this task — like the existing `updater/mod.rs` background loop (see `docs/superpowers/specs/2026-09-23-auto-update-design.md`'s Testing section), this is Tauri-`AppHandle`-shaped plumbing that isn't practically unit-testable without a live app; correctness here rests on the already-tested `check_and_sync_latest` it calls, plus a full `cargo build`/`cargo test` pass at the end confirming nothing broke.

- [ ] **Step 1: Make the helpers `check_and_sync_now` needs reusable**

In `src-tauri/src/lib.rs`, change these three existing private items to `pub(crate)` (no other changes to their bodies):

```rust
pub(crate) fn begin_operation(state: &AppState, project_key: &str) -> Result<(), String> {
```

```rust
pub(crate) fn end_operation(state: &AppState, project_key: &str) {
```

```rust
pub(crate) async fn build_github_client(state: &AppState) -> Result<GithubClient, String> {
```

- [ ] **Step 2: Create the background poller module**

Create `src-tauri/src/latest_poller.rs`:

```rust
use crate::github::client::GithubClient;
use crate::settings::{Settings, SyncMode};
use crate::sync::cache::LatestChannel;
use crate::sync::latest::{check_and_sync_latest, LatestSyncOutcome};
use crate::{begin_operation, build_github_client, end_operation, AppState};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

fn channel_for_mode(mode: SyncMode) -> Option<LatestChannel> {
    match mode {
        SyncMode::Manual => None,
        SyncMode::LatestRelease => Some(LatestChannel::Release),
        SyncMode::LatestPrerelease => Some(LatestChannel::Prerelease),
    }
}

#[derive(Debug, Clone, Serialize)]
struct LatestSyncProgressPayload {
    project_key: String,
    channel: LatestChannel,
    downloaded: u64,
    total: u64,
}

#[derive(Debug, Clone, Serialize)]
struct LatestSyncFinishedPayload {
    project_key: String,
    channel: LatestChannel,
    synced: bool,
    tag: Option<String>,
    error: Option<String>,
}

impl LatestSyncFinishedPayload {
    fn from_outcome(project_key: &str, channel: LatestChannel, outcome: &Result<LatestSyncOutcome, String>) -> Self {
        match outcome {
            Ok(LatestSyncOutcome::Synced { tag }) => Self {
                project_key: project_key.to_string(),
                channel,
                synced: true,
                tag: Some(tag.clone()),
                error: None,
            },
            Ok(_) => Self {
                project_key: project_key.to_string(),
                channel,
                synced: false,
                tag: None,
                error: None,
            },
            Err(e) => Self {
                project_key: project_key.to_string(),
                channel,
                synced: false,
                tag: None,
                error: Some(e.clone()),
            },
        }
    }
}

/// Starts the background loop that keeps every bound project's latest
/// release/prerelease (for projects with that mode selected) synced. Runs
/// unconditionally -- unlike the self-updater, this has nothing to do with
/// this app's own release channel or debug/release build type.
pub fn start_background_latest_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        loop {
            interval.tick().await;
            let bound_projects = {
                let state = app.state::<AppState>();
                Settings::load_from(&state.settings_path).bound_projects
            };
            for project_key in bound_projects {
                if let Err(e) = check_and_sync_now(&app, &project_key).await {
                    eprintln!("latest-sync check failed for {project_key}: {e}");
                }
            }
        }
    });
}

/// Runs one latest-sync check/sync cycle for `project_key`, if it's
/// currently in a latest mode. No-ops (returns `Ok`) for a project in
/// `Manual` mode. Used by both the background loop above and by
/// `set_sync_mode`/`check_latest_now`'s immediate on-demand check.
pub async fn check_and_sync_now(app: &AppHandle, project_key: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .clone()
        .ok_or_else(|| "workspace root not set".to_string())?;
    let project = settings.projects.get(project_key).cloned().unwrap_or_default();

    let Some(channel) = channel_for_mode(project.sync_mode) else {
        return Ok(());
    };
    let (owner, repo) = project_key
        .split_once('/')
        .ok_or_else(|| format!("invalid project_key: {project_key}"))?;

    begin_operation(&state, project_key)?;

    let client_result = build_github_client(&state).await;
    let outcome: Result<LatestSyncOutcome, String> = match client_result {
        Ok(client) => run_check(
            app,
            &client,
            project_key,
            owner,
            repo,
            channel,
            &workspace_root,
            &project.ticked_configs,
        )
        .await,
        Err(e) => Err(e),
    };

    end_operation(&state, project_key);

    let payload = LatestSyncFinishedPayload::from_outcome(project_key, channel, &outcome);
    let _ = app.emit("latest-sync-finished", payload);

    outcome.map(|_| ())
}

#[allow(clippy::too_many_arguments)]
async fn run_check(
    app: &AppHandle,
    client: &GithubClient,
    project_key: &str,
    owner: &str,
    repo: &str,
    channel: LatestChannel,
    workspace_root: &std::path::Path,
    ticked_configs: &[String],
) -> Result<LatestSyncOutcome, String> {
    let http = reqwest::Client::new();
    let app_for_events = app.clone();
    let project_key_for_events = project_key.to_string();

    check_and_sync_latest(
        &http,
        client,
        owner,
        repo,
        workspace_root,
        project_key,
        channel,
        ticked_configs,
        move |downloaded, total| {
            let _ = app_for_events.emit(
                "latest-sync-progress",
                LatestSyncProgressPayload {
                    project_key: project_key_for_events.clone(),
                    channel,
                    downloaded,
                    total,
                },
            );
        },
    )
    .await
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register the module and start the loop**

In `src-tauri/src/lib.rs`, add the module declaration alongside the other `mod` lines (keep alphabetical order):

```rust
mod auth;
mod github;
mod latest_poller;
mod settings;
mod sync;
mod updater;
mod version;
```

Add the import alongside the existing `use updater::start_background_updates;` line:

```rust
use latest_poller::start_background_latest_sync;
```

Also add, near the other `use settings::...` / `use sync::cache::...` imports:

```rust
use settings::{Settings, SyncMode, Theme};
use sync::cache::{build_config_dir, builds_root_dir, cache_dir, LatestChannel};
```

(These replace the existing single-line versions of those two imports.)

In the `.setup()` closure in `run()`, right after the existing `start_background_updates(app.handle().clone());` line, add:

```rust
            start_background_latest_sync(app.handle().clone());
```

- [ ] **Step 4: Add the new Tauri commands**

Add these commands to `src-tauri/src/lib.rs`, after `set_ticked_configs`:

```rust
#[tauri::command]
fn get_sync_mode(project_key: String, state: tauri::State<'_, AppState>) -> Result<SyncMode, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .projects
        .get(&project_key)
        .map(|p| p.sync_mode)
        .unwrap_or_default())
}

fn spawn_latest_check(app: tauri::AppHandle, project_key: String) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = latest_poller::check_and_sync_now(&app, &project_key).await {
            eprintln!("latest-sync check failed for {project_key}: {e}");
        }
    });
}

#[tauri::command]
fn set_sync_mode(
    app: tauri::AppHandle,
    project_key: String,
    mode: SyncMode,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    {
        let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
        let mut settings = Settings::load_from(&state.settings_path);
        settings.set_sync_mode(&project_key, mode);
        settings.save_to(&state.settings_path).map_err(|e| e.to_string())?;
    }

    if mode != SyncMode::Manual {
        spawn_latest_check(app, project_key);
    }

    Ok(())
}

#[tauri::command]
fn check_latest_now(app: tauri::AppHandle, project_key: String) -> Result<(), String> {
    spawn_latest_check(app, project_key);
    Ok(())
}
```

Add these commands after `list_synced_configs`:

```rust
#[tauri::command]
fn list_synced_latest_configs(
    project_key: String,
    channel: LatestChannel,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    sync::cache::list_synced_latest_configs(&workspace_root, &project_key, channel).map_err(|e| e.to_string())
}
```

Add these commands after `get_build_dir` (the last command before `#[cfg_attr(mobile, ...)] pub fn run()`):

```rust
#[tauri::command]
fn get_latest_build_dir(
    project_key: String,
    channel: LatestChannel,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = sync::cache::latest_config_dir(&workspace_root, &project_key, channel, &config_name);
    Ok(dir.exists().then(|| dir.to_string_lossy().to_string()))
}

#[tauri::command]
fn get_latest_build_executable(
    project_key: String,
    channel: LatestChannel,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = sync::cache::latest_config_dir(&workspace_root, &project_key, channel, &config_name);
    Ok(find_build_executable(&dir).map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn launch_latest_build(
    project_key: String,
    channel: LatestChannel,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = sync::cache::latest_config_dir(&workspace_root, &project_key, channel, &config_name);
    let exe = find_build_executable(&dir).ok_or_else(|| "no executable found in this build".to_string())?;
    launch_executable(&exe).map_err(|e| e.to_string())
}
```

Finally, register all seven new commands in the `tauri::generate_handler![...]` list in `run()`:

```rust
        .invoke_handler(tauri::generate_handler![
            login_start,
            logout,
            is_logged_in,
            list_projects,
            list_releases_for_project,
            toggle_favorite,
            get_workspace_root,
            set_workspace_root,
            list_bound_projects,
            bind_project,
            unbind_project,
            get_theme,
            set_theme,
            get_selected_release,
            set_selected_release,
            get_ticked_configs,
            set_ticked_configs,
            get_sync_mode,
            set_sync_mode,
            check_latest_now,
            list_synced_configs,
            list_synced_latest_configs,
            sync_release_asset,
            clear_project_cache,
            get_build_executable,
            launch_build,
            get_build_dir,
            get_latest_build_dir,
            get_latest_build_executable,
            launch_latest_build,
            get_version_label
        ])
```

- [ ] **Step 5: Verify it builds and existing tests still pass**

Run (from `src-tauri`): `cargo build`
Expected: builds with no errors.

Run: `cargo test`
Expected: PASS (all existing tests plus Tasks 1-4's new tests).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/latest_poller.rs src-tauri/src/lib.rs
git commit -m "Add background latest-sync poller and its Tauri command surface"
```

---

## Task 6: Frontend API bindings

**Files:**
- Modify: `src/api/sync.ts`

No dedicated test file for this task — `src/api/sync.ts` is a thin `invoke`/`listen` wrapper layer with no existing unit tests of its own (every existing export is exercised indirectly by the components/hooks that call it); the new exports follow the exact same pattern and are exercised by Tasks 7-10's tests instead.

- [ ] **Step 1: Add the new types and functions**

Add to `src/api/sync.ts`, after the existing `SyncProgress` type:

```ts
export type SyncMode = "manual" | "latest_release" | "latest_prerelease";
export type LatestChannel = "release" | "prerelease";

export type LatestSyncProgress = {
  project_key: string;
  channel: LatestChannel;
  downloaded: number;
  total: number;
};

export type LatestSyncFinished = {
  project_key: string;
  channel: LatestChannel;
  synced: boolean;
  tag: string | null;
  error: string | null;
};
```

Add to the end of `src/api/sync.ts`:

```ts
export function getSyncMode(projectKey: string): Promise<SyncMode> {
  return invoke("get_sync_mode", { projectKey });
}

export function setSyncMode(projectKey: string, mode: SyncMode): Promise<void> {
  return invoke("set_sync_mode", { projectKey, mode });
}

export function checkLatestNow(projectKey: string): Promise<void> {
  return invoke("check_latest_now", { projectKey });
}

export function listSyncedLatestConfigs(projectKey: string, channel: LatestChannel): Promise<string[]> {
  return invoke("list_synced_latest_configs", { projectKey, channel });
}

export function getLatestBuildDir(
  projectKey: string,
  channel: LatestChannel,
  configName: string,
): Promise<string | null> {
  return invoke("get_latest_build_dir", { projectKey, channel, configName });
}

export function getLatestBuildExecutable(
  projectKey: string,
  channel: LatestChannel,
  configName: string,
): Promise<string | null> {
  return invoke("get_latest_build_executable", { projectKey, channel, configName });
}

export function launchLatestBuild(
  projectKey: string,
  channel: LatestChannel,
  configName: string,
): Promise<void> {
  return invoke("launch_latest_build", { projectKey, channel, configName });
}

export function onLatestSyncProgress(callback: (progress: LatestSyncProgress) => void) {
  return listen<LatestSyncProgress>("latest-sync-progress", (event) => callback(event.payload));
}

export function onLatestSyncFinished(callback: (result: LatestSyncFinished) => void) {
  return listen<LatestSyncFinished>("latest-sync-finished", (event) => callback(event.payload));
}
```

- [ ] **Step 2: Verify it type-checks**

Run: `npm run build`
Expected: succeeds (this also confirms Tasks 7-10 haven't referenced anything not yet defined, once they're done -- at this point in the plan it just confirms Task 6 itself compiles cleanly).

- [ ] **Step 3: Commit**

```bash
git add src/api/sync.ts
git commit -m "Add frontend API bindings for latest-mode sync"
```

---

## Task 7: `useLatestSync` hook

**Files:**
- Create: `src/hooks/useLatestSync.ts`
- Create: `src/hooks/useLatestSync.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/hooks/useLatestSync.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useLatestSync } from "./useLatestSync";
import * as syncApi from "../api/sync";

vi.mock("../api/sync");

describe("useLatestSync", () => {
  it("starts idle", () => {
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));

    const { result } = renderHook(() => useLatestSync("org/repo"));

    expect(result.current.state).toEqual({ phase: "idle" });
  });

  it("transitions to syncing on a matching progress event", () => {
    let capturedProgress: (p: syncApi.LatestSyncProgress) => void = () => {};
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation((cb) => {
      capturedProgress = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedProgress({ project_key: "org/repo", channel: "release", downloaded: 5, total: 10 });
    });

    expect(result.current.state).toEqual({
      phase: "syncing",
      channel: "release",
      downloaded: 5,
      total: 10,
    });
  });

  it("ignores progress events for a different project", () => {
    let capturedProgress: (p: syncApi.LatestSyncProgress) => void = () => {};
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation((cb) => {
      capturedProgress = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedProgress({ project_key: "org/other-repo", channel: "release", downloaded: 5, total: 10 });
    });

    expect(result.current.state).toEqual({ phase: "idle" });
  });

  it("transitions to done on a successful finished event", () => {
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
    let capturedFinished: (r: syncApi.LatestSyncFinished) => void = () => {};
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation((cb) => {
      capturedFinished = cb;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedFinished({ project_key: "org/repo", channel: "release", synced: true, tag: "0.2.14", error: null });
    });

    expect(result.current.state).toEqual({ phase: "done", channel: "release", synced: true, tag: "0.2.14" });
  });

  it("transitions to error when the finished event carries an error", () => {
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
    let capturedFinished: (r: syncApi.LatestSyncFinished) => void = () => {};
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation((cb) => {
      capturedFinished = cb;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedFinished({
        project_key: "org/repo",
        channel: "release",
        synced: false,
        tag: null,
        error: "network down",
      });
    });

    expect(result.current.state).toEqual({ phase: "error", channel: "release", message: "network down" });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm run test -- src/hooks/useLatestSync.test.ts`
Expected: FAIL — `useLatestSync` module not found.

- [ ] **Step 3: Implement the hook**

Create `src/hooks/useLatestSync.ts`:

```ts
import { useEffect, useState } from "react";
import { onLatestSyncFinished, onLatestSyncProgress } from "../api/sync";
import type { LatestChannel } from "../api/sync";

export type LatestSyncState =
  | { phase: "idle" }
  | { phase: "syncing"; channel: LatestChannel; downloaded: number; total: number }
  | { phase: "done"; channel: LatestChannel; synced: boolean; tag: string | null }
  | { phase: "error"; channel: LatestChannel; message: string };

/** Tracks backend-initiated latest-release/prerelease syncs (background poll
 * or an immediate on-toggle check) for `projectKey`, via the
 * `latest-sync-progress`/`latest-sync-finished` events the Rust side emits.
 * Unlike `useSync`, nothing here is triggered by a call the frontend makes
 * directly -- the backend runs the whole cycle on its own, so this hook only
 * listens. */
export function useLatestSync(projectKey: string) {
  const [state, setState] = useState<LatestSyncState>({ phase: "idle" });

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;
    let cancelled = false;

    onLatestSyncProgress((progress) => {
      if (progress.project_key !== projectKey) {
        return;
      }
      setState({
        phase: "syncing",
        channel: progress.channel,
        downloaded: progress.downloaded,
        total: progress.total,
      });
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenProgress = fn;
      }
    });

    onLatestSyncFinished((result) => {
      if (result.project_key !== projectKey) {
        return;
      }
      if (result.error) {
        setState({ phase: "error", channel: result.channel, message: result.error });
      } else {
        setState({ phase: "done", channel: result.channel, synced: result.synced, tag: result.tag });
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenFinished = fn;
      }
    });

    return () => {
      cancelled = true;
      unlistenProgress?.();
      unlistenFinished?.();
    };
  }, [projectKey]);

  return { state };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test -- src/hooks/useLatestSync.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useLatestSync.ts src/hooks/useLatestSync.test.ts
git commit -m "Add useLatestSync hook for backend-initiated latest syncs"
```

---

## Task 8: `BuildBrowser` pinned rows and latest-mode support

**Files:**
- Modify: `src/components/BuildBrowser.tsx`
- Modify: `src/components/BuildBrowser.test.tsx`
- Modify: `src/App.css`

- [ ] **Step 1: Update the test file for the new required props and add latest-mode tests**

Replace the full contents of `src/components/BuildBrowser.test.tsx` with:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { BuildBrowser } from "./BuildBrowser";
import type { Release } from "../api/projects";

const releases: Release[] = [
  {
    id: 1,
    tag_name: "0.2.14",
    name: "LastBeacon 0.2.14",
    prerelease: false,
    published_at: "2026-09-01T00:00:00Z",
    assets: [
      { id: 10, name: "shipping.zip", size: 100, browser_download_url: "https://example.com/a" },
      { id: 11, name: "test.zip", size: 100, browser_download_url: "https://example.com/b" },
    ],
  },
  {
    id: 2,
    tag_name: "0.2.13",
    name: "LastBeacon 0.2.13",
    prerelease: false,
    published_at: "2026-08-01T00:00:00Z",
    assets: [{ id: 8, name: "shipping.zip", size: 100, browser_download_url: "https://example.com/c" }],
  },
  {
    id: 3,
    tag_name: "0.3.0-rc1",
    name: "LastBeacon 0.3.0-rc1",
    prerelease: true,
    published_at: "2026-09-10T00:00:00Z",
    assets: [{ id: 30, name: "ps4.zip", size: 100, browser_download_url: "https://example.com/rc" }],
  },
];

const noop = () => {};

function renderBrowser(overrides: Partial<React.ComponentProps<typeof BuildBrowser>> = {}) {
  return render(
    <BuildBrowser
      releases={releases}
      syncMode="manual"
      selectedReleaseTag={null}
      onSelectRelease={noop}
      onSelectLatest={noop}
      tickedConfigs={[]}
      onToggleConfig={noop}
      syncedConfigs={[]}
      onSync={noop}
      onSyncLatest={noop}
      disabled={false}
      {...overrides}
    />,
  );
}

describe("BuildBrowser", () => {
  it("lists releases and prereleases, labeling prereleases distinctly", () => {
    renderBrowser();

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.3.0-rc1 (prerelease)" })).toBeInTheDocument();
  });

  it("always shows the pinned Latest release and Latest prerelease rows", () => {
    renderBrowser();

    expect(screen.getByRole("button", { name: "Latest release" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Latest prerelease" })).toBeInTheDocument();
  });

  it("filters the release list by the search box", () => {
    renderBrowser();

    fireEvent.change(screen.getByLabelText("Search releases"), { target: { value: "0.2.13" } });

    expect(screen.queryByRole("button", { name: "LastBeacon 0.2.14" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
  });

  it("lists every build config across all releases, including prerelease-only ones", () => {
    renderBrowser();

    expect(screen.getByLabelText("shipping.zip")).toBeInTheDocument();
    expect(screen.getByLabelText("test.zip")).toBeInTheDocument();
    expect(screen.getByLabelText("ps4.zip")).toBeInTheDocument();
  });

  it("calls onSelectRelease when a release row is clicked", () => {
    const onSelectRelease = vi.fn();
    renderBrowser({ onSelectRelease });

    fireEvent.click(screen.getByRole("button", { name: "LastBeacon 0.2.14" }));

    expect(onSelectRelease).toHaveBeenCalledWith("0.2.14");
  });

  it("marks the selected release's button as pressed", () => {
    renderBrowser({ selectedReleaseTag: "0.2.13" });

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toHaveAttribute("aria-pressed", "true");
  });

  it("disables a config checkbox when the selected release has no matching asset", () => {
    renderBrowser({ selectedReleaseTag: "0.2.13" });

    // 0.2.13 only has shipping.zip -- test.zip must be disabled for it.
    expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
    expect(screen.getByLabelText("test.zip")).toBeDisabled();
  });

  it("leaves every config checkbox enabled when no release is selected yet", () => {
    renderBrowser();

    expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
    expect(screen.getByLabelText("test.zip")).not.toBeDisabled();
  });

  it("calls onToggleConfig with the config name and new checked state", () => {
    const onToggleConfig = vi.fn();
    renderBrowser({ onToggleConfig });

    fireEvent.click(screen.getByLabelText("shipping.zip"));

    expect(onToggleConfig).toHaveBeenCalledWith("shipping.zip", true);
  });

  it("disables the sync button when nothing ticked is available for the selected release", () => {
    renderBrowser({ selectedReleaseTag: "0.2.14" });

    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  it("shows Sync and calls onSync with only the missing ticked assets", () => {
    const onSync = vi.fn();
    renderBrowser({
      selectedReleaseTag: "0.2.14",
      tickedConfigs: ["shipping.zip", "test.zip"],
      syncedConfigs: ["test.zip"],
      onSync,
    });

    fireEvent.click(screen.getByRole("button", { name: "Sync" }));

    expect(onSync).toHaveBeenCalledWith(releases[0], [releases[0].assets[0]]);
  });

  it("shows a checkmark and re-verifies every ticked asset once fully synced", () => {
    const onSync = vi.fn();
    renderBrowser({
      selectedReleaseTag: "0.2.14",
      tickedConfigs: ["shipping.zip"],
      syncedConfigs: ["shipping.zip"],
      onSync,
    });

    fireEvent.click(screen.getByRole("button", { name: "✓ Synced" }));

    expect(onSync).toHaveBeenCalledWith(releases[0], [releases[0].assets[0]]);
  });

  it("disables the search box, release rows, checkboxes, and sync button when disabled", () => {
    renderBrowser({ selectedReleaseTag: "0.2.14", tickedConfigs: ["shipping.zip"], disabled: true });

    expect(screen.getByLabelText("Search releases")).toBeDisabled();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeDisabled();
    expect(screen.getByLabelText("shipping.zip")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  describe("latest mode", () => {
    it("marks Latest release pressed and no concrete release pressed when syncMode is latest_release", () => {
      renderBrowser({ syncMode: "latest_release", selectedReleaseTag: "0.2.13" });

      expect(screen.getByRole("button", { name: "Latest release" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toHaveAttribute("aria-pressed", "false");
    });

    it("clicking the Latest prerelease row calls onSelectLatest", () => {
      const onSelectLatest = vi.fn();
      renderBrowser({ onSelectLatest });

      fireEvent.click(screen.getByRole("button", { name: "Latest prerelease" }));

      expect(onSelectLatest).toHaveBeenCalledWith("latest_prerelease");
    });

    it("resolves available configs against the most recently published matching release", () => {
      // latest_release should resolve to 0.2.14 (most recent non-prerelease),
      // not 0.3.0-rc1 (most recent overall, but a prerelease).
      renderBrowser({ syncMode: "latest_release", tickedConfigs: ["shipping.zip", "test.zip"] });

      expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
      expect(screen.getByLabelText("test.zip")).not.toBeDisabled();
    });

    it("resolves Latest prerelease's available configs to the prerelease release's assets", () => {
      renderBrowser({ syncMode: "latest_prerelease", tickedConfigs: ["shipping.zip", "ps4.zip"] });

      expect(screen.getByLabelText("ps4.zip")).not.toBeDisabled();
      expect(screen.getByLabelText("shipping.zip")).toBeDisabled();
    });

    it("disables Sync when nothing is ticked in latest mode", () => {
      renderBrowser({ syncMode: "latest_release" });

      expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
    });

    it("clicking Sync in latest mode calls onSyncLatest instead of onSync", () => {
      const onSync = vi.fn();
      const onSyncLatest = vi.fn();
      renderBrowser({ syncMode: "latest_release", tickedConfigs: ["shipping.zip"], onSync, onSyncLatest });

      fireEvent.click(screen.getByRole("button", { name: "Sync" }));

      expect(onSyncLatest).toHaveBeenCalled();
      expect(onSync).not.toHaveBeenCalled();
    });

    it("shows the checkmark in latest mode once every ticked config is synced", () => {
      renderBrowser({ syncMode: "latest_release", tickedConfigs: ["shipping.zip"], syncedConfigs: ["shipping.zip"] });

      expect(screen.getByRole("button", { name: "✓ Synced" })).toBeInTheDocument();
    });
  });

  describe("projects whose asset names embed the release version", () => {
    // Mirrors this app's own Tauri-built installer naming convention
    // ("pixel-build-manager_0.0.3_x64-setup.exe") -- without stripping the
    // release's own version out of the asset name first, every release
    // would register as a brand new build config instead of the one config
    // it actually is.
    const versionedReleases: Release[] = [
      {
        id: 1,
        tag_name: "0.0.3",
        name: "Pixel Build Manager 0.0.3",
        prerelease: true,
        published_at: "2026-09-23T00:00:00Z",
        assets: [
          {
            id: 100,
            name: "pixel-build-manager_0.0.3_x64-setup.exe",
            size: 100,
            browser_download_url: "https://example.com/a",
          },
        ],
      },
      {
        id: 2,
        tag_name: "0.0.2",
        name: "Pixel Build Manager 0.0.2",
        prerelease: true,
        published_at: "2026-09-20T00:00:00Z",
        assets: [
          {
            id: 90,
            name: "pixel-build-manager_0.0.2_x64-setup.exe",
            size: 100,
            browser_download_url: "https://example.com/b",
          },
        ],
      },
    ];

    it("collapses per-release versioned asset names into a single build config", () => {
      renderBrowser({ releases: versionedReleases });

      expect(screen.getAllByLabelText("pixel-build-manager_x64-setup.exe")).toHaveLength(1);
    });

    it("ticking the collapsed config and syncing passes the selected release's actual asset", () => {
      const onSync = vi.fn();
      renderBrowser({
        releases: versionedReleases,
        selectedReleaseTag: "0.0.3",
        tickedConfigs: ["pixel-build-manager_x64-setup.exe"],
        onSync,
      });

      fireEvent.click(screen.getByRole("button", { name: "Sync" }));

      expect(onSync).toHaveBeenCalledWith(versionedReleases[0], [versionedReleases[0].assets[0]]);
    });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm run test -- src/components/BuildBrowser.test.tsx`
Expected: FAIL — TypeScript errors for unknown props (`syncMode`, `onSelectLatest`, `onSyncLatest`), and missing "Latest release"/"Latest prerelease" buttons.

- [ ] **Step 3: Implement the component changes**

Replace the full contents of `src/components/BuildBrowser.tsx` with:

```tsx
import { useMemo, useState } from "react";
import type { Release, ReleaseAsset } from "../api/projects";
import type { SyncMode } from "../api/sync";

function releaseLabel(release: Release): string {
  const base = release.name ?? release.tag_name;
  return release.prerelease ? `${base} (prerelease)` : base;
}

// A build config's *identity* shouldn't include the release's own version --
// otherwise a project whose CI embeds the version in the artifact filename
// (e.g. Tauri/Electron installers: "app_0.0.3_x64-setup.exe") gets a brand
// new "config" every single release, instead of the one stable config it
// actually is. Stripping the release's own tag out of the asset name before
// treating it as a config identity collapses those back into one, the same
// way a project like last-beacon (whose asset names never vary release to
// release, e.g. "last-beacon-windows-x64-shipping.tar.gz") already worked.
//
// Mirrored in Rust as `config_template()` in `sync/latest.rs`, since a
// latest-mode sync must resolve the same identity without any frontend
// involved.
function configTemplate(release: Release, assetName: string): string {
  if (!release.tag_name) {
    return assetName;
  }
  return assetName
    .split(release.tag_name)
    .join("")
    .replace(/([._-])\1+/g, "$1")
    .replace(/^[._-]+|[._-]+$/g, "");
}

// Picks the most recently published release matching `prerelease`, mirroring
// the backend's `resolve_latest_release()` so the UI's "available configs"
// checkboxes agree with whatever the backend will actually sync.
function resolveLatest(releases: Release[], prerelease: boolean): Release | null {
  const matching = releases.filter((release) => release.prerelease === prerelease);
  const sorted = [...matching].sort((a, b) => {
    if (a.published_at === b.published_at) return 0;
    if (a.published_at === null) return 1;
    if (b.published_at === null) return -1;
    return b.published_at.localeCompare(a.published_at);
  });
  return sorted[0] ?? null;
}

type Props = {
  releases: Release[];
  syncMode: SyncMode;
  selectedReleaseTag: string | null;
  onSelectRelease: (tag: string) => void;
  onSelectLatest: (mode: "latest_release" | "latest_prerelease") => void;
  tickedConfigs: string[];
  onToggleConfig: (configName: string, ticked: boolean) => void;
  /** Configs already extracted for the *effective* release: the selected
   * release in manual mode, or whatever the active latest mode currently
   * resolves to. */
  syncedConfigs: string[];
  onSync: (release: Release, assets: ReleaseAsset[]) => void;
  onSyncLatest: () => void;
  disabled: boolean;
};

export function BuildBrowser({
  releases,
  syncMode,
  selectedReleaseTag,
  onSelectRelease,
  onSelectLatest,
  tickedConfigs,
  onToggleConfig,
  syncedConfigs,
  onSync,
  onSyncLatest,
  disabled,
}: Props) {
  const [search, setSearch] = useState("");
  const isLatestMode = syncMode !== "manual";

  // Every build config the project has ever produced, across every release
  // -- including prereleases -- deliberately not scoped to the releases
  // visible below, so a config that's so far only shipped in a prerelease
  // still shows up as a future sync target once a real release adds it.
  const buildConfigs = useMemo(() => {
    const names = new Set<string>();
    for (const release of releases) {
      for (const asset of release.assets) {
        names.add(configTemplate(release, asset.name));
      }
    }
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  }, [releases]);

  const visibleReleases = useMemo(() => {
    const term = search.toLowerCase();
    return releases.filter((release) => (release.name ?? release.tag_name).toLowerCase().includes(term));
  }, [releases, search]);

  const effectiveRelease: Release | null = isLatestMode
    ? resolveLatest(releases, syncMode === "latest_prerelease")
    : releases.find((release) => release.tag_name === selectedReleaseTag) ?? null;

  const availableTickedAssets: ReleaseAsset[] = effectiveRelease
    ? effectiveRelease.assets.filter((asset) =>
        tickedConfigs.includes(configTemplate(effectiveRelease, asset.name)),
      )
    : [];
  const missingAssets = availableTickedAssets.filter((asset) => !syncedConfigs.includes(asset.name));

  const fullySynced = isLatestMode
    ? tickedConfigs.length > 0 && tickedConfigs.every((config) => syncedConfigs.includes(config))
    : availableTickedAssets.length > 0 && missingAssets.length === 0;

  const syncDisabled = disabled || (isLatestMode ? tickedConfigs.length === 0 : availableTickedAssets.length === 0);

  const handleSyncClick = () => {
    if (isLatestMode) {
      if (tickedConfigs.length === 0) {
        return;
      }
      onSyncLatest();
      return;
    }
    if (!effectiveRelease || availableTickedAssets.length === 0) {
      return;
    }
    onSync(effectiveRelease, fullySynced ? availableTickedAssets : missingAssets);
  };

  return (
    <div className="build-browser">
      <input
        className="build-browser__search"
        aria-label="Search releases"
        placeholder="Search releases..."
        value={search}
        disabled={disabled}
        onChange={(event) => setSearch(event.target.value)}
      />
      <div className="release-list">
        <button
          className="release-row release-row--pinned"
          aria-pressed={syncMode === "latest_release"}
          disabled={disabled}
          onClick={() => onSelectLatest("latest_release")}
        >
          Latest release
        </button>
        <button
          className="release-row release-row--pinned"
          aria-pressed={syncMode === "latest_prerelease"}
          disabled={disabled}
          onClick={() => onSelectLatest("latest_prerelease")}
        >
          Latest prerelease
        </button>
        {visibleReleases.length === 0 ? (
          <p className="release-list__empty">No releases found.</p>
        ) : (
          visibleReleases.map((release) => (
            <button
              key={release.id}
              className="release-row"
              aria-label={releaseLabel(release)}
              aria-pressed={!isLatestMode && release.tag_name === selectedReleaseTag}
              disabled={disabled}
              onClick={() => onSelectRelease(release.tag_name)}
            >
              <span className="release-row__name">{releaseLabel(release)}</span>
              {release.published_at && (
                <span className="release-row__date">
                  {new Date(release.published_at).toLocaleDateString()}
                </span>
              )}
            </button>
          ))
        )}
      </div>
      <fieldset className="build-configs">
        <legend>Build configs</legend>
        <div className="build-configs__list">
          {buildConfigs.map((config) => {
            const availableForSelected =
              !effectiveRelease ||
              effectiveRelease.assets.some((asset) => configTemplate(effectiveRelease, asset.name) === config);
            return (
              <label key={config} className="config-chip">
                <input
                  type="checkbox"
                  aria-label={config}
                  checked={tickedConfigs.includes(config)}
                  disabled={disabled || !availableForSelected}
                  onChange={(event) => onToggleConfig(config, event.target.checked)}
                />
                {config}
              </label>
            );
          })}
        </div>
      </fieldset>
      <button
        className={`sync-button${fullySynced ? " sync-button--synced" : ""}`}
        disabled={syncDisabled}
        onClick={handleSyncClick}
      >
        {fullySynced ? "✓ Synced" : "Sync"}
      </button>
    </div>
  );
}
```

Add this to `src/App.css`, right after the existing `.release-row__date { ... }` rule:

```css
.release-row--pinned {
  font-style: italic;
}

.release-list > .release-row--pinned:nth-of-type(2) {
  border-bottom: 2px solid var(--color-border);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test -- src/components/BuildBrowser.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/BuildBrowser.tsx src/components/BuildBrowser.test.tsx src/App.css
git commit -m "Add pinned Latest release/prerelease rows to BuildBrowser"
```

---

## Task 9: `SyncedBuildControls` latest-channel variant

**Files:**
- Modify: `src/components/SyncedBuildControls.tsx`
- Modify: `src/components/SyncedBuildControls.test.tsx`

- [ ] **Step 1: Write the failing tests**

Add this `describe` block to the end of `src/components/SyncedBuildControls.test.tsx`, before the final closing of the outer `describe("SyncedBuildControls", ...)`:

```tsx
  describe("latest-channel mode", () => {
    it("renders both buttons using the latest-channel lookups", async () => {
      vi.mocked(syncApi.getLatestBuildDir).mockResolvedValue("D:\\Builds\\org\\repo\\latest\\release\\shipping.zip");
      vi.mocked(syncApi.getLatestBuildExecutable).mockResolvedValue(
        "D:\\Builds\\org\\repo\\latest\\release\\shipping.zip\\game.exe",
      );

      render(
        <SyncedBuildControls
          projectKey="org/repo"
          latestChannel="release"
          configName="shipping.zip"
          disabled={false}
        />,
      );

      expect(await screen.findByRole("button", { name: "Open folder for shipping.zip" })).toBeInTheDocument();
      expect(await screen.findByRole("button", { name: "Launch shipping.zip" })).toBeInTheDocument();
      expect(syncApi.getLatestBuildDir).toHaveBeenCalledWith("org/repo", "release", "shipping.zip");
    });

    it("launches via launchLatestBuild when latestChannel is set", async () => {
      vi.mocked(syncApi.getLatestBuildDir).mockResolvedValue(null);
      vi.mocked(syncApi.getLatestBuildExecutable).mockResolvedValue("D:\\Builds\\shipping.zip\\game.exe");
      vi.mocked(syncApi.launchLatestBuild).mockResolvedValue(undefined);

      render(
        <SyncedBuildControls
          projectKey="org/repo"
          latestChannel="prerelease"
          configName="shipping.zip"
          disabled={false}
        />,
      );

      fireEvent.click(await screen.findByRole("button", { name: "Launch shipping.zip" }));

      await waitFor(() =>
        expect(syncApi.launchLatestBuild).toHaveBeenCalledWith("org/repo", "prerelease", "shipping.zip"),
      );
    });
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm run test -- src/components/SyncedBuildControls.test.tsx`
Expected: FAIL — TypeScript error for the unknown `latestChannel` prop.

- [ ] **Step 3: Implement the latest-channel variant**

Replace the full contents of `src/components/SyncedBuildControls.tsx` with:

```tsx
import { useEffect, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  getBuildDir,
  getBuildExecutable,
  getLatestBuildDir,
  getLatestBuildExecutable,
  launchBuild,
  launchLatestBuild,
} from "../api/sync";
import type { LatestChannel } from "../api/sync";

type Props =
  | {
      projectKey: string;
      releaseTag: string;
      latestChannel?: undefined;
      configName: string;
      disabled: boolean;
    }
  | {
      projectKey: string;
      releaseTag?: undefined;
      latestChannel: LatestChannel;
      configName: string;
      disabled: boolean;
    };

export function SyncedBuildControls(props: Props) {
  const { projectKey, configName, disabled } = props;
  const [buildDir, setBuildDir] = useState<string | null>(null);
  const [executable, setExecutable] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (props.latestChannel) {
      getLatestBuildDir(projectKey, props.latestChannel, configName)
        .then(setBuildDir)
        .catch((err) => console.error("failed to look up build dir", err));
      getLatestBuildExecutable(projectKey, props.latestChannel, configName)
        .then(setExecutable)
        .catch((err) => console.error("failed to look up build executable", err));
    } else {
      getBuildDir(projectKey, props.releaseTag, configName)
        .then(setBuildDir)
        .catch((err) => console.error("failed to look up build dir", err));
      getBuildExecutable(projectKey, props.releaseTag, configName)
        .then(setExecutable)
        .catch((err) => console.error("failed to look up build executable", err));
    }
  }, [projectKey, props.releaseTag, props.latestChannel, configName]);

  const handleOpenFolder = async () => {
    setError(null);
    if (!buildDir) {
      return;
    }
    try {
      await openPath(buildDir);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleLaunch = async () => {
    setError(null);
    try {
      if (props.latestChannel) {
        await launchLatestBuild(projectKey, props.latestChannel, configName);
      } else {
        await launchBuild(projectKey, props.releaseTag, configName);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="synced-build-row">
      <span className="synced-build-row__name">{configName}</span>
      {buildDir && (
        <button aria-label={`Open folder for ${configName}`} disabled={disabled} onClick={handleOpenFolder}>
          Open Folder
        </button>
      )}
      {executable && (
        <button
          className="synced-build-row__launch"
          aria-label={`Launch ${configName}`}
          disabled={disabled}
          onClick={handleLaunch}
        >
          ▶ Launch
        </button>
      )}
      {error && <p className="error-text">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test -- src/components/SyncedBuildControls.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/SyncedBuildControls.tsx src/components/SyncedBuildControls.test.tsx
git commit -m "Add latest-channel variant to SyncedBuildControls"
```

---

## Task 10: Wire it all together in `App.tsx`

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Update the test baseline and add integration tests**

In `src/App.test.tsx`, update `mockProjectDetailBaseline()` to also mock the new API surface:

```ts
function mockProjectDetailBaseline() {
  vi.mocked(projectsApi.listReleasesForProject).mockResolvedValue([release]);
  vi.mocked(syncApi.getSelectedRelease).mockResolvedValue(null);
  vi.mocked(syncApi.setSelectedRelease).mockResolvedValue(undefined);
  vi.mocked(syncApi.getTickedConfigs).mockResolvedValue([]);
  vi.mocked(syncApi.setTickedConfigs).mockResolvedValue(undefined);
  vi.mocked(syncApi.listSyncedConfigs).mockResolvedValue([]);
  vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
  vi.mocked(syncApi.getSyncMode).mockResolvedValue("manual");
  vi.mocked(syncApi.setSyncMode).mockResolvedValue(undefined);
  vi.mocked(syncApi.checkLatestNow).mockResolvedValue(undefined);
  vi.mocked(syncApi.listSyncedLatestConfigs).mockResolvedValue([]);
  vi.mocked(syncApi.getLatestBuildDir).mockResolvedValue(null);
  vi.mocked(syncApi.getLatestBuildExecutable).mockResolvedValue(null);
  vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));
}
```

Add these two tests to the `describe("ProjectDetail", ...)` block, after the existing `"shows the busy overlay while a sync is in flight"` test:

```tsx
  it("selecting Latest release switches sync mode, persists it, and shows latest configs' controls", async () => {
    mockProjectDetailBaseline();
    vi.mocked(syncApi.getTickedConfigs).mockResolvedValue(["shipping.zip"]);
    vi.mocked(syncApi.listSyncedLatestConfigs).mockResolvedValue(["shipping.zip"]);
    vi.mocked(syncApi.getLatestBuildDir).mockResolvedValue(
      "D:\\Builds\\org\\repo\\latest\\release\\shipping.zip",
    );

    render(<ProjectDetail projectKey="org/repo" />);
    fireEvent.click(await screen.findByRole("button", { name: "Latest release" }));

    await waitFor(() => expect(syncApi.setSyncMode).toHaveBeenCalledWith("org/repo", "latest_release"));
    expect(await screen.findByRole("button", { name: "Open folder for shipping.zip" })).toBeInTheDocument();
  });

  it("clicking Sync while in a latest mode calls checkLatestNow instead of syncing a concrete release", async () => {
    mockProjectDetailBaseline();
    vi.mocked(syncApi.getSyncMode).mockResolvedValue("latest_release");
    vi.mocked(syncApi.getTickedConfigs).mockResolvedValue(["shipping.zip"]);

    render(<ProjectDetail projectKey="org/repo" />);
    fireEvent.click(await screen.findByRole("button", { name: "Sync" }));

    await waitFor(() => expect(syncApi.checkLatestNow).toHaveBeenCalledWith("org/repo"));
    expect(syncApi.syncReleaseAsset).not.toHaveBeenCalled();
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm run test -- src/App.test.tsx`
Expected: FAIL — `BuildBrowser` inside `ProjectDetail` is missing the required `syncMode`/`onSelectLatest`/`onSyncLatest` props (TypeScript error) and `getSyncMode` isn't called yet.

- [ ] **Step 3: Wire `ProjectDetail` up to sync mode and the latest-sync hook**

In `src/App.tsx`, update the imports:

```tsx
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { isLoggedIn, logout, SESSION_EXPIRED_ERROR } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { bindProject, getWorkspaceRoot, listBoundProjects, unbindProject } from "./api/settings";
import {
  checkLatestNow,
  getSelectedRelease,
  getSyncMode,
  getTickedConfigs,
  listSyncedConfigs,
  listSyncedLatestConfigs,
  setSelectedRelease,
  setSyncMode,
  setTickedConfigs,
  type LatestChannel,
  type SyncMode,
} from "./api/sync";
import { getVersionLabel } from "./api/version";
import { BindProjectPopup } from "./components/BindProjectPopup";
import { BuildBrowser } from "./components/BuildBrowser";
import { BusyOverlay } from "./components/BusyOverlay";
import { ClearCacheButton } from "./components/ClearCacheButton";
import { Login } from "./components/Login";
import { SyncedBuildControls } from "./components/SyncedBuildControls";
import { SyncStatus } from "./components/SyncStatus";
import { TabBar } from "./components/TabBar";
import { ThemeToggle } from "./components/ThemeToggle";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { useLatestSync } from "./hooks/useLatestSync";
import { useSync } from "./hooks/useSync";
import { useTheme } from "./hooks/useTheme";
import "./App.css";
```

Replace the `ProjectDetail` function body with:

```tsx
export function ProjectDetail({ projectKey }: { projectKey: string }) {
  const [releases, setReleases] = useState<Release[]>([]);
  const [selectedReleaseTag, setSelectedReleaseTagState] = useState<string | null>(null);
  const [tickedConfigs, setTickedConfigsState] = useState<string[]>([]);
  const [syncedConfigs, setSyncedConfigs] = useState<string[]>([]);
  const [syncMode, setSyncModeState] = useState<SyncMode>("manual");
  const { state, syncConfigs } = useSync(projectKey);
  const { state: latestState } = useLatestSync(projectKey);

  useEffect(() => {
    listReleasesForProject(projectKey)
      .then(setReleases)
      .catch((error) => console.error("failed to load releases", error));
    getSelectedRelease(projectKey)
      .then(setSelectedReleaseTagState)
      .catch((error) => console.error("failed to load selected release", error));
    getTickedConfigs(projectKey)
      .then(setTickedConfigsState)
      .catch((error) => console.error("failed to load ticked configs", error));
    getSyncMode(projectKey)
      .then(setSyncModeState)
      .catch((error) => console.error("failed to load sync mode", error));
  }, [projectKey]);

  useEffect(() => {
    if (syncMode === "manual") {
      if (!selectedReleaseTag) {
        setSyncedConfigs([]);
        return;
      }
      listSyncedConfigs(projectKey, selectedReleaseTag)
        .then(setSyncedConfigs)
        .catch((error) => console.error("failed to list synced configs", error));
      return;
    }
    const channel: LatestChannel = syncMode === "latest_prerelease" ? "prerelease" : "release";
    listSyncedLatestConfigs(projectKey, channel)
      .then(setSyncedConfigs)
      .catch((error) => console.error("failed to list synced latest configs", error));
  }, [projectKey, syncMode, selectedReleaseTag, latestState.phase]);

  const isManualSyncing = state.phase === "syncing";
  const isLatestSyncing = latestState.phase === "syncing";
  const isBusy = isManualSyncing || isLatestSyncing;

  const handleSelectRelease = (tag: string) => {
    setSyncModeState("manual");
    setSelectedReleaseTagState(tag);
    setSyncMode(projectKey, "manual").catch((error) => console.error("failed to persist sync mode", error));
    setSelectedRelease(projectKey, tag).catch((error) =>
      console.error("failed to persist selected release", error),
    );
  };

  const handleSelectLatest = (mode: "latest_release" | "latest_prerelease") => {
    setSyncModeState(mode);
    setSyncMode(projectKey, mode).catch((error) => console.error("failed to persist sync mode", error));
  };

  const handleToggleConfig = (configName: string, ticked: boolean) => {
    setTickedConfigsState((prev) => {
      const next = ticked ? [...prev, configName] : prev.filter((c) => c !== configName);
      setTickedConfigs(projectKey, next).catch((error) =>
        console.error("failed to persist ticked configs", error),
      );
      return next;
    });
  };

  const handleSync = async (release: Release, assets: ReleaseAsset[]) => {
    let succeeded = false;
    try {
      succeeded = await syncConfigs(release, assets);
    } catch (error) {
      console.error("failed to sync build configs", error);
      return;
    }
    if (succeeded && selectedReleaseTag === release.tag_name) {
      try {
        setSyncedConfigs(await listSyncedConfigs(projectKey, release.tag_name));
      } catch (error) {
        console.error("failed to refresh synced configs", error);
      }
    }
  };

  const handleSyncLatest = () => {
    checkLatestNow(projectKey).catch((error) => console.error("failed to trigger latest sync", error));
  };

  const busyLabel = isManualSyncing
    ? state.downloaded >= state.total
      ? `Finishing up ${state.configName}...`
      : `Downloading ${state.configName}: ${Math.round((state.downloaded / state.total) * 100)}%`
    : isLatestSyncing
      ? `Syncing latest ${latestState.channel}: ${Math.round((latestState.downloaded / latestState.total) * 100)}%`
      : undefined;

  const showSyncedBuilds = (syncMode !== "manual" || selectedReleaseTag !== null) && syncedConfigs.length > 0;

  return (
    <div className="project-detail">
      <BusyOverlay active={isBusy} label={busyLabel}>
        <BuildBrowser
          releases={releases}
          syncMode={syncMode}
          selectedReleaseTag={selectedReleaseTag}
          onSelectRelease={handleSelectRelease}
          onSelectLatest={handleSelectLatest}
          tickedConfigs={tickedConfigs}
          onToggleConfig={handleToggleConfig}
          syncedConfigs={syncedConfigs}
          onSync={handleSync}
          onSyncLatest={handleSyncLatest}
          disabled={isBusy}
        />
        <SyncStatus state={state} />
        {showSyncedBuilds && (
          <div className="synced-builds">
            {syncedConfigs.map((config) =>
              syncMode === "manual" ? (
                <SyncedBuildControls
                  key={config}
                  projectKey={projectKey}
                  releaseTag={selectedReleaseTag as string}
                  configName={config}
                  disabled={isBusy}
                />
              ) : (
                <SyncedBuildControls
                  key={config}
                  projectKey={projectKey}
                  latestChannel={syncMode === "latest_prerelease" ? "prerelease" : "release"}
                  configName={config}
                  disabled={isBusy}
                />
              ),
            )}
          </div>
        )}
      </BusyOverlay>
      <ClearCacheButton
        projectKey={projectKey}
        onCleared={() => setSyncedConfigs([])}
        disabled={isBusy}
      />
    </div>
  );
}
```

The rest of `App.tsx` (the `App` function and its exports) is unchanged.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test -- src/App.test.tsx`
Expected: PASS, including all pre-existing `ProjectDetail`/`App` tests.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/App.test.tsx
git commit -m "Wire latest-mode sync into ProjectDetail"
```

---

## Task 11: Full verification pass

**Files:** none (verification only)

- [ ] **Step 1: Format and lint the Rust code**

Run (from `src-tauri`): `cargo fmt`
Then: `cargo clippy --all-targets -- -D warnings`
Expected: clippy reports no warnings. If `cargo fmt` changed anything, that's expected — it'll be included in this task's commit.

- [ ] **Step 2: Run the full Rust test suite**

Run (from `src-tauri`): `cargo test`
Expected: PASS, all tests across the whole crate.

- [ ] **Step 3: Build the Rust backend**

Run (from `src-tauri`): `cargo build`
Expected: succeeds with no errors.

- [ ] **Step 4: Build and test the frontend**

Run (from the repo root): `npm run build`
Expected: succeeds (type-checks and bundles).

Run: `npm run test`
Expected: PASS, all tests across the whole frontend suite.

- [ ] **Step 5: Commit any formatting fixes**

```bash
git add -A
git commit -m "Apply cargo fmt"
```

(Skip this step entirely if `cargo fmt` in Step 1 made no changes — there's nothing to commit.)
