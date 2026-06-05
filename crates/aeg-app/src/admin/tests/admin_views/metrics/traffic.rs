fn assert_traffic_metrics_match_state(metrics: &str, traffic: &serde_json::Value) {
    assert_eq!(
        metric_u64(metrics, "aether_gateway_dataplane_traffic_events_total"),
        traffic["total_events"]
            .as_u64()
            .expect("traffic total events")
    );
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_traffic_request_events_total"
        ),
        traffic["total_request_events"]
            .as_u64()
            .expect("traffic request events")
    );
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_traffic_bytes_received_total"
        ),
        traffic["total_bytes_received"]
            .as_u64()
            .expect("traffic received bytes")
    );
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_traffic_bytes_sent_total"
        ),
        traffic["total_bytes_sent"]
            .as_u64()
            .expect("traffic sent bytes")
    );
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_traffic_retry_attempts_total"
        ),
        traffic["total_retry_attempts"]
            .as_u64()
            .expect("traffic retry attempts")
    );
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_traffic_upstream_pool_hits_total"
        ),
        traffic["total_upstream_pool_hits"]
            .as_u64()
            .expect("traffic pool hits")
    );
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_traffic_upstream_pool_misses_total"
        ),
        traffic["total_upstream_pool_misses"]
            .as_u64()
            .expect("traffic pool misses")
    );
    assert_eq!(
        metric_f64(metrics, "aether_gateway_dataplane_traffic_retry_rate"),
        traffic["total_retried_events"]
            .as_u64()
            .expect("retried events") as f64
            / traffic["total_request_events"]
                .as_u64()
                .expect("request events") as f64
    );
    assert_eq!(
        metric_f64(
            metrics,
            "aether_gateway_dataplane_traffic_upstream_pool_hit_ratio"
        ),
        traffic["total_upstream_pool_hits"]
            .as_u64()
            .expect("pool hits") as f64
            / (traffic["total_upstream_pool_hits"]
                .as_u64()
                .expect("pool hits")
                + traffic["total_upstream_pool_misses"]
                    .as_u64()
                    .expect("pool misses")) as f64
    );
}
