#[test]
fn listener_runtime_status_exposes_recent_recovery_history() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![Listener {
            name: "web".to_string(),
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failure("v1", "web", "bind conflict");
    runtime.observe_http_listener_reload_result("v2", &["web".to_string()], &[], &[]);
    let runtime_snapshot = runtime.snapshot();

    let status =
        build_listener_runtime_status(&snapshot.listeners[0], &snapshot, &runtime_snapshot);
    assert_eq!(status.listener_attempts, 2);
    assert_eq!(status.listener_failures, 1);
    assert_eq!(status.listener_current_status, "accepted");
    assert!(status.listener_current_accepted);
    assert!(!status.listener_current_retained);
    assert!(!status.listener_current_rejected);
    assert!(!status.listener_current_stale);
    assert!(!status.listener_attention_required);
    assert!(status.listener_attention_reasons.is_empty());
    assert_eq!(status.listener_last_good_version, "v2");
    assert!(status.listener_has_ever_failed);
    assert!(status.listener_recovered_from_failure);
    assert!(!status.listener_awaiting_current_attempt);
    assert!(!status.listener_current_attempt_blocked);
    assert!(!status.listener_unrecovered_current_snapshot_failure);
    assert!(!status.listener_unrecovered_historical_failure);
    assert_eq!(status.listener_recovery_version, "v2");
    assert!(status.listener_recovery_unix_seconds > 0);
    assert_eq!(status.listener_serving_version, "v2");
    assert!(status.listener_serving_current_snapshot);
    assert!(!status.listener_serving_last_good_snapshot);
    assert_eq!(status.listener_serving_state, "current-accepted");
    assert_eq!(status.listener_recovery_state, "recovered");
    assert_eq!(status.listener_last_failure_version, "v1");
    assert_eq!(status.listener_recent_events.len(), 2);
    assert_eq!(status.listener_recent_events[0].status, "accepted");
    assert_eq!(status.listener_recent_events[0].version, "v2");
    assert_eq!(status.listener_recent_events[0].message, "");
    assert_eq!(status.listener_recent_events[1].status, "rejected");
    assert_eq!(status.listener_recent_events[1].version, "v1");
    assert_eq!(status.listener_recent_events[1].message, "bind conflict");
}
