pub(super) use axum::{
    body::Body,
    http::header,
    http::{Request, StatusCode},
};
pub(super) use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendPolicy, CookieConfig, GrpcRoute, GrpcRule, HttpRule,
    Listener, PASSIVE_EJECTION_CONSECUTIVE_FAILURES, RouteKind, SelectedBackend,
    SessionPersistence, Snapshot, StreamMatch, StreamRoute, StreamRule, TlsRouteMode,
};
pub(super) use ntgw_observability::{
    AdminRequestStats, HttpAdmissionController, HttpAdmissionOptions, HttpCircuitBreakerController,
    HttpCircuitBreakerOptions, HttpRateLimitController, HttpRateLimitOptions, OverloadStats,
    RetryBudgetController, RuntimeListenerFailure, RuntimeStats, SharedTrafficStats,
    TcpAdmissionController, TcpAdmissionOptions, TrafficObservation, TrafficRuntimeIds,
    UdpAdmissionController, UdpAdmissionOptions, UdpSessionStats,
};
pub(super) use ntgw_xds::ClientStats;
pub(super) use parking_lot::RwLock;
pub(super) use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub(super) use tower::util::ServiceExt;

pub(super) use super::filters::{filter_backends, filter_routes};
pub(super) use super::{
    AdminRouteContract, AdminRuntimeConfig, AppState, BackendListQuery, ListenerListQuery,
    RouteListQuery, build_listener_runtime_status, build_router, build_summary_value,
    collect_listener_runtime_statuses, documented_route_contracts, filter_listeners, find_backend,
    find_listener, find_route, render_metrics,
};

mod admin_views;
mod auth;
mod auth_health;
mod contract;
mod listener_filters;
mod listener_runtime;
mod listener_status_endpoints_basic;
mod listener_status_endpoints_filters;
mod listener_status_logic;
mod metrics;
mod route_backend_filters;
mod summary_core;
mod summary_current_states;
mod summary_readiness;
mod summary_recovery_states;
mod summary_runtime;

fn fixture_snapshot() -> Snapshot {
    Snapshot {
        id: "v-test".to_string(),
        listeners: vec![
            Listener {
                name: "web".to_string(),
                address: "192.0.2.10".to_string(),
                addresses: vec!["192.0.2.10".to_string(), "gw.example.com".to_string()],
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["app.example.com".to_string()],
                attached_routes: vec!["default/web".to_string()],
                ..Listener::default()
            },
            Listener {
                name: "passthrough".to_string(),
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
                hostnames: vec!["secure.example.com".to_string()],
                ..Listener::default()
            },
        ],
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "web".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["app.example.com".to_string()],
            rules: vec![HttpRule::default()],
            ..Default::default()
        }],
        grpc_routes: vec![GrpcRoute {
            name: "grpc".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["grpc.example.com".to_string()],
            rules: vec![GrpcRule::default()],
            ..Default::default()
        }],
        stream_routes: vec![StreamRoute {
            name: "passthrough".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TLS".to_string(),
            parent_refs: vec![],
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch {
                    port: 443,
                    sni_hostname: "secure.example.com".to_string(),
                    mode: TlsRouteMode::default(),
                }],
                ..Default::default()
            }],
            labels: Default::default(),
            annotations: Default::default(),
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "api:80".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 80,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
            },
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "http2-clear:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "H2C".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.30".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
            },
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "tcp-service:9000".to_string().into(),
                namespace: "ops".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 9000,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
            },
        ],
        secrets: vec![Default::default()],
        ..Snapshot::default()
    }
}

fn test_admin_runtime_config() -> AdminRuntimeConfig {
    AdminRuntimeConfig {
        admin_bearer_token: None,
        admin_bearer_token_file: None,
        cluster: "kind".to_string(),
        http3_configured: false,
        node_id: "dp-1".to_string(),
        session_persistence_uses_ephemeral_secret: false,
        snapshot_freshness_timeout: Duration::from_secs(90),
    }
}

fn build_state_with_parts(
    config: AdminRuntimeConfig,
    snapshot: ntgw_ir::SharedSnapshot,
    runtime: ntgw_observability::SharedRuntimeStats,
    xds: ntgw_xds::SharedClientStats,
) -> AppState {
    AppState {
        config: Arc::new(RwLock::new(config)),
        snapshot,
        runtime,
        traffic: SharedTrafficStats::shared(),
        udp_sessions: UdpSessionStats::shared(),
        admin_requests: AdminRequestStats::shared(),
        xds,
        overload: OverloadStats::shared(),
        circuit_breaker: Arc::new(RwLock::new(HttpCircuitBreakerController::new(
            HttpCircuitBreakerOptions::default(),
        ))),
        rate_limit: Arc::new(RwLock::new(HttpRateLimitController::new(
            HttpRateLimitOptions::default(),
        ))),
        retry_budget: Arc::new(RwLock::new(RetryBudgetController::new(Default::default()))),
    }
}

fn test_state(token: Option<&str>) -> AppState {
    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(fixture_snapshot()));

    let mut config = test_admin_runtime_config();
    config.admin_bearer_token = token.map(str::to_string);

    build_state_with_parts(
        config,
        snapshot,
        RuntimeStats::shared(),
        ClientStats::shared(),
    )
}

fn test_state_with_file(path: &std::path::Path) -> AppState {
    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(fixture_snapshot()));

    let mut config = test_admin_runtime_config();
    config.admin_bearer_token_file = Some(path.display().to_string());

    build_state_with_parts(
        config,
        snapshot,
        RuntimeStats::shared(),
        ClientStats::shared(),
    )
}

fn temp_token_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("ntgw-app-{prefix}-{unique}.token"))
}

fn set_http3_configured(state: &AppState, value: bool) {
    state.config.write().http3_configured = value;
}

fn set_session_persistence_uses_ephemeral_secret(state: &AppState, value: bool) {
    state
        .config
        .write()
        .session_persistence_uses_ephemeral_secret = value;
}

fn set_snapshot_freshness_timeout(state: &AppState, value: Duration) {
    state.config.write().snapshot_freshness_timeout = value;
}

fn replace_circuit_breaker(state: &AppState, options: HttpCircuitBreakerOptions) {
    *state.circuit_breaker.write() = HttpCircuitBreakerController::new(options);
}

fn with_circuit_breaker<T>(
    state: &AppState,
    f: impl FnOnce(&mut HttpCircuitBreakerController) -> T,
) -> T {
    let mut guard = state.circuit_breaker.write();
    f(&mut guard)
}

fn replace_rate_limit(state: &AppState, options: HttpRateLimitOptions) {
    *state.rate_limit.write() = HttpRateLimitController::new(options);
}

fn with_rate_limit<T>(state: &AppState, f: impl FnOnce(&mut HttpRateLimitController) -> T) -> T {
    let mut guard = state.rate_limit.write();
    f(&mut guard)
}
