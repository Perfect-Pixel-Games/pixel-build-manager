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

/// Lists the asset IDs currently present in `project_key`'s cache dir, read
/// back from the `<id>-<name>` file-naming scheme `cached_asset_path` writes.
/// Returns an empty list (not an error) when the cache dir doesn't exist yet,
/// since "no cache dir" and "empty cache dir" mean the same thing to callers.
pub fn list_cached_asset_ids(workspace_root: &Path, project_key: &str) -> io::Result<Vec<u64>> {
    let dir = cache_dir(workspace_root, project_key);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if let Some((id_str, _)) = name.split_once('-') {
                if let Ok(id) = id_str.parse::<u64>() {
                    ids.push(id);
                }
            }
        }
    }
    Ok(ids)
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

    #[test]
    fn list_cached_asset_ids_reads_ids_from_cache_file_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cache = cache_dir(root, "org/repo");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("42-build-shipping.zip"), b"data").unwrap();
        fs::write(cache.join("7-build-test.tar.gz"), b"data").unwrap();

        let mut ids = list_cached_asset_ids(root, "org/repo").unwrap();
        ids.sort();

        assert_eq!(ids, vec![7, 42]);
    }

    #[test]
    fn list_cached_asset_ids_returns_empty_when_cache_dir_is_missing() {
        let dir = tempfile::tempdir().unwrap();

        let ids = list_cached_asset_ids(dir.path(), "org/repo").unwrap();

        assert!(ids.is_empty());
    }
}
