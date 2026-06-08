use std::borrow::Cow;
use std::{net::SocketAddr, time::Instant};

use tracing::warn;

use ntgw_ir::{SelectedBackend, SelectedBackendRuntimeIds};
use ntgw_observability::{
    AccessLogOptions, AccessLogRecord, SharedTrafficStats, TrafficObservationRef,
    TrafficRuntimeIds, current_timestamp, write_access_log,
};

use crate::{
    access_log::StreamAccessLogState,
    traffic::{ZERO_UPSTREAM_CONNECT_LATENCY_MS_BUCKETS, stream_route_kind_label},
};

#[allow(clippy::too_many_arguments)]
pub(super) fn record_udp_datagram(
    traffic: &SharedTrafficStats,
    access_log: Option<&AccessLogOptions>,
    access_log_state: Option<&StreamAccessLogState>,
    selected: &SelectedBackend,
    runtime_ids: SelectedBackendRuntimeIds,
    client_addr: SocketAddr,
    payload_len: usize,
    total_response_bytes: usize,
    started_at: Instant,
) {
    traffic.observe_ref(TrafficObservationRef {
        listener_name: selected.listener_name.as_str(),
        protocol: "UDP",
        route_namespace: selected.route_namespace.as_str(),
        route_name: selected.route_name.as_str(),
        route_kind: stream_route_kind_label(&selected.route_kind),
        backend_name: selected.backend_name.as_str(),
        status: None,
        latency_ms: started_at.elapsed().as_millis().min(u64::MAX as u128) as u64,
        bytes_received: payload_len as u64,
        bytes_sent: total_response_bytes as u64,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 0,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: &ZERO_UPSTREAM_CONNECT_LATENCY_MS_BUCKETS,
        response_flags: "",
        runtime_ids: traffic_runtime_ids(runtime_ids),
    });
    let (Some(access_log), Some(access_log_state)) = (access_log, access_log_state) else {
        return;
    };
    let record = AccessLogRecord {
        event: "udp_datagram".to_string(),
        timestamp: current_timestamp(),
        start_time_unix_ms: access_log_state.started_at_unix_ms,
        snapshot_version: access_log_state.snapshot_version.clone(),
        listener: Cow::Owned(selected.listener_name.clone()),
        listener_runtime_id: runtime_ids.listener.map(|id| id.to_string()),
        protocol: Cow::Borrowed("UDP"),
        client_ip: client_addr.ip().to_string(),
        request_id: String::new(),
        route_namespace: Cow::Owned(selected.route_namespace.clone()),
        route_name: Cow::Owned(selected.route_name.clone()),
        route_kind: Cow::Owned(format!("{:?}", selected.route_kind)),
        route_runtime_id: runtime_ids.route.map(|id| id.to_string()),
        rule_runtime_id: runtime_ids.rule.map(|id| id.to_string()),
        backend: Cow::Owned(selected.backend_name.clone()),
        backend_runtime_id: runtime_ids.backend.map(|id| id.to_string()),
        endpoint_runtime_id: runtime_ids.endpoint.map(|id| id.to_string()),
        status: None,
        latency_ms: started_at.elapsed().as_millis(),
        bytes_sent: total_response_bytes,
        bytes_received: payload_len,
        retry_attempts: 0,
        response_flags: String::new(),
        ..AccessLogRecord::default()
    };
    if let Err(err) = write_access_log(access_log, &selected.route_annotations, &record) {
        warn!(
            listener = %selected.listener_name,
            route = %selected.route_name,
            error = %err,
            "failed to emit stream udp access log"
        );
    }
}

fn traffic_runtime_ids(runtime_ids: SelectedBackendRuntimeIds) -> TrafficRuntimeIds {
    TrafficRuntimeIds {
        listener: runtime_ids.listener.map(|id| id.as_u64()),
        route: runtime_ids.route.map(|id| id.as_u64()),
        backend: runtime_ids.backend.map(|id| id.as_u64()),
    }
}
