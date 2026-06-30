#[tokio::test]
async fn livez_returns_ok_while_process_is_running() {
    let app = super::build_router(test_state(None));

    let livez = app
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");

    assert_eq!(livez.status(), StatusCode::OK);
}

#[tokio::test]
async fn livez_returns_service_unavailable_when_http_runtime_exits() {
    let state = test_state(None);
    state.runtime.observe_stream_runtime_started();
    state.runtime.observe_http_runtime_exited("thread exited");
    let app = super::build_router(state);

    let livez = app
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");

    assert_eq!(livez.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn livez_returns_service_unavailable_when_stream_runtime_exits() {
    let snapshot = Snapshot {
        id: "v-stream".to_string(),
        listeners: vec![Listener {
            protocol: "LISTENER_PROTOCOL_TCP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    shared.store(Arc::new(snapshot));
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );
    state.runtime.observe_stream_runtime_exited("task exited");
    let app = super::build_router(state);

    let livez = app
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");

    assert_eq!(livez.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn livez_returns_service_unavailable_when_tls_runtime_exits() {
    let state = test_state(None);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_tls_runtime_exited("task exited");
    let app = super::build_router(state);

    let livez = app
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");

    assert_eq!(livez.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn livez_returns_service_unavailable_when_supervisor_shutdown_is_requested() {
    let state = test_state(None);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_stream_runtime_started();
    state.runtime.observe_supervisor_started();
    state
        .runtime
        .observe_supervisor_shutdown_requested("signal: sigterm");
    let app = super::build_router(state);

    let livez = app
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");

    assert_eq!(livez.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn livez_stays_ok_when_xds_snapshot_is_stale() {
    let state = test_state(None);
    set_snapshot_freshness_timeout(&state, Duration::ZERO);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_stream_runtime_started();
    state.xds.observe_stream_connected();
    state.xds.observe_stream_disconnected();
    let app = super::build_router(state);

    let livez = app
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");

    assert_eq!(livez.status(), StatusCode::OK);
}
