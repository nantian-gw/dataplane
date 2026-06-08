#[test]
fn cache_http_route_context_sets_route_fields_for_local_responses() {
    let mut ctx = RequestContext::default();

    cache_http_route_context(
        &mut ctx,
        HttpRouteContextFields {
            route_name: "orders",
            route_namespace: "default",
            route_annotations: &BTreeMap::from([(
                "gateway.nantian.dev/access-log-mode".to_string(),
                "json".to_string(),
            )]),
            listener_name: "default/gw/http",
            listener_protocol: "HTTP",
            backend_name: Some("default/orders:8080"),
        },
        &AccessLogOptions {
            enabled: true,
            ..AccessLogOptions::default()
        },
    );

    assert_eq!(ctx.route_kind, "Http");
    assert_eq!(ctx.route_name, "orders");
    assert_eq!(ctx.route_namespace, "default");
    assert_eq!(ctx.listener_name, "default/gw/http");
    assert_eq!(ctx.listener_protocol, "HTTP");
    assert_eq!(ctx.backend, "default/orders:8080");
    assert_eq!(
        ctx.route_annotations
            .get("gateway.nantian.dev/access-log-mode")
            .map(String::as_str),
        Some("json")
    );
}

#[test]
fn cache_http_route_context_skips_annotation_copy_when_access_log_disabled() {
    let mut ctx = RequestContext {
        route_annotations: BTreeMap::from([("stale".to_string(), "1".to_string())]),
        ..RequestContext::default()
    };

    cache_http_route_context(
        &mut ctx,
        HttpRouteContextFields {
            route_name: "orders",
            route_namespace: "default",
            route_annotations: &BTreeMap::from([("fresh".to_string(), "1".to_string())]),
            listener_name: "default/gw/http",
            listener_protocol: "HTTP",
            backend_name: None,
        },
        &AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
    );

    assert_eq!(ctx.route_kind, "Http");
    assert_eq!(ctx.route_name, "orders");
    assert_eq!(ctx.route_namespace, "default");
    assert_eq!(ctx.listener_name, "default/gw/http");
    assert_eq!(ctx.listener_protocol, "HTTP");
    assert!(ctx.backend.is_empty());
    assert!(ctx.route_annotations.is_empty());
}

#[test]
fn selected_backend_state_clears_preselected_http_route_context_when_not_observed() {
    let snapshot = sample_runtime_snapshot();
    let selected = sample_selected_backend("10.0.0.10", "default/orders:8443");
    let mut ctx = RequestContext::default();

    cache_http_route_context(
        &mut ctx,
        HttpRouteContextFields {
            route_name: "orders",
            route_namespace: "default",
            route_annotations: &BTreeMap::new(),
            listener_name: "default/gw/http",
            listener_protocol: "HTTP",
            backend_name: Some("default/orders:8443"),
        },
        &AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
    );

    cache_selected_backend_state(
        &mut ctx,
        selected,
        SelectedBackendConfig {
            runtime: snapshot.endpoint_runtime_handle(&sample_selected_backend(
                "10.0.0.10",
                "default/orders:8443",
            )),
            runtime_ids: Default::default(),
            peer_address: UpstreamPeerAddress::from_backend_address("10.0.0.10"),
            peer_port: 8443,
            tls_enabled: false,
            sni: String::new(),
            use_http2: false,
            connect_timeout: None,
            request_timeout: None,
            backend_tls_validation: None,
            client_cert_key: None,
            traffic_topology: sample_traffic_topology("default/orders:8443"),
        },
        false,
    );

    assert!(ctx.route_kind.is_empty());
    assert!(ctx.route_name.is_empty());
    assert!(ctx.route_namespace.is_empty());
    assert!(ctx.listener_name.is_empty());
    assert!(ctx.listener_protocol.is_empty());
    assert!(ctx.backend.is_empty());
    assert!(ctx.route_annotations.is_empty());
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .map(|selected| selected.backend_name.as_str()),
        Some("default/orders:8443")
    );
}
