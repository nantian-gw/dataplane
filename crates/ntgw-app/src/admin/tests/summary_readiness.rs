use super::*;

#[test]
fn summary_view_marks_pending_snapshot_without_last_good_as_not_ready() {
    let shared = Snapshot::shared();
    *shared.write() = fixture_snapshot();
    shared.write().id = "v2".to_string();
    let runtime = RuntimeStats::shared();
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    let value = build_summary_value(&state);
    assert_eq!(value["ready"], false);
    assert_eq!(value["readinessState"], "not-ready");
    assert_eq!(
        value["readinessReason"],
        "current-snapshot-pending-without-last-good"
    );
}

#[test]
fn summary_view_marks_pending_snapshot_with_last_good_as_serving_last_good() {
    let shared = Snapshot::shared();
    *shared.write() = fixture_snapshot();
    shared.write().id = "v2".to_string();
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v1", &["web".to_string()], &[], &[]);
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    let value = build_summary_value(&state);
    assert_eq!(value["ready"], true);
    assert_eq!(value["readinessState"], "serving-last-good");
    assert_eq!(
        value["readinessReason"],
        "serving-last-good-while-current-pending"
    );
}

#[test]
fn summary_view_marks_stale_xds_snapshot_as_not_ready() {
    let state = test_state(None);
    set_snapshot_freshness_timeout(&state, Duration::ZERO);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_stream_runtime_started();
    state.xds.observe_stream_connected();
    state.xds.observe_stream_disconnected();

    let value = build_summary_value(&state);
    assert_eq!(value["ready"], false);
    assert_eq!(value["readinessState"], "not-ready");
    assert_eq!(value["readinessReason"], "xds-snapshot-stale");
    assert_eq!(value["xdsStreamConnected"], false);
}

#[test]
fn summary_view_marks_supervisor_shutdown_as_not_ready() {
    let state = test_state(None);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_stream_runtime_started();
    state.runtime.observe_supervisor_started();
    state
        .runtime
        .observe_supervisor_shutdown_requested("signal: sigterm");

    let value = build_summary_value(&state);
    assert_eq!(value["ready"], false);
    assert_eq!(value["readinessState"], "not-ready");
    assert_eq!(value["readinessReason"], "supervisor-shutting-down");
}
