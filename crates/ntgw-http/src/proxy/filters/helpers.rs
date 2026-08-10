use pingora::prelude::Session;

use super::super::GatewayProxy;
use super::super::RequestContext;
use super::cache;

pub(crate) fn ai_request_body_limit_exceeded(
    current_len: usize,
    chunk_len: usize,
    limit: usize,
) -> bool {
    cache::ai_request_body_limit_exceeded(current_len, chunk_len, limit)
}

#[allow(dead_code)]
pub(crate) fn cache_lookup_method_allowed(method: &str) -> bool {
    cache::cache_lookup_method_allowed(method)
}

pub(crate) fn cache_fast_path_access_log_fields(
    proxy: &GatewayProxy,
    session: &Session,
    ctx: &mut RequestContext,
) {
    cache::cache_fast_path_access_log_fields(proxy, session, ctx)
}

pub(crate) async fn try_cache_hit(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route_namespace: &str,
    route_name: &str,
    host: &str,
    path: &str,
) -> pingora::Result<bool> {
    cache::try_cache_hit(proxy, session, ctx, route_namespace, route_name, host, path).await
}
