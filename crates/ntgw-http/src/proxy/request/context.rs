use std::collections::BTreeMap;

use ntgw_ir::Filter;

use super::extract::{request_content_length, request_id_from_headers};
use super::tracing::record_request_span;
use super::view::RequestView;
use crate::proxy::RequestContext;

pub(crate) fn capture_request_context(ctx: &mut RequestContext, request: &ntgw_ir::RequestMeta) {
    assign_string(
        &mut ctx.client_ip,
        request.source_ip.as_deref().unwrap_or("-"),
    );
    assign_string(&mut ctx.host, request.host.as_deref().unwrap_or("-"));
    assign_string(&mut ctx.method, request.method.as_str());
    assign_string(&mut ctx.path, request.path.as_str());
    assign_string(
        &mut ctx.request_id,
        request_id_from_headers(&request.headers),
    );
    ctx.bytes_received = 0;
    ctx.declared_request_body_bytes = request_content_length(&request.headers);
    record_request_span(ctx);
}

pub(crate) fn capture_request_context_from_view(
    ctx: &mut RequestContext,
    request: &RequestView<'_>,
    source_ip: Option<&str>,
) {
    capture_request_context_from_view_for_features(ctx, request, source_ip, true);
}

pub(crate) fn capture_request_context_from_view_for_features(
    ctx: &mut RequestContext,
    request: &RequestView<'_>,
    source_ip: Option<&str>,
    capture_observability_fields: bool,
) {
    capture_request_context_from_view_for_limits(
        ctx,
        request,
        source_ip,
        capture_observability_fields,
        true,
    );
}

pub(crate) fn capture_request_context_from_view_for_limits(
    ctx: &mut RequestContext,
    request: &RequestView<'_>,
    source_ip: Option<&str>,
    capture_observability_fields: bool,
    capture_declared_request_body_bytes: bool,
) {
    if capture_observability_fields {
        assign_string(&mut ctx.client_ip, source_ip.unwrap_or("-"));
        assign_string(&mut ctx.host, request.host().unwrap_or("-"));
        assign_string(&mut ctx.path, request.path());
        assign_string(&mut ctx.request_id, request.request_id());
    } else {
        ctx.client_ip.clear();
        ctx.host.clear();
        ctx.path.clear();
        ctx.request_id.clear();
    }
    assign_string(&mut ctx.method, request.method());
    ctx.bytes_received = 0;
    ctx.declared_request_body_bytes = if capture_declared_request_body_bytes {
        request.content_length()
    } else {
        0
    };
    record_request_span(ctx);
}

pub(crate) fn effective_http_protocol(ctx: &RequestContext) -> &str {
    if !ctx.listener_protocol.is_empty() {
        return ctx.listener_protocol.as_str();
    }

    if ctx.route_kind.eq_ignore_ascii_case("grpc") {
        "GRPC"
    } else {
        "HTTP"
    }
}

pub(crate) fn response_filters_need_request_headers(filters: &[Filter]) -> bool {
    filters.iter().any(|filter| filter.filter_type == "CORS")
}

pub(crate) fn response_filter_request_headers(
    request_headers: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    const CORS_REQUEST_HEADER_NAMES: [&str; 6] = [
        "origin",
        "access-control-request-method",
        "access-control-request-headers",
        "cookie",
        "authorization",
        "proxy-authorization",
    ];

    let mut filtered = BTreeMap::new();
    for name in CORS_REQUEST_HEADER_NAMES {
        if let Some(values) = request_headers.get(name) {
            filtered.insert(name.to_string(), values.clone());
        }
    }
    filtered
}

pub(crate) fn assign_string(target: &mut String, value: &str) {
    target.clear();
    target.push_str(value);
}
