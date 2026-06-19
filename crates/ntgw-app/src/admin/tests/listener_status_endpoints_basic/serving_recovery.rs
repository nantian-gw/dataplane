#[tokio::test]
async fn listener_status_endpoint_filters_by_serving_and_recovery_state() {
    let state = test_state(Some("top-secret"));
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "retained".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "stale".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "recovered".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "failed".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    state.snapshot.store(Arc::new(snapshot));
    state
        .runtime
        .observe_http_listener_reload_result("v1", &["retained".to_string()], &[], &[]);
    state.runtime.observe_http_listener_reload_result(
        "v2",
        &["recovered".to_string()],
        &["retained".to_string()],
        &[],
    );
    state
        .runtime
        .observe_http_listener_reload_result("v1", &["stale".to_string()], &[], &[]);
    state
        .runtime
        .observe_http_listener_reload_failure("v1", "recovered", "bind conflict");
    state
        .runtime
        .observe_http_listener_reload_failure("v2", "failed", "address in use");
    let app = super::build_router(state);

    let retained = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingState=current-retained")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(retained.status(), StatusCode::OK);
    let body = axum::body::to_bytes(retained.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "retained");
    assert_eq!(payload[0]["listener_serving_state"], "current-retained");
    assert_eq!(payload[0]["listener_recovery_state"], "steady");

    let stale = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingState=last-good-stale")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(stale.status(), StatusCode::OK);
    let body = axum::body::to_bytes(stale.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "stale");
    assert_eq!(payload[0]["listener_serving_state"], "last-good-stale");
    assert_eq!(payload[0]["listener_recovery_state"], "drifted-last-good");

    let recovered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?recoveryState=recovered")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(recovered.status(), StatusCode::OK);
    let body = axum::body::to_bytes(recovered.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "recovered");
    assert_eq!(payload[0]["listener_serving_state"], "current-accepted");
    assert_eq!(payload[0]["listener_recovery_state"], "recovered");

    let failed = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?recoveryState=unrecovered-current")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(failed.status(), StatusCode::OK);
    let body = axum::body::to_bytes(failed.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "failed");
    assert_eq!(payload[0]["listener_serving_state"], "none");
    assert_eq!(payload[0]["listener_recovery_state"], "unrecovered-current");
}
