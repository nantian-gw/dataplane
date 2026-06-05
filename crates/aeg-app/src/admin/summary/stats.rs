use std::collections::BTreeMap;

use aeg_ir::Snapshot;

use super::runtime::SessionPersistenceUsage;

pub(crate) fn snapshot_session_persistence_usage(snapshot: &Snapshot) -> SessionPersistenceUsage {
    let route_rules = snapshot
        .http_routes
        .iter()
        .map(|route| {
            route
                .rules
                .iter()
                .filter(|rule| rule.session_persistence.is_some())
                .count()
        })
        .sum::<usize>()
        + snapshot
            .grpc_routes
            .iter()
            .map(|route| {
                route
                    .rules
                    .iter()
                    .filter(|rule| rule.session_persistence.is_some())
                    .count()
            })
            .sum::<usize>();
    let backend_policies = snapshot
        .backend_policies
        .values()
        .filter(|policy| policy.session_persistence.is_some())
        .count();

    SessionPersistenceUsage {
        route_rules,
        backend_policies,
    }
}

pub(super) fn traffic_upstream_pool_hit_ratio(traffic: &aeg_observability::TrafficSnapshot) -> f64 {
    let total = traffic
        .total_upstream_pool_hits
        .saturating_add(traffic.total_upstream_pool_misses);
    if total == 0 {
        0.0
    } else {
        traffic.total_upstream_pool_hits as f64 / total as f64
    }
}

pub(super) fn traffic_upstream_connect_latency_avg_ms(
    traffic: &aeg_observability::TrafficSnapshot,
) -> f64 {
    if traffic.total_upstream_connect_latency_observations == 0 {
        0.0
    } else {
        traffic.total_upstream_connect_latency_ms as f64
            / traffic.total_upstream_connect_latency_observations as f64
    }
}

pub(super) fn traffic_retry_rate(traffic: &aeg_observability::TrafficSnapshot) -> f64 {
    if traffic.total_request_events == 0 {
        0.0
    } else {
        traffic.total_retried_events as f64 / traffic.total_request_events as f64
    }
}

pub(super) fn traffic_failover_success_rate(traffic: &aeg_observability::TrafficSnapshot) -> f64 {
    if traffic.total_retried_events == 0 {
        0.0
    } else {
        traffic.total_retried_success_events as f64 / traffic.total_retried_events as f64
    }
}

pub(super) fn named_value_total(values: &BTreeMap<String, u64>) -> u64 {
    values.values().copied().sum()
}
