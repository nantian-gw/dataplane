use bytes::Bytes;
use ntgw_ir::SessionPersistence;
use ntgw_wasm::WasmError;
use pingora::prelude::Session;
use pingora_cache::HitStatus;
use std::collections::HashMap;

use super::*;

use crate::extensions::direct_response_filter;
use crate::filters::{
    apply_response_filters, build_cors_preflight_response, build_redirect_location,
    build_redirect_response, ensure_supported_filters, request_redirect_filter,
};
use crate::mirror::{selected_backend_from_subrequest, spawn_request_mirrors};
pub(crate) async fn do_request_filter(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
) -> pingora::Result<bool> {
    reset_request_context(ctx, proxy.access_log.enabled);
    let request_has_body = !session.as_downstream_mut().is_body_empty();
    // Access log: capture HTTP version, query string, connection ID
    if proxy.access_log.enabled {
        ctx.http_version = format!("{:?}", session.req_header().version);
        ctx.query_string = session.req_header().uri.query().unwrap_or("").to_string();
        ctx.connection_id = ctx.request_id.clone();
    }
    let request_server_port = proxy
        .listener_port_hint
        .unwrap_or_else(|| server_port(session));
    let request_context_needs_source_ip = proxy.access_log.enabled || proxy.request_tracing_enabled;
    let initial_request_state = {
        let current = proxy.snapshot.read();
        let request_source_ip =
            if current.request_materialization.source_ip || request_context_needs_source_ip {
                client_ip(session)
            } else {
                None
            };
        let downstream_tls_server_name = downstream_tls_server_name(session);
        prepare_initial_request_state(
            &current,
            &proxy.selected_backend_config_cache,
            ctx,
            session.req_header(),
            request_server_port,
            request_source_ip,
            downstream_tls_server_name.as_deref(),
            proxy.request_tracing_enabled,
            proxy.access_log.enabled,
            proxy.max_request_body_bytes,
            proxy.max_request_header_bytes,
        )
    }?;
    let request_header_bytes = initial_request_state.request_header_bytes;
    let misdirected_request = initial_request_state.misdirected_request;
    let mut request_source_ip = initial_request_state.request_source_ip;
    let fast_path_selected = initial_request_state.fast_path_selected;
    mark_downstream_max_connection_age_if_needed(session, ctx, proxy.downstream_max_connection_age);
    if misdirected_request {
        ctx.status = 421;
        assign_ctx_string(&mut ctx.response_flags, "MR");
        record_request_span(ctx);
        session.respond_error(421).await?;
        return Ok(true);
    }

    if let Some(selected) = selected_backend_from_subrequest(session) {
        ctx.is_mirror_subrequest = true;
        cache_selected_backend_ref(ctx, selected, proxy.access_log.enabled);
        return Ok(false);
    }

    if proxy.max_request_header_bytes > 0 && request_header_bytes > proxy.max_request_header_bytes {
        ctx.status = 431;
        assign_ctx_string(&mut ctx.response_flags, "RH");
        record_request_span(ctx);
        session.respond_error(431).await?;
        return Ok(true);
    }

    if proxy.max_request_body_bytes > 0
        && ctx.declared_request_body_bytes > proxy.max_request_body_bytes
        && request_has_body
    {
        ctx.status = 413;
        assign_ctx_string(&mut ctx.response_flags, "RB");
        record_request_span(ctx);
        session.respond_error(413).await?;
        return Ok(true);
    }

    if !proxy.try_admit_listener(session, ctx).await? {
        return Ok(true);
    }
    if !proxy.try_rate_limit_listener(session, ctx).await? {
        return Ok(true);
    }
    if let Some(fast_path_selected) = fast_path_selected {
        let InitialFastPathSelection {
            selected,
            config,
            frontend_client_certificate_requirement,
        } = fast_path_selected;
        if frontend_client_certificate_requirement
            .closes_connection_without_valid_client_certificate(
                downstream_tls_client_certificate_present(session),
            )
        {
            cache_fast_selected_backend_state(
                ctx,
                selected,
                proxy.selected_display_fields_needed(ctx),
            );
            return Err(missing_frontend_client_certificate_error(ctx));
        }
        if !proxy
            .try_admit_fast_http_route(session, ctx, &selected)
            .await?
        {
            return Ok(true);
        }
        if !proxy
            .try_rate_limit_fast_http_route(session, ctx, &selected)
            .await?
        {
            return Ok(true);
        }
        if ctx.rate_limit_applied {
            proxy.rate_limit.observe_allow();
        }
        cache_fast_selected_backend_state(
            ctx,
            selected.clone(),
            proxy.selected_display_fields_needed(ctx),
        );
        ctx.selected_backend_config = Some(config);
        record_request_span(ctx);
        let fast_host = ctx.host.clone();
        let fast_path = ctx.path.clone();
        if try_cache_hit(
            proxy,
            session,
            ctx,
            selected.route_namespace.as_str(),
            selected.route_name.as_str(),
            fast_host.as_str(),
            fast_path.as_str(),
        )
        .await?
        {
            return Ok(true);
        }
        return Ok(false);
    }
    let (request_headers_complete, request, grpc_selection, http_selection, fallback_selection) = {
        let current = proxy.snapshot.read();
        if request_source_ip.is_none() && current.request_materialization.source_ip {
            request_source_ip = client_ip(session);
        }
        let request_headers_complete = current.request_materialization.requires_full_headers();
        let request = build_selection_request_meta(
            session,
            request_source_ip,
            request_server_port,
            request_headers_complete,
        );
        let session_cache = SessionResolutionCache::new(&proxy.session_manager, &request.headers);
        let session_resolver = |policy: &SessionPersistence| session_cache.resolve_target(policy);
        let session_resolver_ref = &session_resolver;
        cache_snapshot_version_if_observed(
            ctx,
            current.id.as_str(),
            proxy.access_log.enabled,
            proxy.request_tracing_enabled,
        );
        record_request_span(ctx);

        let grpc_selection = if let Some(selected) =
            current.select_grpc_backend_with_session_resolver(&request, session_resolver_ref)
        {
            let config = selected_backend_config_cached(
                &proxy.selected_backend_config_cache,
                &current,
                &selected,
            )?;
            let mirrors = select_request_mirrors_for_selected_backend(&current, &selected);
            let frontend_client_certificate_requirement =
                current.frontend_client_certificate_requirement(selected.listener_name.as_str());
            Some((
                selected,
                mirrors,
                config,
                frontend_client_certificate_requirement,
            ))
        } else {
            None
        };

        let http_selection = if grpc_selection.is_none() {
            current
                .select_http_route_with_session_resolver(&request, session_resolver_ref)
                .map(|route| {
                    let mirrors = select_request_mirrors_for_http_route(&current, &route);
                    let frontend_client_certificate_requirement = current
                        .frontend_client_certificate_requirement(route.listener_name.as_str());
                    (route, mirrors, frontend_client_certificate_requirement)
                })
        } else {
            None
        };

        let fallback_selection = if grpc_selection.is_none() && http_selection.is_none() {
            select_backend_after_http_route_miss(
                &proxy.selected_backend_config_cache,
                &current,
                &request,
                session_resolver_ref,
            )?
            .map(|(selected, config)| {
                let frontend_client_certificate_requirement = current
                    .frontend_client_certificate_requirement(selected.listener_name.as_str());
                (selected, config, frontend_client_certificate_requirement)
            })
        } else {
            None
        };

        (
            request_headers_complete,
            request,
            grpc_selection,
            http_selection,
            fallback_selection,
        )
    };
    let mut full_request = None;
    let session_cache = SessionResolutionCache::new(&proxy.session_manager, &request.headers);

    if let Some((selected, mirrors, config, frontend_client_certificate_requirement)) =
        grpc_selection
    {
        if frontend_client_certificate_requirement
            .closes_connection_without_valid_client_certificate(
                downstream_tls_client_certificate_present(session),
            )
        {
            cache_selected_backend_ref(ctx, &selected, proxy.access_log.enabled);
            return Err(missing_frontend_client_certificate_error(ctx));
        }
        if !proxy
            .try_admit_grpc_request(session, ctx, &selected)
            .await?
        {
            return Ok(true);
        }
        if !proxy
            .try_rate_limit_grpc_request(session, ctx, &selected)
            .await?
        {
            return Ok(true);
        }
        if ctx.rate_limit_applied {
            proxy.rate_limit.observe_allow();
        }
        ensure_supported_filters(&selected.filters)?;
        if let Some(direct_response) = direct_response_filter(&selected.filters) {
            let response_request = request_for_response_filters(
                session,
                &request,
                &mut full_request,
                request_headers_complete,
                &selected.filters,
            );
            cache_selected_backend_ref(ctx, &selected, proxy.access_log.enabled);
            ctx.status = write_direct_response(
                session,
                direct_response,
                &selected.filters,
                Some(&response_request.method),
                Some(&response_request.headers),
            )
            .await?;
            return Ok(true);
        }
        ctx.request_mirrors = spawn_request_mirrors(session, mirrors, request_has_body);
        let filter_request = request_for_response_filters(
            session,
            &request,
            &mut full_request,
            request_headers_complete,
            &selected.filters,
        );
        cache_request_headers_if_needed(ctx, &filter_request.headers, &selected.filters);
        ctx.resolved_session = selected
            .session_persistence
            .as_ref()
            .and_then(|policy| session_cache.resolved_session(policy));
        cache_selected_backend_state(ctx, selected, config, proxy.access_log.enabled);
        record_request_span(ctx);
        return Ok(false);
    }

    let Some((route, mirrors, frontend_client_certificate_requirement)) = http_selection else {
        if let Some((selected, config, frontend_client_certificate_requirement)) =
            fallback_selection
        {
            if frontend_client_certificate_requirement
                .closes_connection_without_valid_client_certificate(
                    downstream_tls_client_certificate_present(session),
                )
            {
                cache_selected_backend_ref(ctx, &selected, proxy.access_log.enabled);
                return Err(missing_frontend_client_certificate_error(ctx));
            }
            if ctx.rate_limit_applied {
                proxy.rate_limit.observe_allow();
            }
            ensure_supported_filters(&selected.filters)?;
            let filter_request = request_for_response_filters(
                session,
                &request,
                &mut full_request,
                request_headers_complete,
                &selected.filters,
            );
            cache_request_headers_if_needed(ctx, &filter_request.headers, &selected.filters);
            ctx.resolved_session = selected
                .session_persistence
                .as_ref()
                .and_then(|policy| session_cache.resolved_session(policy));
            cache_selected_backend_state(ctx, selected, config, proxy.access_log.enabled);
            proxy.seed_retry_budget(ctx);
            record_request_span(ctx);
            let route_ns = ctx
                .selected_backend
                .as_ref()
                .map(|b| b.route_namespace.clone());
            let route_name = ctx.selected_backend.as_ref().map(|b| b.route_name.clone());
            let host = ctx.host.clone();
            let path = ctx.path.clone();
            if let (Some(route_ns), Some(route_name)) = (&route_ns, &route_name)
                && try_cache_hit(
                    proxy,
                    session,
                    ctx,
                    route_ns.as_str(),
                    route_name.as_str(),
                    host.as_str(),
                    path.as_str(),
                )
                .await?
            {
                return Ok(true);
            }
            return Ok(false);
        }
        if ctx.rate_limit_applied {
            proxy.rate_limit.observe_allow();
        }
        assign_ctx_string(&mut ctx.response_flags, "NR");
        ctx.local_response_traffic_topology = proxy.unmatched_traffic_topology.clone();
        if request_is_grpc(&request) {
            ctx.status = write_grpc_no_route_response(session).await?;
        } else {
            ctx.status = write_http_no_route_response(session).await?;
        }
        record_request_span(ctx);
        return Ok(true);
    };

    if frontend_client_certificate_requirement.closes_connection_without_valid_client_certificate(
        downstream_tls_client_certificate_present(session),
    ) {
        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
        return Err(missing_frontend_client_certificate_error(ctx));
    }

    if !proxy.try_admit_http_route(session, ctx, &route).await? {
        return Ok(true);
    }
    if !proxy
        .try_rate_limit_http_route(session, ctx, &route)
        .await?
    {
        return Ok(true);
    }
    if ctx.rate_limit_applied {
        proxy.rate_limit.observe_allow();
    }

    record_request_span(ctx);
    if let Err(err) = ensure_supported_filters(&route.filters) {
        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
        return Err(err);
    }
    let filter_request = request_for_response_filters(
        session,
        &request,
        &mut full_request,
        request_headers_complete,
        &route.filters,
    );
    if let Some(response) = match build_cors_preflight_response(
        &route.filters,
        &filter_request.method,
        &filter_request.headers,
    ) {
        Ok(response) => response,
        Err(err) => {
            cache_selected_http_route_context(ctx, &proxy.access_log, &route);
            return Err(err);
        }
    } {
        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
        ctx.status = response.status.as_u16();
        record_request_span(ctx);
        session
            .write_response_header(Box::new(response), true)
            .await?;
        return Ok(true);
    }
    if let Some(direct_response) = direct_response_filter(&route.filters) {
        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
        ctx.status = write_direct_response(
            session,
            direct_response,
            &route.filters,
            Some(&filter_request.method),
            Some(&filter_request.headers),
        )
        .await?;
        record_request_span(ctx);
        return Ok(true);
    }

    if let Some(redirect) = request_redirect_filter(&route.filters) {
        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
        let location =
            build_redirect_location(session, &request, &route.matched_http_path, redirect);
        let mut response = build_redirect_response(redirect.status_code, &location)?;
        apply_response_filters(
            &mut response,
            &route.filters,
            Some(&filter_request.method),
            Some(&filter_request.headers),
        )?;
        session
            .write_response_header(Box::new(response), true)
            .await?;
        ctx.status = redirect.status_code;
        return Ok(true);
    }

    if let Some(auth) = external_auth_filter(&route.filters) {
        let endpoint = {
            let current = proxy.snapshot.read();
            current
                .select_backend_ref(&auth.backend_ref)
                .map(|(endpoint, _)| endpoint)
        };
        let Some(endpoint) = endpoint else {
            cache_selected_http_route_context(ctx, &proxy.access_log, &route);
            ctx.status = 500;
            assign_ctx_string(&mut ctx.response_flags, "EA");
            record_request_span(ctx);
            session.respond_error(500).await?;
            return Ok(true);
        };

        // forwardBody: read downstream body for auth, buffered in Upstream's
        // retry buffer (≤64KB) for upstream replay. Bodies larger than maxSize
        // return 413 without contacting auth or backend.
        let body = if let Some(max_size) = auth.forward_body_max_size.filter(|&s| s > 0) {
            // Enable retry buffering before reading body so bytes are
            // captured into Upstream's buffer for upstream forwarding.
            // (Upstream normally enables this after request_filter.)
            session.as_downstream_mut().enable_retry_buffering();
            let mut full_body = Vec::new();
            loop {
                match session.as_downstream_mut().read_request_body().await {
                    Ok(Some(chunk)) => {
                        full_body.extend_from_slice(&chunk);
                        if full_body.len() > max_size as usize {
                            cache_selected_http_route_context(ctx, &proxy.access_log, &route);
                            ctx.status = 413;
                            assign_ctx_string(&mut ctx.response_flags, "EA");
                            record_request_span(ctx);
                            session.respond_error(413).await?;
                            return Ok(true);
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
                        ctx.status = 500;
                        assign_ctx_string(&mut ctx.response_flags, "EA");
                        record_request_span(ctx);
                        return Err(e);
                    }
                }
            }
            if full_body.is_empty() {
                None
            } else {
                Some(Bytes::from(full_body))
            }
        } else {
            None
        };
        match run_external_auth(session.req_header(), auth, &endpoint, body.as_ref()).await {
            Ok(ExternalAuthDecision::Allow(headers)) => {
                ctx.external_auth_response_headers = headers;
            }
            Ok(ExternalAuthDecision::Deny(response, body)) => {
                let mut response = *response;
                cache_selected_http_route_context(ctx, &proxy.access_log, &route);
                apply_response_filters(
                    &mut response,
                    &route.filters,
                    Some(&filter_request.method),
                    Some(&filter_request.headers),
                )?;
                ctx.status = response.status.as_u16();
                assign_ctx_string(&mut ctx.response_flags, "EA");
                record_request_span(ctx);
                if body.is_empty() {
                    session
                        .write_response_header(Box::new(response), true)
                        .await?;
                } else {
                    session
                        .write_response_header(Box::new(response), false)
                        .await?;
                    session.write_response_body(Some(body), true).await?;
                }
                return Ok(true);
            }
            Err(_) => {
                cache_selected_http_route_context(ctx, &proxy.access_log, &route);
                ctx.status = 500;
                assign_ctx_string(&mut ctx.response_flags, "EA");
                record_request_span(ctx);
                session.respond_error(500).await?;
                return Ok(true);
            }
        }
    }

    if route.backend_error.is_some() || route.backend.is_none() || route.backend_name.is_none() {
        cache_selected_http_route_context(ctx, &proxy.access_log, &route);
    }
    match selected_backend_from_http_route(route, proxy.access_log.enabled) {
        Ok(Some(selected)) => {
            let config = {
                let current = proxy.snapshot.read();
                selected_backend_config_cached(
                    &proxy.selected_backend_config_cache,
                    &current,
                    &selected,
                )?
            };
            ctx.request_mirrors = spawn_request_mirrors(session, mirrors, request_has_body);
            let filter_request = request_for_response_filters(
                session,
                &request,
                &mut full_request,
                request_headers_complete,
                &selected.filters,
            );
            cache_request_headers_if_needed(ctx, &filter_request.headers, &selected.filters);
            ctx.resolved_session = selected
                .session_persistence
                .as_ref()
                .and_then(|policy| session_cache.resolved_session(policy));
            cache_selected_backend_state(ctx, selected, config, proxy.access_log.enabled);
            proxy.seed_retry_budget(ctx);
            let route_ns = ctx
                .selected_backend
                .as_ref()
                .map(|b| b.route_namespace.clone());
            let route_name = ctx.selected_backend.as_ref().map(|b| b.route_name.clone());
            if let (Some(route_ns), Some(route_name)) = (&route_ns, &route_name) {
                let host = ctx.host.clone();
                let path = ctx.path.clone();
                if try_cache_hit(
                    proxy,
                    session,
                    ctx,
                    route_ns.as_str(),
                    route_name.as_str(),
                    host.as_str(),
                    path.as_str(),
                )
                .await?
                {
                    return Ok(true);
                }
            }
        }
        Ok(None) => {}
        Err(err) => return Err(error_for_backend_selection(err)),
    }

    if let Some(ref wasm) = proxy.wasm_filter {
        let request_headers: HashMap<String, String> = session
            .req_header()
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        if let Err(e) = wasm.pre_process(&request_headers, &[]) {
            match e {
                WasmError::PluginRejected(_name, code) => {
                    let status = code.clamp(400, 599) as u16;
                    ctx.status = status;
                    assign_ctx_string(&mut ctx.response_flags, "WR");
                    record_request_span(ctx);
                    session.respond_error(status).await?;
                    return Ok(true);
                }
                _ => {
                    tracing::warn!(
                        target: "wasm_filter",
                        error = %e,
                        "wasm pre_process failed, continuing"
                    );
                }
            }
        }
    }

    // AI Gateway pre-processing
    if let Some(ref ai_filter) = proxy.ai_filter {
        session.as_downstream_mut().enable_retry_buffering();
        let mut body = Vec::new();
        loop {
            match session.as_downstream_mut().read_request_body().await {
                Ok(Some(chunk)) => {
                    body.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(
                        target: "ai_gateway",
                        error = %e,
                        "AI gateway failed to read request body"
                    );
                    ctx.status = 500;
                    assign_ctx_string(&mut ctx.response_flags, "AR");
                    record_request_span(ctx);
                    session.respond_error(500).await?;
                    return Ok(true);
                }
            }
        }

        ctx.path = session.req_header().uri.path().to_string();
        match ai_filter.pre_process(ctx.path.as_str(), &body, None).await {
            Ok(ai_ctx) => {
                ctx.ai_context = Some(ai_ctx);
            }
            Err(e) => {
                if matches!(e, ntgw_ai::error::AIError::UnsupportedFormat(_)) {
                    // Not an AI route — skip AI processing silently
                } else {
                    tracing::warn!(
                        target: "ai_gateway",
                        error = %e,
                        path = %ctx.path,
                        "AI gateway pre_process failed, rejecting request"
                    );
                    ctx.status = 500;
                    assign_ctx_string(&mut ctx.response_flags, "AR");
                    record_request_span(ctx);
                    session.respond_error(500).await?;
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

async fn try_cache_hit(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route_namespace: &str,
    route_name: &str,
    host: &str,
    path: &str,
) -> pingora::Result<bool> {
    if !proxy.cache.enabled {
        return Ok(false);
    }

    let method = ctx.method.to_uppercase();
    if method != "GET" && method != "HEAD" {
        return Ok(false);
    }

    if !CacheManager::is_request_cacheable(session.req_header()) {
        return Ok(false);
    }

    let mut http_cache = proxy.cache.create_cache();
    let key = proxy
        .cache
        .generate_key(route_namespace, route_name, host, path);
    http_cache.set_cache_key(key);

    let lookup_result = http_cache.cache_lookup().await.unwrap_or(None);
    match lookup_result {
        Some((meta, hit_handler)) => {
            let cached_header = meta.response_header_copy();
            http_cache.cache_found(meta, hit_handler, HitStatus::Fresh);

            let status = cached_header.status.as_u16();
            session
                .write_response_header(Box::new(cached_header), false)
                .await?;

            {
                let body_reader = http_cache.hit_handler();
                loop {
                    match body_reader.read_body().await {
                        Ok(Some(chunk)) => {
                            session.write_response_body(Some(chunk), false).await?;
                        }
                        Ok(None) => {
                            session.write_response_body(None, true).await?;
                            break;
                        }
                        Err(_) => {
                            session.write_response_body(None, true).await?;
                            break;
                        }
                    }
                }
            }

            http_cache.finish_hit_handler().await.ok();
            ctx.status = status;
            record_request_span(ctx);
            Ok(true)
        }
        None => {
            http_cache.cache_miss();
            ctx.http_cache.0 = Some(http_cache);
            Ok(false)
        }
    }
}
