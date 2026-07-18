use crate::RoutePolicyConfig;

#[test]
fn route_policy_full_round_trip() {
    let yaml = r#"
timeout:
  request: 30000
  backend_request: 25000
  connect: 5000
  next_upstream: 15000
body_limit:
  max_request_body_bytes: 1048576
  request_body_buffer_bytes: 65536
  max_request_header_bytes: 16384
proxy:
  request_buffering: true
  response_buffering: false
  buffer_size: 4096
  buffer_count: 4
connection:
  keepalive_requests: 100
  keepalive_time: 10000
  keepalive_timeout: 5000
  upstream_keepalive_pool_size: 10
  upstream_keepalive_idle: 60000
"#;
    let config: RoutePolicyConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.timeout.is_some());
    let t = config.timeout.unwrap();
    assert_eq!(t.request, Some(30_000));
    assert_eq!(t.backend_request, Some(25_000));
    assert_eq!(t.connect, Some(5_000));
    assert_eq!(t.next_upstream, Some(15_000));

    let body = config.body_limit.unwrap();
    assert_eq!(body.max_request_body_bytes, Some(1_048_576));
    assert_eq!(body.request_body_buffer_bytes, Some(65_536));
    assert_eq!(body.max_request_header_bytes, Some(16_384));

    let proxy = config.proxy.unwrap();
    assert_eq!(proxy.request_buffering, Some(true));
    assert_eq!(proxy.response_buffering, Some(false));
    assert_eq!(proxy.buffer_size, Some(4_096));
    assert_eq!(proxy.buffer_count, Some(4));

    let conn = config.connection.unwrap();
    assert_eq!(conn.keepalive_requests, Some(100));
    assert_eq!(conn.keepalive_time, Some(10_000));
    assert_eq!(conn.keepalive_timeout, Some(5_000));
    assert_eq!(conn.upstream_keepalive_pool_size, Some(10));
    assert_eq!(conn.upstream_keepalive_idle, Some(60_000));
}

#[test]
fn route_policy_empty_yields_none_subconfigs() {
    let yaml = "{}";
    let config: RoutePolicyConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.timeout.is_none());
    assert!(config.body_limit.is_none());
    assert!(config.proxy.is_none());
    assert!(config.connection.is_none());
}

#[test]
fn route_policy_partial_timeout_only() {
    let yaml = r#"
timeout:
  request: 120000
"#;
    let config: RoutePolicyConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.timeout.is_some());
    let t = config.timeout.unwrap();
    assert_eq!(t.request, Some(120_000));
    assert!(t.backend_request.is_none());
    assert!(t.connect.is_none());
    assert!(t.next_upstream.is_none());
    assert!(config.body_limit.is_none());
    assert!(config.proxy.is_none());
    assert!(config.connection.is_none());
}
