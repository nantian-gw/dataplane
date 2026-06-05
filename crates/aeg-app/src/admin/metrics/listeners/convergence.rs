use super::super::{context::MetricsContext, prometheus::append_gauge};
use super::counts::ListenerMetricCounts;

pub(super) fn append_listener_convergence_metrics(
    out: &mut String,
    ctx: &MetricsContext,
    counts: &ListenerMetricCounts,
) {
    let snapshot = &ctx.snapshot;
    let listener_runtime_statuses = &counts.listener_runtime_statuses;

    append_gauge(
        out,
        "aether_gateway_dataplane_listener_convergence_blocked_count",
        "Number of listeners not yet converged onto the active snapshot version because they are pending, rejected, or stale.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.listener_current_status.as_str(),
                    "pending" | "rejected" | "stale"
                )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_convergence_blocked_http_count",
        "Number of HTTP-plane listeners not yet converged onto the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "http"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected" | "stale"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_convergence_blocked_stream_count",
        "Number of stream-plane listeners not yet converged onto the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "stream"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected" | "stale"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_convergence_blocked_tls_count",
        "Number of TLS-plane listeners not yet converged onto the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "tls"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected" | "stale"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_convergence_blocked_none_count",
        "Number of listeners outside the HTTP, TLS, and stream planes not yet converged onto the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "none"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected" | "stale"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_convergence_severity_level",
        "Numeric convergence severity for listeners on the active snapshot: 0=ok, 1=warning, 2=critical.",
        counts.listener_convergence_severity_level,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_apply_blocked_count",
        "Number of listeners whose active snapshot application is still blocked because they are pending or rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.listener_current_status.as_str(),
                    "pending" | "rejected"
                )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_apply_blocked_http_count",
        "Number of HTTP-plane listeners whose active snapshot application is still blocked.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "http"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_apply_blocked_stream_count",
        "Number of stream-plane listeners whose active snapshot application is still blocked.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "stream"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_apply_blocked_tls_count",
        "Number of TLS-plane listeners whose active snapshot application is still blocked.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "tls"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_apply_blocked_none_count",
        "Number of listeners outside the HTTP, TLS, and stream planes whose active snapshot application is still blocked.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "none"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_awaiting_current_attempt_count",
        "Number of listeners still pending because they have not yet attempted the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_current_status == "pending"
                    && status.listener_last_attempt_version != snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_awaiting_current_attempt_http_count",
        "Number of HTTP-plane listeners still pending without an attempt for the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "http"
                    && status.listener_current_status == "pending"
                    && status.listener_last_attempt_version != snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_awaiting_current_attempt_stream_count",
        "Number of stream-plane listeners still pending without an attempt for the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "stream"
                    && status.listener_current_status == "pending"
                    && status.listener_last_attempt_version != snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_awaiting_current_attempt_tls_count",
        "Number of TLS-plane listeners still pending without an attempt for the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "tls"
                    && status.listener_current_status == "pending"
                    && status.listener_last_attempt_version != snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_awaiting_current_attempt_none_count",
        "Number of listeners outside the HTTP, TLS, and stream planes still pending without an attempt for the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "none"
                    && status.listener_current_status == "pending"
                    && status.listener_last_attempt_version != snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_attempt_blocked_count",
        "Number of listeners that have already attempted the active snapshot version but are still pending or rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.listener_current_status.as_str(),
                    "pending" | "rejected"
                ) && status.listener_last_attempt_version == snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_attempt_blocked_http_count",
        "Number of HTTP-plane listeners that have already attempted the active snapshot version but are still pending or rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "http"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
                    && status.listener_last_attempt_version == snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_attempt_blocked_stream_count",
        "Number of stream-plane listeners that have already attempted the active snapshot version but are still pending or rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "stream"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
                    && status.listener_last_attempt_version == snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_attempt_blocked_tls_count",
        "Number of TLS-plane listeners that have already attempted the active snapshot version but are still pending or rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "tls"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
                    && status.listener_last_attempt_version == snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_current_attempt_blocked_none_count",
        "Number of listeners outside the HTTP, TLS, and stream planes that have already attempted the active snapshot version but are still pending or rejected.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "none"
                    && matches!(
                        status.listener_current_status.as_str(),
                        "pending" | "rejected"
                    )
                    && status.listener_last_attempt_version == snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_drift_count",
        "Number of listeners still serving a stale last-good version instead of the active snapshot.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_current_status == "stale")
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_drift_http_count",
        "Number of HTTP-plane listeners still serving a stale last-good version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "http" && status.listener_current_status == "stale"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_drift_stream_count",
        "Number of stream-plane listeners still serving a stale last-good version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "stream" && status.listener_current_status == "stale"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_drift_tls_count",
        "Number of TLS-plane listeners still serving a stale last-good version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "tls" && status.listener_current_status == "stale"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_listener_serving_drift_none_count",
        "Number of listeners outside the HTTP, TLS, and stream planes still serving a stale last-good version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.runtime_plane == "none" && status.listener_current_status == "stale"
            })
            .count() as u64,
    );
}
