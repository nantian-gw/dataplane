use super::super::{
    context::MetricsContext,
    prometheus::{append_counter, append_gauge, prometheus_label},
};
use std::fmt::Write as _;

pub(super) fn append_xds_metrics(out: &mut String, ctx: &MetricsContext) {
    let xds = &ctx.xds;

    append_counter(
        out,
        "nantian_gateway_dataplane_xds_connect_failures_total",
        "Total number of failed control plane connection attempts.",
        xds.connect_failures,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_xds_stream_failures_total",
        "Total number of xDS stream failures that triggered a retry.",
        xds.stream_failures,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_xds_last_connect_failure_unix_seconds",
        "Unix timestamp of the most recent control plane connection failure retained by the dataplane, or 0 if none has been recorded.",
        xds.last_connect_failure_unix_seconds,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_xds_last_stream_failure_unix_seconds",
        "Unix timestamp of the most recent xDS stream failure retained by the dataplane, or 0 if none has been recorded.",
        xds.last_stream_failure_unix_seconds,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_xds_last_connect_error_retained",
        "1 if the dataplane retains a most recent control plane connection error detail, 0 otherwise.",
        u64::from(!xds.last_connect_error.is_empty()),
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_xds_last_stream_error_retained",
        "1 if the dataplane retains a most recent xDS stream error detail, 0 otherwise.",
        u64::from(!xds.last_stream_error.is_empty()),
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_xds_snapshots_applied_total",
        "Total number of configuration snapshots successfully applied.",
        xds.snapshots_applied,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_xds_snapshots_nacked_total",
        "Total number of configuration snapshots explicitly rejected after dataplane apply validation failed.",
        xds.snapshots_nacked,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_xds_snapshots_skipped_total",
        "Total number of duplicate configuration snapshots skipped without reapplying runtime state.",
        xds.snapshots_skipped,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_xds_last_apply_timestamp_seconds",
        "Unix timestamp of the most recent successfully applied snapshot.",
        xds.last_apply_unix_seconds,
    );
    append_apply_stage_histograms(out, ctx);
}

fn append_apply_stage_histograms(out: &mut String, ctx: &MetricsContext) {
    if ctx.xds.apply_stage_ms_histograms.is_empty() {
        return;
    }

    let name = "nantian_gateway_dataplane_xds_apply_stage_duration_ms";
    let _ = writeln!(
        out,
        "# HELP {name} xDS snapshot apply stage duration in milliseconds."
    );
    let _ = writeln!(out, "# TYPE {name} histogram");
    for histogram in &ctx.xds.apply_stage_ms_histograms {
        let stage = prometheus_label(&histogram.stage);
        for bucket in &histogram.buckets {
            let _ = writeln!(
                out,
                "{name}_bucket{{stage=\"{stage}\",le=\"{}\"}} {}",
                prometheus_label(&bucket.le),
                bucket.cumulative_count
            );
        }
        let _ = writeln!(out, "{name}_sum{{stage=\"{stage}\"}} {}", histogram.sum);
        let _ = writeln!(out, "{name}_count{{stage=\"{stage}\"}} {}", histogram.count);
    }
}
