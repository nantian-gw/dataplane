use super::*;

#[test]
fn parses_extended_logging_options() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
log:
  level: debug
  format: text
  addSource: true
  includeTarget: true
  includeThreadIds: true
  includeThreadNames: true
  nonBlocking: false
  nonBlockingBufferedLines: 2048
  dropWhenFull: false
accessLog:
  enabled: true
  path: /var/log/ntgw/access.log
  format: "%EVENT% %ROUTE_NAME%"
  mode: json
  sampleRate: 0.5
  routeAnnotationPrefix: custom.dev/access-log-
"#,
    )
    .expect("config should parse");

    assert!(cfg.log.add_source);
    assert!(cfg.log.include_target);
    assert!(cfg.log.include_thread_ids);
    assert!(cfg.log.include_thread_names);
    assert!(!cfg.log.non_blocking);
    assert_eq!(cfg.log.non_blocking_buffered_lines, 2_048);
    assert!(!cfg.log.drop_when_full);
    assert_eq!(cfg.access_log.path, "/var/log/ntgw/access.log");
    assert_eq!(cfg.access_log.format, "%EVENT% %ROUTE_NAME%");
    assert_eq!(cfg.access_log.mode, "json");
    assert_eq!(cfg.access_log.sample_rate, 0.5);
    assert_eq!(
        cfg.access_log.route_annotation_prefix,
        "custom.dev/access-log-"
    );
}

#[test]
fn parses_open_telemetry_logging_defaults_and_overrides() {
    let default_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
"#,
    )
    .expect("default config should parse");

    assert!(!default_cfg.log.open_telemetry.enabled);
    assert!(default_cfg.log.open_telemetry.endpoint.is_empty());
    assert_eq!(default_cfg.log.open_telemetry.protocol, "grpc");
    assert_eq!(default_cfg.log.open_telemetry.timeout_ms, 3_000);
    assert!(!default_cfg.log.open_telemetry.insecure);
    assert_eq!(default_cfg.log.open_telemetry.sample_ratio, 1.0);
    assert_eq!(
        default_cfg.log.open_telemetry.service_name,
        "nantian-dataplane"
    );
    assert!(default_cfg.log.open_telemetry.service_namespace.is_empty());
    assert!(default_cfg.log.non_blocking);
    assert_eq!(default_cfg.log.non_blocking_buffered_lines, 65_536);
    assert!(default_cfg.log.drop_when_full);

    let custom_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
log:
  openTelemetry:
    enabled: true
    endpoint: http://otel-collector.observability:4317
    protocol: grpc
    timeoutMs: 4500
    insecure: true
    sampleRatio: 0.25
    serviceName: edge-gateway
    serviceNamespace: gateways
"#,
    )
    .expect("custom config should parse");

    assert!(custom_cfg.log.open_telemetry.enabled);
    assert_eq!(
        custom_cfg.log.open_telemetry.endpoint,
        "http://otel-collector.observability:4317"
    );
    assert_eq!(custom_cfg.log.open_telemetry.protocol, "grpc");
    assert_eq!(custom_cfg.log.open_telemetry.timeout_ms, 4_500);
    assert!(custom_cfg.log.open_telemetry.insecure);
    assert_eq!(custom_cfg.log.open_telemetry.sample_ratio, 0.25);
    assert_eq!(custom_cfg.log.open_telemetry.service_name, "edge-gateway");
    assert_eq!(custom_cfg.log.open_telemetry.service_namespace, "gateways");
}

#[test]
fn parses_runtime_tls_asset_dir() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtime:
  tlsAssetDir: /var/lib/nantian-gw/tls
"#,
    )
    .expect("config should parse");

    assert_eq!(cfg.runtime.tls_asset_dir, "/var/lib/nantian-gw/tls");
}
