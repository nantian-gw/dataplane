#[test]
fn record_upstream_connection_tracks_pool_hits_and_connect_latency() {
    let mut ctx = RequestContext {
        upstream_connect_started_at: Some(
            std::time::Instant::now() - std::time::Duration::from_millis(12),
        ),
        ..RequestContext::default()
    };

    record_upstream_connection(&mut ctx, false);
    assert_eq!(ctx.upstream_pool_hits, 0);
    assert_eq!(ctx.upstream_pool_misses, 1);
    assert!(ctx.upstream_connect_latency_ms >= 12);
    assert_eq!(
        ctx.upstream_connect_latency_ms_max,
        ctx.upstream_connect_latency_ms
    );
    let bucket_index = upstream_connect_latency_ms_bucket_index(ctx.upstream_connect_latency_ms);
    assert_eq!(ctx.upstream_connect_latency_ms_buckets[bucket_index], 1);

    ctx.upstream_connect_started_at = Some(std::time::Instant::now());
    record_upstream_connection(&mut ctx, true);
    assert_eq!(ctx.upstream_pool_hits, 1);
    assert_eq!(ctx.upstream_pool_misses, 1);
    assert_eq!(ctx.upstream_connect_latency_ms_buckets[bucket_index], 1);
    assert!(ctx.upstream_connect_started_at.is_none());
}

#[test]
fn record_upstream_peer_build_failure_tracks_and_resets_counter() {
    let mut ctx = RequestContext::default();

    record_upstream_peer_build_failure(&mut ctx);
    record_upstream_peer_build_failure(&mut ctx);
    assert_eq!(ctx.upstream_peer_build_failures, 2);

    clear_completed_request_context(&mut ctx);
    assert_eq!(ctx.upstream_peer_build_failures, 0);
}

#[test]
fn record_upstream_tls_handshake_failure_tracks_tls_errors_only() {
    let traffic = SharedTrafficStats::shared();
    let ctx = RequestContext {
        upstream_connect_started_at: Some(
            std::time::Instant::now() - std::time::Duration::from_millis(12),
        ),
        ..RequestContext::default()
    };

    let connect_refused = pingora::Error::new_up(pingora::ErrorType::ConnectRefused);
    record_upstream_tls_handshake_failure(&traffic, &ctx, &connect_refused);
    assert_eq!(traffic.snapshot().total_upstream_tls_handshake_failures, 0);

    let tls_failure = pingora::Error::new_up(pingora::ErrorType::TLSHandshakeFailure);
    record_upstream_tls_handshake_failure(&traffic, &ctx, &tls_failure);
    let snapshot = traffic.snapshot();
    assert_eq!(snapshot.total_upstream_tls_handshake_failures, 1);
    assert_eq!(
        snapshot.total_upstream_tls_handshake_failure_latency_observations,
        1
    );
    assert!(snapshot.total_upstream_tls_handshake_failure_latency_ms >= 12);
    assert_eq!(
        snapshot.max_upstream_tls_handshake_failure_latency_ms,
        snapshot.total_upstream_tls_handshake_failure_latency_ms
    );
    assert_eq!(
        snapshot
            .upstream_tls_handshake_failure_latency_ms_buckets
            .last()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        Some(("+Inf", 1))
    );

    let no_start_ctx = RequestContext::default();
    let tls_timeout = pingora::Error::new_up(pingora::ErrorType::TLSHandshakeTimedout);
    record_upstream_tls_handshake_failure(&traffic, &no_start_ctx, &tls_timeout);
    let snapshot = traffic.snapshot();
    assert_eq!(snapshot.total_upstream_tls_handshake_failures, 2);
    assert_eq!(
        snapshot.total_upstream_tls_handshake_failure_latency_observations,
        1
    );
}

#[test]
fn effective_http_protocol_defaults_to_grpc_for_grpc_route_context() {
    let ctx = RequestContext {
        route_kind: "Grpc".to_string(),
        ..RequestContext::default()
    };

    assert_eq!(effective_http_protocol(&ctx), "GRPC");
}

#[test]
fn normalize_ip_converts_ipv4_mapped_ipv6() {
    assert_eq!(
        normalize_ip(IpAddr::V6(Ipv4Addr::new(10, 1, 2, 3).to_ipv6_mapped())),
        "10.1.2.3"
    );
    assert_eq!(normalize_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)), "::1");
}
