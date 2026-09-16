// `sync_asset` isn't called from anywhere until later tasks wire up the sync
// commands, so clippy would otherwise flag it (and its supporting types) as
// dead code under `-D warnings`.
#![allow(dead_code)]

use crate::sync::cache::{active_dir, cache_dir, cached_asset_path};
use crate::sync::download::{download_with_progress, DownloadError};
use crate::sync::extract::{extract_zip_to_active, ExtractError};
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

    if !cached_path.exists() {
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
    extract_zip_to_active(&cached_path, &active)?;

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
}
