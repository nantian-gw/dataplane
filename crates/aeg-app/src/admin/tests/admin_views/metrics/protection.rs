fn assert_protection_metrics_match_state(
    metrics: &str,
    circuit_breakers: &serde_json::Value,
    rate_limits: &serde_json::Value,
) {
    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_http_circuit_breaker_backend_max_inflight_requests"
        ),
        circuit_breakers["backendMaxInflightRequests"]
            .as_u64()
            .expect("circuit-breaker max inflight")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "aether_gateway_dataplane_http_circuit_breaker_backend_inflight_current",
            "backend",
            "default/api:80"
        ),
        circuit_breakers["backendInflightCurrent"]["default/api:80"]
            .as_u64()
            .expect("circuit-breaker inflight")
    );

    assert_eq!(
        metric_u64(
            metrics,
            "aether_gateway_dataplane_http_rate_limit_global_requests_per_second"
        ),
        rate_limits["global"]["requestsPerSecond"]
            .as_u64()
            .expect("rate-limit rps")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "aether_gateway_dataplane_http_rate_limit_listener_available_tokens",
            "listener",
            "web"
        ),
        rate_limits["listener"]["availableTokensByName"]["web"]
            .as_u64()
            .expect("listener tokens")
    );
    assert_eq!(
        labeled_metric_u64(
            metrics,
            "aether_gateway_dataplane_http_rate_limit_rejected_route_total",
            "route",
            "Http/default/shared"
        ),
        rate_limits["rejectedRouteByName"]["Http/default/shared"]
            .as_u64()
            .expect("route rejected")
    );
}
