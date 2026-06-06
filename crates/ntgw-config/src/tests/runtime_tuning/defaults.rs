use super::*;

#[test]
fn parses_runtime_tuning_defaults() {
    let default_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
"#,
    )
    .expect("default config should parse");

    assert_eq!(
        default_cfg
            .runtime_tuning
            .http_reload_retry_interval()
            .as_millis(),
        1_000
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .stream_reload_retry_interval()
            .as_millis(),
        1_000
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .downstream_read_timeout()
            .expect("default downstream read timeout")
            .as_millis(),
        60_000
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .http_keepalive_request_limit()
            .expect("default keepalive request limit"),
        1000
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .http_max_connection_age()
            .expect("default max connection age")
            .as_millis(),
        3_600_000
    );
    assert_eq!(
        default_cfg.runtime_tuning.request_mirror_max_concurrency,
        1_024
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .udp_response_idle_timeout()
            .as_millis(),
        500
    );
    assert_eq!(
        default_cfg.runtime_tuning.tcp_proxy_buffer_bytes(),
        16 * 1024
    );
    assert!(default_cfg
        .runtime_tuning
        .tcp_session_idle_timeout()
        .is_none());
    assert!(default_cfg
        .runtime_tuning
        .tcp_max_connection_age()
        .is_none());
    assert!(default_cfg.runtime_tuning.retry_budget_enabled());
    assert_eq!(default_cfg.runtime_tuning.retry_budget_ratio_percent(), 20);
    assert_eq!(default_cfg.runtime_tuning.retry_budget_burst(), 16);
    assert_eq!(
        default_cfg
            .runtime_tuning
            .graceful_drain_period()
            .as_millis(),
        0
    );
    assert!(!default_cfg.runtime_tuning.active_health_check_enabled());
    assert_eq!(
        default_cfg
            .runtime_tuning
            .active_health_check_interval()
            .as_millis(),
        5_000
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .active_health_check_timeout()
            .as_millis(),
        1_000
    );
    assert_eq!(
        default_cfg
            .runtime_tuning
            .active_health_check_unhealthy_threshold(),
        2
    );
    assert!(default_cfg
        .runtime_tuning
        .downstream_tcp_keepalive()
        .is_none());
    assert!(default_cfg
        .runtime_tuning
        .upstream_tcp_keepalive()
        .is_none());

    let capacity = default_cfg.runtime_tuning.http_capacity();
    assert_eq!(capacity.worker_threads, 0);
    assert_eq!(capacity.accept_concurrency, 0);
    assert_eq!(capacity.upstream_keepalive_pool_size, 0);
    assert_eq!(capacity.reuse_port, None);
}

#[test]
fn parses_partial_http_capacity_with_nested_defaults() {
    let empty_capacity_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeTuning:
  httpCapacity: {}
"#,
    )
    .expect("partial http capacity config should parse");

    let capacity = empty_capacity_cfg.runtime_tuning.http_capacity();
    assert_eq!(capacity.worker_threads, 0);
    assert_eq!(capacity.accept_concurrency, 0);
    assert_eq!(capacity.upstream_keepalive_pool_size, 0);
    assert_eq!(capacity.reuse_port, None);

    let override_only_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeTuning:
  httpCapacity:
    workerThreads: 4
"#,
    )
    .expect("partial http capacity override config should parse");

    let capacity = override_only_cfg.runtime_tuning.http_capacity();
    assert_eq!(capacity.worker_threads, 4);
    assert_eq!(capacity.accept_concurrency, 0);
    assert_eq!(capacity.upstream_keepalive_pool_size, 0);
    assert_eq!(capacity.reuse_port, None);
}

#[test]
fn rejects_legacy_http_capacity_profile() {
    let result = serde_yaml::from_str::<DataPlaneConfig>(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeTuning:
  httpCapacity:
    profile: legacy
"#,
    );

    assert!(
        result.is_err(),
        "HTTP capacity profile should be removed from the config surface"
    );
}

#[test]
fn rejects_high_concurrency_http_capacity_profile() {
    let result = serde_yaml::from_str::<DataPlaneConfig>(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeTuning:
  httpCapacity:
    profile: highConcurrency
"#,
    );

    assert!(
        result.is_err(),
        "HTTP capacity profile should be removed from the config surface"
    );
}
