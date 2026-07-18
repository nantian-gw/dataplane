use std::time::Instant;

use ntgw_ir::SessionPersistence;
use pingora::{
    Error, ErrorType,
    prelude::{HttpPeer, Session},
};
use tracing::{debug_span, instrument};

use super::*;

fn apply_route_policy_to_peer(peer: &mut HttpPeer, ctx: &RequestContext) {
    if let Some(route_policy) = ctx.route_policy.as_ref() {
        if let Some(timeout_cfg) = route_policy.timeout.as_ref() {
            if let Some(connect_ms) = timeout_cfg.connect {
                let dur = std::time::Duration::from_millis(connect_ms);
                peer.options.connection_timeout = Some(dur);
                peer.options.total_connection_timeout = Some(dur);
            }
            if let Some(backend_req_ms) = timeout_cfg.backend_request {
                let dur = std::time::Duration::from_millis(backend_req_ms);
                if !dur.is_zero() {
                    peer.options.read_timeout = Some(dur);
                    peer.options.write_timeout = Some(dur);
                }
            }
        }
        if let Some(conn_cfg) = route_policy.connection.as_ref()
            && let Some(idle_secs) = conn_cfg.upstream_keepalive_idle
        {
            peer.options.idle_timeout = Some(std::time::Duration::from_secs(idle_secs));
        }
    }
}

#[instrument(skip_all)]
pub(crate) async fn do_upstream_peer(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
) -> pingora::Result<Box<HttpPeer>> {
    let _retry_span = if ctx.retry_attempts > 0 {
        Some(debug_span!("http_retry", attempt = ctx.retry_attempts))
    } else {
        None
    };
    if let Some(backoff) = retry_backoff(ctx) {
        tokio::time::sleep(backoff).await;
    }

    if ctx.fast_selected_backend.is_some() {
        let backend_config = ctx.selected_backend_config.as_ref().ok_or_else(|| {
            Error::new(ErrorType::new("InternalError"))
                .more_context("fast path selected backend config missing from request context")
        })?;
        let peer = {
            let fast = ctx.fast_selected_backend.as_ref().ok_or_else(|| {
                Error::new(ErrorType::new("InternalError"))
                    .more_context("fast path selected backend missing from request context")
            })?;
            build_upstream_peer_for_fast_path(
                &fast.selected,
                backend_config,
                proxy.upstream_tcp_keepalive.clone(),
                &proxy.upstream_tuning,
            )
        };
        let peer = match peer {
            Ok(mut peer) => {
                apply_route_policy_to_peer(&mut peer, ctx);
                peer
            }
            Err(err) => {
                record_upstream_peer_build_failure(ctx);
                return Err(err);
            }
        };
        if ctx.circuit_breaker_permit.is_none() {
            let snap = ctx.cached_snapshot(proxy);
            let permit = {
                let fast = ctx.fast_selected_backend.as_ref().ok_or_else(|| {
                    Error::new(ErrorType::new("InternalError"))
                        .more_context("fast path selected backend missing from request context")
                })?;
                sync_per_backend_cb_limit(&snap, proxy, &fast.selected.backend_name);
                proxy
                    .circuit_breaker
                    .try_acquire_backend(fast.selected.backend_name.as_ref())
                    .map_err(|_| {
                        Error::new(ErrorType::new("CircuitBreakerOpen")).more_context(format!(
                            "backend circuit breaker rejected request for {}",
                            fast.selected.backend_name
                        ))
                    })?
            };
            ctx.circuit_breaker_permit = Some(permit);
        }
        ctx.upstream_connect_started_at = Some(Instant::now());
        return Ok(Box::new(peer));
    }

    let endpoint = if let Some(selected) = ctx.selected_backend.clone() {
        selected
    } else {
        let request = build_request_meta(session);
        capture_request_context(ctx, &request);
        if ctx.request_span.is_none() {
            start_request_span_if_enabled(ctx, &request.headers, proxy.request_tracing_enabled);
        }
        let session_cache = SessionResolutionCache::new(&proxy.session_manager, &request.headers);
        let selected = {
            let current = ctx.cached_snapshot(proxy);
            cache_snapshot_version_if_observed(
                ctx,
                current.id.as_str(),
                proxy.access_log.enabled,
                proxy.request_tracing_enabled,
            );
            record_request_span(ctx);
            let session_resolver =
                |policy: &SessionPersistence| session_cache.resolve_target(policy);
            let selected = select_backend_with_transport_retry_exclusions(
                &current,
                &request,
                &session_resolver,
                ctx,
            )
            .ok_or_else(|| {
                if current.http_routes.is_empty() && current.grpc_routes.is_empty() {
                    Error::new(ErrorType::new("NoHealthyBackend"))
                } else {
                    Error::new(ErrorType::new("NoRouteMatched"))
                }
            })?;
            let config = selected_backend_config_cached(
                &proxy.selected_backend_config_cache,
                &current,
                &selected,
            )?;
            Ok::<_, Error>((selected, config))
        };
        let (selected, config) = selected?;
        ensure_supported_filters(&selected.filters)?;
        super::request::cache_request_headers_for_filters_and_access_log(
            ctx,
            &request.headers,
            &selected.filters,
            &proxy.access_log,
            &selected.route_annotations,
        );
        ctx.resolved_session = selected
            .session_persistence
            .as_ref()
            .and_then(|policy| session_cache.resolved_session(policy));
        cache_selected_backend_state(ctx, selected, config, proxy.access_log.enabled);
        proxy.seed_retry_budget(ctx);
        record_request_span(ctx);
        ctx.selected_backend.clone().ok_or_else(|| {
            Error::new(ErrorType::new("InternalError"))
                .more_context("selected backend missing from request context")
        })?
    };

    let peer = {
        if ctx.selected_backend_config.is_none() {
            let current = ctx.cached_snapshot(proxy);
            ctx.selected_backend_config = Some(selected_backend_config_cached(
                &proxy.selected_backend_config_cache,
                &current,
                endpoint.as_ref(),
            )?);
        }
        let backend_config = ctx.selected_backend_config.as_ref().ok_or_else(|| {
            Error::new(ErrorType::new("InternalError"))
                .more_context("selected backend config missing from request context")
        })?;
        build_upstream_peer_with_cached_config(
            endpoint.as_ref(),
            backend_config,
            proxy.upstream_tcp_keepalive.clone(),
            &proxy.upstream_tuning,
        )
    };
    let mut peer = match peer {
        Ok(peer) => peer,
        Err(err) => {
            record_upstream_peer_build_failure(ctx);
            return Err(err);
        }
    };
    if ctx.circuit_breaker_permit.is_none() {
        let snap = ctx.cached_snapshot(proxy);
        sync_per_backend_cb_limit(&snap, proxy, &endpoint.backend_name);
        ctx.circuit_breaker_permit = Some(
            proxy
                .circuit_breaker
                .try_acquire_backend(endpoint.backend_name.as_str())
                .map_err(|_| {
                    Error::new(ErrorType::new("CircuitBreakerOpen")).more_context(format!(
                        "backend circuit breaker rejected request for {}",
                        endpoint.backend_name
                    ))
                })?,
        );
    }
    apply_route_policy_to_peer(&mut peer, ctx);
    ctx.upstream_connect_started_at = Some(Instant::now());

    Ok(Box::new(peer))
}

use pingora::ErrorSource;

pub(crate) fn do_error_while_proxy(
    proxy: &GatewayProxy,
    peer: &HttpPeer,
    session: &mut Session,
    e: Box<Error>,
    ctx: &mut RequestContext,
    client_reused: bool,
) -> Box<Error> {
    let response_started = session.response_written().is_some();
    let downstream_error = matches!(e.esource(), ErrorSource::Downstream);
    let mut e = e.more_context(format!("Peer: {peer}"));
    if downstream_error {
        e.set_retry(false);
    } else {
        observe_selected_backend_failure(&proxy.snapshot, ctx);
    }

    if !response_started
        && !downstream_error
        && try_prepare_transport_retry(ctx, session, &proxy.retry_budget, e.as_ref())
    {
        e.set_retry(true);
    } else {
        e.retry.decide_reuse(
            !response_started
                && !downstream_error
                && client_reused
                && !session.as_ref().retry_buffer_truncated(),
        );
    }
    record_request_span(ctx);
    e
}

pub(crate) fn do_fail_to_connect(
    proxy: &GatewayProxy,
    session: &mut Session,
    peer: &HttpPeer,
    ctx: &mut RequestContext,
    e: Box<Error>,
) -> Box<Error> {
    record_upstream_tls_handshake_failure(&proxy.traffic, ctx, e.as_ref());
    record_upstream_connection(ctx, false);
    observe_selected_backend_failure(&proxy.snapshot, ctx);
    let mut e = e.more_context(format!("Peer: {peer}"));
    if try_prepare_transport_retry(ctx, session, &proxy.retry_budget, e.as_ref()) {
        e.set_retry(true);
    }
    record_request_span(ctx);
    e
}

fn sync_per_backend_cb_limit(snap: &Snapshot, proxy: &GatewayProxy, backend_name: &str) {
    if let Some(backend) = snap.backends.iter().find(|b| b.name == backend_name)
        && let Some(ref cb) = backend.circuit_breaker
        && cb.max_inflight_requests > 0
    {
        proxy
            .circuit_breaker
            .set_backend_limit(backend_name, cb.max_inflight_requests as usize);
    }
}
