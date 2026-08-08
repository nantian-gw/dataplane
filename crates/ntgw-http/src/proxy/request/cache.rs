use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ntgw_observability::{
    AccessLogMode, AccessLogOptions, AccessLogTemplateRequirements,
    access_log_template_requirements, resolve_access_log_options,
};
use pingora::http::ResponseHeader;
use pingora::prelude::Session;

use super::extract::request_headers;
use crate::proxy::RequestContext;

pub(crate) fn cache_request_headers_if_needed(ctx: &mut RequestContext, session: &Session) {
    if ctx.request_headers.is_none() {
        let headers = request_headers(session.req_header());
        ctx.request_headers = Some(headers);
    }
}

pub(crate) fn cache_access_log_request_headers_if_needed(
    ctx: &mut RequestContext,
    request_headers: &BTreeMap<String, Vec<String>>,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    let Some(required_headers) =
        access_log_request_header_requirements(access_log, route_annotations)
    else {
        return;
    };

    for header_name in required_headers {
        if let Some(value) = request_headers
            .get(&header_name)
            .and_then(|values| values.first())
        {
            ctx.access_log_request_headers
                .insert(Arc::from(header_name.as_str()), value.clone());
        }
    }
}

pub(crate) fn cache_access_log_request_headers_from_header_if_needed(
    ctx: &mut RequestContext,
    request: &pingora::http::RequestHeader,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    let Some(required_headers) =
        access_log_request_header_requirements(access_log, route_annotations)
    else {
        return;
    };

    for header_name in required_headers {
        if let Some(value) = request
            .headers
            .get(header_name.as_str())
            .and_then(|value| value.to_str().ok())
        {
            ctx.access_log_request_headers
                .insert(Arc::from(header_name), value.to_string());
        }
    }
}

pub(crate) fn cache_request_headers_for_filters_and_access_log(
    ctx: &mut RequestContext,
    session: &Session,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    cache_request_headers_if_needed(ctx, session);
    let headers = request_headers(session.req_header());
    cache_access_log_request_headers_if_needed(ctx, &headers, access_log, route_annotations);
}

pub(crate) fn cache_access_log_connection_fields_if_needed(
    session: &Session,
    ctx: &mut RequestContext,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    cache_access_log_connection_fields_from_sources_if_needed(
        ctx,
        access_log,
        route_annotations,
        session
            .as_downstream()
            .digest()
            .and_then(|digest| digest.ssl_digest.as_ref())
            .is_some(),
        session
            .client_addr()
            .and_then(|addr| addr.as_inet().map(|inet| inet.port())),
        session
            .as_downstream()
            .digest()
            .and_then(|digest| digest.socket_digest.as_ref())
            .and_then(|socket| socket.peer_addr())
            .and_then(|addr| addr.as_inet().map(|inet| inet.port())),
    );
}

pub(crate) fn cache_access_log_connection_fields_from_sources_if_needed(
    ctx: &mut RequestContext,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
    downstream_tls_present: bool,
    client_remote_port: Option<u16>,
    digest_peer_remote_port: Option<u16>,
) {
    let Some(requirements) = access_log_response_requirements(access_log, route_annotations) else {
        return;
    };

    if requirements.needs_scheme {
        ctx.access_log_scheme = if downstream_tls_present {
            "https".to_string()
        } else {
            "http".to_string()
        };
    }

    if requirements.needs_remote_port {
        ctx.access_log_remote_port = client_remote_port.or(digest_peer_remote_port);
    }
}

pub(crate) fn cache_access_log_sent_response_headers_if_needed(
    ctx: &mut RequestContext,
    response: &ResponseHeader,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    let Some(requirements) = access_log_response_requirements(access_log, route_annotations) else {
        return;
    };
    if requirements.sent_response_headers.is_empty() {
        return;
    }

    cache_access_log_response_headers(
        &mut ctx.access_log_sent_response_headers,
        response,
        &requirements.sent_response_headers,
    );
}

#[allow(dead_code)]
pub(crate) fn cache_access_log_sent_response_headers_from_written_response_if_needed(
    ctx: &mut RequestContext,
    written_response: Option<&ResponseHeader>,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    let Some(requirements) = access_log_response_requirements(access_log, route_annotations) else {
        return;
    };
    if requirements.sent_response_headers.is_empty() {
        return;
    }
    let Some(response) = written_response else {
        return;
    };

    cache_access_log_response_headers(
        &mut ctx.access_log_sent_response_headers,
        response,
        &requirements.sent_response_headers,
    );
}

#[allow(dead_code)]
pub(crate) fn cache_access_log_upstream_response_headers_if_needed(
    ctx: &mut RequestContext,
    response: &ResponseHeader,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    let Some(requirements) = access_log_response_requirements(access_log, route_annotations) else {
        return;
    };
    if requirements.upstream_response_headers.is_empty() {
        return;
    }

    cache_access_log_response_headers(
        &mut ctx.access_log_upstream_response_headers,
        response,
        &requirements.upstream_response_headers,
    );
}

#[allow(dead_code)]
pub(crate) fn record_access_log_upstream_status_if_needed(
    ctx: &mut RequestContext,
    status: u16,
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    let Some(requirements) = access_log_response_requirements(access_log, route_annotations) else {
        return;
    };
    if requirements.needs_upstream_status {
        ctx.access_log_upstream_statuses.push(status);
    }
}

pub(crate) fn access_log_route_annotations(ctx: &RequestContext) -> &BTreeMap<String, String> {
    if let Some(selected) = ctx.selected_backend.as_ref() {
        return &selected.route_annotations;
    }

    if let Some(selected) = ctx
        .fast_selected_backend
        .as_ref()
        .map(|state| &state.selected)
    {
        return &selected.route_annotations;
    }

    &ctx.route_annotations
}

fn access_log_request_header_requirements(
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) -> Option<BTreeSet<String>> {
    let resolved = resolve_access_log_options(access_log, route_annotations);
    if !resolved.enabled || resolved.mode != AccessLogMode::Text {
        return None;
    }

    let requirements = access_log_template_requirements(&resolved.format);
    (!requirements.request_headers.is_empty()).then_some(requirements.request_headers)
}

pub(crate) fn access_log_response_requirements(
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) -> Option<AccessLogTemplateRequirements> {
    let resolved = resolve_access_log_options(access_log, route_annotations);
    if !resolved.enabled || resolved.mode != AccessLogMode::Text {
        return None;
    }

    Some(access_log_template_requirements(&resolved.format))
}

pub(crate) fn cache_access_log_response_headers(
    target: &mut BTreeMap<Arc<str>, String>,
    response: &ResponseHeader,
    required_headers: &BTreeSet<String>,
) {
    target.clear();

    for header_name in required_headers {
        if let Some(value) = response
            .headers
            .get(header_name.as_str())
            .and_then(|value| value.to_str().ok())
        {
            target.insert(Arc::from(header_name.as_str()), value.to_string());
        }
    }
}
