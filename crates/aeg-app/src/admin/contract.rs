#[cfg(test)]
use serde::Deserialize;

pub(super) const LIVEZ_PATH: &str = "/livez";
pub(super) const READYZ_PATH: &str = "/readyz";
pub(super) const METRICS_PATH: &str = "/metrics";
pub(super) const SUMMARY_PATH: &str = "/v1/summary";
pub(super) const NODE_PATH: &str = "/v1/node";
pub(super) const SNAPSHOT_PATH: &str = "/v1/snapshot";
pub(super) const OVERLOAD_PATH: &str = "/v1/overload";
pub(super) const CIRCUIT_BREAKERS_PATH: &str = "/v1/circuit-breakers";
pub(super) const RATE_LIMITS_PATH: &str = "/v1/rate-limits";
pub(super) const LISTENERS_PATH: &str = "/v1/listeners";
pub(super) const LISTENER_DETAIL_PATH: &str = "/v1/listeners/{name}";
pub(super) const LISTENER_STATUSES_PATH: &str = "/v1/listener-statuses";
pub(super) const LISTENER_STATUS_DETAIL_PATH: &str = "/v1/listener-statuses/{name}";
pub(super) const ROUTES_PATH: &str = "/v1/routes";
pub(super) const ROUTE_DETAIL_PATH: &str = "/v1/routes/{kind}/{namespace}/{name}";
pub(super) const BACKENDS_PATH: &str = "/v1/backends";
pub(super) const BACKEND_DETAIL_PATH: &str = "/v1/backends/{namespace}/{name}";
pub(super) const TRAFFIC_PATH: &str = "/v1/traffic";

#[cfg(test)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct AdminRouteContract {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) auth: String,
    #[serde(rename = "contentType")]
    pub(crate) content_type: String,
}

#[cfg(test)]
pub(crate) fn documented_route_contracts() -> Vec<AdminRouteContract> {
    vec![
        route_contract("GET", LIVEZ_PATH, "none", "text/plain"),
        route_contract("GET", READYZ_PATH, "none", "text/plain"),
        route_contract(
            "GET",
            METRICS_PATH,
            "bearer-when-configured",
            "text/plain; version=0.0.4; charset=utf-8",
        ),
        route_contract(
            "GET",
            SUMMARY_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            NODE_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            SNAPSHOT_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            OVERLOAD_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            CIRCUIT_BREAKERS_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            RATE_LIMITS_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            LISTENERS_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            LISTENER_DETAIL_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            LISTENER_STATUSES_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            LISTENER_STATUS_DETAIL_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            ROUTES_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            ROUTE_DETAIL_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            BACKENDS_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            BACKEND_DETAIL_PATH,
            "bearer-when-configured",
            "application/json",
        ),
        route_contract(
            "GET",
            TRAFFIC_PATH,
            "bearer-when-configured",
            "application/json",
        ),
    ]
}

#[cfg(test)]
fn route_contract(method: &str, path: &str, auth: &str, content_type: &str) -> AdminRouteContract {
    AdminRouteContract {
        method: method.to_string(),
        path: path.to_string(),
        auth: auth.to_string(),
        content_type: content_type.to_string(),
    }
}
