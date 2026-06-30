#[test]
fn tracing_options_include_open_telemetry_identity() {
    let mut cfg = base_config();
    cfg.node_id = "dp-local-1".to_string();
    cfg.log = LogConfig {
        level: "debug".to_string(),
        format: "json".to_string(),
        add_source: true,
        include_target: true,
        include_thread_ids: false,
        include_thread_names: false,
        non_blocking: true,
        non_blocking_buffered_lines: 32_768,
        drop_when_full: true,
        open_telemetry: ntgw_config::OpenTelemetryConfig {
            enabled: true,
            endpoint: "http://otel-collector.observability:4317".to_string(),
            protocol: "grpc".to_string().into(),
            timeout_ms: 4_500,
            insecure: true,
            sample_ratio: 0.25,
            service_name: "edge-gateway".to_string(),
            service_namespace: "gateways".to_string(),
        },
        sentry: SentryConfig::default(),
    };

    let options = to_tracing_options(&cfg);

    assert_eq!(options.level, "debug");
    assert_eq!(options.format, "json");
    assert!(options.add_source);
    assert!(options.include_target);
    assert!(options.non_blocking);
    assert_eq!(options.non_blocking_buffered_lines, 32_768);
    assert!(options.drop_when_full);
    assert!(options.open_telemetry.enabled);
    assert_eq!(
        options.open_telemetry.endpoint,
        "http://otel-collector.observability:4317"
    );
    assert_eq!(options.open_telemetry.protocol, "grpc");
    assert_eq!(options.open_telemetry.timeout_ms, 4_500);
    assert!(options.open_telemetry.insecure);
    assert_eq!(options.open_telemetry.sample_ratio, 0.25);
    assert_eq!(options.open_telemetry.service_name, "edge-gateway");
    assert_eq!(options.open_telemetry.service_namespace, "gateways");
    assert_eq!(options.open_telemetry.service_instance_id, "dp-local-1");
    assert_eq!(options.open_telemetry.deployment_environment, "kind");
}

#[test]
fn http_runtime_disables_request_tracing_without_open_telemetry() {
    let cfg = base_config();

    let runtime = to_http_runtime_options(&cfg);

    assert!(!runtime.request_tracing_enabled);
}

#[test]
fn http_runtime_enables_request_tracing_with_open_telemetry() {
    let mut cfg = base_config();
    cfg.log.open_telemetry.enabled = true;

    let runtime = to_http_runtime_options(&cfg);

    assert!(runtime.request_tracing_enabled);
}
