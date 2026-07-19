use std::{collections::HashMap, sync::Arc};

use super::*;
use crate::cache::CacheManager;
use crate::mirror::wait_for_request_mirrors;
use crate::session::SessionManager;
use ntgw_ai::wasm_filter::WasmPluginFilter;
use pingora_cache::NoCacheReason;
use pingora_cache::cache_control::CacheControl;

use super::context::CacheState;

pub(crate) fn handle_session_persistence_response(
    session_manager: &SessionManager,
    upstream_response: &mut ResponseHeader,
    ctx: &RequestContext,
) -> pingora::Result<()> {
    if let Some(selected) = ctx.selected_backend.as_ref()
        && let Some(policy) = selected.session_persistence.as_ref()
    {
        session_manager.write_response_session(
            upstream_response,
            policy,
            selected,
            ctx.resolved_session.as_ref(),
        )?;
    }
    Ok(())
}

pub(crate) async fn handle_mirror_completion(
    request_mirrors: &mut Vec<crate::mirror::MirrorRequestSession>,
) {
    if !request_mirrors.is_empty() {
        wait_for_request_mirrors(request_mirrors).await;
    }
}

pub(crate) async fn handle_http_cache_response(
    cache: &CacheManager,
    upstream_response: &ResponseHeader,
    ctx: &mut RequestContext,
) {
    if let Some(http_cache) = ctx.http_cache.0.as_mut() {
        let status = upstream_response.status.as_u16();
        if status < 500 && (status < 300 || status == 404) {
            let cache_control = CacheControl::from_resp_headers(upstream_response);
            let has_auth = ctx
                .request_headers
                .as_ref()
                .and_then(|h| h.get("authorization"))
                .is_some();
            if let Some(meta) =
                cache.is_response_cacheable(upstream_response, cache_control.as_ref(), has_auth)
            {
                http_cache.set_cache_meta(meta);
                if http_cache.set_miss_handler().await.is_err() {
                    http_cache.disable(NoCacheReason::StorageError);
                    ctx.http_cache = CacheState::default();
                }
            } else {
                http_cache.disable(NoCacheReason::OriginNotCache);
                ctx.http_cache = CacheState::default();
            }
        } else {
            http_cache.disable(NoCacheReason::OriginNotCache);
            ctx.http_cache = CacheState::default();
        }
    }
}

pub(crate) async fn handle_wasm_post_process_response(
    wasm_filter: &Option<Arc<WasmPluginFilter>>,
    ctx: &RequestContext,
) {
    if let Some(wasm) = wasm_filter {
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
}
