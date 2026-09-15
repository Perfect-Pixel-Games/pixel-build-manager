# Pixel Build Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri + React/TypeScript desktop app that lets a developer log into GitHub, browse projects they have access to, and sync (download/cache/extract) release builds into a per-project active directory.

**Architecture:** Rust backend (Tauri v2) owns all GitHub auth, API calls, downloading, caching, and extraction; React/TypeScript frontend is presentational, driving Rust via `invoke` and listening to progress events. Settings (workspace root, favorites, active release per project) persist to a JSON file in the OS app-data directory; the OAuth token persists in the OS keychain.

**Tech Stack:** Tauri v2, Rust (reqwest, keyring, serde, tokio, zip, thiserror), React 18 + TypeScript + Vite, Vitest + Testing Library for frontend tests, `cargo test` + `wiremock` for backend tests. GitHub REST API calls go through a thin `reqwest`-based client (not `octocrab`) so the base URL can be pointed at a `wiremock` server in tests, matching the device-flow client's pattern.

Spec: `docs/superpowers/specs/2026-09-16-pixel-build-manager-design.md`

---

## Phase 1: Foundation & Auth

### Task 1: Scaffold the Tauri + React + TypeScript project

**Files:**
- Create: entire project scaffold under `E:\GameDev\pixel-build-manager\` (root `package.json`, `src/`, `src-tauri/`, `vite.config.ts`, `tsconfig.json`)

- [ ] **Step 1: Run the Tauri scaffolding CLI**

Run:
```
npm create tauri-app@latest
```
When prompted, answer:
- App name: `pixel-build-manager`
- Window title: `Pixel Build Manager`
- Which language: `TypeScript / JavaScript`
- Package manager: `npm`
- UI template: `React`
- UI flavor: `TypeScript`

This scaffolds into the current directory (run it from `E:\GameDev\pixel-build-manager`; if it creates a nested folder instead, move the contents up one level so `package.json` and `src-tauri/` sit directly under the repo root).

- [ ] **Step 2: Install dependencies**

Run: `npm install`
Expected: completes with no errors, creates `node_modules/` and `package-lock.json`.

- [ ] **Step 3: Verify the scaffold builds and runs**

Run: `npm run tauri dev`
Expected: a window opens showing the default Tauri+React starter page. Close the window / Ctrl+C to stop.

- [ ] **Step 4: Add a `.gitignore` entry check**

Confirm `node_modules/`, `src-tauri/target/`, and `dist/` are present in the generated `.gitignore` (the Tauri scaffold includes this by default — just verify by reading the file).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Scaffold Tauri + React + TypeScript project"
```

---

### Task 2: Add Rust backend dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Edit `src-tauri/Cargo.toml`, adding to `[dependencies]`:

```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["full"] }
keyring = "3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
zip = "2"
futures-util = "0.3"
dirs = "5"
uuid = { version = "1", features = ["v4"] }
```

And add a `[dev-dependencies]` section:

```toml
[dev-dependencies]
wiremock = "0.6"
tempfile = "3"
tokio-test = "0.4"
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: downloads and compiles new crates successfully (no code uses them yet, so no errors expected beyond unused-dependency warnings, which are fine).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "Add backend dependencies for GitHub auth, API access, and packaging"
```

---

### Task 3: Settings module (JSON persistence)

**Files:**
- Create: `src-tauri/src/settings/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register the `settings` module)

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/settings/mod.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub favorite: bool,
    pub active_release_tag: Option<String>,
    pub active_asset_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub projects: HashMap<String, ProjectSettings>,
}

impl Settings {
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self).expect("Settings must serialize");
        std::fs::write(path, contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let settings = Settings::load_from(&path);

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");

        let mut settings = Settings::default();
        settings.workspace_root = Some(PathBuf::from("D:\\Builds"));
        settings.projects.insert(
            "pixel-perfect/last-beacon".to_string(),
            ProjectSettings {
                favorite: true,
                active_release_tag: Some("0.2.14".to_string()),
                active_asset_name: Some("last-beacon-windows-x64-shipping.zip".to_string()),
            },
        );

        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);

        assert_eq!(loaded, settings);
    }
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/lib.rs`, add near the top (after existing `mod`/`use` lines):

```rust
mod settings;
```

- [ ] **Step 3: Run the tests**

Run: `cd src-tauri && cargo test settings::`
Expected: PASS — both `load_from_missing_file_returns_default` and `save_then_load_round_trips` pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/settings/mod.rs src-tauri/src/lib.rs
git commit -m "Add JSON-backed settings module with favorites and active-release tracking"
```

---

### Task 4: Token store (OS keychain wrapper)

**Files:**
- Create: `src-tauri/src/auth/mod.rs`
- Create: `src-tauri/src/auth/token_store.rs`
- Modify: `src-tauri/src/lib.rs` (register the `auth` module)

- [ ] **Step 1: Write the token store with an in-memory fake for testing**

Create `src-tauri/src/auth/token_store.rs`:

```rust
use keyring::Entry;

const SERVICE_NAME: &str = "pixel-build-manager";
const USERNAME: &str = "github-token";

pub trait TokenStore: Send + Sync {
    fn save(&self, token: &str) -> Result<(), String>;
    fn load(&self) -> Result<Option<String>, String>;
    fn clear(&self) -> Result<(), String>;
}

pub struct KeyringTokenStore;

impl TokenStore for KeyringTokenStore {
    fn save(&self, token: &str) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME).map_err(|e| e.to_string())?;
        entry.set_password(token).map_err(|e| e.to_string())
    }

    fn load(&self) -> Result<Option<String>, String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn clear(&self) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
pub struct InMemoryTokenStore {
    token: std::sync::Mutex<Option<String>>,
}

#[cfg(test)]
impl InMemoryTokenStore {
    pub fn new() -> Self {
        Self {
            token: std::sync::Mutex::new(None),
        }
    }
}

#[cfg(test)]
impl TokenStore for InMemoryTokenStore {
    fn save(&self, token: &str) -> Result<(), String> {
        *self.token.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.token.lock().unwrap().clone())
    }

    fn clear(&self) -> Result<(), String> {
        *self.token.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_a_token() {
        let store = InMemoryTokenStore::new();
        assert_eq!(store.load().unwrap(), None);

        store.save("abc123").unwrap();
        assert_eq!(store.load().unwrap(), Some("abc123".to_string()));

        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }
}
```

Note: `KeyringTokenStore` talks to the real OS credential vault, so it is intentionally not unit tested here (it's a thin, five-line wrapper around `keyring::Entry`). The `TokenStore` trait is what makes the auth flow (Task 5+) testable via `InMemoryTokenStore`.

- [ ] **Step 2: Create the auth module file**

Create `src-tauri/src/auth/mod.rs`:

```rust
pub mod token_store;
```

- [ ] **Step 3: Register the module**

In `src-tauri/src/lib.rs`, add:

```rust
mod auth;
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test auth::`
Expected: PASS — `in_memory_store_round_trips_a_token` passes.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/auth/mod.rs src-tauri/src/auth/token_store.rs src-tauri/src/lib.rs
git commit -m "Add token store trait with keyring-backed and in-memory implementations"
```

---

### Task 5: GitHub OAuth Device Flow client

**Files:**
- Create: `src-tauri/src/auth/device_flow.rs`
- Modify: `src-tauri/src/auth/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/auth/device_flow.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DeviceFlowClient {
    http: reqwest::Client,
    base_url: String,
    client_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("access denied")]
    AccessDenied,
    #[error("device code expired")]
    Expired,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PollOutcome {
    AccessToken(String),
    Pending,
    SlowDown,
}

impl DeviceFlowClient {
    pub fn new(client_id: String) -> Self {
        Self::with_base_url(client_id, "https://github.com".to_string())
    }

    pub fn with_base_url(client_id: String, base_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            client_id,
        }
    }

    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse, AuthError> {
        let url = format!("{}/login/device/code", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .form(&[("client_id", self.client_id.as_str()), ("scope", "repo read:org")])
            .send()
            .await?;

        response
            .json::<DeviceCodeResponse>()
            .await
            .map_err(|e| AuthError::UnexpectedResponse(e.to_string()))
    }

    pub async fn poll_for_token(&self, device_code: &str) -> Result<PollOutcome, AuthError> {
        #[derive(Deserialize)]
        struct RawResponse {
            access_token: Option<String>,
            error: Option<String>,
        }

        let url = format!("{}/login/oauth/access_token", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let raw: RawResponse = response
            .json()
            .await
            .map_err(|e| AuthError::UnexpectedResponse(e.to_string()))?;

        if let Some(token) = raw.access_token {
            return Ok(PollOutcome::AccessToken(token));
        }

        match raw.error.as_deref() {
            Some("authorization_pending") => Ok(PollOutcome::Pending),
            Some("slow_down") => Ok(PollOutcome::SlowDown),
            Some("access_denied") => Err(AuthError::AccessDenied),
            Some("expired_token") => Err(AuthError::Expired),
            Some(other) => Err(AuthError::UnexpectedResponse(other.to_string())),
            None => Err(AuthError::UnexpectedResponse(
                "no access_token or error in response".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn request_device_code_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "devcode123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let response = client.request_device_code().await.unwrap();

        assert_eq!(response.device_code, "devcode123");
        assert_eq!(response.user_code, "ABCD-1234");
    }

    #[tokio::test]
    async fn poll_for_token_returns_pending_then_access_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "authorization_pending"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_token"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let first = client.poll_for_token("devcode123").await.unwrap();
        assert_eq!(first, PollOutcome::Pending);

        let second = client.poll_for_token("devcode123").await.unwrap();
        assert_eq!(second, PollOutcome::AccessToken("ghu_token".to_string()));
    }

    #[tokio::test]
    async fn poll_for_token_maps_access_denied_to_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = client.poll_for_token("devcode123").await;

        assert!(matches!(result, Err(AuthError::AccessDenied)));
    }
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/auth/mod.rs`, add:

```rust
pub mod device_flow;
```

- [ ] **Step 3: Run the tests**

Run: `cd src-tauri && cargo test auth::device_flow::`
Expected: PASS — all three tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/auth/device_flow.rs src-tauri/src/auth/mod.rs
git commit -m "Add GitHub OAuth device flow client with mocked HTTP tests"
```

---

### Task 6: Login orchestrator (state machine tying device flow to token storage)

**Files:**
- Create: `src-tauri/src/auth/login.rs`
- Modify: `src-tauri/src/auth/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/auth/login.rs`:

```rust
use crate::auth::device_flow::{AuthError, DeviceFlowClient, PollOutcome};
use crate::auth::token_store::TokenStore;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginStatus {
    AwaitingUser {
        user_code: String,
        verification_uri: String,
    },
    Success,
    Denied,
    Expired,
    Error(String),
}

pub async fn perform_device_login<F: Fn(LoginStatus)>(
    client: &DeviceFlowClient,
    token_store: &dyn TokenStore,
    on_status: F,
) -> Result<(), AuthError> {
    let device_code = client.request_device_code().await?;

    on_status(LoginStatus::AwaitingUser {
        user_code: device_code.user_code.clone(),
        verification_uri: device_code.verification_uri.clone(),
    });

    let mut interval = Duration::from_secs(device_code.interval);

    loop {
        tokio::time::sleep(interval).await;

        match client.poll_for_token(&device_code.device_code).await {
            Ok(PollOutcome::AccessToken(token)) => {
                token_store
                    .save(&token)
                    .map_err(AuthError::UnexpectedResponse)?;
                on_status(LoginStatus::Success);
                return Ok(());
            }
            Ok(PollOutcome::Pending) => continue,
            Ok(PollOutcome::SlowDown) => {
                interval += Duration::from_secs(5);
                continue;
            }
            Err(AuthError::AccessDenied) => {
                on_status(LoginStatus::Denied);
                return Err(AuthError::AccessDenied);
            }
            Err(AuthError::Expired) => {
                on_status(LoginStatus::Expired);
                return Err(AuthError::Expired);
            }
            Err(other) => {
                on_status(LoginStatus::Error(other.to_string()));
                return Err(other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token_store::InMemoryTokenStore;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn successful_login_saves_token_and_reports_statuses() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "devcode123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_token"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let token_store = InMemoryTokenStore::new();
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();

        perform_device_login(&client, &token_store, move |status| {
            statuses_clone.lock().unwrap().push(status);
        })
        .await
        .unwrap();

        assert_eq!(token_store.load().unwrap(), Some("ghu_token".to_string()));
        let recorded = statuses.lock().unwrap();
        assert_eq!(
            recorded[0],
            LoginStatus::AwaitingUser {
                user_code: "ABCD-1234".to_string(),
                verification_uri: "https://github.com/login/device".to_string(),
            }
        );
        assert_eq!(recorded[1], LoginStatus::Success);
    }

    #[tokio::test]
    async fn denied_login_does_not_save_a_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "devcode123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let token_store = InMemoryTokenStore::new();

        let result = perform_device_login(&client, &token_store, |_| {}).await;

        assert!(result.is_err());
        assert_eq!(token_store.load().unwrap(), None);
    }
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/auth/mod.rs`, add:

```rust
pub mod login;
```

- [ ] **Step 3: Run the tests**

Run: `cd src-tauri && cargo test auth::login::`
Expected: PASS — both tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/auth/login.rs src-tauri/src/auth/mod.rs
git commit -m "Add device-login orchestrator state machine"
```

---

### Task 7: Wire auth into Tauri commands and app state

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Replace the generated `lib.rs` with the auth-wired version**

Open `src-tauri/src/lib.rs`. It currently contains the scaffold's `greet` command. Replace the entire file with:

```rust
mod auth;
mod settings;

use auth::device_flow::DeviceFlowClient;
use auth::login::{perform_device_login, LoginStatus};
use auth::token_store::{KeyringTokenStore, TokenStore};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, Manager};

const GITHUB_CLIENT_ID: &str = "REPLACE_WITH_YOUR_GITHUB_OAUTH_APP_CLIENT_ID";

pub struct AppState {
    pub token_store: Arc<dyn TokenStore>,
    pub settings_path: PathBuf,
}

fn settings_path_for(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir must be resolvable")
        .join("settings.json")
}

#[tauri::command]
async fn login_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let client = DeviceFlowClient::new(GITHUB_CLIENT_ID.to_string());
    let token_store = state.token_store.clone();
    let app_for_events = app.clone();

    tauri::async_runtime::spawn(async move {
        let _ = perform_device_login(
            &client,
            token_store.as_ref(),
            move |status: LoginStatus| {
                let _ = app_for_events.emit("login-status", status);
            },
        )
        .await;
    });

    Ok(())
}

#[tauri::command]
fn logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.token_store.clear()
}

#[tauri::command]
fn is_logged_in(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(state.token_store.load()?.is_some())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let settings_path = settings_path_for(&app.handle());
            app.manage(AppState {
                token_store: Arc::new(KeyringTokenStore),
                settings_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![login_start, logout, is_logged_in])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

If the scaffold generated a different default plugin list in `.plugin(...)` calls, keep those lines as-is and just add the `mod`, `use`, `AppState`, commands, `.setup(...)`, and updated `.invoke_handler(...)` shown above.

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: builds successfully with no errors.

- [ ] **Step 3: Run the existing test suite to confirm nothing broke**

Run: `cd src-tauri && cargo test`
Expected: all previously-passing tests (settings, auth::token_store, auth::device_flow, auth::login) still PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "Expose login_start, logout, and is_logged_in as Tauri commands"
```

---

### Task 8: Frontend login UI and state machine

**Files:**
- Create: `src/api/auth.ts`
- Create: `src/components/Login.tsx`
- Create: `src/components/Login.test.tsx`
- Modify: `package.json` (test dependencies + script)

- [ ] **Step 1: Add frontend test tooling**

Run:
```
npm install --save-dev vitest @testing-library/react @testing-library/jest-dom jsdom @types/react @types/react-dom
```

Add to `package.json` under `"scripts"`:

```json
"test": "vitest run"
```

Create `vitest.config.ts` in the project root:

```typescript
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: "./src/test-setup.ts",
  },
});
```

Create `src/test-setup.ts`:

```typescript
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 2: Write the API wrapper**

Create `src/api/auth.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type LoginStatus =
  | { status: "awaiting_user"; user_code: string; verification_uri: string }
  | { status: "success" }
  | { status: "denied" }
  | { status: "expired" }
  | { status: "error"; error: string };

export function loginStart(): Promise<void> {
  return invoke("login_start");
}

export function logout(): Promise<void> {
  return invoke("logout");
}

export function isLoggedIn(): Promise<boolean> {
  return invoke("is_logged_in");
}

export function onLoginStatus(callback: (status: LoginStatus) => void) {
  return listen<LoginStatus>("login-status", (event) => callback(event.payload));
}
```

- [ ] **Step 3: Write the failing test**

Create `src/components/Login.test.tsx`:

```tsx
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Login } from "./Login";
import * as authApi from "../api/auth";

vi.mock("../api/auth");

describe("Login", () => {
  beforeEach(() => {
    vi.mocked(authApi.loginStart).mockResolvedValue(undefined);
    vi.mocked(authApi.onLoginStatus).mockImplementation(() =>
      Promise.resolve(() => {}),
    );
  });

  it("shows the device code once the backend reports awaiting_user", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /log in with github/i }));

    await waitFor(() => expect(authApi.loginStart).toHaveBeenCalled());

    capturedCallback({
      status: "awaiting_user",
      user_code: "ABCD-1234",
      verification_uri: "https://github.com/login/device",
    });

    expect(await screen.findByText("ABCD-1234")).toBeInTheDocument();
  });

  it("calls onLoggedIn when the backend reports success", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    const onLoggedIn = vi.fn();

    render(<Login onLoggedIn={onLoggedIn} />);
    fireEvent.click(screen.getByRole("button", { name: /log in with github/i }));
    await waitFor(() => expect(authApi.loginStart).toHaveBeenCalled());

    capturedCallback({ status: "success" });

    await waitFor(() => expect(onLoggedIn).toHaveBeenCalled());
  });
});
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `npm run test`
Expected: FAIL with "Cannot find module './Login'" (component doesn't exist yet).

- [ ] **Step 5: Implement the Login component**

Create `src/components/Login.tsx`:

```tsx
import { useState } from "react";
import { loginStart, onLoginStatus, LoginStatus } from "../api/auth";

type Props = {
  onLoggedIn: () => void;
};

export function Login({ onLoggedIn }: Props) {
  const [status, setStatus] = useState<LoginStatus | null>(null);

  const handleLogin = async () => {
    setStatus(null);
    await onLoginStatus((newStatus) => {
      setStatus(newStatus);
      if (newStatus.status === "success") {
        onLoggedIn();
      }
    });
    await loginStart();
  };

  return (
    <div>
      <button onClick={handleLogin}>Log in with GitHub</button>
      {status?.status === "awaiting_user" && (
        <p>
          Go to {status.verification_uri} and enter code: <strong>{status.user_code}</strong>
        </p>
      )}
      {status?.status === "denied" && <p>Login was denied.</p>}
      {status?.status === "expired" && <p>The login code expired. Try again.</p>}
      {status?.status === "error" && <p>Login failed: {status.error}</p>}
    </div>
  );
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npm run test`
Expected: PASS — both tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/api/auth.ts src/components/Login.tsx src/components/Login.test.tsx vitest.config.ts src/test-setup.ts package.json package-lock.json
git commit -m "Add frontend login UI wired to the device-flow backend"
```

---

### Task 9: Wire login into the app shell and manually verify end-to-end

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Update `App.tsx` to gate on login state**

Replace the contents of `src/App.tsx` with:

```tsx
import { useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { Login } from "./components/Login";
import "./App.css";

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);

  useEffect(() => {
    isLoggedIn().then(setLoggedIn);
  }, []);

  if (loggedIn === null) {
    return <p>Loading...</p>;
  }

  if (!loggedIn) {
    return <Login onLoggedIn={() => setLoggedIn(true)} />;
  }

  return (
    <div>
      <p>Logged in.</p>
      <button
        onClick={async () => {
          await logout();
          setLoggedIn(false);
        }}
      >
        Log out
      </button>
    </div>
  );
}

export default App;
```

- [ ] **Step 2: Register a real GitHub OAuth App**

In a browser, go to GitHub Settings → Developer settings → OAuth Apps → New OAuth App. Application name: `Pixel Build Manager (Dev)`. Homepage URL: `https://github.com/PixelPerfect` (or any placeholder — not used by device flow). Authorization callback URL: `http://localhost` (unused by device flow, but required by the form). Enable **Device Flow** in the app's settings after creation. Copy the generated **Client ID**.

- [ ] **Step 3: Insert the real Client ID**

In `src-tauri/src/lib.rs`, replace:

```rust
const GITHUB_CLIENT_ID: &str = "REPLACE_WITH_YOUR_GITHUB_OAUTH_APP_CLIENT_ID";
```

with the real Client ID from Step 2.

- [ ] **Step 4: Manually verify the full login flow**

Run: `npm run tauri dev`
Expected:
1. App window shows a "Log in with GitHub" button.
2. Clicking it displays a verification URL and user code.
3. Visiting that URL in a browser and entering the code, then authorizing the app, causes the desktop window to automatically switch to the "Logged in." screen within a few seconds.
4. Clicking "Log out" returns to the login screen.
5. Quit and relaunch the app (`npm run tauri dev` again) — confirm it goes straight to "Logged in." (proving the keychain-stored token persists across restarts), then log out to leave it in a clean state for Phase 2.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "Wire login flow into app shell"
```

**Phase 1 complete.** The app can now authenticate a real GitHub user via device flow and persist the token across restarts.

---

## Phase 2: Project Browsing

### Task 10: GitHub API client — list accessible repositories

**Files:**
- Create: `src-tauri/src/github/mod.rs`
- Create: `src-tauri/src/github/client.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/github/client.rs`:

```rust
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
```

- [ ] **Step 2: Create the module file**

Create `src-tauri/src/github/mod.rs`:

```rust
pub mod client;
```

- [ ] **Step 3: Register the module**

In `src-tauri/src/lib.rs`, add near the other `mod` lines:

```rust
mod github;
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test github::client::`
Expected: PASS — all three tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/github/mod.rs src-tauri/src/github/client.rs src-tauri/src/lib.rs
git commit -m "Add GitHub client for listing all accessible repositories"
```

---

### Task 11: GitHub API client — list releases and assets

**Files:**
- Modify: `src-tauri/src/github/client.rs`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/github/client.rs`, add above the existing `GithubClient` struct definition (alongside `RepoSummary`/`RepoOwner`):

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ReleaseSummary {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}
```

Then add this method inside `impl GithubClient { ... }`, after `list_accessible_repos`:

```rust
    pub async fn list_releases(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<ReleaseSummary>, GithubError> {
        let path = format!("/repos/{}/{}/releases?per_page=100", owner, repo);
        let response = self.request(reqwest::Method::GET, &path).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api { status, body });
        }

        Ok(response.json().await?)
    }
```

And add this test inside `mod tests { ... }`:

```rust
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
        assert_eq!(releases[0].assets[0].name, "last-beacon-windows-x64-shipping.tar.gz");
    }
```

- [ ] **Step 2: Run the tests**

Run: `cd src-tauri && cargo test github::client::`
Expected: PASS — all four tests pass (three from Task 10 plus this one).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/github/client.rs
git commit -m "Add release and asset listing to the GitHub client"
```

---

### Task 12: Favorites toggle in settings

**Files:**
- Modify: `src-tauri/src/settings/mod.rs`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/settings/mod.rs`, add this method inside `impl Settings { ... }`, after `save_to`:

```rust
    pub fn set_favorite(&mut self, project_key: &str, favorite: bool) {
        self.projects.entry(project_key.to_string()).or_default().favorite = favorite;
    }
```

Add this test inside `mod tests { ... }`:

```rust
    #[test]
    fn set_favorite_creates_entry_if_missing() {
        let mut settings = Settings::default();

        settings.set_favorite("pixel-perfect/last-beacon", true);

        assert!(settings.projects["pixel-perfect/last-beacon"].favorite);

        settings.set_favorite("pixel-perfect/last-beacon", false);

        assert!(!settings.projects["pixel-perfect/last-beacon"].favorite);
    }
```

- [ ] **Step 2: Run the tests**

Run: `cd src-tauri && cargo test settings::`
Expected: PASS — all settings tests pass, including the new one.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/settings/mod.rs
git commit -m "Add favorite-toggling to project settings"
```

---

### Task 13: Tauri commands for browsing projects and releases

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the commands and a shared client-builder helper**

In `src-tauri/src/lib.rs`, add these imports near the top (alongside the existing `use` lines):

```rust
use github::client::{GithubClient, ReleaseSummary};
use serde::Serialize;
use settings::Settings;
```

Add this struct and helper function above `#[cfg_attr(mobile, tauri::mobile_entry_point)]`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ProjectListItem {
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub favorite: bool,
}

fn build_github_client(state: &AppState) -> Result<GithubClient, String> {
    let token = state
        .token_store
        .load()?
        .ok_or_else(|| "not logged in".to_string())?;
    Ok(GithubClient::new(token))
}
```

Add these commands above `pub fn run()`:

```rust
#[tauri::command]
async fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<ProjectListItem>, String> {
    let client = build_github_client(&state)?;
    let repos = client
        .list_accessible_repos()
        .await
        .map_err(|e| e.to_string())?;
    let settings = Settings::load_from(&state.settings_path);

    Ok(repos
        .into_iter()
        .map(|repo| {
            let favorite = settings
                .projects
                .get(&repo.full_name)
                .map(|p| p.favorite)
                .unwrap_or(false);
            ProjectListItem {
                full_name: repo.full_name,
                owner: repo.owner.login,
                name: repo.name,
                favorite,
            }
        })
        .collect())
}

#[tauri::command]
async fn list_releases_for_project(
    full_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ReleaseSummary>, String> {
    let (owner, repo) = full_name
        .split_once('/')
        .ok_or_else(|| format!("invalid project full_name: {}", full_name))?;
    let client = build_github_client(&state)?;
    client
        .list_releases(owner, repo)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_favorite(
    full_name: String,
    favorite: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_favorite(&full_name, favorite);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register the new commands**

Update the `.invoke_handler(...)` call inside `run()` to:

```rust
        .invoke_handler(tauri::generate_handler![
            login_start,
            logout,
            is_logged_in,
            list_projects,
            list_releases_for_project,
            toggle_favorite
        ])
```

- [ ] **Step 3: Verify it compiles and existing tests still pass**

Run: `cd src-tauri && cargo build && cargo test`
Expected: builds successfully; all existing tests still PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "Add list_projects, list_releases_for_project, and toggle_favorite commands"
```

---

### Task 14: Frontend project list (favorites + full list)

**Files:**
- Create: `src/api/projects.ts`
- Create: `src/components/ProjectList.tsx`
- Create: `src/components/ProjectList.test.tsx`

- [ ] **Step 1: Write the API wrapper**

Create `src/api/projects.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

export type Project = {
  full_name: string;
  owner: string;
  name: string;
  favorite: boolean;
};

export type ReleaseAsset = {
  id: number;
  name: string;
  size: number;
  browser_download_url: string;
};

export type Release = {
  id: number;
  tag_name: string;
  name: string | null;
  prerelease: boolean;
  published_at: string | null;
  assets: ReleaseAsset[];
};

export function listProjects(): Promise<Project[]> {
  return invoke("list_projects");
}

export function listReleasesForProject(fullName: string): Promise<Release[]> {
  return invoke("list_releases_for_project", { fullName });
}

export function toggleFavorite(fullName: string, favorite: boolean): Promise<void> {
  return invoke("toggle_favorite", { fullName, favorite });
}
```

- [ ] **Step 2: Write the failing test**

Create `src/components/ProjectList.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ProjectList } from "./ProjectList";
import type { Project } from "../api/projects";
import * as projectsApi from "../api/projects";

vi.mock("../api/projects");

const projects: Project[] = [
  { full_name: "pixel-perfect/last-beacon", owner: "pixel-perfect", name: "last-beacon", favorite: true },
  { full_name: "pixel-perfect/other-game", owner: "pixel-perfect", name: "other-game", favorite: false },
];

describe("ProjectList", () => {
  it("renders favorites in their own section, above the full list", () => {
    render(<ProjectList projects={projects} onSelect={() => {}} onToggleFavorite={() => {}} />);

    const favoritesSection = screen.getByRole("region", { name: /favorites/i });
    expect(favoritesSection).toHaveTextContent("last-beacon");

    const allSection = screen.getByRole("region", { name: /all projects/i });
    expect(allSection).toHaveTextContent("last-beacon");
    expect(allSection).toHaveTextContent("other-game");
  });

  it("calls onToggleFavorite when the star is clicked", () => {
    const onToggleFavorite = vi.fn();
    render(<ProjectList projects={projects} onSelect={() => {}} onToggleFavorite={onToggleFavorite} />);

    fireEvent.click(screen.getAllByRole("button", { name: /toggle favorite/i })[1]);

    expect(onToggleFavorite).toHaveBeenCalledWith("pixel-perfect/other-game", true);
  });

  it("calls onSelect when a project is clicked", () => {
    const onSelect = vi.fn();
    render(<ProjectList projects={projects} onSelect={onSelect} onToggleFavorite={() => {}} />);

    fireEvent.click(screen.getByText("last-beacon"));

    expect(onSelect).toHaveBeenCalledWith("pixel-perfect/last-beacon");
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `npm run test`
Expected: FAIL with "Cannot find module './ProjectList'".

- [ ] **Step 4: Implement the component**

Create `src/components/ProjectList.tsx`:

```tsx
import type { Project } from "../api/projects";

type Props = {
  projects: Project[];
  onSelect: (fullName: string) => void;
  onToggleFavorite: (fullName: string, favorite: boolean) => void;
};

function ProjectRow({
  project,
  onSelect,
  onToggleFavorite,
}: {
  project: Project;
  onSelect: (fullName: string) => void;
  onToggleFavorite: (fullName: string, favorite: boolean) => void;
}) {
  return (
    <li>
      <button aria-label="toggle favorite" onClick={() => onToggleFavorite(project.full_name, !project.favorite)}>
        {project.favorite ? "★" : "☆"}
      </button>
      <button onClick={() => onSelect(project.full_name)}>{project.name}</button>
    </li>
  );
}

export function ProjectList({ projects, onSelect, onToggleFavorite }: Props) {
  const favorites = projects.filter((p) => p.favorite);

  return (
    <div>
      <section aria-label="Favorites">
        <h2>Favorites</h2>
        <ul>
          {favorites.map((project) => (
            <ProjectRow
              key={project.full_name}
              project={project}
              onSelect={onSelect}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
        </ul>
      </section>
      <section aria-label="All projects">
        <h2>All projects</h2>
        <ul>
          {projects.map((project) => (
            <ProjectRow
              key={project.full_name}
              project={project}
              onSelect={onSelect}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
        </ul>
      </section>
    </div>
  );
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm run test`
Expected: PASS — all three tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/api/projects.ts src/components/ProjectList.tsx src/components/ProjectList.test.tsx
git commit -m "Add project list UI with favorites section"
```

---

### Task 15: Frontend release list

**Files:**
- Create: `src/components/ReleaseList.tsx`
- Create: `src/components/ReleaseList.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `src/components/ReleaseList.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ReleaseList } from "./ReleaseList";
import type { Release } from "../api/projects";

const releases: Release[] = [
  {
    id: 1,
    tag_name: "0.2.14",
    name: "LastBeacon 0.2.14",
    prerelease: false,
    published_at: "2026-09-01T00:00:00Z",
    assets: [
      { id: 10, name: "last-beacon-windows-x64-shipping.tar.gz", size: 12345, browser_download_url: "https://example.com/a" },
      { id: 11, name: "last-beacon-windows-x64-test.tar.gz", size: 12345, browser_download_url: "https://example.com/b" },
    ],
  },
];

describe("ReleaseList", () => {
  it("lists every asset of every release as its own syncable row", () => {
    render(<ReleaseList releases={releases} activeAssetName={null} onSync={() => {}} />);

    expect(screen.getByText("LastBeacon 0.2.14")).toBeInTheDocument();
    expect(screen.getByText("last-beacon-windows-x64-shipping.tar.gz")).toBeInTheDocument();
    expect(screen.getByText("last-beacon-windows-x64-test.tar.gz")).toBeInTheDocument();
  });

  it("marks the active asset", () => {
    render(
      <ReleaseList
        releases={releases}
        activeAssetName="last-beacon-windows-x64-shipping.tar.gz"
        onSync={() => {}}
      />,
    );

    const activeRow = screen.getByText("last-beacon-windows-x64-shipping.tar.gz").closest("li");
    expect(activeRow).toHaveTextContent("Active");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test`
Expected: FAIL with "Cannot find module './ReleaseList'".

- [ ] **Step 3: Implement the component**

Create `src/components/ReleaseList.tsx`:

```tsx
import type { Release } from "../api/projects";

type Props = {
  releases: Release[];
  activeAssetName: string | null;
  onSync: (release: Release, assetId: number) => void;
};

export function ReleaseList({ releases, activeAssetName, onSync }: Props) {
  return (
    <ul>
      {releases.map((release) => (
        <li key={release.id}>
          <h3>
            {release.name ?? release.tag_name} {release.prerelease ? "(prerelease)" : ""}
          </h3>
          <ul>
            {release.assets.map((asset) => (
              <li key={asset.id}>
                {asset.name}
                {asset.name === activeAssetName && <strong> (Active)</strong>}
                <button onClick={() => onSync(release, asset.id)}>Sync</button>
              </li>
            ))}
          </ul>
        </li>
      ))}
    </ul>
  );
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test`
Expected: PASS — both tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/components/ReleaseList.tsx src/components/ReleaseList.test.tsx
git commit -m "Add release list UI showing every asset as a syncable row"
```

---

### Task 16: Wire browsing into the app shell and manually verify

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Update `App.tsx` to show projects and releases after login**

Replace the contents of `src/App.tsx` with:

```tsx
import { useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release } from "./api/projects";
import { Login } from "./components/Login";
import { ProjectList } from "./components/ProjectList";
import { ReleaseList } from "./components/ReleaseList";
import "./App.css";

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [releases, setReleases] = useState<Release[]>([]);

  useEffect(() => {
    isLoggedIn().then(setLoggedIn);
  }, []);

  useEffect(() => {
    if (loggedIn) {
      listProjects().then(setProjects);
    }
  }, [loggedIn]);

  useEffect(() => {
    if (selectedProject) {
      listReleasesForProject(selectedProject).then(setReleases);
    }
  }, [selectedProject]);

  const handleToggleFavorite = async (fullName: string, favorite: boolean) => {
    await toggleFavorite(fullName, favorite);
    setProjects((prev) =>
      prev.map((p) => (p.full_name === fullName ? { ...p, favorite } : p)),
    );
  };

  if (loggedIn === null) {
    return <p>Loading...</p>;
  }

  if (!loggedIn) {
    return <Login onLoggedIn={() => setLoggedIn(true)} />;
  }

  return (
    <div>
      <button
        onClick={async () => {
          await logout();
          setLoggedIn(false);
        }}
      >
        Log out
      </button>
      <ProjectList
        projects={projects}
        onSelect={setSelectedProject}
        onToggleFavorite={handleToggleFavorite}
      />
      {selectedProject && (
        <ReleaseList releases={releases} activeAssetName={null} onSync={() => {}} />
      )}
    </div>
  );
}

export default App;
```

(The `onSync` handler is wired up for real in Phase 3 — for now it's a no-op placeholder so the browsing UI can be verified end-to-end on its own.)

- [ ] **Step 2: Manually verify browsing end-to-end**

Run: `npm run tauri dev`, log in with the account used in Task 9. Expected:
1. After login, the project list shows every repo the account has access to (e.g. `pixel-perfect/last-beacon`), split into "Favorites" (empty initially) and "All projects".
2. Clicking the star next to a project moves it into "Favorites" immediately.
3. Clicking a project name loads and displays its releases, each with its individual assets listed underneath.
4. Quit and relaunch the app — confirm the favorite you set is still marked (proving it persisted to the settings JSON file).

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "Wire project and release browsing into app shell"
```

**Phase 2 complete.** The app now lists every accessible project, supports favoriting, and shows releases with their individual assets.

---

## Phase 3: Release Sync

### Task 17: Workspace root setting and folder picker

**Files:**
- Modify: `src-tauri/src/settings/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Create: `src/api/settings.ts`
- Create: `src/components/WorkspaceSetup.tsx`
- Create: `src/components/WorkspaceSetup.test.tsx`

- [ ] **Step 1: Add the folder-picker plugin**

Run: `cd src-tauri && cargo add tauri-plugin-dialog`
Run: `npm install @tauri-apps/plugin-dialog`

In `src-tauri/src/lib.rs`, add the plugin to the builder chain in `run()`:

```rust
        .plugin(tauri_plugin_dialog::init())
```

Keep this on its own line among the other `.plugin(...)` calls, before `.setup(...)`.

- [ ] **Step 2: Write the failing test for the settings getter/setter**

In `src-tauri/src/settings/mod.rs`, add these methods inside `impl Settings { ... }`:

```rust
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }
```

Add this test inside `mod tests { ... }`:

```rust
    #[test]
    fn set_workspace_root_updates_the_field() {
        let mut settings = Settings::default();

        settings.set_workspace_root(PathBuf::from("D:\\Builds"));

        assert_eq!(settings.workspace_root, Some(PathBuf::from("D:\\Builds")));
    }
```

- [ ] **Step 3: Run the tests**

Run: `cd src-tauri && cargo test settings::`
Expected: PASS.

- [ ] **Step 4: Add Tauri commands for reading/setting the workspace root**

In `src-tauri/src/lib.rs`, add these commands near the other commands:

```rust
#[tauri::command]
fn get_workspace_root(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .workspace_root
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn set_workspace_root(root: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_workspace_root(PathBuf::from(root));
    settings.save_to(&state.settings_path).map_err(|e| e.to_string())
}
```

Add `get_workspace_root` and `set_workspace_root` to the `tauri::generate_handler![...]` list in `run()`.

- [ ] **Step 5: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: builds successfully.

- [ ] **Step 6: Write the frontend API wrapper and failing component test**

Create `src/api/settings.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export function getWorkspaceRoot(): Promise<string | null> {
  return invoke("get_workspace_root");
}

export function setWorkspaceRoot(root: string): Promise<void> {
  return invoke("set_workspace_root", { root });
}

export function pickFolder(): Promise<string | null> {
  return open({ directory: true, multiple: false }) as Promise<string | null>;
}
```

Create `src/components/WorkspaceSetup.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { WorkspaceSetup } from "./WorkspaceSetup";
import * as settingsApi from "../api/settings";

vi.mock("../api/settings");

describe("WorkspaceSetup", () => {
  it("saves the picked folder and reports it back", async () => {
    vi.mocked(settingsApi.pickFolder).mockResolvedValue("D:\\Builds");
    vi.mocked(settingsApi.setWorkspaceRoot).mockResolvedValue(undefined);
    const onSet = vi.fn();

    render(<WorkspaceSetup onSet={onSet} />);
    fireEvent.click(screen.getByRole("button", { name: /choose workspace folder/i }));

    await waitFor(() => expect(settingsApi.setWorkspaceRoot).toHaveBeenCalledWith("D:\\Builds"));
    expect(onSet).toHaveBeenCalledWith("D:\\Builds");
  });

  it("does nothing if the user cancels the folder picker", async () => {
    vi.mocked(settingsApi.pickFolder).mockResolvedValue(null);
    const onSet = vi.fn();

    render(<WorkspaceSetup onSet={onSet} />);
    fireEvent.click(screen.getByRole("button", { name: /choose workspace folder/i }));

    await waitFor(() => expect(settingsApi.pickFolder).toHaveBeenCalled());
    expect(onSet).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 7: Run the test to verify it fails**

Run: `npm run test`
Expected: FAIL with "Cannot find module './WorkspaceSetup'".

- [ ] **Step 8: Implement the component**

Create `src/components/WorkspaceSetup.tsx`:

```tsx
import { pickFolder, setWorkspaceRoot } from "../api/settings";

type Props = {
  onSet: (root: string) => void;
};

export function WorkspaceSetup({ onSet }: Props) {
  const handleChoose = async () => {
    const folder = await pickFolder();
    if (folder === null) {
      return;
    }
    await setWorkspaceRoot(folder);
    onSet(folder);
  };

  return (
    <div>
      <p>Choose a folder where downloaded builds will be stored.</p>
      <button onClick={handleChoose}>Choose workspace folder</button>
    </div>
  );
}
```

- [ ] **Step 9: Run the tests to verify they pass**

Run: `npm run test`
Expected: PASS — both tests pass.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/settings/mod.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src/api/settings.ts src/components/WorkspaceSetup.tsx src/components/WorkspaceSetup.test.tsx package.json package-lock.json
git commit -m "Add workspace root setting with folder picker"
```

---

### Task 18: Cache and active-directory path management

**Files:**
- Create: `src-tauri/src/sync/mod.rs`
- Create: `src-tauri/src/sync/cache.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/sync/cache.rs`:

```rust
use std::path::{Path, PathBuf};

pub fn project_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    workspace_root.join(project_key.replace('/', "-"))
}

pub fn cache_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    project_dir(workspace_root, project_key).join("cache")
}

pub fn active_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    project_dir(workspace_root, project_key).join("active")
}

pub fn cached_asset_path(
    workspace_root: &Path,
    project_key: &str,
    asset_id: u64,
    asset_name: &str,
) -> PathBuf {
    cache_dir(workspace_root, project_key).join(format!("{}-{}", asset_id, asset_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_replaces_slash_with_dash() {
        let root = Path::new("D:\\Builds");

        let dir = project_dir(root, "pixel-perfect/last-beacon");

        assert_eq!(dir, PathBuf::from("D:\\Builds\\pixel-perfect-last-beacon"));
    }

    #[test]
    fn cache_and_active_dirs_are_siblings_under_the_project_dir() {
        let root = Path::new("D:\\Builds");

        assert_eq!(
            cache_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org-repo\\cache")
        );
        assert_eq!(
            active_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org-repo\\active")
        );
    }

    #[test]
    fn cached_asset_path_is_keyed_by_id_and_name() {
        let root = Path::new("D:\\Builds");

        let path = cached_asset_path(root, "org/repo", 42, "build-shipping.zip");

        assert_eq!(
            path,
            PathBuf::from("D:\\Builds\\org-repo\\cache\\42-build-shipping.zip")
        );
    }
}
```

- [ ] **Step 2: Create the module file**

Create `src-tauri/src/sync/mod.rs`:

```rust
pub mod cache;
```

- [ ] **Step 3: Register the module**

In `src-tauri/src/lib.rs`, add:

```rust
mod sync;
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test sync::cache::`
Expected: PASS — all three tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync/mod.rs src-tauri/src/sync/cache.rs src-tauri/src/lib.rs
git commit -m "Add cache/active directory path management"
```

---

### Task 19: Download with progress and size verification

**Files:**
- Create: `src-tauri/src/sync/download.rs`
- Modify: `src-tauri/src/sync/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/sync/download.rs`:

```rust
use futures_util::StreamExt;
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloaded size {actual} does not match expected size {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
}

pub async fn download_with_progress<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    url: &str,
    destination: &Path,
    expected_size: u64,
    mut on_progress: F,
) -> Result<(), DownloadError> {
    let response = http
        .get(url)
        .header("User-Agent", "pixel-build-manager")
        .send()
        .await?;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(destination).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, expected_size);
    }
    file.flush().await?;

    if downloaded != expected_size {
        tokio::fs::remove_file(destination).await.ok();
        return Err(DownloadError::SizeMismatch {
            expected: expected_size,
            actual: downloaded,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn downloads_file_and_reports_progress() {
        let server = MockServer::start().await;
        let body = vec![7u8; 1000];
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("asset.zip");
        let http = reqwest::Client::new();
        let mut last_progress = (0u64, 0u64);

        download_with_progress(
            &http,
            &format!("{}/asset.zip", server.uri()),
            &destination,
            1000,
            |downloaded, total| last_progress = (downloaded, total),
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), body);
        assert_eq!(last_progress, (1000, 1000));
    }

    #[tokio::test]
    async fn size_mismatch_deletes_the_partial_file_and_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8; 10]))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("asset.zip");
        let http = reqwest::Client::new();

        let result = download_with_progress(
            &http,
            &format!("{}/asset.zip", server.uri()),
            &destination,
            999,
            |_, _| {},
        )
        .await;

        assert!(matches!(result, Err(DownloadError::SizeMismatch { expected: 999, actual: 10 })));
        assert!(!destination.exists());
    }
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/sync/mod.rs`, add:

```rust
pub mod download;
```

- [ ] **Step 3: Run the tests**

Run: `cd src-tauri && cargo test sync::download::`
Expected: PASS — both tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/sync/download.rs src-tauri/src/sync/mod.rs
git commit -m "Add streaming download with progress reporting and size verification"
```

---

### Task 20: Zip extraction with atomic swap into the active directory

**Files:**
- Create: `src-tauri/src/sync/extract.rs`
- Modify: `src-tauri/src/sync/mod.rs`

Note: this assumes release assets are `.zip` files, per the original request. If a tracked repo publishes `.tar.gz` assets instead (like last-beacon does today), extraction will fail with a clear `ExtractError` rather than silently doing nothing — adding `.tar.gz` support is a documented future extension, not handled here.

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/sync/extract.rs`:

```rust
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

pub fn extract_zip_to_active(zip_path: &Path, active_dir: &Path) -> Result<(), ExtractError> {
    let temp_dir = active_dir.with_extension("tmp-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let out_path = match entry.enclosed_name() {
            Some(p) => temp_dir.join(p),
            None => continue,
        };

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut out_file)?;
        }
    }

    if active_dir.exists() {
        fs::remove_dir_all(active_dir)?;
    }
    fs::rename(&temp_dir, active_dir)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_zip(path: &Path, files: &[(&str, &str)]) {
        let file = fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        for (name, contents) in files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn extracts_zip_contents_into_the_active_dir() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("build.zip");
        write_test_zip(&zip_path, &[("game.exe", "binary-contents"), ("readme.txt", "hello")]);
        let active_dir = dir.path().join("active");

        extract_zip_to_active(&zip_path, &active_dir).unwrap();

        assert_eq!(fs::read_to_string(active_dir.join("readme.txt")).unwrap(), "hello");
        assert_eq!(fs::read_to_string(active_dir.join("game.exe")).unwrap(), "binary-contents");
    }

    #[test]
    fn replaces_a_pre_existing_active_dir_entirely() {
        let dir = tempfile::tempdir().unwrap();
        let active_dir = dir.path().join("active");
        fs::create_dir_all(&active_dir).unwrap();
        fs::write(active_dir.join("stale-file.txt"), "old build").unwrap();

        let zip_path = dir.path().join("build.zip");
        write_test_zip(&zip_path, &[("new-file.txt", "new build")]);

        extract_zip_to_active(&zip_path, &active_dir).unwrap();

        assert!(!active_dir.join("stale-file.txt").exists());
        assert_eq!(fs::read_to_string(active_dir.join("new-file.txt")).unwrap(), "new build");
    }
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/sync/mod.rs`, add:

```rust
pub mod extract;
```

- [ ] **Step 3: Run the tests**

Run: `cd src-tauri && cargo test sync::extract::`
Expected: PASS — both tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/sync/extract.rs src-tauri/src/sync/mod.rs
git commit -m "Add zip extraction with atomic swap into the active directory"
```

---

### Task 21: Sync orchestrator and active-release settings tracking

**Files:**
- Create: `src-tauri/src/sync/orchestrator.rs`
- Modify: `src-tauri/src/sync/mod.rs`
- Modify: `src-tauri/src/settings/mod.rs`

- [ ] **Step 1: Add active-release tracking to settings**

In `src-tauri/src/settings/mod.rs`, add this method inside `impl Settings { ... }`:

```rust
    pub fn set_active_release(&mut self, project_key: &str, release_tag: &str, asset_name: &str) {
        let project = self.projects.entry(project_key.to_string()).or_default();
        project.active_release_tag = Some(release_tag.to_string());
        project.active_asset_name = Some(asset_name.to_string());
    }
```

Add this test inside `mod tests { ... }`:

```rust
    #[test]
    fn set_active_release_records_tag_and_asset() {
        let mut settings = Settings::default();

        settings.set_active_release("org/repo", "0.2.14", "build-shipping.zip");

        let project = &settings.projects["org/repo"];
        assert_eq!(project.active_release_tag, Some("0.2.14".to_string()));
        assert_eq!(project.active_asset_name, Some("build-shipping.zip".to_string()));
    }
```

Run: `cd src-tauri && cargo test settings::`
Expected: PASS.

- [ ] **Step 2: Write the failing orchestrator test**

Create `src-tauri/src/sync/orchestrator.rs`:

```rust
use crate::sync::cache::{active_dir, cache_dir, cached_asset_path};
use crate::sync::download::{download_with_progress, DownloadError};
use crate::sync::extract::{extract_zip_to_active, ExtractError};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Extract(#[from] ExtractError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SyncRequest<'a> {
    pub workspace_root: &'a Path,
    pub project_key: &'a str,
    pub asset_id: u64,
    pub asset_name: &'a str,
    pub asset_size: u64,
    pub download_url: &'a str,
}

pub async fn sync_asset<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    request: SyncRequest<'_>,
    on_progress: F,
) -> Result<(), SyncError> {
    std::fs::create_dir_all(cache_dir(request.workspace_root, request.project_key))?;
    let cached_path = cached_asset_path(
        request.workspace_root,
        request.project_key,
        request.asset_id,
        request.asset_name,
    );

    if !cached_path.exists() {
        download_with_progress(
            http,
            request.download_url,
            &cached_path,
            request.asset_size,
            on_progress,
        )
        .await?;
    }

    let active = active_dir(request.workspace_root, request.project_key);
    extract_zip_to_active(&cached_path, &active)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
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

    #[tokio::test]
    async fn syncs_downloads_extracts_and_skips_redownload_on_second_sync() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(move |_: &wiremock::Request| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone())
            })
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let request = || SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_bytes_len(),
            download_url: &format!("{}/asset.zip", server.uri()),
        };

        sync_asset(&http, request(), |_, _| {}).await.unwrap();

        let active = active_dir(workspace.path(), "org/repo");
        assert_eq!(
            std::fs::read_to_string(active.join("game.exe")).unwrap(),
            "binary-contents"
        );
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        sync_asset(&http, request(), |_, _| {}).await.unwrap();

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "second sync of the same asset must not re-download"
        );

        fn zip_bytes_len() -> u64 {
            build_test_zip_bytes().len() as u64
        }
    }
}
```

- [ ] **Step 3: Register the module**

In `src-tauri/src/sync/mod.rs`, add:

```rust
pub mod orchestrator;
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test sync::orchestrator::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync/orchestrator.rs src-tauri/src/sync/mod.rs src-tauri/src/settings/mod.rs
git commit -m "Add sync orchestrator tying download, cache, and extraction together"
```

---

### Task 22: Tauri commands for sync, active state, and cache clearing

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add sync-related imports and a progress event payload**

In `src-tauri/src/lib.rs`, add to the imports:

```rust
use sync::orchestrator::{sync_asset, SyncRequest};
use sync::cache::cache_dir;
```

Add this struct above `pub fn run()`:

```rust
#[derive(Debug, Clone, Serialize)]
struct SyncProgressPayload {
    project_key: String,
    downloaded: u64,
    total: u64,
}
```

- [ ] **Step 2: Add the `sync_release_asset` command**

Add this command near the other commands:

```rust
#[tauri::command]
async fn sync_release_asset(
    app: tauri::AppHandle,
    project_key: String,
    release_tag: String,
    asset_id: u64,
    asset_name: String,
    asset_size: u64,
    download_url: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .clone()
        .ok_or_else(|| "workspace root not set".to_string())?;

    let http = reqwest::Client::new();
    let request = SyncRequest {
        workspace_root: &workspace_root,
        project_key: &project_key,
        asset_id,
        asset_name: &asset_name,
        asset_size,
        download_url: &download_url,
    };

    let project_key_for_events = project_key.clone();
    sync_asset(&http, request, move |downloaded, total| {
        let _ = app.emit(
            "sync-progress",
            SyncProgressPayload {
                project_key: project_key_for_events.clone(),
                downloaded,
                total,
            },
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_active_release(&project_key, &release_tag, &asset_name);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Add `get_active_release` and `clear_project_cache` commands**

```rust
#[derive(Debug, Clone, Serialize)]
struct ActiveRelease {
    release_tag: Option<String>,
    asset_name: Option<String>,
}

#[tauri::command]
fn get_active_release(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<ActiveRelease, String> {
    let settings = Settings::load_from(&state.settings_path);
    let project = settings.projects.get(&project_key);
    Ok(ActiveRelease {
        release_tag: project.and_then(|p| p.active_release_tag.clone()),
        asset_name: project.and_then(|p| p.active_asset_name.clone()),
    })
}

#[tauri::command]
fn clear_project_cache(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())?;
    let dir = cache_dir(&workspace_root, &project_key);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

- [ ] **Step 4: Register the new commands**

Update `tauri::generate_handler![...]` in `run()` to also include `sync_release_asset`, `get_active_release`, and `clear_project_cache`.

- [ ] **Step 5: Verify it compiles and existing tests still pass**

Run: `cd src-tauri && cargo build && cargo test`
Expected: builds successfully; all tests still PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "Add sync_release_asset, get_active_release, and clear_project_cache commands"
```

---

### Task 23: Frontend sync API wrapper and `useSync` state machine hook

**Files:**
- Create: `src/api/sync.ts`
- Create: `src/hooks/useSync.ts`
- Create: `src/hooks/useSync.test.ts`

- [ ] **Step 1: Write the API wrapper**

Create `src/api/sync.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Release, ReleaseAsset } from "./projects";

export type SyncProgress = {
  project_key: string;
  downloaded: number;
  total: number;
};

export type ActiveRelease = {
  release_tag: string | null;
  asset_name: string | null;
};

export function syncReleaseAsset(
  projectKey: string,
  release: Release,
  asset: ReleaseAsset,
): Promise<void> {
  return invoke("sync_release_asset", {
    projectKey,
    releaseTag: release.tag_name,
    assetId: asset.id,
    assetName: asset.name,
    assetSize: asset.size,
    downloadUrl: asset.browser_download_url,
  });
}

export function getActiveRelease(projectKey: string): Promise<ActiveRelease> {
  return invoke("get_active_release", { projectKey });
}

export function clearProjectCache(projectKey: string): Promise<void> {
  return invoke("clear_project_cache", { projectKey });
}

export function onSyncProgress(callback: (progress: SyncProgress) => void) {
  return listen<SyncProgress>("sync-progress", (event) => callback(event.payload));
}
```

- [ ] **Step 2: Write the failing test for the hook**

Create `src/hooks/useSync.test.ts`:

```typescript
import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useSync } from "./useSync";
import * as syncApi from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

vi.mock("../api/sync");

const release: Release = {
  id: 1,
  tag_name: "0.2.14",
  name: "0.2.14",
  prerelease: false,
  published_at: null,
  assets: [],
};
const asset: ReleaseAsset = {
  id: 10,
  name: "build.zip",
  size: 1000,
  browser_download_url: "https://example.com/build.zip",
};

describe("useSync", () => {
  beforeEach(() => {
    vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  });

  it("transitions to done after a successful sync", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await result.current.sync(release, asset);
    });

    expect(result.current.state).toEqual({ phase: "done" });
  });

  it("transitions to error when the sync call rejects", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockRejectedValue(new Error("network down"));
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await result.current.sync(release, asset);
    });

    expect(result.current.state).toEqual({ phase: "error", message: "Error: network down" });
  });

  it("updates progress only for matching project_key events", async () => {
    let capturedCallback: (progress: syncApi.SyncProgress) => void = () => {};
    vi.mocked(syncApi.onSyncProgress).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useSync("org/repo"));

    act(() => {
      result.current.sync(release, asset);
    });
    act(() => {
      capturedCallback({ project_key: "org/other-repo", downloaded: 5, total: 1000 });
    });
    expect(result.current.state).toEqual({ phase: "syncing", downloaded: 0, total: 1000 });

    act(() => {
      capturedCallback({ project_key: "org/repo", downloaded: 500, total: 1000 });
    });
    await waitFor(() =>
      expect(result.current.state).toEqual({ phase: "syncing", downloaded: 500, total: 1000 }),
    );
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `npm run test`
Expected: FAIL with "Cannot find module './useSync'".

- [ ] **Step 4: Implement the hook**

Create `src/hooks/useSync.ts`:

```typescript
import { useCallback, useEffect, useRef, useState } from "react";
import { onSyncProgress, syncReleaseAsset } from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

export type SyncState =
  | { phase: "idle" }
  | { phase: "syncing"; downloaded: number; total: number }
  | { phase: "done" }
  | { phase: "error"; message: string };

export function useSync(projectKey: string) {
  const [state, setState] = useState<SyncState>({ phase: "idle" });
  const unlistenRef = useRef<() => void>();

  useEffect(() => {
    onSyncProgress((progress) => {
      if (progress.project_key !== projectKey) {
        return;
      }
      setState({ phase: "syncing", downloaded: progress.downloaded, total: progress.total });
    }).then((unlisten) => {
      unlistenRef.current = unlisten;
    });

    return () => unlistenRef.current?.();
  }, [projectKey]);

  const sync = useCallback(
    async (release: Release, asset: ReleaseAsset) => {
      setState({ phase: "syncing", downloaded: 0, total: asset.size });
      try {
        await syncReleaseAsset(projectKey, release, asset);
        setState({ phase: "done" });
      } catch (error) {
        setState({ phase: "error", message: String(error) });
      }
    },
    [projectKey],
  );

  return { state, sync };
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm run test`
Expected: PASS — all three tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/api/sync.ts src/hooks/useSync.ts src/hooks/useSync.test.ts
git commit -m "Add sync API wrapper and useSync state machine hook"
```

---

### Task 24: Sync status UI component

**Files:**
- Create: `src/components/SyncStatus.tsx`
- Create: `src/components/SyncStatus.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `src/components/SyncStatus.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SyncStatus } from "./SyncStatus";

describe("SyncStatus", () => {
  it("renders nothing when idle", () => {
    const { container } = render(<SyncStatus state={{ phase: "idle" }} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows download percentage while syncing", () => {
    render(<SyncStatus state={{ phase: "syncing", downloaded: 500, total: 1000 }} />);
    expect(screen.getByText(/50%/)).toBeInTheDocument();
  });

  it("shows a finishing-up message once fully downloaded but not yet done", () => {
    render(<SyncStatus state={{ phase: "syncing", downloaded: 1000, total: 1000 }} />);
    expect(screen.getByText(/finishing up/i)).toBeInTheDocument();
  });

  it("shows a success message when done", () => {
    render(<SyncStatus state={{ phase: "done" }} />);
    expect(screen.getByText(/synced/i)).toBeInTheDocument();
  });

  it("shows the error message on failure", () => {
    render(<SyncStatus state={{ phase: "error", message: "network down" }} />);
    expect(screen.getByText(/network down/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test`
Expected: FAIL with "Cannot find module './SyncStatus'".

- [ ] **Step 3: Implement the component**

Create `src/components/SyncStatus.tsx`:

```tsx
import type { SyncState } from "../hooks/useSync";

type Props = {
  state: SyncState;
};

export function SyncStatus({ state }: Props) {
  if (state.phase === "idle") {
    return null;
  }

  if (state.phase === "syncing") {
    if (state.downloaded >= state.total) {
      return <p>Finishing up...</p>;
    }
    const percent = Math.round((state.downloaded / state.total) * 100);
    return <p>Downloading: {percent}%</p>;
  }

  if (state.phase === "done") {
    return <p>Synced.</p>;
  }

  return <p>Sync failed: {state.message}</p>;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test`
Expected: PASS — all five tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/components/SyncStatus.tsx src/components/SyncStatus.test.tsx
git commit -m "Add sync status UI component"
```

---

### Task 25: Wire sync into the app shell and manually verify end-to-end

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Update `App.tsx` to require a workspace root and wire up syncing**

Replace the contents of `src/App.tsx` with:

```tsx
import { useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { getWorkspaceRoot } from "./api/settings";
import { getActiveRelease } from "./api/sync";
import { Login } from "./components/Login";
import { ProjectList } from "./components/ProjectList";
import { ReleaseList } from "./components/ReleaseList";
import { SyncStatus } from "./components/SyncStatus";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { useSync } from "./hooks/useSync";
import "./App.css";

function ProjectDetail({ projectKey }: { projectKey: string }) {
  const [releases, setReleases] = useState<Release[]>([]);
  const [activeAssetName, setActiveAssetName] = useState<string | null>(null);
  const { state, sync } = useSync(projectKey);

  useEffect(() => {
    listReleasesForProject(projectKey).then(setReleases);
    getActiveRelease(projectKey).then((active) => setActiveAssetName(active.asset_name));
  }, [projectKey]);

  const handleSync = async (release: Release, assetId: number) => {
    const asset = release.assets.find((a) => a.id === assetId) as ReleaseAsset;
    await sync(release, asset);
    setActiveAssetName(asset.name);
  };

  return (
    <div>
      <SyncStatus state={state} />
      <ReleaseList releases={releases} activeAssetName={activeAssetName} onSync={handleSync} />
    </div>
  );
}

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);
  const [workspaceRoot, setWorkspaceRootState] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);

  useEffect(() => {
    isLoggedIn().then(setLoggedIn);
  }, []);

  useEffect(() => {
    if (loggedIn) {
      getWorkspaceRoot().then(setWorkspaceRootState);
      listProjects().then(setProjects);
    }
  }, [loggedIn]);

  const handleToggleFavorite = async (fullName: string, favorite: boolean) => {
    await toggleFavorite(fullName, favorite);
    setProjects((prev) => prev.map((p) => (p.full_name === fullName ? { ...p, favorite } : p)));
  };

  if (loggedIn === null) {
    return <p>Loading...</p>;
  }

  if (!loggedIn) {
    return <Login onLoggedIn={() => setLoggedIn(true)} />;
  }

  if (!workspaceRoot) {
    return <WorkspaceSetup onSet={setWorkspaceRootState} />;
  }

  return (
    <div>
      <button
        onClick={async () => {
          await logout();
          setLoggedIn(false);
        }}
      >
        Log out
      </button>
      <ProjectList projects={projects} onSelect={setSelectedProject} onToggleFavorite={handleToggleFavorite} />
      {selectedProject && <ProjectDetail projectKey={selectedProject} />}
    </div>
  );
}

export default App;
```

- [ ] **Step 2: Manually verify the full sync workflow end-to-end**

Run: `npm run tauri dev`, log in, choose a workspace folder (e.g. a fresh `D:\BuildsTest\`), select a project with releases, and click "Sync" on one of its assets. Expected:
1. A download percentage appears and climbs to 100%, then "Finishing up...", then "Synced.".
2. `D:\BuildsTest\<owner>-<repo>\cache\` contains the downloaded zip.
3. `D:\BuildsTest\<owner>-<repo>\active\` contains the extracted build contents.
4. The synced asset's row in the release list now shows "(Active)".
5. Click "Sync" on a **different** asset (same or different release). Confirm `active\` is fully replaced (old files gone, new files present), and the previously-active asset no longer shows "(Active)" while the new one does.
6. Re-sync the same already-active asset again — confirm it completes quickly without a new entry appearing in `cache\` (proving the cache-skip logic works) and the download percentage jumps straight to 100%.
7. Quit and relaunch the app, reselect the project — confirm the "(Active)" marker is still correct (proving it persisted to settings).

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "Wire sync workflow into app shell"
```

**Phase 3 complete.** The app can now download, cache, and extract release assets into a per-project active directory, enforcing a single active build per project.

---

## Phase 4: CI/CD & Branching

### Task 26: Create the remote repository and `main`/`dev` branches

**Files:** none (repo/GitHub configuration only)

- [ ] **Step 1: Create the GitHub repository and push `main`**

Decide the destination: your personal account or the "Pixel Perfect" org. Run (replacing `<owner>` accordingly):

```bash
gh repo create <owner>/pixel-build-manager --private --source=. --remote=origin --push
```

Expected: creates the repo on GitHub, adds `origin`, and pushes the existing `main` branch (with all commits from Phases 1-3) to it.

- [ ] **Step 2: Create and push `dev` from `main`**

```bash
git checkout -b dev
git push -u origin dev
git checkout main
```

Expected: `dev` now exists on GitHub, identical to `main` at this point.

- [ ] **Step 3: Verify both branches exist remotely**

Run: `gh api repos/<owner>/pixel-build-manager/branches --jq '.[].name'`
Expected output includes both `main` and `dev`.

---

### Task 27: Configure branch protection rules

**Files:** none (GitHub configuration only)

- [ ] **Step 1: Protect `main`**

Run (replace `<owner>`):

```bash
gh api repos/<owner>/pixel-build-manager/branches/main/protection \
  --method PUT \
  --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["Source branch policy", "Validate Pixel Build Manager"]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1
  },
  "restrictions": null
}
EOF
```

This requires the `source-branch-policy` and `validate` jobs (added in Task 28) to pass, and requires at least one PR approval, before merging into `main`.

- [ ] **Step 2: Protect `dev`**

```bash
gh api repos/<owner>/pixel-build-manager/branches/dev/protection \
  --method PUT \
  --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["Source branch policy", "Validate Pixel Build Manager"]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1
  },
  "restrictions": null
}
EOF
```

- [ ] **Step 3: Verify the rules are active**

Run: `gh api repos/<owner>/pixel-build-manager/branches/main/protection --jq '.required_status_checks.contexts'`
Expected output: `["Source branch policy", "Validate Pixel Build Manager"]`

Repeat for `dev`.

Note: these status check names won't actually gate anything until the workflow in Task 28 has run at least once on this repo (GitHub only lets you require checks it has seen reported). If step 1/2 above errors because the checks are unrecognized yet, complete Task 28 first, push once to a throwaway branch to let the checks report at least once, then re-run this task.

---

### Task 28: Add the GitHub Actions build/release workflow

**Files:**
- Create: `.github/workflows/pixel-build-manager-build.yml`

- [ ] **Step 1: Write the workflow file**

Create `.github/workflows/pixel-build-manager-build.yml`:

```yaml
name: Pixel Build Manager Build

on:
  push:
    branches:
      - main
      - dev
  pull_request:
    branches:
      - main
      - dev
  workflow_dispatch:

permissions:
  contents: read
  actions: read

concurrency:
  group: pixel-build-manager-build-${{ github.ref }}
  cancel-in-progress: false

env:
  CARGO_TARGET_DIR: C:\actions-runner\cargo-target\PixelBuildManager

jobs:
  source-branch-policy:
    name: Source branch policy
    runs-on: [self-hosted, Windows]
    steps:
      - name: Require approved pull request source branches
        shell: cmd
        run: |
          if not "%GITHUB_EVENT_NAME%"=="pull_request" exit /b 0

          if "%GITHUB_BASE_REF%"=="dev" (
            if "%GITHUB_HEAD_REF%"=="main" exit /b 0
            if "%GITHUB_HEAD_REF:~0,8%"=="feature/" exit /b 0
            echo Pull requests into dev must come from main or feature/*.
            echo Actual source branch: %GITHUB_HEAD_REF%
            exit /b 1
          )

          if "%GITHUB_BASE_REF%"=="main" (
            if "%GITHUB_HEAD_REF%"=="dev" exit /b 0
            if "%GITHUB_HEAD_REF:~0,7%"=="hotfix/" exit /b 0
            echo Pull requests into main must come from dev or hotfix/*.
            echo Actual source branch: %GITHUB_HEAD_REF%
            exit /b 1
          )

  validate:
    name: Validate Pixel Build Manager
    needs: source-branch-policy
    runs-on: [self-hosted, Windows]
    steps:
      - name: Check out repository
        uses: actions/checkout@v4

      - name: Verify Rust tools
        shell: cmd
        run: |
          where cargo
          where rustup

      - name: Check Rust formatting
        shell: cmd
        working-directory: src-tauri
        run: cargo fmt --check

      - name: Lint Rust with clippy
        shell: cmd
        working-directory: src-tauri
        run: cargo clippy --all-targets -- -D warnings

      - name: Run Rust tests
        shell: cmd
        working-directory: src-tauri
        run: cargo test

      - name: Build Rust backend
        shell: cmd
        working-directory: src-tauri
        run: cargo build

      - name: Install frontend dependencies
        shell: cmd
        run: npm ci

      - name: Build frontend
        shell: cmd
        run: npm run build

      - name: Run frontend tests
        shell: cmd
        run: npm run test

  compute-version:
    name: Compute next version
    if: github.event_name == 'push' && (github.ref == 'refs/heads/dev' || github.ref == 'refs/heads/main')
    needs: validate
    runs-on: [self-hosted, Windows]
    outputs:
      tag: ${{ steps.next-version.outputs.tag }}
    steps:
      - name: Calculate next version tag
        id: next-version
        uses: actions/github-script@v7
        with:
          script: |
            const tags = await github.paginate(github.rest.repos.listTags, {
              owner: context.repo.owner,
              repo: context.repo.repo,
              per_page: 100,
            });
            const versions = tags
              .map((tag) => tag.name.match(/^0\.(\d+)\.(\d+)$/))
              .filter(Boolean)
              .map((match) => ({
                minor: Number(match[1]),
                patch: Number(match[2]),
              }))
              .sort((left, right) => left.minor - right.minor || left.patch - right.patch);
            const latestVersion = versions.at(-1);

            let tag;
            if (context.ref === 'refs/heads/main') {
              const nextMinor = latestVersion === undefined ? 1 : latestVersion.minor + 1;
              tag = `0.${nextMinor}.0`;
            } else {
              const nextMinor = latestVersion === undefined ? 0 : latestVersion.minor;
              const nextPatch = latestVersion === undefined ? 0 : latestVersion.patch + 1;
              tag = `0.${nextMinor}.${nextPatch}`;
            }
            core.setOutput('tag', tag);

  package:
    name: Package ${{ matrix.platform }}
    if: github.event_name == 'push' && (github.ref == 'refs/heads/dev' || github.ref == 'refs/heads/main')
    needs: compute-version
    runs-on: [self-hosted, Windows]
    strategy:
      fail-fast: false
      matrix:
        platform:
          - windows-x64
    steps:
      - name: Check out repository
        uses: actions/checkout@v4

      - name: Install frontend dependencies
        shell: cmd
        run: npm ci

      - name: Write version into tauri.conf.json
        shell: pwsh
        run: |
          $configPath = "src-tauri/tauri.conf.json"
          $config = Get-Content $configPath -Raw | ConvertFrom-Json
          $config.version = "${{ needs.compute-version.outputs.tag }}"
          $config | ConvertTo-Json -Depth 32 | Set-Content $configPath

      - name: Build Tauri app
        shell: cmd
        run: npm run tauri build

      - name: Upload installer artifact
        uses: actions/upload-artifact@v4
        with:
          name: pixel-build-manager-${{ matrix.platform }}
          path: src-tauri/target/release/bundle/**/*
          if-no-files-found: error

  release:
    name: Publish release
    if: github.event_name == 'push' && (github.ref == 'refs/heads/dev' || github.ref == 'refs/heads/main')
    needs: [compute-version, package]
    runs-on: [self-hosted, Windows]
    permissions:
      contents: write
      actions: read
    steps:
      - name: Check out repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Download package artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts/release-downloads
          merge-multiple: true

      - name: Create release tag
        shell: cmd
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git tag "${{ needs.compute-version.outputs.tag }}" "%GITHUB_SHA%"
          git push origin "${{ needs.compute-version.outputs.tag }}"

      - name: Publish GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs.compute-version.outputs.tag }}
          name: Pixel Build Manager ${{ needs.compute-version.outputs.tag }}
          draft: false
          prerelease: ${{ github.ref == 'refs/heads/dev' }}
          files: artifacts/release-downloads/**/*
```

- [ ] **Step 2: Commit and push to `main`**

```bash
git add .github/workflows/pixel-build-manager-build.yml
git commit -m "Add build/package/release workflow for main and dev"
git push origin main
```

- [ ] **Step 3: Sync `dev` with `main` and push**

```bash
git checkout dev
git merge main
git push origin dev
git checkout main
```

Expected: the push to `main` and the push to `dev` each trigger the workflow, running `source-branch-policy` (no-op, not a PR) → `validate` → `compute-version` → `package` → `release`, publishing `0.1.0` (from `main`) and `0.0.1` (from `dev`, if `main`'s push landed first) as GitHub Releases with the Windows installer attached. Check the Actions tab and Releases page to confirm.

- [ ] **Step 4: Re-run Task 27 if branch protection setup failed earlier**

If Task 27's `gh api ... branches/*/protection` calls failed because the status checks weren't recognized yet, re-run them now that the workflow has reported at least one run.

---

### Task 29: Verify the full git-flow policy end-to-end

**Files:** none (verification only)

- [ ] **Step 1: Verify a disallowed PR is rejected**

```bash
git checkout -b not-a-feature-branch
git commit --allow-empty -m "test: trigger branch policy"
git push -u origin not-a-feature-branch
gh pr create --base dev --head not-a-feature-branch --title "test: should be rejected" --body "Verifying source-branch-policy"
```

Expected: the `source-branch-policy` check fails on the PR (since `not-a-feature-branch` doesn't start with `feature/` and isn't `main`), and branch protection blocks merging.

Clean up: `gh pr close <pr-number>` and `git push origin --delete not-a-feature-branch`.

- [ ] **Step 2: Verify an allowed PR succeeds**

```bash
git checkout main
git checkout -b feature/plan-verification
git commit --allow-empty -m "test: trigger branch policy"
git push -u origin feature/plan-verification
gh pr create --base dev --head feature/plan-verification --title "test: should pass" --body "Verifying source-branch-policy"
```

Expected: `source-branch-policy` and `validate` both pass. Merge the PR (`gh pr merge --squash`) and confirm a new `dev` prerelease is published with an incremented patch version.

Clean up: `git push origin --delete feature/plan-verification` and delete the local branch.

- [ ] **Step 3: Verify `dev` → `main` promotion**

```bash
gh pr create --base main --head dev --title "Promote dev to main" --body "Verifying source-branch-policy for main"
```

Expected: checks pass (source branch `dev` is allowed into `main`). Merge it and confirm a new full release is published on `main` with an incremented minor version and patch reset to 0.

**Phase 4 complete.** The repo has `main`/`dev` branches with enforced git-flow PR policy and branch protection, and every push to either branch produces a versioned, installable release of the app.

---

## Plan self-review notes

- **Spec coverage:** Architecture (Tasks 1-9), auth (Tasks 4-9), project browsing + favorites (Tasks 10-16), release sync with cache/active-dir/atomic-swap/size-verification (Tasks 17-25), error handling (size mismatch in Task 19, atomic swap in Task 20, re-login-on-expiry via `LoginStatus::Expired`/`Denied` in Task 6/8), testing approach (unit tests throughout backend via `wiremock`/`tempfile`, frontend tests via Vitest), branch strategy + CI/CD (Tasks 26-29) are all covered.
- **Deviation from spec carried through consistently:** GitHub API access uses a plain `reqwest`-based client instead of `octocrab` (see Tech Stack note and Task 10) — applied consistently in Tasks 10-11, 13, and the CI/CD section needed no changes as a result.
- **Type consistency checked:** `ProjectSettings.active_release_tag`/`active_asset_name` (Task 3) match the fields written in `Settings::set_active_release` (Task 21) and read in `get_active_release` (Task 22). `LoginStatus` variants (Task 6) match the frontend's `LoginStatus` union and the `status` tag values used in `Login.tsx` (Task 8). `SyncState` phases in `useSync` (Task 23) match what `SyncStatus.tsx` (Task 24) switches on.
