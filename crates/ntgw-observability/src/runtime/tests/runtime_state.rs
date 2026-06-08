#[test]
fn records_http_runtime_exit_state() {
    let stats = RuntimeStats::shared();
    stats.observe_http_runtime_started();

    stats.observe_http_runtime_exited("thread exited");

    let snapshot = stats.snapshot();
    assert!(!snapshot.http_runtime_running);
    assert_eq!(snapshot.http_last_exit_message, "thread exited");
    assert!(snapshot.http_last_exit_unix_seconds > 0);
}

#[test]
fn records_stream_runtime_running_state() {
    let stats = RuntimeStats::shared();

    stats.observe_stream_runtime_started();

    let snapshot = stats.snapshot();
    assert!(snapshot.stream_runtime_running);
    assert_eq!(snapshot.stream_last_exit_message, "");
}
