use super::*;

#[tokio::test]
async fn admin_auth_reloads_bearer_token_file() {
    let path = temp_token_path("admin-token");
    fs::write(&path, "old-secret\n").expect("write token");
    let app = super::build_router(test_state_with_file(&path));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .header("Authorization", "Bearer old-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    fs::write(&path, "new-secret\n").expect("rewrite token");

    let stale = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .header("Authorization", "Bearer old-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(stale.status(), StatusCode::UNAUTHORIZED);

    let fresh = app
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .header("Authorization", "Bearer new-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(fresh.status(), StatusCode::OK);

    let _ = fs::remove_file(path);
}
