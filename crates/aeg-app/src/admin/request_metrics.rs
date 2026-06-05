use std::time::Instant;

use aeg_observability::SharedAdminRequestStats;
use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};

pub(crate) async fn observe_admin_request(
    State(stats): State<SharedAdminRequestStats>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = normalize_admin_method(request.method());
    let route = classify_admin_route(request.uri().path());
    let started = Instant::now();
    let response = next.run(request).await;
    let status_class = response_status_class(response.status());
    stats.observe(method, route, status_class, started.elapsed());
    response
}

fn normalize_admin_method(method: &Method) -> &'static str {
    match *method {
        Method::CONNECT => "CONNECT",
        Method::DELETE => "DELETE",
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        Method::PATCH => "PATCH",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::TRACE => "TRACE",
        _ => "OTHER",
    }
}

fn response_status_class(status: StatusCode) -> &'static str {
    match status.as_u16() {
        500..=u16::MAX => "5xx",
        400..=499 => "4xx",
        300..=399 => "3xx",
        200..=299 => "2xx",
        _ => "1xx",
    }
}

fn classify_admin_route(path: &str) -> &'static str {
    match path {
        "/livez" => "livez",
        "/readyz" => "readyz",
        "/metrics" => "metrics",
        "/v1/summary" => "summary",
        "/v1/node" => "node",
        "/v1/snapshot" => "snapshot",
        "/v1/overload" => "overload",
        "/v1/circuit-breakers" => "circuit_breakers",
        "/v1/rate-limits" => "rate_limits",
        "/v1/listeners" => "listeners",
        "/v1/listener-statuses" => "listener_statuses",
        "/v1/routes" => "routes",
        "/v1/backends" => "backends",
        "/v1/traffic" => "traffic",
        _ if path.starts_with("/v1/listeners/") => "listener_detail",
        _ if path.starts_with("/v1/listener-statuses/") => "listener_status_detail",
        _ if path.starts_with("/v1/routes/") => "route_detail",
        _ if path.starts_with("/v1/backends/") => "backend_detail",
        _ => "unknown",
    }
}
