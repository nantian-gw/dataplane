use super::*;

#[tokio::test]
async fn listener_status_endpoint_filters_by_runtime_plane_and_current_failure() {
    let state = test_state(Some("top-secret"));
    state
        .runtime
        .observe_http_listener_reload_failure("v-test", "web", "bind conflict");
    state.runtime.observe_tls_listener_reload_success("v-test");
    let app = build_router(state);

    let stream = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?runtimePlane=tls")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(stream.status(), StatusCode::OK);
    let body = axum::body::to_bytes(stream.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "passthrough");
    assert_eq!(payload[0]["runtime_plane"], "tls");

    let failures = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?currentFailure=true")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(failures.status(), StatusCode::OK);
    let body = axum::body::to_bytes(failures.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "web");
    assert_eq!(payload[0]["listener_current_failure"], true);
}
