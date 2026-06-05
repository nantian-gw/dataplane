use super::super::{context::MetricsContext, prometheus::append_gauge};
use super::counts::ListenerMetricCounts;

pub(super) fn append_listener_recovery_metrics(
    out: &mut String,
    ctx: &MetricsContext,
    counts: &ListenerMetricCounts,
) {
    let snapshot = &ctx.snapshot;
    let listener_runtime_statuses = &counts.listener_runtime_statuses;

    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_has_ever_failed_count",
        "Number of listeners that have observed at least one failure in the current process.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_has_ever_failed)
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_recovered_from_failure_count",
        "Number of listeners that have recovered from a previously observed failure.",
        listener_runtime_statuses
            .iter()
            .filter(|status| status.listener_recovered_from_failure)
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_recovered_from_failure_http_count",
        "Number of listeners in the HTTP runtime plane that have recovered from a previously observed failure.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_recovered_from_failure && status.runtime_plane == "http"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_recovered_from_failure_stream_count",
        "Number of listeners in the stream runtime plane that have recovered from a previously observed failure.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_recovered_from_failure && status.runtime_plane == "stream"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_recovered_from_failure_tls_count",
        "Number of listeners in the TLS runtime plane that have recovered from a previously observed failure.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_recovered_from_failure && status.runtime_plane == "tls"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_recovered_from_failure_none_count",
        "Number of listeners outside the HTTP, TLS, and stream runtime planes that have recovered from a previously observed failure.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_recovered_from_failure && status.runtime_plane == "none"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_failure_count",
        "Number of listeners that have seen a failure and are not yet considered recovered.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed && !status.listener_recovered_from_failure
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_failure_http_count",
        "Number of listeners in the HTTP runtime plane that have seen a failure and are not yet considered recovered.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.runtime_plane == "http"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_failure_stream_count",
        "Number of listeners in the stream runtime plane that have seen a failure and are not yet considered recovered.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.runtime_plane == "stream"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_failure_tls_count",
        "Number of listeners in the TLS runtime plane that have seen a failure and are not yet considered recovered.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.runtime_plane == "tls"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_failure_none_count",
        "Number of listeners outside the HTTP, TLS, and stream runtime planes that have seen a failure and are not yet considered recovered.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.runtime_plane == "none"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_count",
        "Number of unrecovered listeners whose most recent failure belongs to the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version == snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_http_count",
        "Number of HTTP-plane unrecovered listeners whose most recent failure belongs to the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version == snapshot.id
                    && status.runtime_plane == "http"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_stream_count",
        "Number of stream-plane unrecovered listeners whose most recent failure belongs to the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version == snapshot.id
                    && status.runtime_plane == "stream"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_tls_count",
        "Number of TLS-plane unrecovered listeners whose most recent failure belongs to the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version == snapshot.id
                    && status.runtime_plane == "tls"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_current_snapshot_failure_none_count",
        "Number of unrecovered listeners outside the HTTP, TLS, and stream planes whose most recent failure belongs to the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version == snapshot.id
                    && status.runtime_plane == "none"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_count",
        "Number of unrecovered listeners whose most recent failure predates the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version != snapshot.id
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_http_count",
        "Number of HTTP-plane unrecovered listeners whose most recent failure predates the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version != snapshot.id
                    && status.runtime_plane == "http"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_stream_count",
        "Number of stream-plane unrecovered listeners whose most recent failure predates the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version != snapshot.id
                    && status.runtime_plane == "stream"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_tls_count",
        "Number of TLS-plane unrecovered listeners whose most recent failure predates the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version != snapshot.id
                    && status.runtime_plane == "tls"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_unrecovered_historical_failure_none_count",
        "Number of unrecovered listeners outside the HTTP, TLS, and stream planes whose most recent failure predates the active snapshot version.",
        listener_runtime_statuses
            .iter()
            .filter(|status| {
                status.listener_has_ever_failed
                    && !status.listener_recovered_from_failure
                    && status.listener_last_failure_version != snapshot.id
                    && status.runtime_plane == "none"
            })
            .count() as u64,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_listener_failure_recovery_severity_level",
        "Numeric listener failure-recovery severity: 0=ok, 1=warning, 2=critical.",
        counts.listener_failure_recovery_severity_level,
    );
}
