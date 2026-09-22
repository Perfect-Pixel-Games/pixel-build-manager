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
  });
}

export function checkReleaseAsset(projectKey: string, asset: ReleaseAsset): Promise<void> {
  return invoke("check_release_asset", {
    projectKey,
    assetId: asset.id,
    assetName: asset.name,
    assetSize: asset.size,
  });
}

export function getActiveRelease(projectKey: string): Promise<ActiveRelease> {
  return invoke("get_active_release", { projectKey });
}

export function clearProjectCache(projectKey: string): Promise<void> {
  return invoke("clear_project_cache", { projectKey });
}

export function listCachedAssets(projectKey: string): Promise<number[]> {
  return invoke("list_cached_assets", { projectKey });
}

export function deleteCachedAsset(
  projectKey: string,
  assetId: number,
  assetName: string,
): Promise<void> {
  return invoke("delete_cached_asset", { projectKey, assetId, assetName });
}

export function getActiveExecutable(projectKey: string): Promise<string | null> {
  return invoke("get_active_executable", { projectKey });
}

export function launchActiveBuild(projectKey: string): Promise<void> {
  return invoke("launch_active_build", { projectKey });
}

export function onSyncProgress(callback: (progress: SyncProgress) => void) {
  return listen<SyncProgress>("sync-progress", (event) => callback(event.payload));
}
