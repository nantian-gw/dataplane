#[test]
fn runtime_stats_snapshot_accumulates_counters() {
    let stats = RuntimeStats::shared();
    stats.observe_http_listener_reload_attempt("v1");
    stats.observe_http_listener_reload_failures(
        "v1",
        &[
            RuntimeListenerFailure {
                listener: "default/web".to_string(),
                message: "bind conflict".to_string(),
            },
            RuntimeListenerFailure {
                listener: "default/admin".to_string(),
                message: "address in use".to_string(),
            },
        ],
    );
    stats.observe_http_tls_asset_reuses(2);
    stats.observe_http_tls_asset_reuses(0);
    stats.observe_http_listener_reload_result(
        "v2",
        &["default/web".to_string(), "default/admin".to_string()],
        &[],
        &[],
    );
    stats.observe_stream_listener_reload_attempt("v1");
    stats.observe_stream_listener_reload_failures(
        "v1",
        &[RuntimeListenerFailure {
            listener: "default/udp".to_string(),
            message: "udp bind conflict".to_string(),
        }],
    );
    stats.observe_stream_listener_reload_result("v3", &["default/udp".to_string()], &[], &[]);

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.http_listener_reload_failures, 1);
    assert_eq!(snapshot.stream_listener_reload_failures, 1);
    assert_eq!(snapshot.http_tls_asset_reuses, 2);
    assert_eq!(snapshot.http_last_reload_attempt_version, "v2");
    assert_eq!(snapshot.http_last_good_reload_version, "v2");
    assert_eq!(snapshot.http_last_reload_failure_version, "");
    assert_eq!(snapshot.http_last_reload_failure_listener, "");
    assert_eq!(snapshot.http_last_reload_failure_message, "");
    assert!(snapshot.http_current_failures.is_empty());
    assert_eq!(
        snapshot
            .http_listener_progress
            .get("default/web")
            .map(|value| value.last_good_version.as_str()),
        Some("v2")
    );
    assert_eq!(
        snapshot
            .http_listener_progress
            .get("default/web")
            .map(|value| value.failures),
        Some(1)
    );
    let http_events = &snapshot
        .http_listener_progress
        .get("default/web")
        .expect("http listener progress")
        .recent_events;
    assert_eq!(http_events.len(), 2);
    assert_eq!(http_events[0].status, "accepted");
    assert_eq!(http_events[0].version, "v2");
    assert!(http_events[0].message.is_empty());
    assert_eq!(http_events[1].status, "rejected");
    assert_eq!(http_events[1].version, "v1");
    assert_eq!(http_events[1].message, "bind conflict");
    assert_eq!(snapshot.stream_last_reload_attempt_version, "v3");
    assert_eq!(snapshot.stream_last_good_reload_version, "v3");
    assert_eq!(snapshot.stream_last_reload_failure_version, "");
    assert_eq!(snapshot.stream_last_reload_failure_listener, "");
    assert_eq!(snapshot.stream_last_reload_failure_message, "");
    assert!(snapshot.stream_current_failures.is_empty());
    assert_eq!(
        snapshot
            .stream_listener_progress
            .get("default/udp")
            .map(|value| value.last_good_version.as_str()),
        Some("v3")
    );
    let stream_events = &snapshot
        .stream_listener_progress
        .get("default/udp")
        .expect("stream listener progress")
        .recent_events;
    assert_eq!(stream_events.len(), 2);
    assert_eq!(stream_events[0].status, "accepted");
    assert_eq!(stream_events[0].version, "v3");
    assert_eq!(stream_events[1].status, "rejected");
    assert_eq!(stream_events[1].version, "v1");
    assert_eq!(stream_events[1].message, "udp bind conflict");
}
