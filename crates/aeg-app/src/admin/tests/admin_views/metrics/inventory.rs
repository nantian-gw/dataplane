fn assert_inventory_metrics_match_state(metrics: &str, snapshot: &serde_json::Value) {
    assert_eq!(
        metric_u64(metrics, "nantian_gateway_dataplane_listener_count"),
        snapshot["listeners"].as_array().expect("listeners").len() as u64
    );
    assert_eq!(
        metric_u64(metrics, "nantian_gateway_dataplane_http_route_count"),
        snapshot["http_routes"]
            .as_array()
            .expect("http routes")
            .len() as u64
    );
    assert_eq!(
        metric_u64(metrics, "nantian_gateway_dataplane_grpc_route_count"),
        snapshot["grpc_routes"]
            .as_array()
            .expect("grpc routes")
            .len() as u64
    );
    assert_eq!(
        metric_u64(metrics, "nantian_gateway_dataplane_stream_route_count"),
        snapshot["stream_routes"]
            .as_array()
            .expect("stream routes")
            .len() as u64
    );
    assert_eq!(
        metric_u64(metrics, "nantian_gateway_dataplane_backend_count"),
        snapshot["backends"].as_array().expect("backends").len() as u64
    );
    assert_eq!(
        metric_u64(metrics, "nantian_gateway_dataplane_secret_count"),
        snapshot["secrets"].as_array().expect("secrets").len() as u64
    );
}
