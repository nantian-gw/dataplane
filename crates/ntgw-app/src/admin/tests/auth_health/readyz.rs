#[tokio::test]
async fn readyz_returns_service_unavailable_before_any_snapshot_or_last_good() {
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        Snapshot::shared(),
        RuntimeStats::shared(),
        ClientStats::shared(),
    );
    let app = super::build_router(state);

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

#[tokio::test]
async fn readyz_returns_service_unavailable_when_http_runtime_exits() {
    let state = test_state(None);
    state.runtime.observe_stream_runtime_started();
    state.runtime.observe_http_runtime_exited("thread exited");
    let app = super::build_router(state);

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

#[tokio::test]
async fn readyz_returns_service_unavailable_when_stream_runtime_exits() {
    let snapshot = Snapshot {
        id: "v-stream".to_string(),
        listeners: vec![Listener {
            protocol: "LISTENER_PROTOCOL_TCP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    shared.store(Arc::new(snapshot));
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );
    state.runtime.observe_stream_runtime_exited("task exited");
    let app = super::build_router(state);

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

#[tokio::test]
async fn readyz_returns_service_unavailable_when_tls_runtime_exits() {
    let state = test_state(None);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_tls_runtime_exited("task exited");
    let app = super::build_router(state);

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

#[tokio::test]
async fn readyz_returns_service_unavailable_when_supervisor_shutdown_is_requested() {
    let state = test_state(None);
    state.runtime.observe_http_runtime_started();
    state.runtime.observe_stream_runtime_started();
    state.runtime.observe_supervisor_started();
    state
        .runtime
        .observe_supervisor_shutdown_requested("signal: sigterm");
    let app = super::build_router(state);

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

#[tokio::test]
async fn readyz_and_metrics_stay_consistent_after_rejected_snapshot_with_last_good() {
    let shared = Snapshot::shared();
    let mut s = fixture_snapshot();
    s.id = "v2".to_string();
    shared.store(Arc::new(s));

    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v1", &["web".to_string()], &[], &[]);
    runtime.observe_tls_listener_reload_result("v1", &["passthrough".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_failure("v2", "web", "bind conflict");
    runtime.observe_tls_listener_reload_failure("v2", "passthrough", "tcp bind conflict");

    let xds = ClientStats::shared();
    xds.observe_snapshot_applied("v1");
    xds.observe_snapshot_nacked("v2", "listener reload failed");

    let state = build_state_with_parts(test_admin_runtime_config(), shared, runtime, xds);
    let app = super::build_router(state);

    let readyz = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("readyz request should succeed");
    assert_eq!(readyz.status(), StatusCode::OK);
    let readyz_body = axum::body::to_bytes(readyz.into_body(), usize::MAX)
        .await
        .expect("readyz body");
    assert_eq!(readyz_body.as_ref(), b"serving-last-good");

    let summary = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("summary request should succeed");
    assert_eq!(summary.status(), StatusCode::OK);
    let summary_body = axum::body::to_bytes(summary.into_body(), usize::MAX)
        .await
        .expect("summary body");
    let summary: serde_json::Value =
        serde_json::from_slice(&summary_body).expect("summary json should parse");
    assert_eq!(summary["ready"], true);
    assert_eq!(summary["readinessState"], "serving-last-good");
    assert_eq!(
        summary["readinessReason"],
        "serving-last-good-after-rejection"
    );
    assert_eq!(summary["currentSnapshotStatus"], "rejected");
    assert_eq!(summary["servingLastGoodSnapshot"], true);
    assert_eq!(summary["lastGoodSnapshotVersion"], "v1");
    assert_eq!(summary["xdsLastSnapshotVersion"], "v1");
    assert_eq!(summary["xdsSnapshotsNacked"], 1);
    assert_eq!(summary["xdsLastNackVersion"], "v2");
    assert_eq!(summary["xdsLastNackMessage"], "listener reload failed");

    let metrics = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("metrics request should succeed");
    assert_eq!(metrics.status(), StatusCode::OK);
    let metrics_body = axum::body::to_bytes(metrics.into_body(), usize::MAX)
        .await
        .expect("metrics body");
    let metrics = String::from_utf8(metrics_body.to_vec()).expect("metrics utf-8");

    assert!(metrics.contains("nantian_gateway_dataplane_xds_snapshots_applied_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_snapshots_nacked_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_last_nack_info 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_current_snapshot_rejected 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_serving_last_good_snapshot 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_http_current_rejected 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_tls_current_rejected 1"));
    assert!(metrics.contains(
        "snapshot_version=\"v2\",xds_last_snapshot_version=\"v1\",last_good_snapshot_version=\"v1\",current_snapshot_status=\"rejected\""
    ));
    assert!(metrics.contains("current_snapshot_rejection_version=\"v2\""));
    assert!(metrics.contains("current_snapshot_rejection_runtime=\"http+tls\""));
    assert!(metrics.contains("runtime_http_current_status=\"rejected\""));
    assert!(metrics.contains("runtime_tls_current_status=\"rejected\""));
}
