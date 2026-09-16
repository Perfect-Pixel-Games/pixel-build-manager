use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

pub fn extract_zip_to_active(zip_path: &Path, active_dir: &Path) -> Result<(), ExtractError> {
    let temp_dir = active_dir.with_extension("tmp-extract");
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

    if active_dir.exists() {
        fs::remove_dir_all(active_dir)?;
    }
    fs::rename(&temp_dir, active_dir)?;

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

    #[test]
    fn extracts_zip_contents_into_the_active_dir() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("build.zip");
        write_test_zip(
            &zip_path,
            &[("game.exe", "binary-contents"), ("readme.txt", "hello")],
        );
        let active_dir = dir.path().join("active");

        extract_zip_to_active(&zip_path, &active_dir).unwrap();

        assert_eq!(
            fs::read_to_string(active_dir.join("readme.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            fs::read_to_string(active_dir.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }

    #[test]
    fn replaces_a_pre_existing_active_dir_entirely() {
        let dir = tempfile::tempdir().unwrap();
        let active_dir = dir.path().join("active");
        fs::create_dir_all(&active_dir).unwrap();
        fs::write(active_dir.join("stale-file.txt"), "old build").unwrap();

        let zip_path = dir.path().join("build.zip");
        write_test_zip(&zip_path, &[("new-file.txt", "new build")]);

        extract_zip_to_active(&zip_path, &active_dir).unwrap();

        assert!(!active_dir.join("stale-file.txt").exists());
        assert_eq!(
            fs::read_to_string(active_dir.join("new-file.txt")).unwrap(),
            "new build"
        );
    }
}
