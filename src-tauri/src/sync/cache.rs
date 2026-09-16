// These path-computation functions aren't called from anywhere until later tasks
// wire up the sync commands, so clippy would otherwise flag them as dead code
// under `-D warnings`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Encodes an "owner/repo" project key into a single filesystem-safe directory
/// name. GitHub owner and repo names may themselves contain hyphens, so a
/// naive `replace('/', "-")` can collide two different projects onto the same
/// directory (e.g. "foo/bar-baz" and "foo-bar/baz" would both become
/// "foo-bar-baz"). Escaping literal hyphens first (`-` -> `--`) before using a
/// single `-` as the owner/repo separator keeps the mapping collision-free.
fn encode_project_key(project_key: &str) -> String {
    project_key.replace('-', "--").replace('/', "-")
}

pub fn project_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    workspace_root.join(encode_project_key(project_key))
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
    fn project_dir_replaces_slash_with_dash() {
        let root = Path::new("D:\\Builds");

        let dir = project_dir(root, "pixel-perfect/last-beacon");

        assert_eq!(
            dir,
            PathBuf::from("D:\\Builds\\pixel--perfect-last--beacon")
        );
    }

    #[test]
    fn project_dir_does_not_collide_when_hyphens_straddle_the_slash() {
        let root = Path::new("D:\\Builds");

        let first = project_dir(root, "foo/bar-baz");
        let second = project_dir(root, "foo-bar/baz");

        assert_ne!(first, second);
    }

    #[test]
    fn cache_and_active_dirs_are_siblings_under_the_project_dir() {
        let root = Path::new("D:\\Builds");

        assert_eq!(
            cache_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org-repo\\cache")
        );
        assert_eq!(
            active_dir(root, "org/repo"),
            PathBuf::from("D:\\Builds\\org-repo\\active")
        );
    }

    #[test]
    fn cached_asset_path_is_keyed_by_id_and_name() {
        let root = Path::new("D:\\Builds");

        let path = cached_asset_path(root, "org/repo", 42, "build-shipping.zip");

        assert_eq!(
            path,
            PathBuf::from("D:\\Builds\\org-repo\\cache\\42-build-shipping.zip")
        );
    }
}
