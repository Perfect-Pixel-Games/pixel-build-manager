# Version Footer Text Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show an always-visible footer text in the app reporting which build is running: `"Release {tag}"`, `"Pre-release {tag}"`, or `"Development"`.

**Architecture:** A pure Rust formatting function (`format_version_label`) combines the existing build-time `Channel` (already baked in via `PIXEL_BUILD_MANAGER_CHANNEL`) with the app's compiled-in version string (`app.package_info().version`, sourced from `tauri.conf.json`, which CI overwrites with the release tag before building). A new Tauri command exposes the formatted label; the frontend fetches it once on mount and renders it in a footer that sits outside `App()`'s existing Loading/Login/WorkspaceSetup/main-view branches, so it's visible on every screen.

**Tech Stack:** Rust (Tauri v2 backend), TypeScript/React (frontend), `cargo test`, `vitest`.

Reference spec: `docs/superpowers/specs/2026-09-23-version-footer-design.md`

---

### Task 1: Backend — pure version label formatting

**Files:**
- Create: `src-tauri/src/version.rs`
- Modify: `src-tauri/src/lib.rs:1-5` (add `mod version;`)

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/version.rs` with just the test module (the function it calls doesn't exist yet, so this fails to compile):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::channel::Channel;

    #[test]
    fn formats_release_label() {
        assert_eq!(
            format_version_label(Some(Channel::Release), "0.4.0"),
            "Release 0.4.0"
        );
    }

    #[test]
    fn formats_prerelease_label() {
        assert_eq!(
            format_version_label(Some(Channel::Prerelease), "0.3.2"),
            "Pre-release 0.3.2"
        );
    }

    #[test]
    fn formats_development_label_when_channel_unknown() {
        assert_eq!(format_version_label(None, "0.1.0"), "Development");
    }
}
```

- [ ] **Step 2: Register the module so it compiles into the crate**

In `src-tauri/src/lib.rs`, add `version` to the existing `mod` block at the top of the file:

```rust
mod auth;
mod github;
mod settings;
mod sync;
mod updater;
mod version;
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml version::`
Expected: FAIL — compile error, `cannot find function `format_version_label` in this scope`.

- [ ] **Step 4: Write the minimal implementation**

At the top of `src-tauri/src/version.rs`, above the `#[cfg(test)]` block, add:

```rust
use crate::updater::channel::Channel;

/// Formats the build-identity text shown in the app footer. `version` is
/// the app's compiled-in version (see `app.package_info().version`), which
/// CI overwrites with the release tag before building -- see the "Write
/// version into tauri.conf.json" step in
/// `.github/workflows/pixel-build-manager-build.yml`.
pub fn format_version_label(channel: Option<Channel>, version: &str) -> String {
    match channel {
        Some(Channel::Release) => format!("Release {version}"),
        Some(Channel::Prerelease) => format!("Pre-release {version}"),
        None => "Development".to_string(),
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml version::`
Expected: PASS — 3 tests passed.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/version.rs src-tauri/src/lib.rs
git commit -m "Add version label formatting for the app footer"
```

---

### Task 2: Backend — expose `get_version_label` Tauri command

**Files:**
- Modify: `src-tauri/src/lib.rs` (add `use` for `Channel`, add command function, register in `generate_handler!`)

- [ ] **Step 1: Add the `Channel` import**

In `src-tauri/src/lib.rs`, find the existing `use` block that includes:

```rust
use updater::start_background_updates;
```

Change it to also bring in `Channel`:

```rust
use updater::channel::Channel;
use updater::start_background_updates;
```

- [ ] **Step 2: Add the command function**

Add this new command near the other simple query-style commands (e.g. next to `get_workspace_root`):

```rust
#[tauri::command]
fn get_version_label(app: tauri::AppHandle) -> String {
    version::format_version_label(Channel::current(), &app.package_info().version.to_string())
}
```

- [ ] **Step 3: Register it in the invoke handler**

In the `tauri::generate_handler![...]` list at the bottom of `run()`, add `get_version_label` alongside the existing entries:

```rust
        .invoke_handler(tauri::generate_handler![
            login_start,
            logout,
            is_logged_in,
            list_projects,
            list_releases_for_project,
            toggle_favorite,
            get_workspace_root,
            set_workspace_root,
            sync_release_asset,
            check_release_asset,
            get_active_release,
            clear_project_cache,
            list_cached_assets,
            delete_cached_asset,
            get_active_executable,
            launch_active_build,
            get_active_build_dir,
            get_version_label
        ])
```

- [ ] **Step 4: Verify the crate builds and all tests still pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — no failures, including the 3 new tests from Task 1.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "Expose get_version_label Tauri command"
```

---

### Task 3: Frontend — `api/version.ts` wrapper

**Files:**
- Create: `src/api/version.ts`

- [ ] **Step 1: Create the wrapper**

```typescript
import { invoke } from "@tauri-apps/api/core";

export function getVersionLabel(): Promise<string> {
  return invoke("get_version_label");
}
```

This follows the same thin-wrapper pattern as `src/api/settings.ts`'s `getWorkspaceRoot`; no dedicated test file, matching that existing convention (`api/*.ts` files in this repo have no `.test.ts` counterparts — they're exercised indirectly through component tests).

- [ ] **Step 2: Commit**

```bash
git add src/api/version.ts
git commit -m "Add getVersionLabel API wrapper"
```

---

### Task 4: Frontend — always-visible footer in `App.tsx`

**Files:**
- Modify: `src/App.tsx:1-21` (imports), `src/App.tsx:195-259` (`App()` body)
- Modify: `src/App.test.tsx` (add an `App`-level test)

- [ ] **Step 1: Write the failing test**

Add to `src/App.test.tsx` (alongside the existing `ProjectDetail` import/mocks — this needs its own `vi.mock` calls since it exercises `App`, not `ProjectDetail`):

```tsx
import App, { ProjectDetail } from "./App";
import * as authApi from "./api/auth";
import * as versionApi from "./api/version";
// ...(keep the existing imports for projectsApi, syncApi, Release)

vi.mock("./api/auth");
vi.mock("./api/version");

describe("App", () => {
  it("shows the version label in the footer even before logging in", async () => {
    vi.mocked(authApi.isLoggedIn).mockResolvedValue(false);
    vi.mocked(authApi.onLoginStatus).mockResolvedValue(() => {});
    vi.mocked(versionApi.getVersionLabel).mockResolvedValue("Release 0.4.0");

    render(<App />);

    expect(await screen.findByText("Release 0.4.0")).toBeInTheDocument();
  });
});
```

Note: `App` is currently only a default export with no named import used elsewhere in this file, so update the existing import line:

```tsx
import { ProjectDetail } from "./App";
```

to:

```tsx
import App, { ProjectDetail } from "./App";
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test -- App.test.tsx`
Expected: FAIL — the footer text "Release 0.4.0" is never rendered, because `App()` doesn't fetch or display a version label yet.

- [ ] **Step 3: Write the minimal implementation**

In `src/App.tsx`, update the top-of-file imports (currently lines 1-21) to add `type ReactNode` and the new API function:

```tsx
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { isLoggedIn, logout } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { getWorkspaceRoot } from "./api/settings";
import {
  deleteCachedAsset,
  getActiveBuildDir,
  getActiveExecutable,
  getActiveRelease,
  launchActiveBuild,
  listCachedAssets,
} from "./api/sync";
import { getVersionLabel } from "./api/version";
import { ClearCacheButton } from "./components/ClearCacheButton";
import { Login } from "./components/Login";
import { ProjectList } from "./components/ProjectList";
import { ReleaseList } from "./components/ReleaseList";
import { SyncStatus } from "./components/SyncStatus";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { useSync } from "./hooks/useSync";
import "./App.css";
```

Then, inside `function App()` (currently `src/App.tsx:195-259`), add a `versionLabel` state and its fetch effect right after the existing state declarations:

```tsx
function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);
  const [workspaceRoot, setWorkspaceRootState] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [versionLabel, setVersionLabel] = useState<string | null>(null);

  useEffect(() => {
    getVersionLabel()
      .then(setVersionLabel)
      .catch((error) => console.error("failed to load version label", error));
  }, []);

  useEffect(() => {
    isLoggedIn()
      .then(setLoggedIn)
      .catch(() => setLoggedIn(false));
  }, []);
```

(The rest of the existing effects, `handleToggleFavorite`, and `handleLoggedIn` are unchanged.)

Finally, replace the four `if`/return branches and the final `return` (currently `src/App.tsx:229-259`) with a single `content` variable rendered alongside a footer:

```tsx
  let content: ReactNode;
  if (loggedIn === null) {
    content = <p>Loading...</p>;
  } else if (!loggedIn) {
    content = <Login onLoggedIn={handleLoggedIn} />;
  } else if (!workspaceRoot) {
    content = <WorkspaceSetup onSet={setWorkspaceRootState} />;
  } else {
    content = (
      <div>
        <button
          onClick={async () => {
            try {
              await logout();
              setLoggedIn(false);
            } catch (error) {
              console.error("failed to log out", error);
            }
          }}
        >
          Log out
        </button>
        <ProjectList projects={projects} onSelect={setSelectedProject} onToggleFavorite={handleToggleFavorite} />
        {selectedProject && <ProjectDetail key={selectedProject} projectKey={selectedProject} />}
      </div>
    );
  }

  return (
    <div>
      {content}
      <footer>{versionLabel}</footer>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test -- App.test.tsx`
Expected: PASS — both the new `App` test and the existing `ProjectDetail` test pass.

- [ ] **Step 5: Run the full frontend test suite to check for regressions**

Run: `npm run test`
Expected: PASS — all existing component tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx src/App.test.tsx
git commit -m "Show version label in an always-visible app footer"
```

---

### Task 5: Final verification

- [ ] **Step 1: Run the full backend test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 2: Run the full frontend test suite**

Run: `npm run test`
Expected: PASS

- [ ] **Step 3: Run Rust formatting and lint checks (matches CI's `validate` job)**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml --check` then `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: both PASS with no diffs/warnings.

- [ ] **Step 4: Manually verify in a dev build**

Run: `npm run tauri dev`
Expected: the app launches and shows `"Development"` in the footer on every screen (login, workspace setup, and the main project list), since `PIXEL_BUILD_MANAGER_CHANNEL` is unset outside CI.
</content>
