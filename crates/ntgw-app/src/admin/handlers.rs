use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    http::header,
    response::IntoResponse,
};
use serde_json::Value;

use ntgw_ir::Snapshot;
use ntgw_observability::{HttpCircuitBreakerSnapshot, HttpRateLimitSnapshot};

use super::{
    AppState, BackendListQuery, BackendPath, ListenerListQuery, ListenerPath,
    ListenerRuntimeStatus, RouteListQuery, RouteListValueResponse, RoutePath, backend_detail_value,
    backend_list_values, build_listener_runtime_status, build_summary_value,
    collect_listener_runtime_statuses, find_backend, find_listener, find_route,
    listener_detail_value, listener_list_values, render_metrics, route_list_values,
    traffic_view_value,
};
use crate::admin::summary::{build_liveness_state, build_readiness_state};
use crate::admin::types::ApiError;

pub(crate) async fn livez(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = (*state.snapshot.load()).clone();
    let runtime = state.runtime.snapshot();
    let liveness = build_liveness_state(&snapshot, &runtime);

    if liveness.live {
        (StatusCode::OK, liveness.state).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, liveness.reason).into_response()
    }
}

pub(crate) async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = (*state.snapshot.load()).clone();
    let runtime = state.runtime.snapshot();
    let xds = state.xds.snapshot();
    let config = state.current_config();
    let readiness =
        build_readiness_state(&snapshot, &runtime, &xds, config.snapshot_freshness_timeout);

    if readiness.ready {
        (StatusCode::OK, readiness.state).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, readiness.reason).into_response()
    }
}

pub(crate) async fn metrics_view(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        render_metrics(&state),
    )
}

pub(crate) async fn snapshot_view(State(state): State<Arc<AppState>>) -> Json<Snapshot> {
    Json(Snapshot::clone(&state.snapshot.load()))
}

pub(crate) async fn summary_view(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(build_summary_value(&state))
}

pub(crate) async fn node_view(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(build_summary_value(&state))
}

pub(crate) async fn traffic_view(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    Ok(Json(traffic_view_value(
        &snapshot,
        state.traffic.snapshot(),
    )?))
}

pub(crate) async fn overload_view(
    State(state): State<Arc<AppState>>,
) -> Json<ntgw_observability::OverloadSnapshot> {
    Json(state.overload.snapshot())
}

pub(crate) async fn circuit_breaker_view(
    State(state): State<Arc<AppState>>,
) -> Json<HttpCircuitBreakerSnapshot> {
    Json(state.circuit_breaker.read().snapshot())
}

pub(crate) async fn rate_limit_view(
    State(state): State<Arc<AppState>>,
) -> Json<HttpRateLimitSnapshot> {
    Json(state.rate_limit.read().snapshot())
}

pub(crate) async fn listeners_view(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListenerListQuery>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    Ok(Json(listener_list_values(&snapshot, &query)?))
}

pub(crate) async fn listener_detail_view(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ListenerPath>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    let listener = find_listener(&snapshot, path.name.trim())
        .ok_or_else(|| ApiError::not_found("listener not found"))?;
    Ok(Json(listener_detail_value(&snapshot, &listener)?))
}

pub(crate) async fn listener_statuses_view(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListenerListQuery>,
) -> Result<Json<Vec<ListenerRuntimeStatus>>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    let runtime = state.runtime.snapshot();
    Ok(Json(collect_listener_runtime_statuses(
        &snapshot, &runtime, &query,
    )?))
}

pub(crate) async fn listener_status_detail_view(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ListenerPath>,
) -> Result<Json<ListenerRuntimeStatus>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    let runtime = state.runtime.snapshot();
    let listener = find_listener(&snapshot, path.name.trim())
        .ok_or_else(|| ApiError::not_found("listener not found"))?;

    Ok(Json(build_listener_runtime_status(
        &listener, &snapshot, &runtime,
    )))
}

pub(crate) async fn routes_view(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RouteListQuery>,
) -> Result<Json<RouteListValueResponse>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    Ok(Json(route_list_values(&snapshot, &query)?))
}

pub(crate) async fn route_detail_view(
    State(state): State<Arc<AppState>>,
    Path(path): Path<RoutePath>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    let route = find_route(
        &snapshot,
        path.kind.trim(),
        path.namespace.trim(),
        path.name.trim(),
    )?
    .ok_or_else(|| ApiError::not_found("route not found"))?;
    Ok(Json(route))
}

pub(crate) async fn backends_view(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BackendListQuery>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    Ok(Json(backend_list_values(&snapshot, &query)?))
}

pub(crate) async fn backend_detail_view(
    State(state): State<Arc<AppState>>,
    Path(path): Path<BackendPath>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = (*state.snapshot.load()).clone();
    let backend = find_backend(&snapshot, path.namespace.trim(), path.name.trim())
        .ok_or_else(|| ApiError::not_found("backend not found"))?;
    Ok(Json(backend_detail_value(&snapshot, &backend)?))
}
