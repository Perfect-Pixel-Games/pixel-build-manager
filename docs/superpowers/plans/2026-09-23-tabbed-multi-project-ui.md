# Tabbed Multi-Project UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the always-visible favorites/all-projects list and single-active-build-per-project model with tab-based project binding, multi-build-config simultaneous sync/extraction, and app-wide light/dark/system theming.

**Architecture:** Backend: `Settings` gains `bound_projects`, `theme`, and per-project `selected_release_tag`/`ticked_configs` (replacing `active_release_tag`/`active_asset_name`); extraction moves from a single `active/` folder per project to `builds/<release_tag>/<config_name>/`, so multiple configs can coexist on disk. Frontend: a `TabBar` + `BindProjectPopup` replace `ProjectList`; a `BuildBrowser` (search + release list + config checkboxes + tri-state sync button) replaces `ReleaseList`; a `BusyOverlay` dims/disables the panel in place during a sync instead of swapping it out; per-synced-config `SyncedBuildControls` replace the old single Open Folder/Play pair.

**Tech Stack:** Rust (Tauri v2 backend), TypeScript/React 19 (frontend), `cargo test`/`cargo fmt`/`cargo clippy`, `vitest`/`@testing-library/react`.

Reference spec: `docs/superpowers/specs/2026-09-23-tabbed-multi-project-ui-design.md`

**Commit after every task** (not just at the end) — regular, incremental commits are expected throughout this implementation.

---

### Task 1: Backend — `Settings` data model rework

**Files:**
- Modify: `src-tauri/src/settings/mod.rs` (whole file)

- [ ] **Step 1: Write the failing tests**

Replace the `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/settings/mod.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let settings = Settings::load_from(&path);

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn default_theme_is_system() {
        assert_eq!(Settings::default().theme, Theme::System);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");

        let mut projects = HashMap::new();
        projects.insert(
            "pixel-perfect/last-beacon".to_string(),
            ProjectSettings {
                favorite: true,
                selected_release_tag: Some("0.2.14".to_string()),
                ticked_configs: vec!["shipping.zip".to_string()],
            },
        );
        let settings = Settings {
            workspace_root: Some(PathBuf::from("D:\\Builds")),
            projects,
            bound_projects: vec!["pixel-perfect/last-beacon".to_string()],
            theme: Theme::Dark,
        };

        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);

        assert_eq!(loaded, settings);
    }

    #[test]
    fn set_favorite_creates_entry_if_missing() {
        let mut settings = Settings::default();

        settings.set_favorite("pixel-perfect/last-beacon", true);

        assert!(settings.projects["pixel-perfect/last-beacon"].favorite);

        settings.set_favorite("pixel-perfect/last-beacon", false);

        assert!(!settings.projects["pixel-perfect/last-beacon"].favorite);
    }

    #[test]
    fn set_workspace_root_updates_the_field() {
        let mut settings = Settings::default();

        settings.set_workspace_root(PathBuf::from("D:\\Builds"));

        assert_eq!(settings.workspace_root, Some(PathBuf::from("D:\\Builds")));
    }

    #[test]
    fn bind_project_appends_in_order_and_is_idempotent() {
        let mut settings = Settings::default();

        settings.bind_project("org/repo-a");
        settings.bind_project("org/repo-b");
        settings.bind_project("org/repo-a");

        assert_eq!(settings.bound_projects, vec!["org/repo-a", "org/repo-b"]);
    }

    #[test]
    fn unbind_project_removes_it_and_is_a_no_op_when_absent() {
        let mut settings = Settings::default();
        settings.bind_project("org/repo-a");
        settings.bind_project("org/repo-b");

        settings.unbind_project("org/repo-a");
        settings.unbind_project("org/does-not-exist");

        assert_eq!(settings.bound_projects, vec!["org/repo-b"]);
    }

    #[test]
    fn set_selected_release_records_the_tag() {
        let mut settings = Settings::default();

        settings.set_selected_release("org/repo", "0.2.14");

        assert_eq!(
            settings.projects["org/repo"].selected_release_tag,
            Some("0.2.14".to_string())
        );
    }

    #[test]
    fn set_ticked_configs_replaces_the_list() {
        let mut settings = Settings::default();
        settings.set_ticked_configs("org/repo", vec!["a.zip".to_string()]);

        settings.set_ticked_configs("org/repo", vec!["b.zip".to_string(), "c.zip".to_string()]);

        assert_eq!(
            settings.projects["org/repo"].ticked_configs,
            vec!["b.zip".to_string(), "c.zip".to_string()]
        );
    }

    #[test]
    fn set_theme_updates_the_field() {
        let mut settings = Settings::default();

        settings.set_theme(Theme::Dark);

        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn theme_serializes_as_lowercase_strings() {
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml settings::`
Expected: FAIL — compile errors (`Theme` doesn't exist, `ProjectSettings` has no field `selected_release_tag`/`ticked_configs`, `Settings` has no field `bound_projects`/`theme`, no methods `bind_project`/`unbind_project`/`set_selected_release`/`set_ticked_configs`/`set_theme`).

- [ ] **Step 3: Write the minimal implementation**

Replace the top of `src-tauri/src/settings/mod.rs` (everything above the `#[cfg(test)]` block) with:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub favorite: bool,
    pub selected_release_tag: Option<String>,
    #[serde(default)]
    pub ticked_configs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub projects: HashMap<String, ProjectSettings>,
    #[serde(default)]
    pub bound_projects: Vec<String>,
    #[serde(default)]
    pub theme: Theme,
}

impl Settings {
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self).expect("Settings must serialize");
        std::fs::write(path, contents)
    }

    pub fn set_favorite(&mut self, project_key: &str, favorite: bool) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .favorite = favorite;
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }

    /// Appends `project_key` to the tab order if it isn't already bound.
    /// Binding an already-bound project is a no-op rather than moving it to
    /// the end -- rebinding must never reorder existing tabs.
    pub fn bind_project(&mut self, project_key: &str) {
        if !self.bound_projects.iter().any(|p| p == project_key) {
            self.bound_projects.push(project_key.to_string());
        }
    }

    /// Removes `project_key` from the tab order. Leaves its `ProjectSettings`
    /// entry (selected release, ticked configs, favorite) untouched, so
    /// rebinding later restores where the user left off.
    pub fn unbind_project(&mut self, project_key: &str) {
        self.bound_projects.retain(|p| p != project_key);
    }

    pub fn set_selected_release(&mut self, project_key: &str, release_tag: &str) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .selected_release_tag = Some(release_tag.to_string());
    }

    pub fn set_ticked_configs(&mut self, project_key: &str, configs: Vec<String>) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .ticked_configs = configs;
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml settings::`
Expected: PASS — all tests in `settings::tests` pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/settings/mod.rs
git commit -m "Rework Settings for tab binding, per-config ticks, and theme"
```

---

### Task 2: Backend — retarget extraction to per-release-per-config directories

**Files:**
- Modify: `src-tauri/src/sync/cache.rs` (whole file)
- Modify: `src-tauri/src/sync/extract.rs` (whole file)
- Modify: `src-tauri/src/sync/launch.rs` (whole file)
- Modify: `src-tauri/src/sync/orchestrator.rs` (whole file)

This is one coherent change split into four files because they're tightly coupled: `cache.rs` defines *where* a build extracts to, `extract.rs`/`launch.rs` just needed their "active" naming updated since that concept no longer exists, and `orchestrator.rs` wires the new cache path into the sync flow.

- [ ] **Step 1: Write the failing tests for `cache.rs`**

Replace the `#[cfg(test)] mod tests` block in `src-tauri/src/sync/cache.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_nests_owner_and_repo() {
        let root = Path::new("D:\\Builds");

        let dir = project_dir(root, "pixel-perfect/last-beacon");

        assert_eq!(dir, PathBuf::from("D:\\Builds\\pixel-perfect\\last-beacon"));
    }

    #[test]
    fn project_dir_does_not_collide_when_hyphens_straddle_the_slash() {
        let root = Path::new("D:\\Builds");

        let first = project_dir(root, "foo/bar-baz");
        let second = project_dir(root, "foo-bar/baz");

        assert_ne!(first, second);
    }

    #[test]
    fn project_dir_does_not_collide_when_hyphens_sit_at_the_boundary() {
        let root = Path::new("D:\\Builds");

        let first = project_dir(root, "foo-/bar");
        let second = project_dir(root, "foo/-bar");

        assert_ne!(first, second);
    }

    #[test]
    fn cache_and_builds_root_dirs_are_siblings_under_the_project_dir() {
        let root = Path::new("D:\\Builds");

        assert_eq!(
            cache_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org\\repo\\cache")
        );
        assert_eq!(
            builds_root_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org\\repo\\builds")
        );
    }

    #[test]
    fn cached_asset_path_is_keyed_by_id_and_name() {
        let root = Path::new("D:\\Builds");

        let path = cached_asset_path(root, "org/repo", 42, "build-shipping.zip");

        assert_eq!(
            path,
            PathBuf::from("D:\\Builds\\org\\repo\\cache\\42-build-shipping.zip")
        );
    }

    #[test]
    fn build_config_dir_nests_release_then_config_under_builds() {
        let root = Path::new("D:\\Builds");

        let dir = build_config_dir(root, "org/repo", "0.2.14", "shipping.zip");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\builds\\0.2.14\\shipping.zip")
        );
    }

    #[test]
    fn build_config_dir_sanitizes_slashes_in_the_release_tag() {
        let root = Path::new("D:\\Builds");

        let dir = build_config_dir(root, "org/repo", "release/1.2.3", "shipping.zip");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\builds\\release_1.2.3\\shipping.zip")
        );
    }

    #[test]
    fn list_synced_configs_returns_empty_when_the_release_has_nothing_synced() {
        let dir = tempfile::tempdir().unwrap();

        let configs = list_synced_configs(dir.path(), "org/repo", "0.2.14").unwrap();

        assert!(configs.is_empty());
    }

    #[test]
    fn list_synced_configs_lists_every_extracted_config_for_that_release() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(build_config_dir(root, "org/repo", "0.2.14", "shipping.zip")).unwrap();
        fs::create_dir_all(build_config_dir(root, "org/repo", "0.2.14", "test.zip")).unwrap();
        // A different release's configs must not leak into this release's list.
        fs::create_dir_all(build_config_dir(root, "org/repo", "0.2.13", "shipping.zip")).unwrap();

        let mut configs = list_synced_configs(root, "org/repo", "0.2.14").unwrap();
        configs.sort();

        assert_eq!(configs, vec!["shipping.zip".to_string(), "test.zip".to_string()]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml cache::`
Expected: FAIL — compile errors (`builds_root_dir`, `build_config_dir`, `list_synced_configs` don't exist yet).

- [ ] **Step 3: Write the minimal implementation for `cache.rs`**

Replace everything above the `#[cfg(test)]` block in `src-tauri/src/sync/cache.rs` with:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Turns an "owner/repo" project key into a project directory nested as
/// `<workspace_root>/<owner>/<repo>`. Nesting (rather than flattening the key
/// into a single naming-escaped directory component) is what actually makes
/// this collision-free: any string-based encoding scheme that joins owner and
/// repo into one path segment risks two different owner/repo pairs colliding
/// on the same encoded string whenever hyphens straddle the boundary between
/// them (verified: naive `replace('/', "-")`, and even a hyphen-escaping
/// variant, both had live collisions). Separate filesystem directory
/// components can never collide this way, since each level is stored as a
/// distinct entry rather than concatenated into one string.
///
/// This guarantee holds precisely when `owner` and `repo` are each non-empty
/// and contain neither `/` nor `\` themselves -- exactly what GitHub
/// guarantees for the `owner`/`repo` halves of a repo's `full_name`, which is
/// the only source `project_key` comes from in this app. Debug builds assert
/// that precondition rather than silently falling back to something that
/// could collide (an empty owner/repo, or a key with no `/` at all).
pub fn project_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    let (owner, repo) = project_key
        .split_once('/')
        .expect("project_key must be of the form \"owner/repo\"");
    debug_assert!(!owner.is_empty(), "project_key owner must not be empty");
    debug_assert!(!repo.is_empty(), "project_key repo must not be empty");
    debug_assert!(
        !owner.contains(['/', '\\']) && !repo.contains(['/', '\\']),
        "project_key owner/repo must not themselves contain a path separator"
    );
    workspace_root.join(owner).join(repo)
}

pub fn cache_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    project_dir(workspace_root, project_key).join("cache")
}

pub fn builds_root_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    project_dir(workspace_root, project_key).join("builds")
}

/// Replaces characters that are invalid (or awkward) as a single Windows
/// path component with `_`. Needed because -- unlike `owner`/`repo`, which
/// GitHub guarantees are slash-free -- a release's git tag can legally
/// contain `/` (e.g. `"release/1.2.3"`), and `build_config_dir` is the first
/// place a tag is used as a bare directory name rather than passed through
/// verbatim to GitHub's API. Asset/config names don't need this: they're
/// already used unsanitized as filenames in `cached_asset_path` below, since
/// GitHub-uploaded asset names can't contain path separators.
fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

/// The extraction directory for one (release, build config) pair. Each pair
/// gets its own folder so multiple configs -- even across different
/// releases -- can be extracted and coexist on disk at once, rather than the
/// single `active/` folder every sync used to atomically replace.
pub fn build_config_dir(
    workspace_root: &Path,
    project_key: &str,
    release_tag: &str,
    config_name: &str,
) -> PathBuf {
    builds_root_dir(workspace_root, project_key)
        .join(sanitize_path_component(release_tag))
        .join(config_name)
}

pub fn cached_asset_path(
    workspace_root: &Path,
    project_key: &str,
    asset_id: u64,
    asset_name: &str,
) -> PathBuf {
    cache_dir(workspace_root, project_key).join(format!("{}-{}", asset_id, asset_name))
}

/// Lists the build-config names currently extracted for `release_tag`, read
/// back as directory names under that release's folder. Returns an empty
/// list (not an error) when nothing has been synced for this release yet --
/// "no builds dir for this release" and "nothing synced" mean the same thing
/// to callers.
pub fn list_synced_configs(
    workspace_root: &Path,
    project_key: &str,
    release_tag: &str,
) -> io::Result<Vec<String>> {
    let dir = builds_root_dir(workspace_root, project_key).join(sanitize_path_component(release_tag));
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut configs = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                configs.push(name.to_string());
            }
        }
    }
    Ok(configs)
}
```

- [ ] **Step 4: Run the `cache.rs` tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml cache::`
Expected: PASS.

- [ ] **Step 5: Rename `extract.rs`'s "active" naming**

`extract.rs`'s logic doesn't need to change — only the naming, since there's no more "active build" concept anywhere in the app. Replace the whole file's content with (identical logic, renamed):

```rust
use std::fs;
use std::io;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("unrecognized archive format (not a .zip or .tar.gz file)")]
    UnsupportedFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    Zip,
    TarGz,
}

/// Sniffs the archive format from its magic bytes rather than trusting the
/// file name/extension, since asset names aren't guaranteed to accurately
/// reflect their content.
fn detect_archive_kind(archive_path: &Path) -> Result<ArchiveKind, ExtractError> {
    let mut file = fs::File::open(archive_path)?;
    let mut magic = [0u8; 4];
    let read = file.read(&mut magic)?;

    if read >= 2 && magic[0] == 0x1F && magic[1] == 0x8B {
        // gzip magic number -- in this app's domain (packaged game builds),
        // every gzip-compressed asset is a tarball.
        Ok(ArchiveKind::TarGz)
    } else if read >= 2 && &magic[0..2] == b"PK" {
        Ok(ArchiveKind::Zip)
    } else {
        Err(ExtractError::UnsupportedFormat)
    }
}

/// Extracts a release asset (`.zip` or `.tar.gz`, auto-detected from its
/// content) into `target_dir`, replacing whatever was there before.
pub fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<(), ExtractError> {
    match detect_archive_kind(archive_path)? {
        ArchiveKind::Zip => extract_zip(archive_path, target_dir),
        ArchiveKind::TarGz => extract_tar_gz(archive_path, target_dir),
    }
}

/// True if `path` cannot escape the directory it's extracted into (no `..`,
/// no absolute/rooted components, no Windows drive prefix). Mirrors the
/// protection `enclosed_name()` gives the zip extractor below.
fn is_contained(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

pub fn extract_tar_gz(archive_path: &Path, target_dir: &Path) -> Result<(), ExtractError> {
    let temp_dir = target_dir.with_extension("tmp-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let file = fs::File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path: PathBuf = entry.path()?.into_owned();
        if !is_contained(&entry_path) {
            continue;
        }
        let out_path = temp_dir.join(&entry_path);

        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else if entry_type.is_file() {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut out_file)?;
        }
        // Symlinks, hardlinks, and other special entry types are skipped
        // entirely -- same posture as the zip extractor below, which never
        // calls a symlink-creation API either.
    }

    if target_dir.exists() {
        fs::remove_dir_all(target_dir)?;
    }
    fs::rename(&temp_dir, target_dir)?;

    Ok(())
}

pub fn extract_zip(zip_path: &Path, target_dir: &Path) -> Result<(), ExtractError> {
    let temp_dir = target_dir.with_extension("tmp-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let out_path = match entry.enclosed_name() {
            Some(p) => temp_dir.join(p),
            None => continue,
        };

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut out_file)?;
        }
    }

    if target_dir.exists() {
        fs::remove_dir_all(target_dir)?;
    }
    fs::rename(&temp_dir, target_dir)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_zip(path: &Path, files: &[(&str, &str)]) {
        let file = fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        for (name, contents) in files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn write_test_tar_gz(path: &Path, entries: &[&str]) {
        let file = fs::File::create(path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for name in entries {
            let contents = format!("contents of {name}");
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, contents.as_bytes())
                .unwrap();
        }
        builder.finish().unwrap();
    }

    /// `tar::Builder::append_data` refuses to write a `..`-containing path at
    /// all, so a malicious archive can't be built through the normal API.
    /// Real-world malicious tarballs aren't produced by this well-behaved
    /// crate though, so this bypasses it by writing the entry name directly
    /// into the raw header bytes (which skips `Header::set_path`'s checks)
    /// to prove the *extractor's* `is_contained` guard is what stops it.
    fn write_tar_gz_with_traversal_entry(path: &Path, traversal_name: &str, safe_name: &str) {
        let file = fs::File::create(path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);

        let contents = format!("contents of {traversal_name}");
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        let name_bytes = traversal_name.as_bytes();
        header.as_old_mut().name[..name_bytes.len()].copy_from_slice(name_bytes);
        header.set_cksum();
        builder.append(&header, contents.as_bytes()).unwrap();

        let safe_contents = format!("contents of {safe_name}");
        let mut safe_header = tar::Header::new_gnu();
        safe_header.set_size(safe_contents.len() as u64);
        safe_header.set_mode(0o644);
        safe_header.set_cksum();
        builder
            .append_data(&mut safe_header, safe_name, safe_contents.as_bytes())
            .unwrap();

        builder.finish().unwrap();
    }

    #[test]
    fn extracts_zip_contents_into_the_target_dir() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("build.zip");
        write_test_zip(
            &zip_path,
            &[("game.exe", "binary-contents"), ("readme.txt", "hello")],
        );
        let target_dir = dir.path().join("target");

        extract_zip(&zip_path, &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("readme.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }

    #[test]
    fn replaces_a_pre_existing_target_dir_entirely() {
        let dir = tempfile::tempdir().unwrap();
        let target_dir = dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("stale-file.txt"), "old build").unwrap();

        let zip_path = dir.path().join("build.zip");
        write_test_zip(&zip_path, &[("new-file.txt", "new build")]);

        extract_zip(&zip_path, &target_dir).unwrap();

        assert!(!target_dir.join("stale-file.txt").exists());
        assert_eq!(
            fs::read_to_string(target_dir.join("new-file.txt")).unwrap(),
            "new build"
        );
    }

    #[test]
    fn extracts_tar_gz_contents_into_the_target_dir() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("build.tar.gz");
        write_test_tar_gz(&archive_path, &["game.exe", "readme.txt"]);
        let target_dir = dir.path().join("target");

        extract_tar_gz(&archive_path, &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("readme.txt")).unwrap(),
            "contents of readme.txt"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("game.exe")).unwrap(),
            "contents of game.exe"
        );
    }

    #[test]
    fn tar_gz_replaces_a_pre_existing_target_dir_entirely() {
        let dir = tempfile::tempdir().unwrap();
        let target_dir = dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("stale-file.txt"), "old build").unwrap();

        let archive_path = dir.path().join("build.tar.gz");
        write_test_tar_gz(&archive_path, &["new-file.txt"]);

        extract_tar_gz(&archive_path, &target_dir).unwrap();

        assert!(!target_dir.join("stale-file.txt").exists());
        assert_eq!(
            fs::read_to_string(target_dir.join("new-file.txt")).unwrap(),
            "contents of new-file.txt"
        );
    }

    #[test]
    fn tar_gz_skips_path_traversal_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("evil.tar.gz");
        write_tar_gz_with_traversal_entry(&archive_path, "../escaped.txt", "safe.txt");
        let target_dir = dir.path().join("target");

        extract_tar_gz(&archive_path, &target_dir).unwrap();

        // The malicious entry must not have escaped the extraction root.
        assert!(!dir.path().join("escaped.txt").exists());
        assert!(!target_dir.join("../escaped.txt").exists());
        // The safe entry alongside it still extracts normally.
        assert_eq!(
            fs::read_to_string(target_dir.join("safe.txt")).unwrap(),
            "contents of safe.txt"
        );
    }

    #[test]
    fn extract_archive_dispatches_zip_by_content_not_file_name() {
        let dir = tempfile::tempdir().unwrap();
        // Deliberately named without a .zip extension, to prove detection
        // is by magic bytes, not the file name.
        let archive_path = dir.path().join("asset-download");
        write_test_zip(&archive_path, &[("game.exe", "zip-contents")]);
        let target_dir = dir.path().join("target");

        extract_archive(&archive_path, &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("game.exe")).unwrap(),
            "zip-contents"
        );
    }

    #[test]
    fn extract_archive_dispatches_tar_gz_by_content_not_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("asset-download");
        write_test_tar_gz(&archive_path, &["game.exe"]);
        let target_dir = dir.path().join("target");

        extract_archive(&archive_path, &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("game.exe")).unwrap(),
            "contents of game.exe"
        );
    }

    #[test]
    fn extract_archive_reports_unsupported_format_for_unrecognized_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("not-an-archive");
        fs::write(&archive_path, b"just some plain text, not an archive").unwrap();
        let target_dir = dir.path().join("target");

        let result = extract_archive(&archive_path, &target_dir);

        assert!(matches!(result, Err(ExtractError::UnsupportedFormat)));
    }
}
```

- [ ] **Step 6: Rename `launch.rs`'s "active" naming**

Replace the whole file with (same logic, `active_dir` parameter renamed to `build_dir`, `find_active_executable` renamed to `find_build_executable`):

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Substrings (checked case-insensitively) that mark a `.exe` as auxiliary
/// tooling bundled alongside a build rather than the game itself, so it's
/// never picked as the thing "Play" launches.
const NON_GAME_EXE_MARKERS: &[&str] = &[
    "crashpad",
    "crashhandler",
    "vc_redist",
    "vcredist",
    "dotnet",
    "uninstall",
    "unins0",
    "redist",
];

fn is_likely_game_executable(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    !NON_GAME_EXE_MARKERS
        .iter()
        .any(|marker| name.contains(marker))
}

fn collect_exe_files(dir: &Path, depth: u32, out: &mut Vec<(u32, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_exe_files(&path, depth + 1, out);
        } else if path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("exe"))
            .unwrap_or(false)
        {
            out.push((depth, path));
        }
    }
}

/// Finds the executable a synced build's launch button should run: the
/// shallowest `.exe` under `build_dir` (recursing into subfolders),
/// preferring ones that don't look like bundled installer/crash-reporter
/// tooling. Returns `None` if the build dir has no executables at all.
pub fn find_build_executable(build_dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    collect_exe_files(build_dir, 0, &mut candidates);
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    candidates
        .iter()
        .find(|(_, path)| is_likely_game_executable(path))
        .or_else(|| candidates.first())
        .map(|(_, path)| path.clone())
}

/// Spawns `exe_path` detached, with its working directory set to its own
/// folder -- most game engines resolve their asset paths relative to the
/// executable's directory, not the caller's cwd.
pub fn launch_executable(exe_path: &Path) -> io::Result<()> {
    let mut command = Command::new(exe_path);
    if let Some(parent) = exe_path.parent() {
        command.current_dir(parent);
    }
    command.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_an_exe_at_the_top_level_of_the_build_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("game.exe"), b"").unwrap();

        let found = find_build_executable(dir.path()).unwrap();

        assert_eq!(found, dir.path().join("game.exe"));
    }

    #[test]
    fn finds_an_exe_nested_in_a_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("WindowsNoEditor");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("MyGame.exe"), b"").unwrap();

        let found = find_build_executable(dir.path()).unwrap();

        assert_eq!(found, nested.join("MyGame.exe"));
    }

    #[test]
    fn prefers_a_real_game_exe_over_bundled_installer_tooling() {
        let dir = tempfile::tempdir().unwrap();
        // Alphabetically first, so this proves the denylist -- not sort
        // order -- is what skips it.
        fs::write(dir.path().join("Crashpad.exe"), b"").unwrap();
        fs::write(dir.path().join("MyGame.exe"), b"").unwrap();

        let found = find_build_executable(dir.path()).unwrap();

        assert_eq!(found, dir.path().join("MyGame.exe"));
    }

    #[test]
    fn returns_none_when_no_executable_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), b"hi").unwrap();

        assert!(find_build_executable(dir.path()).is_none());
    }

    #[test]
    fn prefers_the_shallowest_exe_when_multiple_real_candidates_exist() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Launcher.exe"), b"").unwrap();
        let nested = dir.path().join("Binaries");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Deeper.exe"), b"").unwrap();

        let found = find_build_executable(dir.path()).unwrap();

        assert_eq!(found, dir.path().join("Launcher.exe"));
    }

    #[test]
    fn returns_none_when_the_build_dir_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        assert!(find_build_executable(&missing).is_none());
    }
}
```

- [ ] **Step 7: Run `extract::` and `launch::` tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml extract:: launch::`
Expected: PASS.

- [ ] **Step 8: Write the failing tests for `orchestrator.rs`**

Replace the `#[cfg(test)] mod tests` block in `src-tauri/src/sync/orchestrator.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn build_test_zip_bytes() -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            writer.start_file("game.exe", options).unwrap();
            writer.write_all(b"binary-contents").unwrap();
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[tokio::test]
    async fn syncs_downloads_extracts_and_skips_redownload_on_second_sync() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(move |_: &wiremock::Request| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone())
            })
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let zip_len = build_test_zip_bytes().len() as u64;
        let download_url = format!("{}/asset.zip", server.uri());
        let request = || SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_len,
            download_url: &download_url,
            auth_token: "test-token",
        };

        sync_asset(&http, request(), |_, _| {}).await.unwrap();

        let target = build_config_dir(workspace.path(), "org/repo", "0.2.14", "asset.zip");
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
            "binary-contents"
        );
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        sync_asset(&http, request(), |_, _| {}).await.unwrap();

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "second sync of the same asset must not re-download"
        );
    }

    #[tokio::test]
    async fn resyncs_when_the_cached_copy_size_does_not_match_the_expected_size() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(move |_: &wiremock::Request| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone())
            })
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let zip_len = build_test_zip_bytes().len() as u64;
        let download_url = format!("{}/asset.zip", server.uri());
        let request = SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_len,
            download_url: &download_url,
            auth_token: "test-token",
        };

        // Simulate a cached copy left over from an interrupted/corrupted
        // previous download: present on disk, but the wrong size.
        std::fs::create_dir_all(cache_dir(workspace.path(), "org/repo")).unwrap();
        let cached_path = cached_asset_path(workspace.path(), "org/repo", 1, "asset.zip");
        std::fs::write(&cached_path, b"not a real archive").unwrap();

        sync_asset(&http, request, |_, _| {}).await.unwrap();

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "a cached copy whose size doesn't match the expected asset size must be re-downloaded"
        );
        let target = build_config_dir(workspace.path(), "org/repo", "0.2.14", "asset.zip");
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }

    #[tokio::test]
    async fn two_different_configs_of_the_same_release_coexist_on_disk() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone()))
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let zip_len = zip_bytes.len() as u64;

        let shipping_request = SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "shipping.zip",
            asset_size: zip_len,
            download_url: &format!("{}/shipping.zip", server.uri()),
            auth_token: "test-token",
        };
        let test_request = SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 2,
            asset_name: "test.zip",
            asset_size: zip_len,
            download_url: &format!("{}/test.zip", server.uri()),
            auth_token: "test-token",
        };

        sync_asset(&http, shipping_request, |_, _| {}).await.unwrap();
        sync_asset(&http, test_request, |_, _| {}).await.unwrap();

        assert!(build_config_dir(workspace.path(), "org/repo", "0.2.14", "shipping.zip")
            .join("game.exe")
            .exists());
        assert!(
            build_config_dir(workspace.path(), "org/repo", "0.2.14", "test.zip")
                .join("game.exe")
                .exists(),
            "syncing a second config must not remove the first config's extracted folder"
        );
    }

    fn build_test_tar_gz_bytes() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let encoder =
                flate2::write::GzEncoder::new(&mut buffer, flate2::Compression::default());
            let mut builder = tar::Builder::new(encoder);
            let contents = b"binary-contents";
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "game.exe", &contents[..])
                .unwrap();
            builder.finish().unwrap();
        }
        buffer
    }

    #[tokio::test]
    async fn syncs_a_tar_gz_asset_end_to_end() {
        let tar_gz_bytes = build_test_tar_gz_bytes();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/asset.tar.gz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tar_gz_bytes.clone()))
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let download_url = format!("{}/asset.tar.gz", server.uri());
        let request = SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "asset.tar.gz",
            asset_size: tar_gz_bytes.len() as u64,
            download_url: &download_url,
            auth_token: "test-token",
        };

        sync_asset(&http, request, |_, _| {}).await.unwrap();

        let target = build_config_dir(workspace.path(), "org/repo", "0.2.14", "asset.tar.gz");
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }
}
```

- [ ] **Step 9: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml orchestrator::`
Expected: FAIL — compile errors (`SyncRequest` has no field `release_tag`, `build_config_dir` not imported/used).

- [ ] **Step 10: Write the minimal implementation for `orchestrator.rs`**

Replace everything above the `#[cfg(test)]` block in `src-tauri/src/sync/orchestrator.rs` with:

```rust
use crate::sync::cache::{build_config_dir, cache_dir, cached_asset_path};
use crate::sync::download::{download_with_progress, DownloadError};
use crate::sync::extract::{extract_archive, ExtractError};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Extract(#[from] ExtractError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SyncRequest<'a> {
    pub workspace_root: &'a Path,
    pub project_key: &'a str,
    pub release_tag: &'a str,
    pub asset_id: u64,
    pub asset_name: &'a str,
    pub asset_size: u64,
    pub download_url: &'a str,
    pub auth_token: &'a str,
}

/// Ensures a valid copy of the requested asset is present in the project's
/// cache dir, downloading (or re-downloading, if the existing copy's size
/// doesn't match) as needed.
async fn ensure_cached_copy<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    request: &SyncRequest<'_>,
    on_progress: F,
) -> Result<PathBuf, SyncError> {
    std::fs::create_dir_all(cache_dir(request.workspace_root, request.project_key))?;
    let cached_path = cached_asset_path(
        request.workspace_root,
        request.project_key,
        request.asset_id,
        request.asset_name,
    );

    let cached_copy_is_valid = std::fs::metadata(&cached_path)
        .map(|metadata| metadata.len() == request.asset_size)
        .unwrap_or(false);

    if !cached_copy_is_valid {
        download_with_progress(
            http,
            request.download_url,
            request.auth_token,
            &cached_path,
            request.asset_size,
            on_progress,
        )
        .await?;
    }

    Ok(cached_path)
}

/// Downloads/verifies the asset (if needed) and (re-)extracts it into its
/// own `builds/<release_tag>/<config_name>/` folder, keyed by release and
/// build config so multiple configs -- even across different releases --
/// can be extracted and coexist on disk at once. Always re-extracts even
/// when the cached copy was already valid, which is what makes this safe to
/// call both for "sync what's missing" and "re-verify/repair" alike: the
/// frontend decides which of those two it wants purely by choosing which
/// assets to pass in, not by passing a different flag here.
pub async fn sync_asset<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    request: SyncRequest<'_>,
    on_progress: F,
) -> Result<(), SyncError> {
    let cached_path = ensure_cached_copy(http, &request, on_progress).await?;

    let target = build_config_dir(
        request.workspace_root,
        request.project_key,
        request.release_tag,
        request.asset_name,
    );
    extract_archive(&cached_path, &target)?;

    Ok(())
}
```

- [ ] **Step 11: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync::`
Expected: PASS — all of `cache::`, `extract::`, `launch::`, `orchestrator::`, `download::` pass.

- [ ] **Step 12: Commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/sync/cache.rs src-tauri/src/sync/extract.rs src-tauri/src/sync/launch.rs src-tauri/src/sync/orchestrator.rs
git commit -m "Retarget sync/extraction to per-release-per-config directories"
```

---

### Task 3: Backend — rewrite the Tauri command surface in `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs` (whole file)

This task removes commands whose concept no longer exists (`check_release_asset`, `get_active_release`, `list_cached_assets`, `delete_cached_asset`, `get_active_executable`, `launch_active_build`, `get_active_build_dir` — none of these survive the redesign, since there's no more single "active build" or per-asset cache browsing/deletion UI), and adds the commands the new frontend needs (tab binding, theme, selected release, ticked configs, synced-configs lookup, per-config executable/launch/dir). `lib.rs` has no `#[cfg(test)]` tests of its own (commands are thin wrappers over the already-tested `settings`/`sync` modules), so this task is verified by `cargo build`/`cargo clippy` rather than new unit tests.

- [ ] **Step 1: Replace the whole file**

Replace all of `src-tauri/src/lib.rs` with:

```rust
mod auth;
mod github;
mod settings;
mod sync;
mod updater;
mod version;

use auth::device_flow::DeviceFlowClient;
use auth::login::{perform_device_login, LoginStatus};
use auth::session::ensure_valid_access_token;
use auth::token_store::{KeyringTokenStore, TokenStore};
use github::client::{GithubClient, ReleaseSummary};
use serde::Serialize;
use settings::{Settings, Theme};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use sync::cache::{build_config_dir, builds_root_dir, cache_dir};
use sync::launch::{find_build_executable, launch_executable};
use sync::orchestrator::{sync_asset, SyncRequest};
use tauri::{Emitter, Manager};
use updater::channel::Channel;
use updater::start_background_updates;

const GITHUB_CLIENT_ID: &str = "Ov23ligQDGOJvlWsEXJc";

pub struct AppState {
    pub token_store: Arc<dyn TokenStore>,
    pub settings_path: PathBuf,
    /// Serializes settings.json read-modify-write cycles across commands so
    /// concurrent writes (e.g. rapid favorite toggles) can't clobber each other.
    pub settings_lock: Mutex<()>,
    /// Project keys with an in-flight sync or cache-clear operation, so the
    /// two can never race against each other's filesystem writes for the
    /// same project (e.g. clearing a cache mid-download).
    pub active_operations: Mutex<HashSet<String>>,
    /// Serializes token refreshes so two concurrent GitHub-backed commands
    /// can't both try to redeem the same (single-use) refresh token at
    /// once. Held across an `.await`, so this must be a tokio mutex rather
    /// than `std::sync::Mutex`.
    pub token_refresh_lock: tokio::sync::Mutex<()>,
}

/// Marks `project_key` as having an in-flight operation, failing if one is
/// already running. Callers must pair this with `end_operation` on every
/// exit path (success or error).
fn begin_operation(state: &AppState, project_key: &str) -> Result<(), String> {
    let mut active = state.active_operations.lock().map_err(|e| e.to_string())?;
    if !active.insert(project_key.to_string()) {
        return Err(format!(
            "another sync or cache operation is already in progress for {project_key}"
        ));
    }
    Ok(())
}

fn end_operation(state: &AppState, project_key: &str) {
    if let Ok(mut active) = state.active_operations.lock() {
        active.remove(project_key);
    }
}

fn settings_path_for(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir must be resolvable")
        .join("settings.json")
}

fn workspace_root_from_settings(state: &AppState) -> Result<PathBuf, String> {
    Settings::load_from(&state.settings_path)
        .workspace_root
        .ok_or_else(|| "workspace root not set".to_string())
}

#[tauri::command]
async fn login_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let client = DeviceFlowClient::new(GITHUB_CLIENT_ID.to_string());
    let token_store = state.token_store.clone();
    let app_for_events = app.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            perform_device_login(&client, token_store.as_ref(), move |status: LoginStatus| {
                let _ = app_for_events.emit("login-status", status);
            })
            .await
        {
            eprintln!("device login failed: {e}");
        }
    });

    Ok(())
}

#[tauri::command]
fn logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.token_store.clear()
}

#[tauri::command]
fn is_logged_in(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(state.token_store.load()?.is_some())
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectListItem {
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub favorite: bool,
}

async fn build_github_client(state: &AppState) -> Result<GithubClient, String> {
    let _guard = state.token_refresh_lock.lock().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after the unix epoch")
        .as_secs();
    let device_flow_client = DeviceFlowClient::new(GITHUB_CLIENT_ID.to_string());
    let token =
        ensure_valid_access_token(&device_flow_client, state.token_store.as_ref(), now).await?;
    Ok(GithubClient::new(token))
}

#[tauri::command]
async fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<ProjectListItem>, String> {
    let client = build_github_client(&state).await?;
    let repos = client
        .list_accessible_repos_with_releases()
        .await
        .map_err(|e| e.to_string())?;
    let settings = Settings::load_from(&state.settings_path);

    Ok(repos
        .into_iter()
        .map(|repo| {
            let favorite = settings
                .projects
                .get(&repo.full_name)
                .map(|p| p.favorite)
                .unwrap_or(false);
            ProjectListItem {
                full_name: repo.full_name,
                owner: repo.owner.login,
                name: repo.name,
                favorite,
            }
        })
        .collect())
}

#[tauri::command]
async fn list_releases_for_project(
    full_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ReleaseSummary>, String> {
    let (owner, repo) = full_name
        .split_once('/')
        .ok_or_else(|| format!("invalid project full_name: {}", full_name))?;
    let client = build_github_client(&state).await?;
    client
        .list_releases(owner, repo)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_favorite(
    full_name: String,
    favorite: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_favorite(&full_name, favorite);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_workspace_root(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .workspace_root
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn get_version_label(app: tauri::AppHandle) -> String {
    version::format_version_label(Channel::current(), &app.package_info().version.to_string())
}

#[tauri::command]
fn set_workspace_root(root: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_workspace_root(PathBuf::from(root));
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_bound_projects(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(Settings::load_from(&state.settings_path).bound_projects)
}

#[tauri::command]
fn bind_project(full_name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.bind_project(&full_name);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn unbind_project(full_name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.unbind_project(&full_name);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_theme(state: tauri::State<'_, AppState>) -> Theme {
    Settings::load_from(&state.settings_path).theme
}

#[tauri::command]
fn set_theme(theme: Theme, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_theme(theme);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_selected_release(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .projects
        .get(&project_key)
        .and_then(|p| p.selected_release_tag.clone()))
}

#[tauri::command]
fn set_selected_release(
    project_key: String,
    release_tag: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_selected_release(&project_key, &release_tag);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_ticked_configs(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let settings = Settings::load_from(&state.settings_path);
    Ok(settings
        .projects
        .get(&project_key)
        .map(|p| p.ticked_configs.clone())
        .unwrap_or_default())
}

#[tauri::command]
fn set_ticked_configs(
    project_key: String,
    configs: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.settings_lock.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load_from(&state.settings_path);
    settings.set_ticked_configs(&project_key, configs);
    settings
        .save_to(&state.settings_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_synced_configs(
    project_key: String,
    release_tag: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    sync::cache::list_synced_configs(&workspace_root, &project_key, &release_tag)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
struct SyncProgressPayload {
    project_key: String,
    downloaded: u64,
    total: u64,
}

// Each parameter here is a distinct argument the frontend passes via
// `invoke`, mirroring the asset/release fields it already has on hand;
// bundling them into a struct would just move the same field count behind
// an extra layer without reducing what callers need to supply.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn sync_release_asset(
    app: tauri::AppHandle,
    project_key: String,
    release_tag: String,
    asset_id: u64,
    asset_name: String,
    asset_size: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    begin_operation(&state, &project_key)?;
    let result = sync_release_asset_inner(
        app,
        &project_key,
        &release_tag,
        asset_id,
        &asset_name,
        asset_size,
        &state,
    )
    .await;
    end_operation(&state, &project_key);
    result
}

#[allow(clippy::too_many_arguments)]
async fn sync_release_asset_inner(
    app: tauri::AppHandle,
    project_key: &str,
    release_tag: &str,
    asset_id: u64,
    asset_name: &str,
    asset_size: u64,
    state: &AppState,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(state)?;

    let client = build_github_client(state).await?;
    let (owner, repo) = project_key
        .split_once('/')
        .ok_or_else(|| format!("invalid project_key: {project_key}"))?;
    let download_url = client.asset_download_url(owner, repo, asset_id);
    let auth_token = client.token();

    let http = reqwest::Client::new();
    let request = SyncRequest {
        workspace_root: &workspace_root,
        project_key,
        release_tag,
        asset_id,
        asset_name,
        asset_size,
        download_url: &download_url,
        auth_token,
    };

    let project_key_for_events = project_key.to_string();
    sync_asset(&http, request, move |downloaded, total| {
        let _ = app.emit(
            "sync-progress",
            SyncProgressPayload {
                project_key: project_key_for_events.clone(),
                downloaded,
                total,
            },
        );
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_project_cache(
    project_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    begin_operation(&state, &project_key)?;
    let result = clear_project_cache_inner(&project_key, &state);
    end_operation(&state, &project_key);
    result
}

// Clears everything this project has on disk: the raw downloaded cache
// *and* every extracted release/config build under `builds/`. There's no
// separate "clear builds" affordance, since leaving extracted builds behind
// after a cache clear would contradict "manually cleared" -- the one button
// is the manual-clear mechanism for the whole project's disk footprint.
fn clear_project_cache_inner(project_key: &str, state: &AppState) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(state)?;

    let cache = cache_dir(&workspace_root, project_key);
    if cache.exists() {
        std::fs::remove_dir_all(&cache).map_err(|e| e.to_string())?;
    }
    let builds = builds_root_dir(&workspace_root, project_key);
    if builds.exists() {
        std::fs::remove_dir_all(&builds).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_build_executable(
    project_key: String,
    release_tag: String,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = build_config_dir(&workspace_root, &project_key, &release_tag, &config_name);
    Ok(find_build_executable(&dir).map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn launch_build(
    project_key: String,
    release_tag: String,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = build_config_dir(&workspace_root, &project_key, &release_tag, &config_name);
    let exe = find_build_executable(&dir)
        .ok_or_else(|| "no executable found in this build".to_string())?;
    launch_executable(&exe).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_build_dir(
    project_key: String,
    release_tag: String,
    config_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_root = workspace_root_from_settings(&state)?;
    let dir = build_config_dir(&workspace_root, &project_key, &release_tag, &config_name);
    Ok(dir.exists().then(|| dir.to_string_lossy().to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = settings_path_for(app.handle());
            app.manage(AppState {
                token_store: Arc::new(KeyringTokenStore),
                settings_path,
                settings_lock: Mutex::new(()),
                active_operations: Mutex::new(HashSet::new()),
                token_refresh_lock: tokio::sync::Mutex::new(()),
            });

            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            start_background_updates(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            login_start,
            logout,
            is_logged_in,
            list_projects,
            list_releases_for_project,
            toggle_favorite,
            get_workspace_root,
            set_workspace_root,
            list_bound_projects,
            bind_project,
            unbind_project,
            get_theme,
            set_theme,
            get_selected_release,
            set_selected_release,
            get_ticked_configs,
            set_ticked_configs,
            list_synced_configs,
            sync_release_asset,
            clear_project_cache,
            get_build_executable,
            launch_build,
            get_build_dir,
            get_version_label
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Verify the crate builds**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: PASS — no compile errors, no unused-import warnings.

- [ ] **Step 3: Run the full backend test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — every test across `settings::`, `sync::cache::`, `sync::extract::`, `sync::launch::`, `sync::orchestrator::`, `sync::download::` passes.

- [ ] **Step 4: Run clippy (matches CI's `validate` job)**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: PASS — no warnings.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/lib.rs
git commit -m "Rewrite Tauri command surface for tab binding, theme, and per-config sync"
```

---

### Task 4: Frontend — API wrapper updates (`api/settings.ts`, `api/sync.ts`)

**Files:**
- Modify: `src/api/settings.ts` (whole file)
- Modify: `src/api/sync.ts` (whole file)

No dedicated test files for these — matches this repo's existing convention that `api/*.ts` files (thin `invoke` wrappers) aren't unit tested directly, only exercised indirectly through the component/hook tests that mock them (see e.g. `src/api/version.ts`, which has no `version.test.ts`).

- [ ] **Step 1: Replace `src/api/settings.ts`**

```typescript
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export type Theme = "light" | "dark" | "system";

export function getWorkspaceRoot(): Promise<string | null> {
  return invoke("get_workspace_root");
}

export function setWorkspaceRoot(root: string): Promise<void> {
  return invoke("set_workspace_root", { root });
}

export function pickFolder(): Promise<string | null> {
  return open({ directory: true, multiple: false }) as Promise<string | null>;
}

export function getTheme(): Promise<Theme> {
  return invoke("get_theme");
}

export function setTheme(theme: Theme): Promise<void> {
  return invoke("set_theme", { theme });
}

export function listBoundProjects(): Promise<string[]> {
  return invoke("list_bound_projects");
}

export function bindProject(fullName: string): Promise<void> {
  return invoke("bind_project", { fullName });
}

export function unbindProject(fullName: string): Promise<void> {
  return invoke("unbind_project", { fullName });
}
```

- [ ] **Step 2: Replace `src/api/sync.ts`**

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Release, ReleaseAsset } from "./projects";

export type SyncProgress = {
  project_key: string;
  downloaded: number;
  total: number;
};

export function syncReleaseAsset(
  projectKey: string,
  release: Release,
  asset: ReleaseAsset,
): Promise<void> {
  return invoke("sync_release_asset", {
    projectKey,
    releaseTag: release.tag_name,
    assetId: asset.id,
    assetName: asset.name,
    assetSize: asset.size,
  });
}

export function clearProjectCache(projectKey: string): Promise<void> {
  return invoke("clear_project_cache", { projectKey });
}

export function getSelectedRelease(projectKey: string): Promise<string | null> {
  return invoke("get_selected_release", { projectKey });
}

export function setSelectedRelease(projectKey: string, releaseTag: string): Promise<void> {
  return invoke("set_selected_release", { projectKey, releaseTag });
}

export function getTickedConfigs(projectKey: string): Promise<string[]> {
  return invoke("get_ticked_configs", { projectKey });
}

export function setTickedConfigs(projectKey: string, configs: string[]): Promise<void> {
  return invoke("set_ticked_configs", { projectKey, configs });
}

export function listSyncedConfigs(projectKey: string, releaseTag: string): Promise<string[]> {
  return invoke("list_synced_configs", { projectKey, releaseTag });
}

export function getBuildExecutable(
  projectKey: string,
  releaseTag: string,
  configName: string,
): Promise<string | null> {
  return invoke("get_build_executable", { projectKey, releaseTag, configName });
}

export function launchBuild(
  projectKey: string,
  releaseTag: string,
  configName: string,
): Promise<void> {
  return invoke("launch_build", { projectKey, releaseTag, configName });
}

export function getBuildDir(
  projectKey: string,
  releaseTag: string,
  configName: string,
): Promise<string | null> {
  return invoke("get_build_dir", { projectKey, releaseTag, configName });
}

export function onSyncProgress(callback: (progress: SyncProgress) => void) {
  return listen<SyncProgress>("sync-progress", (event) => callback(event.payload));
}
```

- [ ] **Step 3: Verify the frontend still typechecks**

Run: `npm run build`
Expected: FAIL at this point — `src/App.tsx`, `src/hooks/useSync.ts`, `src/components/ReleaseList.tsx`, and their tests still reference the removed exports (`checkReleaseAsset`, `getActiveRelease`, `listCachedAssets`, `deleteCachedAsset`, `getActiveExecutable`, `launchActiveBuild`, `getActiveBuildDir`). This is expected and will be resolved as later tasks rewrite those files — do not attempt to fix them here.

- [ ] **Step 4: Commit**

```bash
git add src/api/settings.ts src/api/sync.ts
git commit -m "Update API wrappers for tab binding, theme, and per-config sync"
```

---

### Task 5: Frontend — theming (`useTheme` hook + `ThemeToggle` component)

**Files:**
- Modify: `src/test-setup.ts`
- Create: `src/hooks/useTheme.ts`
- Create: `src/hooks/useTheme.test.ts`
- Create: `src/components/ThemeToggle.tsx`
- Create: `src/components/ThemeToggle.test.tsx`

- [ ] **Step 1: Reset the theme attribute between tests**

`useTheme` sets a `data-theme` attribute directly on `document.documentElement`, which lives outside the React tree `@testing-library/react`'s `cleanup()` unmounts — without resetting it, one test's theme choice would leak into the next. Update `src/test-setup.ts`:

```typescript
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// @testing-library/react's auto-cleanup relies on a global `afterEach`,
// which vitest does not inject unless `test.globals: true` is set. Register
// it explicitly so component trees are unmounted between tests.
afterEach(() => {
  cleanup();
  // useTheme sets this directly on <html>, outside the React tree cleanup()
  // unmounts -- reset it so one test's theme choice can't leak into the next.
  document.documentElement.removeAttribute("data-theme");
});
```

- [ ] **Step 2: Write the failing test for `useTheme`**

Create `src/hooks/useTheme.test.ts`:

```typescript
import { describe, expect, it, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useTheme } from "./useTheme";
import * as settingsApi from "../api/settings";

vi.mock("../api/settings");

describe("useTheme", () => {
  it("loads the persisted theme on mount and applies it to the root element", async () => {
    vi.mocked(settingsApi.getTheme).mockResolvedValue("dark");

    const { result } = renderHook(() => useTheme());

    await waitFor(() => expect(result.current.theme).toBe("dark"));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("removes the data-theme attribute for system so the media query decides", async () => {
    vi.mocked(settingsApi.getTheme).mockResolvedValue("system");
    document.documentElement.setAttribute("data-theme", "dark");

    renderHook(() => useTheme());

    await waitFor(() =>
      expect(document.documentElement.hasAttribute("data-theme")).toBe(false),
    );
  });

  it("persists and applies a new theme when setTheme is called", async () => {
    vi.mocked(settingsApi.getTheme).mockResolvedValue("system");
    vi.mocked(settingsApi.setTheme).mockResolvedValue(undefined);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.theme).toBe("system"));

    act(() => {
      result.current.setTheme("light");
    });

    expect(result.current.theme).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(settingsApi.setTheme).toHaveBeenCalledWith("light");
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `npm run test -- useTheme.test.ts`
Expected: FAIL — `src/hooks/useTheme.ts` does not exist yet.

- [ ] **Step 4: Write the minimal implementation**

Create `src/hooks/useTheme.ts`:

```typescript
import { useEffect, useState } from "react";
import { getTheme, setTheme as persistTheme, type Theme } from "../api/settings";

export type { Theme };

/** Loads the persisted theme on mount, applies it to <html data-theme>, and
 * persists any change the caller makes via the returned setter. "system"
 * removes the attribute entirely so `@media (prefers-color-scheme)` decides. */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>("system");

  useEffect(() => {
    getTheme()
      .then(setThemeState)
      .catch((error) => console.error("failed to load theme", error));
  }, []);

  useEffect(() => {
    if (theme === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  }, [theme]);

  const setTheme = (next: Theme) => {
    setThemeState(next);
    persistTheme(next).catch((error) => console.error("failed to save theme", error));
  };

  return { theme, setTheme };
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `npm run test -- useTheme.test.ts`
Expected: PASS.

- [ ] **Step 6: Write the failing test for `ThemeToggle`**

Create `src/components/ThemeToggle.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ThemeToggle } from "./ThemeToggle";

describe("ThemeToggle", () => {
  it("renders a button for each theme option", () => {
    render(<ThemeToggle theme="system" onChange={() => {}} />);

    expect(screen.getByRole("button", { name: "Light" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "System" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dark" })).toBeInTheDocument();
  });

  it("marks the current theme's button as pressed", () => {
    render(<ThemeToggle theme="dark" onChange={() => {}} />);

    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
  });

  it("calls onChange with the clicked theme", () => {
    const onChange = vi.fn();
    render(<ThemeToggle theme="system" onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: "Dark" }));

    expect(onChange).toHaveBeenCalledWith("dark");
  });
});
```

- [ ] **Step 7: Run the test to verify it fails**

Run: `npm run test -- ThemeToggle.test.tsx`
Expected: FAIL — `src/components/ThemeToggle.tsx` does not exist yet.

- [ ] **Step 8: Write the minimal implementation**

Create `src/components/ThemeToggle.tsx`:

```tsx
import type { Theme } from "../hooks/useTheme";

type Props = {
  theme: Theme;
  onChange: (theme: Theme) => void;
};

const OPTIONS: { value: Theme; label: string; icon: string }[] = [
  { value: "light", label: "Light", icon: "☀" },
  { value: "system", label: "System", icon: "🖥" },
  { value: "dark", label: "Dark", icon: "🌙" },
];

export function ThemeToggle({ theme, onChange }: Props) {
  return (
    <div className="theme-toggle" role="group" aria-label="Theme">
      {OPTIONS.map((option) => (
        <button
          key={option.value}
          aria-label={option.label}
          aria-pressed={theme === option.value}
          onClick={() => onChange(option.value)}
        >
          {option.icon}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 9: Run the test to verify it passes**

Run: `npm run test -- ThemeToggle.test.tsx`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src/test-setup.ts src/hooks/useTheme.ts src/hooks/useTheme.test.ts src/components/ThemeToggle.tsx src/components/ThemeToggle.test.tsx
git commit -m "Add useTheme hook and ThemeToggle component"
```

---

### Task 6: Frontend — rewrite `useSync` for sequential multi-config sync

**Files:**
- Modify: `src/hooks/useSync.ts` (whole file)
- Modify: `src/hooks/useSync.test.ts` (whole file)

The old hook exposed `sync(release, asset)` (download + activate one asset) and `check(asset)` (download only, never activate). Neither concept survives: there's no more "activate," and there's no more "download without extracting" path (see Task 2/3's removal of `check_release_asset`/`ensure_asset_cached`). The new hook exposes a single `syncConfigs(release, assets)` that syncs a list of assets one at a time, reporting which one is currently in flight.

- [ ] **Step 1: Write the failing tests**

Replace all of `src/hooks/useSync.test.ts` with:

```typescript
import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useSync } from "./useSync";
import * as syncApi from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

vi.mock("../api/sync");

const release: Release = {
  id: 1,
  tag_name: "0.2.14",
  name: "0.2.14",
  prerelease: false,
  published_at: null,
  assets: [],
};
const shippingAsset: ReleaseAsset = {
  id: 10,
  name: "shipping.zip",
  size: 1000,
  browser_download_url: "https://example.com/shipping.zip",
};
const testAsset: ReleaseAsset = {
  id: 11,
  name: "test.zip",
  size: 500,
  browser_download_url: "https://example.com/test.zip",
};

describe("useSync", () => {
  beforeEach(() => {
    vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  });

  it("syncs a single asset and transitions to done", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSync("org/repo"));

    let succeeded: boolean | undefined;
    await act(async () => {
      succeeded = await result.current.syncConfigs(release, [shippingAsset]);
    });

    expect(succeeded).toBe(true);
    expect(result.current.state).toEqual({ phase: "done" });
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledWith("org/repo", release, shippingAsset);
  });

  it("syncs multiple assets one at a time, in order", async () => {
    const callOrder: string[] = [];
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(async (_projectKey, _release, asset) => {
      callOrder.push(asset.name);
    });
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await result.current.syncConfigs(release, [shippingAsset, testAsset]);
    });

    expect(callOrder).toEqual(["shipping.zip", "test.zip"]);
  });

  it("reports which config is currently syncing", async () => {
    let resolveShipping: () => void = () => {};
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(
      () => new Promise((resolve) => (resolveShipping = () => resolve(undefined))),
    );
    const { result } = renderHook(() => useSync("org/repo"));

    act(() => {
      result.current.syncConfigs(release, [shippingAsset]);
    });

    expect(result.current.state).toEqual({
      phase: "syncing",
      configName: "shipping.zip",
      downloaded: 0,
      total: 1000,
    });

    await act(async () => resolveShipping());
  });

  it("updates progress only for matching project_key events", async () => {
    let capturedCallback: (progress: syncApi.SyncProgress) => void = () => {};
    vi.mocked(syncApi.onSyncProgress).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useSync("org/repo"));

    act(() => {
      result.current.syncConfigs(release, [shippingAsset]);
    });
    act(() => {
      capturedCallback({ project_key: "org/other-repo", downloaded: 5, total: 1000 });
    });
    expect(result.current.state).toMatchObject({ downloaded: 0, total: 1000 });

    act(() => {
      capturedCallback({ project_key: "org/repo", downloaded: 500, total: 1000 });
    });
    await waitFor(() =>
      expect(result.current.state).toMatchObject({ downloaded: 500, total: 1000 }),
    );
  });

  it("transitions to error and rethrows when a sync call rejects, without syncing the rest of the batch", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockRejectedValue(new Error("network down"));
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await expect(result.current.syncConfigs(release, [shippingAsset, testAsset])).rejects.toThrow(
        "network down",
      );
    });

    expect(result.current.state).toEqual({ phase: "error", message: "Error: network down" });
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(1);
  });

  it("ignores a second syncConfigs() call while one is already in flight, resolving it to false", async () => {
    let resolveFirstSync: () => void = () => {};
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(
      () => new Promise((resolve) => (resolveFirstSync = () => resolve(undefined))),
    );
    const { result } = renderHook(() => useSync("org/repo"));

    let firstCall!: Promise<boolean>;
    let secondCall!: Promise<boolean>;
    act(() => {
      firstCall = result.current.syncConfigs(release, [shippingAsset]);
    });
    act(() => {
      secondCall = result.current.syncConfigs(release, [testAsset]);
    });

    expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(1);
    await expect(secondCall).resolves.toBe(false);

    await act(async () => resolveFirstSync());

    await expect(firstCall).resolves.toBe(true);
    expect(result.current.state).toEqual({ phase: "done" });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm run test -- useSync.test.ts`
Expected: FAIL — `result.current.syncConfigs` is not a function (the hook still only exposes `sync`/`check`).

- [ ] **Step 3: Write the minimal implementation**

Replace all of `src/hooks/useSync.ts` with:

```typescript
import { useCallback, useEffect, useRef, useState } from "react";
import { onSyncProgress, syncReleaseAsset } from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

export type SyncState =
  | { phase: "idle" }
  | { phase: "syncing"; configName: string; downloaded: number; total: number }
  | { phase: "done" }
  | { phase: "error"; message: string };

/** Syncs (downloads + extracts) one or more build configs of a release, one
 * at a time, reporting progress for whichever config is currently
 * downloading. Guards against overlapping calls the same way the previous
 * single-asset version did. */
export function useSync(projectKey: string) {
  const [state, setState] = useState<SyncState>({ phase: "idle" });
  const inFlightRef = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    onSyncProgress((progress) => {
      if (progress.project_key !== projectKey) {
        return;
      }
      setState((prev) =>
        prev.phase === "syncing"
          ? { ...prev, downloaded: progress.downloaded, total: progress.total }
          : prev,
      );
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectKey]);

  // Runs sync for every asset in sequence, guarding against overlapping
  // calls. Returns true if every asset synced successfully, false if it was
  // skipped because another sync was already in flight, and rethrows (after
  // recording the error state) on the first failure, stopping the batch --
  // so the caller never mistakes a partially-synced batch for a fully
  // completed one.
  const syncConfigs = useCallback(
    async (release: Release, assets: ReleaseAsset[]) => {
      if (inFlightRef.current) {
        return false;
      }
      inFlightRef.current = true;
      try {
        for (const asset of assets) {
          setState({ phase: "syncing", configName: asset.name, downloaded: 0, total: asset.size });
          await syncReleaseAsset(projectKey, release, asset);
        }
        setState({ phase: "done" });
        return true;
      } catch (error) {
        setState({ phase: "error", message: String(error) });
        throw error;
      } finally {
        inFlightRef.current = false;
      }
    },
    [projectKey],
  );

  return { state, syncConfigs };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run test -- useSync.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useSync.ts src/hooks/useSync.test.ts
git commit -m "Rewrite useSync to sync multiple build configs sequentially"
```

---
</content>
