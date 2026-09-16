import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
