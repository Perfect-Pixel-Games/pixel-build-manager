use keyring::Entry;

const SERVICE_NAME: &str = "pixel-build-manager";
const USERNAME: &str = "github-token";

pub trait TokenStore: Send + Sync {
    fn save(&self, token: &str) -> Result<(), String>;
    fn load(&self) -> Result<Option<String>, String>;
    fn clear(&self) -> Result<(), String>;
}

pub struct KeyringTokenStore;

impl TokenStore for KeyringTokenStore {
    fn save(&self, token: &str) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME).map_err(|e| e.to_string())?;
        entry.set_password(token).map_err(|e| e.to_string())
    }

    fn load(&self) -> Result<Option<String>, String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn clear(&self) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
pub struct InMemoryTokenStore {
    token: std::sync::Mutex<Option<String>>,
}

#[cfg(test)]
impl InMemoryTokenStore {
    pub fn new() -> Self {
        Self {
            token: std::sync::Mutex::new(None),
        }
    }
}

#[cfg(test)]
impl TokenStore for InMemoryTokenStore {
    fn save(&self, token: &str) -> Result<(), String> {
        *self.token.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.token.lock().unwrap().clone())
    }

    fn clear(&self) -> Result<(), String> {
        *self.token.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_a_token() {
        let store = InMemoryTokenStore::new();
        assert_eq!(store.load().unwrap(), None);

        store.save("abc123").unwrap();
        assert_eq!(store.load().unwrap(), Some("abc123".to_string()));

        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }
}
