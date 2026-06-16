use std::sync::{Arc, RwLock};

use tokio::sync::watch;

use ntgw_config::{
    AccessLogConfig, AdminAuthConfig, DataPlaneConfig, ExperimentalConfig, LogConfig,
    RuntimeConfig, RuntimeProtectionConfig, RuntimeTuningConfig, SessionPersistenceConfig,
    XdsTlsConfig, XdsTransportConfig,
};
use ntgw_observability::{
    HttpCircuitBreakerController, HttpRateLimitController, RetryBudgetController,
};

use super::{ReloadTargets, apply_config_snapshot, build_config_snapshot};

fn test_config() -> DataPlaneConfig {
    DataPlaneConfig {
        node_id: "dp-1".to_string(),
        cluster: "kind".to_string(),
        control_plane_addr: "http://127.0.0.1:18080".to_string(),
        admin_addr: "127.0.0.1:19080".to_string(),
        log: LogConfig::default(),
        access_log: AccessLogConfig::default(),
        admin_auth: AdminAuthConfig::default(),
        runtime: RuntimeConfig::default(),
        session_persistence: SessionPersistenceConfig::default(),
        xds_tls: XdsTlsConfig::default(),
        xds_transport: XdsTransportConfig::default(),
        runtime_protection: RuntimeProtectionConfig::default(),
        runtime_tuning: RuntimeTuningConfig::default(),
        experimental: ExperimentalConfig::default(),
    }
}

#[test]
fn apply_config_snapshot_updates_shared_targets_and_watch_channels() {
    let initial = build_config_snapshot(&test_config()).expect("initial snapshot");
    let mut updated = test_config();
    updated.node_id = "dp-2".to_string();
    updated.cluster = "prod".to_string();
    updated.control_plane_addr = "https://control.example.com:8443".to_string();
    updated.admin_auth.bearer_token = "secret-token".to_string();
    updated.runtime.enable_http3 = true;
    updated.session_persistence.secret_key = "persistent-secret".to_string();
    updated
        .runtime_protection
        .http_backend_circuit_breaker_max_requests = 17;
    updated
        .runtime_protection
        .http_global_rate_limit_requests_per_second = 9;
    updated.runtime_protection.http_global_rate_limit_burst = 11;
    updated
        .runtime_protection
        .http_listener_rate_limit_requests_per_second = 5;
    updated.runtime_protection.http_listener_rate_limit_burst = 7;
    updated.runtime_protection.tcp_listener_connection_limit = 13;
    updated.runtime_tuning.active_health_check_enabled = true;
    updated.runtime_tuning.retry_budget_enabled = false;
    updated.runtime_tuning.retry_budget_ratio_percent = 35;
    updated.runtime_tuning.retry_budget_burst = 9;
    updated.runtime_tuning.active_health_check_interval_ms = 12_000;
    updated.runtime_tuning.active_health_check_timeout_ms = 1_500;
    updated
        .runtime_tuning
        .active_health_check_unhealthy_threshold = 4;

    let next = build_config_snapshot(&updated).expect("updated snapshot");
    let admin = Arc::new(RwLock::new(initial.admin.clone()));
    let (http_tx, http_rx) = watch::channel(Arc::new(initial.http.clone()));
    let (shared_tls_tx, shared_tls_rx) = watch::channel(Arc::new(initial.shared_tls.clone()));
    let (stream_tx, stream_rx) = watch::channel(Arc::new(initial.stream.clone()));
    let (xds_tx, xds_rx) = watch::channel(Arc::new(initial.xds.clone()));
    let (active_health_tx, active_health_rx) =
        watch::channel(Arc::new(initial.active_health.clone()));
    let circuit_breaker = Arc::new(RwLock::new(HttpCircuitBreakerController::new(
        initial.circuit_breaker.clone(),
    )));
    let rate_limit = Arc::new(RwLock::new(HttpRateLimitController::new(
        initial.rate_limit.clone(),
    )));
    let retry_budget = Arc::new(RwLock::new(RetryBudgetController::new(
        initial.retry_budget.clone(),
    )));

    apply_config_snapshot(
        next,
        &ReloadTargets {
            admin: admin.clone(),
            http: http_tx,
            shared_tls: shared_tls_tx,
            stream: stream_tx,
            xds: xds_tx,
            active_health: active_health_tx,
            circuit_breaker: circuit_breaker.clone(),
            rate_limit: rate_limit.clone(),
            retry_budget: retry_budget.clone(),
        },
    );

    let admin = admin.read().unwrap_or_else(|err| err.into_inner()).clone();
    assert_eq!(admin.node_id, "dp-2");
    assert_eq!(admin.cluster, "prod");
    assert_eq!(admin.admin_bearer_token.as_deref(), Some("secret-token"));
    assert!(admin.http3_configured);
    assert!(!admin.session_persistence_uses_ephemeral_secret);

    assert!(http_rx.borrow().runtime.enable_http3);
    assert_eq!(
        xds_rx.borrow().connect_options.endpoint,
        updated.control_plane_addr
    );
    assert_eq!(xds_rx.borrow().node_id, "dp-2");
    assert_eq!(xds_rx.borrow().cluster, "prod");
    assert!(active_health_rx.borrow().enabled);
    assert_eq!(active_health_rx.borrow().probe_interval.as_millis(), 12_000);
    assert_eq!(active_health_rx.borrow().probe_timeout.as_millis(), 1_500);
    assert_eq!(active_health_rx.borrow().unhealthy_threshold, 4);
    assert_eq!(
        stream_rx
            .borrow()
            .runtime
            .tcp_admission
            .listener_connection_limit,
        updated.runtime_protection.tcp_listener_connection_limit
    );
    assert!(shared_tls_rx.borrow().http.runtime.enable_http3);
    assert_eq!(
        shared_tls_rx
            .borrow()
            .stream
            .runtime
            .tcp_admission
            .listener_connection_limit,
        updated.runtime_protection.tcp_listener_connection_limit
    );
    assert!(!http_rx.borrow().session_persistence.uses_ephemeral_secret());
    assert!(
        !shared_tls_rx
            .borrow()
            .http
            .session_persistence
            .uses_ephemeral_secret()
    );

    let circuit_breaker = circuit_breaker
        .read()
        .unwrap_or_else(|err| err.into_inner())
        .snapshot();
    assert_eq!(circuit_breaker.backend_max_inflight_requests, 17);

    let rate_limit = rate_limit
        .read()
        .unwrap_or_else(|err| err.into_inner())
        .snapshot();
    assert_eq!(rate_limit.global.requests_per_second, 9);
    assert_eq!(rate_limit.global.burst, 11);
    assert_eq!(rate_limit.listener.requests_per_second, 5);
    assert_eq!(rate_limit.listener.burst, 7);

    let retry_budget = retry_budget
        .read()
        .unwrap_or_else(|err| err.into_inner())
        .snapshot();
    assert!(!retry_budget.enabled);
    assert_eq!(retry_budget.ratio_percent, 35);
    assert_eq!(retry_budget.burst, 9);
}

#[test]
fn shared_secret_produces_non_ephemeral_session_persistence() {
    let mut cfg = test_config();
    cfg.session_persistence.shared_secret = Some("prod-shared-secret".to_string());

    let snapshot = build_config_snapshot(&cfg).expect("snapshot with shared_secret");
    assert!(!snapshot.http.session_persistence.uses_ephemeral_secret());
}

#[test]
fn without_shared_secret_still_resolves_secret_key() {
    let mut cfg = test_config();
    cfg.session_persistence.secret_key = "inline-key".to_string();
    assert!(cfg.session_persistence.shared_secret.is_none());

    let snapshot = build_config_snapshot(&cfg).expect("snapshot with secret_key");
    assert!(!snapshot.http.session_persistence.uses_ephemeral_secret());
}

#[test]
fn build_config_snapshot_rejects_unknown_named_text_access_log_format() {
    let mut cfg = test_config();
    cfg.access_log.mode = "text".to_string();
    cfg.access_log.format_name = "missing".to_string();

    let err = build_config_snapshot(&cfg).expect_err("missing named format should fail");
    assert!(err.to_string().contains("accessLog.formatName"));
    assert!(err.to_string().contains("missing"));
}

#[test]
fn build_config_snapshot_ignores_unknown_named_access_log_format_in_json_mode() {
    let mut cfg = test_config();
    cfg.access_log.mode = "json".to_string();
    cfg.access_log.format = "%EVENT%".to_string();
    cfg.access_log.format_name = "missing".to_string();

    let snapshot = build_config_snapshot(&cfg).expect("json mode should ignore named formats");
    assert_eq!(
        snapshot.http.access_log.mode,
        ntgw_observability::AccessLogMode::Json
    );
    assert_eq!(snapshot.http.access_log.format, "%EVENT%");
    assert_eq!(
        snapshot.stream.access_log.mode,
        ntgw_observability::AccessLogMode::Json
    );
    assert_eq!(snapshot.stream.access_log.format, "%EVENT%");
}
