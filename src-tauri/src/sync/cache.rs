// These path-computation functions aren't called from anywhere until later tasks
// wire up the sync commands, so clippy would otherwise flag them as dead code
// under `-D warnings`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn project_dir(workspace_root: &Path, project_key: &str) -> PathBuf {
    workspace_root.join(project_key.replace('/', "-"))
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

        assert_eq!(dir, PathBuf::from("D:\\Builds\\pixel-perfect-last-beacon"));
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
