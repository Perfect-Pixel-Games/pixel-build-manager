mod auth;
mod github;
mod settings;

use auth::device_flow::DeviceFlowClient;
use auth::login::{perform_device_login, LoginStatus};
use auth::token_store::{KeyringTokenStore, TokenStore};
use github::client::{GithubClient, ReleaseSummary};
use serde::Serialize;
use settings::Settings;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

const GITHUB_CLIENT_ID: &str = "REPLACE_WITH_YOUR_GITHUB_OAUTH_APP_CLIENT_ID";

pub struct AppState {
    pub token_store: Arc<dyn TokenStore>,
    pub settings_path: PathBuf,
    /// Serializes settings.json read-modify-write cycles across commands so
    /// concurrent writes (e.g. rapid favorite toggles) can't clobber each other.
    pub settings_lock: Mutex<()>,
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
            });
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
            set_workspace_root
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
