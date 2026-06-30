#[test]
fn listener_runtime_status_tracks_retained_current_version() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "web".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "api".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v1", &["web".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_result(
        "v2",
        &["api".to_string()],
        &["web".to_string()],
        &[],
    );
    let runtime_snapshot = runtime.snapshot();

    let web = build_listener_runtime_status(&snapshot.listeners[0], &snapshot, &runtime_snapshot);
    let api = build_listener_runtime_status(&snapshot.listeners[1], &snapshot, &runtime_snapshot);

    assert_eq!(web.listener_attempts, 1);
    assert_eq!(web.listener_current_status, "retained");
    assert!(!web.listener_current_accepted);
    assert!(web.listener_current_retained);
    assert!(!web.listener_current_rejected);
    assert!(!web.listener_current_stale);
    assert!(!web.listener_attention_required);
    assert!(web.listener_attention_reasons.is_empty());
    assert_eq!(web.listener_last_good_version, "v2");
    assert!(!web.listener_has_ever_failed);
    assert!(!web.listener_recovered_from_failure);
    assert_eq!(web.listener_recovery_version, "");
    assert_eq!(web.listener_recovery_unix_seconds, 0);
    assert_eq!(web.listener_serving_version, "v2");
    assert!(web.listener_serving_current_snapshot);
    assert!(!web.listener_serving_last_good_snapshot);
    assert_eq!(web.listener_serving_state, "current-retained");
    assert_eq!(web.listener_recovery_state, "steady");
    assert_eq!(web.listener_recent_events.len(), 2);
    assert_eq!(web.listener_recent_events[0].status, "retained");
    assert_eq!(web.listener_recent_events[0].version, "v2");
    assert_eq!(web.listener_recent_events[1].status, "accepted");
    assert_eq!(web.listener_recent_events[1].version, "v1");

    assert_eq!(api.listener_attempts, 1);
    assert_eq!(api.listener_current_status, "accepted");
    assert!(api.listener_current_accepted);
    assert!(!api.listener_current_retained);
    assert!(!api.listener_current_rejected);
    assert!(!api.listener_current_stale);
    assert!(!api.listener_attention_required);
    assert!(api.listener_attention_reasons.is_empty());
    assert_eq!(api.listener_last_good_version, "v2");
    assert!(!api.listener_has_ever_failed);
    assert!(!api.listener_recovered_from_failure);
    assert_eq!(api.listener_recovery_version, "");
    assert_eq!(api.listener_recovery_unix_seconds, 0);
    assert_eq!(api.listener_serving_version, "v2");
    assert!(api.listener_serving_current_snapshot);
    assert!(!api.listener_serving_last_good_snapshot);
    assert_eq!(api.listener_serving_state, "current-accepted");
    assert_eq!(api.listener_recovery_state, "steady");
    assert_eq!(api.listener_recent_events[0].status, "accepted");
    assert_eq!(api.listener_recent_events[0].version, "v2");
}

#[test]
fn listener_runtime_status_marks_serving_last_good_snapshot_after_rejection() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![Listener {
            name: "web".to_string().into(),
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v1", &["web".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_failure("v2", "web", "bind conflict");
    let runtime_snapshot = runtime.snapshot();

    let status =
        build_listener_runtime_status(&snapshot.listeners[0], &snapshot, &runtime_snapshot);
    assert_eq!(status.listener_current_status, "rejected");
    assert!(!status.listener_current_accepted);
    assert!(!status.listener_current_retained);
    assert!(status.listener_current_rejected);
    assert!(!status.listener_current_stale);
    assert!(status.listener_attention_required);
    assert_eq!(
        status.listener_attention_reasons,
        vec!["rejected".to_string(), "unrecovered_failure".to_string()]
    );
    assert_eq!(status.listener_serving_version, "v1");
    assert!(!status.listener_serving_current_snapshot);
    assert!(status.listener_serving_last_good_snapshot);
    assert_eq!(status.listener_serving_state, "last-good-rejected");
    assert_eq!(status.listener_recovery_state, "unrecovered-current");
    assert!(status.listener_has_ever_failed);
    assert!(!status.listener_recovered_from_failure);
    assert!(!status.listener_awaiting_current_attempt);
    assert!(status.listener_current_attempt_blocked);
    assert!(status.listener_unrecovered_current_snapshot_failure);
    assert!(!status.listener_unrecovered_historical_failure);
    assert_eq!(status.listener_recovery_version, "");
    assert_eq!(status.listener_recovery_unix_seconds, 0);
}

#[test]
fn listener_runtime_status_marks_stale_when_last_good_lags_snapshot() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![Listener {
            name: "web".to_string().into(),
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v1", &["web".to_string()], &[], &[]);
    let runtime_snapshot = runtime.snapshot();

    let status =
        build_listener_runtime_status(&snapshot.listeners[0], &snapshot, &runtime_snapshot);
    assert_eq!(status.listener_current_status, "stale");
    assert!(!status.listener_current_accepted);
    assert!(!status.listener_current_retained);
    assert!(!status.listener_current_rejected);
    assert!(status.listener_current_stale);
    assert!(status.listener_attention_required);
    assert_eq!(status.listener_attention_reasons, vec!["stale".to_string()]);
    assert_eq!(status.listener_serving_version, "v1");
    assert!(!status.listener_serving_current_snapshot);
    assert!(status.listener_serving_last_good_snapshot);
    assert_eq!(status.listener_serving_state, "last-good-stale");
    assert_eq!(status.listener_recovery_state, "drifted-last-good");
    assert!(!status.listener_has_ever_failed);
    assert!(!status.listener_recovered_from_failure);
    assert!(!status.listener_awaiting_current_attempt);
    assert!(!status.listener_current_attempt_blocked);
    assert!(!status.listener_unrecovered_current_snapshot_failure);
    assert!(!status.listener_unrecovered_historical_failure);
    assert_eq!(status.listener_recovery_version, "");
    assert_eq!(status.listener_recovery_unix_seconds, 0);
}
