use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteTimeouts {
    pub request: Option<Duration>,
    pub backend_request: Option<Duration>,
}
