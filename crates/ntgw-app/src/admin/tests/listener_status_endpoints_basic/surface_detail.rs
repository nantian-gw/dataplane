#[tokio::test]
async fn listener_status_endpoints_expose_runtime_state() {
    let state = test_state(Some("top-secret"));
    state
        .runtime
        .observe_http_listener_reload_failure("v-test", "web", "bind conflict");
    state.runtime.observe_tls_listener_reload_success("v-test");
    let app = super::build_router(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?protocol=http&currentStatus=rejected")
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
    assert_eq!(payload[0]["name"], "web");
    assert_eq!(payload[0]["runtime_plane"], "http");
    assert_eq!(payload[0]["runtime_current_status"], "rejected");
    assert_eq!(payload[0]["listener_current_status"], "rejected");
    assert_eq!(payload[0]["listener_current_accepted"], false);
    assert_eq!(payload[0]["listener_current_retained"], false);
    assert_eq!(payload[0]["listener_current_rejected"], true);
    assert_eq!(payload[0]["listener_current_stale"], false);
    assert_eq!(payload[0]["listener_current_failure"], true);
    assert_eq!(payload[0]["listener_current_failure_version"], "v-test");
    assert_eq!(
        payload[0]["listener_current_failure_message"],
        "bind conflict"
    );
    assert_eq!(payload[0]["listener_attempts"], 1);
    assert_eq!(payload[0]["listener_failures"], 1);
    assert_eq!(payload[0]["listener_last_attempt_version"], "v-test");
    assert_eq!(payload[0]["listener_last_good_version"], "");
    assert_eq!(payload[0]["listener_serving_version"], "");
    assert_eq!(payload[0]["listener_serving_current_snapshot"], false);
    assert_eq!(payload[0]["listener_serving_last_good_snapshot"], false);
    assert_eq!(payload[0]["listener_serving_state"], "none");
    assert_eq!(payload[0]["listener_recovery_state"], "unrecovered-current");
    assert_eq!(payload[0]["listener_last_failure_version"], "v-test");
    assert_eq!(
        payload[0]["listener_recent_events"][0]["status"],
        "rejected"
    );
    assert_eq!(payload[0]["listener_recent_events"][0]["version"], "v-test");
    assert_eq!(
        payload[0]["listener_recent_events"][0]["message"],
        "bind conflict"
    );

    let detail = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses/passthrough")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(detail.status(), StatusCode::OK);
    let body = axum::body::to_bytes(detail.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload["name"], "passthrough");
    assert_eq!(payload["runtime_plane"], "tls");
    assert_eq!(payload["runtime_current_status"], "accepted");
    assert_eq!(payload["listener_current_status"], "pending");
    assert_eq!(payload["listener_current_accepted"], false);
    assert_eq!(payload["listener_current_retained"], false);
    assert_eq!(payload["listener_current_rejected"], false);
    assert_eq!(payload["listener_current_stale"], false);
    assert_eq!(payload["listener_current_failure"], false);
    assert_eq!(payload["listener_current_failure_message"], "");
    assert_eq!(payload["listener_attempts"], 0);
    assert_eq!(payload["listener_failures"], 0);
    assert_eq!(payload["listener_serving_version"], "");
    assert_eq!(payload["listener_serving_current_snapshot"], false);
    assert_eq!(payload["listener_serving_last_good_snapshot"], false);
    assert_eq!(payload["listener_serving_state"], "none");
    assert_eq!(payload["listener_recovery_state"], "awaiting-current");
    assert_eq!(payload["listener_recent_events"], serde_json::json!([]));
}

#[tokio::test]
async fn listener_status_endpoints_expose_runtime_ids() {
    let state = test_state(Some("top-secret"));
    let expected_web_runtime_id = {
        let mut s = (**state.snapshot.load()).clone();
        s.rebuild_runtime_indexes();
        let id = s
            .listener_runtime_id("web")
            .expect("listener runtime id")
            .to_string();
        state.snapshot.store(Arc::new(s));
        id
    };
    let app = super::build_router(state);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?name=web")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(list.status(), StatusCode::OK);
    let body = axum::body::to_bytes(list.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload[0]["runtimeId"].as_str(),
        Some(expected_web_runtime_id.as_str())
    );

    let detail = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses/web")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(detail.status(), StatusCode::OK);
    let body = axum::body::to_bytes(detail.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        payload["runtimeId"].as_str(),
        Some(expected_web_runtime_id.as_str())
    );
}
