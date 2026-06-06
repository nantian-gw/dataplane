use ntgw_ai::multitenant::{Tenant, TenantManager, TenantQuota};

fn tenant_with_limits(id: &str, keys: Vec<&str>) -> Tenant {
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
    let manager = TenantManager::new(vec![tenant_with_limits("tenant-1", vec!["sk-alice"])]);
    let tenant = manager.resolve("sk-alice");
    assert!(tenant.is_some());
    assert_eq!(tenant.unwrap().tenant_id, "tenant-1");
}

#[test]
fn unknown_key_returns_none() {
    let manager = TenantManager::new(vec![tenant_with_limits("tenant-1", vec!["sk-alice"])]);
    assert!(manager.resolve("sk-unknown").is_none());
}

#[test]
fn quota_exceeded_returns_false() {
    let mut tenant = tenant_with_limits("tenant-1", vec!["sk-alice"]);
    tenant.quota.tokens_per_minute = 100;
    tenant.quota.tokens_per_day = 0; // disable per-day
    tenant.quota.requests_per_minute = 0; // disable per-request
    let manager = TenantManager::new(vec![tenant]);

    assert!(manager.check_quota("tenant-1", 80));
    assert!(manager.check_quota("tenant-1", 20));
    assert!(!manager.check_quota("tenant-1", 50));
}

#[test]
fn model_not_allowed_returns_false() {
    let tenant = tenant_with_limits("tenant-1", vec!["sk-alice"]);
    let manager = TenantManager::new(vec![tenant]);
    assert!(!manager.check_model_access("tenant-1", "gemini-pro"));
}
