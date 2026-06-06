#[test]
fn reset_request_context_clears_cached_backend_state() {
    let mut ctx = RequestContext {
        started_at: None,
        started_at_unix_ms: 0,
        upstream_connect_started_at: Some(std::time::Instant::now()),
        backend: "default/orders:8443".to_string(),
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        route_kind: "Http".to_string(),
        retry_attempts: 2,
        upstream_pool_hits: 3,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 9,
        upstream_connect_latency_ms_max: 9,
        upstream_connect_latency_ms_buckets: {
            let mut buckets = [0; ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
            buckets[ntgw_observability::upstream_connect_latency_ms_bucket_index(9)] = 1;
            buckets
        },
        response_flags: "UF".to_string(),
        selected_backend: Some(Arc::new(sample_selected_backend(
            "127.0.0.1",
            "default/orders:8443",
        ))),
        selected_backend_config: Some(Arc::new(SelectedBackendConfig {
            runtime: sample_runtime_handle("10.0.0.10"),
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
        })),
        ..RequestContext::default()
    };

    reset_request_context(&mut ctx, true);

    assert!(ctx.started_at.is_some());
    assert!(ctx.started_at_unix_ms > 0);
    assert!(ctx.upstream_connect_started_at.is_none());
    assert!(ctx.backend.is_empty());
    assert!(ctx.route_name.is_empty());
    assert!(ctx.route_namespace.is_empty());
    assert!(ctx.route_kind.is_empty());
    assert_eq!(ctx.retry_attempts, 0);
    assert_eq!(ctx.upstream_pool_hits, 0);
    assert_eq!(ctx.upstream_pool_misses, 0);
    assert_eq!(ctx.upstream_peer_build_failures, 0);
    assert_eq!(ctx.upstream_connect_latency_ms, 0);
    assert_eq!(ctx.upstream_connect_latency_ms_max, 0);
    assert_eq!(
        ctx.upstream_connect_latency_ms_buckets.iter().sum::<u32>(),
        0
    );
    assert!(ctx.response_flags.is_empty());
    assert!(ctx.selected_backend.is_none());
    assert!(ctx.selected_backend_config.is_none());
}

#[test]
fn reset_request_context_skips_unix_timestamp_when_access_log_disabled() {
    let mut ctx = RequestContext {
        started_at_unix_ms: 123,
        ..RequestContext::default()
    };

    reset_request_context(&mut ctx, false);

    assert!(ctx.started_at.is_some());
    assert_eq!(ctx.started_at_unix_ms, 0);
}

#[test]
fn clear_completed_request_context_clears_request_buffers() {
    let mut ctx = RequestContext {
        client_ip: "192.0.2.10".to_string(),
        host: "example.com".to_string(),
        method: "GET".to_string(),
        path: "/orders".to_string(),
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        route_kind: "Http".to_string(),
        backend: "default/orders:8080".to_string(),
        response_flags: "UF".to_string(),
        route_annotations: BTreeMap::from([("team".to_string(), "edge".to_string())]),
        request_mirrors: vec![],
        ..RequestContext::default()
    };

    clear_completed_request_context(&mut ctx);

    assert!(ctx.client_ip.is_empty());
    assert!(ctx.host.is_empty());
    assert!(ctx.method.is_empty());
    assert!(ctx.path.is_empty());
    assert!(ctx.route_name.is_empty());
    assert!(ctx.route_namespace.is_empty());
    assert!(ctx.route_kind.is_empty());
    assert!(ctx.backend.is_empty());
    assert!(ctx.response_flags.is_empty());
    assert!(ctx.route_annotations.is_empty());
}

#[test]
fn request_context_keeps_selected_backend_storage_pointer_sized() {
    let ctx = RequestContext::default();
    assert!(
        std::mem::size_of_val(&ctx.selected_backend) <= std::mem::size_of::<usize>() * 2,
        "selected backend storage should stay pointer-sized, got {} bytes",
        std::mem::size_of_val(&ctx.selected_backend)
    );
}
