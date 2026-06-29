use std::{
    collections::BTreeMap,
    fmt::{self},
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use pingora::utils::tls::CertKey;
use pingora_cache::HttpCache;
use tracing::Span;

use super::GatewayProxy;
use super::cache::CachedBackendTlsValidation;
use crate::mirror::MirrorRequestSession;
use crate::session::ResolvedSession;
use ntgw_ai::filter::AIContext;
use ntgw_ir::{
    BackendEndpoint, EndpointRuntimeHandle, RouteKind, SelectedBackend, SelectedBackendRuntimeIds,
    SharedSnapshot, Snapshot,
};
use ntgw_observability::{
    AccessLogOptions, HttpAdmissionPermit, HttpCircuitBreakerPermit, SharedTrafficStats,
    TrafficTopology, UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT, epoch_millis,
    upstream_connect_latency_ms_bucket_index,
};

use super::request::record_request_span;

#[derive(Debug, Clone)]
pub(crate) struct FastSelectedBackendState {
    pub(crate) selected: ntgw_ir::CompiledSelectedHttpBackend,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct TransportRetryExcludedEndpoint {
    backend_name: String,
    address: String,
    port: u32,
}

impl TransportRetryExcludedEndpoint {
    fn new(backend_name: &str, endpoint: &BackendEndpoint) -> Self {
        Self {
            backend_name: backend_name.to_string(),
            address: endpoint.address.clone(),
            port: endpoint.port,
        }
    }

    fn matches_selected(&self, selected: &SelectedBackend) -> bool {
        self.backend_name == selected.backend_name
            && self.address == selected.backend.address
            && self.port == selected.backend.port
    }
}

#[derive(Clone)]
pub(crate) struct SelectedBackendConfig {
    pub(crate) runtime: EndpointRuntimeHandle,
    pub(crate) runtime_ids: SelectedBackendRuntimeIds,
    pub(crate) peer_address: UpstreamPeerAddress,
    pub(crate) peer_port: u16,
    pub(crate) tls_enabled: bool,
    pub(crate) sni: String,
    pub(crate) use_http2: bool,
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) request_timeout: Option<Duration>,
    pub(crate) backend_tls_validation: Option<CachedBackendTlsValidation>,
    pub(crate) client_cert_key: Option<Arc<CertKey>>,
    pub(crate) traffic_topology: TrafficTopology,
}

impl fmt::Debug for SelectedBackendConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedBackendConfig")
            .field("runtime", &self.runtime)
            .field("runtime_ids", &self.runtime_ids)
            .field("peer_address", &self.peer_address)
            .field("peer_port", &self.peer_port)
            .field("tls_enabled", &self.tls_enabled)
            .field("sni", &self.sni)
            .field("use_http2", &self.use_http2)
            .field("connect_timeout", &self.connect_timeout)
            .field("request_timeout", &self.request_timeout)
            .field(
                "backend_tls_validation_group_key",
                &self
                    .backend_tls_validation
                    .as_ref()
                    .map(|validation| validation.group_key),
            )
            .field(
                "client_cert_key_configured",
                &self.client_cert_key.is_some(),
            )
            .field("traffic_topology", &self.traffic_topology)
            .finish()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum UpstreamPeerAddress {
    Ip(IpAddr),
    Host(String),
}

impl UpstreamPeerAddress {
    pub(crate) fn from_backend_address(address: &str) -> Self {
        address
            .parse::<IpAddr>()
            .map(Self::Ip)
            .unwrap_or_else(|_| Self::Host(address.to_string()))
    }
}

#[derive(Debug, Default)]
pub struct RequestContext {
    pub started_at: Option<Instant>,
    pub started_at_unix_ms: u128,
    pub(crate) upstream_connect_started_at: Option<Instant>,
    pub(crate) request_span: Option<Span>,
    pub snapshot_version: String,
    pub(crate) cached_snapshot: Option<Arc<Snapshot>>,
    pub client_ip: String,
    pub host: String,
    pub method: String,
    pub path: String,
    pub request_id: String,
    pub request_headers: Option<BTreeMap<String, Vec<String>>>,
    pub status: u16,
    pub listener_name: String,
    pub listener_protocol: String,
    pub route_name: String,
    pub route_namespace: String,
    pub backend: String,
    pub route_kind: String,
    pub route_annotations: BTreeMap<String, String>,
    pub response_flags: String,
    pub bytes_received: usize,
    pub(crate) declared_request_body_bytes: usize,
    pub(crate) runtime_ids: SelectedBackendRuntimeIds,
    pub selected_backend: Option<Arc<SelectedBackend>>,
    pub(crate) fast_selected_backend: Option<FastSelectedBackendState>,
    pub(crate) selected_backend_config: Option<Arc<SelectedBackendConfig>>,
    pub(crate) external_auth_response_headers: BTreeMap<String, Vec<String>>,
    pub(crate) local_response_traffic_topology: Option<Arc<TrafficTopology>>,
    pub(crate) transport_retry_excluded_endpoints: Vec<TransportRetryExcludedEndpoint>,
    pub(crate) retry_backoff: Option<Duration>,
    pub(crate) retry_attempts: u32,
    pub(crate) upstream_pool_hits: u32,
    pub(crate) upstream_pool_misses: u32,
    pub(crate) upstream_peer_build_failures: u32,
    pub(crate) upstream_connect_latency_ms: u64,
    pub(crate) upstream_connect_latency_ms_max: u64,
    pub(crate) upstream_connect_latency_ms_buckets: [u32; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
    pub(crate) resolved_session: Option<ResolvedSession>,
    pub(crate) request_mirrors: Vec<MirrorRequestSession>,
    pub(crate) is_mirror_subrequest: bool,
    pub(crate) admission_permit: Option<HttpAdmissionPermit>,
    pub(crate) circuit_breaker_permit: Option<HttpCircuitBreakerPermit>,
    pub(crate) rate_limit_applied: bool,
    pub(crate) retry_budget_seeded: bool,
    pub(crate) backend_observation_recorded: bool,
    pub(crate) request_body_bytes_seen: usize,
    pub(crate) route_policy: Option<ntgw_config::RoutePolicyConfig>,
    pub(crate) access_log_request_headers: BTreeMap<String, String>,
    pub(crate) access_log_sent_response_headers: BTreeMap<String, String>,
    pub(crate) access_log_upstream_response_headers: BTreeMap<String, String>,
    pub(crate) access_log_upstream_statuses: Vec<u16>,
    pub(crate) access_log_scheme: String,
    pub(crate) access_log_remote_port: Option<u16>,
    pub http_version: String,
    pub query_string: String,
    pub upstream_addr: String,
    pub response_content_type: String,
    pub connection_id: String,
    #[allow(dead_code)]
    pub(crate) http_cache: CacheState,
    pub(crate) cached_response_body: Vec<Bytes>,
    pub(crate) cached_response_body_bytes: usize,
    pub(crate) ai_context: Option<AIContext>,
}

#[derive(Default)]
pub(crate) struct CacheState(pub(crate) Option<HttpCache>);

impl CacheState {
    pub(crate) fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

impl std::ops::Deref for CacheState {
    type Target = Option<HttpCache>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for CacheState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::fmt::Debug for CacheState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_some() {
            f.write_str("HttpCache(active)")
        } else {
            f.write_str("None")
        }
    }
}

impl RequestContext {
    pub(crate) fn cached_snapshot(&mut self, proxy: &GatewayProxy) -> Arc<Snapshot> {
        self.cached_snapshot
            .get_or_insert_with(|| Arc::clone(&proxy.snapshot.load()))
            .clone()
    }
}

pub(crate) fn record_upstream_connection(ctx: &mut RequestContext, reused: bool) {
    if reused {
        ctx.upstream_pool_hits = ctx.upstream_pool_hits.saturating_add(1);
        ctx.upstream_connect_started_at = None;
        record_request_span(ctx);
        return;
    }

    ctx.upstream_pool_misses = ctx.upstream_pool_misses.saturating_add(1);
    if let Some(started_at) = ctx.upstream_connect_started_at.take() {
        let latency_ms = saturating_latency_ms(started_at.elapsed().as_millis());
        ctx.upstream_connect_latency_ms =
            ctx.upstream_connect_latency_ms.saturating_add(latency_ms);
        ctx.upstream_connect_latency_ms_max = ctx.upstream_connect_latency_ms_max.max(latency_ms);
        let bucket_index = upstream_connect_latency_ms_bucket_index(latency_ms);
        ctx.upstream_connect_latency_ms_buckets[bucket_index] =
            ctx.upstream_connect_latency_ms_buckets[bucket_index].saturating_add(1);
    }
    record_request_span(ctx);
}

pub(crate) fn record_upstream_peer_build_failure(ctx: &mut RequestContext) {
    ctx.upstream_peer_build_failures = ctx.upstream_peer_build_failures.saturating_add(1);
    record_request_span(ctx);
}

pub(crate) fn record_upstream_tls_handshake_failure(
    traffic: &SharedTrafficStats,
    ctx: &RequestContext,
    error: &pingora::Error,
) {
    if !is_upstream_tls_handshake_failure(error) {
        return;
    }

    let latency_ms = ctx
        .upstream_connect_started_at
        .as_ref()
        .map(|started_at| saturating_latency_ms(started_at.elapsed().as_millis()));
    traffic.observe_upstream_tls_handshake_failure(latency_ms);
}

fn is_upstream_tls_handshake_failure(error: &pingora::Error) -> bool {
    matches!(
        error.etype(),
        pingora::ErrorType::TLSWantX509Lookup
            | pingora::ErrorType::TLSHandshakeFailure
            | pingora::ErrorType::TLSHandshakeTimedout
            | pingora::ErrorType::InvalidCert
    )
}

pub(crate) fn reset_request_context(ctx: &mut RequestContext, record_start_time_unix_ms: bool) {
    clear_request_context(ctx);
    ctx.started_at = Some(Instant::now());
    ctx.started_at_unix_ms = request_start_time_unix_ms(record_start_time_unix_ms);
}

pub(crate) fn clear_completed_request_context(ctx: &mut RequestContext) {
    clear_request_context(ctx);
}

fn clear_request_context(ctx: &mut RequestContext) {
    ctx.started_at = None;
    ctx.started_at_unix_ms = 0;
    ctx.upstream_connect_started_at = None;
    ctx.request_span = None;
    clear_string(&mut ctx.snapshot_version);
    clear_string(&mut ctx.client_ip);
    clear_string(&mut ctx.host);
    clear_string(&mut ctx.method);
    clear_string(&mut ctx.path);
    clear_string(&mut ctx.request_id);
    ctx.request_headers = None;
    ctx.access_log_request_headers.clear();
    ctx.access_log_sent_response_headers.clear();
    ctx.access_log_upstream_response_headers.clear();
    ctx.access_log_upstream_statuses.clear();
    clear_string(&mut ctx.access_log_scheme);
    ctx.access_log_remote_port = None;
    ctx.status = 0;
    clear_string(&mut ctx.listener_name);
    clear_string(&mut ctx.listener_protocol);
    clear_string(&mut ctx.route_name);
    clear_string(&mut ctx.route_namespace);
    clear_string(&mut ctx.backend);
    clear_string(&mut ctx.route_kind);
    ctx.route_annotations.clear();
    clear_string(&mut ctx.response_flags);
    ctx.bytes_received = 0;
    ctx.declared_request_body_bytes = 0;
    ctx.runtime_ids = SelectedBackendRuntimeIds::default();
    ctx.selected_backend = None;
    ctx.fast_selected_backend = None;
    ctx.selected_backend_config = None;
    ctx.external_auth_response_headers.clear();
    ctx.local_response_traffic_topology = None;
    ctx.transport_retry_excluded_endpoints.clear();
    ctx.retry_backoff = None;
    ctx.retry_attempts = 0;
    ctx.upstream_pool_hits = 0;
    ctx.upstream_pool_misses = 0;
    ctx.upstream_peer_build_failures = 0;
    ctx.upstream_connect_latency_ms = 0;
    ctx.upstream_connect_latency_ms_max = 0;
    ctx.upstream_connect_latency_ms_buckets = [0; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
    ctx.resolved_session = None;
    ctx.request_mirrors.clear();
    ctx.is_mirror_subrequest = false;
    ctx.http_cache = CacheState::default();
    ctx.cached_response_body.clear();
    ctx.cached_response_body_bytes = 0;
    ctx.admission_permit = None;
    ctx.circuit_breaker_permit = None;
    ctx.cached_snapshot = None;
    ctx.rate_limit_applied = false;
    ctx.retry_budget_seeded = false;
    ctx.backend_observation_recorded = false;
    ctx.request_body_bytes_seen = 0;
    ctx.route_policy = None;
    clear_string(&mut ctx.http_version);
    clear_string(&mut ctx.query_string);
    clear_string(&mut ctx.upstream_addr);
    clear_string(&mut ctx.response_content_type);
    clear_string(&mut ctx.connection_id);
}

fn clear_string(value: &mut String) {
    value.clear();
}

pub(crate) fn request_start_time_unix_ms(enabled: bool) -> u128 {
    if enabled { epoch_millis() } else { 0 }
}

pub(crate) fn observe_selected_backend_failure(
    snapshot: &SharedSnapshot,
    ctx: &mut RequestContext,
) {
    observe_selected_backend_result(snapshot, ctx, true);
}

pub(crate) fn observe_selected_backend_success(
    snapshot: &SharedSnapshot,
    ctx: &mut RequestContext,
) {
    observe_selected_backend_result(snapshot, ctx, false);
}

fn observe_selected_backend_result(
    snapshot: &SharedSnapshot,
    ctx: &mut RequestContext,
    failed: bool,
) {
    if ctx.backend_observation_recorded {
        return;
    }
    let handle = if let Some(config) = ctx.selected_backend_config.as_ref() {
        config.runtime.clone()
    } else if let Some(selected) = ctx.selected_backend.as_ref() {
        snapshot.load().endpoint_runtime_handle(selected)
    } else {
        return;
    };
    if failed {
        handle.record_failure(Instant::now());
    } else {
        handle.record_success();
    }
    ctx.backend_observation_recorded = true;
}

pub(crate) fn remember_transport_retry_excluded_endpoint(ctx: &mut RequestContext) {
    let excluded = if let Some(selected) = ctx.selected_backend.as_ref() {
        Some(TransportRetryExcludedEndpoint::new(
            selected.backend_name.as_str(),
            &selected.backend,
        ))
    } else {
        ctx.fast_selected_backend.as_ref().map(|fast| {
            TransportRetryExcludedEndpoint::new(
                fast.selected.backend_name.as_str(),
                &fast.selected.backend,
            )
        })
    };

    if let Some(excluded) = excluded
        && !ctx
            .transport_retry_excluded_endpoints
            .iter()
            .any(|current| current == &excluded)
    {
        ctx.transport_retry_excluded_endpoints.push(excluded);
    }
}

pub(crate) fn selected_backend_is_transport_retry_excluded(
    ctx: &RequestContext,
    selected: &SelectedBackend,
) -> bool {
    ctx.transport_retry_excluded_endpoints
        .iter()
        .any(|excluded| excluded.matches_selected(selected))
}

pub(crate) fn cache_selected_backend_state(
    ctx: &mut RequestContext,
    selected: SelectedBackend,
    config: impl Into<Arc<SelectedBackendConfig>>,
    access_log_enabled: bool,
) {
    let config = config.into();
    let selected = Arc::new(selected);
    let upstream_addr = match &config.peer_address {
        UpstreamPeerAddress::Ip(ip) => {
            let ip_str = ip.to_string();
            let mut s = String::with_capacity(ip_str.len() + 6);
            s.push_str(&ip_str);
            s.push(':');
            s.push_str(&config.peer_port.to_string());
            s
        }
        UpstreamPeerAddress::Host(h) => {
            let mut s = String::with_capacity(h.len() + 6);
            s.push_str(h);
            s.push(':');
            s.push_str(&config.peer_port.to_string());
            s
        }
    };
    cache_selected_backend_fields(ctx, selected.as_ref(), access_log_enabled);
    ctx.runtime_ids = config.runtime_ids;
    ctx.selected_backend = Some(selected);
    ctx.fast_selected_backend = None;
    ctx.selected_backend_config = Some(config);
    ctx.upstream_addr = upstream_addr;
    ctx.local_response_traffic_topology = None;
    ctx.backend_observation_recorded = false;
}

#[cfg(test)]
pub(crate) fn cache_selected_backend(
    ctx: &mut RequestContext,
    selected: SelectedBackend,
    access_log_enabled: bool,
) {
    let selected = Arc::new(selected);
    cache_selected_backend_fields(ctx, selected.as_ref(), access_log_enabled);
    ctx.runtime_ids = SelectedBackendRuntimeIds::default();
    ctx.selected_backend = Some(selected);
    ctx.fast_selected_backend = None;
    ctx.selected_backend_config = None;
    ctx.local_response_traffic_topology = None;
    ctx.backend_observation_recorded = false;
}

pub(crate) fn cache_selected_backend_ref(
    ctx: &mut RequestContext,
    selected: &SelectedBackend,
    access_log_enabled: bool,
) {
    cache_selected_backend_fields(ctx, selected, access_log_enabled);
    ctx.runtime_ids = SelectedBackendRuntimeIds::default();
    ctx.selected_backend = Some(Arc::new(selected.clone()));
    ctx.fast_selected_backend = None;
    ctx.selected_backend_config = None;
    ctx.local_response_traffic_topology = None;
    ctx.backend_observation_recorded = false;
}

pub(crate) fn cache_fast_selected_backend_state(
    ctx: &mut RequestContext,
    selected: ntgw_ir::CompiledSelectedHttpBackend,
    display_fields_needed: bool,
) {
    if display_fields_needed {
        assign_ctx_string(&mut ctx.route_kind, route_kind_name(&selected.route_kind));
        assign_ctx_string(&mut ctx.route_name, selected.route_name.as_str());
        assign_ctx_string(&mut ctx.route_namespace, selected.route_namespace.as_str());
        assign_ctx_string(&mut ctx.listener_name, selected.listener_name.as_str());
        assign_ctx_string(
            &mut ctx.listener_protocol,
            selected.listener_protocol.as_str(),
        );
        assign_ctx_string(&mut ctx.backend, selected.backend_name.as_str());
    } else {
        clear_selected_backend_display_fields(ctx);
    }
    if !ctx.route_annotations.is_empty() {
        ctx.route_annotations.clear();
    }
    ctx.runtime_ids = selected.runtime_ids;
    ctx.selected_backend = None;
    ctx.fast_selected_backend = Some(FastSelectedBackendState { selected });
    ctx.selected_backend_config = None;
    ctx.local_response_traffic_topology = None;
    ctx.backend_observation_recorded = false;
    record_request_span(ctx);
}

fn cache_selected_backend_fields(
    ctx: &mut RequestContext,
    selected: &SelectedBackend,
    access_log_enabled: bool,
) {
    let display_fields_needed = access_log_enabled || ctx.request_span.is_some();
    if display_fields_needed {
        assign_ctx_string(&mut ctx.route_kind, route_kind_name(&selected.route_kind));
        assign_ctx_string(&mut ctx.route_name, selected.route_name.as_str());
        assign_ctx_string(&mut ctx.route_namespace, selected.route_namespace.as_str());
        assign_ctx_string(&mut ctx.listener_name, selected.listener_name.as_str());
        assign_ctx_string(
            &mut ctx.listener_protocol,
            selected.listener_protocol.as_str(),
        );
        assign_ctx_string(&mut ctx.backend, selected.backend_name.as_str());
    } else {
        clear_selected_backend_display_fields(ctx);
    }
    if !ctx.route_annotations.is_empty() {
        ctx.route_annotations.clear();
    }
    ctx.runtime_ids = SelectedBackendRuntimeIds::default();
    record_request_span(ctx);
}

fn clear_selected_backend_display_fields(ctx: &mut RequestContext) {
    clear_string(&mut ctx.route_kind);
    clear_string(&mut ctx.route_name);
    clear_string(&mut ctx.route_namespace);
    clear_string(&mut ctx.listener_name);
    clear_string(&mut ctx.listener_protocol);
    clear_string(&mut ctx.backend);
}

pub(crate) fn assign_ctx_string(target: &mut String, value: &str) {
    target.clear();
    target.push_str(value);
}

pub(crate) struct HttpRouteContextFields<'a> {
    pub(crate) route_name: &'a str,
    pub(crate) route_namespace: &'a str,
    pub(crate) route_annotations: &'a BTreeMap<String, String>,
    pub(crate) listener_name: &'a str,
    pub(crate) listener_protocol: &'a str,
    pub(crate) backend_name: Option<&'a str>,
}

pub(crate) fn cache_http_route_context(
    ctx: &mut RequestContext,
    fields: HttpRouteContextFields<'_>,
    access_log: &AccessLogOptions,
) {
    assign_ctx_string(&mut ctx.route_kind, "Http");
    assign_ctx_string(&mut ctx.route_name, fields.route_name);
    assign_ctx_string(&mut ctx.route_namespace, fields.route_namespace);
    cache_route_annotations(ctx, access_log, fields.route_annotations);
    assign_ctx_string(&mut ctx.listener_name, fields.listener_name);
    assign_ctx_string(&mut ctx.listener_protocol, fields.listener_protocol);
    if let Some(backend_name) = fields.backend_name {
        assign_ctx_string(&mut ctx.backend, backend_name);
    } else {
        ctx.backend.clear();
    }
}

pub(crate) fn cache_route_annotations(
    ctx: &mut RequestContext,
    access_log: &AccessLogOptions,
    annotations: &BTreeMap<String, String>,
) {
    if access_log.enabled {
        ctx.route_annotations = annotations.clone();
    } else if !ctx.route_annotations.is_empty() {
        ctx.route_annotations.clear();
    }
}

pub(crate) fn route_kind_name(route_kind: &RouteKind) -> &'static str {
    match route_kind {
        RouteKind::Http => "Http",
        RouteKind::Grpc => "Grpc",
        RouteKind::Tcp => "Tcp",
        RouteKind::Udp => "Udp",
        RouteKind::Tls => "Tls",
    }
}

pub(crate) fn route_budget_key(route_kind: &str, namespace: &str, name: &str) -> String {
    let cap = route_kind.len() + namespace.len() + name.len() + 2;
    let mut s = String::with_capacity(cap);
    s.push_str(route_kind);
    s.push('/');
    s.push_str(namespace);
    s.push('/');
    s.push_str(name);
    s
}

pub(crate) fn store_admission_permit(ctx: &mut RequestContext, permit: HttpAdmissionPermit) {
    if let Some(existing) = ctx.admission_permit.as_mut() {
        existing.merge(permit);
    } else {
        ctx.admission_permit = Some(permit);
    }
}

pub(crate) fn saturating_latency_ms(value: u128) -> u64 {
    value.min(u64::MAX as u128) as u64
}
