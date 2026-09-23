use crate::auth::device_flow::{DeviceFlowClient, TokenResponse};
use crate::auth::token_store::TokenStore;
use serde::{Deserialize, Serialize};

/// Returned by `ensure_valid_access_token` (and surfaced verbatim as a Tauri
/// command error string) when the session can no longer be refreshed and
/// the user needs to log in again. The frontend matches on this exact
/// string to route back to the Login screen instead of silently showing
/// stale/empty data.
pub const SESSION_EXPIRED: &str = "session_expired";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: u64,
    pub refresh_token_expires_at: u64,
}

impl StoredToken {
    pub(crate) fn from_response(response: TokenResponse, now: u64) -> Self {
        Self {
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            access_token_expires_at: now + response.expires_in,
            refresh_token_expires_at: now + response.refresh_token_expires_in,
        }
    }
}

pub fn save_stored_token(store: &dyn TokenStore, token: &StoredToken) -> Result<(), String> {
    let json = serde_json::to_string(token).map_err(|e| e.to_string())?;
    store.save(&json)
}

/// Returns a currently-valid access token, transparently refreshing it via
/// `client` if it has expired. If nothing is stored, the stored data can't
/// be parsed (e.g. a pre-refresh-token-era plain string), or the refresh
/// token itself is expired or rejected by GitHub, the stored credentials
/// are cleared and `SESSION_EXPIRED` is returned.
pub async fn ensure_valid_access_token(
    client: &DeviceFlowClient,
    store: &dyn TokenStore,
    now: u64,
) -> Result<String, String> {
    let Some(raw) = store.load()? else {
        return Err(SESSION_EXPIRED.to_string());
    };
    let Ok(stored) = serde_json::from_str::<StoredToken>(&raw) else {
        let _ = store.clear();
        return Err(SESSION_EXPIRED.to_string());
    };

    if stored.access_token_expires_at > now {
        return Ok(stored.access_token);
    }

    if stored.refresh_token_expires_at <= now {
        let _ = store.clear();
        return Err(SESSION_EXPIRED.to_string());
    }

    match client.refresh_access_token(&stored.refresh_token).await {
        Ok(response) => {
            let refreshed = StoredToken::from_response(response, now);
            save_stored_token(store, &refreshed)?;
            Ok(refreshed.access_token)
        }
        Err(_) => {
            let _ = store.clear();
            Err(SESSION_EXPIRED.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::device_flow::DeviceFlowClient;
    use crate::auth::token_store::{InMemoryTokenStore, TokenStore};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn seed(store: &InMemoryTokenStore, token: &StoredToken) {
        save_stored_token(store, token).unwrap();
    }

    #[tokio::test]
    async fn returns_the_stored_access_token_when_not_expired() {
        let server = MockServer::start().await;
        let store = InMemoryTokenStore::new();
        seed(
            &store,
            &StoredToken {
                access_token: "ghu_current".to_string(),
                refresh_token: "ghr_current".to_string(),
                access_token_expires_at: 1_100,
                refresh_token_expires_at: 100_000,
            },
        );
        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let token = ensure_valid_access_token(&client, &store, 1_000).await.unwrap();

        assert_eq!(token, "ghu_current");
    }

    #[tokio::test]
    async fn refreshes_when_the_access_token_is_expired_but_the_refresh_token_is_still_valid() {
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
        let store = InMemoryTokenStore::new();
        seed(
            &store,
            &StoredToken {
                access_token: "ghu_stale".to_string(),
                refresh_token: "ghr_current".to_string(),
                access_token_expires_at: 900,
                refresh_token_expires_at: 100_000,
            },
        );
        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let token = ensure_valid_access_token(&client, &store, 1_000).await.unwrap();

        assert_eq!(token, "ghu_new");
        let raw = store.load().unwrap().unwrap();
        let stored: StoredToken = serde_json::from_str(&raw).unwrap();
        assert_eq!(stored.access_token, "ghu_new");
        assert_eq!(stored.refresh_token, "ghr_new");
        assert_eq!(stored.access_token_expires_at, 1_000 + 28800);
        assert_eq!(stored.refresh_token_expires_at, 1_000 + 15811200);
    }

    #[tokio::test]
    async fn clears_the_store_and_returns_session_expired_when_the_refresh_token_has_expired() {
        let server = MockServer::start().await;
        let store = InMemoryTokenStore::new();
        seed(
            &store,
            &StoredToken {
                access_token: "ghu_stale".to_string(),
                refresh_token: "ghr_stale".to_string(),
                access_token_expires_at: 900,
                refresh_token_expires_at: 950,
            },
        );
        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = ensure_valid_access_token(&client, &store, 1_000).await;

        assert_eq!(result, Err(SESSION_EXPIRED.to_string()));
        assert_eq!(store.load().unwrap(), None);
    }

    #[tokio::test]
    async fn clears_the_store_and_returns_session_expired_when_github_rejects_the_refresh_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "bad_refresh_token",
                "error_description": "The refresh token passed is incorrect or expired."
            })))
            .mount(&server)
            .await;
        let store = InMemoryTokenStore::new();
        seed(
            &store,
            &StoredToken {
                access_token: "ghu_stale".to_string(),
                refresh_token: "ghr_revoked".to_string(),
                access_token_expires_at: 900,
                refresh_token_expires_at: 100_000,
            },
        );
        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = ensure_valid_access_token(&client, &store, 1_000).await;

        assert_eq!(result, Err(SESSION_EXPIRED.to_string()));
        assert_eq!(store.load().unwrap(), None);
    }

    #[tokio::test]
    async fn returns_session_expired_when_nothing_is_stored() {
        let server = MockServer::start().await;
        let store = InMemoryTokenStore::new();
        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = ensure_valid_access_token(&client, &store, 1_000).await;

        assert_eq!(result, Err(SESSION_EXPIRED.to_string()));
    }

    #[tokio::test]
    async fn returns_session_expired_and_clears_unparseable_legacy_data() {
        let server = MockServer::start().await;
        let store = InMemoryTokenStore::new();
        store.save("gho_legacyplaintoken").unwrap();
        let client = DeviceFlowClient::with_base_url("client-id".to_string(), server.uri());

        let result = ensure_valid_access_token(&client, &store, 1_000).await;

        assert_eq!(result, Err(SESSION_EXPIRED.to_string()));
        assert_eq!(store.load().unwrap(), None);
    }
}
