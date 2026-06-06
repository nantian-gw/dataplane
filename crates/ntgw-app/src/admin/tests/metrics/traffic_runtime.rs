use super::*;

#[test]
fn render_metrics_exposes_traffic_counters() {
    let state = test_state(None);
    state.xds.observe_snapshot_applied("v-test");
    state
        .xds
        .observe_snapshot_nacked("v-next", "listener reload failed");
    state.xds.observe_snapshot_skipped();
    state.xds.observe_apply_stage_duration("decode", 7);
    state.xds.observe_apply_stage_duration("listener_apply", 44);
    state
        .runtime
        .observe_http_listener_reload_failure("v-test", "web", "bind conflict");
    state.runtime.observe_http_tls_asset_reuses(3);
    state
        .runtime
        .observe_tls_listener_reload_failure("v-test", "passthrough", "tcp bind conflict");
    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(503),
        latency_ms: 27,
        bytes_received: 128,
        bytes_sent: 512,
        retry_attempts: 2,
        retried_success: false,
        upstream_pool_hits: 3,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 11,
        upstream_connect_latency_ms_max: 11,
        upstream_connect_latency_ms_buckets: {
            let mut buckets = [0; ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
            buckets[ntgw_observability::upstream_connect_latency_ms_bucket_index(11)] = 1;
            buckets
        },
        response_flags: "DC".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    state
        .traffic
        .observe_upstream_tls_handshake_failure(Some(13));
    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "stream".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/stream:80".to_string(),
        status: Some(200),
        latency_ms: 61_000,
        bytes_received: 64,
        bytes_sent: 1024,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 1,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: [0;
            ntgw_observability::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
        response_flags: String::new(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    state.udp_sessions.observe_session_started("default/gw/udp");
    state.udp_sessions.observe_queue_enqueued("default/gw/udp");
    state
        .udp_sessions
        .observe_queue_overflow_drop("default/gw/udp");
    state.udp_sessions.observe_idle_eviction("default/gw/udp");

    let metrics = render_metrics(&state);

    assert!(metrics.contains("nantian_gateway_dataplane_traffic_events_total 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_request_events_total 2"));
    assert!(metrics.contains(
        "# HELP nantian_gateway_dataplane_traffic_bytes_received_total Total downstream request body, session payload, and datagram bytes received across observed traffic."
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_bytes_received_total 192"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_bytes_sent_total 1536"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_latency_ms_total 61027"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_latency_ms_max 61000"));
    assert!(metrics.contains(
        "# HELP nantian_gateway_dataplane_traffic_request_latency_ms Cumulative Prometheus histogram of downstream request latency in milliseconds"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_request_latency_ms_bucket{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"5xx\",response_flag=\"DC\",le=\"25\"} 0"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_request_latency_ms_bucket{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"5xx\",response_flag=\"DC\",le=\"50\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_request_latency_ms_sum{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"5xx\",response_flag=\"DC\"} 27"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_request_latency_ms_count{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"5xx\",response_flag=\"DC\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_request_latency_ms_bucket{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"2xx\",response_flag=\"none\",le=\"60000\"} 0"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_request_latency_ms_bucket{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"2xx\",response_flag=\"none\",le=\"+Inf\"} 1"
    ));
    for line in metrics
        .lines()
        .filter(|line| line.starts_with("nantian_gateway_dataplane_traffic_request_latency_ms_"))
    {
        assert!(
            !line.contains("route=") && !line.contains("backend="),
            "request latency histogram must stay low-cardinality, got: {line}"
        );
    }
    assert_traffic_metrics_do_not_expose_topology_labels(&metrics);
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_retried_events_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_retry_attempts_total 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_retried_success_events_total 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_retry_rate 0.5"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_failover_success_rate 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_upstream_pool_hits_total 4"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_upstream_pool_misses_total 1"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_upstream_peer_build_failures_total 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_upstream_pool_hit_ratio 0.8"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_total 11")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_max 11")
    );
    assert!(metrics
        .contains("nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_average 11"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_bucket{le=\"10\"} 0"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_bucket{le=\"25\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_bucket{le=\"+Inf\"} 1"
    ));
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_sum 11")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_count 1")
    );
    assert!(metrics
        .contains("nantian_gateway_dataplane_traffic_upstream_tls_handshake_failures_total 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failure_latency_ms_bucket{le=\"10\"} 0"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failure_latency_ms_bucket{le=\"25\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failure_latency_ms_bucket{le=\"+Inf\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failure_latency_ms_sum 13"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failure_latency_ms_count 1"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_status_5xx_total 1"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"DC\"} 1")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"none\"} 1")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"IT\"} 0")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"MA\"} 0")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"UT\"} 0")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_traffic_response_flags_total{flag=\"UC\"} 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_access_log_writer_queue_depth"));
    assert!(metrics.contains("nantian_gateway_dataplane_access_log_writer_dropped_lines_total"));
    assert!(metrics.contains("nantian_gateway_dataplane_access_log_writer_flush_latency_ms_total"));
    assert!(metrics.contains("nantian_gateway_dataplane_access_log_writer_sink_errors_total"));
    assert!(metrics.contains("nantian_gateway_dataplane_udp_sessions_active_current 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_udp_session_queue_depth_current 1"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_udp_session_queue_overflow_dropped_total 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_udp_session_idle_evictions_total 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_udp_sessions_active_listener_current{listener=\"default/gw/udp\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_udp_session_queue_depth_listener_current{listener=\"default/gw/udp\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_udp_session_queue_overflow_dropped_listener_total{listener=\"default/gw/udp\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_udp_session_idle_evictions_listener_total{listener=\"default/gw/udp\"} 1"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_snapshots_applied_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_snapshots_nacked_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_snapshots_skipped_total 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_xds_apply_stage_duration_ms_bucket{stage=\"decode\",le=\"10\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_xds_apply_stage_duration_ms_sum{stage=\"listener_apply\"} 44"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_xds_apply_stage_duration_ms_count{stage=\"listener_apply\"} 1"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_last_nack_info 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_last_connect_failure_unix_seconds"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_last_stream_failure_unix_seconds"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_last_connect_error_retained 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_xds_last_stream_error_retained 1"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_runtime_http_listener_reload_failures_total 1")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_http_tls_asset_reuses_total 3"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_runtime_tls_listener_reload_failures_total 1")
    );
    assert!(metrics
        .contains("nantian_gateway_dataplane_runtime_stream_listener_reload_failures_total 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_current_snapshot_rejected 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_serving_last_good_snapshot 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_http_current_rejected 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_tls_current_rejected 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_stream_current_rejected 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_http_current_failure_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_tls_current_failure_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_stream_current_failure_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_idle_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_warming_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_pending_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_accepted_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_retained_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_rejected_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_stale_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_convergence_blocked_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_convergence_blocked_http_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_convergence_blocked_tls_count 1"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_convergence_blocked_stream_count 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_convergence_blocked_none_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_convergence_severity_level 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_apply_blocked_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_apply_blocked_http_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_apply_blocked_tls_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_apply_blocked_stream_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_apply_blocked_none_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_awaiting_current_attempt_count 0"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_awaiting_current_attempt_http_count 0"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_awaiting_current_attempt_tls_count 0")
    );
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_awaiting_current_attempt_stream_count 0"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_awaiting_current_attempt_none_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_current_attempt_blocked_count 2"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_current_attempt_blocked_http_count 1")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_current_attempt_blocked_tls_count 1")
    );
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_current_attempt_blocked_stream_count 0"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_current_attempt_blocked_none_count 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_drift_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_drift_http_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_drift_tls_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_drift_stream_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_drift_none_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_current_snapshot_count 0"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_serving_last_good_snapshot_count 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_serving_state_none_count 2"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_serving_state_current_accepted_count 0"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_serving_state_current_retained_count 0"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_serving_state_last_good_rejected_count 0"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_serving_state_last_good_stale_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_has_ever_failed_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_recovered_from_failure_count 0"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_recovered_from_failure_http_count 0")
    );
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_recovered_from_failure_tls_count 0")
    );
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_recovered_from_failure_stream_count 0"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_recovered_from_failure_none_count 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_unrecovered_failure_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_unrecovered_failure_http_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_unrecovered_failure_tls_count 1"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_unrecovered_failure_stream_count 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_unrecovered_failure_none_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_risk_pending_unrecovered_count 0"));
    assert!(
        metrics.contains("nantian_gateway_dataplane_listener_risk_rejected_unrecovered_count 2")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_listener_risk_stale_unrecovered_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_required_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_severity_level 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_http_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_tls_count 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_stream_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_none_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_pending_count 0"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_rejected_count 2"));
    assert!(metrics.contains("nantian_gateway_dataplane_listener_attention_stale_count 0"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_listener_attention_unrecovered_failure_count 2"));
    assert!(metrics.contains("last_good_snapshot_version=\"v-test\""));
    assert!(metrics.contains("current_snapshot_status=\"rejected\""));
    assert!(metrics.contains("current_snapshot_rejection_version=\"v-test\""));
    assert!(metrics.contains("current_snapshot_rejection_runtime=\"http+tls\""));
    assert!(metrics.contains("runtime_http_required=\"true\""));
    assert!(metrics.contains("runtime_http_current_status=\"rejected\""));
    assert!(metrics.contains("runtime_tls_required=\"true\""));
    assert!(metrics.contains("runtime_tls_current_status=\"rejected\""));
    assert!(metrics.contains("runtime_stream_required=\"false\""));
    assert!(metrics.contains("runtime_stream_current_status=\"idle\""));
    assert!(metrics.contains("runtime_http_last_reload_failure_listener=\"web\""));
    assert!(metrics.contains("runtime_tls_last_reload_failure_listener=\"passthrough\""));
    assert!(metrics.contains("runtime_last_reload_attempt_version=\"v-test\""));
    assert!(metrics.contains("runtime_last_good_reload_version=\"\""));
    assert!(metrics.contains("runtime_last_reload_failure_version=\"v-test\""));
    assert!(metrics.contains("runtime_tls_last_reload_attempt_version=\"v-test\""));
    assert!(metrics.contains("runtime_tls_last_good_reload_version=\"\""));
    assert!(metrics.contains("runtime_tls_last_reload_failure_version=\"v-test\""));
}

#[test]
fn render_metrics_describes_byte_counters_as_payload_bytes() {
    let state = test_state(None);
    state.traffic.observe(TrafficObservation {
        protocol: "TCP".to_string(),
        route_kind: "Tcp".to_string(),
        bytes_received: 42,
        bytes_sent: 128,
        ..TrafficObservation::default()
    });

    let metrics = render_metrics(&state);

    assert!(metrics.contains("nantian_gateway_dataplane_traffic_events_total 1"));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_request_events_total 0"));
    assert!(metrics.contains(
        "# HELP nantian_gateway_dataplane_traffic_bytes_received_total Total downstream request body, session payload, and datagram bytes received across observed traffic."
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_bytes_received_total 42"));
    assert!(metrics.contains(
        "# HELP nantian_gateway_dataplane_traffic_bytes_sent_total Total downstream response body, session payload, and datagram bytes sent across observed traffic."
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_traffic_bytes_sent_total 128"));
}

fn assert_traffic_metrics_do_not_expose_topology_labels(metrics: &str) {
    const FORBIDDEN_LABELS: &[&str] = &[
        "route",
        "route_namespace",
        "route_name",
        "backend",
        "backend_namespace",
        "backend_name",
        "pod",
        "endpoint",
    ];

    for line in metrics
        .lines()
        .filter(|line| line.starts_with("nantian_gateway_dataplane_traffic_"))
    {
        let Some((_, labels_and_value)) = line.split_once('{') else {
            continue;
        };
        let labels = labels_and_value
            .split_once('}')
            .map(|(labels, _)| labels)
            .unwrap_or(labels_and_value);
        for (label, _) in labels.split(',').filter_map(|label| label.split_once('=')) {
            assert!(
                !FORBIDDEN_LABELS.contains(&label),
                "traffic metrics must not expose high-cardinality topology label `{label}`: {line}"
            );
        }
    }
}
