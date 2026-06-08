fn assert_overload_metrics_match_state(metrics: &str, overload: &serde_json::Value) {
    assert_eq!(
        metric_u64(
            metrics,
            "nantian_gateway_dataplane_http_global_inflight_current"
        ),
        overload["httpGlobalInflightCurrent"]
            .as_u64()
            .expect("http global inflight")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "nantian_gateway_dataplane_http_listener_inflight_current",
            "listener",
            "web"
        ),
        overload["httpListenerInflightCurrent"]["web"]
            .as_u64()
            .expect("http listener inflight")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "nantian_gateway_dataplane_http_route_inflight_current",
            "route",
            "Http/default/shared"
        ),
        overload["httpRouteInflightCurrent"]["Http/default/shared"]
            .as_u64()
            .expect("http route inflight")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "nantian_gateway_dataplane_http_overload_rejected_total",
            "scope",
            "total"
        ),
        overload["httpRejectedTotal"]
            .as_u64()
            .expect("http rejected total")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "nantian_gateway_dataplane_tcp_overload_rejected_listener_total",
            "listener",
            "passthrough"
        ),
        overload["tcpRejectedListenerByName"]["passthrough"]
            .as_u64()
            .expect("tcp rejected listener")
    );
}
