use super::*;

#[tokio::test]
async fn listener_status_endpoint_filters_by_attempt_progress_and_failure_age() {
    let state = test_state(Some("top-secret"));
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "pending-historical".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "rejected-current".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "stale-historical".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "accepted-clean".to_string().into(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    state.snapshot.store(Arc::new(snapshot));
    state
        .runtime
        .observe_http_listener_reload_failure("v1", "pending-historical", "bind conflict");
    state
        .runtime
        .observe_http_listener_reload_failure("v2", "rejected-current", "port busy");
    state.runtime.observe_http_listener_reload_result(
        "v1",
        &["stale-historical".to_string()],
        &[],
        &[],
    );
    state
        .runtime
        .observe_http_listener_reload_failure("v1", "stale-historical", "address in use");
    state.runtime.observe_http_listener_reload_result(
        "v2",
        &["accepted-clean".to_string()],
        &[],
        &[],
    );
    let app = build_router(state);

    let awaiting = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?attemptProgress=awaiting-current")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(awaiting.status(), StatusCode::OK);
    let body = axum::body::to_bytes(awaiting.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "pending-historical");
    assert_eq!(payload[0]["listener_awaiting_current_attempt"], true);
    assert_eq!(payload[0]["listener_current_attempt_blocked"], false);
    assert_eq!(payload[0]["listener_unrecovered_historical_failure"], true);

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?attemptProgress=blocked-current")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(blocked.status(), StatusCode::OK);
    let body = axum::body::to_bytes(blocked.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "rejected-current");
    assert_eq!(payload[0]["listener_awaiting_current_attempt"], false);
    assert_eq!(payload[0]["listener_current_attempt_blocked"], true);
    assert_eq!(
        payload[0]["listener_unrecovered_current_snapshot_failure"],
        true
    );

    let current = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?unrecoveredFailureAge=current")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(current.status(), StatusCode::OK);
    let body = axum::body::to_bytes(current.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "rejected-current");

    let historical = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?unrecoveredFailureAge=historical")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(historical.status(), StatusCode::OK);
    let body = axum::body::to_bytes(historical.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(2));
    assert_eq!(payload[0]["name"], "pending-historical");
    assert_eq!(payload[1]["name"], "stale-historical");

    let none = app
        .oneshot(
            Request::builder()
                .uri("/v1/listener-statuses?attemptProgress=other&unrecoveredFailureAge=none")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(none.status(), StatusCode::OK);
    let body = axum::body::to_bytes(none.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload.as_array().map(Vec::len), Some(1));
    assert_eq!(payload[0]["name"], "accepted-clean");
    assert_eq!(payload[0]["listener_awaiting_current_attempt"], false);
    assert_eq!(payload[0]["listener_current_attempt_blocked"], false);
    assert_eq!(
        payload[0]["listener_unrecovered_current_snapshot_failure"],
        false
    );
    assert_eq!(payload[0]["listener_unrecovered_historical_failure"], false);
}
