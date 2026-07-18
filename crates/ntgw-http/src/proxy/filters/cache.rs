use pingora::prelude::Session;
use pingora_cache::HitStatus;
use tracing::debug_span;

use crate::cache::CacheManager;

use super::super::{
    GatewayProxy, RequestContext, record_request_span,
    write_response_header_with_access_log_capture,
};

pub(super) fn ai_request_body_limit_exceeded(
    current_len: usize,
    chunk_len: usize,
    limit: usize,
) -> bool {
    limit > 0 && current_len.saturating_add(chunk_len) > limit
}

pub(super) fn cache_lookup_method_allowed(method: &str) -> bool {
    method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD")
}

pub(super) fn cache_fast_path_access_log_fields(
    proxy: &GatewayProxy,
    session: &Session,
    ctx: &mut RequestContext,
) {
    if !proxy.access_log.enabled {
        return;
    }

    let route_access_log_annotations =
        super::super::request::access_log_route_annotations(ctx).clone();
    super::super::request::cache_access_log_connection_fields_if_needed(
        session,
        ctx,
        &proxy.access_log,
        &route_access_log_annotations,
    );
    super::super::request::cache_access_log_request_headers_from_header_if_needed(
        ctx,
        session.req_header(),
        &proxy.access_log,
        &route_access_log_annotations,
    );
}

pub(super) async fn try_cache_hit(
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

    if !cache_lookup_method_allowed(&ctx.method) {
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

    let lookup_span = debug_span!("http_cache_lookup", hit = tracing::field::Empty);
    let _guard = lookup_span.enter();
    let lookup_result = http_cache.cache_lookup().await.unwrap_or(None);
    match lookup_result {
        Some((meta, hit_handler)) => {
            lookup_span.record("hit", true);
            let cached_header = meta.response_header_copy();
            http_cache.cache_found(meta, hit_handler, HitStatus::Fresh);

            let status = cached_header.status.as_u16();
            let route_access_log_annotations =
                super::super::request::access_log_route_annotations(ctx).clone();
            write_response_header_with_access_log_capture(
                session,
                cached_header,
                false,
                ctx,
                &proxy.access_log,
                &route_access_log_annotations,
            )
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
            lookup_span.record("hit", false);
            http_cache.cache_miss();
            ctx.http_cache.0 = Some(http_cache);
            Ok(false)
        }
    }
}
