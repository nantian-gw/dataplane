use crate::proxy::UpstreamTuningOptions;
use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendRef, CompiledSelectedHttpBackend, HttpMatch, HttpRoute,
    HttpRule, Listener, RouteKind, SelectedBackendRuntimeIds, Snapshot,
};
use pingora::http::RequestHeader;

use super::super::selection::SelectedBackendConfigCache;
use super::super::{
    build_upstream_peer_for_fast_path, cache_fast_selected_backend_state,
    fast_path_request_features_are_safe, fast_path_request_from_header,
    prepare_initial_request_state, RequestContext, SelectedBackendConfig, UpstreamPeerAddress,
};

#[test]
fn fast_path_request_from_header_borrows_routing_fields_without_materializing_headers() {
    let mut request = RequestHeader::build("GET", b"/orders?id=123", None).expect("request header");
    request.insert_header("host", "example.com").expect("host");

    let view = fast_path_request_from_header(&request, 80);

    assert_eq!(view.host, Some("example.com"));
    assert_eq!(view.port, 80);
    assert_eq!(view.path, "/orders");
    assert_eq!(view.method, "GET");
    assert!(!view.is_grpc);
}

#[test]
fn fast_path_request_from_header_detects_grpc_content_type() {
    let mut request =
        RequestHeader::build("POST", b"/pkg.Service/Call", None).expect("request header");
    request
        .insert_header("content-type", "application/grpc")
        .expect("content-type");

    let view = fast_path_request_from_header(&request, 80);

    assert!(view.is_grpc);
}

#[test]
fn fast_path_is_allowed_only_when_request_features_are_disabled() {
    assert!(fast_path_request_features_are_safe(
        false, false, false, false
    ));
    assert!(!fast_path_request_features_are_safe(
        true, false, false, false
    ));
    assert!(!fast_path_request_features_are_safe(
        false, true, false, false
    ));
    assert!(!fast_path_request_features_are_safe(
        false, false, true, false
    ));
    assert!(!fast_path_request_features_are_safe(
        false, false, false, true
    ));
}

#[test]
fn initial_request_state_carries_fast_path_selection_from_current_snapshot() {
    let mut snapshot = sample_fast_path_snapshot();
    snapshot.rebuild_runtime_indexes();
    let cache = SelectedBackendConfigCache;
    let mut request = RequestHeader::build("GET", b"/orders?id=123", None).expect("request header");
    request.insert_header("host", "example.com").expect("host");
    request
        .insert_header("content-length", "123")
        .expect("content-length");
    let mut ctx = RequestContext::default();

    let state = prepare_initial_request_state(
        &snapshot, &cache, &mut ctx, &request, 80, None, None, false, false, 0, 0,
    )
    .expect("initial request state");

    assert_eq!(ctx.method, "GET");
    assert_eq!(ctx.declared_request_body_bytes, 0);
    assert_eq!(state.request_header_bytes, 0);
    assert!(!state.misdirected_request);
    assert!(state.request_source_ip.is_none());
    let selected = state
        .fast_path_selected
        .as_ref()
        .map(|selected| selected.selected.backend_name.as_str());
    assert_eq!(selected, Some("default/orders:8080"));
}

#[test]
fn initial_request_state_skips_snapshot_version_when_unobserved() {
    let mut snapshot = sample_fast_path_snapshot();
    snapshot.rebuild_runtime_indexes();
    let cache = SelectedBackendConfigCache;
    let mut request = RequestHeader::build("GET", b"/orders", None).expect("request header");
    request.insert_header("host", "example.com").expect("host");
    let mut ctx = RequestContext::default();

    let state = prepare_initial_request_state(
        &snapshot, &cache, &mut ctx, &request, 80, None, None, false, false, 0, 0,
    )
    .expect("initial request state");

    assert!(state.fast_path_selected.is_some());
    assert!(ctx.snapshot_version.is_empty());
}

#[test]
fn cache_fast_selected_backend_state_fills_context_without_full_selected_backend() {
    let selected = CompiledSelectedHttpBackend {
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        matched_http_path: aeg_ir::MatchedHttpPath {
            path: "/".to_string(),
            path_type: "PathPrefix".to_string(),
        },
        runtime_ids: SelectedBackendRuntimeIds::default(),
    };
    let mut ctx = RequestContext::default();

    cache_fast_selected_backend_state(&mut ctx, selected, true);

    assert_eq!(ctx.route_kind, "Http");
    assert_eq!(ctx.route_name, "orders");
    assert_eq!(ctx.route_namespace, "default");
    assert_eq!(ctx.listener_name, "default/gw/http");
    assert_eq!(ctx.listener_protocol, "HTTP");
    assert_eq!(ctx.backend, "default/orders:8080");
    assert!(ctx.selected_backend.is_none());
    assert!(ctx.fast_selected_backend.is_some());
}

#[test]
fn cache_fast_selected_backend_state_can_skip_context_display_strings() {
    let selected = CompiledSelectedHttpBackend {
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        matched_http_path: aeg_ir::MatchedHttpPath::default(),
        runtime_ids: SelectedBackendRuntimeIds::default(),
    };
    let mut ctx = RequestContext::default();

    cache_fast_selected_backend_state(&mut ctx, selected, false);

    assert!(ctx.route_kind.is_empty());
    assert!(ctx.route_name.is_empty());
    assert!(ctx.route_namespace.is_empty());
    assert!(ctx.listener_name.is_empty());
    assert!(ctx.listener_protocol.is_empty());
    assert!(ctx.backend.is_empty());
    assert!(ctx.selected_backend.is_none());
    assert!(ctx.fast_selected_backend.is_some());
}

#[test]
fn fast_path_state_builds_upstream_peer_from_cached_config() {
    let selected = CompiledSelectedHttpBackend {
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        matched_http_path: aeg_ir::MatchedHttpPath::default(),
        runtime_ids: SelectedBackendRuntimeIds::default(),
    };
    let config = sample_fast_selected_backend_config("10.0.0.10", 8080);

    let peer = build_upstream_peer_for_fast_path(
        &selected,
        &config,
        None,
        &UpstreamTuningOptions::default(),
    )
    .expect("fast path upstream peer");

    assert_eq!(peer.options.connection_timeout, config.connect_timeout);
    assert_eq!(peer.options.read_timeout, config.request_timeout);
}

fn sample_fast_selected_backend_config(address: &str, port: u16) -> SelectedBackendConfig {
    let selected_backend = aeg_ir::SelectedBackend {
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        route_annotations: Default::default(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: address.to_string(),
            port: u32::from(port),
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };
    let snapshot = aeg_ir::Snapshot::default();

    SelectedBackendConfig {
        runtime: snapshot.endpoint_runtime_handle(&selected_backend),
        runtime_ids: SelectedBackendRuntimeIds::default(),
        peer_address: UpstreamPeerAddress::from_backend_address(address),
        peer_port: port,
        tls_enabled: false,
        sni: String::new(),
        use_http2: false,
        connect_timeout: Some(std::time::Duration::from_millis(500)),
        request_timeout: Some(std::time::Duration::from_secs(2)),
        backend_tls_validation: None,
        client_cert_key: None,
        traffic_topology: aeg_observability::TrafficTopology::from_parts(
            "default/gw/http",
            "Http",
            "default",
            "orders",
            "default/orders:8080",
        ),
    }
}

fn sample_fast_path_snapshot() -> Snapshot {
    Snapshot {
        id: "snapshot-1".to_string(),
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            port: 80,
            protocol: "HTTP".to_string(),
            attached_routes: vec!["default/orders".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "orders".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["example.com".to_string()],
            rules: vec![HttpRule {
                matches: vec![HttpMatch {
                    path: "/orders".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: "GET".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "orders".to_string(),
                    port: 8080,
                    weight: 1,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            namespace: "default".to_string(),
            name: "orders:8080".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
        }],
        ..Snapshot::default()
    }
}
