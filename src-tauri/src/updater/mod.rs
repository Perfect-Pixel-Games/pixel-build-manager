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
    let endpoint_url = endpoint_url.parse().map_err(|e| format!("{e}"))?;

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
        .download_and_install(
            |_chunk_length: usize, _content_length: Option<u64>| {},
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    app.restart();
}
