#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_current_status_filter() {
    let app = super::build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?currentStatus=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
