use aeg_ai::keyring::ApiKeyManager;

#[test]
fn test_resolve_basic() {
    let mgr = ApiKeyManager::new();
    mgr.rotate("gw-key-1", "sk-backend-123".into(), 0);

    let cred = mgr.resolve("gw-key-1").unwrap();
    assert_eq!(cred.api_key, "sk-backend-123");
    assert_eq!(cred.priority, 0);
}

#[test]
fn test_resolve_nonexistent() {
    let mgr = ApiKeyManager::new();
    assert!(mgr.resolve("nonexistent").is_none());
}

#[test]
fn test_rotation_prefers_lower_priority() {
    let mgr = ApiKeyManager::new();
    mgr.rotate("gw-key-1", "sk-old-key".into(), 10);
    mgr.rotate("gw-key-1", "sk-new-key".into(), 0);

    let cred = mgr.resolve("gw-key-1").unwrap();
    assert_eq!(cred.api_key, "sk-new-key");
}

#[test]
fn test_revoke() {
    let mgr = ApiKeyManager::new();
    mgr.rotate("gw-key-1", "sk-old".into(), 0);
    assert!(mgr.revoke("gw-key-1", "sk-old"));
    assert!(mgr.resolve("gw-key-1").is_none());
}

#[test]
fn test_noop_when_empty() {
    let mgr = ApiKeyManager::new();
    assert!(mgr.is_empty());
}
