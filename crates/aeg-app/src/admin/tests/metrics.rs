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
            if let Some(name) = current_name.take() {
                if !has_samples {
                    families.push(name);
                }
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

    if let Some(name) = current_name {
        if !has_samples {
            families.push(name);
        }
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
fn runtime_metrics_include_tls_plane_reload_state() {
    let state = test_state(None);
    state.runtime.observe_tls_runtime_started();
    state.runtime.observe_tls_listener_reload_success("v-test");

    let metrics = render_metrics(&state);

    assert!(
        metrics.contains("aether_gateway_dataplane_runtime_tls_listener_reload_failures_total 0")
    );
    assert!(metrics.contains("aether_gateway_dataplane_runtime_tls_current_rejected 0"));
    assert!(metrics.contains("runtime_tls_required=\"true\""));
    assert!(metrics.contains("runtime_tls_current_status=\"accepted\""));
    assert!(metrics.contains("runtime_tls_last_reload_attempt_version=\"v-test\""));
    assert!(metrics.contains("runtime_tls_last_good_reload_version=\"v-test\""));
}
