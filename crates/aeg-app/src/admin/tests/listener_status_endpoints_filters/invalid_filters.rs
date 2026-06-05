use super::super::*;

#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_serving_snapshot_filter() {
    let app = build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingSnapshot=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_serving_state_filter() {
    let app = build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?servingState=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_attempt_progress_filter() {
    let app = build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?attemptProgress=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_recovery_state_filter() {
    let app = build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?recoveryState=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_unrecovered_failure_age_filter() {
    let app = build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?unrecoveredFailureAge=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn listener_status_endpoint_rejects_invalid_runtime_plane_filter() {
    let app = build_router(test_state(Some("top-secret")));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?runtimePlane=broken")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
