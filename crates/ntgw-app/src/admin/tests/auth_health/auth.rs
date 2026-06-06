#[tokio::test]
async fn admin_auth_protects_management_routes() {
    let app = super::build_router(test_state(Some("top-secret")));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .header("Authorization", "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_auth_allows_probe_endpoints_without_token() {
    let app = super::build_router(test_state(Some("top-secret")));

    let livez = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(livez.status(), StatusCode::OK);

    let readyz = app
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(readyz.status(), StatusCode::SERVICE_UNAVAILABLE);
}
