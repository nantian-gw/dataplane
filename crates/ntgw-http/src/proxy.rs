use std::{
    boxed::Box,
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use async_trait::async_trait;
use bytes::Bytes;
use http::Version;
use ntgw_ai::wasm_filter::WasmPluginFilter;
use pingora::{
    Error, ErrorSource, ErrorType,
    http::{RequestHeader, ResponseHeader},
    prelude::HttpPeer,
    protocols::l4::ext::TcpKeepalive,
    proxy::{FailToProxy, ProxyHttp, Session},
};
use pingora_cache::NoCacheReason;
use pingora_cache::cache_control::CacheControl;
use tracing::error;

pub(crate) use self::backend::UpstreamTuningOptions;
use crate::cache::CacheManager;
use crate::filters::{apply_request_filters, apply_response_filters, ensure_supported_filters};
use crate::mirror::{forward_body_chunk, is_mirror_subrequest, wait_for_request_mirrors};
use crate::session::{SessionManager, SessionPersistenceOptions};
use ntgw_ir::{
    BackendPolicy, BackendSelectionError, Filter, FrontendClientCertificateRequirement,
    PersistentSessionTarget, RequestMeta, RequestMirrorContext, RetryPolicy, RouteKind,
    RouteTimeouts, SelectedBackend, SelectedHttpRoute, SessionPersistence, SharedSnapshot,
    Snapshot,
};
use ntgw_observability::{
    AccessLogOptions, HttpAdmissionController, HttpCircuitBreakerController,
    HttpRateLimitController, RetryBudgetController, SharedTrafficStats, TrafficTopology,
};

mod backend;
mod cache;
mod context;
mod downstream_tls;
mod external_auth;
mod filters;
mod guards;
mod logging;
mod request;
mod responses;
mod retry;
mod selection;
mod upstream;

use self::backend::{
    build_upstream_peer_for_fast_path, build_upstream_peer_with_cached_config,
    error_for_backend_selection, validate_backend_tls_subject_alt_name_result,
};
pub(crate) use self::cache::BackendTlsValidationCacheKey;
use self::cache::SessionResolutionCache;
pub use self::context::RequestContext;
#[cfg(test)]
use self::context::UpstreamPeerAddress;
#[cfg(test)]
use self::context::cache_selected_backend;
#[cfg(test)]
use self::context::clear_completed_request_context;
use self::context::{
    HttpRouteContextFields, SelectedBackendConfig, assign_ctx_string,
    cache_fast_selected_backend_state, cache_http_route_context, cache_route_annotations,
    cache_selected_backend_ref, cache_selected_backend_state, observe_selected_backend_failure,
    observe_selected_backend_success, record_upstream_connection,
    record_upstream_peer_build_failure, record_upstream_tls_handshake_failure,
    request_start_time_unix_ms, reset_request_context, route_budget_key, route_kind_name,
    selected_backend_is_transport_retry_excluded, store_admission_permit,
};
pub use self::downstream_tls::DownstreamTlsInfo;
use self::downstream_tls::{downstream_tls_client_certificate_present, downstream_tls_server_name};
use self::external_auth::{
    ExternalAuthDecision, apply_external_auth_response_headers, external_auth_filter,
    run_external_auth,
};
use self::logging::observe_completed_request;
pub(crate) use self::request::{
    RequestView, build_request_meta_from_header_with_port, capture_request_context_from_view,
    fast_path_request_from_header, start_request_span_from_header,
    start_request_span_from_header_if_enabled,
};
use self::request::{
    access_log_route_annotations, build_request_meta, build_selection_request_meta,
    cache_access_log_sent_response_headers_from_written_response_if_needed,
    cache_access_log_sent_response_headers_if_needed,
    cache_access_log_upstream_response_headers_if_needed, cache_request_headers_if_needed,
    capture_request_context, capture_request_context_from_view_for_limits, client_ip,
    inject_request_span_context, record_access_log_upstream_status_if_needed, record_request_span,
    request_header_bytes_for_limit, response_filters_need_request_headers, server_port,
    start_request_span_if_enabled,
};
use self::responses::{
    request_is_grpc, write_direct_response, write_grpc_no_route_response,
    write_http_no_route_response, write_response_header_with_access_log_capture,
};
use self::retry::{
    is_downstream_connection_closed, proxy_error_code, proxy_error_flag_for,
    request_is_retry_replayable, response_is_retryable, retry_backoff, retry_status_error,
    selected_retry_policy, should_suppress_proxy_error_log, try_prepare_retry,
    try_prepare_transport_retry,
};
use self::selection::{
    SelectedBackendConfigCache, selected_backend_config_cached,
    selected_backend_config_cached_for_fast_path, selected_backend_from_http_route,
};

const DEFAULT_HTTP_ROUTE_RETRIES: u32 = 1;
const DEFAULT_TRANSPORT_CONNECT_RETRIES: u32 = 1;
const DEFAULT_MAX_H2_UPSTREAM_STREAMS: usize = 128;
const TRANSPORT_RETRY_ENDPOINT_SELECTION_ATTEMPTS: usize = 8;

pub(crate) fn route_filters_have_request_mirror(filters: &[Filter]) -> bool {
    filters.iter().any(|filter| filter.request_mirror.is_some())
}

pub(crate) fn unmatched_traffic_topology(listener_name: &str) -> Arc<TrafficTopology> {
    Arc::new(TrafficTopology::unmatched(listener_name))
}

pub(crate) fn fast_path_request_features_are_safe(
    request_tracing_enabled: bool,
    request_headers_required: bool,
    request_source_ip_required: bool,
) -> bool {
    !request_tracing_enabled && !request_headers_required && !request_source_ip_required
}

#[allow(private_interfaces)]
pub(crate) struct InitialFastPathSelection {
    pub(super) selected: ntgw_ir::CompiledSelectedHttpBackend,
    config: Arc<SelectedBackendConfig>,
    pub(super) frontend_client_certificate_requirement: FrontendClientCertificateRequirement,
}

#[allow(private_interfaces)]
pub(crate) struct InitialRequestState {
    pub(super) request_header_bytes: usize,
    pub(super) misdirected_request: bool,
    pub(super) request_source_ip: Option<String>,
    pub(super) fast_path_selected: Option<InitialFastPathSelection>,
}

#[allow(clippy::too_many_arguments)]
#[allow(private_interfaces)]
pub(crate) fn prepare_initial_request_state(
    current: &Snapshot,
    selected_backend_config_cache: &SelectedBackendConfigCache,
    ctx: &mut RequestContext,
    request_header: &RequestHeader,
    request_server_port: u32,
    request_source_ip: Option<String>,
    downstream_tls_server_name: Option<&str>,
    request_tracing_enabled: bool,
    access_log_enabled: bool,
    max_request_body_bytes: usize,
    max_request_header_bytes: usize,
) -> pingora::Result<InitialRequestState> {
    let request_context_needs_source_ip = access_log_enabled || request_tracing_enabled;
    let request_context_needs_observability_fields = access_log_enabled || request_tracing_enabled;
    let request_view = RequestView::from_header_with_port(request_header, request_server_port);
    capture_request_context_from_view_for_limits(
        ctx,
        &request_view,
        request_context_needs_source_ip
            .then_some(request_source_ip.as_deref())
            .flatten(),
        request_context_needs_observability_fields,
        max_request_body_bytes > 0,
    );
    start_request_span_from_header_if_enabled(ctx, request_header, request_tracing_enabled);
    let request_header_bytes =
        request_header_bytes_for_limit(&request_view, max_request_header_bytes);
    let misdirected_request = https_request_is_misdirected_in_snapshot(
        current,
        &request_view,
        downstream_tls_server_name,
    );

    let fast_path_selected = if !misdirected_request
        && fast_path_request_features_are_safe(
            request_tracing_enabled,
            current.request_materialization.requires_full_headers(),
            current.request_materialization.source_ip,
        ) {
        cache_snapshot_version_if_observed(
            ctx,
            current.id.as_str(),
            access_log_enabled,
            request_tracing_enabled,
        );
        record_request_span(ctx);
        current
            .select_http_fast_path(fast_path_request_from_header(
                request_header,
                request_server_port,
            ))
            .map(|selected| {
                let config = selected_backend_config_cached_for_fast_path(
                    selected_backend_config_cache,
                    current,
                    &selected,
                )?;
                let frontend_client_certificate_requirement = current
                    .frontend_client_certificate_requirement(selected.listener_name.as_str());
                Ok::<_, Box<Error>>(InitialFastPathSelection {
                    selected,
                    config,
                    frontend_client_certificate_requirement,
                })
            })
            .transpose()?
    } else {
        None
    };

    Ok(InitialRequestState {
        request_header_bytes,
        misdirected_request,
        request_source_ip,
        fast_path_selected,
    })
}

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
    if request_headers_complete || !response_filters_need_request_headers(filters) {
        return request;
    }

    full_request.get_or_insert_with(|| build_request_meta(session))
}

#[derive(Clone)]
pub struct GatewayProxy {
    pub(crate) snapshot: SharedSnapshot,
    pub(crate) access_log: AccessLogOptions,
    pub(crate) session_manager: SessionManager,
    pub(crate) traffic: SharedTrafficStats,
    pub(crate) admission: HttpAdmissionController,
    pub(crate) circuit_breaker: HttpCircuitBreakerController,
    pub(crate) rate_limit: HttpRateLimitController,
    pub(crate) retry_budget: RetryBudgetController,
    pub(crate) selected_backend_config_cache: Arc<SelectedBackendConfigCache>,
    pub(crate) unmatched_traffic_topology: Option<Arc<TrafficTopology>>,
    pub(crate) downstream_read_timeout: Option<Duration>,
    pub(crate) downstream_max_connection_age: Option<Duration>,
    pub(crate) upstream_tcp_keepalive: Option<TcpKeepalive>,
    pub(crate) upstream_tuning: UpstreamTuningOptions,
    pub(crate) request_tracing_enabled: bool,
    pub(crate) max_request_body_bytes: usize,
    pub(crate) max_request_header_bytes: usize,
    pub(crate) ai_gateway_max_request_body_bytes: usize,
    pub(crate) listener_name_hint: Option<String>,
    pub(crate) listener_port_hint: Option<u32>,
    pub(crate) cache: Arc<CacheManager>,
    pub(crate) wasm_filter: Option<Arc<WasmPluginFilter>>,
    pub(crate) ai_filter: Option<Arc<ntgw_ai::filter::AIGatewayFilter>>,
}

impl GatewayProxy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        snapshot: SharedSnapshot,
        access_log: AccessLogOptions,
        session_options: SessionPersistenceOptions,
        traffic: SharedTrafficStats,
        admission: HttpAdmissionController,
        circuit_breaker: HttpCircuitBreakerController,
        rate_limit: HttpRateLimitController,
        retry_budget: RetryBudgetController,
        downstream_read_timeout: Option<Duration>,
        downstream_max_connection_age: Option<Duration>,
        upstream_tcp_keepalive: Option<TcpKeepalive>,
        upstream_tuning: UpstreamTuningOptions,
        request_tracing_enabled: bool,
        max_request_body_bytes: usize,
        max_request_header_bytes: usize,
        ai_gateway_max_request_body_bytes: usize,
        listener_name_hint: Option<String>,
        listener_port_hint: Option<u32>,
        cache: Arc<CacheManager>,
        wasm_filter: Option<Arc<WasmPluginFilter>>,
        ai_filter: Option<Arc<ntgw_ai::filter::AIGatewayFilter>>,
    ) -> Self {
        Self {
            snapshot,
            access_log,
            session_manager: SessionManager::new(session_options),
            traffic,
            admission,
            circuit_breaker,
            rate_limit,
            retry_budget,
            selected_backend_config_cache: Arc::new(SelectedBackendConfigCache),
            unmatched_traffic_topology: listener_name_hint
                .as_deref()
                .map(unmatched_traffic_topology),
            downstream_read_timeout,
            downstream_max_connection_age,
            upstream_tcp_keepalive,
            upstream_tuning,
            request_tracing_enabled,
            max_request_body_bytes,
            max_request_header_bytes,
            ai_gateway_max_request_body_bytes,
            listener_name_hint,
            listener_port_hint,
            cache,
            wasm_filter,
            ai_filter,
        }
    }
}

#[async_trait]
impl ProxyHttp for GatewayProxy {
    type CTX = RequestContext;

    fn new_ctx(&self) -> Self::CTX {
        RequestContext {
            started_at: Some(Instant::now()),
            started_at_unix_ms: request_start_time_unix_ms(self.access_log.enabled),
            ..RequestContext::default()
        }
    }

    fn allow_spawning_subrequest(&self, session: &Session, _ctx: &Self::CTX) -> bool
    where
        Self::CTX: Send + Sync,
    {
        !is_mirror_subrequest(session)
    }

    async fn early_request_filter(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<()>
    where
        Self::CTX: Send + Sync,
    {
        session
            .as_downstream_mut()
            .set_read_timeout(self.downstream_read_timeout);
        Ok(())
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool>
    where
        Self::CTX: Send + Sync,
    {
        self.apply_downstream_read_timeout(session, ctx);
        filters::do_request_filter(self, session, ctx).await
    }

    async fn request_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()>
    where
        Self::CTX: Send + Sync,
    {
        if !ctx.is_mirror_subrequest {
            let chunk_len = body.as_ref().map(Bytes::len).unwrap_or_default();
            ctx.bytes_received = ctx.bytes_received.saturating_add(chunk_len);
            ctx.request_body_bytes_seen = ctx.request_body_bytes_seen.saturating_add(chunk_len);
            let effective_limit = self.effective_max_request_body_bytes(ctx);
            if effective_limit > 0 && ctx.request_body_bytes_seen > effective_limit {
                assign_ctx_string(&mut ctx.response_flags, "RB");
                record_request_span(ctx);
                return Err(Error::new(ErrorType::HTTPStatus(413)).more_context(format!(
                    "request body exceeded configured limit of {} bytes",
                    effective_limit
                )));
            }
        }

        if !ctx.request_mirrors.is_empty() {
            let mut active = Vec::with_capacity(ctx.request_mirrors.len());
            for mirror in ctx.request_mirrors.drain(..) {
                if forward_body_chunk(&mirror, body, end_of_stream).await {
                    active.push(mirror);
                }
            }
            ctx.request_mirrors = active;
        }

        Ok(())
    }
    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        upstream::do_upstream_peer(self, session, ctx).await
    }

    async fn connected_to_upstream(
        &self,
        _session: &mut Session,
        reused: bool,
        _peer: &HttpPeer,
        #[cfg(unix)] _fd: std::os::unix::io::RawFd,
        #[cfg(windows)] _sock: std::os::windows::io::RawSocket,
        _digest: Option<&pingora::protocols::Digest>,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()>
    where
        Self::CTX: Send + Sync,
    {
        validate_backend_tls_subject_alt_name_result(_peer, _digest)?;
        record_upstream_connection(ctx, reused);
        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        remove_downstream_close_connection_token(upstream_request)?;
        apply_external_auth_response_headers(
            upstream_request,
            &ctx.external_auth_response_headers,
        )?;
        if let Some(selected) = ctx.selected_backend.as_ref() {
            apply_request_filters(
                upstream_request,
                &selected.filters,
                selected.matched_http_path.as_ref(),
            )?;
        }
        inject_request_span_context(ctx, upstream_request);
        Ok(())
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()>
    where
        Self::CTX: Send + Sync,
    {
        if let Some(ct) = upstream_response.headers.get("content-type") {
            ctx.response_content_type = ct.to_str().unwrap_or("-").to_string();
        }
        let route_access_log_annotations = access_log_route_annotations(ctx).clone();
        record_access_log_upstream_status_if_needed(
            ctx,
            upstream_response.status.as_u16(),
            &self.access_log,
            &route_access_log_annotations,
        );
        cache_access_log_upstream_response_headers_if_needed(
            ctx,
            upstream_response,
            &self.access_log,
            &route_access_log_annotations,
        );
        let status = upstream_response.status.as_u16();
        if status >= 500 {
            observe_selected_backend_failure(&self.snapshot, ctx);
        } else {
            observe_selected_backend_success(&self.snapshot, ctx);
        }
        if response_is_retryable(ctx, status) {
            if try_prepare_retry(ctx, session, &self.retry_budget) {
                record_request_span(ctx);
                return Err(retry_status_error(status, true));
            }
            record_request_span(ctx);
            return Err(retry_status_error(status, false));
        }

        if let Some(selected) = ctx.selected_backend.as_ref() {
            apply_response_filters(
                upstream_response,
                &selected.filters,
                Some(ctx.method.as_str()),
                ctx.request_headers.as_ref(),
            )?;
        }

        // AI Gateway post-processing — deferred to response_body_filter
        // where the actual response body is available.

        if let Some(selected) = ctx.selected_backend.as_ref()
            && let Some(policy) = selected.session_persistence.as_ref()
        {
            self.session_manager.write_response_session(
                upstream_response,
                policy,
                selected,
                ctx.resolved_session.as_ref(),
            )?;
        }
        let route_access_log_annotations = access_log_route_annotations(ctx).clone();
        cache_access_log_sent_response_headers_if_needed(
            ctx,
            upstream_response,
            &self.access_log,
            &route_access_log_annotations,
        );

        if !ctx.request_mirrors.is_empty() {
            wait_for_request_mirrors(&mut ctx.request_mirrors).await;
        }

        if let Some(http_cache) = ctx.http_cache.0.as_mut() {
            let status = upstream_response.status.as_u16();
            if status < 500 && (status < 300 || status == 404) {
                let cache_control = CacheControl::from_resp_headers(upstream_response);
                let has_auth = ctx
                    .request_headers
                    .as_ref()
                    .and_then(|h| h.get("authorization"))
                    .is_some();
                if let Some(meta) = self.cache.is_response_cacheable(
                    upstream_response,
                    cache_control.as_ref(),
                    has_auth,
                ) {
                    http_cache.set_cache_meta(meta);
                    if http_cache.set_miss_handler().await.is_err() {
                        http_cache.disable(NoCacheReason::StorageError);
                        ctx.http_cache = context::CacheState::default();
                    }
                } else {
                    http_cache.disable(NoCacheReason::OriginNotCache);
                    ctx.http_cache = context::CacheState::default();
                }
            } else {
                http_cache.disable(NoCacheReason::OriginNotCache);
                ctx.http_cache = context::CacheState::default();
            }
        }

        record_request_span(ctx);

        if let Some(ref wasm) = self.wasm_filter {
            let request_headers: HashMap<String, String> = ctx
                .request_headers
                .as_ref()
                .map(|h| h.iter().map(|(k, v)| (k.clone(), v.join(","))).collect())
                .unwrap_or_default();
            if let Err(e) = wasm.post_process(request_headers, vec![]).await {
                tracing::warn!(
                    target: "wasm_filter",
                    error = %e,
                    "wasm post_process failed, continuing"
                );
            }
        }

        Ok(())
    }

    fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        _end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Option<Duration>>
    where
        Self::CTX: Send + Sync,
    {
        let ai_filter_active = self.ai_filter.is_some();
        let should_buffer = ctx.http_cache.is_some() || ai_filter_active;

        if should_buffer && let Some(chunk) = body {
            if cache_response_body_limit_exceeded(
                ctx.cached_response_body_bytes,
                chunk.len(),
                self.cache.max_entry_size_bytes(),
            ) {
                if let Some(http_cache) = ctx.http_cache.0.as_mut() {
                    http_cache.disable(NoCacheReason::ResponseTooLarge);
                }
                ctx.http_cache = context::CacheState::default();
                ctx.cached_response_body.clear();
                ctx.cached_response_body_bytes = 0;
            } else {
                ctx.cached_response_body_bytes =
                    ctx.cached_response_body_bytes.saturating_add(chunk.len());
                ctx.cached_response_body.push(chunk.clone());
            }
            if ai_filter_active {
                *body = None;
            }
        }

        if _end_of_stream && self.ai_filter.is_some() && ctx.ai_context.is_some() {
            // Deferred to logging() which is async — avoids block_on in sync context
        }

        Ok(None)
    }

    async fn logging(&self, session: &mut Session, _e: Option<&pingora::Error>, ctx: &mut Self::CTX)
    where
        Self::CTX: Send + Sync,
    {
        ctx.status = session
            .response_written()
            .map(|resp| resp.status.as_u16())
            .unwrap_or(ctx.status);

        let route_access_log_annotations = access_log_route_annotations(ctx).clone();
        cache_access_log_sent_response_headers_from_written_response_if_needed(
            ctx,
            session.response_written(),
            &self.access_log,
            &route_access_log_annotations,
        );

        if let Some(http_cache) = ctx.http_cache.0.as_mut()
            && let Some(miss_handler) = http_cache.miss_handler()
        {
            let total = ctx.cached_response_body.len();
            for (i, chunk) in ctx.cached_response_body.drain(..).enumerate() {
                let last = i + 1 == total;
                if miss_handler.write_body(chunk, last).await.is_err() {
                    break;
                }
            }
            http_cache.finish_miss_handler().await.ok();
        }

        // AI Gateway post-processing — performed here in async context
        if let Some(ref ai_filter) = self.ai_filter
            && let Some(ai_ctx) = ctx.ai_context.take()
        {
            let response_body: Vec<u8> = ctx
                .cached_response_body
                .iter()
                .flat_map(|b| b.iter())
                .copied()
                .collect();
            if let Err(e) = ai_filter
                .post_process(ai_ctx, &response_body, ctx.status)
                .await
            {
                tracing::warn!(target: "ai_gateway", error = %e, "AI gateway post_process failed");
            }
        }

        record_request_span(ctx);
        let latency_ms = ctx
            .started_at
            .map(|started| started.elapsed().as_millis())
            .unwrap_or_default();
        let bytes_sent = session.body_bytes_sent();

        observe_completed_request(&self.access_log, &self.traffic, ctx, latency_ms, bytes_sent);
    }

    async fn fail_to_proxy(
        &self,
        session: &mut Session,
        e: &pingora::Error,
        ctx: &mut Self::CTX,
    ) -> FailToProxy
    where
        Self::CTX: Send + Sync,
    {
        let code = proxy_error_code(e);
        let downstream_closed = is_downstream_connection_closed(e);

        ctx.status = code;
        if ctx.response_flags.is_empty() {
            assign_ctx_string(&mut ctx.response_flags, proxy_error_flag_for(e));
        }
        if code > 0 && !downstream_closed {
            session.respond_error(code).await.unwrap_or_else(|err| {
                error!(error = %err, "failed to send error response to downstream");
            });
        }
        record_request_span(ctx);

        FailToProxy {
            error_code: code,
            can_reuse_downstream: false,
        }
    }

    fn suppress_error_log(
        &self,
        session: &Session,
        _ctx: &Self::CTX,
        error: &pingora::Error,
    ) -> bool {
        should_suppress_proxy_error_log(error, session.response_written().is_some())
    }

    fn error_while_proxy(
        &self,
        peer: &HttpPeer,
        session: &mut Session,
        e: Box<Error>,
        ctx: &mut Self::CTX,
        client_reused: bool,
    ) -> Box<Error> {
        upstream::do_error_while_proxy(self, peer, session, e, ctx, client_reused)
    }

    fn fail_to_connect(
        &self,
        session: &mut Session,
        peer: &HttpPeer,
        ctx: &mut Self::CTX,
        e: Box<Error>,
    ) -> Box<Error> {
        upstream::do_fail_to_connect(self, session, peer, ctx, e)
    }
}

impl GatewayProxy {
    pub(super) fn apply_downstream_read_timeout(
        &self,
        session: &mut Session,
        ctx: &RequestContext,
    ) {
        let timeout = ctx
            .route_policy
            .as_ref()
            .and_then(|rp| rp.timeout.as_ref())
            .and_then(|t| t.request)
            .map(std::time::Duration::from_millis)
            .or(self.downstream_read_timeout);
        session.as_downstream_mut().set_read_timeout(timeout);
    }

    pub(super) fn effective_max_request_body_bytes(&self, ctx: &RequestContext) -> usize {
        if let Some(limit) = ctx
            .route_policy
            .as_ref()
            .and_then(|rp| rp.body_limit.as_ref())
            .and_then(|bl| bl.max_request_body_bytes)
        {
            return limit;
        }
        self.max_request_body_bytes
    }

    pub(super) fn selected_display_fields_needed(&self, ctx: &RequestContext) -> bool {
        self.access_log.enabled || ctx.request_span.is_some()
    }
}

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
    request: &RequestMeta,
    session_resolver: &F,
) -> pingora::Result<Option<(SelectedBackend, Arc<SelectedBackendConfig>)>>
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

fn cache_response_body_limit_exceeded(current_len: usize, chunk_len: usize, limit: usize) -> bool {
    limit > 0 && current_len.saturating_add(chunk_len) > limit
}

#[cfg(test)]
mod tests;
