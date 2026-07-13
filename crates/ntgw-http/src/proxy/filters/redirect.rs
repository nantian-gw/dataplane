use crate::filters::{
    apply_response_filters, build_redirect_location, build_redirect_response,
    request_redirect_filter,
};
use pingora::prelude::Session;

use super::super::{
    cache_selected_http_route_context,
    write_response_header_with_access_log_capture, GatewayProxy, RequestContext,
    SelectedHttpRoute,
};

/// Handles request redirect filter for a selected HTTP route.
/// Returns `Ok(true)` if the request was redirected,
/// `Ok(false)` if processing should continue.
pub(super) async fn handle_redirect(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
    request: &ntgw_ir::RequestMeta,
    filter_request: &ntgw_ir::RequestMeta,
) -> pingora::Result<bool> {
    let Some(redirect) = request_redirect_filter(&route.filters) else {
        return Ok(false);
    };

    cache_selected_http_route_context(ctx, &proxy.access_log, route);
    let location =
        build_redirect_location(session, request, &route.matched_http_path, redirect);
    let mut response = build_redirect_response(redirect.status_code, &location)?;
    apply_response_filters(
        &mut response,
        &route.filters,
        Some(&filter_request.method),
        Some(&filter_request.headers),
    )?;
    write_response_header_with_access_log_capture(
        session,
        response,
        true,
        ctx,
        &proxy.access_log,
        &route.route_annotations,
    )
    .await?;
    ctx.status = redirect.status_code;
    Ok(true)
}
