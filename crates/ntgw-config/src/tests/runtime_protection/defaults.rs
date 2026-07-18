use super::*;

#[test]
fn parses_runtime_protection_defaults() {
    let default_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
node_id: dp
cluster: kind
control_plane_addr: http://127.0.0.1:18080
admin_addr: 127.0.0.1:19080
"#,
    )
    .expect("default config should parse");

    assert_eq!(default_cfg.runtime_protection.http_global_inflight_limit, 0);
    assert_eq!(
        default_cfg.runtime_protection.http_listener_inflight_limit,
        0
    );
    assert_eq!(default_cfg.runtime_protection.http_route_inflight_limit, 0);
    assert_eq!(
        default_cfg
            .runtime_protection
            .http_backend_circuit_breaker_max_requests,
        0
    );
    assert_eq!(
        default_cfg
            .runtime_protection
            .http_global_rate_limit_requests_per_second,
        0
    );
    assert_eq!(
        default_cfg.runtime_protection.http_global_rate_limit_burst,
        0
    );
    assert_eq!(
        default_cfg
            .runtime_protection
            .http_listener_rate_limit_requests_per_second,
        0
    );
    assert_eq!(
        default_cfg
            .runtime_protection
            .http_listener_rate_limit_burst,
        0
    );
    assert_eq!(
        default_cfg
            .runtime_protection
            .http_route_rate_limit_requests_per_second,
        0
    );
    assert_eq!(
        default_cfg.runtime_protection.http_route_rate_limit_burst,
        0
    );
    assert_eq!(
        default_cfg.runtime_protection.http_max_request_body_bytes,
        10 * 1024 * 1024
    );
    assert_eq!(
        default_cfg.runtime_protection.http_max_request_header_bytes,
        65_536
    );
    assert_eq!(
        default_cfg.runtime_protection.tcp_global_connection_limit,
        0
    );
    assert_eq!(
        default_cfg.runtime_protection.tcp_listener_connection_limit,
        0
    );
    assert_eq!(default_cfg.runtime_protection.udp_global_datagram_limit, 0);
    assert_eq!(
        default_cfg.runtime_protection.udp_listener_datagram_limit,
        0
    );
}
