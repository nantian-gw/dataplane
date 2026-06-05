use std::fmt::Write as _;

use super::{
    context::MetricsContext,
    prometheus::{append_gauge, prometheus_label},
};

pub(super) fn append_node_info_metrics(out: &mut String, ctx: &MetricsContext) {
    let snapshot = &ctx.snapshot;
    let runtime = &ctx.runtime;
    let xds = &ctx.xds;
    let current_snapshot = &ctx.current_snapshot;
    let http_runtime = &ctx.http_runtime;
    let tls_runtime = &ctx.tls_runtime;
    let stream_runtime = &ctx.stream_runtime;
    let node_id = &ctx.node_id;
    let cluster = &ctx.cluster;
    out.push_str("# HELP aether_gateway_dataplane_node_info Static dataplane node information.\n");
    out.push_str("# TYPE aether_gateway_dataplane_node_info gauge\n");
    let _ = writeln!(
        out,
        "aether_gateway_dataplane_node_info{{node_id=\"{}\",cluster=\"{}\",snapshot_version=\"{}\",xds_last_snapshot_version=\"{}\",last_good_snapshot_version=\"{}\",current_snapshot_status=\"{}\",current_snapshot_rejection_version=\"{}\",current_snapshot_rejection_runtime=\"{}\",runtime_http_required=\"{}\",runtime_http_current_status=\"{}\",runtime_tls_required=\"{}\",runtime_tls_current_status=\"{}\",runtime_stream_required=\"{}\",runtime_stream_current_status=\"{}\",runtime_last_reload_attempt_version=\"{}\",runtime_last_good_reload_version=\"{}\",runtime_last_reload_failure_version=\"{}\",runtime_http_last_reload_attempt_version=\"{}\",runtime_http_last_good_reload_version=\"{}\",runtime_http_last_reload_failure_version=\"{}\",runtime_http_last_reload_failure_listener=\"{}\",runtime_tls_last_reload_attempt_version=\"{}\",runtime_tls_last_good_reload_version=\"{}\",runtime_tls_last_reload_failure_version=\"{}\",runtime_tls_last_reload_failure_listener=\"{}\",runtime_stream_last_reload_attempt_version=\"{}\",runtime_stream_last_good_reload_version=\"{}\",runtime_stream_last_reload_failure_version=\"{}\",runtime_stream_last_reload_failure_listener=\"{}\"}} 1",
        prometheus_label(node_id),
        prometheus_label(cluster),
        prometheus_label(&snapshot.id),
        prometheus_label(&xds.last_snapshot_version),
        prometheus_label(&current_snapshot.last_good_snapshot_version),
        prometheus_label(current_snapshot.status),
        prometheus_label(&current_snapshot.rejection_version),
        prometheus_label(&current_snapshot.rejection_runtime),
        http_runtime.required,
        prometheus_label(http_runtime.status),
        tls_runtime.required,
        prometheus_label(tls_runtime.status),
        stream_runtime.required,
        prometheus_label(stream_runtime.status),
        prometheus_label(aggregate_runtime_version([
            runtime.http_last_reload_attempt_version.as_str(),
            runtime.tls_last_reload_attempt_version.as_str(),
            runtime.stream_last_reload_attempt_version.as_str(),
        ].as_slice())),
        prometheus_label(aggregate_runtime_version([
            runtime.http_last_good_reload_version.as_str(),
            runtime.tls_last_good_reload_version.as_str(),
            runtime.stream_last_good_reload_version.as_str(),
        ].as_slice())),
        prometheus_label(aggregate_runtime_version([
            runtime.http_last_reload_failure_version.as_str(),
            runtime.tls_last_reload_failure_version.as_str(),
            runtime.stream_last_reload_failure_version.as_str(),
        ].as_slice())),
        prometheus_label(&runtime.http_last_reload_attempt_version),
        prometheus_label(&runtime.http_last_good_reload_version),
        prometheus_label(&runtime.http_last_reload_failure_version),
        prometheus_label(&runtime.http_last_reload_failure_listener),
        prometheus_label(&runtime.tls_last_reload_attempt_version),
        prometheus_label(&runtime.tls_last_good_reload_version),
        prometheus_label(&runtime.tls_last_reload_failure_version),
        prometheus_label(&runtime.tls_last_reload_failure_listener),
        prometheus_label(&runtime.stream_last_reload_attempt_version),
        prometheus_label(&runtime.stream_last_good_reload_version),
        prometheus_label(&runtime.stream_last_reload_failure_version),
        prometheus_label(&runtime.stream_last_reload_failure_listener),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_xds_last_nack_info",
        "1 if the dataplane has a retained last xDS NACK detail, 0 otherwise.",
        u64::from(!xds.last_nack_version.is_empty()),
    );
}

fn aggregate_runtime_version<'a>(values: &'a [&'a str]) -> &'a str {
    values
        .iter()
        .copied()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}
