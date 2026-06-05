use serde_json::{json, Map, Value};

use super::{
    super::{
        filters::{is_http_listener, is_https_listener, is_stream_listener},
        AppState,
    },
    listener_signals::ListenerSignalSummary,
    overview_sections::{build_overview_sections, build_warning_data},
    runtime::{CurrentSnapshotState, ReadinessState, RuntimePlaneState, SessionPersistenceUsage},
};

pub(super) struct SummaryValueInputs<'a> {
    pub(super) snapshot: &'a aeg_ir::Snapshot,
    pub(super) runtime: &'a aeg_observability::RuntimeStatsSnapshot,
    pub(super) traffic: &'a aeg_observability::TrafficSnapshot,
    pub(super) xds: &'a aeg_xds::ClientStatsSnapshot,
    pub(super) overload: &'a aeg_observability::OverloadSnapshot,
    pub(super) readiness: &'a ReadinessState,
    pub(super) current_snapshot: &'a CurrentSnapshotState,
    pub(super) http_runtime: &'a RuntimePlaneState,
    pub(super) tls_runtime: &'a RuntimePlaneState,
    pub(super) stream_runtime: &'a RuntimePlaneState,
    pub(super) session_persistence: SessionPersistenceUsage,
    pub(super) retry_rate: f64,
    pub(super) failover_success_rate: f64,
    pub(super) upstream_pool_hit_ratio: f64,
    pub(super) upstream_connect_latency_avg_ms: f64,
    pub(super) http_listener_inflight_current: u64,
    pub(super) http_route_inflight_current: u64,
    pub(super) tcp_listener_connections_current: u64,
    pub(super) udp_listener_datagrams_current: u64,
}

pub(super) fn build_summary_document(
    state: &AppState,
    inputs: SummaryValueInputs<'_>,
    listener_signals: &ListenerSignalSummary,
) -> Value {
    let config = state.current_config();
    let http3_available = aeg_http::http3_available();
    let http3_enabled = config.http3_configured && http3_available;
    let warnings = build_warning_data(state, &inputs, listener_signals);
    let sections = build_overview_sections(
        state,
        &inputs,
        &warnings,
        listener_signals,
        http3_available,
        http3_enabled,
    );

    let mut summary = Map::new();
    for (key, value) in [
        ("nodeId", json!(config.node_id)),
        ("cluster", json!(config.cluster)),
        ("ready", json!(inputs.readiness.ready)),
        ("readinessState", json!(inputs.readiness.state)),
        ("readinessReason", json!(inputs.readiness.reason)),
        ("summarySurface", json!("dataplane-summary")),
        ("summarySchemaVersion", json!(1)),
        ("metaOverview", sections.meta_overview.clone()),
        ("instanceOverview", sections.instance_overview.clone()),
        ("healthOverview", sections.health_overview.clone()),
        ("warningOverview", sections.warning_overview.clone()),
        ("warningCategories", json!(warnings.categories.clone())),
        ("warningCategoryCounts", warnings.counts.clone()),
        (
            "currentSnapshotStatus",
            json!(inputs.current_snapshot.status),
        ),
        (
            "currentSnapshotAccepted",
            json!(inputs.current_snapshot.accepted),
        ),
        (
            "currentSnapshotRejected",
            json!(inputs.current_snapshot.rejected),
        ),
        (
            "servingLastGoodSnapshot",
            json!(inputs.current_snapshot.serving_last_good_snapshot),
        ),
        (
            "currentSnapshotFallbackState",
            json!(inputs.current_snapshot.fallback_state),
        ),
        (
            "lastGoodSnapshotVersion",
            json!(inputs.current_snapshot.last_good_snapshot_version),
        ),
        (
            "currentSnapshotRejectionVersion",
            json!(inputs.current_snapshot.rejection_version),
        ),
        (
            "currentSnapshotRejectionRuntime",
            json!(inputs.current_snapshot.rejection_runtime),
        ),
        (
            "currentSnapshotRejectionMessage",
            json!(inputs.current_snapshot.rejection_message),
        ),
        ("runtimeHttpRequired", json!(inputs.http_runtime.required)),
        (
            "runtimeHttpCurrentStatus",
            json!(inputs.http_runtime.status),
        ),
        (
            "runtimeHttpCurrentAccepted",
            json!(inputs.http_runtime.accepted),
        ),
        (
            "runtimeHttpCurrentRejected",
            json!(inputs.http_runtime.rejected),
        ),
        (
            "runtimeHttpRunning",
            json!(inputs.runtime.http_runtime_running),
        ),
        (
            "runtimeHttpLastExitUnixSeconds",
            json!(inputs.runtime.http_last_exit_unix_seconds),
        ),
        (
            "runtimeHttpLastExitMessage",
            json!(inputs.runtime.http_last_exit_message),
        ),
        ("runtimeTlsRequired", json!(inputs.tls_runtime.required)),
        ("runtimeTlsCurrentStatus", json!(inputs.tls_runtime.status)),
        (
            "runtimeTlsCurrentAccepted",
            json!(inputs.tls_runtime.accepted),
        ),
        (
            "runtimeTlsCurrentRejected",
            json!(inputs.tls_runtime.rejected),
        ),
        (
            "runtimeTlsRunning",
            json!(inputs.runtime.tls_runtime_running),
        ),
        (
            "runtimeTlsLastExitUnixSeconds",
            json!(inputs.runtime.tls_last_exit_unix_seconds),
        ),
        (
            "runtimeTlsLastExitMessage",
            json!(inputs.runtime.tls_last_exit_message),
        ),
        (
            "runtimeStreamRequired",
            json!(inputs.stream_runtime.required),
        ),
        (
            "runtimeStreamCurrentStatus",
            json!(inputs.stream_runtime.status),
        ),
        (
            "runtimeStreamCurrentAccepted",
            json!(inputs.stream_runtime.accepted),
        ),
        (
            "runtimeStreamCurrentRejected",
            json!(inputs.stream_runtime.rejected),
        ),
        (
            "runtimeStreamRunning",
            json!(inputs.runtime.stream_runtime_running),
        ),
        (
            "runtimeStreamLastExitUnixSeconds",
            json!(inputs.runtime.stream_last_exit_unix_seconds),
        ),
        (
            "runtimeStreamLastExitMessage",
            json!(inputs.runtime.stream_last_exit_message),
        ),
        ("xdsStreamConnected", json!(inputs.xds.stream_connected)),
        (
            "xdsLastControlPlaneContactUnixSeconds",
            json!(inputs.xds.last_control_plane_contact_unix_seconds),
        ),
        ("snapshotOverview", sections.snapshot_overview.clone()),
        ("runtimeOverview", sections.runtime_overview.clone()),
        ("snapshotVersion", json!(inputs.snapshot.id)),
        ("listenerCount", json!(inputs.snapshot.listeners.len())),
        (
            "httpListenerCount",
            json!(inputs
                .snapshot
                .listeners
                .iter()
                .filter(|listener| is_http_listener(&listener.protocol))
                .count()),
        ),
        (
            "httpsListenerCount",
            json!(inputs
                .snapshot
                .listeners
                .iter()
                .filter(|listener| is_https_listener(&listener.protocol))
                .count()),
        ),
        (
            "streamListenerCount",
            json!(inputs
                .snapshot
                .listeners
                .iter()
                .filter(|listener| is_stream_listener(&listener.protocol))
                .count()),
        ),
        ("httpRouteCount", json!(inputs.snapshot.http_routes.len())),
        ("grpcRouteCount", json!(inputs.snapshot.grpc_routes.len())),
        (
            "streamRouteCount",
            json!(inputs.snapshot.stream_routes.len()),
        ),
        (
            "routeCount",
            json!(
                inputs.snapshot.http_routes.len()
                    + inputs.snapshot.grpc_routes.len()
                    + inputs.snapshot.stream_routes.len()
            ),
        ),
        ("backendCount", json!(inputs.snapshot.backends.len())),
        ("secretCount", json!(inputs.snapshot.secrets.len())),
        ("resourceOverview", sections.resource_overview.clone()),
        ("http3Configured", json!(config.http3_configured)),
        ("http3Available", json!(http3_available)),
        ("http3Enabled", json!(http3_enabled)),
        (
            "sessionPersistenceConfigured",
            json!(!config.session_persistence_uses_ephemeral_secret),
        ),
        (
            "sessionPersistenceUsesEphemeralSecret",
            json!(config.session_persistence_uses_ephemeral_secret),
        ),
        (
            "sessionPersistenceActive",
            json!(inputs.session_persistence.active()),
        ),
        (
            "sessionPersistenceRouteRuleCount",
            json!(inputs.session_persistence.route_rules),
        ),
        (
            "sessionPersistenceBackendPolicyCount",
            json!(inputs.session_persistence.backend_policies),
        ),
        ("featureOverview", sections.feature_overview.clone()),
        ("xdsConnectFailures", json!(inputs.xds.connect_failures)),
        ("xdsStreamFailures", json!(inputs.xds.stream_failures)),
        ("xdsLastConnectError", json!(inputs.xds.last_connect_error)),
        (
            "xdsLastConnectFailureUnixSeconds",
            json!(inputs.xds.last_connect_failure_unix_seconds),
        ),
        ("xdsLastStreamError", json!(inputs.xds.last_stream_error)),
        (
            "xdsLastStreamFailureUnixSeconds",
            json!(inputs.xds.last_stream_failure_unix_seconds),
        ),
        ("xdsSnapshotsApplied", json!(inputs.xds.snapshots_applied)),
        ("xdsSnapshotsNacked", json!(inputs.xds.snapshots_nacked)),
        ("xdsSnapshotsSkipped", json!(inputs.xds.snapshots_skipped)),
        ("xdsLastNackVersion", json!(inputs.xds.last_nack_version)),
        ("xdsLastNackMessage", json!(inputs.xds.last_nack_message)),
        (
            "xdsLastSnapshotVersion",
            json!(inputs.xds.last_snapshot_version),
        ),
        (
            "xdsLastApplyUnixSeconds",
            json!(inputs.xds.last_apply_unix_seconds),
        ),
        ("xdsOverview", sections.xds_overview.clone()),
        (
            "runtimeHttpListenerReloadFailures",
            json!(inputs.runtime.http_listener_reload_failures),
        ),
        (
            "runtimeHttpLastReloadAttemptVersion",
            json!(inputs.runtime.http_last_reload_attempt_version),
        ),
        (
            "runtimeHttpLastGoodReloadVersion",
            json!(inputs.runtime.http_last_good_reload_version),
        ),
        (
            "runtimeHttpLastReloadFailureVersion",
            json!(inputs.runtime.http_last_reload_failure_version),
        ),
        (
            "runtimeHttpLastReloadFailureListener",
            json!(inputs.runtime.http_last_reload_failure_listener),
        ),
        (
            "runtimeHttpLastReloadFailureMessage",
            json!(inputs.runtime.http_last_reload_failure_message),
        ),
        (
            "runtimeHttpCurrentFailures",
            json!(inputs.runtime.http_current_failures),
        ),
        (
            "runtimeHttpTlsAssetReuses",
            json!(inputs.runtime.http_tls_asset_reuses),
        ),
        (
            "runtimeTlsListenerReloadFailures",
            json!(inputs.runtime.tls_listener_reload_failures),
        ),
        (
            "runtimeTlsLastReloadAttemptVersion",
            json!(inputs.runtime.tls_last_reload_attempt_version),
        ),
        (
            "runtimeTlsLastGoodReloadVersion",
            json!(inputs.runtime.tls_last_good_reload_version),
        ),
        (
            "runtimeTlsLastReloadFailureVersion",
            json!(inputs.runtime.tls_last_reload_failure_version),
        ),
        (
            "runtimeTlsLastReloadFailureListener",
            json!(inputs.runtime.tls_last_reload_failure_listener),
        ),
        (
            "runtimeTlsLastReloadFailureMessage",
            json!(inputs.runtime.tls_last_reload_failure_message),
        ),
        (
            "runtimeTlsCurrentFailures",
            json!(inputs.runtime.tls_current_failures),
        ),
        (
            "runtimeStreamListenerReloadFailures",
            json!(inputs.runtime.stream_listener_reload_failures),
        ),
        (
            "runtimeStreamLastReloadAttemptVersion",
            json!(inputs.runtime.stream_last_reload_attempt_version),
        ),
        (
            "runtimeStreamLastGoodReloadVersion",
            json!(inputs.runtime.stream_last_good_reload_version),
        ),
        (
            "runtimeStreamLastReloadFailureVersion",
            json!(inputs.runtime.stream_last_reload_failure_version),
        ),
        (
            "runtimeStreamLastReloadFailureListener",
            json!(inputs.runtime.stream_last_reload_failure_listener),
        ),
        (
            "runtimeStreamLastReloadFailureMessage",
            json!(inputs.runtime.stream_last_reload_failure_message),
        ),
        (
            "runtimeStreamCurrentFailures",
            json!(inputs.runtime.stream_current_failures),
        ),
        ("trafficTotalEvents", json!(inputs.traffic.total_events)),
        (
            "trafficTotalBytesReceived",
            json!(inputs.traffic.total_bytes_received),
        ),
        (
            "trafficTotalBytesSent",
            json!(inputs.traffic.total_bytes_sent),
        ),
        (
            "trafficTotalRetriedEvents",
            json!(inputs.traffic.total_retried_events),
        ),
        (
            "trafficTotalRetryAttempts",
            json!(inputs.traffic.total_retry_attempts),
        ),
        (
            "trafficRetriedSuccessEvents",
            json!(inputs.traffic.total_retried_success_events),
        ),
        ("trafficRetryRate", json!(inputs.retry_rate)),
        (
            "trafficFailoverSuccessRate",
            json!(inputs.failover_success_rate),
        ),
        ("trafficMaxLatencyMs", json!(inputs.traffic.max_latency_ms)),
        (
            "trafficUpstreamPoolHits",
            json!(inputs.traffic.total_upstream_pool_hits),
        ),
        (
            "trafficUpstreamPoolMisses",
            json!(inputs.traffic.total_upstream_pool_misses),
        ),
        (
            "trafficUpstreamPoolHitRatio",
            json!(inputs.upstream_pool_hit_ratio),
        ),
        (
            "trafficUpstreamConnectLatencyMsAvg",
            json!(inputs.upstream_connect_latency_avg_ms),
        ),
        (
            "trafficUpstreamConnectLatencyMsMax",
            json!(inputs.traffic.max_upstream_connect_latency_ms),
        ),
        ("trafficOverview", sections.traffic_overview.clone()),
        (
            "overloadHttpGlobalInflightCurrent",
            json!(inputs.overload.http_global_inflight_current),
        ),
        (
            "overloadHttpListenerInflightCurrent",
            json!(inputs.http_listener_inflight_current),
        ),
        (
            "overloadHttpRouteInflightCurrent",
            json!(inputs.http_route_inflight_current),
        ),
        (
            "overloadHttpRejectedTotal",
            json!(inputs.overload.http_rejected_total),
        ),
        (
            "overloadHttpRejectedGlobal",
            json!(inputs.overload.http_rejected_global_total),
        ),
        (
            "overloadHttpRejectedListener",
            json!(inputs.overload.http_rejected_listener_total),
        ),
        (
            "overloadHttpRejectedRoute",
            json!(inputs.overload.http_rejected_route_total),
        ),
        (
            "overloadTcpGlobalConnectionsCurrent",
            json!(inputs.overload.tcp_global_connections_current),
        ),
        (
            "overloadTcpListenerConnectionsCurrent",
            json!(inputs.tcp_listener_connections_current),
        ),
        (
            "overloadTcpRejectedTotal",
            json!(inputs.overload.tcp_rejected_total),
        ),
        (
            "overloadTcpRejectedGlobal",
            json!(inputs.overload.tcp_rejected_global_total),
        ),
        (
            "overloadTcpRejectedListener",
            json!(inputs.overload.tcp_rejected_listener_total),
        ),
        (
            "overloadUdpGlobalDatagramsCurrent",
            json!(inputs.overload.udp_global_datagrams_current),
        ),
        (
            "overloadUdpListenerDatagramsCurrent",
            json!(inputs.udp_listener_datagrams_current),
        ),
        (
            "overloadUdpRejectedTotal",
            json!(inputs.overload.udp_rejected_total),
        ),
        (
            "overloadUdpRejectedGlobal",
            json!(inputs.overload.udp_rejected_global_total),
        ),
        (
            "overloadUdpRejectedListener",
            json!(inputs.overload.udp_rejected_listener_total),
        ),
        ("overloadOverview", sections.overload_overview.clone()),
        ("summaryOverviews", sections.summary_overviews.clone()),
        ("warnings", json!(warnings.messages.clone())),
    ] {
        summary.insert(key.to_string(), value);
    }

    summary.extend(listener_signals.top_level_fields.clone());
    Value::Object(summary)
}
