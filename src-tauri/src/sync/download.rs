use futures_util::StreamExt;
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("download failed with HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("downloaded size {actual} does not match expected size {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
}

// `Accept: application/octet-stream` + a bearer token are required to fetch
// a private repo's release asset bytes from GitHub's asset-content API --
// without them the request 404s (GitHub masks private-resource existence
// rather than returning 401/403) instead of streaming the asset.
pub async fn download_with_progress<F: FnMut(u64, u64)>(
    http: &reqwest::Client,
    url: &str,
    auth_token: &str,
    destination: &Path,
    expected_size: u64,
    mut on_progress: F,
) -> Result<(), DownloadError> {
    let response = http
        .get(url)
        .header("User-Agent", "pixel-build-manager")
        .header("Accept", "application/octet-stream")
        .bearer_auth(auth_token)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(DownloadError::Http { status, body });
    }

    let result =
        write_response_to_file(response, destination, expected_size, &mut on_progress).await;

    // Any failure past this point (network drop mid-stream, disk write error,
    // size mismatch) can leave a partial/corrupt file at `destination`. Since
    // callers (e.g. the sync orchestrator) treat "file exists" as "fully
    // cached" and skip re-downloading, an uncleaned partial file would be fed
    // straight to extraction and fail there with a confusing error instead of
    // self-healing via a clean re-download next time.
    if result.is_err() {
        tokio::fs::remove_file(destination).await.ok();
    }

    result
}

async fn write_response_to_file<F: FnMut(u64, u64)>(
    response: reqwest::Response,
    destination: &Path,
    expected_size: u64,
    on_progress: &mut F,
) -> Result<(), DownloadError> {
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(destination).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, expected_size);
    }
    file.flush().await?;

    if downloaded != expected_size {
        return Err(DownloadError::SizeMismatch {
            expected: expected_size,
            actual: downloaded,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn downloads_file_and_reports_progress() {
        let server = MockServer::start().await;
        let body = vec![7u8; 1000];
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("asset.zip");
        let http = reqwest::Client::new();
        let mut last_progress = (0u64, 0u64);

        download_with_progress(
            &http,
            &format!("{}/asset.zip", server.uri()),
            "test-token",
            &destination,
            1000,
            |downloaded, total| last_progress = (downloaded, total),
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), body);
        assert_eq!(last_progress, (1000, 1000));
    }

    #[tokio::test]
    async fn size_mismatch_deletes_the_partial_file_and_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8; 10]))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("asset.zip");
        let http = reqwest::Client::new();

        let result = download_with_progress(
            &http,
            &format!("{}/asset.zip", server.uri()),
            "test-token",
            &destination,
            999,
            |_, _| {},
        )
        .await;

        assert!(matches!(
            result,
            Err(DownloadError::SizeMismatch {
                expected: 999,
                actual: 10
            })
        ));
        assert!(!destination.exists());
    }

    #[tokio::test]
    async fn sends_a_bearer_token_and_octet_stream_accept_header() {
        // GitHub's asset-content API requires both of these to serve a
        // private repo's asset bytes -- without them it 404s instead of
        // erroring with 401/403 (masking whether the resource exists at
        // all), which is exactly the symptom this test guards against.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .and(header("Authorization", "Bearer test-token"))
            .and(header("Accept", "application/octet-stream"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8; 10]))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("asset.zip");
        let http = reqwest::Client::new();

        download_with_progress(
            &http,
            &format!("{}/asset.zip", server.uri()),
            "test-token",
            &destination,
            10,
            |_, _| {},
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), vec![1u8; 10]);
    }

    #[tokio::test]
    async fn non_success_status_errors_before_writing_the_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/asset.zip"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("asset.zip");
        let http = reqwest::Client::new();

        let result = download_with_progress(
            &http,
            &format!("{}/asset.zip", server.uri()),
            "test-token",
            &destination,
            1000,
            |_, _| {},
        )
        .await;

        assert!(matches!(
            result,
            Err(DownloadError::Http { status: 404, .. })
        ));
        assert!(!destination.exists());
    }
}
