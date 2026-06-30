use super::*;

#[tokio::test]
async fn listener_status_endpoint_filters_by_recovered_from_failure() {
    let state = test_state(Some("top-secret"));
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "recovered".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "failed".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    state.snapshot.store(Arc::new(snapshot));
    state
        .runtime
        .observe_http_listener_reload_failure("v1", "recovered", "bind conflict");
    state
        .runtime
        .observe_http_listener_reload_result("v2", &["recovered".to_string()], &[], &[]);
    state
        .runtime
        .observe_http_listener_reload_failure("v2", "failed", "address in use");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?recoveredFromFailure=true")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "recovered");
    assert_eq!(payload[0]["listener_recovered_from_failure"], true);
    assert_eq!(payload[0]["listener_recovery_version"], "v2");
}
