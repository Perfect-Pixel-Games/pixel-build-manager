# Version Footer Text — Design

## Purpose

The app currently has no in-UI indication of which build (release, pre-release,
or a local development build) is running, or what tag it was built from. Add a
small, always-visible text element showing that information.

## Scope

Covers:
1. A Rust-side pure formatting function mapping (channel, version) to a
   display label.
2. A Tauri command exposing that label to the frontend.
3. A frontend fetch + always-visible footer rendering it on every screen.

Explicitly out of scope:
- Any UI for triggering or displaying update checks (see
  `docs/superpowers/specs/2026-09-23-auto-update-design.md`, which is
  silent-by-design and unrelated to this display text).
- Any change to how CI computes the version tag or sets
  `PIXEL_BUILD_MANAGER_CHANNEL` (already implemented; see
  `.github/workflows/pixel-build-manager-build.yml` and
  `src-tauri/src/updater/channel.rs`).

## Display format

- Full release build → `"Release {version}"` (e.g. `"Release 0.4.0"`)
- Pre-release build → `"Pre-release {version}"` (e.g. `"Pre-release 0.3.2"`)
- Development build (channel unknown — local `cargo build`/`tauri dev`,
  outside CI) → `"Development"` (no version number)

## Backend

**New `src-tauri/src/version.rs`:**

```rust
pub fn format_version_label(channel: Option<Channel>, version: &str) -> String {
    match channel {
        Some(Channel::Release) => format!("Release {version}"),
        Some(Channel::Prerelease) => format!("Pre-release {version}"),
        None => "Development".to_string(),
    }
}
```

Pure function, unit-tested directly (mirrors the existing test style in
`updater/channel.rs`'s `from_env_value`) — no need to vary
`PIXEL_BUILD_MANAGER_CHANNEL` at test time.

**New Tauri command in `lib.rs`:**

```rust
#[tauri::command]
fn get_version_label(app: tauri::AppHandle) -> String {
    version::format_version_label(Channel::current(), &app.package_info().version.to_string())
}
```

Registered in the existing `generate_handler![...]` list.

`app.package_info().version` is sourced from `tauri.conf.json`'s `version`
field, embedded at compile time by `tauri-build`. CI's `package` job
overwrites that field with the computed tag (see "Write version into
tauri.conf.json" step in the build workflow) before running `tauri build`, so
this is exactly the release tag for CI-built artifacts. For a local build
outside CI, it stays whatever's checked into the repo — irrelevant here since
`Channel::current()` is `None` in that case, so the version number isn't shown
at all.

## Frontend

**New `src/api/version.ts`:**

```typescript
import { invoke } from "@tauri-apps/api/core";

export function getVersionLabel(): Promise<string> {
  return invoke("get_version_label");
}
```

**`src/App.tsx` changes:**

- Add `versionLabel` state, fetched once via `useEffect` on mount (independent
  of login state, so it's available before the user even logs in).
- Restructure the body of `App()` so the existing four branches (Loading,
  Login, WorkspaceSetup, main view) become the *content* of a shared outer
  layout, with a `<footer>` rendered alongside that content on every branch,
  showing `versionLabel` once loaded (renders nothing/empty before the fetch
  resolves — no loading state needed for a small footer string).

No new component file — the footer is small enough to inline directly in
`App.tsx`.

## Error handling

- If `get_version_label` fails to invoke (should not happen — it has no
  fallible internal logic), the footer simply stays empty; `console.error`
  the failure for diagnostics, consistent with other `invoke` call sites in
  `App.tsx`.

## Testing

- **Rust unit tests** in `version.rs` for all three cases: `Release`,
  `Prerelease`, `None` → correct label strings, including the exact version
  string interpolation.
- **Frontend test**: extend `App.test.tsx` to mock `invoke` returning a
  version label and assert it renders in the footer on at least one screen
  state (e.g. the Login screen, since that's reachable without further
  mocking of projects/workspace state).
</content>
