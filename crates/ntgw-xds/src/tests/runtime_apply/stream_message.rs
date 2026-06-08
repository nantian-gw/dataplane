#[tokio::test]
async fn wait_for_stream_message_returns_stale_stream_error_when_idle_too_long() {
    let result = wait_for_stream_message(
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<Option<ConfigSnapshot>, tonic::Status>(None)
        },
        Duration::from_millis(10),
    )
    .await;

    assert!(result
        .expect_err("idle stream should be treated as stale")
        .to_string()
        .contains("stale xds stream"));
}

#[tokio::test]
async fn wait_for_stream_message_returns_message_before_stale_timeout() {
    let result = wait_for_stream_message(
        async { Ok::<Option<ConfigSnapshot>, tonic::Status>(Some(ConfigSnapshot::default())) },
        Duration::from_millis(10),
    )
    .await
    .expect("message should arrive before timeout");

    assert!(result.is_some());
}
