use super::*;

#[tokio::test]
async fn circuit_breaker_view_returns_current_and_rejection_counts() {
    let state = test_state(Some("top-secret"));
    replace_circuit_breaker(
        &state,
        HttpCircuitBreakerOptions {
            backend_max_inflight_requests: 1,
        },
    );
    let _permit = with_circuit_breaker(&state, |controller| {
        controller
            .try_acquire_backend("default/echo:8080")
            .expect("first circuit-breaker request should pass")
    });
    assert!(with_circuit_breaker(&state, |controller| {
        controller.try_acquire_backend("default/echo:8080").is_err()
    }));

    let app = super::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/circuit-breakers")
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
    assert_eq!(payload["backendMaxInflightRequests"], 1);
    assert_eq!(payload["backendInflightCurrent"]["default/echo:8080"], 1);
    assert_eq!(payload["rejectedTotal"], 1);
    assert_eq!(payload["rejectedBackendTotal"], 1);
    assert_eq!(payload["rejectedBackendByName"]["default/echo:8080"], 1);
}
