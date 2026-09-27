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
