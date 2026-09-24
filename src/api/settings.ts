import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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

export function unbindProject(fullName: string): Promise<void> {
  return invoke("unbind_project", { fullName });
}
