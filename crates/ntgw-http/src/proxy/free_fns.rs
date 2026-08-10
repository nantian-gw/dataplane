use std::sync::Arc;
use std::time::{Duration, SystemTime};

use http::Version;
use pingora::{Error, ErrorType};
use pingora::proxy::Session;
use ntgw_ir::{PersistentSessionTarget, SelectedHttpRoute, SessionPersistence, Snapshot};

use super::{
    GatewayProxy, RequestContext,
};
use super::context::{
    assign_ctx_string, cache_http_route_context, HttpRouteContextFields, SelectedBackendConfig,
};
use super::request::{record_request_span, RequestView};
use super::selection::{selected_backend_config_cached, SelectedBackendConfigCache};
use ntgw_observability::AccessLogOptions;

pub(crate) fn https_request_is_misdirected_in_snapshot(
    current: &Snapshot,
    request_view: &RequestView<'_>,
    downstream_tls_server_name: Option<&str>,
) -> bool {
    let Some(server_name) = downstream_tls_server_name else {
        return false;
    };
    let request_key = request_view.routing_key();
    current.https_request_is_misdirected(&request_key, Some(server_name))
}

pub(crate) fn cache_snapshot_version_if_observed(
    ctx: &mut RequestContext,
    snapshot_id: &str,
    access_log_enabled: bool,
    request_tracing_enabled: bool,
) {
    if access_log_enabled || request_tracing_enabled {
        assign_ctx_string(&mut ctx.snapshot_version, snapshot_id);
    }
}

pub(crate) fn missing_frontend_client_certificate_error(ctx: &mut RequestContext) -> Box<Error> {
    ctx.status = 499;
    assign_ctx_string(&mut ctx.response_flags, "DC");
    record_request_span(ctx);
    Error::new_down(ErrorType::ConnectionClosed)
        .more_context("strict frontend client certificate validation requires a client certificate")
}

pub(crate) fn cache_selected_http_route_context(
    ctx: &mut RequestContext,
    access_log: &AccessLogOptions,
    route: &SelectedHttpRoute,
) {
    cache_http_route_context(
        ctx,
        HttpRouteContextFields {
            route_name: route.route_name.as_str(),
            route_namespace: route.route_namespace.as_str(),
            route_annotations: &route.route_annotations,
            listener_name: route.listener_name.as_str(),
            listener_protocol: route.listener_protocol.as_str(),
            backend_name: route.backend_name.as_deref(),
        },
        access_log,
    );
    ctx.route_policy = route.route_policy.clone();
}

pub(crate) fn mark_downstream_max_connection_age_if_needed(
    session: &mut Session,
    ctx: &mut RequestContext,
    max_connection_age: Option<Duration>,
) {
    let Some(max_connection_age) = max_connection_age else {
        return;
    };
    if session.req_header().version == Version::HTTP_2 {
        return;
    }
    let Some(connection_age) = downstream_connection_age(session) else {
        return;
    };
    if connection_age < max_connection_age {
        return;
    }

    session.as_downstream_mut().set_keepalive(None);
    if ctx.response_flags.is_empty() {
        assign_ctx_string(&mut ctx.response_flags, "MA");
        record_request_span(ctx);
    }
}

pub(crate) fn downstream_connection_age(session: &Session) -> Option<Duration> {
    let now = SystemTime::now();
    session
        .as_downstream()
        .digest()?
        .timing_digest
        .iter()
        .flatten()
        .filter_map(|timing| now.duration_since(timing.established_ts).ok())
        .max()
}

pub(crate) fn select_backend_after_http_route_miss<F>(
    cache: &SelectedBackendConfigCache,
    current: &Snapshot,
    request: &ntgw_ir::RequestMeta,
    session_resolver: &F,
) -> pingora::Result<Option<(super::SelectedBackend, Arc<SelectedBackendConfig>)>>
where
    F: Fn(&SessionPersistence) -> Option<PersistentSessionTarget>,
{
    let Some(selected) = current.select_backend_with_session_resolver(request, session_resolver)
    else {
        return Ok(None);
    };
    let config = selected_backend_config_cached(cache, current, &selected)?;
    Ok(Some((selected, config)))
}

pub(crate) fn cache_response_body_limit_exceeded(current_len: usize, chunk_len: usize, limit: usize) -> bool {
    limit > 0 && current_len.saturating_add(chunk_len) > limit
}