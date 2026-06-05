pub(super) fn traffic_upstream_pool_hit_ratio(
    traffic: &aeg_observability::TrafficSnapshot,
) -> Option<f64> {
    let total = traffic
        .total_upstream_pool_hits
        .saturating_add(traffic.total_upstream_pool_misses);
    if total == 0 {
        None
    } else {
        Some(traffic.total_upstream_pool_hits as f64 / total as f64)
    }
}

pub(super) fn traffic_upstream_connect_latency_avg_ms(
    traffic: &aeg_observability::TrafficSnapshot,
) -> Option<f64> {
    if traffic.total_upstream_connect_latency_observations == 0 {
        None
    } else {
        Some(
            traffic.total_upstream_connect_latency_ms as f64
                / traffic.total_upstream_connect_latency_observations as f64,
        )
    }
}

pub(super) fn traffic_retry_rate(traffic: &aeg_observability::TrafficSnapshot) -> Option<f64> {
    if traffic.total_request_events == 0 {
        None
    } else {
        Some(traffic.total_retried_events as f64 / traffic.total_request_events as f64)
    }
}

pub(super) fn traffic_failover_success_rate(
    traffic: &aeg_observability::TrafficSnapshot,
) -> Option<f64> {
    if traffic.total_retried_events == 0 {
        None
    } else {
        Some(traffic.total_retried_success_events as f64 / traffic.total_retried_events as f64)
    }
}
