use super::*;

#[test]
fn parses_extended_logging_options() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
log:
  level: debug
  format: text
  add_source: true
  include_target: true
  include_thread_ids: true
  include_thread_names: true
  non_blocking: false
  non_blocking_buffered_lines: 2048
  drop_when_full: false
access_log:
  enabled: true
  path: /var/log/ntgw/access.log
  format: "%EVENT% %ROUTE_NAME%"
  mode: json
  sample_rate: 0.5
  route_annotation_prefix: custom.dev/access-log-
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
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
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
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
log:
  open_telemetry:
    enabled: true
    endpoint: http://otel-collector.observability:4317
    protocol: grpc
    timeout_ms: 4500
    insecure: true
    sample_ratio: 0.25
    service_name: edge-gateway
    service_namespace: gateways
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
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
runtime:
  tls_asset_dir: /var/lib/nantian-gw/tls
"#,
    )
    .expect("config should parse");

    assert_eq!(cfg.runtime.tls_asset_dir, "/var/lib/nantian-gw/tls");
}

#[test]
fn parses_named_access_log_formats() {
    let cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
access_log:
  enabled: true
  path: /var/log/ntgw/access.log
  mode: text
  format_name: main
  formats:
    main: '$remote_addr "$request" $status'
    upstream_debug: '$request_id $upstream_addr $upstream_status'
  format: "%EVENT% %ROUTE_NAME%"
  sample_rate: 0.5
  route_annotation_prefix: custom.dev/access-log-
"#,
    )
    .expect("config should parse");

    assert_eq!(cfg.access_log.mode, "text");
    assert_eq!(cfg.access_log.format_name, "main");
    assert_eq!(
        cfg.access_log.formats.get("main").map(String::as_str),
        Some(r#"$remote_addr "$request" $status"#)
    );
    assert_eq!(
        cfg.access_log
            .formats
            .get("upstream_debug")
            .map(String::as_str),
        Some("$request_id $upstream_addr $upstream_status")
    );
    assert_eq!(cfg.access_log.format, "%EVENT% %ROUTE_NAME%");
}
