# Latest Release/Prerelease Tracking — Design

## Purpose

Today, syncing a build config means the user manually picks one release from
the list each time a new one ships. This spec adds an auto-tracking mode per
project: pin to "whatever is currently the latest release" or "whatever is
currently the latest prerelease," and the manager keeps that pinned build
current on disk automatically as new releases ship, without the user
revisiting the tab.

## Scope

Covers:
1. Two new pinned entries at the top of a project's release list — "Latest
   release" and "Latest prerelease" — selectable exactly like a concrete
   release, mutually exclusive with picking a concrete release.
2. A background poll that detects when a bound project's latest
   release/prerelease has changed and re-syncs the ticked build configs for
   it automatically.
3. An immediate check triggered the moment the user switches a project into
   one of these modes (doesn't wait for the next poll).
4. A dedicated on-disk location for latest-tracked builds that always holds
   at most one version per channel, so it never accumulates a cache of old
   "latest" builds the way manually-synced releases can.

Explicitly out of scope:
- Any change to manual-mode syncing, browsing, or storage (`builds/<tag>/`,
  the shared per-project `cache/`). That path is untouched.
- Desktop notifications when a new latest build finishes syncing. The
  existing in-app progress UI is the only feedback surface for this
  iteration.
- Configurable poll interval. Fixed at 5 minutes for now.

## Behavior

Selecting "Latest release" or "Latest prerelease" replaces manual selection
for that project: the pinned row shows selected, and the concrete-release
rows below are no longer the active target. Switching back to a concrete
release returns to today's exact behavior, including restoring whichever tag
was last manually selected (that state is preserved, not lost, while a latest
mode is active). Ticked build configs are unaffected by mode — they're
already release-agnostic name templates (`configTemplate`), so the same
ticked set applies to whichever concrete release the active mode currently
resolves to.

While a latest mode is active, the app:
- Checks in the background every 5 minutes whether a newer release exists in
  that channel for every *bound* project with a latest mode selected.
- Checks immediately when the mode is switched on.
- If the resolved release differs from what's currently synced for that
  channel, re-syncs the ticked configs and replaces what was there.

## Settings changes (`src-tauri/src/settings/mod.rs`)

New enum, serialized the same flat lowercase way `Theme` already is:

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    #[default]
    Manual,
    LatestRelease,
    LatestPrerelease,
}
```

`ProjectSettings` gains `#[serde(default)] pub sync_mode: SyncMode`. Existing
`selected_release_tag` and `ticked_configs` fields are unchanged and keep
their current meaning regardless of `sync_mode` — `selected_release_tag` is
simply ignored while a latest mode is active, and restored to relevance the
moment the project goes back to `Manual`.

## Storage layout (`src-tauri/src/sync/cache.rs`)

New top-level directory per project, sibling to `cache/` and `builds/`:

```
<project_dir>/latest/<release|prerelease>/<config_name>/   (extracted build)
<project_dir>/latest/<release|prerelease>/.synced_tag       (marker file)
```

New functions, parallel to the existing `build_config_dir`/`cache_dir`
helpers and following the same sanitization rules:
- `latest_channel_dir(workspace_root, project_key, channel: &str) -> PathBuf`
  — `channel` is the literal `"release"` or `"prerelease"`.
- `latest_config_dir(workspace_root, project_key, channel, config_name) -> PathBuf`
- `read_latest_synced_tag(...) -> Option<String>` /
  `write_latest_synced_tag(...)` — read/write the marker file.
- `list_synced_latest_configs(workspace_root, project_key, channel) -> Vec<String>`
  — lists extracted config subdirectories, mirroring `list_synced_configs`
  (the marker file is a regular file, so the existing `is_dir()` filter
  already excludes it with no extra logic needed).

**No-buildup guarantee:** every time a new tag is detected for a channel, the
entire `latest/<channel>/` directory is deleted and re-extracted from
scratch — never additive. The raw downloaded archive is deleted immediately
after a successful extraction rather than being kept in the shared
`cache/` (which stays exactly as it behaves today, used only by manual-mode
syncs).

## Orchestration (new `src-tauri/src/sync/latest.rs`)

**Resolving "latest":** given the project's already-fetched `Vec<ReleaseSummary>`
(same data the manual browser uses), sort a copy by `published_at` descending
(don't rely on GitHub's list ordering), then take the first entry with
`prerelease == false` (release channel) or `prerelease == true` (prerelease
channel). No match (e.g. a project with no prerelease yet) means that
channel is a no-op until one exists.

**Sync-or-skip:** compare the resolved release's tag against
`read_latest_synced_tag`. Equal → nothing to do. Different (or no marker
yet) → if `ticked_configs` is non-empty, clear the channel dir, download and
extract each ticked config's matching asset from that release (reusing
`sync::orchestrator`'s existing download/extract building blocks), then
write the new marker. Empty ticked-configs list → skip entirely, same as
today's manual Sync button being disabled with nothing ticked.

**Concurrency:** wraps the same `begin_operation`/`end_operation` guard on
`AppState.active_operations` that manual sync and cache-clear already use.
A background check that finds the project busy simply skips this cycle
(logged, not surfaced as an error) and is retried on the next poll or the
next immediate-check trigger — mirroring how the self-updater already defers
to in-flight operations rather than racing them.

## Triggering

- **Background loop:** a new `start_background_latest_sync(app: AppHandle)`,
  structurally parallel to `updater::start_background_updates`, spawned from
  `.setup()`. Every 5 minutes it iterates `bound_projects`, loads each
  project's `sync_mode`, and runs the check above for any that aren't
  `Manual`. Unlike the self-updater, this loop always runs (no debug-build or
  channel gate) — it has nothing to do with the app's own update channel.
- **Immediate check on toggle:** the new `set_sync_mode` command persists the
  mode, then — if the new mode isn't `Manual` — spawns the same check
  in-process (`tauri::async_runtime::spawn`, matching the existing
  `login_start` fire-and-forget pattern) so the user doesn't wait for the
  next poll after flipping the toggle.

## New Tauri commands (`src-tauri/src/lib.rs`)

- `get_sync_mode(project_key) -> SyncMode`
- `set_sync_mode(project_key, mode: SyncMode)` — persists + triggers
  immediate check as above.
- `list_synced_latest_configs(project_key, channel) -> Vec<String>`
- `get_latest_build_dir(project_key, channel, config_name) -> Option<String>`
- `get_latest_build_executable(project_key, channel, config_name) -> Option<String>`
- `launch_latest_build(project_key, channel, config_name)`

These sit alongside the existing tag-keyed commands (`list_synced_configs`,
`get_build_dir`, `get_build_executable`, `launch_build`) rather than
generalizing them to accept a tag-or-channel selector. That keeps the manual
path's working commands completely untouched, and each command stays
single-purpose — consistent with `get_build_dir` and `get_build_executable`
already being two separate commands today rather than one parameterized one.

## Events & frontend integration

**The wrinkle:** background- and immediate-check-triggered syncs run
entirely on the Rust side, not through the frontend's `syncConfigs()` call.
Today's `useSync` hook ignores a `sync-progress` event unless its own state
is already `"syncing"` (it was put there by an in-flight `syncConfigs`
call) — so a sync nobody in the frontend "started" would currently produce
no visible feedback at all.

To fix this without disturbing manual-mode's existing hook, add two new
Rust-emitted events scoped to latest-mode syncs:
- `latest-sync-progress` — `{ project_key, channel, downloaded, total }`
  (same shape as today's `sync-progress`, plus `channel`).
- `latest-sync-finished` — `{ project_key, channel, synced: bool, tag:
  Option<String>, error: Option<String> }`. Emitted whether the cycle ended
  in "synced a new version," "already current, no-op," or "failed" — the
  frontend needs all three to know when to stop showing a busy state and
  whether to refresh.

And a new frontend hook, `useLatestSync(projectKey)`, independent of
`useSync`, with its own `idle | checking | syncing | done | error` state
driven by those two events. `ProjectDetail` shows whichever of the two
hooks' states is active in the same existing `BusyOverlay`/`SyncStatus`
components — no new visual components, per the earlier decision to reuse
the existing sync UI as-is.

## UI changes

- **`BuildBrowser`:** two pinned rows above the scrollable release list,
  `aria-pressed` when `syncMode` matches that channel, calling a new
  `onSelectLatest(channel)` prop (distinct from `onSelectRelease`, since the
  backend call and persisted state differ). The build-config checkboxes'
  "available for the current selection" check resolves against the
  currently-latest release/prerelease (same sort-and-pick logic as the
  backend, computed client-side from the already-loaded `releases` prop)
  instead of `selectedReleaseTag` when a latest mode is active.
- **`ProjectDetail` (`App.tsx`):** loads/persists `sync_mode` via the new
  API; when it isn't `Manual`, sources `syncedConfigs` from
  `listSyncedLatestConfigs` instead of `listSyncedConfigs`, and renders
  `SyncedBuildControls` with the latest-channel variant of its props.
- **`SyncedBuildControls`:** gains an alternate `latestChannel` prop,
  mutually exclusive with the existing `releaseTag` prop; when set, it calls
  `getLatestBuildDir`/`getLatestBuildExecutable`/`launchLatestBuild` instead
  of their tag-keyed equivalents.

## Error handling

- **Network failure / GitHub rate limit during a check:** log and continue;
  retried at the next 5-minute poll (or next immediate-check trigger).
  Reported to the frontend via `latest-sync-finished`'s `error` field if the
  check was already visibly "syncing" to the user; a poll that fails before
  ever starting a download just logs silently, matching the self-updater's
  precedent.
- **Busy (in-flight manual sync/cache-clear):** skip this cycle entirely,
  no error surfaced; retried next cycle.
- **No prerelease exists yet:** "Latest prerelease" mode is selectable and
  persists, but resolves to nothing and never syncs until a prerelease is
  published — no error, no special UI state.

## Testing approach

- **Rust unit tests:** the sort-and-pick "resolve latest" logic (release vs.
  prerelease, ties, missing `published_at`), the marker read/write and
  clear-and-resync behavior in the new `latest.rs` (via `wiremock`, matching
  the existing pattern in `sync/orchestrator.rs`), and the background loop's
  skip-when-busy / skip-when-no-ticked-configs paths.
- **Frontend tests:** `BuildBrowser`'s two pinned rows (selection,
  `aria-pressed`, mutual exclusivity with concrete release rows) and
  `useLatestSync`'s state transitions across its two new events, following
  the existing per-component `*.test.tsx` / `*.test.ts` pattern.

## Open questions / follow-ups for later

- No configurable poll interval or a way to disable background polling
  independent of switching back to Manual — not requested for this
  iteration.
- No desktop/OS-level notification when a latest build finishes syncing in
  the background — in-app UI only, for now.
