use super::*;

struct HttpCandidate<'a> {
    route: &'a crate::HttpRoute,
    rule_index: usize,
    rule: &'a HttpRule,
    listener_match: Option<MatchedListener<'a>>,
    matched_http_path: MatchedHttpPath,
    resolution: HttpBackendResolution,
    score: HttpCandidateScore,
}

struct GrpcCandidate<'a> {
    route: &'a crate::GrpcRoute,
    rule_index: usize,
    rule: &'a GrpcRule,
    listener_match: Option<MatchedListener<'a>>,
    selected_backend: crate::ResolvedHttpBackend,
    score: GrpcCandidateScore,
}

#[tracing::instrument(skip(snapshot, request, session_resolver))]
pub(super) fn select_http_route(
    snapshot: &Snapshot,
    request: &RequestMeta,
    session_resolver: &impl Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
) -> Option<SelectedHttpRoute> {
    let listeners = matched_listeners(
        snapshot,
        request.host.as_deref(),
        request.port,
        ListenerKind::Http,
    );
    if listeners.enforce_attachments && listeners.listeners.is_empty() {
        return None;
    }

    let mut best: Option<HttpCandidate<'_>> = None;
    visit_http_route_candidates(snapshot, request.host.as_deref(), |route| {
        let listener_match =
            route_listener_match(snapshot, &listeners, &route.namespace, &route.name);
        if listeners.enforce_attachments && listener_match.is_none() {
            return true;
        }
        if let Some(listener_match) = listener_match
            && !mesh::route_accepts_service_frontend(
                snapshot,
                &route.parent_refs,
                &route.namespace,
                listener_match.listener,
            )
        {
            return true;
        }
        let Some(route_host_score) = best_hostname_score(&route.hostnames, request.host.as_deref())
        else {
            return true;
        };

        for (rule_index, rule) in route.rules.iter().enumerate() {
            let Some((matched_http_path, rule_score)) = best_http_rule_match(rule, request) else {
                continue;
            };
            let resolution = snapshot.resolve_http_backend_refs_with_session(
                &rule.backend_refs,
                rule.session_persistence.as_ref(),
                request,
                session_resolver,
            );
            if resolution.selected.is_none()
                && resolution.error.is_none()
                && !has_non_backend_http_filter(&rule.filters)
            {
                continue;
            }

            let score = HttpCandidateScore {
                listener_host: listener_match
                    .map(|item| item.host_score)
                    .unwrap_or_default(),
                route_host: route_host_score,
                rule: rule_score,
            };
            if best.as_ref().is_some_and(|current| score <= current.score) {
                continue;
            }

            let candidate = HttpCandidate {
                route,
                rule_index,
                rule,
                listener_match,
                matched_http_path,
                resolution,
                score,
            };

            best = Some(candidate);
        }
        true
    });

    best.map(|candidate| {
        tracing::trace!(
            route_name = %candidate.route.name,
            route_namespace = %candidate.route.namespace,
            "http route matched"
        );
        let mut filters = candidate.rule.filters.clone();
        if let Some(selected_backend) = candidate.resolution.selected.as_ref() {
            filters.extend(selected_backend.filters.clone());
        }
        let backend = candidate
            .resolution
            .selected
            .as_ref()
            .map(|item| item.endpoint.clone());
        let backend_name = candidate
            .resolution
            .selected
            .as_ref()
            .map(|item| item.backend_name.clone());
        let session_persistence = candidate.rule.session_persistence.clone().or_else(|| {
            candidate
                .resolution
                .selected
                .as_ref()
                .and_then(|item| item.session_persistence.clone())
        });
        let security_policy = backend_name
            .as_deref()
            .and_then(|name| snapshot.backend_index.get(name))
            .and_then(|index| snapshot.backends.get(*index))
            .and_then(|backend| backend.security_policy.clone());

        SelectedHttpRoute {
            route_name: candidate.route.name.clone(),
            route_namespace: candidate.route.namespace.clone(),
            rule_index: Some(candidate.rule_index),
            route_annotations: candidate.route.annotations.clone(),
            listener_name: candidate
                .listener_match
                .map(|item| item.listener.name.clone())
                .unwrap_or_default(),
            listener_protocol: candidate
                .listener_match
                .map(|item| item.listener.protocol.clone())
                .unwrap_or_default(),
            filters,
            matched_http_path: candidate.matched_http_path,
            backend,
            backend_name,
            backend_error: candidate.resolution.error,
            timeouts: candidate.rule.timeouts.clone(),
            retry: candidate.rule.retry.clone(),
            session_persistence,
            backend_tls: candidate
                .listener_match
                .and_then(|item| item.listener.backend_tls.clone()),
            route_policy: None,
            security_policy,
        }
    })
}

pub(super) fn select_grpc_backend(
    snapshot: &Snapshot,
    request: &RequestMeta,
    session_resolver: &impl Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
) -> Option<SelectedBackend> {
    if !is_grpc_request(request) {
        return None;
    }

    let grpc = parse_grpc_path(&request.path);
    let listeners = matched_listeners(
        snapshot,
        request.host.as_deref(),
        request.port,
        ListenerKind::Grpc,
    );
    if listeners.enforce_attachments && listeners.listeners.is_empty() {
        return None;
    }

    let mut best: Option<GrpcCandidate<'_>> = None;
    visit_grpc_route_candidates(snapshot, request.host.as_deref(), |route| {
        let listener_match =
            route_listener_match(snapshot, &listeners, &route.namespace, &route.name);
        if listeners.enforce_attachments && listener_match.is_none() {
            return true;
        }
        if let Some(listener_match) = listener_match
            && !mesh::route_accepts_service_frontend(
                snapshot,
                &route.parent_refs,
                &route.namespace,
                listener_match.listener,
            )
        {
            return true;
        }
        let Some(route_host_score) = best_hostname_score(&route.hostnames, request.host.as_deref())
        else {
            return true;
        };

        for (rule_index, rule) in route.rules.iter().enumerate() {
            let Some(rule_score) = best_grpc_rule_match(rule, request, grpc.as_ref()) else {
                continue;
            };
            let resolution = snapshot.resolve_http_backend_refs_with_session(
                &rule.backend_refs,
                rule.session_persistence.as_ref(),
                request,
                session_resolver,
            );
            let Some(selected_backend) = resolution.selected else {
                continue;
            };

            let score = GrpcCandidateScore {
                listener_host: listener_match
                    .map(|item| item.host_score)
                    .unwrap_or_default(),
                route_host: route_host_score,
                rule: rule_score,
            };
            if best.as_ref().is_some_and(|current| score <= current.score) {
                continue;
            }

            let candidate = GrpcCandidate {
                route,
                rule_index,
                rule,
                listener_match,
                selected_backend,
                score,
            };

            best = Some(candidate);
        }
        true
    });

    best.map(|candidate| {
        let mut filters = candidate.rule.filters.clone();
        filters.extend(candidate.selected_backend.filters.clone());
        let session_persistence = candidate
            .rule
            .session_persistence
            .clone()
            .or_else(|| candidate.selected_backend.session_persistence.clone());

        SelectedBackend {
            route_policy: None,
            route_kind: RouteKind::Grpc,
            route_name: candidate.route.name.clone(),
            route_namespace: candidate.route.namespace.clone(),
            rule_index: Some(candidate.rule_index),
            route_annotations: candidate.route.annotations.clone(),
            listener_name: candidate
                .listener_match
                .map(|item| item.listener.name.clone())
                .unwrap_or_default(),
            listener_protocol: candidate
                .listener_match
                .map(|item| item.listener.protocol.clone())
                .unwrap_or_default(),
            backend: candidate.selected_backend.endpoint,
            backend_name: candidate.selected_backend.backend_name,
            filters,
            matched_http_path: None,
            timeouts: None,
            retry: None,
            session_persistence,
            backend_tls: candidate
                .listener_match
                .and_then(|item| item.listener.backend_tls.clone()),
        }
    })
}
