use bytes::Bytes;
use ntgw_ir::JwtAuthFilter;
use pingora::http::RequestHeader;
use std::sync::{Arc, LazyLock, Mutex};

use crate::filters::jwt::{JwtError, JwtValidator};

use super::super::{
    assign_ctx_string, cache_selected_http_route_context, record_request_span,
    SelectedHttpRoute,
};
use crate::filters::jwt_auth_filter;

static JWT_VALIDATORS: LazyLock<Mutex<HashMap<String, Arc<JwtValidator>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn get_or_create_validator(jwt_auth: &JwtAuthFilter) -> Result<Arc<JwtValidator>, String> {
    let mut validators = JWT_VALIDATORS
        .lock()
        .map_err(|_| "JWT_VALIDATORS lock poisoned".to_string())?;
    if let Some(validator) = validators.get(&jwt_auth.jwks_url) {
        return Ok(Arc::clone(validator));
    }
    let validator = Arc::new(
        JwtValidator::new(jwt_auth).map_err(|e| format!("failed to create JWT validator: {e}"))?,
    );
    validators.insert(jwt_auth.jwks_url.clone(), Arc::clone(&validator));
    Ok(validator)
}

fn extract_bearer_token(request: &RequestHeader, jwt_auth: &JwtAuthFilter) -> Option<String> {
    let header_value = request.headers.get(&jwt_auth.header_name)?.to_str().ok()?;

    if header_value.starts_with(&jwt_auth.token_prefix) {
        Some(header_value[jwt_auth.token_prefix.len()..].to_string())
    } else {
        None
    }
}

use std::collections::HashMap;
use pingora::prelude::Session;

use super::super::{GatewayProxy, RequestContext};

/// Handles JWT authentication for a selected HTTP route.
/// Returns `Ok(true)` if the request was handled (early return, e.g. denied),
/// `Ok(false)` if processing should continue past JWT validation.
pub(super) async fn handle_jwt_auth(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
) -> pingora::Result<bool> {
    let jwt_auth = match jwt_auth_filter(&route.filters) {
        Some(jwt) => jwt,
        None => return Ok(false),
    };

    let token = extract_bearer_token(session.req_header(), jwt_auth);
    let token = match token {
        Some(t) => t,
        None => {
            cache_selected_http_route_context(ctx, &proxy.access_log, route);
            ctx.status = 401;
            assign_ctx_string(&mut ctx.response_flags, "JWT");
            record_request_span(ctx);
            session
                .write_response_body(Some(Bytes::from("missing JWT token")), false)
                .await?;
            return Ok(true);
        }
    };

    let validator = match get_or_create_validator(jwt_auth) {
        Ok(v) => v,
        Err(e) => {
            ctx.status = 500;
            assign_ctx_string(&mut ctx.response_flags, "JWT");
            record_request_span(ctx);
            session
                .write_response_body(
                    Some(Bytes::from(format!("JWT validator error: {e}"))),
                    false,
                )
                .await?;
            return Ok(true);
        }
    };
    match validator
        .validate(&token, &jwt_auth.claims_to_headers)
        .await
    {
        Ok(claims) => {
            let req_header = session.req_header_mut();
            for (header_name, header_value) in claims {
                let _ = req_header.insert_header(header_name, header_value);
            }
        }
        Err(JwtError::Expired) => {
            cache_selected_http_route_context(ctx, &proxy.access_log, route);
            ctx.status = 401;
            assign_ctx_string(&mut ctx.response_flags, "JWT");
            record_request_span(ctx);
            session
                .write_response_body(Some(Bytes::from("JWT token expired")), false)
                .await?;
            return Ok(true);
        }
        Err(e) => {
            cache_selected_http_route_context(ctx, &proxy.access_log, route);
            ctx.status = 401;
            assign_ctx_string(&mut ctx.response_flags, "JWT");
            record_request_span(ctx);
            let body = Bytes::from(format!("JWT validation failed: {e}"));
            session.write_response_body(Some(body), false).await?;
            return Ok(true);
        }
    }

    Ok(false)
}
