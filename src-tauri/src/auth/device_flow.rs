use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DeviceFlowClient {
    http: reqwest::Client,
    base_url: String,
    client_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("access denied")]
    AccessDenied,
    #[error("device code expired")]
    Expired,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PollOutcome {
    AccessToken(TokenResponse),
    Pending,
    SlowDown,
}

/// A GitHub OAuth token response. `refresh_token`/`expires_in`/
/// `refresh_token_expires_in` are only present because this app's OAuth
/// client has "token expiration" enabled -- without that setting GitHub
/// omits them and issues a non-expiring token instead. Treating them as
/// required keeps that assumption explicit: if the setting is ever turned
/// off, login fails loudly instead of silently mis-tracking expiry.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub refresh_token_expires_in: u64,
}

impl DeviceFlowClient {
    pub fn new(client_id: String) -> Self {
        Self::with_base_url(client_id, "https://github.com".to_string())
    }

    pub fn with_base_url(client_id: String, base_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            client_id,
        }
    }

    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse, AuthError> {
        let url = format!("{}/login/device/code", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("scope", "repo read:org"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::UnexpectedResponse(format!(
                "HTTP {status}: {body}"
            )));
        }

        response
            .json::<DeviceCodeResponse>()
            .await
            .map_err(|e| AuthError::UnexpectedResponse(e.to_string()))
    }

    pub async fn poll_for_token(&self, device_code: &str) -> Result<PollOutcome, AuthError> {
        #[derive(Deserialize)]
        struct RawResponse {
            access_token: Option<String>,
            refresh_token: Option<String>,
            expires_in: Option<u64>,
            refresh_token_expires_in: Option<u64>,
            error: Option<String>,
        }

        let url = format!("{}/login/oauth/access_token", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::UnexpectedResponse(format!(
                "HTTP {status}: {body}"
            )));
        }

        let raw: RawResponse = response
            .json()
            .await
            .map_err(|e| AuthError::UnexpectedResponse(e.to_string()))?;

        if let Some(access_token) = raw.access_token {
            let (Some(refresh_token), Some(expires_in), Some(refresh_token_expires_in)) = (
                raw.refresh_token,
                raw.expires_in,
                raw.refresh_token_expires_in,
            ) else {
                return Err(AuthError::UnexpectedResponse(
                    "access_token response is missing refresh_token/expires_in fields".to_string(),
                ));
            };
            return Ok(PollOutcome::AccessToken(TokenResponse {
                access_token,
                refresh_token,
                expires_in,
                refresh_token_expires_in,
            }));
        }

        match raw.error.as_deref() {
            Some("authorization_pending") => Ok(PollOutcome::Pending),
            Some("slow_down") => Ok(PollOutcome::SlowDown),
            Some("access_denied") => Err(AuthError::AccessDenied),
            Some("expired_token") => Err(AuthError::Expired),
            Some(other) => Err(AuthError::UnexpectedResponse(other.to_string())),
            None => Err(AuthError::UnexpectedResponse(
                "no access_token or error in response".to_string(),
            )),
        }
    }

    /// Exchanges a still-valid refresh token for a new access/refresh token
    /// pair. GitHub rotates refresh tokens on every use (the old one stops
    /// working), so callers must persist the returned `refresh_token`, not
    /// reuse the one passed in.
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenResponse, AuthError> {
        #[derive(Deserialize)]
        struct RawResponse {
            access_token: Option<String>,
            refresh_token: Option<String>,
            expires_in: Option<u64>,
            refresh_token_expires_in: Option<u64>,
            error: Option<String>,
            error_description: Option<String>,
        }

        let url = format!("{}/login/oauth/access_token", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::UnexpectedResponse(format!(
                "HTTP {status}: {body}"
            )));
        }

        let raw: RawResponse = response
            .json()
            .await
            .map_err(|e| AuthError::UnexpectedResponse(e.to_string()))?;

        if let (
            Some(access_token),
            Some(refresh_token),
            Some(expires_in),
            Some(refresh_token_expires_in),
        ) = (
            raw.access_token,
            raw.refresh_token,
            raw.expires_in,
            raw.refresh_token_expires_in,
        ) {
            return Ok(TokenResponse {
                access_token,
                refresh_token,
                expires_in,
                refresh_token_expires_in,
            });
        }

        Err(AuthError::UnexpectedResponse(
            raw.error_description
                .or(raw.error)
                .unwrap_or_else(|| "refresh response is missing token fields".to_string()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn request_device_code_parses_response() {
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

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());
        let response = client.request_device_code().await.unwrap();

        assert_eq!(response.device_code, "devcode123");
        assert_eq!(response.user_code, "ABCD-1234");
    }

    #[tokio::test]
    async fn poll_for_token_returns_pending_then_access_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "authorization_pending"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_token",
                "refresh_token": "ghr_refresh",
                "expires_in": 28800,
                "refresh_token_expires_in": 15811200
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let first = client.poll_for_token("devcode123").await.unwrap();
        assert_eq!(first, PollOutcome::Pending);

        let second = client.poll_for_token("devcode123").await.unwrap();
        assert_eq!(
            second,
            PollOutcome::AccessToken(TokenResponse {
                access_token: "ghu_token".to_string(),
                refresh_token: "ghr_refresh".to_string(),
                expires_in: 28800,
                refresh_token_expires_in: 15811200,
            })
        );
    }

    #[tokio::test]
    async fn poll_for_token_errors_when_access_token_response_is_missing_refresh_fields() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_token"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = client.poll_for_token("devcode123").await;

        assert!(matches!(result, Err(AuthError::UnexpectedResponse(_))));
    }

    #[tokio::test]
    async fn refresh_access_token_returns_new_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_new",
                "refresh_token": "ghr_new",
                "expires_in": 28800,
                "refresh_token_expires_in": 15811200
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let response = client.refresh_access_token("ghr_old").await.unwrap();

        assert_eq!(
            response,
            TokenResponse {
                access_token: "ghu_new".to_string(),
                refresh_token: "ghr_new".to_string(),
                expires_in: 28800,
                refresh_token_expires_in: 15811200,
            }
        );
    }

    #[tokio::test]
    async fn refresh_access_token_maps_a_rejected_refresh_token_to_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "bad_refresh_token",
                "error_description": "The refresh token passed is incorrect or expired."
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = client.refresh_access_token("ghr_old").await;

        assert!(matches!(result, Err(AuthError::UnexpectedResponse(_))));
    }

    #[tokio::test]
    async fn poll_for_token_maps_access_denied_to_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = client.poll_for_token("devcode123").await;

        assert!(matches!(result, Err(AuthError::AccessDenied)));
    }

    #[tokio::test]
    async fn poll_for_token_maps_slow_down_to_outcome() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "slow_down"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = client.poll_for_token("devcode123").await.unwrap();

        assert_eq!(result, PollOutcome::SlowDown);
    }

    #[tokio::test]
    async fn poll_for_token_maps_expired_token_to_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "expired_token"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = client.poll_for_token("devcode123").await;

        assert!(matches!(result, Err(AuthError::Expired)));
    }
}
