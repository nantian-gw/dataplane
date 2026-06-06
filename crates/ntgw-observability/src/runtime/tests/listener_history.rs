#[test]
fn runtime_stats_snapshot_tracks_retained_listener_versions() {
    let stats = RuntimeStats::shared();
    stats.observe_http_listener_reload_result("v1", &["default/web".to_string()], &[], &[]);
    stats.observe_http_listener_reload_result(
        "v2",
        &["default/api".to_string()],
        &["default/web".to_string()],
        &[],
    );

    let snapshot = stats.snapshot();
    let web = snapshot
        .http_listener_progress
        .get("default/web")
        .expect("web listener progress");
    assert_eq!(web.attempts, 1);
    assert_eq!(web.last_good_version, "v2");
    assert_eq!(web.recent_events.len(), 2);
    assert_eq!(web.recent_events[0].status, "retained");
    assert_eq!(web.recent_events[0].version, "v2");
    assert_eq!(web.recent_events[1].status, "accepted");
    assert_eq!(web.recent_events[1].version, "v1");
}

#[test]
fn runtime_stats_snapshot_limits_recent_listener_history() {
    let stats = RuntimeStats::shared();

    for version in 0..(LISTENER_EVENT_HISTORY_LIMIT + 2) {
        let version = format!("v{version}");
        stats.observe_http_listener_reload_result(
            version.as_str(),
            &["default/web".to_string()],
            &[],
            &[],
        );
    }

    let snapshot = stats.snapshot();
    let events = &snapshot
        .http_listener_progress
        .get("default/web")
        .expect("http listener progress")
        .recent_events;
    assert_eq!(events.len(), LISTENER_EVENT_HISTORY_LIMIT);
    assert_eq!(
        events[0].version,
        format!("v{}", LISTENER_EVENT_HISTORY_LIMIT + 1)
    );
    assert_eq!(events[LISTENER_EVENT_HISTORY_LIMIT - 1].version, "v2");
}
