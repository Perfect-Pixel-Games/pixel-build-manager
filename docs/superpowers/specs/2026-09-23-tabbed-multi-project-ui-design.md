# Tabbed Multi-Project UI — Design

## Purpose

Replace the current always-visible favorites/all-projects list and
single-active-build-per-project model with a tab-based UI: the user
explicitly binds the projects they care about as tabs, and per project can
sync/extract *multiple* build configs of a release at once instead of
swapping a single "active" build. Also adds app-wide light/dark/system
theming, and small login-screen and footer polish.

## Scope

Covers:
1. App-wide theme (light/dark/system), persisted.
2. Project binding as tabs, replacing `ProjectList`'s always-visible
   favorites/all-projects sections.
3. Per-project detail view: search, release list, per-config checkboxes,
   sync button, busy overlay, per-config open-folder/launch buttons.
4. Settings/extraction data model changes needed to support multiple
   simultaneously-synced configs per project.
5. Login screen: auto-opened verification link + copy button for the code.
6. Version footer placement/styling carried through the redesign.

Explicitly out of scope:
- Drag-to-reorder tabs (bind order is tab order).
- Auto-pruning of extracted builds or cached zips (still manual "Clear
  cache" only, per the original design's stance).
- Any settings-migration path for the breaking data-model change described
  below — this is a pre-1.0 internal tool; existing users re-select and
  re-tick after upgrading.
- Filename-based parsing of build configs into structured fields (still
  opaque asset names, per the original design).

## Data model & persistence

**`src-tauri/src/settings/mod.rs` changes:**

- `Settings` gains:
  - `bound_projects: Vec<String>` — project keys (`full_name`) currently
    bound to a tab, in bind order (bind order *is* tab order; no reordering
    UI).
  - `theme: Theme` — new enum `Light | Dark | System`, default `System`.
- `ProjectSettings` changes:
  - Remove `active_release_tag` / `active_asset_name` — there is no longer
    a single "active build" per project.
  - Add `selected_release_tag: Option<String>` — which release is
    highlighted/expanded in the release list. Purely a UI cursor (restores
    where the user was looking), not a statement about what's synced.
  - Add `ticked_configs: Vec<String>` — the set of build-config checkboxes
    the user has ticked for this project, persisted so it survives restarts.
- Unbinding a project (closing its tab) removes it from `bound_projects`
  but leaves its `ProjectSettings` entry, cache, and extracted builds alone
  — rebinding later restores `selected_release_tag`/`ticked_configs` as they
  were.
- This is a breaking shape change to `settings.json`. `Settings::load_from`
  already falls back to `Settings::default()` on any deserialization failure
  (see existing `unwrap_or_default()`), so upgrading just resets settings to
  defaults — no migration code, matching the project's existing "no
  auto-migration" stance elsewhere (e.g. no cache-pruning policy).

**Extraction layout change (`src-tauri/src/sync`):**

- Today: one `<root>\<org>-<repo>\active\` folder per project, atomically
  replaced on every sync.
- New: `<root>\<org>-<repo>\builds\<release_tag>\<config_name>\`, one folder
  per synced (release, config) pair. Multiple can exist simultaneously;
  nothing deletes old ones automatically (same manual-clear-only philosophy
  as the cache).
- Extraction is still atomic per folder (extract to a temp dir alongside the
  target, then rename over it on success) — same guarantee as today, just
  keyed differently.
- "Is (release, config) synced?" is derived from disk presence of its
  extraction folder (mirrors today's `listCachedAssets`/`cachedAssetIds`
  pattern in `App.tsx`/`ProjectDetail`), not stored as separate state.

**Backend commands affected** (exact signatures are an implementation
decision for the plan, not fixed here):
- Sync becomes "sync this release's ticked configs", extracting each
  missing one into its own folder rather than one "activate" call.
- A re-verify/re-sync command re-downloads/re-extracts a given (release,
  config) pair unconditionally (used when the sync button is clicked while
  already showing ✓).
- Open-folder and launch need a (release, config) argument instead of
  reading "the" active build.
- `toggle_favorite` is unaffected — favorites remain a project-level flag,
  just no longer surfaced outside the binding popup.

## Theming

- Three-way control (light / dark / system icons) in a corner near the
  footer. Persisted via the new `Settings.theme` field
  (`get_theme`/`set_theme` commands, mirroring the existing
  `get_workspace_root`/`set_workspace_root` pattern in `src/api/settings.ts`).
- Implemented as CSS custom properties switched by a `data-theme` attribute
  on the root element: `"light"`, `"dark"`, or absent (falls through to
  `@media (prefers-color-scheme: dark)` for `"system"`). Switching is
  instant, no reload.
- Default `System` until the user changes it.

## Tab bar & project binding

- Default state (no bound projects): tab bar shows only a `+` button.
- `+` opens a popup: a search box, then every project the user can access
  that is **not** already in `bound_projects`, each with its existing
  favorite star toggle (favorites have no UI outside this popup now — no
  more standalone always-visible favorites/all-projects list).
- Picking a project from the popup appends it to `bound_projects` and opens
  its tab.
- Each bound tab renders a close `×` (visible on hover, consistent with
  common tab-bar conventions) that unbinds it: removes it from
  `bound_projects` and closes the tab. Does not touch cache, extracted
  builds, or its `ProjectSettings` entry (see above) — it reappears in the
  `+` popup, and rebinding restores its prior selection/ticked state.
- `ProjectDetail` is rendered per the currently-focused tab, same
  component-per-project-key pattern as today (`key={selectedProject}`).

## Per-project detail view

Replaces `ReleaseList`'s single-build-type-dropdown-plus-flat-list UI.

- **Search box** filtering the release list below by tag/name
  (case-insensitive substring match).
- **Scrollable release list**, filling remaining vertical space. One row
  per **release only** — prereleases are excluded entirely, no toggle (this
  view is for "what am I picking to sync," not full release-history
  browsing). Each row shows tag/name and published date. Clicking a row
  selects it (updates `selected_release_tag`, expands the config
  checkboxes/sync controls for it).
- **Build-config checkboxes**, below the list: one per build config the
  *entire project* has ever produced across any release (union of asset
  names, same derivation `ReleaseList.tsx` already does for `buildTypes`) —
  not scoped to the selected release. A checkbox is disabled/grayed when the
  currently selected release has no matching asset for that config (can't
  tick something that would be a no-op for this release).
- **Sync button**, acting on (selected release × ticked configs):
  - Shows "Sync" whenever at least one ticked config isn't yet extracted
    for the selected release; clicking downloads/extracts only the missing
    ones (already-synced ticked configs are left alone).
  - Once every ticked config is synced for the selected release, becomes a
    ✓ button. Clicking it while showing ✓ re-verifies/re-downloads/re-
    extracts every ticked config unconditionally (repair path, equivalent to
    today's "Check").
- **Busy overlay**: while a sync is in flight, a centered throbber + status
  text (e.g. "Downloading win64_shipping… 63%") renders in a positioned
  overlay on top of the existing panel; the panel underneath is dimmed
  (reduced opacity/grayscale) and `pointer-events: none` rather than being
  replaced — search, the release list, checkboxes, and sync/folder/launch
  buttons are all inert until the operation finishes.
- **Per-config open-folder/launch buttons**, below the sync controls: one
  "Open Folder" + one launch button *per synced config of the selected
  release* (only configs actually extracted — not every ticked/available
  one). Each opens/launches that specific config's own extraction folder,
  independent of the others.

## Login screen

- Idle state unchanged: "Log in with GitHub" button.
- Awaiting-user state (device code received):
  - Automatically opens `verification_uri` in the system browser via the
    existing `@tauri-apps/plugin-opener` (`openPath`/equivalent URL-open
    call) as soon as the status arrives — no extra user click needed.
  - The user code renders in a monospaced box with an adjacent "Copy"
    button (clipboard write), so the user doesn't have to retype it.
  - A small fallback text link ("Open manually") remains below, in case the
    auto-open failed or the user closed the tab.
- Denied/expired/error states unchanged.

## Footer

- The existing app version label (`getVersionLabel`) stays, pinned to the
  bottom of the window and aligned bottom-right (not centered), styled
  through the same theme CSS variables as the rest of the UI so it isn't
  visually stale in dark mode.

## Error handling

- Sync/re-verify failures for one config in a batch don't abort the others
  — each (release, config) sync is independent, matching today's
  per-asset error isolation in `ProjectDetail.handleSelect`.
- Auto-opening the verification URL failing silently (e.g. no default
  browser) is why the manual fallback link always renders regardless of
  whether the auto-open call itself reported success — the login flow
  cannot depend on knowing that the browser opened correctly.
- Session-expiry handling (`SESSION_EXPIRED_ERROR` routing back to Login)
  is unchanged, but must now also handle the case where the user is mid-way
  through a bound-tab session; on re-login, previously bound tabs and their
  per-project settings are unaffected (all settings-side, not session-side).

## Testing

- **Rust unit tests**:
  - New `Settings` shape: `bound_projects` add/remove, `ticked_configs`
    persistence, `theme` round-trip — same style as existing
    `save_then_load_round_trips`/`set_favorite_creates_entry_if_missing`
    tests.
  - Extraction path keyed by `(release_tag, config_name)`: verify multiple
    configs can coexist on disk, and that re-verify/re-sync only touches the
    targeted (release, config) pair.
- **Frontend component tests**:
  - Tab bar: binding via the `+` popup appends a tab and persists it;
    closing a tab's `×` unbinds without clearing its underlying settings.
  - Checkbox disable-when-asset-missing logic for a given selected release.
  - Sync button tri-state: "Sync" (something missing) → syncing → ✓ (all
    ticked configs present) → re-verify on click while ✓.
  - Busy overlay: asserts underlying controls are disabled/non-interactive
    while a sync is in flight, without the panel being unmounted.
  - Login: verification link auto-open call fires on `awaiting_user`, copy
    button copies the code, fallback link always present.
  - Footer: extend existing `App.test.tsx` version-label assertion to check
    bottom-right placement/styling hook (e.g. a stable class/test-id) still
    renders under the new layout.
