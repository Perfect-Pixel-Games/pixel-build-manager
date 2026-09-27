use crate::sync::cache::{build_config_dir, cache_dir, cached_asset_path};
use crate::sync::download::{download_with_progress, DownloadError};
use crate::sync::extract::{extract_archive, ExtractError};
use std::path::{Path, PathBuf};

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
    pub release_tag: &'a str,
    pub asset_id: u64,
    pub asset_name: &'a str,
    pub asset_size: u64,
    pub download_url: &'a str,
    pub auth_token: &'a str,
}

/// Ensures a valid copy of the requested asset is present in the project's
/// cache dir, downloading (or re-downloading, if the existing copy's size
/// doesn't match) as needed.
async fn ensure_cached_copy<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    request: &SyncRequest<'_>,
    on_progress: F,
) -> Result<PathBuf, SyncError> {
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
            request.auth_token,
            &cached_path,
            request.asset_size,
            on_progress,
        )
        .await?;
    }

    Ok(cached_path)
}

/// Downloads/verifies the asset (if needed) and (re-)extracts it into its
/// own `builds/<release_tag>/<config_name>/` folder, keyed by release and
/// build config so multiple configs -- even across different releases --
/// can be extracted and coexist on disk at once. Always re-extracts even
/// when the cached copy was already valid, which is what makes this safe to
/// call both for "sync what's missing" and "re-verify/repair" alike: the
/// frontend decides which of those two it wants purely by choosing which
/// assets to pass in, not by passing a different flag here.
pub async fn sync_asset<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    request: SyncRequest<'_>,
    on_progress: F,
) -> Result<(), SyncError> {
    let cached_path = ensure_cached_copy(http, &request, on_progress).await?;

    let target = build_config_dir(
        request.workspace_root,
        request.project_key,
        request.release_tag,
        request.asset_name,
    );
    extract_archive(&cached_path, &target)?;

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
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_len,
            download_url: &download_url,
            auth_token: "test-token",
        };

        sync_asset(&http, request(), |_, _| {}).await.unwrap();

        let target = build_config_dir(workspace.path(), "org/repo", "0.2.14", "asset.zip");
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
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
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "asset.zip",
            asset_size: zip_len,
            download_url: &download_url,
            auth_token: "test-token",
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
        let target = build_config_dir(workspace.path(), "org/repo", "0.2.14", "asset.zip");
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }

    #[tokio::test]
    async fn two_different_configs_of_the_same_release_coexist_on_disk() {
        let zip_bytes = build_test_zip_bytes();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone()))
            .mount(&server)
            .await;

        let workspace = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let zip_len = zip_bytes.len() as u64;

        let shipping_request = SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "shipping.zip",
            asset_size: zip_len,
            download_url: &format!("{}/shipping.zip", server.uri()),
            auth_token: "test-token",
        };
        let test_request = SyncRequest {
            workspace_root: workspace.path(),
            project_key: "org/repo",
            release_tag: "0.2.14",
            asset_id: 2,
            asset_name: "test.zip",
            asset_size: zip_len,
            download_url: &format!("{}/test.zip", server.uri()),
            auth_token: "test-token",
        };

        sync_asset(&http, shipping_request, |_, _| {})
            .await
            .unwrap();
        sync_asset(&http, test_request, |_, _| {}).await.unwrap();

        assert!(
            build_config_dir(workspace.path(), "org/repo", "0.2.14", "shipping.zip")
                .join("game.exe")
                .exists()
        );
        assert!(
            build_config_dir(workspace.path(), "org/repo", "0.2.14", "test.zip")
                .join("game.exe")
                .exists(),
            "syncing a second config must not remove the first config's extracted folder"
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
            release_tag: "0.2.14",
            asset_id: 1,
            asset_name: "asset.tar.gz",
            asset_size: tar_gz_bytes.len() as u64,
            download_url: &download_url,
            auth_token: "test-token",
        };

        sync_asset(&http, request, |_, _| {}).await.unwrap();

        let target = build_config_dir(workspace.path(), "org/repo", "0.2.14", "asset.tar.gz");
        assert_eq!(
            std::fs::read_to_string(target.join("game.exe")).unwrap(),
            "binary-contents"
        );
    }
}
