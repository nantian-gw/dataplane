use super::*;

#[test]
fn parses_runtime_tuning_overrides() {
    let custom_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeTuning:
  httpReloadRetryIntervalMs: 2500
  streamReloadRetryIntervalMs: 1800
  downstreamReadTimeoutMs: 250
  httpMaxConnectionAgeMs: 3750
  httpKeepaliveRequestLimit: 7
  httpCapacity:
    workerThreads: 4
    acceptConcurrency: 3
    upstreamKeepalivePoolSize: 1024
    reusePort: false
  requestMirrorMaxConcurrency: 16
  udpResponseIdleTimeoutMs: 900
  tcpProxyBufferBytes: 65536
  tcpSessionIdleTimeoutMs: 4200
  tcpMaxConnectionAgeMs: 12500
  retryBudgetEnabled: false
  retryBudgetRatioPercent: 35
  retryBudgetBurst: 9
  gracefulDrainPeriodMs: 4500
  activeHealthCheckEnabled: false
  activeHealthCheckIntervalMs: 8000
  activeHealthCheckTimeoutMs: 1500
  activeHealthCheckUnhealthyThreshold: 4
  downstreamTcpKeepalive:
    enabled: true
    idleMs: 61000
    intervalMs: 14000
    probeCount: 5
    userTimeoutMs: 7000
  upstreamTcpKeepalive:
    enabled: true
    idleMs: 45000
    intervalMs: 11000
    probeCount: 3
    userTimeoutMs: 0
"#,
    )
    .expect("custom runtime tuning config should parse");

    assert_eq!(
        custom_cfg
            .runtime_tuning
            .http_reload_retry_interval()
            .as_millis(),
        2_500
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .stream_reload_retry_interval()
            .as_millis(),
        1_800
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .downstream_read_timeout()
            .expect("custom downstream read timeout")
            .as_millis(),
        250
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .http_max_connection_age()
            .expect("custom http max connection age")
            .as_millis(),
        3_750
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .http_keepalive_request_limit()
            .expect("http keepalive request limit"),
        7
    );
    let capacity = custom_cfg.runtime_tuning.http_capacity();
    assert_eq!(capacity.worker_threads, 4);
    assert_eq!(capacity.accept_concurrency, 3);
    assert_eq!(capacity.upstream_keepalive_pool_size, 1_024);
    assert_eq!(capacity.reuse_port, Some(false));
    assert_eq!(custom_cfg.runtime_tuning.request_mirror_max_concurrency, 16);
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .udp_response_idle_timeout()
            .as_millis(),
        900
    );
    assert_eq!(custom_cfg.runtime_tuning.tcp_proxy_buffer_bytes(), 65_536);
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .tcp_session_idle_timeout()
            .expect("tcp session idle timeout")
            .as_millis(),
        4_200
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .tcp_max_connection_age()
            .expect("tcp max connection age")
            .as_millis(),
        12_500
    );
    assert!(!custom_cfg.runtime_tuning.retry_budget_enabled());
    assert_eq!(custom_cfg.runtime_tuning.retry_budget_ratio_percent(), 35);
    assert_eq!(custom_cfg.runtime_tuning.retry_budget_burst(), 9);
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .graceful_drain_period()
            .as_millis(),
        4_500
    );
    assert!(!custom_cfg.runtime_tuning.active_health_check_enabled());
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .active_health_check_interval()
            .as_millis(),
        8_000
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .active_health_check_timeout()
            .as_millis(),
        1_500
    );
    assert_eq!(
        custom_cfg
            .runtime_tuning
            .active_health_check_unhealthy_threshold(),
        4
    );

    let downstream = custom_cfg
        .runtime_tuning
        .downstream_tcp_keepalive()
        .expect("downstream keepalive");
    assert_eq!(downstream.idle.as_millis(), 61_000);
    assert_eq!(downstream.interval.as_millis(), 14_000);
    assert_eq!(downstream.count, 5);
    #[cfg(target_os = "linux")]
    assert_eq!(downstream.user_timeout.as_millis(), 7_000);

    let upstream = custom_cfg
        .runtime_tuning
        .upstream_tcp_keepalive()
        .expect("upstream keepalive");
    assert_eq!(upstream.idle.as_millis(), 45_000);
    assert_eq!(upstream.interval.as_millis(), 11_000);
    assert_eq!(upstream.count, 3);
    #[cfg(target_os = "linux")]
    assert_eq!(upstream.user_timeout.as_millis(), 0);
}
