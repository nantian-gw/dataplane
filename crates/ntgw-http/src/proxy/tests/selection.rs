use std::collections::BTreeMap;

use ntgw_ir::{
    BackendEndpoint, BackendRef, BackendSelectionError, Filter, MatchedHttpPath,
    RequestMirrorFilter, SelectedHttpRoute,
};

use super::super::route_filters_have_request_mirror;
use super::super::selection::selected_backend_from_http_route;

#[test]
fn route_filters_have_request_mirror_detects_mirror_filters_only() {
    assert!(!route_filters_have_request_mirror(&[]));
    assert!(!route_filters_have_request_mirror(&[Filter {
        filter_type: "RequestHeaderModifier".to_string(),
        ..Filter::default()
    }]));
    assert!(route_filters_have_request_mirror(&[Filter {
        filter_type: "RequestMirror".to_string(),
        request_mirror: Some(RequestMirrorFilter {
            backend_ref: BackendRef {
                namespace: "default".to_string().into(),
                name: "mirror".to_string().into(),
                port: 8080,
                ..BackendRef::default()
            },
            ..RequestMirrorFilter::default()
        }),
        ..Filter::default()
    }]));
}

#[test]
fn selected_backend_from_http_route_preserves_success_fields() {
    let route = SelectedHttpRoute {
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(2),
        route_annotations: BTreeMap::from([(
            "gateway.nantian.dev/access-log-mode".to_string(),
            "json".to_string(),
        )]),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        filters: Vec::new(),
        matched_http_path: MatchedHttpPath {
            path: "/orders".to_string(),
            path_type: "PathPrefix".to_string(),
        },
        backend: Some(BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        }),
        backend_name: Some("default/orders:8080".to_string()),
        backend_error: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        route_policy: None,
    };

    let selected = selected_backend_from_http_route(route, true)
        .expect("route conversion should not error")
        .expect("selected backend");

    assert_eq!(selected.route_name, "orders");
    assert_eq!(selected.route_namespace, "default");
    assert_eq!(selected.rule_index, Some(2));
    assert_eq!(selected.listener_name, "default/gw/http");
    assert_eq!(selected.listener_protocol, "LISTENER_PROTOCOL_HTTP");
    assert_eq!(selected.backend_name, "default/orders:8080");
    assert_eq!(selected.backend.address, "10.0.0.10");
    assert_eq!(
        selected
            .route_annotations
            .get("gateway.nantian.dev/access-log-mode")
            .map(String::as_str),
        Some("json")
    );
}

#[test]
fn selected_backend_from_http_route_drops_annotations_when_access_log_disabled() {
    let route = SelectedHttpRoute {
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        route_annotations: BTreeMap::from([("annotation".to_string(), "value".to_string())]),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        filters: Vec::new(),
        matched_http_path: MatchedHttpPath::default(),
        backend: Some(BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        }),
        backend_name: Some("default/orders:8080".to_string()),
        backend_error: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        route_policy: None,
    };

    let selected = selected_backend_from_http_route(route, false)
        .expect("route conversion should not error")
        .expect("selected backend");

    assert!(selected.route_annotations.is_empty());
}

#[test]
fn selected_backend_from_http_route_forwards_backend_error() {
    let route = SelectedHttpRoute {
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        filters: Vec::new(),
        matched_http_path: MatchedHttpPath::default(),
        backend: None,
        backend_name: None,
        backend_error: Some(BackendSelectionError::NoHealthyBackends),
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        route_policy: None,
    };

    let err = selected_backend_from_http_route(route, true).expect_err("backend error");

    assert_eq!(err, BackendSelectionError::NoHealthyBackends);
}
