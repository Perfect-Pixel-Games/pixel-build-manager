# In-App Auto-Update — Design

## Purpose

The original app design explicitly deferred "in-app auto-update mechanism for
the tool itself" as out of scope for v1. This spec covers that mechanism: the
packaged app checks its own GitHub repo for newer releases and updates itself
in place, without the user manually downloading and reinstalling.

## Scope

Covers:
1. Self-update checking, downloading, and installing for packaged builds of
   Pixel Build Manager itself (not the game builds it manages for other
   projects — that sync flow is unrelated and untouched).
2. Channel-aware update selection: a release build only ever updates to a
   newer release; a prerelease build only ever updates to a newer prerelease.
3. CI changes needed to produce signed, self-update-compatible artifacts.

Explicitly out of scope:
- Any update behavior for debug/dev builds (`tauri dev`). These never check,
  download, or restart.
- A settings toggle to disable auto-update. Not requested; can be added later
  if needed.
- Any UI for update status. The flow is silent by design (see below).

## Channel identity (build-time)

CI already knows its channel per branch (`main` → full release, `dev` →
prerelease — see the existing versioning scheme in
`docs/superpowers/specs/2026-09-16-pixel-build-manager-design.md`). Rather
than inferring channel from the version number at runtime, CI passes it in
explicitly:

- `package` job sets `PIXEL_BUILD_MANAGER_CHANNEL=release` when building from
  `main`, `PIXEL_BUILD_MANAGER_CHANNEL=prerelease` when building from `dev`,
  as an env var for the `npm run tauri build` step.
- The Rust binary reads this at compile time via `option_env!()` into a
  `Channel::Release | Channel::Prerelease | Channel::Unknown` constant.
- A binary with `Channel::Unknown` (the env var wasn't set — should never
  happen for a CI-built artifact, but is the safe default for e.g. a raw
  local `cargo build --release`) skips update checks entirely and logs a
  warning once at startup.
- Debug builds (`cfg!(debug_assertions)` is true) skip the entire updater
  subsystem regardless of channel — no check, no download, no restart. This
  reuses the same debug/release distinction already relied on elsewhere in
  `main.rs`.

## Update artifacts & CI changes

**One-time setup (outside CI, done once by a maintainer):**
- Generate a minisign keypair via `npm run tauri signer generate`.
- Public key goes into `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).
- Private key + its password become GitHub Actions repo secrets
  (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).

**`tauri.conf.json` changes:**
- Enable `bundle.createUpdaterArtifacts` so `tauri build` produces a signed
  `.nsis.zip` + `.sig` alongside the normal NSIS/MSI installer.
- Add `plugins.updater.pubkey`. No static `endpoints` array here — the
  endpoint is resolved dynamically per channel at runtime (see below), not
  fixed in config.

**`.github/workflows/pixel-build-manager-build.yml` changes:**
- `package` job: pass `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as env vars into the `npm run tauri
  build` step, and set `PIXEL_BUILD_MANAGER_CHANNEL` based on
  `github.ref` (`main` → `release`, `dev` → `prerelease`).
- `release` job: after the version tag/release are computed, add a step that
  reads the generated `.sig` file contents and writes a `latest.json`
  manifest (`version`, `pub_date`, `platforms["windows-x86_64"].{url,
  signature}`), then includes it in the assets uploaded alongside the
  installer. This replaces what `tauri-action` would auto-generate — this
  workflow calls `tauri build` directly rather than using that action, so the
  manifest step is written by hand.

This means every release (both channels) publishes: the installer, the
`.nsis.zip` + `.sig` updater artifact, and a `latest.json` manifest.

## Runtime update flow

New `src-tauri/src/updater/` module, registered via
`tauri_plugin_updater::Builder::new().build()` in `lib.rs`. In the app's
`.setup()` hook, if `!cfg!(debug_assertions)` and channel is known (not
`Unknown`): spawn a background task that runs an immediate check, then
re-checks every **15 minutes** (`tokio::time::interval`) for the lifetime of
the app. This covers both "check on startup" and "a release publishes while
the app is already open."

Each check cycle:
1. **Resolve the endpoint URL** for the baked-in channel:
   - **release channel** →
     `https://github.com/Perfect-Pixel-Games/pixel-build-manager/releases/latest/download/latest.json`.
     GitHub's `latest` release alias natively means "latest non-prerelease,
     non-draft release," so this gives channel filtering for free with no
     extra API call.
   - **prerelease channel** → one unauthenticated GitHub API call
     (`GET /repos/Perfect-Pixel-Games/pixel-build-manager/releases`,
     already-sorted newest-first) to find the first entry with
     `prerelease: true`, then build the endpoint from that release's tag:
     `.../releases/download/<tag>/latest.json`.
   - The repo is public, so no auth token is required for either lookup —
     this works even before the user has logged into GitHub in-app.
2. Build an `Updater` for that one endpoint
   (`app.updater_builder().endpoints(vec![endpoint])?.build()?`) and call
   `.check().await`. The plugin does its own semver comparison against the
   currently-running version, so it naturally no-ops if already current —
   no manual "is this actually newer" guard needed on top.
3. If an update is found, check `AppState.active_operations` (the existing
   in-flight sync/cache-clear tracking set, already used to prevent a sync
   and a cache-clear from racing each other). If non-empty, skip installing
   this cycle — it will be retried automatically at the next 15-minute tick,
   once operations have finished.
4. If clear: `update.download_and_install(...).await`, then `app.restart()`.
   This is silent and immediate — no confirmation prompt, no pre-restart
   notice — matching the explicit ask for automatic restart-and-update.

No frontend changes are required. The entire flow is backend-only and
invisible to the user until the app relaunches on the new version.

## Error handling

- **Network failure / GitHub rate limit during check:** log and continue;
  retried at the next 15-minute interval. Never blocks normal app use.
- **Unknown channel:** update checks are skipped entirely for the process
  lifetime; logged once at startup.
- **Download or signature verification failure** (handled inside the
  updater plugin — tampered or corrupt artifacts are rejected): log and
  retry at the next interval, rather than retrying immediately.
- **Busy (in-flight sync/cache-clear):** skip only the install step for this
  cycle; the check itself still runs each interval regardless.

## Testing approach

- **Rust unit tests** for the channel → endpoint resolution logic (the one
  piece of genuinely new logic here): given a channel and a mocked release
  list (via `wiremock`, matching the existing test pattern used for the
  GitHub client), resolves the correct endpoint URL for both channels,
  including the prerelease list-and-filter step.
- The actual check/download/install/restart cycle end-to-end is not
  practically testable in CI (requires a live signed release and an actual
  install), so it's manual/integration verification only — consistent with
  this repo's existing precedent for GitHub API integration behavior
  (see the original design's Testing approach section).
- No frontend tests needed — no UI surface is added.

## Open questions / follow-ups for later

- No settings toggle to disable auto-update exists yet; not requested for
  this iteration.
