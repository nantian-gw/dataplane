use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::BackendEndpoint;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionPersistence {
    pub session_name: String,
    pub session_type: String,
    pub absolute_timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub cookie: Option<CookieConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CookieConfig {
    pub lifetime_type: String,
}

#[derive(Debug, Clone)]
pub struct PersistentSessionTarget {
    pub backend_name: String,
    pub endpoint: BackendEndpoint,
}

impl SessionPersistence {
    pub fn is_cookie(&self) -> bool {
        self.session_type != "Header"
    }

    pub fn cookie_lifetime_type(&self) -> &str {
        self.cookie
            .as_ref()
            .map(|cookie| cookie.lifetime_type.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Session")
    }
}
