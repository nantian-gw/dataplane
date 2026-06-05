use crate::{
    default_http_path_match, has_non_backend_http_filter, hostname_matches, is_grpc_request,
    matches_grpc_rule, matches_http_rule, mesh, normalize_host_ref, normalize_http_path_match,
    parse_grpc_path, GrpcPath, GrpcRule, HostnameRouteIndex, HttpBackendResolution, HttpRule,
    Listener, MatchedHttpPath, PersistentSessionTarget, RequestMeta, RouteKind, SelectedBackend,
    SelectedHttpRoute, SessionPersistence, Snapshot,
};

mod candidates;
mod scoring;
mod selection;

use self::candidates::{
    matched_listeners, route_listener_match, visit_grpc_route_candidates,
    visit_http_route_candidates, ListenerKind, MatchedListener,
};
use self::scoring::{
    best_grpc_rule_match, best_hostname_score, best_http_rule_match, listener_hostname_score,
    GrpcCandidateScore, HostnameScore, HttpCandidateScore,
};

pub(super) fn select_http_route(
    snapshot: &Snapshot,
    request: &RequestMeta,
    session_resolver: &impl Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
) -> Option<SelectedHttpRoute> {
    selection::select_http_route(snapshot, request, session_resolver)
}

pub(super) fn select_grpc_backend(
    snapshot: &Snapshot,
    request: &RequestMeta,
    session_resolver: &impl Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
) -> Option<SelectedBackend> {
    selection::select_grpc_backend(snapshot, request, session_resolver)
}

pub(super) fn has_service_frontend_http_route_candidate(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    candidates::has_service_frontend_http_route_candidate(snapshot, request)
}

pub(super) fn has_service_frontend_grpc_route_candidate(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    candidates::has_service_frontend_grpc_route_candidate(snapshot, request)
}

pub(super) fn matches_service_frontend_http_listener(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    candidates::matches_service_frontend_http_listener(snapshot, request)
}

pub(super) fn matches_service_frontend_grpc_listener(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    candidates::matches_service_frontend_grpc_listener(snapshot, request)
}

pub(super) fn https_request_is_misdirected(
    snapshot: &Snapshot,
    request: &RequestMeta,
    server_name: Option<&str>,
) -> bool {
    candidates::https_request_is_misdirected(snapshot, request, server_name)
}

pub(crate) fn is_http_listener(protocol: &str) -> bool {
    candidates::is_http_listener(protocol)
}

pub(crate) fn is_grpc_listener(protocol: &str) -> bool {
    candidates::is_grpc_listener(protocol)
}
