mod auth;
mod github;
mod latest_poller;
mod settings;
mod sync;
mod updater;
mod version;

use auth::device_flow::DeviceFlowClient;
use auth::login::{perform_device_login, LoginStatus};
use auth::session::{ensure_valid_access_token, SESSION_EXPIRED};
use auth::token_store::{KeyringTokenStore, TokenStore};
use github::client::{GithubClient, ReleaseSummary};
use github::repo_url::resolve_bindable_repo;
use latest_poller::start_background_latest_sync;
use serde::Serialize;
use settings::{Settings, SyncMode, Theme};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use sync::cache::{build_config_dir, builds_root_dir, cache_dir, LatestChannel};
use sync::launch::{find_build_executable, launch_executable};
use sync::orchestrator::{sync_asset, SyncRequest};
use tauri::{Emitter, Manager};
use updater::channel::Channel;
use updater::start_background_updates;

const GITHUB_CLIENT_ID: &str = "Ov23ligQDGOJvlWsEXJc";

pub struct AppState {
    pub token_store: Arc<dyn TokenStore>,
    pub settings_path: PathBuf,
    /// Serializes settings.json read-modify-write cycles across commands so
    /// concurrent writes (e.g. rapid favorite toggles) can't clobber each other.
    pub settings_lock: Mutex<()>,
    /// Project keys with an in-flight sync or cache-clear operation, so the
    /// two can never race against each other's filesystem writes for the
    /// same project (e.g. clearing a cache mid-download).
    pub active_operations: Mutex<HashSet<String>>,
    /// Serializes token refreshes so two concurrent GitHub-backed commands
    /// can't both try to redeem the same (single-use) refresh token at
    /// once. Held across an `.await`, so this must be a tokio mutex rather
    /// than `std::sync::Mutex`.
    pub token_refresh_lock: tokio::sync::Mutex<()>,
}

/// Marks `project_key` as having an in-flight operation, failing if one is
/// already running. Callers must pair this with `end_operation` on every
/// exit path (success or error).
pub(crate) fn begin_operation(state: &AppState, project_key: &str) -> Result<(), String> {
    let mut active = state.active_operations.lock().map_err(|e| e.to_string())?;
    if !active.insert(project_key.to_string()) {
        return Err(format!(
            "another sync or cache operation is already in progress for {project_key}"
        ));
    }
    Ok(())
}

pub(crate) fn end_operation(state: &AppState, project_key: &str) {
    if let Ok(mut active) = state.active_operations.lock() {
        active.remove(project_key);
    }
}

fn settings_path_for(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir must be resolvable")
        .join("settings.json")
}

fn workspace_root_from_settings(state: &AppState) -> Result<PathBuf, String> {
    Settings::load_from(&state.settings_path)
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())
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
        if let Err(e) =
            perform_device_login(&client, token_store.as_ref(), move |status: LoginStatus| {
                let _ = app_for_events.emit("login-status", status);
            })
            .await
        {
            eprintln!("device login failed: {e}");
        }
    });

    Ok(())
}

/// The bound projects that logging out would remove (see `logout`).
#[tauri::command]
fn list_account_bound_projects(state: tauri::State<'_, AppState>) -> Vec<String> {
    Settings::load_from(&state.settings_path).account_bound_projects()
}

/// Logs out, and removes every project that was bound from the account's
/// project list: its tab, its per-project settings, and everything it has
/// on disk (cache and extracted builds). Projects bound by URL are kept.
/// Returns the removed project keys.
#[tauri::command]
fn logout(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    logout_and_remove_account_projects(&state)
}

fn logout_and_remove_account_projects(state: &AppState) -> Result<Vec<String>, String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    let to_remove = settings.account_bound_projects();

    // Claim every project first so no sync can write into a folder we're
    // about to delete; if one is mid-sync, refuse before changing anything.
    let mut claimed: Vec<String> = Vec::new();
    for key in &to_remove {
        if let Err(e) = begin_operation(state, key) {
            for claimed_key in &claimed {
                end_operation(state, claimed_key);
            }
            return Err(format!("Can't log out yet: {e}"));
        }
        claimed.push(key.clone());
    }

    let result = (|| {
        state.token_store.clear()?;

        let mut disk_errors = Vec::new();
        if let Some(workspace_root) = settings.workspace_root.clone() {
            for key in &to_remove {
                if let Err(e) = remove_project_from_disk(&workspace_root, key) {
                    disk_errors.push(format!("{key}: {e}"));
                }
            }
        }
        for key in &to_remove {
            settings.remove_project(key);
        }
        settings
            .save_to(&state.settings_path)
            .map_err(|e| e.to_string())?;

        if disk_errors.is_empty() {
            Ok(to_remove.clone())
        } else {
            Err(format!(
                "Logged out, but some project files couldn't be deleted: {}",
                disk_errors.join("; ")
            ))
        }
    })();

    for key in &claimed {
        end_operation(state, key);
    }
    result
}

/// Deletes a project's cache and builds, then its now-empty folders.
fn remove_project_from_disk(
    workspace_root: &std::path::Path,
    project_key: &str,
) -> Result<(), String> {
    for dir in [
        cache_dir(workspace_root, project_key),
        builds_root_dir(workspace_root, project_key),
    ] {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
    }
    // Only succeeds when empty, so anything else a user put there survives.
    let project = sync::cache::project_dir(workspace_root, project_key);
    let _ = std::fs::remove_dir(&project);
    if let Some(owner) = project.parent() {
        let _ = std::fs::remove_dir(owner);
    }
    Ok(())
}

#[tauri::command]
fn is_logged_in(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(state.token_store.load()?.is_some())
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectListItem {
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub favorite: bool,
}

pub(crate) async fn build_github_client(state: &AppState) -> Result<GithubClient, String> {
    let _guard = state.token_refresh_lock.lock().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after the unix epoch")
        .as_secs();
    let device_flow_client = DeviceFlowClient::new(GITHUB_CLIENT_ID.to_string());
    let token =
        ensure_valid_access_token(&device_flow_client, state.token_store.as_ref(), now).await?;
    Ok(GithubClient::new(token))
}

/// An authenticated client when the user is logged in, otherwise an
/// anonymous one. Logging in is optional: anonymously, only public repos
/// are reachable (at GitHub's lower unauthenticated rate limit). If the
/// stored session has expired it is cleared, and this falls back to
/// anonymous rather than failing, so public repos keep working.
pub(crate) async fn github_client_or_anonymous(state: &AppState) -> Result<GithubClient, String> {
    if state.token_store.load()?.is_none() {
        return Ok(GithubClient::anonymous());
    }
    match build_github_client(state).await {
        Err(e) if e == SESSION_EXPIRED => Ok(GithubClient::anonymous()),
        other => other,
    }
}

/// Lists the logged-in user's own and organisation repos that have
/// releases. Empty when logged out -- anonymously, repos can only be bound
/// by URL. A session that has expired still reports `SESSION_EXPIRED`, so
/// the frontend can switch to its logged-out state.
#[tauri::command]
async fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<ProjectListItem>, String> {
    if state.token_store.load()?.is_none() {
        return Ok(Vec::new());
    }
    let client = build_github_client(&state).await?;
    let repos = client
        .list_accessible_repos_with_releases()
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
    let client = github_client_or_anonymous(&state).await?;
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
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_favorite(&full_name, favorite);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_workspace_root(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .workspace_root
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn get_version_label(app: tauri::AppHandle) -> String {
    version::format_version_label(Channel::current(), &app.package_info().version.to_string())
}

#[tauri::command]
fn set_workspace_root(root: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_workspace_root(PathBuf::from(root));
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_bound_projects(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(Settings::load_from(&state.settings_path).bound_projects)
}

#[tauri::command]
fn bind_project(full_name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.bind_project_via(&full_name, false);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

/// Binds any GitHub repo with releases by its URL -- including public repos
/// the user isn't a member of, which never appear in `list_projects`.
/// Returns the bound project so the frontend can show it without a refetch.
#[tauri::command]
async fn bind_project_by_url(
    url: String,
    state: tauri::State<'_, AppState>,
) -> Result<ProjectListItem, String> {
    let client = github_client_or_anonymous(&state).await?;
    let repo = resolve_bindable_repo(&client, &url).await?;

    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.bind_project_via(&repo.full_name, true);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())?;

    let favorite = settings
        .projects
        .get(&repo.full_name)
        .map(|p| p.favorite)
        .unwrap_or(false);
    Ok(ProjectListItem {
        full_name: repo.full_name,
        owner: repo.owner.login,
        name: repo.name,
        favorite,
    })
}

#[tauri::command]
fn unbind_project(full_name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.unbind_project(&full_name);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_theme(state: tauri::State<'_, AppState>) -> Theme {
    Settings::load_from(&state.settings_path).theme
}

#[tauri::command]
fn set_theme(theme: Theme, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_theme(theme);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_selected_release(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .projects
        .get(&project_key)
        .and_then(|p| p.selected_release_tag.clone()))
}

#[tauri::command]
fn set_selected_release(
    project_key: String,
    release_tag: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_selected_release(&project_key, &release_tag);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_ticked_configs(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .projects
        .get(&project_key)
        .map(|p| p.ticked_configs.clone())
        .unwrap_or_default())
}

#[tauri::command]
fn set_ticked_configs(
    project_key: String,
    configs: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_ticked_configs(&project_key, configs);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_sync_mode(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<SyncMode, String> {
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
        settings
            .save_to(&state.settings_path)
            .map_err(|e| e.to_string())?;
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

#[tauri::command]
fn list_synced_configs(
    project_key: String,
    release_tag: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    sync::cache::list_synced_configs(&workspace_root, &project_key, &release_tag)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_synced_latest_configs(
    project_key: String,
    channel: LatestChannel,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    sync::cache::list_synced_latest_configs(&workspace_root, &project_key, channel)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
struct SyncProgressPayload {
    project_key: String,
    downloaded: u64,
    total: u64,
}

// Each parameter here is a distinct argument the frontend passes via
// `invoke`, mirroring the asset/release fields it already has on hand;
// bundling them into a struct would just move the same field count behind
// an extra layer without reducing what callers need to supply.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn sync_release_asset(
    app: tauri::AppHandle,
    project_key: String,
    release_tag: String,
    asset_id: u64,
    asset_name: String,
    asset_size: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    begin_operation(&state, &project_key)?;
    let result = sync_release_asset_inner(
        app,
        &project_key,
        &release_tag,
        asset_id,
        &asset_name,
        asset_size,
        &state,
    )
    .await;
    end_operation(&state, &project_key);
    result
}

#[allow(clippy::too_many_arguments)]
async fn sync_release_asset_inner(
    app: tauri::AppHandle,
    project_key: &str,
    release_tag: &str,
    asset_id: u64,
    asset_name: &str,
    asset_size: u64,
    state: &AppState,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(state)?;

    let client = github_client_or_anonymous(state).await?;
    let (owner, repo) = project_key
        .split_once('/')
        .ok_or_else(|| format!("invalid project_key: {project_key}"))?;
    let download_url = client.asset_download_url(owner, repo, asset_id);
    let auth_token = client.token();

    let http = reqwest::Client::new();
    let request = SyncRequest {
        workspace_root: &workspace_root,
        project_key,
        release_tag,
        asset_id,
        asset_name,
        asset_size,
        download_url: &download_url,
        auth_token,
    };

    let project_key_for_events = project_key.to_string();
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
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_project_cache(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    begin_operation(&state, &project_key)?;
    let result = clear_project_cache_inner(&project_key, &state);
    end_operation(&state, &project_key);
    result
}

// Clears everything this project has on disk: the raw downloaded cache
// *and* every extracted release/config build under `builds/`. There's no
// separate "clear builds" affordance, since leaving extracted builds behind
// after a cache clear would contradict "manually cleared" -- the one button
// is the manual-clear mechanism for the whole project's disk footprint.
fn clear_project_cache_inner(project_key: &str, state: &AppState) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(state)?;

    let cache = cache_dir(&workspace_root, project_key);
    if cache.exists() {
        std::fs::remove_dir_all(&cache).map_err(|e| e.to_string())?;
    }
    let builds = builds_root_dir(&workspace_root, project_key);
    if builds.exists() {
        std::fs::remove_dir_all(&builds).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_build_executable(
    project_key: String,
    release_tag: String,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = build_config_dir(&workspace_root, &project_key, &release_tag, &config_name);
    Ok(find_build_executable(&dir).map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn launch_build(
    project_key: String,
    release_tag: String,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = build_config_dir(&workspace_root, &project_key, &release_tag, &config_name);
    let exe = find_build_executable(&dir)
        .ok_or_else(|| "no executable found in this build".to_string())?;
    launch_executable(&exe).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_build_dir(
    project_key: String,
    release_tag: String,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = build_config_dir(&workspace_root, &project_key, &release_tag, &config_name);
    Ok(dir.exists().then(|| dir.to_string_lossy().to_string()))
}

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
    let exe = find_build_executable(&dir)
        .ok_or_else(|| "no executable found in this build".to_string())?;
    launch_executable(&exe).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = settings_path_for(app.handle());
            app.manage(AppState {
                token_store: Arc::new(KeyringTokenStore),
                settings_path,
                settings_lock: Mutex::new(()),
                active_operations: Mutex::new(HashSet::new()),
                token_refresh_lock: tokio::sync::Mutex::new(()),
            });

            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            start_background_updates(app.handle().clone());
            start_background_latest_sync(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            login_start,
            logout,
            list_account_bound_projects,
            is_logged_in,
            list_projects,
            list_releases_for_project,
            toggle_favorite,
            get_workspace_root,
            set_workspace_root,
            list_bound_projects,
            bind_project,
            bind_project_by_url,
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::token_store::InMemoryTokenStore;

    fn state_with_store(store: InMemoryTokenStore) -> AppState {
        state_with(store, PathBuf::from("unused-settings.json"))
    }

    fn state_with(store: InMemoryTokenStore, settings_path: PathBuf) -> AppState {
        AppState {
            token_store: Arc::new(store),
            settings_path,
            settings_lock: Mutex::new(()),
            active_operations: Mutex::new(HashSet::new()),
            token_refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    #[tokio::test]
    async fn logged_out_users_get_an_anonymous_client() {
        let state = state_with_store(InMemoryTokenStore::new());

        let client = github_client_or_anonymous(&state).await.unwrap();

        assert_eq!(client.token(), "");
    }

    #[tokio::test]
    async fn an_unusable_session_falls_back_to_anonymous_and_is_cleared() {
        let store = InMemoryTokenStore::new();
        store.save("not a stored-token json blob").unwrap();
        let state = state_with_store(store);

        let client = github_client_or_anonymous(&state).await.unwrap();

        assert_eq!(client.token(), "");
        assert_eq!(state.token_store.load().unwrap(), None);
    }

    #[tokio::test]
    async fn a_valid_session_gets_an_authenticated_client() {
        let store = InMemoryTokenStore::new();
        let far_future = u64::MAX / 2;
        auth::session::save_stored_token(
            &store,
            &auth::session::StoredToken {
                access_token: "live-token".to_string(),
                refresh_token: "refresh".to_string(),
                access_token_expires_at: far_future,
                refresh_token_expires_at: far_future,
            },
        )
        .unwrap();
        let state = state_with_store(store);

        let client = github_client_or_anonymous(&state).await.unwrap();

        assert_eq!(client.token(), "live-token");
    }

    /// A workspace with one account-bound and one URL-bound project, each
    /// with a cached download and an extracted build on disk.
    fn logged_in_workspace() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let mut settings = Settings::default();
        settings.set_workspace_root(workspace.clone());
        settings.bind_project_via("org/private-game", false);
        settings.set_selected_release("org/private-game", "1.0");
        settings.bind_project_via("someone/public-game", true);
        for key in ["org/private-game", "someone/public-game"] {
            std::fs::create_dir_all(cache_dir(&workspace, key)).unwrap();
            std::fs::write(cache_dir(&workspace, key).join("1-a.zip"), b"zip").unwrap();
            std::fs::create_dir_all(builds_root_dir(&workspace, key).join("1.0")).unwrap();
        }
        let settings_path = dir.path().join("settings.json");
        settings.save_to(&settings_path).unwrap();

        let store = InMemoryTokenStore::new();
        store.save("token").unwrap();
        (dir, state_with(store, settings_path))
    }

    #[test]
    fn logout_removes_account_bound_projects_and_their_files_but_keeps_url_bound_ones() {
        let (dir, state) = logged_in_workspace();
        let workspace = dir.path().join("workspace");

        let removed = logout_and_remove_account_projects(&state).unwrap();

        assert_eq!(removed, vec!["org/private-game"]);
        assert_eq!(state.token_store.load().unwrap(), None);
        let settings = Settings::load_from(&state.settings_path);
        assert_eq!(settings.bound_projects, vec!["someone/public-game"]);
        assert!(!settings.projects.contains_key("org/private-game"));
        assert!(!workspace.join("org").exists());
        assert!(cache_dir(&workspace, "someone/public-game")
            .join("1-a.zip")
            .exists());
        assert!(builds_root_dir(&workspace, "someone/public-game").exists());
    }

    #[test]
    fn logout_keeps_unrelated_files_in_a_removed_project_folder() {
        let (dir, state) = logged_in_workspace();
        let project = dir
            .path()
            .join("workspace")
            .join("org")
            .join("private-game");
        std::fs::write(project.join("notes.txt"), b"mine").unwrap();

        logout_and_remove_account_projects(&state).unwrap();

        assert!(project.join("notes.txt").exists());
        assert!(!project.join("cache").exists());
        assert!(!project.join("builds").exists());
    }

    #[test]
    fn logout_is_refused_without_changes_while_an_account_project_is_syncing() {
        let (_dir, state) = logged_in_workspace();
        begin_operation(&state, "org/private-game").unwrap();

        let err = logout_and_remove_account_projects(&state).unwrap_err();

        assert!(err.contains("Can't log out yet"), "{err}");
        assert!(state.token_store.load().unwrap().is_some());
        let settings = Settings::load_from(&state.settings_path);
        assert!(settings
            .bound_projects
            .contains(&"org/private-game".to_string()));
    }
}
