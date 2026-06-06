use super::*;
use std::collections::BTreeSet;

pub(super) struct CandidateListeners<'a> {
    pub(super) listeners: Vec<MatchedListener<'a>>,
    pub(super) enforce_attachments: bool,
}

#[derive(Clone, Copy)]
pub(super) struct MatchedListener<'a> {
    pub(super) listener_index: usize,
    pub(super) listener: &'a Listener,
    pub(super) host_score: HostnameScore,
}

#[derive(Clone, Copy)]
pub(super) enum ListenerKind {
    Http,
    Grpc,
}

pub(super) fn has_service_frontend_http_route_candidate(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    has_service_frontend_route_candidate(
        snapshot,
        request,
        ListenerKind::Http,
        &snapshot.http_routes,
        &snapshot.http_route_hostname_index,
        |route| (&route.namespace, &route.name, &route.parent_refs),
    )
}

pub(super) fn has_service_frontend_grpc_route_candidate(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    has_service_frontend_route_candidate(
        snapshot,
        request,
        ListenerKind::Grpc,
        &snapshot.grpc_routes,
        &snapshot.grpc_route_hostname_index,
        |route| (&route.namespace, &route.name, &route.parent_refs),
    )
}

pub(super) fn matches_service_frontend_http_listener(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    matches_service_frontend_listener(snapshot, request, ListenerKind::Http)
}

pub(super) fn matches_service_frontend_grpc_listener(
    snapshot: &Snapshot,
    request: &RequestMeta,
) -> bool {
    matches_service_frontend_listener(snapshot, request, ListenerKind::Grpc)
}

pub(super) fn https_request_is_misdirected(
    snapshot: &Snapshot,
    request: &RequestMeta,
    server_name: Option<&str>,
) -> bool {
    let Some(server_name) = server_name
        .map(normalize_host_ref)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    if request.port == 0 {
        return false;
    }

    let sni_listeners = best_https_listener_names(snapshot, Some(server_name), request.port);
    if sni_listeners.is_empty() {
        return false;
    }

    let host_listeners = best_https_listener_names(snapshot, request.host.as_deref(), request.port);
    if host_listeners.is_empty() {
        return false;
    }

    sni_listeners.is_disjoint(&host_listeners)
}

pub(super) fn matched_listeners<'a>(
    snapshot: &'a Snapshot,
    request_host: Option<&str>,
    request_port: u32,
    kind: ListenerKind,
) -> CandidateListeners<'a> {
    let mut saw_candidate_listener = false;
    let mut best_score = None;
    let mut listeners = Vec::new();

    visit_candidate_listeners(snapshot, request_port, kind, |listener_index, listener| {
        saw_candidate_listener = true;
        let Some(host_score) = listener_hostname_score(listener, request_host) else {
            return;
        };

        match best_score {
            Some(score) if host_score < score => {}
            Some(score) if host_score == score => {
                listeners.push(MatchedListener {
                    listener_index,
                    listener,
                    host_score,
                });
            }
            _ => {
                best_score = Some(host_score);
                listeners.clear();
                listeners.push(MatchedListener {
                    listener_index,
                    listener,
                    host_score,
                });
            }
        }
    });

    if !saw_candidate_listener {
        return CandidateListeners {
            listeners,
            enforce_attachments: false,
        };
    }

    CandidateListeners {
        listeners,
        enforce_attachments: true,
    }
}

fn best_https_listener_names<'a>(
    snapshot: &'a Snapshot,
    request_host: Option<&str>,
    request_port: u32,
) -> BTreeSet<&'a str> {
    let mut best_score = None;
    let mut listeners = BTreeSet::new();

    visit_candidate_listeners(snapshot, request_port, ListenerKind::Http, |_, listener| {
        if !is_https_listener(&listener.protocol) {
            return;
        }
        let Some(host_score) = listener_hostname_score(listener, request_host) else {
            return;
        };

        match best_score {
            Some(score) if host_score < score => {}
            Some(score) if host_score == score => {
                listeners.insert(listener.name.as_str());
            }
            _ => {
                best_score = Some(host_score);
                listeners.clear();
                listeners.insert(listener.name.as_str());
            }
        }
    });

    listeners
}

fn has_service_frontend_route_candidate<'a, T, F>(
    snapshot: &'a Snapshot,
    request: &RequestMeta,
    listener_kind: ListenerKind,
    routes: &'a [T],
    route_index: &HostnameRouteIndex,
    route_meta: F,
) -> bool
where
    F: Fn(&'a T) -> (&'a str, &'a str, &'a [crate::ParentRef]),
{
    let listeners = matched_listeners(
        snapshot,
        request.host.as_deref(),
        request.port,
        listener_kind,
    );
    if !listeners.enforce_attachments {
        return false;
    }

    let source_namespace = snapshot.source_namespace(request);
    listeners.listeners.iter().any(|listener_match| {
        let listener_frontend = snapshot.service_frontend_for_listener(listener_match.listener);
        listener_frontend.is_some()
            && any_hostname_route_candidate(
                routes,
                route_index,
                request.host.as_deref(),
                snapshot.runtime_indexes_ready,
                |route| {
                    let (route_namespace, route_name, parent_refs) = route_meta(route);
                    route_listener_match(snapshot, &listeners, route_namespace, route_name)
                        .is_some_and(|matched_listener| {
                            matched_listener.listener_index == listener_match.listener_index
                        })
                        && mesh::route_accepts_service_frontend(
                            snapshot,
                            parent_refs,
                            route_namespace,
                            listener_match.listener,
                            source_namespace,
                        )
                },
            )
    })
}

fn matches_service_frontend_listener(
    snapshot: &Snapshot,
    request: &RequestMeta,
    listener_kind: ListenerKind,
) -> bool {
    matched_listeners(
        snapshot,
        request.host.as_deref(),
        request.port,
        listener_kind,
    )
    .listeners
    .iter()
    .any(|listener_match| {
        snapshot
            .service_frontend_for_listener(listener_match.listener)
            .is_some()
    })
}

fn visit_candidate_listeners<'a>(
    snapshot: &'a Snapshot,
    request_port: u32,
    kind: ListenerKind,
    mut visit: impl FnMut(usize, &'a Listener),
) {
    if !snapshot.runtime_indexes_ready {
        for (index, listener) in snapshot.listeners.iter().enumerate() {
            if !matches_listener_kind(listener, kind) {
                continue;
            }
            if request_port != 0 && listener.port != request_port {
                continue;
            }
            visit(index, listener);
        }
        return;
    }

    let indices: &[usize] = match kind {
        ListenerKind::Http => {
            if request_port == 0 {
                &snapshot.http_listener_indices
            } else {
                snapshot
                    .http_listener_port_index
                    .get(&request_port)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
            }
        }
        ListenerKind::Grpc => {
            if request_port == 0 {
                &snapshot.grpc_listener_indices
            } else {
                snapshot
                    .grpc_listener_port_index
                    .get(&request_port)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
            }
        }
    };

    indices
        .iter()
        .filter_map(|index| {
            snapshot
                .listeners
                .get(*index)
                .map(|listener| (*index, listener))
        })
        .for_each(|(index, listener)| visit(index, listener));
}

pub(super) fn visit_http_route_candidates<'a>(
    snapshot: &'a Snapshot,
    request_host: Option<&str>,
    visit: impl FnMut(&'a crate::HttpRoute) -> bool,
) -> bool {
    visit_hostname_route_candidates(
        &snapshot.http_routes,
        &snapshot.http_route_hostname_index,
        request_host,
        snapshot.runtime_indexes_ready,
        visit,
    )
}

pub(super) fn visit_grpc_route_candidates<'a>(
    snapshot: &'a Snapshot,
    request_host: Option<&str>,
    visit: impl FnMut(&'a crate::GrpcRoute) -> bool,
) -> bool {
    visit_hostname_route_candidates(
        &snapshot.grpc_routes,
        &snapshot.grpc_route_hostname_index,
        request_host,
        snapshot.runtime_indexes_ready,
        visit,
    )
}

fn any_hostname_route_candidate<'a, T>(
    routes: &'a [T],
    index: &HostnameRouteIndex,
    request_host: Option<&str>,
    runtime_indexes_ready: bool,
    mut predicate: impl FnMut(&'a T) -> bool,
) -> bool {
    !visit_hostname_route_candidates(
        routes,
        index,
        request_host,
        runtime_indexes_ready,
        |route| !predicate(route),
    )
}

fn visit_hostname_route_candidates<'a, T>(
    routes: &'a [T],
    index: &HostnameRouteIndex,
    request_host: Option<&str>,
    runtime_indexes_ready: bool,
    mut visit: impl FnMut(&'a T) -> bool,
) -> bool {
    visit_hostname_route_candidate_indices(
        routes.len(),
        index,
        request_host,
        runtime_indexes_ready,
        |route_index| {
            let Some(route) = routes.get(route_index) else {
                return true;
            };
            visit(route)
        },
    )
}

fn visit_hostname_route_candidate_indices(
    route_count: usize,
    index: &HostnameRouteIndex,
    request_host: Option<&str>,
    runtime_indexes_ready: bool,
    mut visit: impl FnMut(usize) -> bool,
) -> bool {
    if !runtime_indexes_ready {
        for route_index in 0..route_count {
            if !visit(route_index) {
                return false;
            }
        }
        return true;
    }

    index.visit_candidate_indices(request_host, visit)
}

pub(super) fn route_listener_match<'a>(
    snapshot: &Snapshot,
    listeners: &'a CandidateListeners<'a>,
    route_namespace: &str,
    route_name: &str,
) -> Option<MatchedListener<'a>> {
    if !listeners.enforce_attachments {
        return None;
    }

    if snapshot.runtime_indexes_ready {
        listeners
            .listeners
            .iter()
            .filter(|listener| {
                snapshot.route_attachment_listener_index.contains_listener(
                    route_namespace,
                    route_name,
                    listener.listener_index,
                )
            })
            .copied()
            .max_by(|left, right| left.host_score.cmp(&right.host_score))
    } else {
        listeners
            .listeners
            .iter()
            .filter(|listener| {
                listener
                    .listener
                    .attached_routes
                    .iter()
                    .any(|attached| route_attachment_matches(attached, route_namespace, route_name))
            })
            .copied()
            .max_by(|left, right| left.host_score.cmp(&right.host_score))
    }
}

fn matches_listener_kind(listener: &Listener, kind: ListenerKind) -> bool {
    match kind {
        ListenerKind::Http => is_http_listener(&listener.protocol),
        ListenerKind::Grpc => is_grpc_listener(&listener.protocol),
    }
}

fn route_attachment_matches(attached: &str, route_namespace: &str, route_name: &str) -> bool {
    attached
        .split_once('/')
        .is_some_and(|(namespace, name)| namespace == route_namespace && name == route_name)
}

pub(crate) fn is_http_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTP"
            | "LISTENER_PROTOCOL_HTTPS"
            | "LISTENER_PROTOCOL_HTTP3"
            | "HTTP"
            | "HTTPS"
            | "HTTP3"
    )
}

pub(crate) fn is_grpc_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_GRPC"
            | "LISTENER_PROTOCOL_HTTP"
            | "LISTENER_PROTOCOL_HTTPS"
            | "GRPC"
            | "HTTP"
            | "HTTPS"
    )
}

fn is_https_listener(protocol: &str) -> bool {
    matches!(protocol, "LISTENER_PROTOCOL_HTTPS" | "HTTPS")
}
