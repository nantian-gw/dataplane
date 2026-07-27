use std::collections::BTreeMap;

use opentelemetry::propagation::TextMapPropagator;
use pingora::http::RequestHeader;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::context::effective_http_protocol;
use crate::proxy::RequestContext;

pub(crate) fn start_request_span(
    ctx: &mut RequestContext,
    request_headers: &BTreeMap<String, Vec<String>>,
) {
    start_request_span_with_extractor(
        ctx,
        &super::trace::TraceHeaderExtractor {
            headers: request_headers,
        },
    );
}

pub(crate) fn start_request_span_if_enabled(
    ctx: &mut RequestContext,
    request_headers: &BTreeMap<String, Vec<String>>,
    enabled: bool,
) {
    if enabled {
        start_request_span(ctx, request_headers);
    }
}

pub(crate) fn start_request_span_from_header(ctx: &mut RequestContext, request: &RequestHeader) {
    start_request_span_with_extractor(ctx, &super::trace::RequestHeaderExtractor { request });
}

pub(crate) fn start_request_span_from_header_if_enabled(
    ctx: &mut RequestContext,
    request: &RequestHeader,
    enabled: bool,
) {
    if enabled {
        start_request_span_from_header(ctx, request);
    }
}

fn start_request_span_with_extractor(ctx: &mut RequestContext, extractor: &dyn opentelemetry::propagation::Extractor) {
    let span = tracing::info_span!(
        "gateway.request",
        otel.kind = "server",
        "http.request.method" = tracing::field::Empty,
        "url.path" = tracing::field::Empty,
        "server.address" = tracing::field::Empty,
        "client.address" = tracing::field::Empty,
        "network.protocol.name" = tracing::field::Empty,
        "http.response.status_code" = tracing::field::Empty,
        "gateway.listener" = tracing::field::Empty,
        "gateway.route.name" = tracing::field::Empty,
        "gateway.route.namespace" = tracing::field::Empty,
        "gateway.route.kind" = tracing::field::Empty,
        "gateway.backend" = tracing::field::Empty,
        "gateway.snapshot.version" = tracing::field::Empty,
        "gateway.request_id" = tracing::field::Empty,
        "gateway.retry.attempts" = tracing::field::Empty,
        "gateway.response_flags" = tracing::field::Empty,
    );
    let _ = span.set_parent(super::trace::trace_context_propagator().extract(extractor));
    ctx.request_span = Some(span);
    record_request_span(ctx);
}

pub(crate) fn inject_request_span_context(
    ctx: &RequestContext,
    upstream_request: &mut RequestHeader,
) {
    let Some(span) = ctx.request_span.as_ref() else {
        return;
    };

    super::trace::trace_context_propagator().inject_context(
        &span.context(),
        &mut super::trace::RequestHeaderInjector {
            request: upstream_request,
        },
    );
}

pub(crate) fn record_request_span(ctx: &RequestContext) {
    let Some(span) = ctx.request_span.as_ref() else {
        return;
    };

    super::trace::record_span_string(span, "http.request.method", ctx.method.as_str());
    super::trace::record_span_string(span, "url.path", ctx.path.as_str());
    super::trace::record_span_string(span, "server.address", ctx.host.as_str());
    super::trace::record_span_string(span, "client.address", ctx.client_ip.as_str());
    super::trace::record_span_string(span, "network.protocol.name", effective_http_protocol(ctx));
    super::trace::record_span_string(span, "gateway.listener", ctx.listener_name.as_str());
    super::trace::record_span_string(span, "gateway.route.name", ctx.route_name.as_str());
    super::trace::record_span_string(
        span,
        "gateway.route.namespace",
        ctx.route_namespace.as_str(),
    );
    super::trace::record_span_string(span, "gateway.route.kind", ctx.route_kind.as_str());
    super::trace::record_span_string(span, "gateway.backend", ctx.backend.as_str());
    super::trace::record_span_string(
        span,
        "gateway.snapshot.version",
        ctx.snapshot_version.as_str(),
    );
    super::trace::record_span_string(span, "gateway.request_id", ctx.request_id.as_str());
    super::trace::record_span_string(span, "gateway.response_flags", ctx.response_flags.as_str());
    span.record("gateway.retry.attempts", ctx.retry_attempts);
    if ctx.status > 0 {
        span.record("http.response.status_code", u64::from(ctx.status));
    }
}

pub(crate) fn server_port(session: &pingora::prelude::Session) -> u32 {
    session
        .as_downstream()
        .server_addr()
        .and_then(|addr| addr.as_inet().map(|inet| inet.port() as u32))
        .unwrap_or_default()
}
