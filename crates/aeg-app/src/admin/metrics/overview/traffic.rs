use super::super::{
    context::MetricsContext,
    prometheus::{
        append_counter, append_gauge, append_gauge_f64, append_histogram,
        append_labeled_counter_map, append_labeled_gauge_map, prometheus_label,
    },
};
use std::fmt::Write as _;

const KNOWN_RESPONSE_FLAGS: &[&str] = &[
    "none", "CB", "DC", "IB", "IT", "MA", "NR", "OL", "RB", "RH", "RL", "UC", "UF", "UH", "UT",
];

pub(super) fn append_traffic_metrics(out: &mut String, ctx: &MetricsContext) {
    let traffic = &ctx.traffic;

    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_events_total",
        "Total number of observed downstream requests, sessions, and datagrams.",
        traffic.total_events,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_request_events_total",
        "Total number of observed request-like HTTP/gRPC traffic events, including HTTP, HTTPS, GRPC, GRPCS, H2C, HTTP2, and HTTP/2.",
        traffic.total_request_events,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_bytes_received_total",
        "Total downstream request body, session payload, and datagram bytes received across observed traffic.",
        traffic.total_bytes_received,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_bytes_sent_total",
        "Total downstream response body, session payload, and datagram bytes sent across observed traffic.",
        traffic.total_bytes_sent,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_latency_ms_total",
        "Total observed downstream latency in milliseconds across all traffic events.",
        traffic.total_latency_ms,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_traffic_latency_ms_max",
        "Maximum observed downstream latency in milliseconds.",
        traffic.max_latency_ms,
    );
    append_request_latency_histograms(out, ctx);
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_retried_events_total",
        "Total number of request-like HTTP/gRPC traffic events that required at least one retry.",
        traffic.total_retried_events,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_retry_attempts_total",
        "Total number of retry attempts spent serving request-like HTTP/gRPC traffic.",
        traffic.total_retry_attempts,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_retried_success_events_total",
        "Total number of retried request-like HTTP/gRPC traffic events that completed without a 5xx or proxy failure.",
        traffic.total_retried_success_events,
    );
    append_optional_gauge_f64(
        out,
        "nantian_gateway_dataplane_traffic_retry_rate",
        "Ratio of observed request-like HTTP/gRPC traffic events that required at least one retry.",
        ctx.retry_rate,
    );
    append_optional_gauge_f64(
        out,
        "nantian_gateway_dataplane_traffic_failover_success_rate",
        "Ratio of retried downstream traffic events that completed without a 5xx or proxy failure.",
        ctx.failover_success_rate,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_upstream_pool_hits_total",
        "Total number of request-like HTTP/gRPC upstream acquisition attempts that reused an existing pooled connection or stream.",
        traffic.total_upstream_pool_hits,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_upstream_pool_misses_total",
        "Total number of request-like HTTP/gRPC upstream acquisition attempts that required establishing a new upstream connection.",
        traffic.total_upstream_pool_misses,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_upstream_peer_build_failures_total",
        "Total number of request-like HTTP/gRPC upstream peer build failures before connection establishment.",
        traffic.total_upstream_peer_build_failures,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failures_total",
        "Total number of request-like HTTP/gRPC upstream TLS handshake failures during connection establishment.",
        traffic.total_upstream_tls_handshake_failures,
    );
    append_optional_gauge_f64(
        out,
        "nantian_gateway_dataplane_traffic_upstream_pool_hit_ratio",
        "Ratio of request-like HTTP/gRPC upstream acquisition attempts served from the existing upstream pool.",
        ctx.upstream_pool_hit_ratio,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_total",
        "Total upstream connection establishment latency in milliseconds across request-like HTTP/gRPC new upstream connections.",
        traffic.total_upstream_connect_latency_ms,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_max",
        "Maximum observed request-like HTTP/gRPC upstream connection establishment latency in milliseconds.",
        traffic.max_upstream_connect_latency_ms,
    );
    append_optional_gauge_f64(
        out,
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms_average",
        "Average upstream connection establishment latency in milliseconds across request-like HTTP/gRPC new upstream connections.",
        ctx.upstream_connect_latency_avg_ms,
    );
    append_histogram(
        out,
        "nantian_gateway_dataplane_traffic_upstream_connect_latency_ms",
        "Bucketed upstream connection establishment latency in milliseconds across observed request-like HTTP/gRPC new upstream connections.",
        traffic
            .upstream_connect_latency_ms_buckets
            .iter()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        traffic.total_upstream_connect_latency_ms,
        traffic.total_upstream_connect_latency_observations,
    );
    append_histogram(
        out,
        "nantian_gateway_dataplane_traffic_upstream_tls_handshake_failure_latency_ms",
        "Bucketed request-like HTTP/gRPC latency in milliseconds before upstream TLS handshake failures are observed.",
        traffic
            .upstream_tls_handshake_failure_latency_ms_buckets
            .iter()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        traffic.total_upstream_tls_handshake_failure_latency_ms,
        traffic.total_upstream_tls_handshake_failure_latency_observations,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_status_1xx_total",
        "Total number of observed request-like HTTP/gRPC traffic events that completed with a 1xx status.",
        traffic.status_1xx,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_status_2xx_total",
        "Total number of observed request-like HTTP/gRPC traffic events that completed with a 2xx status.",
        traffic.status_2xx,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_status_3xx_total",
        "Total number of observed request-like HTTP/gRPC traffic events that completed with a 3xx status.",
        traffic.status_3xx,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_status_4xx_total",
        "Total number of observed request-like HTTP/gRPC traffic events that completed with a 4xx status.",
        traffic.status_4xx,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_status_5xx_total",
        "Total number of observed request-like HTTP/gRPC traffic events that completed with a 5xx status.",
        traffic.status_5xx,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_traffic_status_other_total",
        "Total number of observed request-like HTTP/gRPC traffic events without an HTTP status or outside the standard 1xx-5xx ranges.",
        traffic.status_other,
    );
    append_labeled_counter_map(
        out,
        "nantian_gateway_dataplane_traffic_response_flags_total",
        "Total number of observed request-like HTTP/gRPC traffic events by response flag.",
        "flag",
        &traffic.response_flags,
        KNOWN_RESPONSE_FLAGS,
    );

    let udp = &ctx.udp_sessions;
    append_gauge(
        out,
        "nantian_gateway_dataplane_udp_sessions_active_current",
        "Current number of active UDP upstream sessions.",
        udp.active_sessions_current,
    );
    append_labeled_gauge_map(
        out,
        "nantian_gateway_dataplane_udp_sessions_active_listener_current",
        "Current number of active UDP upstream sessions by listener.",
        "listener",
        &udp.active_sessions_by_listener,
        &ctx.udp_listener_metric_labels,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_udp_session_queue_depth_current",
        "Current number of queued UDP datagrams waiting for per-session upstream processing.",
        udp.queue_depth_current,
    );
    append_labeled_gauge_map(
        out,
        "nantian_gateway_dataplane_udp_session_queue_depth_listener_current",
        "Current number of queued UDP datagrams by listener.",
        "listener",
        &udp.queue_depth_by_listener,
        &ctx.udp_listener_metric_labels,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_udp_session_queue_overflow_dropped_total",
        "Total number of UDP datagrams dropped because a per-session queue was full.",
        udp.queue_overflow_dropped_total,
    );
    append_labeled_counter_map(
        out,
        "nantian_gateway_dataplane_udp_session_queue_overflow_dropped_listener_total",
        "Total number of UDP datagrams dropped by listener because a per-session queue was full.",
        "listener",
        &udp.queue_overflow_dropped_by_listener,
        &ctx.udp_listener_metric_labels,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_udp_session_idle_evictions_total",
        "Total number of UDP upstream sessions evicted after the session idle timeout.",
        udp.idle_evictions_total,
    );
    append_labeled_counter_map(
        out,
        "nantian_gateway_dataplane_udp_session_idle_evictions_listener_total",
        "Total number of UDP upstream sessions evicted by listener after the session idle timeout.",
        "listener",
        &udp.idle_evictions_by_listener,
        &ctx.udp_listener_metric_labels,
    );

    let access_log = &ctx.access_log_writers;
    append_gauge(
        out,
        "nantian_gateway_dataplane_access_log_writer_count",
        "Current number of active access log writer workers.",
        access_log.writers,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_access_log_writer_queue_depth",
        "Current number of queued or in-flight access log lines across all writer workers.",
        access_log.queue_depth,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_access_log_writer_dropped_lines_total",
        "Total number of access log lines dropped because writer queues were full.",
        access_log.dropped_lines_total,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_access_log_writer_flushes_total",
        "Total number of access log writer flush operations.",
        access_log.flushes_total,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_access_log_writer_flush_latency_ms_total",
        "Total access log writer flush latency in milliseconds.",
        access_log.flush_latency_ms_total,
    );
    append_gauge(
        out,
        "nantian_gateway_dataplane_access_log_writer_flush_latency_ms_max",
        "Maximum observed access log writer flush latency in milliseconds.",
        access_log.flush_latency_ms_max,
    );
    append_counter(
        out,
        "nantian_gateway_dataplane_access_log_writer_sink_errors_total",
        "Total number of access log sink write or flush errors.",
        access_log.sink_errors_total,
    );
}

fn append_request_latency_histograms(out: &mut String, ctx: &MetricsContext) {
    if ctx.traffic.request_latency_ms_histograms.is_empty() {
        return;
    }

    let name = "nantian_gateway_dataplane_traffic_request_latency_ms";
    let _ = writeln!(
        out,
        "# HELP {name} Cumulative Prometheus histogram of downstream request latency in milliseconds using low-cardinality traffic labels. Calculate p95/p99 with histogram_quantile() over rate({name}_bucket[window]) for a time-windowed view."
    );
    let _ = writeln!(out, "# TYPE {name} histogram");
    for histogram in &ctx.traffic.request_latency_ms_histograms {
        let labels = request_latency_labels(histogram);
        for bucket in &histogram.buckets {
            let _ = writeln!(
                out,
                "{name}_bucket{{{labels},le=\"{}\"}} {}",
                prometheus_label(&bucket.le),
                bucket.cumulative_count
            );
        }
        let _ = writeln!(out, "{name}_sum{{{labels}}} {}", histogram.sum);
        let _ = writeln!(out, "{name}_count{{{labels}}} {}", histogram.count);
    }
}

fn append_optional_gauge_f64(out: &mut String, name: &str, help: &str, value: Option<f64>) {
    if let Some(value) = value {
        append_gauge_f64(out, name, help, value);
    }
}

fn request_latency_labels(histogram: &aeg_observability::TrafficLabeledHistogram) -> String {
    format!(
        "listener=\"{}\",protocol=\"{}\",route_kind=\"{}\",status_class=\"{}\",response_flag=\"{}\"",
        prometheus_label(&histogram.listener),
        prometheus_label(&histogram.protocol),
        prometheus_label(&histogram.route_kind),
        prometheus_label(&histogram.status_class),
        prometheus_label(&histogram.response_flag),
    )
}
