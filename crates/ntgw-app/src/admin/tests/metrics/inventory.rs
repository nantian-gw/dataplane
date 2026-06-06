use super::*;

#[test]
fn render_metrics_exposes_snapshot_inventory_and_session_persistence_state() {
    let state = test_state(None);
    set_http3_configured(&state, true);
    set_session_persistence_uses_ephemeral_secret(&state, true);

    let mut snapshot = fixture_snapshot();
    snapshot.http_routes[0].rules[0].session_persistence = Some(SessionPersistence {
        session_name: "route-sticky".to_string(),
        session_type: "Cookie".to_string(),
        cookie: Some(CookieConfig {
            lifetime_type: "Permanent".to_string(),
        }),
        ..SessionPersistence::default()
    });
    snapshot.grpc_routes[0].rules[0].session_persistence = Some(SessionPersistence {
        session_name: "grpc-sticky".to_string(),
        session_type: "Header".to_string(),
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
    *state.snapshot.write() = snapshot;

    state
        .xds
        .observe_connect_failure_with_error("connection refused");
    state.xds.observe_stream_failure_with_error("rpc closed");
    state.xds.observe_snapshot_applied("v-test");

    let metrics = render_metrics(&state);

    assert!(metrics.contains("nantian_gateway_dataplane_ready 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_route_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_grpc_route_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_stream_route_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_backend_count 3"));
    assert!(metrics.contains("nantian_gateway_dataplane_secret_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_http3_configured 1"));
    assert!(metrics.contains(&format!(
        "nantian_gateway_dataplane_http3_available {}",
        u64::from(ntgw_http::http3_available())
    )));
    assert!(metrics.contains(&format!(
        "nantian_gateway_dataplane_http3_enabled {}",
        u64::from(ntgw_http::http3_available())
    )));
    assert!(metrics.contains("nantian_gateway_dataplane_session_persistence_active 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_session_persistence_ephemeral_secret 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_session_persistence_route_rule_count 2"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_session_persistence_backend_policy_count 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_xds_connect_failures_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_stream_failures_total 1"));

    let last_apply = metric_value(
        &metrics,
        "nantian_gateway_dataplane_xds_last_apply_timestamp_seconds",
    )
    .expect("last apply timestamp metric");
    assert!(last_apply.parse::<u64>().expect("unix timestamp") > 0);
}

#[test]
fn render_metrics_omits_header_only_families_without_known_labels() {
    let state = test_state(None);
    *state.snapshot.write() = Snapshot {
        id: "v-empty".to_string(),
        ..Snapshot::default()
    };

    let metrics = render_metrics(&state);
    let empty_families = empty_metric_families(&metrics);

    assert!(
        empty_families.is_empty(),
        "metrics contained header-only families: {empty_families:?}"
    );
}

#[test]
fn render_metrics_stays_stable_for_empty_snapshot_with_default_runtime_controls() {
    let state = test_state(None);
    *state.snapshot.write() = Snapshot::default();

    let metrics = render_metrics(&state);
    let empty_families = empty_metric_families(&metrics);

    assert!(
        empty_families.is_empty(),
        "metrics contained header-only families: {empty_families:?}"
    );

    for expected in [
        "nantian_gateway_dataplane_ready 0",
        "nantian_gateway_dataplane_listener_count 0",
        "nantian_gateway_dataplane_http_route_count 0",
        "nantian_gateway_dataplane_grpc_route_count 0",
        "nantian_gateway_dataplane_stream_route_count 0",
        "nantian_gateway_dataplane_backend_count 0",
        "nantian_gateway_dataplane_http_circuit_breaker_backend_max_inflight_requests 0",
        "nantian_gateway_dataplane_http_rate_limit_global_requests_per_second 0",
        "nantian_gateway_dataplane_http_rate_limit_global_burst 0",
        "nantian_gateway_dataplane_http_rate_limit_global_available_tokens 0",
    ] {
        assert!(
            metrics.contains(expected),
            "missing metric sample: {expected}"
        );
    }
}

#[test]
fn render_metrics_omits_traffic_ratio_gauges_without_denominators() {
    let state = test_state(None);
    *state.snapshot.write() = Snapshot::default();

    let metrics = render_metrics(&state);

    for name in [
        "nantian_gateway_dataplane_traffic_retry_rate",
        "nantian_gateway_dataplane_traffic_failover_success_rate",
        "nantian_gateway_dataplane_traffic_upstream_pool_hit_ratio",
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_average",
    ] {
        assert!(
            metric_value(&metrics, name).is_none(),
            "{name} must be absent until its denominator has at least one observation"
        );
    }

    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(200),
        latency_ms: 10,
        bytes_received: 1,
        bytes_sent: 2,
        response_flags: String::new(),
        ..TrafficObservation::default()
    });

    let metrics = render_metrics(&state);
    assert_eq!(
        metric_value(&metrics, "nantian_gateway_dataplane_traffic_retry_rate"),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_failover_success_rate"
        ),
        None
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_pool_hit_ratio"
        ),
        None
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_average"
        ),
        None
    );
}

#[test]
fn render_metrics_retry_rate_uses_request_event_denominator() {
    let state = test_state(None);
    *state.snapshot.write() = Snapshot::default();

    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(503),
        latency_ms: 10,
        bytes_received: 1,
        bytes_sent: 2,
        retry_attempts: 1,
        response_flags: "UC".to_string(),
        ..TrafficObservation::default()
    });
    state.traffic.observe(TrafficObservation {
        listener_name: "tcp".to_string(),
        protocol: "TCP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "tcp".to_string(),
        route_kind: "Tcp".to_string(),
        backend_name: "default/tcp:9000".to_string(),
        status: None,
        latency_ms: 120_000,
        bytes_received: 10,
        bytes_sent: 20,
        retry_attempts: 4,
        retried_success: true,
        response_flags: String::new(),
        ..TrafficObservation::default()
    });

    let metrics = render_metrics(&state);

    assert_eq!(
        metric_value(&metrics, "nantian_gateway_dataplane_traffic_events_total"),
        Some("2")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_request_events_total"
        ),
        Some("1")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_retried_events_total"
        ),
        Some("1")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_retry_attempts_total"
        ),
        Some("1")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_retried_success_events_total"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(&metrics, "nantian_gateway_dataplane_traffic_retry_rate"),
        Some("1")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_failover_success_rate"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_status_other_total"
        ),
        Some("0")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"none\"} 0")
    );
}

#[test]
fn render_metrics_upstream_pool_views_ignore_stream_events() {
    let state = test_state(None);
    *state.snapshot.write() = Snapshot::default();
    let mut buckets = [0; ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
    buckets[ntgw_observability::upstream_connect_latency_ms_bucket_index(17)] = 1;

    state.traffic.observe(TrafficObservation {
        listener_name: "tcp".to_string(),
        protocol: "TCP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "tcp".to_string(),
        route_kind: "Tcp".to_string(),
        backend_name: "default/tcp:9000".to_string(),
        status: None,
        latency_ms: 120_000,
        bytes_received: 10,
        bytes_sent: 0,
        upstream_pool_hits: 3,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 17,
        upstream_connect_latency_ms_max: 17,
        upstream_connect_latency_ms_buckets: buckets,
        response_flags: "UF".to_string(),
        ..TrafficObservation::default()
    });

    let metrics = render_metrics(&state);

    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_pool_hits_total"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_pool_misses_total"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_peer_build_failures_total"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_pool_hit_ratio"
        ),
        None
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_total"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_max"
        ),
        Some("0")
    );
    assert_eq!(
        metric_value(
            &metrics,
            "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_average"
        ),
        None
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_count 0")
    );
}

#[test]
fn render_metrics_ready_matches_summary_readiness_for_pending_snapshot_without_last_good() {
    let shared = Snapshot::shared();
    *shared.write() = fixture_snapshot();
    shared.write().id = "v2".to_string();
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );

    let summary = build_summary_value(&state);
    assert_eq!(summary["ready"], false);
    assert_eq!(
        summary["readinessReason"],
        "current-snapshot-pending-without-last-good"
    );

    let metrics = render_metrics(&state);

    assert!(metrics.contains("nantian_gateway_dataplane_ready 0"));
}
