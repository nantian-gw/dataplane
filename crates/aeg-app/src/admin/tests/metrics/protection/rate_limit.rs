#[test]
fn render_metrics_exposes_rate_limit_counters() {
    let state = test_state(None);
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

    let metrics = render_metrics(&state);

    assert!(
        metrics.contains("nantian_gateway_dataplane_http_rate_limit_global_requests_per_second 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_http_rate_limit_global_burst 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_rate_limit_global_available_tokens 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_rate_limit_listener_requests_per_second 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_http_rate_limit_listener_burst 2"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_rate_limit_listener_available_tokens{listener=\"web\"} 1"
    ));
    assert!(
        metrics.contains("nantian_gateway_dataplane_http_rate_limit_route_requests_per_second 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_http_rate_limit_route_burst 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_rate_limit_route_available_tokens{route=\"Http/default/shared\"} 0"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_http_rate_limit_allowed_total 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_rate_limit_rejected_total{scope=\"total\"} 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_rate_limit_rejected_total{scope=\"route\"} 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_rate_limit_rejected_route_total{route=\"Http/default/shared\"} 1"
    ));
}

#[test]
fn render_metrics_exposes_rate_limit_scope_enabled_state() {
    let state = test_state(None);
    let disabled = render_metrics(&state);

    assert!(disabled.contains("nantian_gateway_dataplane_http_rate_limit_global_enabled 0"));
    assert!(disabled.contains("nantian_gateway_dataplane_http_rate_limit_listener_enabled 0"));
    assert!(disabled.contains("nantian_gateway_dataplane_http_rate_limit_route_enabled 0"));

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
    let enabled = render_metrics(&state);

    assert!(enabled.contains("nantian_gateway_dataplane_http_rate_limit_global_enabled 1"));
    assert!(enabled.contains("nantian_gateway_dataplane_http_rate_limit_listener_enabled 1"));
    assert!(enabled.contains("nantian_gateway_dataplane_http_rate_limit_route_enabled 1"));
}
