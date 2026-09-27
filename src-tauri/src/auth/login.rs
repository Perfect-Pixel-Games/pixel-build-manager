use crate::auth::device_flow::{AuthError, DeviceFlowClient, PollOutcome};
use crate::auth::session::{save_stored_token, StoredToken};
use crate::auth::token_store::TokenStore;
use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the unix epoch")
        .as_secs()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginStatus {
    AwaitingUser {
        user_code: String,
        verification_uri: String,
    },
    Success,
    Denied,
    Expired,
    Error {
        error: String,
    },
}

pub async fn perform_device_login<F: Fn(LoginStatus)>(
    client: &DeviceFlowClient,
    token_store: &dyn TokenStore,
    on_status: F,
) -> Result<(), AuthError> {
    let device_code = match client.request_device_code().await {
        Ok(device_code) => device_code,
        Err(e) => {
            on_status(LoginStatus::Error {
                error: e.to_string(),
            });
            return Err(e);
        }
    };

    on_status(LoginStatus::AwaitingUser {
        user_code: device_code.user_code.clone(),
        verification_uri: device_code.verification_uri.clone(),
    });

    let mut interval = Duration::from_secs(device_code.interval);

    loop {
        tokio::time::sleep(interval).await;

        match client.poll_for_token(&device_code.device_code).await {
            Ok(PollOutcome::AccessToken(response)) => {
                let stored = StoredToken::from_response(response, now_secs());
                if let Err(e) = save_stored_token(token_store, &stored) {
                    let err = AuthError::UnexpectedResponse(e);
                    on_status(LoginStatus::Error {
                        error: err.to_string(),
                    });
                    return Err(err);
                }
                on_status(LoginStatus::Success);
                return Ok(());
            }
            Ok(PollOutcome::Pending) => continue,
            Ok(PollOutcome::SlowDown) => {
                interval += Duration::from_secs(5);
                continue;
            }
            Err(AuthError::AccessDenied) => {
                on_status(LoginStatus::Denied);
                return Err(AuthError::AccessDenied);
            }
            Err(AuthError::Expired) => {
                on_status(LoginStatus::Expired);
                return Err(AuthError::Expired);
            }
            Err(other) => {
                on_status(LoginStatus::Error {
                    error: other.to_string(),
                });
                return Err(other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token_store::{InMemoryTokenStore, TokenStore};
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A `TokenStore` that always fails on `save`, used to verify that
    /// a save failure still surfaces a terminal `LoginStatus::Error`.
    struct FailingTokenStore;

    impl TokenStore for FailingTokenStore {
        fn save(&self, _token: &str) -> Result<(), String> {
            Err("keyring unavailable".to_string())
        }

        fn load(&self) -> Result<Option<String>, String> {
            Ok(None)
        }

        fn clear(&self) -> Result<(), String> {
            Ok(())
        }
    }

    /// Tauri events are JSON-serialized before crossing the IPC boundary to
    /// the frontend. A bare-tuple variant like `Error(String)` cannot be
    /// represented under `#[serde(tag = "status")]` internal tagging (the
    /// payload must serialize to an object so the tag can be merged in), so
    /// `serde_json::to_value` on it fails silently at the `emit()` call site
    /// in lib.rs (`let _ = app.emit(...)`) -- the frontend never receives an
    /// error status at all. This test pins the actual wire format so that
    /// regression is caught here instead of only in production.
    #[test]
    fn error_status_serializes_to_a_taggable_json_object() {
        let value = serde_json::to_value(LoginStatus::Error {
            error: "network error: connection refused".to_string(),
        })
        .expect("LoginStatus::Error must be JSON-serializable for Tauri's emit() to work");

        assert_eq!(
            value,
            serde_json::json!({
                "status": "error",
                "error": "network error: connection refused"
            })
        );
    }

    #[tokio::test]
    async fn successful_login_saves_token_and_reports_statuses() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "devcode123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_token",
                "refresh_token": "ghr_token",
                "expires_in": 28800,
                "refresh_token_expires_in": 15811200
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let token_store = InMemoryTokenStore::new();
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();

        perform_device_login(&client, &token_store, move |status| {
            statuses_clone.lock().unwrap().push(status);
        })
        .await
        .unwrap();

        let raw = token_store.load().unwrap().unwrap();
        let stored: StoredToken = serde_json::from_str(&raw).unwrap();
        assert_eq!(stored.access_token, "ghu_token");
        assert_eq!(stored.refresh_token, "ghr_token");
        let recorded = statuses.lock().unwrap();
        assert_eq!(
            recorded[0],
            LoginStatus::AwaitingUser {
                user_code: "ABCD-1234".to_string(),
                verification_uri: "https://github.com/login/device".to_string(),
            }
        );
        assert_eq!(recorded[1], LoginStatus::Success);
    }

    #[tokio::test]
    async fn denied_login_does_not_save_a_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "devcode123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let token_store = InMemoryTokenStore::new();
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();

        let result = perform_device_login(&client, &token_store, move |status| {
            statuses_clone.lock().unwrap().push(status);
        })
        .await;

        assert!(result.is_err());
        assert_eq!(token_store.load().unwrap(), None);
        let recorded = statuses.lock().unwrap();
        assert_eq!(recorded.last(), Some(&LoginStatus::Denied));
    }

    #[tokio::test]
    async fn device_code_request_failure_reports_error_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("bad-client-id".to_string(), server.uri());
        let token_store = InMemoryTokenStore::new();
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();

        let result = perform_device_login(&client, &token_store, move |status| {
            statuses_clone.lock().unwrap().push(status);
        })
        .await;

        assert!(result.is_err());
        let recorded = statuses.lock().unwrap();
        match recorded.last() {
            Some(LoginStatus::Error { .. }) => {}
            other => panic!("expected LoginStatus::Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn save_failure_after_successful_poll_reports_error_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "devcode123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_token",
                "refresh_token": "ghr_token",
                "expires_in": 28800,
                "refresh_token_expires_in": 15811200
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let token_store = FailingTokenStore;
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();

        let result = perform_device_login(&client, &token_store, move |status| {
            statuses_clone.lock().unwrap().push(status);
        })
        .await;

        assert!(result.is_err());
        let recorded = statuses.lock().unwrap();
        match recorded.last() {
            Some(LoginStatus::Error { .. }) => {}
            other => panic!("expected LoginStatus::Error, got {:?}", other),
        }
    }
}
