#[test]
fn render_metrics_exposes_endpoint_health_runtime_state() {
    let state = test_state(None);
    {
        let snapshot = state.snapshot.write();
        let endpoint = snapshot.backends[0].endpoints[0].clone();
        let selected = SelectedBackend {
            route_kind: RouteKind::Http,
            route_name: String::new(),
            route_namespace: String::new(),
            rule_index: None,
            route_annotations: Default::default(),
            listener_name: String::new(),
            listener_protocol: String::new(),
            backend: endpoint.clone(),
            backend_name: "default/api:80".to_string(),
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: None,
        };

        for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
            snapshot.record_endpoint_failure(&selected);
        }
        snapshot.record_endpoint_active_probe_failure(selected.backend_name.as_str(), &endpoint, 1);
    }

    let metrics = render_metrics(&state);

    assert!(metrics.contains("aether_gateway_dataplane_endpoint_runtime_tracked_current 1"));
    assert!(metrics.contains("aether_gateway_dataplane_endpoint_passive_ejected_current 1"));
    assert!(metrics.contains("aether_gateway_dataplane_endpoint_active_unhealthy_current 1"));
}

#[test]
fn render_metrics_exposes_endpoint_recovery_latency_histogram() {
    let state = test_state(None);
    {
        let snapshot = state.snapshot.write();
        let endpoint = snapshot.backends[0].endpoints[0].clone();
        let selected = SelectedBackend {
            route_kind: RouteKind::Http,
            route_name: String::new(),
            route_namespace: String::new(),
            rule_index: None,
            route_annotations: Default::default(),
            listener_name: String::new(),
            listener_protocol: String::new(),
            backend: endpoint.clone(),
            backend_name: "default/api:80".to_string(),
            filters: Vec::new(),
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence: None,
            backend_tls: None,
        };

        for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
            snapshot.record_endpoint_failure(&selected);
        }
        snapshot.record_endpoint_success(&selected);
    }

    let metrics = render_metrics(&state);

    assert!(
        metrics.contains("# TYPE aether_gateway_dataplane_endpoint_recovery_latency_ms histogram")
    );
    assert!(metrics
        .contains("aether_gateway_dataplane_endpoint_recovery_latency_ms_bucket{le=\"+Inf\"} 1"));
    assert!(metrics.contains("aether_gateway_dataplane_endpoint_recovery_latency_ms_count 1"));
}
