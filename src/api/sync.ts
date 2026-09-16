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
