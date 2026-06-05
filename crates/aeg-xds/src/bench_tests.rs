use crate::{SNAPSHOT_APPLIED_MESSAGE, SNAPSHOT_REJECTED_MESSAGE_PREFIX};

#[tokio::test]
async fn reload_bench_marks_apply_success_as_ready() {
    let fixture = crate::bench::ReloadBench::new(crate::bench::ApplyBenchConfig {
        include_http: true,
        include_stream: true,
    });

    let outcome = fixture
        .apply_success("v1")
        .await
        .expect("runtime apply should succeed");

    assert!(outcome.ready);
    assert_eq!(outcome.version, "v1");
    assert_eq!(outcome.message, SNAPSHOT_APPLIED_MESSAGE);
}

#[tokio::test]
async fn reload_bench_preserves_last_good_readiness_on_partial_failure() {
    let fixture = crate::bench::ReloadBench::new(crate::bench::ApplyBenchConfig {
        include_http: true,
        include_stream: true,
    });

    let outcome = fixture.apply_failure_with_last_good("v1", "v2").await;

    assert!(outcome.ready);
    assert_eq!(outcome.last_good_version, "v1");
    assert!(outcome
        .apply_error
        .contains("HTTP runtime apply failed: bind conflict"));
    assert!(outcome
        .message
        .starts_with(SNAPSHOT_REJECTED_MESSAGE_PREFIX));
    assert!(outcome.message.contains("bind conflict"));
}
