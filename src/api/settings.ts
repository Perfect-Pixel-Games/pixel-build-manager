import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export function getWorkspaceRoot(): Promise<string | null> {
  return invoke("get_workspace_root");
}

export function setWorkspaceRoot(root: string): Promise<void> {
  return invoke("set_workspace_root", { root });
}

export function pickFolder(): Promise<string | null> {
  return open({ directory: true, multiple: false }) as Promise<string | null>;
}
