#[test]
fn duplicate_snapshot_log_only_emits_at_debug_level() {
    let info_writer = SharedTestWriter::default();
    let info_subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_writer(info_writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(info_subscriber, || {
        super::super::log_duplicate_snapshot_skipped("v-debug");
    });
    assert!(
        info_writer.contents().trim().is_empty(),
        "duplicate snapshot logs should stay quiet at info"
    );

    let debug_writer = SharedTestWriter::default();
    let debug_subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_writer(debug_writer.clone())
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(debug_subscriber, || {
        super::super::log_duplicate_snapshot_skipped("v-debug");
    });
    let output = debug_writer.contents();
    assert!(output.contains("skipped duplicate snapshot"));
    assert!(output.contains("v-debug"));
}
