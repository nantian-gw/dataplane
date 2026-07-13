use crate::filters::build_cors_preflight_response;
use pingora::prelude::Session;

use super::super::{
    cache_selected_http_route_context, record_request_span,
    write_response_header_with_access_log_capture, GatewayProxy, RequestContext,
    SelectedHttpRoute,
};

/// Handles CORS preflight requests for a selected HTTP route.
/// Returns `Ok(true)` if the request was a preflight and was handled,
/// `Ok(false)` if processing should continue.
pub(super) async fn handle_cors_preflight(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
    filter_request: &ntgw_ir::RequestMeta,
) -> pingora::Result<bool> {
    if let Some(response) = match build_cors_preflight_response(
        &route.filters,
        &filter_request.method,
        &filter_request.headers,
    ) {
        Ok(response) => response,
        Err(err) => {
            cache_selected_http_route_context(ctx, &proxy.access_log, route);
            return Err(err);
        }
    } {
        cache_selected_http_route_context(ctx, &proxy.access_log, route);
        ctx.status = response.status.as_u16();
        record_request_span(ctx);
        write_response_header_with_access_log_capture(
            session,
            response,
            true,
            ctx,
            &proxy.access_log,
            &route.route_annotations,
        )
        .await?;
        return Ok(true);
    }

    Ok(false)
}
