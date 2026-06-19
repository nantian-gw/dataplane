use std::collections::BTreeSet;

use super::super::{
    AppState,
    filters::is_http_listener,
    summary::{
        CurrentSnapshotState, RuntimePlaneState, SessionPersistenceUsage,
        build_current_snapshot_state, build_readiness_state, build_runtime_plane_state,
        snapshot_requires_http_runtime, snapshot_requires_stream_runtime,
        snapshot_requires_tls_runtime, snapshot_session_persistence_usage,
    },
};
use ntgw_ir::Snapshot;
use ntgw_observability::{
    AccessLogWriterSnapshot, AdminRequestStatsSnapshot, HttpCircuitBreakerSnapshot,
    HttpRateLimitSnapshot, OverloadSnapshot, ProcessSnapshot, RetryBudgetSnapshot,
    RuntimeStatsSnapshot, TrafficSnapshot, UdpSessionSnapshot, snapshot_access_log_writers,
    snapshot_process,
};
use ntgw_xds::ClientStatsSnapshot;

pub(super) struct MetricsContext {
    pub(super) node_id: String,
    pub(super) cluster: String,
    pub(super) snapshot: Snapshot,
    pub(super) runtime: RuntimeStatsSnapshot,
    pub(super) traffic: TrafficSnapshot,
    pub(super) udp_sessions: UdpSessionSnapshot,
    pub(super) admin_requests: AdminRequestStatsSnapshot,
    pub(super) access_log_writers: AccessLogWriterSnapshot,
    pub(super) process: ProcessSnapshot,
    pub(super) xds: ClientStatsSnapshot,
    pub(super) overload: OverloadSnapshot,
    pub(super) circuit_breaker: HttpCircuitBreakerSnapshot,
    pub(super) rate_limit: HttpRateLimitSnapshot,
    pub(super) retry_budget: RetryBudgetSnapshot,
    pub(super) current_snapshot: CurrentSnapshotState,
    pub(super) http_runtime: RuntimePlaneState,
    pub(super) tls_runtime: RuntimePlaneState,
    pub(super) stream_runtime: RuntimePlaneState,
    pub(super) session_persistence: SessionPersistenceUsage,
    pub(super) ready: u64,
    pub(super) http3_available: u64,
    pub(super) http3_configured: u64,
    pub(super) http3_enabled: u64,
    pub(super) session_persistence_active: u64,
    pub(super) session_persistence_ephemeral: u64,
    pub(super) retry_rate: Option<f64>,
    pub(super) failover_success_rate: Option<f64>,
    pub(super) upstream_pool_hit_ratio: Option<f64>,
    pub(super) upstream_connect_latency_avg_ms: Option<f64>,
    pub(super) http_listener_metric_labels: Vec<String>,
    pub(super) tcp_listener_metric_labels: Vec<String>,
    pub(super) udp_listener_metric_labels: Vec<String>,
    pub(super) route_metric_labels: Vec<String>,
    pub(super) backend_metric_labels: Vec<String>,
}

impl MetricsContext {
    pub(super) fn from_state(state: &AppState) -> Self {
        let config = state.current_config();
        let snapshot = Snapshot::clone(&state.snapshot.load());
        let runtime = state.runtime.snapshot();
        let traffic = state.traffic.snapshot();
        let udp_sessions = state.udp_sessions.snapshot();
        let admin_requests = state.admin_requests.snapshot();
        let access_log_writers = snapshot_access_log_writers();
        let process = snapshot_process();
        let xds = state.xds.snapshot();
        let overload = state.overload.snapshot();
        let circuit_breaker = state
            .circuit_breaker
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .snapshot();
        let rate_limit = state
            .rate_limit
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .snapshot();
        let retry_budget = state
            .retry_budget
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .snapshot();
        let current_snapshot = build_current_snapshot_state(&snapshot, &runtime, &xds);
        let http_runtime = build_runtime_plane_state(
            snapshot_requires_http_runtime(&snapshot),
            snapshot.id.as_str(),
            runtime.http_last_reload_attempt_version.as_str(),
            runtime.http_last_good_reload_version.as_str(),
            runtime.http_last_reload_failure_version.as_str(),
            runtime.http_last_reload_failure_message.as_str(),
        );
        let tls_runtime = build_runtime_plane_state(
            snapshot_requires_tls_runtime(&snapshot),
            snapshot.id.as_str(),
            runtime.tls_last_reload_attempt_version.as_str(),
            runtime.tls_last_good_reload_version.as_str(),
            runtime.tls_last_reload_failure_version.as_str(),
            runtime.tls_last_reload_failure_message.as_str(),
        );
        let stream_runtime = build_runtime_plane_state(
            snapshot_requires_stream_runtime(&snapshot),
            snapshot.id.as_str(),
            runtime.stream_last_reload_attempt_version.as_str(),
            runtime.stream_last_good_reload_version.as_str(),
            runtime.stream_last_reload_failure_version.as_str(),
            runtime.stream_last_reload_failure_message.as_str(),
        );
        let session_persistence = snapshot_session_persistence_usage(&snapshot);
        let ready = u64::from(
            build_readiness_state(&snapshot, &runtime, &xds, config.snapshot_freshness_timeout)
                .ready,
        );
        let http3_available = u64::from(ntgw_http::http3_available());
        let http3_configured = u64::from(config.http3_configured);
        let http3_enabled = u64::from(config.http3_configured && ntgw_http::http3_available());
        let session_persistence_active = u64::from(session_persistence.active());
        let session_persistence_ephemeral =
            u64::from(config.session_persistence_uses_ephemeral_secret);
        let retry_rate = super::traffic::traffic_retry_rate(&traffic);
        let failover_success_rate = super::traffic::traffic_failover_success_rate(&traffic);
        let upstream_pool_hit_ratio = super::traffic::traffic_upstream_pool_hit_ratio(&traffic);
        let upstream_connect_latency_avg_ms =
            super::traffic::traffic_upstream_connect_latency_avg_ms(&traffic);
        let http_listener_metric_labels = http_listener_metric_labels(&snapshot);
        let tcp_listener_metric_labels = tcp_listener_metric_labels(&snapshot);
        let udp_listener_metric_labels = udp_listener_metric_labels(&snapshot);
        let route_metric_labels = route_metric_labels(&snapshot);
        let backend_metric_labels = backend_metric_labels(&snapshot);

        Self {
            node_id: config.node_id,
            cluster: config.cluster,
            snapshot,
            runtime,
            traffic,
            udp_sessions,
            admin_requests,
            access_log_writers,
            process,
            xds,
            overload,
            circuit_breaker,
            rate_limit,
            retry_budget,
            current_snapshot,
            http_runtime,
            tls_runtime,
            stream_runtime,
            session_persistence,
            ready,
            http3_available,
            http3_configured,
            http3_enabled,
            session_persistence_active,
            session_persistence_ephemeral,
            retry_rate,
            failover_success_rate,
            upstream_pool_hit_ratio,
            upstream_connect_latency_avg_ms,
            http_listener_metric_labels,
            tcp_listener_metric_labels,
            udp_listener_metric_labels,
            route_metric_labels,
            backend_metric_labels,
        }
    }
}

fn http_listener_metric_labels(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .listeners
        .iter()
        .filter(|listener| is_http_listener(listener.protocol.as_str()))
        .map(|listener| listener.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn tcp_listener_metric_labels(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .listeners
        .iter()
        .filter(|listener| is_tcp_listener(listener.protocol.as_str()))
        .map(|listener| listener.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn udp_listener_metric_labels(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .listeners
        .iter()
        .filter(|listener| is_udp_listener(listener.protocol.as_str()))
        .map(|listener| listener.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn route_metric_labels(snapshot: &Snapshot) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for route in &snapshot.http_routes {
        labels.insert(format!("Http/{}/{}", route.namespace, route.name));
    }
    for route in &snapshot.grpc_routes {
        labels.insert(format!("Grpc/{}/{}", route.namespace, route.name));
    }
    labels.into_iter().collect()
}

fn backend_metric_labels(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .backends
        .iter()
        .map(|backend| format!("{}/{}", backend.namespace, backend.name))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn is_tcp_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_TCP"
            | "LISTENER_PROTOCOL_TLS"
            | "LISTENER_PROTOCOL_TLS_PASSTHROUGH"
            | "TCP"
            | "TLS"
            | "TLS_PASSTHROUGH"
    )
}

fn is_udp_listener(protocol: &str) -> bool {
    matches!(protocol, "LISTENER_PROTOCOL_UDP" | "UDP")
}
