mod auth;
mod contract;
mod filters;
mod handlers;
mod listener_status;
mod metrics;
mod request_metrics;
mod router;
mod summary;
mod traffic;
mod types;

use std::sync::{Arc, RwLock};

use ntgw_observability::{
    HttpCircuitBreakerController, HttpRateLimitController, RetryBudgetController,
    SharedAdminRequestStats, SharedOverloadStats, SharedRuntimeStats, SharedTrafficStats,
    SharedUdpSessionStats,
};
use ntgw_xds::SharedClientStats;

#[cfg(test)]
use self::contract::{AdminRouteContract, documented_route_contracts};
pub(crate) use self::router::build_router;
use self::{
    filters::{
        backend_detail_value, backend_list_values, filter_listeners, find_backend, find_listener,
        find_route, listener_detail_value, listener_list_values, route_list_values,
    },
    handlers::{
        backend_detail_view, backends_view, circuit_breaker_view, listener_detail_view,
        listener_status_detail_view, listener_statuses_view, listeners_view, livez, metrics_view,
        node_view, overload_view, rate_limit_view, readyz, route_detail_view, routes_view,
        snapshot_view, summary_view, traffic_view,
    },
    listener_status::collect_listener_runtime_statuses,
    metrics::render_metrics,
    request_metrics::observe_admin_request,
    summary::{build_listener_runtime_status, build_summary_value},
    traffic::traffic_view_value,
    types::{
        BackendListQuery, BackendPath, ListenerListQuery, ListenerPath, ListenerRuntimeStatus,
        RouteListQuery, RouteListValueResponse, RoutePath,
    },
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(crate) struct AdminRuntimeConfig {
    pub(crate) admin_bearer_token: Option<String>,
    pub(crate) admin_bearer_token_file: Option<String>,
    pub(crate) cluster: String,
    pub(crate) http3_configured: bool,
    pub(crate) node_id: String,
    pub(crate) session_persistence_uses_ephemeral_secret: bool,
    pub(crate) snapshot_freshness_timeout: std::time::Duration,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<RwLock<AdminRuntimeConfig>>,
    pub(crate) snapshot: ntgw_ir::SharedSnapshot,
    pub(crate) runtime: SharedRuntimeStats,
    pub(crate) traffic: SharedTrafficStats,
    pub(crate) udp_sessions: SharedUdpSessionStats,
    pub(crate) admin_requests: SharedAdminRequestStats,
    pub(crate) xds: SharedClientStats,
    pub(crate) overload: SharedOverloadStats,
    pub(crate) circuit_breaker: Arc<RwLock<HttpCircuitBreakerController>>,
    pub(crate) rate_limit: Arc<RwLock<HttpRateLimitController>>,
    pub(crate) retry_budget: Arc<RwLock<RetryBudgetController>>,
}

impl AppState {
    pub(crate) fn current_config(&self) -> AdminRuntimeConfig {
        self.config
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}
