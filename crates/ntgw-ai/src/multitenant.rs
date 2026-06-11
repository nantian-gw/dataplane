use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Per-tenant resource quota.
#[derive(Debug, Clone, Default)]
pub struct TenantQuota {
    pub tokens_per_minute: u64,
    pub tokens_per_day: u64,
    pub requests_per_minute: u64,
}

/// A tenant represents an isolated user/organisation within the AI Gateway.
#[derive(Debug, Clone)]
pub struct Tenant {
    pub tenant_id: String,
    pub api_keys: Vec<String>,
    pub quota: TenantQuota,
    pub allowed_models: Vec<String>,
    /// Monthly cost cap in dollars. `None` means no limit.
    pub cost_limit: Option<f64>,
}

// ── internal quota tracking state ─────────────────────────────────

struct TenantQuotaState {
    minute_tokens: u64,
    day_tokens: u64,
    minute_requests: u64,
    minute_reset: Instant,
    day_reset: Instant,
}

impl TenantQuotaState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            minute_tokens: 0,
            day_tokens: 0,
            minute_requests: 0,
            minute_reset: now + Duration::from_secs(60),
            day_reset: now + Duration::from_secs(86400),
        }
    }

    /// Reset expired windows and increment.
    /// Returns `true` if the operation stays within the tenant's quota limits.
    fn consume(&mut self, quota: &TenantQuota, tokens: u64) -> bool {
        let now = Instant::now();

        // reset minute window if expired
        if now >= self.minute_reset {
            self.minute_tokens = 0;
            self.minute_requests = 0;
            self.minute_reset = now + Duration::from_secs(60);
        }
        // reset day window if expired
        if now >= self.day_reset {
            self.day_tokens = 0;
            self.day_reset = now + Duration::from_secs(86400);
        }

        // check limits
        let new_minute_tokens = self.minute_tokens.saturating_add(tokens);
        let new_day_tokens = self.day_tokens.saturating_add(tokens);
        let new_minute_requests = self.minute_requests.saturating_add(1);

        if quota.tokens_per_minute > 0 && new_minute_tokens > quota.tokens_per_minute {
            return false;
        }
        if quota.tokens_per_day > 0 && new_day_tokens > quota.tokens_per_day {
            return false;
        }
        if quota.requests_per_minute > 0 && new_minute_requests > quota.requests_per_minute {
            return false;
        }

        self.minute_tokens = new_minute_tokens;
        self.day_tokens = new_day_tokens;
        self.minute_requests = new_minute_requests;
        true
    }
}

// ── TenantManager ─────────────────────────────────────────────────

/// Manages tenant registry, API key resolution, and quota enforcement.
pub struct TenantManager {
    /// Tenant definitions keyed by tenant_id.
    tenants: HashMap<String, Tenant>,
    /// Maps API key → tenant_id for O(1) lookup.
    api_key_index: HashMap<String, String>,
    /// Runtime quota tracking per tenant_id.
    quota_state: DashMap<String, TenantQuotaState>,
    /// Counter for opportunistic stale-entry cleanup.
    check_count: AtomicU64,
}

/// Cleanup is triggered every N check_quota calls.
const CLEANUP_INTERVAL: u64 = 1000;

impl TenantManager {
    /// Create a new `TenantManager` pre-populated with the given tenants.
    pub fn new(tenants: Vec<Tenant>) -> Self {
        let mut tenant_map = HashMap::new();
        let mut key_index = HashMap::new();

        for t in tenants {
            for key in &t.api_keys {
                key_index.insert(key.clone(), t.tenant_id.clone());
            }
            tenant_map.insert(t.tenant_id.clone(), t);
        }

        Self {
            tenants: tenant_map,
            api_key_index: key_index,
            quota_state: DashMap::new(),
            check_count: AtomicU64::new(0),
        }
    }

    /// Resolve an API key to its owning `Tenant`.
    ///
    /// Returns `None` if the key is unknown.
    pub fn resolve(&self, api_key: &str) -> Option<&Tenant> {
        let tid = self.api_key_index.get(api_key)?;
        self.tenants.get(tid)
    }

    /// Remove quota state entries for tenants no longer in the registry.
    fn maybe_cleanup(&self) {
        let count = self.check_count.fetch_add(1, Ordering::Relaxed);
        if !count.is_multiple_of(CLEANUP_INTERVAL) {
            return;
        }
        self.quota_state
            .retain(|tid, _| self.tenants.contains_key(tid));
    }

    /// Cleanup exposed for testing.
    #[doc(hidden)]
    pub fn force_cleanup(&self) {
        self.quota_state
            .retain(|tid, _| self.tenants.contains_key(tid));
    }

    /// Check whether the tenant has sufficient token quota.
    ///
    /// `tokens` is the estimated token count for the current request.
    /// Returns `true` if the request fits within the tenant's limits.
    pub fn check_quota(&self, tenant_id: &str, tokens: u64) -> bool {
        let tenant = match self.tenants.get(tenant_id) {
            Some(t) => t,
            None => return false,
        };

        // 0-values mean no limit
        if tenant.quota.tokens_per_minute == 0
            && tenant.quota.tokens_per_day == 0
            && tenant.quota.requests_per_minute == 0
        {
            return true;
        }

        self.maybe_cleanup();
        let mut entry = self
            .quota_state
            .entry(tenant_id.to_owned())
            .or_insert_with(TenantQuotaState::new);
        entry.consume(&tenant.quota, tokens)
    }

    /// Check whether the tenant is allowed to access the given model.
    ///
    /// Returns `true` if the model is in the tenant's allow list or the
    /// allow list is empty (meaning any model is permitted).
    pub fn check_model_access(&self, tenant_id: &str, model: &str) -> bool {
        let tenant = match self.tenants.get(tenant_id) {
            Some(t) => t,
            None => return false,
        };

        // An empty allow list means all models are allowed.
        if tenant.allowed_models.is_empty() {
            return true;
        }

        tenant.allowed_models.iter().any(|m| m == model)
    }

    /// Check whether the tenant's accumulated cost exceeds its monthly cap.
    ///
    /// `current_cost` is the tenant's accumulated cost in dollars.
    /// Returns `true` if the cost is within the limit (or no limit is set).
    pub fn check_cost_limit(&self, tenant_id: &str, current_cost: f64) -> bool {
        let tenant = match self.tenants.get(tenant_id) {
            Some(t) => t,
            None => return false,
        };

        match tenant.cost_limit {
            Some(limit) => current_cost <= limit,
            None => true,
        }
    }

    /// Returns the total number of registered tenants.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.tenants.len()
    }

    /// Returns `true` if no tenants are registered.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tenant(id: &str, keys: Vec<&str>) -> Tenant {
        Tenant {
            tenant_id: id.to_string(),
            api_keys: keys.into_iter().map(|s| s.to_string()).collect(),
            quota: TenantQuota {
                tokens_per_minute: 100,
                tokens_per_day: 10_000,
                requests_per_minute: 50,
            },
            allowed_models: vec!["gpt-4".into(), "claude-3".into()],
            cost_limit: Some(100.0),
        }
    }

    #[test]
    fn resolve_by_api_key_succeeds() {
        let manager = TenantManager::new(vec![test_tenant("t1", vec!["key-abc"])]);
        let tenant = manager.resolve("key-abc");
        assert!(tenant.is_some());
        assert_eq!(tenant.unwrap().tenant_id, "t1");
    }

    #[test]
    fn unknown_key_returns_none() {
        let manager = TenantManager::new(vec![test_tenant("t1", vec!["key-abc"])]);
        assert!(manager.resolve("key-unknown").is_none());
    }

    #[test]
    fn quota_exceeded_returns_false() {
        let mut t = test_tenant("t1", vec!["key-abc"]);
        t.quota.tokens_per_minute = 100;
        let manager = TenantManager::new(vec![t]);

        // first request under limit
        assert!(manager.check_quota("t1", 80));
        // second request — 80 + 30 = 110 > 100 → exceeded
        assert!(!manager.check_quota("t1", 30));
    }

    #[test]
    fn model_not_in_allowed_list_returns_false() {
        let t = test_tenant("t1", vec!["key-abc"]);
        let manager = TenantManager::new(vec![t]);
        assert!(!manager.check_model_access("t1", "gemini-pro"));
    }

    #[test]
    fn model_in_allowed_list_returns_true() {
        let t = test_tenant("t1", vec!["key-abc"]);
        let manager = TenantManager::new(vec![t]);
        assert!(manager.check_model_access("t1", "gpt-4"));
    }

    #[test]
    fn cost_limit_exceeded_returns_false() {
        let t = test_tenant("t1", vec!["key-abc"]);
        let manager = TenantManager::new(vec![t]);
        assert!(manager.check_cost_limit("t1", 50.0));
        assert!(!manager.check_cost_limit("t1", 150.0));
    }
}
