use super::*;

#[test]
fn parses_xds_transport_defaults_and_overrides() {
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
        default_cfg.xds_transport.connect_timeout().as_millis(),
        5_000
    );
    assert_eq!(
        default_cfg.xds_transport.keepalive_interval().as_millis(),
        10_000
    );
    assert_eq!(
        default_cfg.xds_transport.keepalive_timeout().as_millis(),
        5_000
    );
    assert_eq!(
        default_cfg
            .xds_transport
            .initial_reconnect_backoff()
            .as_millis(),
        2_000
    );
    assert_eq!(
        default_cfg
            .xds_transport
            .max_reconnect_backoff()
            .as_millis(),
        30_000
    );
    assert_eq!(default_cfg.xds_transport.apply_timeout().as_millis(), 3_000);
    assert_eq!(
        default_cfg.xds_transport.apply_poll_interval().as_millis(),
        25
    );
    assert_eq!(
        default_cfg.xds_transport.stale_stream_timeout().as_millis(),
        30_000
    );
    assert_eq!(
        default_cfg
            .xds_transport
            .snapshot_freshness_timeout()
            .as_millis(),
        90_000
    );

    let custom_cfg: DataPlaneConfig = serde_yaml::from_str(
        r#"
nodeId: dp
cluster: kind
controlPlaneAddr: http://127.0.0.1:18080
adminAddr: 127.0.0.1:19080
xdsTransport:
  connectTimeoutMs: 9000
  keepaliveIntervalMs: 15000
  keepaliveTimeoutMs: 4000
  initialReconnectBackoffMs: 750
  maxReconnectBackoffMs: 12000
  applyTimeoutMs: 4500
  applyPollIntervalMs: 40
  staleStreamTimeoutMs: 45000
  snapshotFreshnessTimeoutMs: 55000
"#,
    )
    .expect("custom transport config should parse");

    assert_eq!(
        custom_cfg.xds_transport.connect_timeout().as_millis(),
        9_000
    );
    assert_eq!(
        custom_cfg.xds_transport.keepalive_interval().as_millis(),
        15_000
    );
    assert_eq!(
        custom_cfg.xds_transport.keepalive_timeout().as_millis(),
        4_000
    );
    assert_eq!(
        custom_cfg
            .xds_transport
            .initial_reconnect_backoff()
            .as_millis(),
        750
    );
    assert_eq!(
        custom_cfg.xds_transport.max_reconnect_backoff().as_millis(),
        12_000
    );
    assert_eq!(custom_cfg.xds_transport.apply_timeout().as_millis(), 4_500);
    assert_eq!(
        custom_cfg.xds_transport.apply_poll_interval().as_millis(),
        40
    );
    assert_eq!(
        custom_cfg.xds_transport.stale_stream_timeout().as_millis(),
        45_000
    );
    assert_eq!(
        custom_cfg
            .xds_transport
            .snapshot_freshness_timeout()
            .as_millis(),
        55_000
    );
}
