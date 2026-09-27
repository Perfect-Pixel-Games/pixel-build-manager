import { invoke } from "@tauri-apps/api/core";

export function getVersionLabel(): Promise<string> {
  return invoke("get_version_label");
}
