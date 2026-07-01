#[tokio::test]
async fn listener_status_endpoint_filters_by_serving_snapshot_and_version() {
    let state = test_state(Some("top-secret"));
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "retained".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "accepted".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "stale".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
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
        &["accepted".to_string()],
        &["retained".to_string()],
        &[],
    );
    state
        .runtime
        .observe_http_listener_reload_result("v1", &["stale".to_string()], &[], &[]);
    let app = super::build_router(state);

    let current = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingSnapshot=current")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(current.status(), StatusCode::OK);
    let body = axum::body::to_bytes(current.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(2));
    assert_eq!(payload[0]["name"], "retained");
    assert_eq!(payload[0]["listener_current_status"], "retained");
    assert_eq!(payload[0]["listener_serving_version"], "v2");
    assert_eq!(payload[1]["name"], "accepted");
    assert_eq!(payload[1]["listener_current_status"], "accepted");
    assert_eq!(payload[1]["listener_serving_version"], "v2");

    let last_good = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingSnapshot=last-good")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(last_good.status(), StatusCode::OK);
    let body = axum::body::to_bytes(last_good.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "stale");
    assert_eq!(payload[0]["listener_current_status"], "stale");
    assert_eq!(payload[0]["listener_serving_version"], "v1");

    let version = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingVersion=v1")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(version.status(), StatusCode::OK);
    let body = axum::body::to_bytes(version.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "stale");
}
