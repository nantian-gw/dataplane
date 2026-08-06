use super::*;

pub(crate) const TRANSPORT_RETRY_ENDPOINT_SELECTION_ATTEMPTS: usize = 8;

pub(crate) fn select_request_mirrors_for_selected_backend(
    snapshot: &Snapshot,
    selected: &SelectedBackend,
) -> Vec<SelectedBackend> {
    if !route_filters_have_request_mirror(&selected.filters) {
        return Vec::new();
    }

    snapshot.select_request_mirrors(&RequestMirrorContext {
        route_policy: None,
        route_kind: selected.route_kind,
        route_name: selected.route_name.clone(),
        route_namespace: selected.route_namespace.clone(),
        rule_index: selected.rule_index,
        filters: selected.filters.clone(),
        matched_http_path: selected.matched_http_path.clone(),
        timeouts: selected.timeouts.clone(),
        backend_tls: selected.backend_tls.clone(),
    })
}

pub(crate) fn select_request_mirrors_for_http_route(
    snapshot: &Snapshot,
    route: &SelectedHttpRoute,
) -> Vec<SelectedBackend> {
    if route.backend.is_none()
        || route.backend_name.is_none()
        || !route_filters_have_request_mirror(&route.filters)
    {
        return Vec::new();
    }

    snapshot.select_request_mirrors(&RequestMirrorContext {
        route_policy: None,
        route_kind: RouteKind::Http,
        route_name: route.route_name.clone(),
        route_namespace: route.route_namespace.clone(),
        rule_index: route.rule_index,
        filters: route.filters.clone(),
        matched_http_path: Some(route.matched_http_path.clone()),
        timeouts: route.timeouts.clone(),
        backend_tls: route.backend_tls.clone(),
    })
}

pub(crate) fn select_backend_with_transport_retry_exclusions<F>(
    current: &Snapshot,
    request: &RequestMeta,
    session_resolver: &F,
    ctx: &RequestContext,
) -> Option<SelectedBackend>
where
    F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
{
    if ctx.transport_retry_excluded_endpoints.is_empty() {
        return current.select_backend_with_session_resolver(request, session_resolver);
    }

    let attempts = TRANSPORT_RETRY_ENDPOINT_SELECTION_ATTEMPTS.max(
        ctx.transport_retry_excluded_endpoints
            .len()
            .saturating_add(1),
    );
    let mut last_resort = None;
    for _ in 0..attempts {
        let selected = current.select_backend_with_session_resolver(request, session_resolver)?;
        if !selected_backend_is_transport_retry_excluded(ctx, &selected) {
            return Some(selected);
        }
        if last_resort.is_none() {
            last_resort = Some(selected);
        }
    }

    last_resort
}

pub(crate) fn remove_downstream_close_connection_token(
    upstream_request: &mut RequestHeader,
) -> pingora::Result<()> {
    if !upstream_request
        .headers
        .get_all(http::header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|token| token.eq_ignore_ascii_case("close"))
    {
        return Ok(());
    }

    let retained: Vec<&str> = upstream_request
        .headers
        .get_all(http::header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .filter(|token| !token.eq_ignore_ascii_case("close"))
        .collect();
    let retained = (!retained.is_empty()).then(|| retained.join(", "));
    upstream_request.remove_header(&http::header::CONNECTION);
    if let Some(retained) = retained {
        upstream_request.insert_header(http::header::CONNECTION, retained)?;
    }

    Ok(())
}

pub(crate) fn request_for_response_filters<'a>(
    session: &Session,
    request: &'a RequestMeta,
    full_request: &'a mut Option<RequestMeta>,
    request_headers_complete: bool,
    filters: &[Filter],
) -> &'a RequestMeta {
    if request_headers_complete
        || (!response_filters_need_request_headers(filters) && !cors_filter_present(filters))
    {
        return request;
    }

    full_request.get_or_insert_with(|| build_request_meta_with_headers(session))
}

/// Returns true if the filter list contains a CORS filter that needs access to
/// request headers (Origin, Access-Control-Request-Method, etc.) for preflight checks.
pub(crate) fn cors_filter_present(filters: &[Filter]) -> bool {
    filters.iter().any(|f| f.cors.is_some())
}
