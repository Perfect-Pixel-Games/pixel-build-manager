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
    AccessToken(String),
    Pending,
    SlowDown,
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
            .form(&[("client_id", self.client_id.as_str()), ("scope", "repo read:org")])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::UnexpectedResponse(format!("HTTP {status}: {body}")));
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
            return Err(AuthError::UnexpectedResponse(format!("HTTP {status}: {body}")));
        }

        let raw: RawResponse = response
            .json()
            .await
            .map_err(|e| AuthError::UnexpectedResponse(e.to_string()))?;

        if let Some(token) = raw.access_token {
            return Ok(PollOutcome::AccessToken(token));
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
                "access_token": "ghu_token"
            })))
            .mount(&server)
            .await;

        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let first = client.poll_for_token("devcode123").await.unwrap();
        assert_eq!(first, PollOutcome::Pending);

        let second = client.poll_for_token("devcode123").await.unwrap();
        assert_eq!(second, PollOutcome::AccessToken("ghu_token".to_string()));
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
