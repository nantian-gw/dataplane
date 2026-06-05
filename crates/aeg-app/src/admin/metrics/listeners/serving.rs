use super::super::prometheus::append_gauge;
use super::counts::ListenerMetricCounts;

pub(super) fn append_listener_serving_metrics(out: &mut String, counts: &ListenerMetricCounts) {
    let listener_runtime_statuses = &counts.listener_runtime_statuses;

    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_current_snapshot_count",
        "Number of listeners currently serving the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_serving_current_snapshot)
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_state_none_count",
        "Number of listeners that do not currently expose any serving version for the active snapshot view.",
        counts.listener_serving_none_count as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_state_current_accepted_count",
        "Number of listeners serving the active snapshot through a normal accepted apply.",
        counts.listener_serving_current_accepted_count as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_state_current_retained_count",
        "Number of listeners serving the active snapshot through a retained in-place runtime state.",
        counts.listener_serving_current_retained_count as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_last_good_snapshot_count",
        "Number of listeners currently serving a retained last-good snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_serving_last_good_snapshot)
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_state_last_good_rejected_count",
        "Number of listeners still serving last-good because the active snapshot was explicitly rejected for them.",
        counts.listener_serving_last_good_rejected_count as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_state_last_good_stale_count",
        "Number of listeners still serving last-good because they have drifted behind the active snapshot without an explicit current rejection.",
        counts.listener_serving_last_good_stale_count as u64,
    );
}
