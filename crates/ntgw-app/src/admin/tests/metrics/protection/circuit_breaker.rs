#[test]
fn render_metrics_exposes_circuit_breaker_counters() {
    let state = test_state(None);
    replace_circuit_breaker(
        &state,
        HttpCircuitBreakerOptions {
            backend_max_inflight_requests: 1,
        },
    );
    let _permit = with_circuit_breaker(&state, |controller| {
        controller
            .try_acquire_backend("default/echo:8080")
            .expect("first circuit-breaker request should pass")
    });
    assert!(with_circuit_breaker(&state, |controller| {
        controller.try_acquire_backend("default/echo:8080").is_err()
    }));

    let metrics = render_metrics(&state);

    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_circuit_breaker_backend_max_inflight_requests 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_circuit_breaker_backend_inflight_current{backend=\"default/echo:8080\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_circuit_breaker_rejected_total{scope=\"total\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_circuit_breaker_rejected_total{scope=\"backend\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_circuit_breaker_rejected_backend_total{backend=\"default/echo:8080\"} 1"
    ));
}
