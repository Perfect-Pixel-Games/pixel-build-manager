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
}
