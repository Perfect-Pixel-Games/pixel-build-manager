# Pixel Build Manager — Design

## Purpose

A desktop tool that lets a developer authenticate with GitHub and quickly download,
cache, and switch between build releases of any project they have access to (repo
or org). Inspired by Unreal Engine's UnSync. Primary use case: a developer on a
team wants fast, low-friction access to the specific build/version their team is
currently working with, without manually hunting through GitHub Releases pages
and manually managing zip files.

## Scope

This spec covers:
1. The desktop app itself (Tauri + React/TypeScript frontend, Rust backend).
2. The CI/CD workflows and branch strategy for building/releasing this tool,
   modeled on `E:\GameDev\last-beacon\.github\workflows\last-beacon-build.yml`.

Out of scope for v1 (explicitly deferred, not designed here):
- macOS/Linux builds (target platforms start Windows-only; architecture leaves
  room to add them later).
- Auto-pruning/retention policy for cached zips (manual "Clear cache" only).
- In-app auto-update mechanism for the tool itself.
- Filename-based parsing of release assets into structured platform/config fields.

## Architecture

- **Tauri v2** app: Rust backend owns all logic (GitHub auth, API calls,
  downloading, caching, extraction, settings persistence). The **React +
  TypeScript** frontend is purely presentational — it calls Rust via Tauri
  `invoke` commands and listens for emitted progress events (download %,
  extraction phase). No GitHub API calls, tokens, or filesystem logic live in
  the frontend.
- **GitHub API access**: `octocrab` where it cleanly covers the need (orgs,
  repos, releases); raw `reqwest` for anything octocrab doesn't expose well
  (device flow polling, binary asset downloads with progress).
- **Token storage**: OS keychain via the `keyring` crate (Windows Credential
  Manager / macOS Keychain / Linux Secret Service). Token never persisted in
  plaintext and never exposed to the webview/JS context.
- **Local settings** (root workspace folder path, favorited projects,
  last-synced release+asset per project): a JSON file in the OS app-data
  directory, read/written from Rust.

## Authentication

- **GitHub OAuth Device Flow.** Requires a GitHub OAuth App registered under
  the user's account/org to obtain a Client ID (device flow needs no client
  secret). Scopes: `repo` (private repo + release access) and `read:org`
  (list orgs/org membership).
- Flow: user clicks "Login with GitHub" → backend requests a device code →
  UI shows the user code and a link to `github.com/login/device` → backend
  polls the token endpoint until the user completes the grant → token is
  stored in the OS keychain.
- Auth-expired/revoked tokens surface as a re-login prompt rather than a
  generic error.

## Project browsing

- After login, the backend fetches the user's orgs plus their personal
  account, then repos across those (paginated as needed).
- UI shows two sections: **Favorites** (pinned projects, persisted in local
  settings) at the top, and the **full list** of every accessible org/repo
  below it. Any repo can be favorited/unfavorited from either section.
- Selecting a project fetches its releases: name, tag, published date,
  prerelease flag, and its list of assets.

## Release sync workflow

- On first run (or via settings), the user picks a **root workspace folder**
  (e.g. `D:\Builds\`). The backend creates `<root>\<org>-<repo>\cache\` and
  `<root>\<org>-<repo>\active\` on first sync for that project.
- Each release's assets are listed as flat, individually-syncable entries —
  no filename parsing (e.g. `LastBeacon-windows-x64-shipping.zip` and
  `LastBeacon-windows-x64-test.zip` both appear as separate sync targets
  under the same release).
- **Sync** is a single action per asset:
  1. Download the asset into `cache\` if not already present there (cache
     keyed by asset ID/URL, so re-syncing an already-cached asset skips the
     download). Emits download progress events (bytes / total) for the UI.
  2. Verify downloaded file size against the GitHub asset's reported size
     before extracting; on mismatch, delete the cached file and surface a
     "download failed, retry" error instead of extracting a corrupt file.
  3. Extract into a temp folder alongside `active\`, then atomically
     swap/rename over `active\` on success. This guarantees `active\` is
     never left half-unzipped if extraction fails partway.
- The app tracks, per project, which release+asset is currently active and
  displays it clearly (e.g. "Active: v0.2.14 · shipping"). Switching to a
  different asset (same or different release) always replaces `active\`;
  re-syncing the already-active asset is a no-op unless the user explicitly
  forces a re-extract (e.g. to repair a manually-modified active folder).
- Per project, only one unzipped/active release-asset exists at a time —
  this is enforced by the fact that `active\` is always fully replaced on
  sync, never merged or added to.
- Cache is not auto-pruned in v1. A manual "Clear cache" button per project,
  plus a "Clear all caches" in settings, covers disk cleanup. Cache size is
  shown per project so it isn't a silent black hole.

## Error handling

- Network/API failures (rate limit, auth expired, no internet) surface as
  inline errors with a retry action; auth-expired specifically triggers a
  re-login prompt.
- Corrupt/incomplete downloads are caught by the size-verification step
  above and never reach extraction.
- Extraction failures never corrupt an existing active build, per the
  atomic-swap approach above.

## Testing approach

- **Rust**: unit tests for the cache-key logic, the download/verify/extract
  state machine, and settings (de)serialization — all pure logic, no live
  GitHub connection required.
- **GitHub API interactions**: integration tests behind a feature flag / run
  manually against a real test repo, not part of normal CI (they need a real
  token). CI runs `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test` (unit only) — mirrors last-beacon's `validate` job pattern.
- **Frontend**: component-level tests for the sync state machine UI
  (idle/downloading/extracting/active/error) using mocked Tauri `invoke`
  responses.

## Branch strategy

Same git-flow variant as last-beacon:
- `feature/*` → PR into `dev`.
- `dev` → PR into `main`.
- `hotfix/*` → allowed directly into `main`.
- A `source-branch-policy` gate job (ported from last-beacon, branch-name
  logic only, not project-specific) enforces this on pull requests.

Repo currently has zero commits. Order of operations: scaffold the Tauri +
React app, commit to `main`, branch `dev` off it, then add the workflow
file. Branch protection rules requiring these CI checks must be configured
manually afterward in GitHub's UI — not something a workflow file can set.

## CI/CD workflows

Modeled on `last-beacon-build.yml`, using the same self-hosted Windows
runners (`[self-hosted, Windows]`) last-beacon uses. Triggers: `push` and
`pull_request` on `main`/`dev`, plus `workflow_dispatch`.

**Versioning**: same `0.MINOR.PATCH` scheme as last-beacon — pushes to `dev`
bump **patch** (published as a prerelease), pushes to `main` bump **minor**
and reset patch to 0 (published as a full release). Tags are plain version
strings, auto-created and pushed by `github-actions[bot]`.

**Necessary deviation from last-beacon's job structure**: Tauri needs the
version baked into `src-tauri/tauri.conf.json` *before* `tauri build` runs,
since it's embedded in the installer's metadata and filename (e.g.
`pixel-build-manager_0.3.0_x64-setup.exe`). last-beacon computes its version
*after* packaging, which doesn't work here. Jobs:

1. `source-branch-policy` — unchanged, PR-time gate.
2. `validate` — `cargo fmt --check`, `cargo clippy -D warnings`,
   `cargo test`, `cargo build` for `src-tauri`, plus `npm ci` and frontend
   lint/typecheck/build.
3. `compute-version` *(push to `dev`/`main` only)* — inspects existing tags,
   computes the next version per the branch rule, outputs it for downstream
   jobs.
4. `package` *(needs `compute-version`)* — writes the version into
   `tauri.conf.json`, builds the frontend, runs `tauri build`, uploads the
   installer as an artifact. Structured as a matrix with a single
   `windows-x64` entry today, so `macos`/`linux` can be added later as
   additional matrix entries rather than a rewrite.
5. `release` *(needs `package`)* — tags/pushes the tag, publishes a
   prerelease (from `dev`) or full release (from `main`) with the installer
   attached. Mirrors last-beacon's `dev-release`/`main-release` split but as
   one parameterized job, since the logic is identical apart from the
   prerelease flag and version-bump type.

No secrets beyond the implicit default `GITHUB_TOKEN` are required (same as
last-beacon), with `contents: write` permission in the release job.

## Open questions / follow-ups for later

- Whether to register the GitHub OAuth App under the user's personal account
  or the "Pixel Perfect" org (affects who can manage/rotate the Client ID).
- Exact UI layout/component library choice within React — left as an
  implementation detail for the planning/build phase.
