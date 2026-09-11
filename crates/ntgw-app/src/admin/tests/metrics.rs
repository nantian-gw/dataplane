use super::*;

mod backfill;
mod inventory;
mod listener_risks;
mod protection;
mod traffic_runtime;

fn empty_metric_families(metrics: &str) -> Vec<String> {
    let mut families = Vec::new();
    let mut current_name: Option<String> = None;
    let mut has_samples = false;

    for line in metrics.lines() {
        if let Some(rest) = line.strip_prefix("# HELP ") {
            if let Some(name) = current_name.take()
                && !has_samples
            {
                families.push(name);
            }
            current_name = rest.split_whitespace().next().map(str::to_string);
            has_samples = false;
            continue;
        }

        if line.starts_with("# TYPE ") || line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        has_samples = true;
    }

    if let Some(name) = current_name
        && !has_samples
    {
        families.push(name);
    }

    families
}

fn duplicate_metric_families(metrics: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut duplicates = std::collections::BTreeSet::new();

    for line in metrics.lines() {
        let Some(rest) = line.strip_prefix("# HELP ") else {
            continue;
        };
        let Some(name) = rest.split_whitespace().next() else {
            continue;
        };
        if !seen.insert(name.to_string()) {
            duplicates.insert(name.to_string());
        }
    }

    duplicates.into_iter().collect()
}

fn metric_value<'a>(metrics: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name} ");
    metrics
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}

#[test]
fn render_metrics_does_not_emit_duplicate_metric_families() {
    let state = test_state(None);

    let metrics = render_metrics(&state);

    assert_eq!(duplicate_metric_families(&metrics), Vec::<String>::new());
}

#[test]
fn render_metrics_exposes_canonical_prefix_aliases() {
    let state = test_state(None);
    state.xds.observe_snapshot_applied("v-test");
    state.xds.observe_apply_stage_duration("decode", 7);
    state.traffic.observe(TrafficObservation {
        listener_name: "web".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:80".to_string(),
        status: Some(200),
        latency_ms: 42,
        bytes_received: 10,
        bytes_sent: 20,
        response_flags: String::new(),
        ..TrafficObservation::default()
    });

    let metrics = render_metrics(&state);

    for expected in [
        "nantian_gateway_dataplane_ready 1",
        "nantian_gw_dataplane_ready 1",
        "nantian_gateway_dataplane_traffic_events_total 1",
        "nantian_gw_dataplane_traffic_events_total 1",
        "nantian_gateway_dataplane_http_listener_inflight_current{listener=\"web\"} 0",
        "nantian_gw_dataplane_http_listener_inflight_current{listener=\"web\"} 0",
        "nantian_gateway_dataplane_traffic_request_latency_ms_count{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"2xx\",response_flag=\"none\"} 1",
        "nantian_gw_dataplane_traffic_request_latency_ms_count{listener=\"web\",protocol=\"HTTP\",route_kind=\"HTTPRoute\",status_class=\"2xx\",response_flag=\"none\"} 1",
        "nantian_gateway_dataplane_xds_apply_stage_duration_ms_count{stage=\"decode\"} 1",
        "nantian_gw_dataplane_xds_apply_stage_duration_ms_count{stage=\"decode\"} 1",
    ] {
        assert!(
            metrics.contains(expected),
            "missing metric sample: {expected}"
        );
    }

    assert_eq!(duplicate_metric_families(&metrics), Vec::<String>::new());
}

#[test]
fn runtime_metrics_include_tls_plane_reload_state() {
    let state = test_state(None);
    state.runtime.observe_tls_runtime_started();
    state.runtime.observe_tls_listener_reload_success("v-test");

    let metrics = render_metrics(&state);

    assert!(
        metrics.contains("nantian_gateway_dataplane_runtime_tls_listener_reload_failures_total 0")
    );
    assert!(metrics.contains("nantian_gateway_dataplane_runtime_tls_current_rejected 0"));
    assert!(metrics.contains("runtime_tls_required=\"true\""));
    assert!(metrics.contains("runtime_tls_current_status=\"accepted\""));
    assert!(metrics.contains("runtime_tls_last_reload_attempt_version=\"v-test\""));
    assert!(metrics.contains("runtime_tls_last_good_reload_version=\"v-test\""));
}
