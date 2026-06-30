#[test]
fn listener_runtime_status_tracks_owning_runtime_plane() {
    let snapshot = Snapshot {
        id: "v1".to_string(),
        listeners: vec![
            Listener {
                name: "web".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "tcp".to_string().into(),
                protocol: "LISTENER_PROTOCOL_TCP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failure("v1", "web", "bind conflict");
    runtime.observe_stream_listener_reload_success("v1");
    let runtime_snapshot = runtime.snapshot();

    let http = build_listener_runtime_status(&snapshot.listeners[0], &snapshot, &runtime_snapshot);
    let stream =
        build_listener_runtime_status(&snapshot.listeners[1], &snapshot, &runtime_snapshot);

    assert_eq!(http.runtime_plane, "http");
    assert!(http.runtime_required);
    assert_eq!(http.runtime_current_status, "rejected");
    assert!(!http.runtime_current_accepted);
    assert!(http.runtime_current_rejected);
    assert_eq!(http.listener_current_status, "rejected");
    assert!(!http.listener_current_accepted);
    assert!(!http.listener_current_retained);
    assert!(http.listener_current_rejected);
    assert!(!http.listener_current_stale);
    assert!(http.listener_attention_required);
    assert_eq!(
        http.listener_attention_reasons,
        vec!["rejected".to_string(), "unrecovered_failure".to_string()]
    );
    assert!(http.listener_current_failure);
    assert!(!http.listener_awaiting_current_attempt);
    assert!(http.listener_current_attempt_blocked);
    assert!(http.listener_unrecovered_current_snapshot_failure);
    assert!(!http.listener_unrecovered_historical_failure);
    assert_eq!(http.listener_current_failure_version, "v1");
    assert_eq!(http.listener_current_failure_message, "bind conflict");
    assert_eq!(http.listener_attempts, 1);
    assert_eq!(http.listener_failures, 1);
    assert_eq!(http.listener_last_attempt_version, "v1");
    assert_eq!(http.listener_last_good_version, "");
    assert_eq!(http.listener_serving_version, "");
    assert!(!http.listener_serving_current_snapshot);
    assert!(!http.listener_serving_last_good_snapshot);
    assert_eq!(http.listener_serving_state, "none");
    assert_eq!(http.listener_recovery_state, "unrecovered-current");
    assert_eq!(http.listener_last_failure_version, "v1");
    assert_eq!(http.listener_last_failure_message, "bind conflict");
    assert!(http.listener_last_failure_unix_seconds > 0);
    assert!(http.listener_has_ever_failed);
    assert!(!http.listener_recovered_from_failure);
    assert_eq!(http.listener_recovery_version, "");
    assert_eq!(http.listener_recovery_unix_seconds, 0);
    assert_eq!(http.listener_recent_events.len(), 1);
    assert_eq!(http.listener_recent_events[0].status, "rejected");
    assert_eq!(http.listener_recent_events[0].version, "v1");
    assert_eq!(http.listener_recent_events[0].message, "bind conflict");
    assert!(http.listener_recent_events[0].unix_seconds > 0);

    assert_eq!(stream.runtime_plane, "stream");
    assert!(stream.runtime_required);
    assert_eq!(stream.runtime_current_status, "accepted");
    assert!(stream.runtime_current_accepted);
    assert!(!stream.runtime_current_rejected);
    assert_eq!(stream.listener_current_status, "pending");
    assert!(!stream.listener_current_accepted);
    assert!(!stream.listener_current_retained);
    assert!(!stream.listener_current_rejected);
    assert!(!stream.listener_current_stale);
    assert!(stream.listener_attention_required);
    assert_eq!(
        stream.listener_attention_reasons,
        vec!["pending".to_string()]
    );
    assert!(!stream.listener_current_failure);
    assert!(stream.listener_awaiting_current_attempt);
    assert!(!stream.listener_current_attempt_blocked);
    assert!(!stream.listener_unrecovered_current_snapshot_failure);
    assert!(!stream.listener_unrecovered_historical_failure);
    assert_eq!(stream.listener_current_failure_version, "");
    assert_eq!(stream.listener_current_failure_message, "");
    assert_eq!(stream.listener_attempts, 0);
    assert_eq!(stream.listener_failures, 0);
    assert_eq!(stream.listener_serving_version, "");
    assert!(!stream.listener_serving_current_snapshot);
    assert!(!stream.listener_serving_last_good_snapshot);
    assert_eq!(stream.listener_serving_state, "none");
    assert_eq!(stream.listener_recovery_state, "awaiting-current");
    assert!(!stream.listener_has_ever_failed);
    assert!(!stream.listener_recovered_from_failure);
    assert_eq!(stream.listener_recovery_version, "");
    assert_eq!(stream.listener_recovery_unix_seconds, 0);
    assert!(stream.listener_recent_events.is_empty());
}

#[test]
fn listener_runtime_status_tracks_shared_tls_plane() {
    let snapshot = Snapshot {
        id: "v1".to_string(),
        listeners: vec![
            Listener {
                name: "https".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "passthrough".to_string().into(),
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let runtime = RuntimeStats::shared();
    runtime.observe_tls_listener_reload_result(
        "v1",
        &["https".to_string(), "passthrough".to_string()],
        &[],
        &[],
    );
    let runtime_snapshot = runtime.snapshot();

    let https = build_listener_runtime_status(&snapshot.listeners[0], &snapshot, &runtime_snapshot);
    let passthrough =
        build_listener_runtime_status(&snapshot.listeners[1], &snapshot, &runtime_snapshot);

    assert_eq!(https.runtime_plane, "tls");
    assert!(https.runtime_required);
    assert_eq!(https.runtime_current_status, "accepted");
    assert_eq!(passthrough.runtime_plane, "tls");
    assert!(passthrough.runtime_required);
    assert_eq!(passthrough.runtime_current_status, "accepted");
}
