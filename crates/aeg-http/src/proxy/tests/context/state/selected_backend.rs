#[test]
fn cache_selected_backend_state_replaces_cached_policy_and_protocol() {
    let snapshot = sample_runtime_snapshot();
    let first = sample_selected_backend("10.0.0.10", "default/orders:8443");
    let second = sample_selected_backend("10.0.0.11", "default/orders:8443");
    let mut ctx = RequestContext::default();

    cache_selected_backend_state(
        &mut ctx,
        first,
        SelectedBackendConfig {
            runtime: snapshot.endpoint_runtime_handle(&sample_selected_backend(
                "10.0.0.10",
                "default/orders:8443",
            )),
            runtime_ids: Default::default(),
            peer_address: UpstreamPeerAddress::from_backend_address("10.0.0.10"),
            peer_port: 8443,
            tls_enabled: true,
            sni: "orders.default.svc".to_string(),
            use_http2: false,
            connect_timeout: Some(Duration::from_secs(1)),
            request_timeout: Some(Duration::from_secs(5)),
            backend_tls_validation: None,
            client_cert_key: None,
            traffic_topology: sample_traffic_topology("default/orders:8443"),
        },
        true,
    );

    cache_selected_backend_state(
        &mut ctx,
        second,
        SelectedBackendConfig {
            runtime: snapshot.endpoint_runtime_handle(&sample_selected_backend(
                "10.0.0.11",
                "default/orders:8443",
            )),
            runtime_ids: Default::default(),
            peer_address: UpstreamPeerAddress::from_backend_address("10.0.0.11"),
            peer_port: 8443,
            tls_enabled: false,
            sni: String::new(),
            use_http2: true,
            connect_timeout: None,
            request_timeout: None,
            backend_tls_validation: None,
            client_cert_key: None,
            traffic_topology: sample_traffic_topology("default/orders:8443"),
        },
        true,
    );

    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .map(|selected| selected.backend.address.as_str()),
        Some("10.0.0.11")
    );
    assert_eq!(
        ctx.selected_backend_config
            .as_ref()
            .map(|cfg| cfg.use_http2),
        Some(true)
    );
    assert!(!ctx.backend_observation_recorded);
}

#[test]
fn cache_selected_backend_state_caches_runtime_ids_from_config() {
    let snapshot = sample_runtime_snapshot();
    let selected = sample_selected_backend("10.0.0.10", "default/orders:8443");
    let runtime_ids = snapshot.selected_backend_runtime_ids(&selected);
    let mut ctx = RequestContext::default();

    cache_selected_backend_state(
        &mut ctx,
        selected,
        SelectedBackendConfig {
            runtime: snapshot.endpoint_runtime_handle(&sample_selected_backend(
                "10.0.0.10",
                "default/orders:8443",
            )),
            runtime_ids,
            peer_address: UpstreamPeerAddress::from_backend_address("10.0.0.10"),
            peer_port: 8443,
            tls_enabled: true,
            sni: "orders.default.svc".to_string(),
            use_http2: false,
            connect_timeout: Some(Duration::from_secs(1)),
            request_timeout: Some(Duration::from_secs(5)),
            backend_tls_validation: None,
            client_cert_key: None,
            traffic_topology: sample_traffic_topology("default/orders:8443"),
        },
        true,
    );

    assert_eq!(ctx.runtime_ids, runtime_ids);
}

#[test]
fn cache_selected_backend_state_skips_context_display_strings_when_not_observed() {
    let snapshot = sample_runtime_snapshot();
    let selected = sample_selected_backend("10.0.0.10", "default/orders:8443");
    let mut ctx = RequestContext::default();

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
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .map(|selected| selected.backend_name.as_str()),
        Some("default/orders:8443")
    );
}

#[test]
fn cache_selected_backend_state_keeps_filters_only_on_selected_backend() {
    let snapshot = sample_runtime_snapshot();
    let mut selected = sample_selected_backend("10.0.0.10", "default/orders:8443");
    selected.filters = vec![Filter {
        filter_type: "ResponseHeaderModifier".to_string(),
        ..Filter::default()
    }];
    let mut ctx = RequestContext::default();

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
        true,
    );

    assert_eq!(
        ctx.selected_backend.as_ref().map(|item| item.filters.len()),
        Some(1)
    );
    assert_eq!(
        format!("{ctx:?}").matches("filters: [").count(),
        1,
        "request context should not keep a second owned filters vector"
    );
}

#[test]
fn cache_selected_backend_state_keeps_session_persistence_only_on_selected_backend() {
    let snapshot = sample_runtime_snapshot();
    let mut selected = sample_selected_backend("10.0.0.10", "default/orders:8443");
    selected.session_persistence = Some(SessionPersistence {
        session_name: "gw-session".to_string(),
        session_type: "Cookie".to_string(),
        cookie: Some(CookieConfig {
            lifetime_type: "Session".to_string(),
        }),
        ..SessionPersistence::default()
    });
    let mut ctx = RequestContext::default();

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
        true,
    );

    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .and_then(|item| item.session_persistence.as_ref())
            .map(|policy| policy.session_name.as_str()),
        Some("gw-session")
    );
    assert_eq!(
        format!("{ctx:?}")
            .matches("session_persistence: Some")
            .count(),
        1,
        "request context should not keep a second owned session persistence policy"
    );
}

#[test]
fn cache_selected_backend_state_keeps_retry_policy_only_on_selected_backend() {
    let snapshot = sample_runtime_snapshot();
    let mut selected = sample_selected_backend("10.0.0.10", "default/orders:8443");
    selected.retry = Some(RetryPolicy {
        codes: vec![500, 503],
        attempts: 2,
        backoff: Some(std::time::Duration::from_millis(50)),
    });
    let mut ctx = RequestContext::default();

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
        true,
    );

    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .and_then(|item| item.retry.as_ref())
            .map(|policy| policy.codes.as_slice()),
        Some(&[500, 503][..])
    );
    assert_eq!(
        format!("{ctx:?}").matches("RetryPolicy").count(),
        1,
        "request context should not keep a second owned retry policy"
    );
}

#[test]
fn cache_selected_backend_preserves_retry_policy() {
    let mut ctx = RequestContext::default();

    cache_selected_backend(
        &mut ctx,
        SelectedBackend {
            route_kind: RouteKind::Http,
            route_name: "route".to_string(),
            route_namespace: "default".to_string(),
            rule_index: None,
            route_annotations: BTreeMap::new(),
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            backend: BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            },
            backend_name: "default/echo:8080".to_string(),
            filters: Vec::new(),
            matched_http_path: Some(MatchedHttpPath {
                path: "/".to_string(),
                path_type: "PathPrefix".to_string(),
            }),
            timeouts: None,
            retry: Some(RetryPolicy {
                codes: vec![500, 503],
                attempts: 2,
                backoff: Some(std::time::Duration::from_millis(50)),
            }),
            session_persistence: None,
            backend_tls: None,
        },
        true,
    );

    assert_eq!(ctx.backend, "default/echo:8080");
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .map(|item| item.backend_name.as_str()),
        Some("default/echo:8080")
    );
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .and_then(|item| item.retry.as_ref())
            .map(|policy| policy.codes.as_slice()),
        Some(&[500, 503][..])
    );
}

#[test]
fn cache_selected_backend_ref_preserves_retry_policy() {
    let mut ctx = RequestContext::default();
    let selected = SelectedBackend {
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/echo:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: Some(MatchedHttpPath {
            path: "/".to_string(),
            path_type: "PathPrefix".to_string(),
        }),
        timeouts: None,
        retry: Some(RetryPolicy {
            codes: vec![500, 503],
            attempts: 2,
            backoff: Some(std::time::Duration::from_millis(50)),
        }),
        session_persistence: None,
        backend_tls: None,
    };

    cache_selected_backend_ref(&mut ctx, &selected, true);

    assert_eq!(ctx.backend, "default/echo:8080");
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .map(|item| item.backend_name.as_str()),
        Some("default/echo:8080")
    );
    assert_eq!(selected.backend_name, "default/echo:8080");
    assert_eq!(
        ctx.selected_backend
            .as_ref()
            .and_then(|item| item.retry.as_ref())
            .map(|policy| policy.codes.as_slice()),
        Some(&[500, 503][..])
    );
}
