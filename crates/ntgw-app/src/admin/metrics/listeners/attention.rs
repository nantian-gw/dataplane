use super::super::prometheus::append_gauge;
use super::counts::ListenerMetricCounts;

pub(super) fn append_listener_attention_metrics(out: &mut String, counts: &ListenerMetricCounts) {
    let listener_runtime_statuses = &counts.listener_runtime_statuses;

    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_severity_level",
        "Numeric listener attention severity: 0=ok, 1=warning, 2=critical.",
        counts.listener_attention_severity_level,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_risk_pending_unrecovered_count",
        "Number of listeners currently pending on the active snapshot while also carrying an unrecovered failure history.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_current_status == "pending"
                    && status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_risk_rejected_unrecovered_count",
        "Number of listeners currently rejected on the active snapshot while also carrying an unrecovered failure history.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_current_status == "rejected"
                    && status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_risk_stale_unrecovered_count",
        "Number of listeners currently stale on the active snapshot while also carrying an unrecovered failure history.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_current_status == "stale"
                    && status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_required_count",
        "Number of listeners currently requiring operator attention.",
        counts.listener_attention_required_count as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_http_count",
        "Number of listeners currently requiring attention in the HTTP runtime plane.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_attention_required && status.runtime_plane == "http")
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_stream_count",
        "Number of listeners currently requiring attention in the stream runtime plane.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_attention_required && status.runtime_plane == "stream")
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_tls_count",
        "Number of listeners currently requiring attention in the TLS runtime plane.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_attention_required && status.runtime_plane == "tls")
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_none_count",
        "Number of listeners currently requiring attention outside the HTTP, TLS, and stream runtime planes.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_attention_required && status.runtime_plane == "none")
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_pending_count",
        "Number of listeners currently requiring attention because they are still pending.",
        counts.listener_attention_pending_count as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_rejected_count",
        "Number of listeners currently requiring attention because they are rejected.",
        counts.listener_attention_rejected_count as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_stale_count",
        "Number of listeners currently requiring attention because they are still serving a stale last-good version.",
        counts.listener_attention_stale_count as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_attention_unrecovered_failure_count",
        "Number of listeners currently requiring attention because they have not recovered from an observed failure.",
        counts.listener_attention_unrecovered_failure_count as u64,
    );
}
