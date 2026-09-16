// These path-computation functions aren't called from anywhere until later tasks
// wire up the sync commands, so clippy would otherwise flag them as dead code
// under `-D warnings`.
#![allow(dead_code)]

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

pub fn active_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    project_dir(workspace_root, project_key).join("active")
}

pub fn cached_asset_path(
    workspace_root: &Path,
    project_key: &str,
    asset_id: u64,
    asset_name: &str,
) -> PathBuf {
    cache_dir(workspace_root, project_key).join(format!("{}-{}", asset_id, asset_name))
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
    fn cache_and_active_dirs_are_siblings_under_the_project_dir() {
        let root = Path::new("D:\\Builds");

        assert_eq!(
            cache_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org\\repo\\cache")
        );
        assert_eq!(
            active_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org\\repo\\active")
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
}
