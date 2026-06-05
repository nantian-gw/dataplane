#[test]
fn xds_runtime_config_includes_transport_and_tls_options() {
    let mut cfg = base_config();
    cfg.node_id = "dp-local-1".to_string();
    cfg.control_plane_addr = "https://controlplane.nantian-gw.svc:18080".to_string();
    cfg.xds_tls = aeg_config::XdsTlsConfig {
        enabled: true,
        ca_path: "/etc/nantian-gw/ca.crt".to_string(),
        cert_path: "/etc/nantian-gw/tls.crt".to_string(),
        key_path: "/etc/nantian-gw/tls.key".to_string(),
        domain_name: "controlplane.nantian-gw.svc".to_string(),
    };
    cfg.xds_transport = aeg_config::XdsTransportConfig {
        connect_timeout_ms: 4_000,
        keepalive_interval_ms: 20_000,
        keepalive_timeout_ms: 7_000,
        initial_reconnect_backoff_ms: 900,
        max_reconnect_backoff_ms: 12_000,
        apply_timeout_ms: 5_500,
        apply_poll_interval_ms: 250,
        stale_stream_timeout_ms: 45_000,
        snapshot_freshness_timeout_ms: 75_000,
    };

    let xds = to_xds_runtime_config(&cfg);

    assert_eq!(
        xds.connect_options.endpoint,
        "https://controlplane.nantian-gw.svc:18080"
    );
    let tls = xds.connect_options.tls.expect("xds tls");
    assert_eq!(tls.ca_path, "/etc/nantian-gw/ca.crt");
    assert_eq!(tls.cert_path, "/etc/nantian-gw/tls.crt");
    assert_eq!(tls.key_path, "/etc/nantian-gw/tls.key");
    assert_eq!(tls.domain_name, "controlplane.nantian-gw.svc");
    assert_eq!(
        xds.connect_options.transport.connect_timeout.as_millis(),
        4_000
    );
    assert_eq!(
        xds.connect_options
            .transport
            .initial_reconnect_backoff
            .as_millis(),
        900
    );
    assert_eq!(
        xds.connect_options
            .transport
            .stale_stream_timeout
            .as_millis(),
        45_000
    );
    assert_eq!(xds.node_id, "dp-local-1");
    assert_eq!(xds.cluster, "kind");
}
