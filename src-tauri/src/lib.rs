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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let settings_path = settings_path_for(app.handle());
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
