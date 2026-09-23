import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

export function logout(): Promise<void> {
  return invoke("logout");
}

export function isLoggedIn(): Promise<boolean> {
  return invoke("is_logged_in");
}

export function onLoginStatus(callback: (status: LoginStatus) => void) {
  return listen<LoginStatus>("login-status", (event) => callback(event.payload));
}
