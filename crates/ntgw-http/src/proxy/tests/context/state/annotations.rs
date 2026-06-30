#[test]
fn cache_selected_backend_skips_route_annotations_when_access_log_disabled() {
    let mut ctx = RequestContext {
        route_annotations: BTreeMap::from([("stale".to_string(), "1".to_string())]),
        ..RequestContext::default()
    };

    cache_selected_backend(
        &mut ctx,
        SelectedBackend { route_policy: None,
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            route_annotations: BTreeMap::from([("k".to_string(), "v".to_string())]),
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            backend: BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            },
            backend_name: "default/echo:8080".to_string(),
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: None,
        },
        false,
    );

    assert!(ctx.route_annotations.is_empty());
}

#[test]
fn cache_selected_backend_keeps_access_log_annotations_on_selected_backend_only() {
    let mut ctx = RequestContext::default();

    cache_selected_backend(
        &mut ctx,
        SelectedBackend { route_policy: None,
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            route_annotations: BTreeMap::from([("k".to_string(), "v".to_string())]),
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            backend: BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            },
            backend_name: "default/echo:8080".to_string(),
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: None,
        },
        true,
    );

    assert!(
        ctx.route_annotations.is_empty(),
        "selected backend annotations must not be cloned into RequestContext"
    );
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .and_then(|selected| selected.route_annotations.get("k"))
            .map(String::as_str),
        Some("v")
    );
}

#[test]
fn cache_route_annotations_skips_copy_when_access_log_disabled() {
    let mut ctx = RequestContext {
        route_annotations: BTreeMap::from([("stale".to_string(), "1".to_string())]),
        ..RequestContext::default()
    };

    cache_route_annotations(
        &mut ctx,
        &AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        &BTreeMap::from([("k".to_string(), "v".to_string())]),
    );

    assert!(ctx.route_annotations.is_empty());
}
