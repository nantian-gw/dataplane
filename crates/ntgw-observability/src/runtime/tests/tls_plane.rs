#[test]
fn runtime_stats_snapshot_tracks_tls_plane_state() {
    let stats = RuntimeStats::shared();
    stats.observe_tls_runtime_started();
    stats.observe_tls_listener_reload_failure("v2", "default/gw/tls", "bind failed");
    stats.observe_tls_listener_reload_result("v3", &["default/gw/tls".to_string()], &[], &[]);

    let snapshot = stats.snapshot();
    assert!(snapshot.tls_runtime_running);
    assert_eq!(snapshot.tls_listener_reload_failures, 1);
    assert_eq!(snapshot.tls_last_reload_attempt_version, "v3");
    assert_eq!(snapshot.tls_last_good_reload_version, "v3");
    assert_eq!(snapshot.tls_last_reload_failure_version, "");
    assert_eq!(snapshot.tls_last_reload_failure_listener, "");
    assert_eq!(snapshot.tls_last_reload_failure_message, "");
    assert!(snapshot.tls_current_failures.is_empty());
    let events = &snapshot
        .tls_listener_progress
        .get("default/gw/tls")
        .expect("tls listener progress")
        .recent_events;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].status, "accepted");
    assert_eq!(events[0].version, "v3");
    assert_eq!(events[1].status, "rejected");
    assert_eq!(events[1].version, "v2");
    assert_eq!(events[1].message, "bind failed");
}
