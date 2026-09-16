use crate::auth::device_flow::{AuthError, DeviceFlowClient, PollOutcome};
use crate::auth::token_store::TokenStore;
use serde::Serialize;
use std::time::Duration;

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
    Error(String),
}

pub async fn perform_device_login<F: Fn(LoginStatus)>(
    client: &DeviceFlowClient,
    token_store: &dyn TokenStore,
    on_status: F,
) -> Result<(), AuthError> {
    let device_code = client.request_device_code().await?;

    on_status(LoginStatus::AwaitingUser {
        user_code: device_code.user_code.clone(),
        verification_uri: device_code.verification_uri.clone(),
    });

    let mut interval = Duration::from_secs(device_code.interval);

    loop {
        tokio::time::sleep(interval).await;

        match client.poll_for_token(&device_code.device_code).await {
            Ok(PollOutcome::AccessToken(token)) => {
                if let Err(e) = token_store.save(&token) {
                    let err = AuthError::UnexpectedResponse(e);
                    on_status(LoginStatus::Error(err.to_string()));
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
                on_status(LoginStatus::Error(other.to_string()));
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
                "access_token": "ghu_token"
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

        assert_eq!(token_store.load().unwrap(), Some("ghu_token".to_string()));
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
                "access_token": "ghu_token"
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
            Some(LoginStatus::Error(_)) => {}
            other => panic!("expected LoginStatus::Error, got {:?}", other),
        }
    }
}
