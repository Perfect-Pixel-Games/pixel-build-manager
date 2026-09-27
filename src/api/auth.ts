import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";

// Matches SESSION_EXPIRED in src-tauri/src/auth/session.rs -- returned by
// GitHub-backed commands when the stored refresh token can no longer be
// redeemed, so the caller knows to route back to the Login screen instead
// of treating it like any other failed request.
export const SESSION_EXPIRED_ERROR = "session_expired";

export type LoginStatus =
  | { status: "awaiting_user"; user_code: string; verification_uri: string }
  | { status: "success" }
  | { status: "denied" }
  | { status: "expired" }
  | { status: "error"; error: string };

export function loginStart(): Promise<void> {
  return invoke("login_start");
}

/** Logs out, removing every project bound from the GitHub account (tab,
 * settings, cache and builds). Projects bound by URL are kept. Resolves to
 * the removed project keys. */
export function logout(): Promise<string[]> {
  return invoke("logout");
}

/** The bound projects that logging out would remove. */
export function listAccountBoundProjects(): Promise<string[]> {
  return invoke("list_account_bound_projects");
}

/** Asks the user to confirm a logout that will remove `projectKeys`. */
export function confirmLogout(projectKeys: string[]): Promise<boolean> {
  const count = projectKeys.length === 1 ? "1 project" : `${projectKeys.length} projects`;
  return ask(
    `Logging out will remove ${count} bound from your GitHub account, and delete ` +
      `their downloaded builds:\n\n${projectKeys.join("\n")}\n\nProjects bound by URL are kept.`,
    { title: "Log out", kind: "warning", okLabel: "Log out", cancelLabel: "Cancel" },
  );
}

export function isLoggedIn(): Promise<boolean> {
  return invoke("is_logged_in");
}

export function onLoginStatus(callback: (status: LoginStatus) => void) {
  return listen<LoginStatus>("login-status", (event) => callback(event.payload));
}
