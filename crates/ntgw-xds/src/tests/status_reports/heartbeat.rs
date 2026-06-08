#[test]
fn heartbeat_status_report_omits_version_but_preserves_readiness() {
    let snapshot = Snapshot::shared();
    snapshot.write().id = "v1".to_string();
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_success("v1");

    let heartbeat = build_status_report("dp-1", &snapshot, &runtime, false);
    let ack = build_status_report("dp-1", &snapshot, &runtime, true);

    assert_eq!(heartbeat.node_id, "dp-1");
    assert_eq!(heartbeat.version, "");
    assert!(heartbeat.ready);
    assert_eq!(heartbeat.message, SNAPSHOT_APPLIED_MESSAGE);
    assert!(heartbeat.observed_at.is_some());

    assert_eq!(ack.version, "v1");
    assert!(ack.ready);
    assert_eq!(ack.message, SNAPSHOT_APPLIED_MESSAGE);
    assert!(ack.observed_at.is_some());
}

#[test]
fn status_report_observed_at_is_close_to_now() {
    let snapshot = Snapshot::shared();
    snapshot.write().id = "v1".to_string();
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_success("v1");

    let before = SystemTime::now();
    let report = build_status_report("dp-1", &snapshot, &runtime, true);
    let after = SystemTime::now();

    let observed_at: SystemTime = report
        .observed_at
        .expect("status report should include observed_at")
        .try_into()
        .expect("observed_at should convert to system time");
    assert!(observed_at >= before);
    assert!(observed_at <= after);
}

#[test]
fn warming_status_report_stays_unready_without_snapshot_version() {
    let snapshot = Snapshot::shared();
    let runtime = RuntimeStats::shared();

    let report = build_status_report("dp-1", &snapshot, &runtime, false);

    assert_eq!(report.node_id, "dp-1");
    assert_eq!(report.version, "");
    assert!(!report.ready);
    assert_eq!(report.message, WAITING_FOR_SNAPSHOT_MESSAGE);
}

#[test]
fn heartbeat_status_report_surfaces_runtime_rejection_message() {
    let snapshot = Snapshot::shared();
    snapshot.write().id = "v2".to_string();
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_success("v1");
    runtime.observe_http_listener_reload_failure("v2", "default/web", "listener reload failed");

    let report = build_status_report("dp-1", &snapshot, &runtime, false);

    assert!(report.ready);
    assert_eq!(
        report.message,
        format!("{SNAPSHOT_REJECTED_MESSAGE_PREFIX}listener reload failed")
    );
}

#[test]
fn heartbeat_status_report_surfaces_stream_rejection_message() {
    let snapshot = Snapshot::shared();
    snapshot.write().id = "v3".to_string();
    let runtime = RuntimeStats::shared();
    runtime.observe_stream_listener_reload_success("v1");
    runtime.observe_stream_listener_reload_failure("v3", "default/tcp", "tcp bind conflict");

    let report = build_status_report("dp-1", &snapshot, &runtime, false);

    assert!(report.ready);
    assert_eq!(
        report.message,
        format!("{SNAPSHOT_REJECTED_MESSAGE_PREFIX}tcp bind conflict")
    );
}

#[test]
fn heartbeat_status_report_surfaces_tls_rejection_message() {
    let snapshot = Snapshot::shared();
    snapshot.write().id = "v4".to_string();
    let runtime = RuntimeStats::shared();
    runtime.observe_tls_listener_reload_success("v1");
    runtime.observe_tls_listener_reload_failure("v4", "default/https", "tls bind conflict");

    let report = build_status_report("dp-1", &snapshot, &runtime, false);

    assert!(report.ready);
    assert_eq!(
        report.message,
        format!("{SNAPSHOT_REJECTED_MESSAGE_PREFIX}tls bind conflict")
    );
}
