use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::mpsc,
    sync::Arc,
    thread,
    time::Duration,
};

use ntgw_ir::{
    BackendCluster, BackendEndpoint, CookieConfig, CorsFilter, Filter, HttpRoute, HttpRule,
    MatchedHttpPath, RequestMeta, RetryPolicy, RouteKind, SelectedBackend, SessionPersistence,
    Snapshot, PASSIVE_EJECTION_CONSECUTIVE_FAILURES,
};
use ntgw_observability::{upstream_connect_latency_ms_bucket_index, SharedTrafficStats};
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pingora::http::{RequestHeader, ResponseHeader};
use tracing_subscriber::prelude::*;

use super::super::context::{
    cache_http_route_context, HttpRouteContextFields, SelectedBackendConfig, UpstreamPeerAddress,
};
use super::super::request::{
    build_request_meta_from_header, build_selection_request_meta_from_header_with_port,
    cache_request_headers_if_needed, capture_request_context, capture_request_context_from_view,
    capture_request_context_from_view_for_features, capture_request_context_from_view_for_limits,
    effective_http_protocol, inject_request_span_context, normalize_ip,
    request_header_bytes_for_limit, request_id_from_headers, start_request_span,
    start_request_span_if_enabled, RequestView,
};
use super::super::retry::{response_is_retryable, retry_backoff, retry_completed_successfully};
use super::super::{
    cache_route_annotations, cache_selected_backend, cache_selected_backend_ref,
    cache_selected_backend_state, clear_completed_request_context,
    observe_selected_backend_failure, observe_selected_backend_success, record_upstream_connection,
    record_upstream_peer_build_failure, record_upstream_tls_handshake_failure,
    reset_request_context, RequestContext, SessionResolutionCache,
};
use crate::session::{SessionManager, SessionPersistenceOptions};
use crate::AccessLogOptions;

mod request;
mod retry_session;
mod runtime;
mod state;

fn sample_selected_backend(address: &str, backend_name: &str) -> SelectedBackend {
    SelectedBackend {
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "listener".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: address.to_string(),
            port: 8443,
            healthy: true,
        },
        backend_name: backend_name.to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    }
}

fn sample_runtime_snapshot() -> Snapshot {
    let mut snapshot = Snapshot {
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8443".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTPS".to_string(),
            endpoints: vec![
                BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8443,
                    healthy: true,
                },
                BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8443,
                    healthy: true,
                },
            ],
            wasm_plugin: None,
        }],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    snapshot
}

fn sample_runtime_handle(address: &str) -> ntgw_ir::EndpointRuntimeHandle {
    let snapshot = sample_runtime_snapshot();
    snapshot.endpoint_runtime_handle(&sample_selected_backend(address, "default/orders:8443"))
}

fn sample_traffic_topology(backend_name: &str) -> ntgw_observability::TrafficTopology {
    ntgw_observability::TrafficTopology::from_parts(
        "listener",
        "Http",
        "default",
        "orders",
        backend_name,
    )
}

fn assert_valid_traceparent(traceparent: &str) {
    let segments: Vec<_> = traceparent.split('-').collect();
    assert_eq!(segments.len(), 4, "traceparent should have 4 segments");
    assert_eq!(segments[0].len(), 2, "version should be 2 hex chars");
    assert_eq!(segments[1].len(), 32, "trace id should be 32 hex chars");
    assert_eq!(segments[2].len(), 16, "span id should be 16 hex chars");
    assert_eq!(segments[3].len(), 2, "trace flags should be 2 hex chars");
}

fn traceparent_trace_id(traceparent: &str) -> &str {
    traceparent
        .split('-')
        .nth(1)
        .expect("traceparent trace id segment")
}

fn collect_selected_addresses(
    snapshot: &Snapshot,
    request: &RequestMeta,
    attempts: usize,
) -> Vec<String> {
    (0..attempts)
        .map(|_| {
            snapshot
                .select_backend(request)
                .expect("backend")
                .backend
                .address
        })
        .collect()
}
