use super::*;

#[test]
fn parses_runtime_protection_overrides() {
    let custom_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
runtimeProtection:
  httpGlobalInflightLimit: 128
  httpListenerInflightLimit: 32
  httpRouteInflightLimit: 8
  httpBackendCircuitBreakerMaxRequests: 12
  httpGlobalRateLimitRequestsPerSecond: 400
  httpGlobalRateLimitBurst: 80
  httpListenerRateLimitRequestsPerSecond: 120
  httpListenerRateLimitBurst: 24
  httpRouteRateLimitRequestsPerSecond: 30
  httpRouteRateLimitBurst: 6
  httpMaxRequestBodyBytes: 1048576
  httpMaxRequestHeaderBytes: 8192
  tcpGlobalConnectionLimit: 256
  tcpListenerConnectionLimit: 64
  udpGlobalDatagramLimit: 512
  udpListenerDatagramLimit: 96
"#,
    )
    .expect("custom runtime protection config should parse");

    assert_eq!(
        custom_cfg.runtime_protection.http_global_inflight_limit,
        128
    );
    assert_eq!(
        custom_cfg.runtime_protection.http_listener_inflight_limit,
        32
    );
    assert_eq!(custom_cfg.runtime_protection.http_route_inflight_limit, 8);
    assert_eq!(
        custom_cfg
            .runtime_protection
            .http_backend_circuit_breaker_max_requests,
        12
    );
    assert_eq!(
        custom_cfg
            .runtime_protection
            .http_global_rate_limit_requests_per_second,
        400
    );
    assert_eq!(
        custom_cfg.runtime_protection.http_global_rate_limit_burst,
        80
    );
    assert_eq!(
        custom_cfg
            .runtime_protection
            .http_listener_rate_limit_requests_per_second,
        120
    );
    assert_eq!(
        custom_cfg.runtime_protection.http_listener_rate_limit_burst,
        24
    );
    assert_eq!(
        custom_cfg
            .runtime_protection
            .http_route_rate_limit_requests_per_second,
        30
    );
    assert_eq!(custom_cfg.runtime_protection.http_route_rate_limit_burst, 6);
    assert_eq!(
        custom_cfg.runtime_protection.http_max_request_body_bytes,
        1_048_576
    );
    assert_eq!(
        custom_cfg.runtime_protection.http_max_request_header_bytes,
        8_192
    );
    assert_eq!(
        custom_cfg.runtime_protection.tcp_global_connection_limit,
        256
    );
    assert_eq!(
        custom_cfg.runtime_protection.tcp_listener_connection_limit,
        64
    );
    assert_eq!(custom_cfg.runtime_protection.udp_global_datagram_limit, 512);
    assert_eq!(
        custom_cfg.runtime_protection.udp_listener_datagram_limit,
        96
    );
}
