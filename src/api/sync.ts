import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Release, ReleaseAsset } from "./projects";

export type SyncProgress = {
  project_key: string;
  downloaded: number;
  total: number;
};

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
  });
}

export function clearProjectCache(projectKey: string): Promise<void> {
  return invoke("clear_project_cache", { projectKey });
}

export function getSelectedRelease(projectKey: string): Promise<string | null> {
  return invoke("get_selected_release", { projectKey });
}

export function setSelectedRelease(projectKey: string, releaseTag: string): Promise<void> {
  return invoke("set_selected_release", { projectKey, releaseTag });
}

export function getTickedConfigs(projectKey: string): Promise<string[]> {
  return invoke("get_ticked_configs", { projectKey });
}

export function setTickedConfigs(projectKey: string, configs: string[]): Promise<void> {
  return invoke("set_ticked_configs", { projectKey, configs });
}

export function listSyncedConfigs(projectKey: string, releaseTag: string): Promise<string[]> {
  return invoke("list_synced_configs", { projectKey, releaseTag });
}

export function getBuildExecutable(
  projectKey: string,
  releaseTag: string,
  configName: string,
): Promise<string | null> {
  return invoke("get_build_executable", { projectKey, releaseTag, configName });
}

export function launchBuild(
  projectKey: string,
  releaseTag: string,
  configName: string,
): Promise<void> {
  return invoke("launch_build", { projectKey, releaseTag, configName });
}

export function getBuildDir(
  projectKey: string,
  releaseTag: string,
  configName: string,
): Promise<string | null> {
  return invoke("get_build_dir", { projectKey, releaseTag, configName });
}

export function onSyncProgress(callback: (progress: SyncProgress) => void) {
  return listen<SyncProgress>("sync-progress", (event) => callback(event.payload));
}

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
