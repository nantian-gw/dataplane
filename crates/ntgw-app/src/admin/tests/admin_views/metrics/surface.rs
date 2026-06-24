#[tokio::test]
async fn metrics_view_returns_prometheus_payload() {
    let app = super::build_router(test_state(Some("top-secret")));

    let without_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(without_auth.status(), StatusCode::OK);

    let authorized = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(authorized.status(), StatusCode::OK);
    assert_eq!(
        authorized.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/plain; version=0.0.4; charset=utf-8"
    );

    let body = axum::body::to_bytes(authorized.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload = String::from_utf8(body.to_vec()).expect("utf-8");
    assert!(payload.contains("nantian_gateway_dataplane_ready 1"));
    assert!(payload.contains("process_cpu_seconds_total"));
    assert!(payload.contains("process_resident_memory_bytes"));
    assert!(payload.contains("process_open_fds"));
    assert!(payload.contains("process_threads"));
    assert!(payload.contains("nantian_gateway_dataplane_listener_count 2"));
    assert!(payload.contains("nantian_gateway_dataplane_http_route_count 1"));
    assert!(payload.contains("nantian_gateway_dataplane_grpc_route_count 1"));
    assert!(payload.contains("nantian_gateway_dataplane_stream_route_count 1"));
    assert!(payload.contains("nantian_gateway_dataplane_backend_count 3"));
    assert!(payload.contains("nantian_gateway_dataplane_secret_count 1"));
    assert!(payload.contains("nantian_gateway_dataplane_node_info{node_id=\"dp-1\""));
}

#[tokio::test]
async fn metrics_view_exposes_admin_request_observability() {
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
        .expect("livez request should succeed");
    assert_eq!(livez.status(), StatusCode::OK);

    let authorized_summary = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("authorized summary request should succeed");
    assert_eq!(authorized_summary.status(), StatusCode::OK);

    let unauthorized_summary = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("unauthorized summary request should succeed");
    assert_eq!(unauthorized_summary.status(), StatusCode::UNAUTHORIZED);

    let payload = authorized_text(&app, "/metrics").await;

    assert!(payload.contains(
        "nantian_gateway_dataplane_admin_requests_total{method=\"GET\",route=\"livez\",status_class=\"2xx\"} 1"
    ));
    assert!(payload.contains(
        "nantian_gateway_dataplane_admin_requests_total{method=\"GET\",route=\"summary\",status_class=\"2xx\"} 1"
    ));
    assert!(payload.contains(
        "nantian_gateway_dataplane_admin_requests_total{method=\"GET\",route=\"summary\",status_class=\"4xx\"} 1"
    ));
    assert!(payload.contains(
        "nantian_gateway_dataplane_admin_request_duration_seconds_bucket{method=\"GET\",route=\"summary\",status_class=\"2xx\",le=\"+Inf\"} 1"
    ));
    assert!(payload.contains(
        "nantian_gateway_dataplane_admin_request_duration_seconds_count{method=\"GET\",route=\"summary\",status_class=\"2xx\"} 1"
    ));
}
