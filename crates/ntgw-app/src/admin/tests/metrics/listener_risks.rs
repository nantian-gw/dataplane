use super::*;

#[test]
fn render_metrics_exposes_listener_overlap_risks() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "pending-failed".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "rejected-failed".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "stale-failed".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    shared.store(Arc::new(snapshot));
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failure("v1", "pending-failed", "bind conflict");
    runtime.observe_http_listener_reload_result("v1", &["stale-failed".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_failure("v1", "stale-failed", "address in use");
    runtime.observe_http_listener_reload_failure("v2", "rejected-failed", "port busy");

    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    let metrics = render_metrics(&state);

    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_count 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_http_count 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_stream_count 0"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_none_count 0"
    ));
    assert!(
        metrics
            .contains("nantian_gateway_dataplane_listener_unrecovered_historical_failure_count 2")
    );
    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_http_count 2"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_stream_count 0"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_none_count 0"
    ));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_failure_recovery_severity_level 2")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_risk_pending_unrecovered_count 1")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_risk_rejected_unrecovered_count 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_risk_stale_unrecovered_count 1"));
}
