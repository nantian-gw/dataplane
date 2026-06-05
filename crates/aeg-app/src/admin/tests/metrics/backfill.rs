use super::*;

#[test]
fn render_metrics_backfills_zero_samples_for_snapshot_labels() {
    let state = test_state(None);

    let metrics = render_metrics(&state);

    for expected in [
        "nantian_gateway_dataplane_http_listener_inflight_current{listener=\"web\"} 0",
        "nantian_gateway_dataplane_http_route_inflight_current{route=\"Http/default/web\"} 0",
        "nantian_gateway_dataplane_http_route_inflight_current{route=\"Grpc/default/grpc\"} 0",
        "nantian_gateway_dataplane_http_overload_rejected_listener_total{listener=\"web\"} 0",
        "nantian_gateway_dataplane_http_overload_rejected_route_total{route=\"Http/default/web\"} 0",
        "nantian_gateway_dataplane_http_overload_rejected_route_total{route=\"Grpc/default/grpc\"} 0",
        "nantian_gateway_dataplane_tcp_listener_connections_current{listener=\"passthrough\"} 0",
        "nantian_gateway_dataplane_tcp_overload_rejected_listener_total{listener=\"passthrough\"} 0",
        "nantian_gateway_dataplane_http_circuit_breaker_backend_inflight_current{backend=\"default/api:80\"} 0",
        "nantian_gateway_dataplane_http_circuit_breaker_backend_inflight_current{backend=\"default/http2-clear:8080\"} 0",
        "nantian_gateway_dataplane_http_circuit_breaker_backend_inflight_current{backend=\"ops/tcp-service:9000\"} 0",
        "nantian_gateway_dataplane_http_circuit_breaker_rejected_backend_total{backend=\"default/api:80\"} 0",
        "nantian_gateway_dataplane_http_rate_limit_listener_available_tokens{listener=\"web\"} 0",
        "nantian_gateway_dataplane_http_rate_limit_route_available_tokens{route=\"Http/default/web\"} 0",
        "nantian_gateway_dataplane_http_rate_limit_route_available_tokens{route=\"Grpc/default/grpc\"} 0",
        "nantian_gateway_dataplane_http_rate_limit_rejected_listener_total{listener=\"web\"} 0",
        "nantian_gateway_dataplane_http_rate_limit_rejected_route_total{route=\"Http/default/web\"} 0",
        "nantian_gateway_dataplane_http_rate_limit_rejected_route_total{route=\"Grpc/default/grpc\"} 0",
    ] {
        assert!(metrics.contains(expected), "missing metric sample: {expected}");
    }
}
