use std::sync::Arc;

use axum::{Router, middleware, routing::get};

use super::{
    AppState, auth, backend_detail_view, backends_view, circuit_breaker_view,
    contract::{
        BACKEND_DETAIL_PATH, BACKENDS_PATH, CIRCUIT_BREAKERS_PATH, LISTENER_DETAIL_PATH,
        LISTENER_STATUS_DETAIL_PATH, LISTENER_STATUSES_PATH, LISTENERS_PATH, LIVEZ_PATH,
        METRICS_PATH, NODE_PATH, OVERLOAD_PATH, RATE_LIMITS_PATH, READYZ_PATH, ROUTE_DETAIL_PATH,
        ROUTES_PATH, SNAPSHOT_PATH, SUMMARY_PATH, TRAFFIC_PATH,
    },
    listener_detail_view, listener_status_detail_view, listener_statuses_view, listeners_view,
    livez, metrics_view, node_view, observe_admin_request, overload_view, rate_limit_view, readyz,
    route_detail_view, routes_view, snapshot_view, summary_view, traffic_view,
};

pub(crate) fn build_router(state: AppState) -> Router {
    let state = Arc::new(state);
    let admin_requests = state.admin_requests.clone();
    let public = Router::new()
        .route(LIVEZ_PATH, get(livez))
        .route(READYZ_PATH, get(readyz))
        .route(METRICS_PATH, get(metrics_view))
        .layer(middleware::from_fn_with_state(
            admin_requests.clone(),
            observe_admin_request,
        ))
        .with_state(state.clone());

    let protected = Router::new()
        .route(SUMMARY_PATH, get(summary_view))
        .route(NODE_PATH, get(node_view))
        .route(SNAPSHOT_PATH, get(snapshot_view))
        .route(OVERLOAD_PATH, get(overload_view))
        .route(CIRCUIT_BREAKERS_PATH, get(circuit_breaker_view))
        .route(RATE_LIMITS_PATH, get(rate_limit_view))
        .route(LISTENERS_PATH, get(listeners_view))
        .route(LISTENER_DETAIL_PATH, get(listener_detail_view))
        .route(LISTENER_STATUSES_PATH, get(listener_statuses_view))
        .route(
            LISTENER_STATUS_DETAIL_PATH,
            get(listener_status_detail_view),
        )
        .route(ROUTES_PATH, get(routes_view))
        .route(ROUTE_DETAIL_PATH, get(route_detail_view))
        .route(BACKENDS_PATH, get(backends_view))
        .route(BACKEND_DETAIL_PATH, get(backend_detail_view))
        .route(TRAFFIC_PATH, get(traffic_view))
        .layer(middleware::from_fn_with_state(
            auth::AdminAuth::new(state.config.clone()),
            auth::require_bearer_auth,
        ))
        .layer(middleware::from_fn_with_state(
            admin_requests,
            observe_admin_request,
        ))
        .with_state(state.clone());

    public.merge(protected)
}
