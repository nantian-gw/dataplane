use super::*;
use std::path::Path;

#[test]
fn parses_sample_yaml() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
"#,
    )
    .expect("config should parse");

    assert_eq!(cfg.runtime.http_listen_addr, "0.0.0.0:80");
    assert!(cfg.access_log.enabled);
    assert_eq!(cfg.access_log.path, "/var/log/nantian-gw/access.log");
    assert_eq!(cfg.access_log.mode, "json");
    assert_eq!(cfg.access_log.sample_rate, 0.5);
    assert!(cfg.access_log.format.contains("%REQUEST%"));
    assert!(cfg.access_log.formats.is_empty());
    assert!(cfg.access_log.format_name.is_empty());
    assert_eq!(
        cfg.access_log.route_annotation_prefix,
        "gateway.nantian.dev/access-log-"
    );
    assert!(cfg.xds_tls.enabled());
}

#[test]
fn bundled_dataplane_configs_match_schema() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    for relative_path in [
        "configs/dataplane/config.yaml",
        "configs/dataplane/config.production.yaml",
    ] {
        let path = repo_root.join(relative_path);
        let raw = fs::read_to_string(&path).expect("bundled dataplane config should be readable");
        let cfg = serde_yaml::from_str::<DataPlaneConfig>(&raw)
            .unwrap_or_else(|err| panic!("{relative_path} should match dataplane schema: {err}"));
        assert!(
            cfg.access_log.formats.is_empty(),
            "{relative_path} should keep accessLog.formats empty when omitted"
        );
        assert!(
            cfg.access_log.format_name.is_empty(),
            "{relative_path} should keep accessLog.formatName empty when omitted"
        );
    }
}

#[test]
fn bundled_production_config_enables_xds_mtls() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let path = repo_root.join("configs/dataplane/config.production.yaml");
    let raw = fs::read_to_string(&path).expect("production config should be readable");
    let cfg: DataPlaneConfig = serde_yaml::from_str(&raw).expect("production config should parse");

    assert!(cfg.control_plane_addr.starts_with("https://"));
    assert!(cfg.xds_tls.enabled());
    assert!(!cfg.xds_tls.ca_path.trim().is_empty());
    assert!(!cfg.xds_tls.cert_path.trim().is_empty());
    assert!(!cfg.xds_tls.key_path.trim().is_empty());
    assert!(!cfg.xds_tls.domain_name.trim().is_empty());
    assert!(!cfg.admin_auth.bearer_token_file.trim().is_empty());
}

#[test]
fn bundled_dataplane_config_enables_runtime_protection() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let path = repo_root.join("configs/dataplane/config.yaml");
    let raw = fs::read_to_string(&path).expect("bundled dataplane config should be readable");
    let cfg: DataPlaneConfig =
        serde_yaml::from_str(&raw).expect("bundled dataplane config should parse");
    let protection = cfg.runtime_protection;

    assert!(protection.http_global_inflight_limit > 0);
    assert!(protection.http_listener_inflight_limit > 0);
    assert!(protection.http_route_inflight_limit > 0);
    assert!(protection.http_backend_circuit_breaker_max_requests > 0);
    assert!(protection.http_global_rate_limit_requests_per_second > 0);
    assert!(protection.http_global_rate_limit_burst > 0);
    assert!(protection.http_listener_rate_limit_requests_per_second > 0);
    assert!(protection.http_listener_rate_limit_burst > 0);
    assert!(protection.http_route_rate_limit_requests_per_second > 0);
    assert!(protection.http_route_rate_limit_burst > 0);
    assert_eq!(protection.http_max_request_body_bytes, 10 * 1024 * 1024);
    assert_eq!(protection.http_max_request_header_bytes, 65_536);
    assert!(protection.tcp_global_connection_limit > 0);
    assert!(protection.tcp_listener_connection_limit > 0);
    assert!(protection.udp_global_datagram_limit > 0);
    assert!(protection.udp_listener_datagram_limit > 0);
}

#[test]
fn reloading_data_plane_config_reloads_after_file_change() {
    let dir = tempfile_dir();
    let config_path = dir.join("config.yaml");
    fs::write(
        &config_path,
        r#"
nodeId: dp-old
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeTuning:
  requestMirrorMaxConcurrency: 8
"#,
    )
    .expect("initial config should be written");

    let config = super::super::ReloadingDataPlaneConfig::new(&config_path, Duration::ZERO)
        .expect("reloadable config should initialize");
    let initial = config.load().expect("initial config should load");
    assert_eq!(initial.node_id, "dp-old");
    assert_eq!(initial.runtime_tuning.request_mirror_max_concurrency, 8);

    fs::write(
        &config_path,
        r#"
nodeId: dp-new
cluster: prod
controlPlaneAddr: https://controlplane.example:18080
adminAddr: 0.0.0.0:19080
runtimeTuning:
  requestMirrorMaxConcurrency: 17
"#,
    )
    .expect("updated config should be written");

    let reloaded = config.load().expect("updated config should load");
    assert_eq!(reloaded.node_id, "dp-new");
    assert_eq!(reloaded.cluster, "prod");
    assert_eq!(
        reloaded.control_plane_addr,
        "https://controlplane.example:18080"
    );
    assert_eq!(reloaded.admin_addr, "0.0.0.0:19080");
    assert_eq!(reloaded.runtime_tuning.request_mirror_max_concurrency, 17);
}

#[test]
fn resolves_admin_token_from_file() {
    let dir = tempfile_dir();
    let token_path = dir.join("token");
    fs::write(&token_path, " top-secret \n").expect("token file should be written");

    let token = AdminAuthConfig {
        bearer_token_file: token_path.display().to_string(),
        ..AdminAuthConfig::default()
    }
    .resolve_bearer_token()
    .expect("token should resolve");

    assert_eq!(token.as_deref(), Some("top-secret"));
}

#[test]
fn xds_tls_is_enabled_when_material_is_configured() {
    let tls = XdsTlsConfig {
        cert_path: "/certs/tls.crt".to_string(),
        key_path: "/certs/tls.key".to_string(),
        ..XdsTlsConfig::default()
    };

    assert!(tls.enabled());
}

#[test]
fn resolves_session_persistence_secret_from_file() {
    let dir = tempfile_dir();
    let secret_path = dir.join("session-secret");
    fs::write(&secret_path, " sticky-secret \n").expect("secret file should be written");

    let secret = SessionPersistenceConfig {
        secret_key_file: secret_path.display().to_string(),
        ..SessionPersistenceConfig::default()
    }
    .resolve_secret()
    .expect("secret should resolve");

    assert_eq!(secret.as_deref(), Some("sticky-secret".as_bytes()));
}

#[test]
fn config_applies_env_overrides_to_runtime_identity() {
    let mut cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: default
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
"#,
    )
    .expect("config should parse");

    cfg.apply_env_overrides(|key| match key {
        "NANTIAN_GW_NODE_ID" => Some("dp-pod-1".to_string()),
        "NANTIAN_GW_CLUSTER" => Some("kind".to_string()),
        "NANTIAN_GW_CONTROL_PLANE_ADDR" => Some("http://controlplane:18080".to_string()),
        "NANTIAN_GW_ADMIN_ADDR" => Some("0.0.0.0:19080".to_string()),
        _ => None,
    });

    assert_eq!(cfg.node_id, "dp-pod-1");
    assert_eq!(cfg.cluster, "kind");
    assert_eq!(cfg.control_plane_addr, "http://controlplane:18080");
    assert_eq!(cfg.admin_addr, "0.0.0.0:19080");
    assert_eq!(trimmed_non_empty("   "), None);
}

#[test]
fn experimental_config_defaults_disabled() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
"#,
    )
    .expect("config should parse");

    assert!(!cfg.experimental.enable_experimental_gateway);
    assert!(!cfg.experimental.enable_ai_gateway);
    assert_eq!(
        cfg.experimental.ai_gateway_max_request_body_bytes,
        10 * 1024 * 1024
    );
}

#[test]
fn experimental_config_enabled() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
experimental:
  enableExperimentalGateway: true
  enableAiGateway: true
  aiGatewayMaxRequestBodyBytes: 8192
"#,
    )
    .expect("config should parse");

    assert!(cfg.experimental.enable_experimental_gateway);
    assert!(cfg.experimental.enable_ai_gateway);
    assert_eq!(cfg.experimental.ai_gateway_max_request_body_bytes, 8192);
}
