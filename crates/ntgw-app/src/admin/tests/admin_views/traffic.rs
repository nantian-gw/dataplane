use super::*;

#[tokio::test]
async fn traffic_view_returns_observed_flow_stats() {
    let state = test_state(Some("top-secret"));
    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(200),
        latency_ms: 12,
        bytes_received: 64,
        bytes_sent: 256,
        retry_attempts: 1,
        retried_success: true,
        upstream_pool_hits: 1,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 8,
        upstream_connect_latency_ms_max: 8,
        upstream_connect_latency_ms_buckets: {
            let mut buckets = [0; ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
            buckets[ntgw_observability::upstream_connect_latency_ms_bucket_index(8)] = 1;
            buckets
        },
        response_flags: "UC".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    state
        .traffic
        .observe_upstream_tls_handshake_failure(Some(13));
    let app = super::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/traffic")
                .header("Authorization", "Bearer top-secret")
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
    assert_eq!(payload["total_events"], 1);
    assert_eq!(payload["total_retried_events"], 1);
    assert_eq!(payload["total_retry_attempts"], 1);
    assert_eq!(payload["total_retried_success_events"], 1);
    assert_eq!(payload["total_upstream_pool_hits"], 1);
    assert_eq!(payload["total_upstream_pool_misses"], 1);
    assert_eq!(payload["total_upstream_peer_build_failures"], 0);
    assert_eq!(payload["total_upstream_connect_latency_observations"], 1);
    assert_eq!(payload["total_upstream_connect_latency_ms"], 8);
    assert_eq!(payload["max_upstream_connect_latency_ms"], 8);
    assert_eq!(payload["total_upstream_tls_handshake_failures"], 1);
    assert_eq!(
        payload["total_upstream_tls_handshake_failure_latency_observations"],
        1
    );
    assert_eq!(
        payload["total_upstream_tls_handshake_failure_latency_ms"],
        13
    );
    assert_eq!(payload["max_upstream_tls_handshake_failure_latency_ms"], 13);
    assert_eq!(
        payload["upstream_connect_latency_ms_buckets"]
            .as_array()
            .and_then(|buckets| buckets.iter().find(|bucket| bucket["le"] == "10"))
            .and_then(|bucket| bucket["cumulative_count"].as_u64()),
        Some(1)
    );
    assert_eq!(
        payload["upstream_tls_handshake_failure_latency_ms_buckets"]
            .as_array()
            .and_then(|buckets| buckets.iter().find(|bucket| bucket["le"] == "25"))
            .and_then(|bucket| bucket["cumulative_count"].as_u64()),
        Some(1)
    );
    assert_eq!(payload["status_2xx"], 1);
    assert_eq!(payload["response_flags"]["UC"], 1);
    assert!(payload["nodes"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["node_id"] == "listener:web")));
    assert!(payload["nodes"].as_array().is_some_and(|items| items
        .iter()
        .any(|item| item["node_id"] == "route:HTTPRoute:default/web")));
    assert!(payload["nodes"].as_array().is_some_and(|items| items
        .iter()
        .any(|item| item["node_id"] == "backend:default/api:80")));
    assert!(payload["edges"].as_array().is_some_and(|items| items
        .iter()
        .any(|item| item["edge_id"] == "edge:listener:web:route:HTTPRoute:default/web")));
    assert!(payload["edges"].as_array().is_some_and(|items| items
        .iter()
        .any(|item| item["edge_id"] == "edge:route:HTTPRoute:default/web:backend:default/api:80")));
}

#[tokio::test]
async fn traffic_view_nodes_expose_runtime_refs() {
    let mut indexed = fixture_snapshot();
    indexed.rebuild_runtime_indexes();
    let listener_id = indexed.listener_runtime_id("web").expect("listener id");
    let route_id = indexed
        .http_route_runtime_id("default", "web")
        .expect("route id");
    let backend_id = indexed
        .backend_runtime_id("default/api:80")
        .expect("backend id");
    let snapshot = Snapshot::shared();
    *snapshot.write() = indexed;
    let mut config = test_admin_runtime_config();
    config.admin_bearer_token = Some("top-secret".to_string());
    let state = build_state_with_parts(
        config,
        snapshot,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );
    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(200),
        latency_ms: 12,
        bytes_received: 64,
        bytes_sent: 256,
        response_flags: String::new(),
        runtime_ids: TrafficRuntimeIds {
            listener: Some(listener_id.as_u64()),
            route: Some(route_id.as_u64()),
            backend: Some(backend_id.as_u64()),
        },
        ..TrafficObservation::default()
    });
    let app = super::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/traffic")
                .header("Authorization", "Bearer top-secret")
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
    let nodes = payload["nodes"].as_array().expect("nodes array");
    let listener = nodes
        .iter()
        .find(|node| node["node_id"] == "listener:web")
        .expect("listener node");
    let route = nodes
        .iter()
        .find(|node| node["node_id"] == "route:HTTPRoute:default/web")
        .expect("route node");
    let backend = nodes
        .iter()
        .find(|node| node["node_id"] == "backend:default/api:80")
        .expect("backend node");

    assert_eq!(listener["runtimeRef"]["kind"], "Listener");
    assert_eq!(listener["runtimeRef"]["name"], "web");
    assert_eq!(route["runtimeRef"]["kind"], "HTTPRoute");
    assert_eq!(route["runtimeRef"]["namespace"], "default");
    assert_eq!(route["runtimeRef"]["name"], "web");
    assert_eq!(backend["runtimeRef"]["kind"], "Backend");
    assert_eq!(backend["runtimeRef"]["name"], "default/api:80");
}
