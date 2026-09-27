use crate::github::client::GithubClient;
use crate::settings::{Settings, SyncMode};
use crate::sync::cache::LatestChannel;
use crate::sync::latest::{check_and_sync_latest, LatestSyncOutcome};
use crate::{begin_operation, end_operation, github_client_or_anonymous, AppState};
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
    fn from_outcome(
        project_key: &str,
        channel: LatestChannel,
        outcome: &Result<LatestSyncOutcome, String>,
    ) -> Self {
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
    let project = settings
        .projects
        .get(project_key)
        .cloned()
        .unwrap_or_default();

    let Some(channel) = channel_for_mode(project.sync_mode) else {
        return Ok(());
    };
    let (owner, repo) = project_key
        .split_once('/')
        .ok_or_else(|| format!("invalid project_key: {project_key}"))?;

    begin_operation(&state, project_key)?;

    let client_result = github_client_or_anonymous(&state).await;
    let outcome: Result<LatestSyncOutcome, String> = match client_result {
        Ok(client) => {
            run_check(
                app,
                &client,
                project_key,
                owner,
                repo,
                channel,
                &workspace_root,
                &project.ticked_configs,
            )
            .await
        }
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
