#[test]
fn runtime_tuning_fans_out_to_http_and_stream_runtime_options() {
    let mut cfg = base_config();
    cfg.runtime_tuning = RuntimeTuningConfig {
        http_reload_retry_interval_ms: 2_200,
        stream_reload_retry_interval_ms: 3_300,
        downstream_read_timeout_ms: 1_700,
        http_max_connection_age_ms: 3_750,
        http_keepalive_request_limit: 11,
        http_capacity: HttpCapacityConfig {
            worker_threads: 4,
            accept_concurrency: 3,
            upstream_keepalive_pool_size: 1_024,
            reuse_port: Some(false),
        },
        request_mirror_max_concurrency: 17,
        udp_response_idle_timeout_ms: 700,
        tcp_proxy_buffer_bytes: 96 * 1024,
        tcp_session_idle_timeout_ms: 4_200,
        tcp_max_connection_age_ms: 12_500,
        retry_budget_enabled: false,
        retry_budget_ratio_percent: 35,
        retry_budget_burst: 9,
        graceful_drain_period_ms: 4_500,
        active_health_check_enabled: true,
        active_health_check_interval_ms: 8_000,
        active_health_check_timeout_ms: 1_500,
        active_health_check_unhealthy_threshold: 3,
        downstream_tcp_keepalive: ntgw_config::TcpKeepaliveConfig {
            enabled: true,
            idle_ms: 65_000,
            interval_ms: 12_000,
            probe_count: 5,
            user_timeout_ms: 4_000,
        },
upstream_tcp_keepalive: ntgw_config::TcpKeepaliveConfig {
            enabled: true,
            idle_ms: 45_000,
            interval_ms: 10_000,
            probe_count: 4,
            user_timeout_ms: 3_000,
        },
        work_stealing: true,
        downstream_tcp_fastopen: Some(1024),
        downstream_dscp: Some(46),
        upstream_tcp_recv_buf: 262144,
        upstream_tcp_fast_open: true,
        upstream_connection_timeout_ms: 10_000,
        upstream_read_timeout_ms: 60_000,
        upstream_idle_timeout_ms: 120_000,
        upstream_dscp: Some(46),
        stream_upstream_pool_size: 0,
        stream_upstream_pool_idle_timeout_ms: 30_000,
        http_cache: ntgw_config::HttpCacheConfig::default(),
    };
    cfg.runtime_protection = RuntimeProtectionConfig {
        http_backend_circuit_breaker_max_requests: 12,
        http_max_request_body_bytes: 4_096,
        http_max_request_header_bytes: 8_192,
        http_global_rate_limit_requests_per_second: 400,
        http_global_rate_limit_burst: 80,
        http_listener_rate_limit_requests_per_second: 120,
        http_listener_rate_limit_burst: 24,
        http_route_rate_limit_requests_per_second: 30,
        http_route_rate_limit_burst: 6,
        ..RuntimeProtectionConfig::default()
    };

    let http = to_http_runtime_options(&cfg);
    assert_eq!(http.reload_retry_interval.as_millis(), 2_200);
    assert_eq!(
        http.downstream_read_timeout
            .expect("downstream read timeout")
            .as_millis(),
        1_700
    );
    assert_eq!(
        http.downstream_max_connection_age
            .expect("http max connection age")
            .as_millis(),
        3_750
    );
    assert_eq!(http.keepalive_request_limit, Some(11));
    assert_eq!(http.capacity.worker_threads, 4);
    assert_eq!(http.capacity.accept_concurrency, 3);
    assert_eq!(http.capacity.upstream_keepalive_pool_size, 1_024);
    assert_eq!(http.capacity.reuse_port, Some(false));
    assert_eq!(http.max_request_body_bytes, 4_096);
    assert_eq!(http.max_request_header_bytes, 8_192);
    assert!(!http.retry_budget.enabled);
    assert_eq!(http.retry_budget.ratio_percent, 35);
    assert_eq!(http.retry_budget.burst, 9);
    assert_eq!(http.circuit_breaker.backend_max_inflight_requests, 12);
    assert_eq!(http.rate_limit.global_requests_per_second, 400);
    assert_eq!(http.rate_limit.global_burst, 80);
    assert_eq!(http.rate_limit.listener_requests_per_second, 120);
    assert_eq!(http.rate_limit.listener_burst, 24);
    assert_eq!(http.rate_limit.route_requests_per_second, 30);
    assert_eq!(http.rate_limit.route_burst, 6);
    assert_eq!(
        http.downstream_tcp_keepalive
            .as_ref()
            .expect("downstream keepalive")
            .idle
            .as_millis(),
        65_000
    );
    assert_eq!(
        http.upstream_tcp_keepalive
            .as_ref()
            .expect("upstream keepalive")
            .interval
            .as_millis(),
        10_000
    );

    let stream = to_stream_runtime_options(&cfg);
    assert_eq!(stream.reload_retry_interval.as_millis(), 3_300);
    assert_eq!(stream.udp_response_idle_timeout.as_millis(), 700);
    assert_eq!(stream.tcp_proxy_buffer_bytes, 96 * 1024);
    assert_eq!(
        stream
            .tcp_session_idle_timeout
            .expect("tcp session idle timeout")
            .as_millis(),
        4_200
    );
    assert_eq!(
        stream
            .tcp_max_connection_age
            .expect("tcp max connection age")
            .as_millis(),
        12_500
    );
}
