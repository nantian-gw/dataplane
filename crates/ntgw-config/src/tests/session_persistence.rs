use crate::{DataPlaneConfig, SessionPersistenceConfig};

#[test]
fn resolve_shared_secret_returns_sha256_hash_for_configured_secret() {
    let cfg = SessionPersistenceConfig {
        shared_secret: Some("my-shared-secret".to_string()),
        ..SessionPersistenceConfig::default()
    };
    let result = cfg.resolve_shared_secret();
    assert!(result.is_some());
    let hash = result.unwrap();
    assert_eq!(hash.len(), 32);

    let result2 = cfg.resolve_shared_secret();
    assert!(result2.is_some());
    assert_eq!(result2.unwrap(), hash);
}

#[test]
fn resolve_shared_secret_returns_none_when_not_configured() {
    let cfg = SessionPersistenceConfig::default();
    assert!(cfg.resolve_shared_secret().is_none());
}

#[test]
fn resolve_shared_secret_returns_none_for_empty_string() {
    let cfg = SessionPersistenceConfig {
        shared_secret: Some("".to_string()),
        ..SessionPersistenceConfig::default()
    };
    assert!(cfg.resolve_shared_secret().is_none());
}

#[test]
fn resolve_shared_secret_returns_none_for_whitespace_only() {
    let cfg = SessionPersistenceConfig {
        shared_secret: Some("   ".to_string()),
        ..SessionPersistenceConfig::default()
    };
    assert!(cfg.resolve_shared_secret().is_none());
}

#[test]
fn resolve_shared_secret_different_secrets_produce_different_hashes() {
    let cfg1 = SessionPersistenceConfig {
        shared_secret: Some("secret-a".to_string()),
        ..SessionPersistenceConfig::default()
    };
    let cfg2 = SessionPersistenceConfig {
        shared_secret: Some("secret-b".to_string()),
        ..SessionPersistenceConfig::default()
    };
    assert_ne!(cfg1.resolve_shared_secret(), cfg2.resolve_shared_secret());
}

#[test]
fn resolve_shared_secret_parses_from_yaml() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
session_persistence:
  shared_secret: "yaml-secret"
"#,
    )
    .expect("config should parse");
    let result = cfg.session_persistence.resolve_shared_secret();
    assert!(result.is_some());
    assert_eq!(result.unwrap().len(), 32);
}

#[test]
fn config_without_shared_secret_still_resolves_secret_key() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
session_persistence:
  secret_key: "inline-key"
"#,
    )
    .expect("config should parse");
    assert!(cfg.session_persistence.resolve_shared_secret().is_none());
    let secret = cfg
        .session_persistence
        .resolve_secret()
        .expect("resolve_secret");
    assert_eq!(secret, Some("inline-key".to_string().into_bytes()));
}
