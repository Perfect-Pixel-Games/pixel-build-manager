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
