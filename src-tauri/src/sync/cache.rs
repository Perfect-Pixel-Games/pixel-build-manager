use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Which of a project's two auto-tracked channels a latest-mode sync
/// targets. Modeled as an enum (not a raw string) so an invalid value from
/// the frontend fails Tauri's argument deserialization instead of ever
/// reaching a directory-path computation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LatestChannel {
    Release,
    Prerelease,
}

impl LatestChannel {
    fn dir_name(self) -> &'static str {
        match self {
            LatestChannel::Release => "release",
            LatestChannel::Prerelease => "prerelease",
        }
    }
}

const LATEST_SYNCED_TAG_FILE: &str = ".synced_tag";

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
/// path component with `_`. Needed for both a release's git tag (which can
/// legally contain `/`, e.g. `"release/1.2.3"`) and a release asset's name
/// (`config_name`) -- both are GitHub-controlled data (a tag can be
/// free-text on an unpublished draft release; an asset can be renamed by
/// anyone with push access, or a compromised/malicious upstream) being used
/// as a bare directory *component* for the first time in `build_config_dir`,
/// rather than passed through verbatim to GitHub's API. Replacing `:` and
/// `\` also neutralizes a Windows drive-letter (`C:\...`) or UNC (`\\server\
/// share\...`) prefix, which `Path::join` would otherwise treat as replacing
/// the base path entirely rather than nesting under it.
fn sanitize_path_component(value: &str) -> String {
    if value == "." || value == ".." {
        return "_".to_string();
    }
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
        .join(sanitize_path_component(config_name))
}

pub fn cached_asset_path(
    workspace_root: &Path,
    project_key: &str,
    asset_id: u64,
    asset_name: &str,
) -> PathBuf {
    cache_dir(workspace_root, project_key).join(format!(
        "{}-{}",
        asset_id,
        sanitize_path_component(asset_name)
    ))
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
    let dir =
        builds_root_dir(workspace_root, project_key).join(sanitize_path_component(release_tag));
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

/// The extraction root for one project's auto-tracked channel. Sibling to
/// `cache/` and `builds/` -- deliberately its own top-level directory rather
/// than living under `builds/<tag>/`, since its whole contents get wiped and
/// replaced on every new release rather than accumulating per-tag like
/// manual-mode builds do.
pub fn latest_channel_dir(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
) -> PathBuf {
    project_dir(workspace_root, project_key)
        .join("latest")
        .join(channel.dir_name())
}

/// The extraction directory for one ticked config within a latest channel.
/// Keyed by the *ticked config's stable template name*, not the release
/// asset's literal file name -- unlike manual mode's `build_config_dir`,
/// this must stay the same path across releases even for a project whose
/// asset names embed the version, since the whole point is one stable
/// location that gets overwritten in place.
pub fn latest_config_dir(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
    config_name: &str,
) -> PathBuf {
    latest_channel_dir(workspace_root, project_key, channel)
        .join(sanitize_path_component(config_name))
}

/// Reads which release tag is currently extracted into `channel`'s
/// directory, if any. Colocating this marker with the data it describes
/// (rather than in `settings.json`) means it's naturally reset whenever the
/// channel dir is cleared, with no separate bookkeeping to keep in sync.
pub fn read_latest_synced_tag(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
) -> Option<String> {
    let marker =
        latest_channel_dir(workspace_root, project_key, channel).join(LATEST_SYNCED_TAG_FILE);
    fs::read_to_string(marker).ok()
}

pub fn write_latest_synced_tag(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
    tag: &str,
) -> io::Result<()> {
    let dir = latest_channel_dir(workspace_root, project_key, channel);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(LATEST_SYNCED_TAG_FILE), tag)
}

/// Lists the build-config names currently extracted for `channel`, mirroring
/// `list_synced_configs`. The marker file above is a regular file, so the
/// existing `is_dir()` filter already excludes it with no extra logic.
pub fn list_synced_latest_configs(
    workspace_root: &Path,
    project_key: &str,
    channel: LatestChannel,
) -> io::Result<Vec<String>> {
    let dir = latest_channel_dir(workspace_root, project_key, channel);
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
    fn build_config_dir_sanitizes_a_release_tag_that_is_exactly_dot_dot() {
        let root = Path::new("D:\\Builds");

        let dir = build_config_dir(root, "org/repo", "..", "shipping.zip");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\builds\\_\\shipping.zip")
        );
    }

    // A release asset's name is GitHub-controlled data (renamable by anyone
    // with push access to the repo, not just this app's user), so it needs
    // exactly the same defense the release tag already gets -- otherwise a
    // hostile config_name could delete/replace an arbitrary directory (see
    // the two cases below) rather than just this project's own builds/.
    #[test]
    fn build_config_dir_sanitizes_a_config_name_that_is_exactly_dot_dot() {
        let root = Path::new("D:\\Builds");

        let dir = build_config_dir(root, "org/repo", "0.2.14", "..");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\builds\\0.2.14\\_")
        );
    }

    #[test]
    fn build_config_dir_sanitizes_a_config_name_shaped_like_an_absolute_windows_path() {
        let root = Path::new("D:\\Builds");

        // Without sanitization, `Path::join` treats a drive-letter-prefixed
        // argument as replacing the base path entirely rather than nesting
        // under it -- this proves that can no longer happen.
        let dir = build_config_dir(root, "org/repo", "0.2.14", "C:\\Windows\\System32");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\builds\\0.2.14\\C__Windows_System32")
        );
    }

    #[test]
    fn build_config_dir_sanitizes_a_config_name_shaped_like_a_unc_path() {
        let root = Path::new("D:\\Builds");

        let dir = build_config_dir(root, "org/repo", "0.2.14", "\\\\server\\share");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\builds\\0.2.14\\__server_share")
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

        assert_eq!(
            configs,
            vec!["shipping.zip".to_string(), "test.zip".to_string()]
        );
    }

    #[test]
    fn latest_channel_dir_nests_under_a_dedicated_latest_folder() {
        let root = Path::new("D:\\Builds");

        assert_eq!(
            latest_channel_dir(root, "org/repo", LatestChannel::Release),
            PathBuf::from("D:\\Builds\\org\\repo\\latest\\release")
        );
        assert_eq!(
            latest_channel_dir(root, "org/repo", LatestChannel::Prerelease),
            PathBuf::from("D:\\Builds\\org\\repo\\latest\\prerelease")
        );
    }

    #[test]
    fn latest_config_dir_sanitizes_the_config_name() {
        let root = Path::new("D:\\Builds");

        let dir = latest_config_dir(root, "org/repo", LatestChannel::Release, "..");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\org\\repo\\latest\\release\\_")
        );
    }

    #[test]
    fn read_latest_synced_tag_returns_none_when_nothing_has_synced_yet() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release),
            None
        );
    }

    #[test]
    fn write_then_read_latest_synced_tag_round_trips() {
        let dir = tempfile::tempdir().unwrap();

        write_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release, "0.2.14").unwrap();

        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release),
            Some("0.2.14".to_string())
        );
    }

    #[test]
    fn the_two_channels_have_independent_synced_tags() {
        let dir = tempfile::tempdir().unwrap();

        write_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release, "0.2.14").unwrap();
        write_latest_synced_tag(
            dir.path(),
            "org/repo",
            LatestChannel::Prerelease,
            "0.3.0-rc1",
        )
        .unwrap();

        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Release),
            Some("0.2.14".to_string())
        );
        assert_eq!(
            read_latest_synced_tag(dir.path(), "org/repo", LatestChannel::Prerelease),
            Some("0.3.0-rc1".to_string())
        );
    }

    #[test]
    fn list_synced_latest_configs_returns_empty_when_nothing_has_synced_yet() {
        let dir = tempfile::tempdir().unwrap();

        let configs =
            list_synced_latest_configs(dir.path(), "org/repo", LatestChannel::Release).unwrap();

        assert!(configs.is_empty());
    }

    #[test]
    fn list_synced_latest_configs_lists_extracted_configs_and_excludes_the_marker_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(latest_config_dir(
            root,
            "org/repo",
            LatestChannel::Release,
            "shipping.zip",
        ))
        .unwrap();
        write_latest_synced_tag(root, "org/repo", LatestChannel::Release, "0.2.14").unwrap();

        let configs = list_synced_latest_configs(root, "org/repo", LatestChannel::Release).unwrap();

        assert_eq!(configs, vec!["shipping.zip".to_string()]);
    }
}
