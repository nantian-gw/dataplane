use super::super::{
    context::MetricsContext,
    prometheus::{
        append_counter, append_gauge, append_histogram, append_labeled_counter_map,
        append_labeled_counter_values, append_labeled_gauge_map,
    },
};

pub(super) fn append_protection_metrics(out: &mut String, ctx: &MetricsContext) {
    let overload = &ctx.overload;
    let circuit_breaker = &ctx.circuit_breaker;
    let rate_limit = &ctx.rate_limit;
    let retry_budget = &ctx.retry_budget;
    let endpoint_runtime = ctx.snapshot.endpoint_runtime_snapshot();

    append_labeled_counter_values(
        out,
        "aether_gateway_dataplane_http_overload_rejected_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by overload admission control.",
        "scope",
        &[
            ("total", overload.http_rejected_total),
            ("global", overload.http_rejected_global_total),
            ("listener", overload.http_rejected_listener_total),
            ("route", overload.http_rejected_route_total),
        ],
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_http_overload_rejected_listener_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by each listener inflight budget.",
        "listener",
        &overload.http_rejected_listener_by_name,
        &ctx.http_listener_metric_labels,
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_http_overload_rejected_route_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by each route inflight budget.",
        "route",
        &overload.http_rejected_route_by_name,
        &ctx.route_metric_labels,
    );
    append_labeled_counter_values(
        out,
        "aether_gateway_dataplane_tcp_overload_rejected_total",
        "Total number of TCP and TLS passthrough sessions rejected by overload admission control.",
        "scope",
        &[
            ("total", overload.tcp_rejected_total),
            ("global", overload.tcp_rejected_global_total),
            ("listener", overload.tcp_rejected_listener_total),
        ],
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_tcp_overload_rejected_listener_total",
        "Total number of TCP and TLS passthrough sessions rejected by each listener connection budget.",
        "listener",
        &overload.tcp_rejected_listener_by_name,
        &ctx.tcp_listener_metric_labels,
    );
    append_labeled_counter_values(
        out,
        "aether_gateway_dataplane_udp_overload_rejected_total",
        "Total number of UDP datagrams rejected by overload admission control.",
        "scope",
        &[
            ("total", overload.udp_rejected_total),
            ("global", overload.udp_rejected_global_total),
            ("listener", overload.udp_rejected_listener_total),
        ],
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_udp_overload_rejected_listener_total",
        "Total number of UDP datagrams rejected by each listener datagram budget.",
        "listener",
        &overload.udp_rejected_listener_by_name,
        &ctx.udp_listener_metric_labels,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_circuit_breaker_backend_max_inflight_requests",
        "Configured maximum concurrent in-flight requests allowed per backend cluster by the HTTP circuit breaker.",
        circuit_breaker.backend_max_inflight_requests as u64,
    );
    append_labeled_gauge_map(
        out,
        "aether_gateway_dataplane_http_circuit_breaker_backend_inflight_current",
        "Current number of in-flight requests held against each backend cluster by the HTTP circuit breaker.",
        "backend",
        &circuit_breaker.backend_inflight_current,
        &ctx.backend_metric_labels,
    );
    append_labeled_counter_values(
        out,
        "aether_gateway_dataplane_http_circuit_breaker_rejected_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by the HTTP circuit breaker.",
        "scope",
        &[
            ("total", circuit_breaker.rejected_total),
            ("backend", circuit_breaker.rejected_backend_total),
        ],
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_http_circuit_breaker_rejected_backend_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected for each backend cluster by the HTTP circuit breaker.",
        "backend",
        &circuit_breaker.rejected_backend_by_name,
        &ctx.backend_metric_labels,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_global_enabled",
        "1 if the global HTTP rate limit scope is enabled, 0 otherwise.",
        u64::from(rate_limit.global.enabled),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_global_requests_per_second",
        "Configured global HTTP request rate limit in requests per second across HTTP, HTTPS, and gRPC traffic.",
        u64::from(rate_limit.global.requests_per_second),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_global_burst",
        "Configured global HTTP request rate limit burst across HTTP, HTTPS, and gRPC traffic.",
        u64::from(rate_limit.global.burst),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_global_available_tokens",
        "Current available global HTTP rate limit tokens across HTTP, HTTPS, and gRPC traffic.",
        rate_limit.global.available_tokens,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_listener_enabled",
        "1 if the per-listener HTTP rate limit scope is enabled, 0 otherwise.",
        u64::from(rate_limit.listener.enabled),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_listener_requests_per_second",
        "Configured per-listener HTTP request rate limit in requests per second across HTTP, HTTPS, and gRPC traffic.",
        u64::from(rate_limit.listener.requests_per_second),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_listener_burst",
        "Configured per-listener HTTP request rate limit burst across HTTP, HTTPS, and gRPC traffic.",
        u64::from(rate_limit.listener.burst),
    );
    append_labeled_gauge_map(
        out,
        "aether_gateway_dataplane_http_rate_limit_listener_available_tokens",
        "Current available HTTP rate limit tokens for each listener.",
        "listener",
        &rate_limit.listener.available_tokens_by_name,
        &ctx.http_listener_metric_labels,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_route_enabled",
        "1 if the per-route HTTP rate limit scope is enabled, 0 otherwise.",
        u64::from(rate_limit.route.enabled),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_route_requests_per_second",
        "Configured per-route HTTP request rate limit in requests per second across HTTP, HTTPS, and gRPC traffic.",
        u64::from(rate_limit.route.requests_per_second),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_rate_limit_route_burst",
        "Configured per-route HTTP request rate limit burst across HTTP, HTTPS, and gRPC traffic.",
        u64::from(rate_limit.route.burst),
    );
    append_labeled_gauge_map(
        out,
        "aether_gateway_dataplane_http_rate_limit_route_available_tokens",
        "Current available HTTP rate limit tokens for each route.",
        "route",
        &rate_limit.route.available_tokens_by_name,
        &ctx.route_metric_labels,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_http_rate_limit_allowed_total",
        "Total number of HTTP, HTTPS, and gRPC requests admitted after HTTP rate limiting.",
        rate_limit.allowed_total,
    );
    append_labeled_counter_values(
        out,
        "aether_gateway_dataplane_http_rate_limit_rejected_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by HTTP rate limiting.",
        "scope",
        &[
            ("total", rate_limit.rejected_total),
            ("global", rate_limit.rejected_global_total),
            ("listener", rate_limit.rejected_listener_total),
            ("route", rate_limit.rejected_route_total),
        ],
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_http_rate_limit_rejected_listener_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by each listener HTTP rate limit bucket.",
        "listener",
        &rate_limit.rejected_listener_by_name,
        &ctx.http_listener_metric_labels,
    );
    append_labeled_counter_map(
        out,
        "aether_gateway_dataplane_http_rate_limit_rejected_route_total",
        "Total number of HTTP, HTTPS, and gRPC requests rejected by each route HTTP rate limit bucket.",
        "route",
        &rate_limit.rejected_route_by_name,
        &ctx.route_metric_labels,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_retry_budget_enabled",
        "1 if the HTTP retry budget is enabled, 0 otherwise.",
        u64::from(retry_budget.enabled),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_retry_budget_ratio_percent",
        "Configured HTTP retry budget refill ratio as a percentage of retryable request volume.",
        u64::from(retry_budget.ratio_percent),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_retry_budget_burst",
        "Configured HTTP retry budget burst capacity in full retry tokens.",
        u64::from(retry_budget.burst),
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_retry_budget_available_tokens",
        "Current available HTTP retry budget tokens rounded down to full retry attempts.",
        retry_budget.available_tokens,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_http_retry_budget_available_milli_tokens",
        "Current available HTTP retry budget tokens in milli-token units.",
        retry_budget.available_milli_tokens,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_http_retry_budget_retryable_requests_observed_total",
        "Total number of HTTP, HTTPS, and gRPC requests observed as retryable by the retry budget.",
        retry_budget.retryable_requests_observed_total,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_http_retry_budget_allowed_total",
        "Total number of HTTP, HTTPS, and gRPC retry attempts allowed by the retry budget.",
        retry_budget.retry_allowed_total,
    );
    append_counter(
        out,
        "aether_gateway_dataplane_http_retry_budget_rejected_total",
        "Total number of HTTP, HTTPS, and gRPC retry attempts rejected because the retry budget was exhausted.",
        retry_budget.retry_rejected_total,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_endpoint_runtime_tracked_current",
        "Current number of endpoints with non-default runtime health state.",
        endpoint_runtime.tracked_endpoints as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_endpoint_passive_ejected_current",
        "Current number of endpoints temporarily removed by passive failure ejection.",
        endpoint_runtime.passive_ejected_current as u64,
    );
    append_gauge(
        out,
        "aether_gateway_dataplane_endpoint_active_unhealthy_current",
        "Current number of endpoints marked unhealthy by active health probing.",
        endpoint_runtime.active_unhealthy_current as u64,
    );
    append_histogram(
        out,
        "aether_gateway_dataplane_endpoint_recovery_latency_ms",
        "Bucketed latency in milliseconds from endpoint ejection or active unhealthy marking to observed recovery.",
        endpoint_runtime
            .recovery_latency_ms_buckets
            .iter()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        endpoint_runtime.recovery_latency_ms_sum,
        endpoint_runtime.recovery_latency_ms_count,
    );
}
