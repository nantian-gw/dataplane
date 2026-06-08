#[tokio::test]
async fn wait_for_runtime_apply_result_wakes_on_async_success_event() {
    let runtime = RuntimeStats::shared();
    let waiter = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            wait_for_runtime_apply_result(
                runtime,
                "v-async-success",
                RuntimeApplyRequirements {
                    http: true,
                    tls: true,
                    stream: true,
                },
                &TransportOptions {
                    apply_timeout: Duration::from_millis(150),
                    apply_poll_interval: Duration::from_secs(60),
                    ..TransportOptions::default()
                },
            )
            .await
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;
    runtime.observe_http_listener_reload_success("v-async-success");
    runtime.observe_tls_listener_reload_success("v-async-success");
    runtime.observe_stream_listener_reload_success("v-async-success");

    let result = tokio::time::timeout(Duration::from_millis(100), waiter)
        .await
        .expect("waiter should not block on poll interval")
        .expect("wait task should join");
    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn wait_for_runtime_apply_result_wakes_on_async_rejection_event() {
    let runtime = RuntimeStats::shared();
    let waiter = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            wait_for_runtime_apply_result(
                runtime,
                "v-async-failure",
                RuntimeApplyRequirements {
                    http: false,
                    tls: false,
                    stream: true,
                },
                &TransportOptions {
                    apply_timeout: Duration::from_millis(150),
                    apply_poll_interval: Duration::from_secs(60),
                    ..TransportOptions::default()
                },
            )
            .await
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;
    runtime.observe_stream_listener_reload_failure(
        "v-async-failure",
        "default/tcp",
        "bind conflict",
    );

    let result = tokio::time::timeout(Duration::from_millis(100), waiter)
        .await
        .expect("waiter should not block on poll interval")
        .expect("wait task should join");
    assert_eq!(
        result,
        Err("stream runtime apply failed: bind conflict".to_string())
    );
}
