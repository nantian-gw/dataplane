use super::super::prometheus::append_gauge;
use super::counts::ListenerMetricCounts;

pub(super) fn append_listener_current_metrics(out: &mut String, counts: &ListenerMetricCounts) {
    let listener_runtime_statuses = &counts.listener_runtime_statuses;

    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_idle_count",
        "Number of listeners currently classified as idle.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "idle")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_warming_count",
        "Number of listeners currently classified as warming.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "warming")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_pending_count",
        "Number of listeners currently classified as pending.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "pending")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_accepted_count",
        "Number of listeners currently classified as accepted.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "accepted")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_retained_count",
        "Number of listeners currently classified as retained.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "retained")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_rejected_count",
        "Number of listeners currently classified as rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "rejected")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_stale_count",
        "Number of listeners currently classified as stale.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "stale")
            .count() as u64,
    );
}
