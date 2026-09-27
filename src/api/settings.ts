import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Project } from "./projects";

export type Theme = "light" | "dark" | "system";

export function getWorkspaceRoot(): Promise<string | null> {
  return invoke("get_workspace_root");
}

export function setWorkspaceRoot(root: string): Promise<void> {
  return invoke("set_workspace_root", { root });
}

export function pickFolder(): Promise<string | null> {
  return open({ directory: true, multiple: false }) as Promise<string | null>;
}

export function getTheme(): Promise<Theme> {
  return invoke("get_theme");
}

export function setTheme(theme: Theme): Promise<void> {
  return invoke("set_theme", { theme });
}

export function listBoundProjects(): Promise<string[]> {
  return invoke("list_bound_projects");
}

export function bindProject(fullName: string): Promise<void> {
  return invoke("bind_project", { fullName });
}

/** Binds any GitHub repo with releases by URL, including public repos the
 * user isn't a member of. Rejects with a user-facing message if the URL is
 * invalid, the repo can't be found, or it has no releases. */
export function bindProjectByUrl(url: string): Promise<Project> {
  return invoke("bind_project_by_url", { url });
}

export function unbindProject(fullName: string): Promise<void> {
  return invoke("unbind_project", { fullName });
}
