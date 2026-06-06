use super::*;

#[tokio::test]
async fn listener_status_endpoint_filters_by_attention_reason() {
    let state = test_state(Some("top-secret"));
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "failed".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "stale".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    *state.snapshot.write() = snapshot;
    state
        .runtime
        .observe_http_listener_reload_failure("v2", "failed", "address in use");
    state
        .runtime
        .observe_http_listener_reload_result("v1", &["stale".to_string()], &[], &[]);
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?attentionReason=stale")
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
    assert_eq!(payload[0]["name"], "stale");
    assert_eq!(
        payload[0]["listener_attention_reasons"],
        serde_json::json!(["stale"])
    );
}
