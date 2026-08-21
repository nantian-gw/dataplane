use super::*;

#[tokio::test]
async fn rate_limit_view_returns_available_tokens_and_rejection_counts() {
    let state = test_state(Some("top-secret"));
    replace_rate_limit(
        &state,
        HttpRateLimitOptions {
            global_requests_per_second: 1,
            global_burst: 2,
            listener_requests_per_second: 1,
            listener_burst: 2,
            route_requests_per_second: 1,
            route_burst: 1,
            backend_requests_per_second: 0,
            backend_burst: 0,
        },
    );
    assert!(with_rate_limit(&state, |controller| {
        controller
            .try_acquire("web", "Http/default/shared")
            .expect("first rate-limited request should pass")
    }));
    with_rate_limit(&state, |controller| controller.observe_allow());
    assert!(with_rate_limit(&state, |controller| {
        controller
            .try_acquire("web", "Http/default/shared")
            .is_err()
    }));

    let app = super::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/rate-limits")
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
    assert_eq!(payload["allowedTotal"], 1);
    assert_eq!(payload["rejectedTotal"], 1);
    assert_eq!(payload["rejectedRouteTotal"], 1);
    assert_eq!(payload["global"]["availableTokens"], 1);
    assert_eq!(payload["listener"]["availableTokensByName"]["web"], 1);
    assert_eq!(
        payload["route"]["availableTokensByName"]["Http/default/shared"],
        0
    );
    assert_eq!(payload["rejectedRouteByName"]["Http/default/shared"], 1);
}
