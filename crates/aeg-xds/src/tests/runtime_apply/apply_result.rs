#[tokio::test]
async fn wait_for_runtime_apply_result_ignores_unneeded_stream_runtime() {
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_success("v1");

    let result = wait_for_runtime_apply_result(
        runtime,
        "v1",
        RuntimeApplyRequirements {
            http: true,
            tls: false,
            stream: false,
        },
        &TransportOptions::default(),
    )
    .await;

    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn wait_for_runtime_apply_result_surfaces_stream_failure() {
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_success("v2");
    runtime.observe_stream_listener_reload_failure("v2", "default/udp", "udp bind conflict");

    let result = wait_for_runtime_apply_result(
        runtime,
        "v2",
        RuntimeApplyRequirements {
            http: false,
            tls: false,
            stream: true,
        },
        &TransportOptions {
            apply_timeout: Duration::from_secs(3),
            apply_poll_interval: Duration::from_millis(25),
            ..TransportOptions::default()
        },
    )
    .await;

    assert_eq!(
        result,
        Err("stream runtime apply failed: udp bind conflict".to_string())
    );
}

#[tokio::test]
async fn wait_for_runtime_apply_result_surfaces_tls_failure() {
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_success("v2");
    runtime.observe_tls_listener_reload_failure("v2", "default/https", "tls bind conflict");

    let result = wait_for_runtime_apply_result(
        runtime,
        "v2",
        RuntimeApplyRequirements {
            http: false,
            tls: true,
            stream: false,
        },
        &TransportOptions {
            apply_timeout: Duration::from_secs(3),
            apply_poll_interval: Duration::from_millis(25),
            ..TransportOptions::default()
        },
    )
    .await;

    assert_eq!(
        result,
        Err("TLS runtime apply failed: tls bind conflict".to_string())
    );
}

#[tokio::test]
async fn wait_for_runtime_apply_result_uses_configured_timeout() {
    let runtime = RuntimeStats::shared();

    let result = wait_for_runtime_apply_result(
        runtime,
        "v-timeout",
        RuntimeApplyRequirements {
            http: true,
            tls: false,
            stream: false,
        },
        &TransportOptions {
            apply_timeout: Duration::from_millis(20),
            apply_poll_interval: Duration::from_millis(5),
            ..TransportOptions::default()
        },
    )
    .await;

    assert_eq!(
        result,
        Err("timed out waiting for HTTP runtime apply result for snapshot v-timeout".to_string())
    );
}
