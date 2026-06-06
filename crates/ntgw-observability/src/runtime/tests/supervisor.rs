#[test]
fn records_supervisor_shutdown_request_and_exit_state() {
    let stats = RuntimeStats::shared();
    stats.observe_supervisor_started();

    stats.observe_supervisor_shutdown_requested("signal: sigterm");
    stats.observe_supervisor_exited("graceful shutdown complete");

    let snapshot = stats.snapshot();
    assert!(!snapshot.supervisor_running);
    assert!(snapshot.supervisor_shutdown_requested);
    assert_eq!(snapshot.supervisor_last_shutdown_reason, "signal: sigterm");
    assert_eq!(
        snapshot.supervisor_last_exit_message,
        "graceful shutdown complete"
    );
    assert!(snapshot.supervisor_last_exit_unix_seconds > 0);
}
