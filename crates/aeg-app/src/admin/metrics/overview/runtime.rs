use super::super::{
    context::MetricsContext,
    prometheus::{append_counter, append_counter_f64, append_gauge},
};

pub(super) fn append_runtime_metrics(out: &mut String, ctx: &MetricsContext) {
    let runtime = &ctx.runtime;
    let current_snapshot = &ctx.current_snapshot;
    let http_runtime = &ctx.http_runtime;
    let tls_runtime = &ctx.tls_runtime;
    let stream_runtime = &ctx.stream_runtime;

    append_counter(
        out,
        "aether_gateway_dataplane_runtime_http_listener_reload_failures_total",
        "Total number of HTTP listener reload attempts that failed before the listener could start.",
        runtime.http_listener_reload_failures,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_runtime_http_tls_asset_reuses_total",
        "Total number of HTTP TLS asset materializations that reused an existing certificate bundle on disk.",
        runtime.http_tls_asset_reuses,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_runtime_tls_listener_reload_failures_total",
        "Total number of shared TLS listener reload attempts that failed before the listener could start.",
        runtime.tls_listener_reload_failures,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_runtime_stream_listener_reload_failures_total",
        "Total number of stream listener reload attempts that failed before the listener could start.",
        runtime.stream_listener_reload_failures,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_current_snapshot_rejected",
        "1 if the current snapshot version is rejected by the dataplane runtime, 0 otherwise.",
        u64::from(current_snapshot.rejected),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_serving_last_good_snapshot",
        "1 if the dataplane is serving a retained last-good snapshot after the current snapshot was rejected, 0 otherwise.",
        u64::from(current_snapshot.serving_last_good_snapshot),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_runtime_http_current_rejected",
        "1 if the HTTP runtime has rejected the current snapshot version, 0 otherwise.",
        u64::from(http_runtime.rejected),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_runtime_tls_current_rejected",
        "1 if the shared TLS runtime has rejected the current snapshot version, 0 otherwise.",
        u64::from(tls_runtime.rejected),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_runtime_stream_current_rejected",
        "1 if the stream runtime has rejected the current snapshot version, 0 otherwise.",
        u64::from(stream_runtime.rejected),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_runtime_http_current_failure_count",
        "Number of HTTP listeners currently failing for the active snapshot version.",
        runtime.http_current_failures.len() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_runtime_tls_current_failure_count",
        "Number of shared TLS listeners currently failing for the active snapshot version.",
        runtime.tls_current_failures.len() as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_runtime_stream_current_failure_count",
        "Number of stream listeners currently failing for the active snapshot version.",
        runtime.stream_current_failures.len() as u64,
    );
    append_process_metrics(out, ctx);
}

fn append_process_metrics(out: &mut String, ctx: &MetricsContext) {
    let process = &ctx.process;

    if let Some(value) = process.cpu_seconds_total {
        append_counter_f64(
            out,
            "process_cpu_seconds_total",
            "Total user and system CPU time spent by this dataplane process in seconds.",
            value,
        );
    }
    if let Some(value) = process.resident_memory_bytes {
        append_gauge(
            out,
            "process_resident_memory_bytes",
            "Resident memory size of this dataplane process in bytes.",
            value,
        );
    }
    if let Some(value) = process.virtual_memory_bytes {
        append_gauge(
            out,
            "process_virtual_memory_bytes",
            "Virtual memory size of this dataplane process in bytes.",
            value,
        );
    }
    if let Some(value) = process.open_fds {
        append_gauge(
            out,
            "process_open_fds",
            "Number of open file descriptors for this dataplane process.",
            value,
        );
    }
    if let Some(value) = process.max_fds {
        append_gauge(
            out,
            "process_max_fds",
            "Maximum number of open file descriptors for this dataplane process.",
            value,
        );
    }
    if let Some(value) = process.threads {
        append_gauge(
            out,
            "process_threads",
            "Number of OS threads in this dataplane process.",
            value,
        );
    }
}
