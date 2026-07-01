use super::*;

include!("transport_resources/resources_features.rs");
include!("transport_resources/xds.rs");
include!("transport_resources/runtime_reload.rs");
include!("transport_resources/traffic.rs");
include!("transport_resources/composed_overviews.rs");

#[test]
fn summary_view_exposes_resources_features_xds_runtime_traffic_and_composed_overviews() {
    let value = build_runtime_rejection_summary_value();

    assert_resource_and_feature_overviews(&value);
    assert_xds_overview(&value);
    assert_runtime_reload_overview(&value);
    assert_traffic_overview(&value);
    assert_composed_overviews_and_warnings(&value);
}

#[test]
fn summary_traffic_retry_rate_uses_request_event_denominator() {
    let state = test_state(None);
    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string().into(),
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
        protocol: "TCP".to_string().into(),
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

    let value = build_summary_value(&state);

    assert_eq!(value["trafficTotalEvents"], 2);
    assert_eq!(value["trafficTotalRetriedEvents"], 1);
    assert_eq!(value["trafficTotalRetryAttempts"], 1);
    assert_eq!(value["trafficRetriedSuccessEvents"], 0);
    assert_eq!(value["trafficRetryRate"], 1.0);
    assert_eq!(value["trafficFailoverSuccessRate"], 0.0);
    assert_eq!(value["trafficOverview"]["rates"]["retry"], 1.0);
    assert_eq!(value["trafficOverview"]["rates"]["failoverSuccess"], 0.0);
    assert_eq!(
        value["trafficOverview"]["summary"]["counts"]["retriedEvents"],
        1
    );
    assert_eq!(
        value["trafficOverview"]["summary"]["counts"]["retryAttempts"],
        1
    );
    assert_eq!(
        value["trafficOverview"]["summary"]["counts"]["retriedSuccessEvents"],
        0
    );
    assert_eq!(value["trafficOverview"]["events"]["retried"], 1);
    assert_eq!(value["trafficOverview"]["events"]["retryAttempts"], 1);
    assert_eq!(value["trafficOverview"]["events"]["retriedSuccess"], 0);
    assert_eq!(
        value["trafficOverview"]["summary"]["status"]["retryRate"],
        1.0
    );
}

#[test]
fn summary_traffic_upstream_pool_views_ignore_stream_events() {
    let state = test_state(None);
    let mut buckets = [0; ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
    buckets[ntgw_observability::upstream_connect_latency_ms_bucket_index(17)] = 1;

    state.traffic.observe(TrafficObservation {
        listener_name: "tcp".to_string(),
        protocol: "TCP".to_string().into(),
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

    let value = build_summary_value(&state);

    assert_eq!(value["trafficTotalEvents"], 1);
    assert_eq!(value["trafficUpstreamPoolHits"], 0);
    assert_eq!(value["trafficUpstreamPoolMisses"], 0);
    assert_eq!(value["trafficUpstreamPoolHitRatio"], 0.0);
    assert_eq!(value["trafficUpstreamConnectLatencyMsAvg"], 0.0);
    assert_eq!(value["trafficUpstreamConnectLatencyMsMax"], 0);
    assert_eq!(
        value["trafficOverview"]["summary"]["counts"]["upstreamPoolHits"],
        0
    );
    assert_eq!(
        value["trafficOverview"]["summary"]["counts"]["upstreamPoolMisses"],
        0
    );
    assert_eq!(
        value["trafficOverview"]["summary"]["status"]["upstreamPoolHitRatio"],
        0.0
    );
    assert_eq!(
        value["trafficOverview"]["summary"]["status"]["upstreamConnectAvgMs"],
        0.0
    );
    assert_eq!(
        value["trafficOverview"]["summary"]["status"]["upstreamConnectMaxMs"],
        0
    );
}
