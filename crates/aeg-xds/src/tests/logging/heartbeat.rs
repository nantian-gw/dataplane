#[test]
fn cancelled_heartbeat_error_stays_quiet_at_info() {
    let writer = SharedTestWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_writer(writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        super::super::log_heartbeat_report_failure(
            "dp-1",
            &tonic::Status::cancelled("operation was canceled"),
        );
    });

    assert!(
        writer.contents().trim().is_empty(),
        "expected heartbeat interruption should stay quiet at info"
    );
}

#[test]
fn cancelled_heartbeat_error_logs_at_debug() {
    let writer = SharedTestWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_writer(writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        super::super::log_heartbeat_report_failure(
            "dp-1",
            &tonic::Status::cancelled("operation was canceled"),
        );
    });

    let output = writer.contents();
    assert!(
        !output.trim().is_empty(),
        "expected heartbeat interruption details to be visible at debug"
    );
    assert!(output.contains("dataplane heartbeat interrupted during xds reconnect"));
    assert!(output.contains("dp-1"));
}
