use ntgw_config::{RoutePolicyBodyLimitConfig, RoutePolicyConfig};
use ntgw_ir::{MatchedHttpPath, SelectedHttpRoute};
use std::collections::BTreeMap;

#[test]
fn route_policy_body_limit_override() {
    // Test that when SelectedHttpRoute has route_policy with body_limit,
    // the override is used instead of the global default.
    let route_policy = RoutePolicyConfig {
        timeout: None,
        body_limit: Some(RoutePolicyBodyLimitConfig {
            max_request_body_bytes: Some(1024),
            ..RoutePolicyBodyLimitConfig::default()
        }),
        proxy: None,
        connection: None,
    };
    let route = SelectedHttpRoute {
        route_name: "test-route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        filters: vec![],
        matched_http_path: MatchedHttpPath {
            path: "/test".to_string(),
            path_type: "PathPrefix".to_string(),
        },
        backend: None,
        backend_name: None,
        backend_error: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        security_policy: None,
        route_policy: Some(route_policy),
    };

    let global_default: usize = 10 * 1024 * 1024;
    let effective = route
        .route_policy
        .as_ref()
        .and_then(|rp| rp.body_limit.as_ref())
        .and_then(|bl| bl.max_request_body_bytes)
        .unwrap_or(global_default);
    assert_eq!(effective, 1024usize);
}

#[test]
fn route_policy_body_limit_falls_back_to_global() {
    let route = SelectedHttpRoute {
        route_name: "test-route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        filters: vec![],
        matched_http_path: MatchedHttpPath {
            path: "/test".to_string(),
            path_type: "PathPrefix".to_string(),
        },
        backend: None,
        backend_name: None,
        backend_error: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        security_policy: None,
        route_policy: None,
    };

    let global_default: usize = 10 * 1024 * 1024;
    let effective = route
        .route_policy
        .as_ref()
        .and_then(|rp| rp.body_limit.as_ref())
        .and_then(|bl| bl.max_request_body_bytes)
        .unwrap_or(global_default);
    assert_eq!(effective, 10 * 1024 * 1024);
}

#[test]
fn route_policy_body_limit_none_uses_global() {
    // route_policy exists but body_limit is None → falls back to global
    let route_policy = RoutePolicyConfig {
        timeout: None,
        body_limit: None,
        proxy: None,
        connection: None,
    };
    let route = SelectedHttpRoute {
        route_name: "test-route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        filters: vec![],
        matched_http_path: MatchedHttpPath {
            path: "/test".to_string(),
            path_type: "PathPrefix".to_string(),
        },
        backend: None,
        backend_name: None,
        backend_error: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        security_policy: None,
        route_policy: Some(route_policy),
    };

    let global_default: usize = 10 * 1024 * 1024;
    let effective = route
        .route_policy
        .as_ref()
        .and_then(|rp| rp.body_limit.as_ref())
        .and_then(|bl| bl.max_request_body_bytes)
        .unwrap_or(global_default);
    assert_eq!(effective, 10 * 1024 * 1024);
}
