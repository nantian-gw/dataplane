#[test]
fn expected_stream_reconnect_error_stays_quiet_at_info() {
    let writer = SharedTestWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_writer(writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        super::super::log_stream_failure_retry(
            &anyhow::anyhow!(
                "status: Unknown, message: \"h2 protocol error: error reading a body from connection\""
            ),
            Duration::from_millis(1500),
        );
    });

    assert!(
        writer.contents().trim().is_empty(),
        "expected reconnect noise should stay quiet at info"
    );
}

#[test]
fn expected_stream_reconnect_error_logs_at_debug() {
    let writer = SharedTestWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_writer(writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        super::super::log_stream_failure_retry(
            &anyhow::anyhow!(
                "status: Unknown, message: \"h2 protocol error: error reading a body from connection\""
            ),
            Duration::from_millis(1500),
        );
    });

    let output = writer.contents();
    assert!(
        !output.trim().is_empty(),
        "expected reconnect details to be visible at debug"
    );
    assert!(output.contains("xds stream closed, retrying"));
    assert!(output.contains("h2 protocol error: error reading a body from connection"));
}

#[test]
fn unexpected_stream_error_logs_at_warn() {
    let writer = SharedTestWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_writer(writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        super::super::log_stream_failure_retry(
            &anyhow::anyhow!("stale xds stream: no control-plane message received for 30s"),
            Duration::from_millis(1500),
        );
    });

    let output = writer.contents();
    assert!(output.contains(" WARN "));
    assert!(output.contains("xds stream failed, retrying"));
    assert!(output.contains("stale xds stream"));
}
