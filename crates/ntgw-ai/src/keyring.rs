use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use zeroize::Zeroize;

#[derive(Debug, Clone, Zeroize)]
pub struct BackendCredential {
    pub api_key: String,
    pub expires_at: Option<String>, // ISO 8601 timestamp
    pub priority: u8,               // lower = preferred
}

impl Drop for BackendCredential {
    fn drop(&mut self) {
        self.api_key.zeroize();
    }
}

#[derive(Debug, Clone)]
pub struct ApiKeyManager {
    keys: Arc<RwLock<HashMap<String, Vec<BackendCredential>>>>,
}

impl ApiKeyManager {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Look up best backend key for a gateway key + provider combination.
    /// Returns the credential with lowest priority (preferred).
#[must_use]
    pub fn resolve(&self, gateway_key: &str) -> Option<BackendCredential> {
        let keys = self.keys.read();
        let creds = keys.get(gateway_key)?;
        // Return the one with lowest priority (preferred)
        creds.iter().min_by_key(|c| c.priority).cloned()
    }

    /// Rotate keys: add a new key with lower priority (becomes preferred).
    /// Expired credentials are removed before the new key is added.
    pub fn rotate(&self, gateway_key: &str, new_key: String, priority: u8) {
        let mut keys = self.keys.write();
        let entry = keys.entry(gateway_key.to_string()).or_default();
        entry.retain(|c| c.expires_at.is_none());
        let cred = BackendCredential {
            api_key: new_key,
            expires_at: None,
            priority,
        };
        entry.push(cred);
    }

    /// Remove a specific key.
    pub fn revoke(&self, gateway_key: &str, api_key: &str) -> bool {
        let mut keys = self.keys.write();
        if let Some(creds) = keys.get_mut(gateway_key) {
            let before = creds.len();
            creds.retain(|c| c.api_key != api_key);
            creds.len() != before
        } else {
            false
        }
    }

    /// Reload keys from external config map.
    pub fn reload(&self, key_map: HashMap<String, Vec<BackendCredential>>) {
        let mut keys = self.keys.write();
        *keys = key_map;
    }

    /// Check if any keys are configured.
    pub fn is_empty(&self) -> bool {
        self.keys.read().is_empty()
    }
}

impl Default for ApiKeyManager {
    fn default() -> Self {
        Self::new()
    }
}
