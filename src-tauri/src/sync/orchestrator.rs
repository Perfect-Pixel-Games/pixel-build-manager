use crate::sync::cache::{active_dir, cache_dir, cached_asset_path};
use crate::sync::download::{download_with_progress, DownloadError};
use crate::sync::extract::{extract_to_active, ExtractError};
use std::path::Path;

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
    pub asset_id: u64,
    pub asset_name: &'a str,
    pub asset_size: u64,
    pub download_url: &'a str,
}

pub async fn sync_asset<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    request: SyncRequest<'_>,
    on_progress: F,
) -> Result<(), SyncError> {
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
            &cached_path,
            request.asset_size,
            on_progress,
        )
        .await?;
    }

    let active = active_dir(request.workspace_root, request.project_key);
    extract_to_active(&cached_path, &active)?;

    Ok(())
}

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
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_len,
            download_url: &download_url,
        };

        sync_asset(&http, request(), |_, _| {}).await.unwrap();

        let active = active_dir(workspace.path(), "org/repo");
        assert_eq!(
            std::fs::read_to_string(active.join("game.exe")).unwrap(),
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
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_len,
            download_url: &download_url,
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
        let active = active_dir(workspace.path(), "org/repo");
        assert_eq!(
            std::fs::read_to_string(active.join("game.exe")).unwrap(),
            "binary-contents"
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
            asset_id: 1,
            asset_name: "asset.tar.gz",
            asset_size: tar_gz_bytes.len() as u64,
            download_url: &download_url,
        };

        sync_asset(&http, request, |_, _| {}).await.unwrap();

        let active = active_dir(workspace.path(), "org/repo");
        assert_eq!(
            std::fs::read_to_string(active.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }
}
