#[tokio::test]
async fn metrics_view_matches_management_endpoint_state() {
    let state = test_state(Some("top-secret"));
    set_http3_configured(&state, true);
    set_session_persistence_uses_ephemeral_secret(&state, true);

    let mut snapshot = fixture_snapshot();
    snapshot.http_routes[0].rules[0].session_persistence = Some(SessionPersistence {
        session_name: "sticky".to_string(),
        session_type: "Cookie".to_string(),
        cookie: Some(CookieConfig {
            lifetime_type: "Permanent".to_string(),
        }),
        ..SessionPersistence::default()
    });
    snapshot.backend_policies = std::iter::once((
        "default/api:80".to_string(),
        BackendPolicy {
            session_persistence: Some(SessionPersistence {
                session_name: "backend-sticky".to_string(),
                session_type: "Header".to_string(),
                ..SessionPersistence::default()
            }),
            ..BackendPolicy::default()
        },
    ))
    .collect();
    state.snapshot.store(Arc::new(snapshot));

    state
        .xds
        .observe_connect_failure_with_error("connection refused");
    state.xds.observe_stream_failure_with_error("rpc closed");
    state.xds.observe_snapshot_applied("v-test");

    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(503),
        latency_ms: 25,
        bytes_received: 64,
        bytes_sent: 256,
        retry_attempts: 2,
        retried_success: false,
        upstream_pool_hits: 3,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 11,
        upstream_connect_latency_ms_max: 11,
        upstream_connect_latency_ms_buckets: {
            let mut buckets = [0; ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
            buckets[ntgw_observability::upstream_connect_latency_ms_bucket_index(11)] = 1;
            buckets
        },
        response_flags: "UT".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });

    let http_listener = HttpAdmissionController::new(
        HttpAdmissionOptions {
            listener_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_listener_permit = http_listener
        .try_acquire("web", "Http/default/listener-current")
        .expect("http listener permit");
    assert!(http_listener
        .try_acquire("web", "Http/default/listener-rejected")
        .is_err());

    let http_route = HttpAdmissionController::new(
        HttpAdmissionOptions {
            route_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_route_permit = http_route
        .try_acquire("web", "Http/default/shared")
        .expect("http route permit");
    assert!(http_route
        .try_acquire("admin", "Http/default/shared")
        .is_err());

    let tcp_listener = TcpAdmissionController::new(
        TcpAdmissionOptions {
            listener_connection_limit: 1,
            ..TcpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _tcp_listener_permit = tcp_listener
        .try_acquire("passthrough")
        .expect("tcp listener permit");
    assert!(tcp_listener.try_acquire("passthrough").is_err());

    replace_circuit_breaker(
        &state,
        HttpCircuitBreakerOptions {
            backend_max_inflight_requests: 1,
        },
    );
    let _circuit_breaker_permit = with_circuit_breaker(&state, |controller| {
        controller
            .try_acquire_backend("default/api:80")
            .expect("first circuit-breaker request should pass")
    });
    assert!(with_circuit_breaker(&state, |controller| {
        controller.try_acquire_backend("default/api:80").is_err()
    }));

    replace_rate_limit(
        &state,
        HttpRateLimitOptions {
            global_requests_per_second: 1,
            global_burst: 2,
            listener_requests_per_second: 1,
            listener_burst: 2,
            route_requests_per_second: 1,
            route_burst: 1,
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

    let metrics = authorized_text(&app, "/metrics").await;
    let snapshot: serde_json::Value = authorized_json(&app, "/v1/snapshot").await;
    let traffic: serde_json::Value = authorized_json(&app, "/v1/traffic").await;
    let overload: serde_json::Value = authorized_json(&app, "/v1/overload").await;
    let circuit_breakers: serde_json::Value = authorized_json(&app, "/v1/circuit-breakers").await;
    let rate_limits: serde_json::Value = authorized_json(&app, "/v1/rate-limits").await;

    assert_inventory_metrics_match_state(&metrics, &snapshot);
    assert_traffic_metrics_match_state(&metrics, &traffic);
    assert_overload_metrics_match_state(&metrics, &overload);
    assert_protection_metrics_match_state(&metrics, &circuit_breakers, &rate_limits);
}
