use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    sync::OnceLock,
};

use http::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use ntgw_ir::{Filter, RequestMeta};
use ntgw_observability::{
    AccessLogMode, AccessLogOptions, AccessLogTemplateRequirements,
    access_log_template_requirements, resolve_access_log_options,
};
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use pingora::http::ResponseHeader;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::*;

static TRACE_CONTEXT_PROPAGATOR: OnceLock<TraceContextPropagator> = OnceLock::new();

pub(crate) struct RequestView<'a> {
    req: &'a RequestHeader,
    port: u32,
}

impl<'a> RequestView<'a> {
    pub(crate) fn from_header_with_port(req: &'a RequestHeader, port: u32) -> Self {
        Self { req, port }
    }

    pub(crate) fn materialize(&self) -> RequestMeta {
        RequestMeta::with_port(
            self.raw_host().map(ToOwned::to_owned),
            self.port,
            self.path_and_query(),
            self.method(),
            request_headers(self.req),
        )
    }

    pub(crate) fn routing_key(&self) -> RequestMeta {
        RequestMeta::with_port(
            self.raw_host().map(ToOwned::to_owned),
            self.port,
            self.path_and_query(),
            self.method(),
            BTreeMap::new(),
        )
    }

    pub(crate) fn selection_meta(&self, materialize_headers: bool) -> RequestMeta {
        if materialize_headers {
            return self.materialize();
        }

        RequestMeta::with_port(
            self.raw_host().map(ToOwned::to_owned),
            self.port,
            self.path_and_query(),
            self.method(),
            grpc_content_type_headers(self.req),
        )
    }

    pub(crate) fn raw_host(&self) -> Option<&'a str> {
        request_host_value(self.req)
    }

    fn host(&self) -> Option<&'a str> {
        self.raw_host().map(normalize_authority_host_ref)
    }

    pub(crate) fn path(&self) -> &'a str {
        self.req.uri.path()
    }

    fn path_and_query(&self) -> &'a str {
        self.req
            .uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or_else(|| self.req.uri.path())
    }

    pub(crate) fn method(&self) -> &'a str {
        self.req.method.as_str()
    }

    fn request_id(&self) -> &'a str {
        request_id_from_header(self.req)
    }

    pub(crate) fn content_length(&self) -> usize {
        request_content_length_from_header(self.req)
    }

    pub(crate) fn header_bytes(&self) -> usize {
        request_header_bytes_from_header(self.req)
    }
}

pub(crate) fn request_header_bytes_for_limit(
    request: &RequestView<'_>,
    max_request_header_bytes: usize,
) -> usize {
    if max_request_header_bytes == 0 {
        0
    } else {
        request.header_bytes()
    }
}

#[cfg(test)]
pub(crate) fn build_request_meta_from_header(req: &RequestHeader) -> RequestMeta {
    build_request_meta_from_header_with_port(req, 0)
}

pub(crate) fn build_request_meta_from_header_with_port(
    req: &RequestHeader,
    port: u32,
) -> RequestMeta {
    let mut meta = RequestView::from_header_with_port(req, port).materialize();
    meta.source_ip = None;
    meta
}

pub(crate) fn fast_path_request_from_header(
    req: &RequestHeader,
    port: u32,
) -> ntgw_ir::HttpFastPathRequest<'_> {
    let view = RequestView::from_header_with_port(req, port);
    ntgw_ir::HttpFastPathRequest {
        host: view.raw_host(),
        port,
        path: view.path(),
        method: view.method(),
        is_grpc: req
            .headers
            .get_all(CONTENT_TYPE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .any(|value| value.starts_with("application/grpc")),
    }
}

pub(crate) fn build_selection_request_meta_from_header_with_port(
    req: &RequestHeader,
    port: u32,
    source_ip: Option<String>,
    materialize_headers: bool,
) -> RequestMeta {
    let mut meta =
        RequestView::from_header_with_port(req, port).selection_meta(materialize_headers);
    meta.source_ip = source_ip;
    meta
}

pub(crate) fn build_selection_request_meta(
    session: &Session,
    source_ip: Option<String>,
    port: u32,
    materialize_headers: bool,
) -> RequestMeta {
    build_selection_request_meta_from_header_with_port(
        session.req_header(),
        port,
        source_ip,
        materialize_headers,
    )
}

pub(crate) fn build_request_meta(session: &Session) -> RequestMeta {
    let req = session.req_header();
    let mut meta = build_request_meta_from_header_with_port(req, server_port(session));
    meta.source_ip = client_ip(session);
    meta
}

fn request_host_value(req: &RequestHeader) -> Option<&str> {
    req.headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .or_else(|| req.uri.authority().map(|authority| authority.as_str()))
}

pub(crate) fn client_ip(session: &Session) -> Option<String> {
    session
        .client_addr()
        .or_else(|| {
            session
                .as_downstream()
                .digest()
                .and_then(|digest| digest.socket_digest.as_ref())
                .and_then(|socket| socket.peer_addr())
        })
        .map(|addr| {
            addr.as_inet()
                .map(|inet| normalize_ip(inet.ip()))
                .unwrap_or_else(|| addr.to_string())
        })
}

pub(crate) fn normalize_ip(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(ipv4) => ipv4.to_string(),
        IpAddr::V6(ipv6) => ipv6
            .to_ipv4_mapped()
            .map(|ipv4| ipv4.to_string())
            .unwrap_or_else(|| ipv6.to_string()),
    }
}

pub(crate) fn request_headers(req: &RequestHeader) -> BTreeMap<String, Vec<String>> {
    req.headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .fold(BTreeMap::new(), |mut acc, (name, value)| {
            acc.entry(name).or_default().push(value);
            acc
        })
}

fn grpc_content_type_headers(req: &RequestHeader) -> BTreeMap<String, Vec<String>> {
    let values = req
        .headers
        .get_all(CONTENT_TYPE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter(|value| value.starts_with("application/grpc"))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if values.is_empty() {
        BTreeMap::new()
    } else {
        BTreeMap::from([("content-type".to_string(), values)])
    }
}

fn request_header_bytes_from_header(req: &RequestHeader) -> usize {
    req.headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| name.as_str().len().saturating_add(value.len()))
        })
        .sum()
}

pub(crate) fn start_request_span(
    ctx: &mut RequestContext,
    request_headers: &BTreeMap<String, Vec<String>>,
) {
    start_request_span_with_extractor(
        ctx,
        &TraceHeaderExtractor {
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
    start_request_span_with_extractor(ctx, &RequestHeaderExtractor { request });
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

fn start_request_span_with_extractor(ctx: &mut RequestContext, extractor: &dyn Extractor) {
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
    let _ = span.set_parent(trace_context_propagator().extract(extractor));
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

    trace_context_propagator().inject_context(
        &span.context(),
        &mut RequestHeaderInjector {
            request: upstream_request,
        },
    );
}

pub(crate) fn record_request_span(ctx: &RequestContext) {
    let Some(span) = ctx.request_span.as_ref() else {
        return;
    };

    record_span_string(span, "http.request.method", ctx.method.as_str());
    record_span_string(span, "url.path", ctx.path.as_str());
    record_span_string(span, "server.address", ctx.host.as_str());
    record_span_string(span, "client.address", ctx.client_ip.as_str());
    record_span_string(span, "network.protocol.name", effective_http_protocol(ctx));
    record_span_string(span, "gateway.listener", ctx.listener_name.as_str());
    record_span_string(span, "gateway.route.name", ctx.route_name.as_str());
    record_span_string(
        span,
        "gateway.route.namespace",
        ctx.route_namespace.as_str(),
    );
    record_span_string(span, "gateway.route.kind", ctx.route_kind.as_str());
    record_span_string(span, "gateway.backend", ctx.backend.as_str());
    record_span_string(
        span,
        "gateway.snapshot.version",
        ctx.snapshot_version.as_str(),
    );
    record_span_string(span, "gateway.request_id", ctx.request_id.as_str());
    record_span_string(span, "gateway.response_flags", ctx.response_flags.as_str());
    span.record("gateway.retry.attempts", ctx.retry_attempts);
    if ctx.status > 0 {
        span.record("http.response.status_code", u64::from(ctx.status));
    }
}

pub(crate) fn server_port(session: &Session) -> u32 {
    session
        .as_downstream()
        .server_addr()
        .and_then(|addr| addr.as_inet().map(|inet| inet.port() as u32))
        .unwrap_or_default()
}

pub(crate) fn capture_request_context(ctx: &mut RequestContext, request: &RequestMeta) {
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

pub(crate) fn cache_request_headers_if_needed(
    ctx: &mut RequestContext,
    request_headers: &BTreeMap<String, Vec<String>>,
    filters: &[Filter],
) {
    if ctx.request_headers.is_none() && response_filters_need_request_headers(filters) {
        ctx.request_headers = Some(response_filter_request_headers(request_headers));
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
                .insert(header_name.clone(), value.clone());
        }
    }
}

pub(crate) fn cache_access_log_request_headers_from_header_if_needed(
    ctx: &mut RequestContext,
    request: &RequestHeader,
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
                .insert(header_name, value.to_string());
        }
    }
}

pub(crate) fn cache_request_headers_for_filters_and_access_log(
    ctx: &mut RequestContext,
    request_headers: &BTreeMap<String, Vec<String>>,
    filters: &[Filter],
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) {
    cache_request_headers_if_needed(ctx, request_headers, filters);
    cache_access_log_request_headers_if_needed(ctx, request_headers, access_log, route_annotations);
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    ctx.selected_backend
        .as_ref()
        .map(|selected| &selected.route_annotations)
        .unwrap_or(&ctx.route_annotations)
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

#[allow(dead_code)]
fn access_log_response_requirements(
    access_log: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) -> Option<AccessLogTemplateRequirements> {
    let resolved = resolve_access_log_options(access_log, route_annotations);
    if !resolved.enabled || resolved.mode != AccessLogMode::Text {
        return None;
    }

    Some(access_log_template_requirements(&resolved.format))
}

#[allow(dead_code)]
fn cache_access_log_response_headers(
    target: &mut BTreeMap<String, String>,
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
            target.insert(header_name.clone(), value.to_string());
        }
    }
}

pub(crate) fn request_id_from_headers(headers: &BTreeMap<String, Vec<String>>) -> &str {
    [
        "x-request-id",
        "x-correlation-id",
        "traceparent",
        "grpc-trace-bin",
    ]
    .into_iter()
    .find_map(|name| headers.get(name).and_then(|values| values.first()))
    .map(|value| value.as_str())
    .unwrap_or_default()
}

fn request_id_from_header(req: &RequestHeader) -> &str {
    [
        "x-request-id",
        "x-correlation-id",
        "traceparent",
        "grpc-trace-bin",
    ]
    .into_iter()
    .find_map(|name| req.headers.get(name).and_then(|value| value.to_str().ok()))
    .unwrap_or_default()
}

fn request_content_length(headers: &BTreeMap<String, Vec<String>>) -> usize {
    headers
        .get("content-length")
        .and_then(|values| values.first())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default()
}

fn request_content_length_from_header(req: &RequestHeader) -> usize {
    req.headers
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default()
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

fn response_filter_request_headers(
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

fn assign_string(target: &mut String, value: &str) {
    target.clear();
    target.push_str(value);
}

fn trace_context_propagator() -> &'static TraceContextPropagator {
    TRACE_CONTEXT_PROPAGATOR.get_or_init(TraceContextPropagator::new)
}

fn record_span_string(span: &Span, field: &'static str, value: &str) {
    if !value.is_empty() && value != "-" {
        span.record(field, value);
    }
}

fn normalize_authority_host_ref(host: &str) -> &str {
    if let Some(host) = host.strip_prefix('[') {
        return host.split_once(']').map(|(value, _)| value).unwrap_or(host);
    }
    host.split(':').next().unwrap_or(host)
}

struct TraceHeaderExtractor<'a> {
    headers: &'a BTreeMap<String, Vec<String>>,
}

impl Extractor for TraceHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers
            .get(key)
            .or_else(|| self.headers.get(&key.to_ascii_lowercase()))
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(String::as_str).collect()
    }
}

struct RequestHeaderExtractor<'a> {
    request: &'a RequestHeader,
}

impl Extractor for RequestHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.request
            .headers
            .get(key)
            .and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.request
            .headers
            .keys()
            .map(HeaderName::as_str)
            .collect()
    }
}

struct RequestHeaderInjector<'a> {
    request: &'a mut RequestHeader,
}

impl Injector for RequestHeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        let Ok(header_name) = HeaderName::from_bytes(key.as_bytes()) else {
            return;
        };
        let Ok(header_value) = HeaderValue::from_str(&value) else {
            return;
        };
        self.request.headers.insert(header_name, header_value);
    }
}
