mod auth;
mod github;
mod settings;
mod sync;
mod updater;
mod version;

use auth::device_flow::DeviceFlowClient;
use auth::login::{perform_device_login, LoginStatus};
use auth::token_store::{KeyringTokenStore, TokenStore};
use github::client::{GithubClient, ReleaseSummary};
use serde::Serialize;
use settings::Settings;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use sync::cache::{active_dir, cache_dir, cached_asset_path, list_cached_asset_ids};
use sync::launch::{find_active_executable, launch_executable};
use sync::orchestrator::{ensure_asset_cached, sync_asset, SyncRequest};
use tauri::{Emitter, Manager};
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
}

/// Marks `project_key` as having an in-flight operation, failing if one is
/// already running. Callers must pair this with `end_operation` on every
/// exit path (success or error).
fn begin_operation(state: &AppState, project_key: &str) -> Result<(), String> {
    let mut active = state.active_operations.lock().map_err(|e| e.to_string())?;
    if !active.insert(project_key.to_string()) {
        return Err(format!(
            "another sync or cache operation is already in progress for {project_key}"
        ));
    }
    Ok(())
}

fn end_operation(state: &AppState, project_key: &str) {
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

#[tauri::command]
fn logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.token_store.clear()
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

fn build_github_client(state: &AppState) -> Result<GithubClient, String> {
    let token = state
        .token_store
        .load()?
        .ok_or_else(|| "not logged in".to_string())?;
    Ok(GithubClient::new(token))
}

#[tauri::command]
async fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<ProjectListItem>, String> {
    let client = build_github_client(&state)?;
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
fn set_workspace_root(root: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_workspace_root(PathBuf::from(root));
    settings
        .save_to(&state.settings_path)
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
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .clone()
        .ok_or_else(|| "workspace root not set".to_string())?;

    let client = build_github_client(state)?;
    let (owner, repo) = project_key
        .split_once('/')
        .ok_or_else(|| format!("invalid project_key: {project_key}"))?;
    let download_url = client.asset_download_url(owner, repo, asset_id);
    let auth_token = client.token();

    let http = reqwest::Client::new();
    let request = SyncRequest {
        workspace_root: &workspace_root,
        project_key,
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
    .map_err(|e| e.to_string())?;

    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_active_release(project_key, release_tag, asset_name);
    settings.save_to(&state.settings_path).map_err(|e| {
        format!(
            "build was downloaded and installed, but failed to record it as the active \
             release ({e}) -- try syncing again"
        )
    })
}

// Downloads/verifies the asset into the cache, same as sync_release_asset,
// but never extracts it into the active build dir or records it as active --
// this is the "Check"/"Sync" button's action, kept deliberately separate
// from picking an option (which does activate it).
#[tauri::command]
async fn check_release_asset(
    app: tauri::AppHandle,
    project_key: String,
    asset_id: u64,
    asset_name: String,
    asset_size: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    begin_operation(&state, &project_key)?;
    let result =
        check_release_asset_inner(app, &project_key, asset_id, &asset_name, asset_size, &state)
            .await;
    end_operation(&state, &project_key);
    result
}

async fn check_release_asset_inner(
    app: tauri::AppHandle,
    project_key: &str,
    asset_id: u64,
    asset_name: &str,
    asset_size: u64,
    state: &AppState,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(state)?;

    let client = build_github_client(state)?;
    let (owner, repo) = project_key
        .split_once('/')
        .ok_or_else(|| format!("invalid project_key: {project_key}"))?;
    let download_url = client.asset_download_url(owner, repo, asset_id);
    let auth_token = client.token();

    let http = reqwest::Client::new();
    let request = SyncRequest {
        workspace_root: &workspace_root,
        project_key,
        asset_id,
        asset_name,
        asset_size,
        download_url: &download_url,
        auth_token,
    };

    let project_key_for_events = project_key.to_string();
    ensure_asset_cached(&http, request, move |downloaded, total| {
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
    begin_operation(&state, &project_key)?;
    let result = clear_project_cache_inner(&project_key, &state);
    end_operation(&state, &project_key);
    result
}

fn clear_project_cache_inner(project_key: &str, state: &AppState) -> Result<(), String> {
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())?;
    let dir = cache_dir(&workspace_root, project_key);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn list_cached_assets(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<u64>, String> {
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())?;
    list_cached_asset_ids(&workspace_root, &project_key).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_cached_asset(
    project_key: String,
    asset_id: u64,
    asset_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    begin_operation(&state, &project_key)?;
    let result = delete_cached_asset_inner(&project_key, asset_id, &asset_name, &state);
    end_operation(&state, &project_key);
    result
}

fn delete_cached_asset_inner(
    project_key: &str,
    asset_id: u64,
    asset_name: &str,
    state: &AppState,
) -> Result<(), String> {
    let settings = Settings::load_from(&state.settings_path);
    let workspace_root = settings
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())?;
    let path = cached_asset_path(&workspace_root, project_key, asset_id, asset_name);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn workspace_root_from_settings(state: &AppState) -> Result<PathBuf, String> {
    Settings::load_from(&state.settings_path)
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())
}

#[tauri::command]
fn get_active_executable(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let active = active_dir(&workspace_root, &project_key);
    Ok(find_active_executable(&active).map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn launch_active_build(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let active = active_dir(&workspace_root, &project_key);
    let exe = find_active_executable(&active)
        .ok_or_else(|| "no executable found in the active build".to_string())?;
    launch_executable(&exe).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_active_build_dir(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let active = active_dir(&workspace_root, &project_key);
    Ok(active
        .exists()
        .then(|| active.to_string_lossy().to_string()))
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
            });

            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            start_background_updates(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            login_start,
            logout,
            is_logged_in,
            list_projects,
            list_releases_for_project,
            toggle_favorite,
            get_workspace_root,
            set_workspace_root,
            sync_release_asset,
            check_release_asset,
            get_active_release,
            clear_project_cache,
            list_cached_assets,
            delete_cached_asset,
            get_active_executable,
            launch_active_build,
            get_active_build_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
