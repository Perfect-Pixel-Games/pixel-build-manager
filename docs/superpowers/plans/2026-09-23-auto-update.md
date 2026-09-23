# In-App Auto-Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make packaged builds of Pixel Build Manager check GitHub for a newer release matching their own channel (release or prerelease), download it, and restart into it automatically — silently, on a 15-minute background poll starting at launch — while debug (`tauri dev`) builds never do any of this.

**Architecture:** A new `updater` Rust module resolves the correct GitHub update-manifest URL for the binary's build-time-baked channel (release → GitHub's `latest` alias; prerelease → an API lookup for the newest `prerelease: true` release), then hands that URL to the official `tauri-plugin-updater` to check/download/install/restart. The whole flow is backend-only — no frontend changes. CI is extended to sign the existing NSIS installer and publish a `latest.json` manifest alongside it on every release.

**Tech Stack:** `tauri-plugin-updater` (Rust + its `UpdaterExt` trait), the existing `GithubClient` (extended to support unauthenticated requests), `wiremock` for HTTP-level Rust tests, GitHub Actions (`softprops/action-gh-release`, PowerShell steps) for the CI signing/manifest changes.

Spec: `docs/superpowers/specs/2026-09-23-auto-update-design.md`

---

## Task 1: Unauthenticated `GithubClient` support

The update check must work before the user logs in (the repo is public, so no token is needed), but `GithubClient` currently always sends a bearer token. Add an anonymous constructor and make the token header conditional.

**Files:**
- Modify: `src-tauri/src/github/client.rs`

- [ ] **Step 1: Write the failing test for the anonymous constructor**

Add to the `tests` module at the bottom of `src-tauri/src/github/client.rs`:

```rust
    #[test]
    fn anonymous_has_no_token() {
        let client = GithubClient::anonymous();

        assert_eq!(client.token(), "");
    }
```

- [ ] **Step 2: Run it to confirm it fails to compile**

Run: `cd src-tauri && cargo test anonymous_has_no_token`
Expected: FAIL — `no function or associated item named 'anonymous' found for struct 'GithubClient'`

- [ ] **Step 3: Add the `anonymous()` constructor**

In `src-tauri/src/github/client.rs`, add next to the existing `new`/`with_base_url` constructors:

```rust
    /// A client with no auth token, for calling public, unauthenticated
    /// endpoints (e.g. checking this app's own public repo for updates
    /// before the user has logged in).
    pub fn anonymous() -> Self {
        Self::with_base_url(String::new(), "https://api.github.com".to_string())
    }
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `cd src-tauri && cargo test anonymous_has_no_token`
Expected: PASS

- [ ] **Step 5: Write the failing test for skipping the auth header when the token is empty**

Add to the same `tests` module. This needs a new import — add `header_exists` to the existing `use wiremock::matchers::{...)}` line at the top of the `tests` module so it reads:

```rust
    use wiremock::matchers::{header_exists, method, path, query_param};
```

Then add the test:

```rust
    #[tokio::test]
    async fn anonymous_client_sends_no_authorization_header() {
        let server = MockServer::start().await;
        // If the client (incorrectly) sends an Authorization header, this
        // mock catches it and fails the request; the real assertion is that
        // the second, headerless mock is the one that actually responds.
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/releases"))
            .and(header_exists("Authorization"))
            .respond_with(ResponseTemplate::new(401).set_body_string("must not send auth"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let client = GithubClient::anonymous_with_base_url(server.uri());
        let releases = client.list_releases("owner", "repo").await.unwrap();

        assert!(releases.is_empty());
    }
```

This test references `anonymous_with_base_url`, which doesn't exist yet (the plain `anonymous()` constructor hardcodes the real GitHub API base URL, which a test can't point at a mock server). Add it in the same step as a second constructor, next to `anonymous()`:

```rust
    /// Like `anonymous()`, but pointed at a custom base URL. Exists purely
    /// so tests can exercise the anonymous/no-token request path against a
    /// mock server instead of the real GitHub API.
    #[cfg(test)]
    pub fn anonymous_with_base_url(base_url: String) -> Self {
        Self::with_base_url(String::new(), base_url)
    }
```

- [ ] **Step 6: Run it to confirm it fails**

Run: `cd src-tauri && cargo test anonymous_client_sends_no_authorization_header`
Expected: FAIL — the request still carries `Authorization: Bearer ` (an empty bearer token is still a present header), so the first mock matches and returns 401, causing `.unwrap()` to panic.

- [ ] **Step 7: Make the auth header conditional**

In `src-tauri/src/github/client.rs`, replace the `request` method:

```rust
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "pixel-build-manager")
    }
```

with:

```rust
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let builder = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "pixel-build-manager");
        if self.token.is_empty() {
            builder
        } else {
            builder.bearer_auth(&self.token)
        }
    }
```

- [ ] **Step 8: Run both new tests to confirm they pass**

Run: `cd src-tauri && cargo test anonymous`
Expected: both `anonymous_has_no_token` and `anonymous_client_sends_no_authorization_header` PASS

- [ ] **Step 9: Run the full existing test suite to confirm nothing else broke**

Run: `cd src-tauri && cargo test`
Expected: all tests PASS (the existing authenticated tests still send a token, since `"token123"` is non-empty)

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/github/client.rs
git commit -m "Support unauthenticated GithubClient requests for pre-login update checks"
```

---

## Task 2: Update channel type

The binary needs to know, at runtime, whether it was built as a release or prerelease — baked in by CI at compile time (Task 5), never inferred from the version number.

**Files:**
- Create: `src-tauri/src/updater/mod.rs`
- Create: `src-tauri/src/updater/channel.rs`
- Modify: `src-tauri/src/lib.rs` (register the `updater` module)

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, change:

```rust
mod auth;
mod github;
mod settings;
mod sync;
```

to:

```rust
mod auth;
mod github;
mod settings;
mod sync;
mod updater;
```

- [ ] **Step 2: Create the module file**

Create `src-tauri/src/updater/mod.rs`:

```rust
pub mod channel;
```

- [ ] **Step 3: Write the failing tests for channel parsing**

Create `src-tauri/src/updater/channel.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Release,
    Prerelease,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_value_parses_release() {
        assert_eq!(Channel::from_env_value(Some("release")), Some(Channel::Release));
    }

    #[test]
    fn from_env_value_parses_prerelease() {
        assert_eq!(
            Channel::from_env_value(Some("prerelease")),
            Some(Channel::Prerelease)
        );
    }

    #[test]
    fn from_env_value_is_none_when_unset() {
        assert_eq!(Channel::from_env_value(None), None);
    }

    #[test]
    fn from_env_value_is_none_for_unrecognized_values() {
        assert_eq!(Channel::from_env_value(Some("nightly")), None);
    }
}
```

- [ ] **Step 4: Run the tests to confirm they fail**

Run: `cd src-tauri && cargo test channel::tests`
Expected: FAIL — `no function or associated item named 'from_env_value' found for enum 'Channel'`

- [ ] **Step 5: Implement `from_env_value` and `current`**

In `src-tauri/src/updater/channel.rs`, add to the `impl` (create the `impl` block above the `#[cfg(test)]` module):

```rust
impl Channel {
    /// The channel baked into this binary at compile time by CI via the
    /// `PIXEL_BUILD_MANAGER_CHANNEL` env var (see the `package` job in
    /// `.github/workflows/pixel-build-manager-build.yml`). `None` means the
    /// var was unset or unrecognized -- e.g. a local `cargo build` outside
    /// CI -- and callers must treat that as "skip update checks", never
    /// guess a channel.
    pub fn current() -> Option<Channel> {
        Self::from_env_value(option_env!("PIXEL_BUILD_MANAGER_CHANNEL"))
    }

    fn from_env_value(value: Option<&str>) -> Option<Channel> {
        match value {
            Some("release") => Some(Channel::Release),
            Some("prerelease") => Some(Channel::Prerelease),
            _ => None,
        }
    }
}
```

- [ ] **Step 6: Run the tests to confirm they pass**

Run: `cd src-tauri && cargo test channel::tests`
Expected: all 4 tests PASS

- [ ] **Step 7: Run the full test suite and clippy**

Run: `cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS with no warnings

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/updater/mod.rs src-tauri/src/updater/channel.rs
git commit -m "Add build-time update channel detection"
```

---

## Task 3: Update endpoint resolution

Given a channel, resolve the `latest.json` manifest URL to check.

**Files:**
- Create: `src-tauri/src/updater/endpoint.rs`
- Modify: `src-tauri/src/updater/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/updater/endpoint.rs`:

```rust
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

        assert!(matches!(result, Err(EndpointError::NoPrereleaseFound(_, _))));
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd src-tauri && cargo test endpoint::tests`
Expected: FAIL — `cannot find function 'resolve_update_endpoint' in this scope`

- [ ] **Step 3: Implement `resolve_update_endpoint`**

In `src-tauri/src/updater/endpoint.rs`, add above the `#[cfg(test)]` module:

```rust
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
```

- [ ] **Step 4: Register the new submodule**

In `src-tauri/src/updater/mod.rs`, change:

```rust
pub mod channel;
```

to:

```rust
pub mod channel;
pub mod endpoint;
```

- [ ] **Step 5: Run the tests to confirm they pass**

Run: `cd src-tauri && cargo test endpoint::tests`
Expected: all 3 tests PASS

- [ ] **Step 6: Run the full test suite and clippy**

Run: `cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS with no warnings

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/updater/mod.rs src-tauri/src/updater/endpoint.rs
git commit -m "Add channel-aware update endpoint resolution"
```

---

## Task 4: Background check/install loop

Wire the channel + endpoint logic into the actual `tauri-plugin-updater` plugin: poll every 15 minutes, defer installing while a sync/cache-clear is in flight, install and restart silently. This part isn't unit-testable (it needs a live signed release to exercise for real, per the spec's testing section) — verification here is `cargo build`/`clippy` passing plus the manual end-to-end check called out in Task 5.

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/updater/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the plugin dependency**

In `src-tauri/Cargo.toml`, add a new section after `[dependencies]` (before `[dev-dependencies]`):

```toml
[target.'cfg(any(target_os = "macos", windows, target_os = "linux"))'.dependencies]
tauri-plugin-updater = "2"
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: downloads and compiles `tauri-plugin-updater` and its dependencies with no errors

- [ ] **Step 3: Implement the background loop**

In `src-tauri/src/updater/mod.rs`, replace the full file contents:

```rust
pub mod channel;
pub mod endpoint;

use crate::github::client::GithubClient;
use crate::AppState;
use channel::Channel;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

const POLL_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Starts the background update-check loop. No-op if the binary's channel
/// is unknown (see `Channel::current`) -- callers are expected to only call
/// this for release builds in the first place (see `lib.rs`'s `#[cfg(not(debug_assertions))]`
/// gate), but this is a second, cheap line of defense.
pub fn start_background_updates(app: AppHandle) {
    let Some(channel) = Channel::current() else {
        eprintln!("update channel unknown; skipping auto-update checks");
        return;
    };

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(e) = check_and_install(&app, channel).await {
                eprintln!("update check failed: {e}");
            }
        }
    });
}

async fn check_and_install(app: &AppHandle, channel: Channel) -> Result<(), String> {
    let client = GithubClient::anonymous();
    let endpoint_url = endpoint::resolve_update_endpoint(channel, &client)
        .await
        .map_err(|e| e.to_string())?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint_url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(());
    };

    let has_active_operations = {
        let state = app.state::<AppState>();
        let active = state.active_operations.lock().map_err(|e| e.to_string())?;
        !active.is_empty()
    };
    if has_active_operations {
        // A sync or cache-clear is in flight; try again on the next poll
        // rather than interrupting it with a restart.
        return Ok(());
    }

    update
        .download_and_install(|_chunk_length, _content_length| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    app.restart();
}
```

- [ ] **Step 4: Wire it up in `lib.rs`**

In `src-tauri/src/lib.rs`, add the import next to the other `use` statements (after `use sync::orchestrator::...`):

```rust
use updater::start_background_updates;
```

Then change the `.setup()` closure from:

```rust
        .setup(|app| {
            let settings_path = settings_path_for(app.handle());
            app.manage(AppState {
                token_store: Arc::new(KeyringTokenStore),
                settings_path,
                settings_lock: Mutex::new(()),
                active_operations: Mutex::new(HashSet::new()),
            });
            Ok(())
        })
```

to:

```rust
        .setup(|app| {
            let settings_path = settings_path_for(app.handle());
            app.manage(AppState {
                token_store: Arc::new(KeyringTokenStore),
                settings_path,
                settings_lock: Mutex::new(()),
                active_operations: Mutex::new(HashSet::new()),
            });

            // Debug (`tauri dev`) builds never check for or install updates.
            #[cfg(not(debug_assertions))]
            {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                start_background_updates(app.handle().clone());
            }

            Ok(())
        })
```

- [ ] **Step 5: Verify it builds and existing tests still pass**

Run: `cd src-tauri && cargo build && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all PASS with no warnings. (The new `updater` module code paths in `check_and_install` aren't exercised by `cargo test` since nothing calls them outside the `#[cfg(not(debug_assertions))]`-gated `start_background_updates` — that's expected; see Task 5's manual verification step.)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/updater/mod.rs src-tauri/src/lib.rs
git commit -m "Wire up silent background update checking and installation"
```

---

## Task 5: CI signing, manifest generation, and tauri.conf.json

Make CI produce a signed installer and a `latest.json` manifest on every release, and bake the channel into the binary.

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `.github/workflows/pixel-build-manager-build.yml`

- [ ] **Step 1 (manual, human-only): Generate the signing keypair**

This step creates a real private key and must be done by a maintainer with admin access to the `Perfect-Pixel-Games/pixel-build-manager` GitHub repo — do not script or automate key generation or secret creation.

1. Run: `npm run tauri signer generate -- -w src-tauri/updater.key`
2. When prompted, set a password for the private key file. Remember it — it's needed in step 3.
3. The command prints a public key to the console (starts with `dW50cnVzdGVk...`). Copy it.
4. In the GitHub repo's Settings → Secrets and variables → Actions, add two repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY` — the full contents of the generated `src-tauri/updater.key` file.
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password chosen in step 2.
5. Delete the local `src-tauri/updater.key` file once its contents are safely stored in the GitHub secret (it must never be committed — confirm `src-tauri/*.key` is covered by `src-tauri/.gitignore`; if not, add it before proceeding).
6. Keep the public key string from step 3 for the next step.

- [ ] **Step 2: Add the public key and updater artifact config to `tauri.conf.json`**

In `src-tauri/tauri.conf.json`, change the `bundle` section from:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
```

to:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  },
  "plugins": {
    "updater": {
      "pubkey": "PASTE_THE_PUBLIC_KEY_FROM_TASK_5_STEP_1_HERE"
    }
  }
```

Replace `PASTE_THE_PUBLIC_KEY_FROM_TASK_5_STEP_1_HERE` with the actual public key string copied in Step 1.

- [ ] **Step 3: Verify the frontend/config still parses and the Rust side still builds**

Run: `cd src-tauri && cargo build`
Expected: PASS (this only validates the file is well-formed JSON that Tauri's build script can read; it doesn't yet exercise signing since that needs the env vars from Step 4)

- [ ] **Step 4: Pass signing secrets and the channel into the `package` job**

In `.github/workflows/pixel-build-manager-build.yml`, find the `package` job's "Build Tauri app" step:

```yaml
      - name: Build Tauri app
        shell: cmd
        run: npm run tauri build
```

Replace it with:

```yaml
      - name: Build Tauri app
        shell: cmd
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          PIXEL_BUILD_MANAGER_CHANNEL: ${{ github.ref == 'refs/heads/main' && 'release' || 'prerelease' }}
        run: npm run tauri build
```

(The existing "Upload installer artifact" step right after it already uploads `src-tauri/target/release/bundle/**/*`, which recursively includes the new `.sig` file the signed build produces — no change needed there.)

- [ ] **Step 5: Generate `latest.json` in the `release` job**

In `.github/workflows/pixel-build-manager-build.yml`, find the `release` job's "Download package artifacts" step:

```yaml
      - name: Download package artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts/release-downloads
          merge-multiple: true
```

Immediately after it (and before the "Create release tag" step), add:

```yaml
      - name: Generate latest.json update manifest
        shell: pwsh
        run: |
          $sigFile = Get-ChildItem -Path artifacts/release-downloads -Recurse -Filter "*-setup.exe.sig" | Select-Object -First 1
          if (-not $sigFile) {
            throw "no NSIS .sig file found under artifacts/release-downloads -- was createUpdaterArtifacts enabled and signing secrets set?"
          }
          $signature = (Get-Content $sigFile.FullName -Raw).Trim()
          $installerName = $sigFile.Name -replace '\.sig$', ''
          $tag = "${{ needs.compute-version.outputs.tag }}"
          $manifest = @{
            version = $tag
            pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
            platforms = @{
              "windows-x86_64" = @{
                signature = $signature
                url = "https://github.com/Perfect-Pixel-Games/pixel-build-manager/releases/download/$tag/$installerName"
              }
            }
          }
          $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path artifacts/release-downloads/latest.json
```

This runs before "Publish GitHub Release", whose `files: artifacts/release-downloads/**/*` glob already picks up the new `latest.json` automatically — no change needed to that step.

- [ ] **Step 6: Commit the CI and config changes**

```bash
git add src-tauri/tauri.conf.json .github/workflows/pixel-build-manager-build.yml
git commit -m "Sign release builds and publish an update manifest from CI"
```

- [ ] **Step 7 (manual verification, after this branch is merged and both a `dev` and `main` build have run):**

1. Confirm the `dev` push's `release` job succeeds and the published prerelease has three assets: the `-setup.exe` installer, its `.sig` file, and `latest.json`.
2. Confirm the `main` push's `release` job succeeds the same way, and that `https://github.com/Perfect-Pixel-Games/pixel-build-manager/releases/latest/download/latest.json` resolves (i.e. GitHub's `latest` alias points at it).
3. Install a slightly older prerelease build locally, launch it, and confirm it silently updates and restarts into the newer prerelease within 15 minutes (or immediately on launch if already stale at startup).
4. Repeat with a release-channel install against a newer full release.
5. Launch the app via `npm run tauri dev` and confirm no update check happens (add a temporary log line if needed to observe this, then remove it).

---

## Self-review notes

- **Spec coverage:** debug-build exclusion (Task 4 Step 4's `cfg(not(debug_assertions))` gate), channel-aware release/prerelease selection (Tasks 2–3), 15-minute poll covering both startup and mid-session releases (Task 4's `tokio::time::interval`, which ticks immediately then every 15 min), silent/no-prompt restart (Task 4's `check_and_install`, no UI calls), busy-guard deferring installs during an active sync (Task 4's `active_operations` check), and the CI signing/manifest changes (Task 5) are all covered.
- No settings toggle to disable auto-update was in scope (per the spec's "Open questions" section) — none added.
