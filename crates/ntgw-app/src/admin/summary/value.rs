use serde_json::Value;

use super::super::AppState;
use super::{
    listener_signals::build_listener_signal_summary,
    listener_status::build_listener_runtime_status,
    overview_builder::{SummaryValueInputs, build_summary_document},
    runtime::{
        build_current_snapshot_state, build_readiness_state, build_runtime_plane_state,
        snapshot_requires_http_runtime, snapshot_requires_stream_runtime,
        snapshot_requires_tls_runtime,
    },
    stats::{
        named_value_total, snapshot_session_persistence_usage, traffic_failover_success_rate,
        traffic_retry_rate, traffic_upstream_connect_latency_avg_ms,
        traffic_upstream_pool_hit_ratio,
    },
};

pub(crate) fn build_summary_value(state: &AppState) -> Value {
    let config = state.current_config();
    let snapshot = (*state.snapshot.load()).clone();
    let runtime = state.runtime.snapshot();
    let traffic = state.traffic.snapshot();
    let xds = state.xds.snapshot();
    let overload = state.overload.snapshot();
    let readiness =
        build_readiness_state(&snapshot, &runtime, &xds, config.snapshot_freshness_timeout);
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
    let retry_rate = traffic_retry_rate(&traffic);
    let failover_success_rate = traffic_failover_success_rate(&traffic);
    let upstream_pool_hit_ratio = traffic_upstream_pool_hit_ratio(&traffic);
    let upstream_connect_latency_avg_ms = traffic_upstream_connect_latency_avg_ms(&traffic);
    let http_listener_inflight_current =
        named_value_total(&overload.http_listener_inflight_current);
    let http_route_inflight_current = named_value_total(&overload.http_route_inflight_current);
    let tcp_listener_connections_current =
        named_value_total(&overload.tcp_listener_connections_current);
    let udp_listener_datagrams_current =
        named_value_total(&overload.udp_listener_datagrams_current);
    let listener_runtime_statuses = snapshot
        .listeners
        .iter()
        .map(|listener| build_listener_runtime_status(listener, &snapshot, &runtime))
        .collect::<Vec<_>>();
    let listener_signals =
        build_listener_signal_summary(&listener_runtime_statuses, snapshot.id.as_str());

    build_summary_document(
        state,
        SummaryValueInputs {
            snapshot: &snapshot,
            runtime: &runtime,
            traffic: &traffic,
            xds: &xds,
            overload: &overload,
            readiness: &readiness,
            current_snapshot: &current_snapshot,
            http_runtime: &http_runtime,
            tls_runtime: &tls_runtime,
            stream_runtime: &stream_runtime,
            session_persistence,
            retry_rate,
            failover_success_rate,
            upstream_pool_hit_ratio,
            upstream_connect_latency_avg_ms,
            http_listener_inflight_current,
            http_route_inflight_current,
            tcp_listener_connections_current,
            udp_listener_datagrams_current,
        },
        &listener_signals,
    )
}
