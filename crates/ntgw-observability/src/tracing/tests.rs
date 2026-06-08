use super::{
    OpenTelemetryOptions, OpenTelemetryProtocol, TracingOptions, build_log_writer,
    resolve_open_telemetry_options,
};

#[test]
fn tracing_options_default_to_non_blocking_lossy_stdout() {
    let options = TracingOptions::default();

    assert!(options.non_blocking);
    assert_eq!(options.non_blocking_buffered_lines, 65_536);
    assert!(options.drop_when_full);
}

#[test]
fn build_log_writer_returns_guard_for_non_blocking_output() {
    let options = TracingOptions {
        non_blocking: true,
        non_blocking_buffered_lines: 2,
        drop_when_full: true,
        ..TracingOptions::default()
    };

    let (_writer, guard) = build_log_writer(&options);

    assert!(guard.is_some());
}

#[test]
fn build_log_writer_can_use_direct_stdout() {
    let options = TracingOptions {
        non_blocking: false,
        ..TracingOptions::default()
    };

    let (_writer, guard) = build_log_writer(&options);

    assert!(guard.is_none());
}

#[test]
fn resolve_open_telemetry_options_returns_none_when_disabled() {
    let options = OpenTelemetryOptions::default();

    let resolved = resolve_open_telemetry_options(&options).expect("otel options should parse");

    assert!(resolved.is_none());
}

#[test]
fn resolve_open_telemetry_options_rejects_missing_endpoint() {
    let options = OpenTelemetryOptions {
        enabled: true,
        ..OpenTelemetryOptions::default()
    };

    let err = resolve_open_telemetry_options(&options).expect_err("missing endpoint");

    assert!(err.to_string().contains("endpoint"));
}

#[test]
fn resolve_open_telemetry_options_rejects_unsupported_protocol() {
    let options = OpenTelemetryOptions {
        enabled: true,
        endpoint: "http://collector:4318".to_string(),
        protocol: "http".to_string(),
        ..OpenTelemetryOptions::default()
    };

    let err = resolve_open_telemetry_options(&options).expect_err("unsupported protocol");

    assert!(err.to_string().contains("protocol"));
}

#[test]
fn resolve_open_telemetry_options_clamps_sample_ratio_and_preserves_identity() {
    let options = OpenTelemetryOptions {
        enabled: true,
        endpoint: "http://collector:4317".to_string(),
        protocol: "grpc".to_string(),
        timeout_ms: 4_500,
        insecure: true,
        sample_ratio: 4.0,
        service_name: "edge-gateway".to_string(),
        service_namespace: "gateways".to_string(),
        service_instance_id: "dp-local-1".to_string(),
        deployment_environment: "kind".to_string(),
    };

    let resolved = resolve_open_telemetry_options(&options)
        .expect("otel options should parse")
        .expect("otel options should be enabled");

    assert_eq!(resolved.protocol, OpenTelemetryProtocol::Grpc);
    assert_eq!(resolved.endpoint, "http://collector:4317");
    assert_eq!(resolved.timeout_ms, 4_500);
    assert!(resolved.insecure);
    assert_eq!(resolved.sample_ratio, 1.0);
    assert_eq!(resolved.service_name, "edge-gateway");
    assert_eq!(resolved.service_namespace, "gateways");
    assert_eq!(resolved.service_instance_id, "dp-local-1");
    assert_eq!(resolved.deployment_environment, "kind");
}
