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
