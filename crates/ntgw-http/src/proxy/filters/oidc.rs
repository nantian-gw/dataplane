use base64::Engine;
use bytes::Bytes;
use hmac::{Hmac, Mac};
use ntgw_ir::OidcAuthConfig;
use pingora::http::ResponseHeader;
use pingora::prelude::Session;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{GatewayProxy, RequestContext, SelectedHttpRoute};
use crate::filters::build_redirect_response;

type HmacSha256 = Hmac<Sha256>;

/// Handles OIDC Authorization Code flow for a selected HTTP route.
///
/// Flow:
/// 1. If request path matches `callback_path` → exchange authorization code for tokens,
///    create HMAC-signed session cookie, redirect to original URL.
/// 2. If valid session cookie exists → continue (return `Ok(false)`).
/// 3. Otherwise → redirect to provider authorization URL.
///
/// Returns `Ok(true)` if the request was handled (redirect or error),
/// `Ok(false)` if processing should continue (valid session found).
pub(super) async fn handle_oidc(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
) -> pingora::Result<bool> {
    let oidc_config = match route
        .security_policy
        .as_ref()
        .and_then(|sp| sp.authn.as_ref())
        .and_then(|an| an.oidc.as_ref())
    {
        Some(config) => config,
        None => return Ok(false),
    };

    let path = session.req_header().uri.path();

    // Check if this is the OIDC callback path
    if path == oidc_config.callback_path {
        return handle_oidc_callback(proxy, session, ctx, route, oidc_config).await;
    }

    // Load session signing key from snapshot for verification
    let signing_key = {
        let snap = proxy.snapshot.load();
        snap.secrets
            .iter()
            .find(|s| s.name == oidc_config.session_signing_key_ref)
            .map(|s| s.key_pem.as_str())
            .unwrap_or("")
            .to_string()
    };

    // Check for existing valid session cookie
    if let Some(cookie_value) =
        extract_cookie(session.req_header(), effective_cookie_name(oidc_config))
        && verify_session(&cookie_value, &signing_key)
    {
        return Ok(false);
    }

    // No valid session → redirect to provider authorization URL
    redirect_to_authorization(proxy, session, ctx, route, oidc_config).await
}

async fn handle_oidc_callback(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
    oidc_config: &OidcAuthConfig,
) -> pingora::Result<bool> {
    // Extract authorization code from query string
    let query = session.req_header().uri.query().unwrap_or("");
    let code = match extract_query_param(query, "code") {
        Some(code) => code,
        None => {
            send_oidc_error(
                proxy,
                session,
                ctx,
                route,
                400,
                "missing authorization code",
            )
            .await?;
            return Ok(true);
        }
    };

    // Load client secret from snapshot
    let client_secret = {
        let snap = proxy.snapshot.load();
        snap.secrets
            .iter()
            .find(|s| s.name == oidc_config.client_secret_ref)
            .map(|s| s.oidc_client_secret.as_str())
            .unwrap_or("")
            .to_string()
    };

    // Exchange authorization code for tokens
    let token_response = exchange_code_for_tokens(
        oidc_config,
        &client_secret,
        &code,
        &oidc_config.redirect_url,
    )
    .await;

    let token_data = match token_response {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!(
                target: "oidc_filter",
                error = %e,
                "failed to exchange authorization code for tokens"
            );
            send_oidc_error(proxy, session, ctx, route, 500, "token exchange failed").await?;
            return Ok(true);
        }
    };

    // Load session signing key for creating the cookie
    let signing_key = {
        let snap = proxy.snapshot.load();
        snap.secrets
            .iter()
            .find(|s| s.name == oidc_config.session_signing_key_ref)
            .map(|s| s.key_pem.as_str())
            .unwrap_or("")
            .to_string()
    };

    // Create signed session cookie
    let session_payload = OidcSessionPayload {
        sub: token_data.sub,
        exp: token_data.exp,
        access_token_hash: hash_token(&token_data.access_token),
    };

    let signed_cookie = match create_session_cookie(&session_payload, &signing_key) {
        Ok(cookie) => cookie,
        Err(e) => {
            tracing::warn!(
                target: "oidc_filter",
                error = %e,
                "failed to create OIDC session cookie"
            );
            send_oidc_error(proxy, session, ctx, route, 500, "session creation failed").await?;
            return Ok(true);
        }
    };

    // Determine redirect target: use state param or redirect_url
    let redirect_target =
        extract_query_param(query, "state").unwrap_or_else(|| oidc_config.redirect_url.clone());

    let cookie_name = effective_cookie_name(oidc_config);

    // Send redirect with Set-Cookie
    let mut response = ResponseHeader::build(302, None)?;
    response.insert_header("location", &redirect_target)?;
    response.insert_header(
        "set-cookie",
        format!("{cookie_name}={signed_cookie}; Path=/; HttpOnly; SameSite=Lax"),
    )?;
    response.insert_header("content-length", "0")?;
    session
        .write_response_header(Box::new(response), true)
        .await?;

    cache_oidc_context(ctx, proxy, route, 302);
    Ok(true)
}

async fn redirect_to_authorization(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
    oidc_config: &OidcAuthConfig,
) -> pingora::Result<bool> {
    // Build the original request URL for the state parameter
    let host = session
        .req_header()
        .headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let path_and_query = session.req_header().uri.to_string();
    let scheme = if session.req_header().uri.scheme_str() == Some("https") {
        "https"
    } else {
        "http"
    };
    let original_url = format!("{scheme}://{host}{path_and_query}");

    // Build authorization URL
    let scopes = if oidc_config.scopes.is_empty() {
        "openid".to_string()
    } else {
        oidc_config.scopes.join(" ")
    };

    let auth_url = format!(
        "{}?client_id={}&response_type=code&scope={}&redirect_uri={}&state={}",
        oidc_config.provider_authorization_url,
        url_encode(&oidc_config.client_id),
        url_encode(&scopes),
        url_encode(&oidc_config.redirect_url),
        url_encode(&original_url),
    );

    let response = build_redirect_response(302, &auth_url)?;
    session
        .write_response_header(Box::new(response), true)
        .await?;

    cache_oidc_context(ctx, proxy, route, 302);
    Ok(true)
}

async fn exchange_code_for_tokens(
    oidc_config: &OidcAuthConfig,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenData, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", &oidc_config.client_id),
        ("client_secret", client_secret),
    ];

    let response = client
        .post(&oidc_config.provider_token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("token request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("token exchange returned {status}: {body}"));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("failed to parse token response: {e}"))?;

    // Decode access token to extract sub and exp claims
    let claims = decode_jwt_claims(&token_response.access_token)?;

    Ok(TokenData {
        access_token: token_response.access_token,
        exp: claims.exp_or_default(),
        sub: claims.sub,
    })
}

fn decode_jwt_claims(token: &str) -> Result<JwtClaims, String> {
    // JWT has three parts separated by dots: header.payload.signature
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err("invalid JWT format".to_string());
    }

    // Decode the payload (second part)
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("failed to decode JWT payload: {e}"))?;

    let claims: JwtClaims =
        serde_json::from_slice(&payload).map_err(|e| format!("failed to parse JWT claims: {e}"))?;

    Ok(claims)
}

fn extract_query_param(query: &str, param_name: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=')
            && key == param_name
        {
            return Some(url_decode(value));
        }
    }
    None
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(result)
}

fn verify_session(cookie_value: &str, signing_key: &str) -> bool {
    let parts: Vec<&str> = cookie_value.splitn(2, '.').collect();
    if parts.len() != 2 {
        return false;
    }

    let (payload_b64, signature_b64) = (parts[0], parts[1]);

    let payload = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let signature = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(signature_b64) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Verify HMAC signature
    let Ok(mut mac) = HmacSha256::new_from_slice(signing_key.as_bytes()) else {
        return false;
    };
    mac.update(&payload);
    if mac.verify_slice(&signature).is_err() {
        return false;
    }

    // Parse and check expiration
    let session: OidcSessionPayload = match serde_json::from_slice(&payload) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    session.exp > now
}

fn create_session_cookie(
    payload: &OidcSessionPayload,
    signing_key: &str,
) -> Result<String, String> {
    if signing_key.is_empty() {
        return Err("session signing key not found".to_string());
    }

    let payload_json =
        serde_json::to_vec(payload).map_err(|e| format!("failed to serialize session: {e}"))?;

    let mut mac = HmacSha256::new_from_slice(signing_key.as_bytes())
        .map_err(|e| format!("failed to create HMAC: {e}"))?;
    mac.update(&payload_json);
    let signature = mac.finalize().into_bytes();

    let encoded_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&payload_json);
    let encoded_signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature);

    Ok(format!("{encoded_payload}.{encoded_signature}"))
}

fn extract_cookie(request: &pingora::http::RequestHeader, cookie_name: &str) -> Option<String> {
    for (header_name, value) in request.headers.iter() {
        if !header_name.as_str().eq_ignore_ascii_case("cookie") {
            continue;
        }
        let cookie = value.to_str().ok()?;
        for item in cookie.split(';') {
            if let Some((name, val)) = item.trim().split_once('=')
                && name == cookie_name
            {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn effective_cookie_name(oidc_config: &OidcAuthConfig) -> &str {
    if oidc_config.session_cookie_name.is_empty() {
        "_ntgw_oidc_session"
    } else {
        &oidc_config.session_cookie_name
    }
}

async fn send_oidc_error(
    proxy: &GatewayProxy,
    session: &mut Session,
    ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
    status: u16,
    message: &str,
) -> pingora::Result<()> {
    let body = Bytes::from(format!("OIDC error: {message}"));
    let mut response = ResponseHeader::build(status, None)?;
    response.insert_header("content-type", "text/plain")?;
    response.insert_header("content-length", body.len().to_string())?;

    cache_oidc_context(ctx, proxy, route, status);
    session
        .write_response_header(Box::new(response), false)
        .await?;
    session.write_response_body(Some(body), true).await?;
    Ok(())
}

fn cache_oidc_context(
    ctx: &mut RequestContext,
    proxy: &GatewayProxy,
    route: &SelectedHttpRoute,
    status: u16,
) {
    use super::super::assign_ctx_string;
    use super::super::cache_selected_http_route_context;
    use super::super::record_request_span;

    cache_selected_http_route_context(ctx, &proxy.access_log, route);
    ctx.status = status;
    assign_ctx_string(&mut ctx.response_flags, "OC");
    record_request_span(ctx);
}

#[derive(serde::Serialize, serde::Deserialize)]
struct OidcSessionPayload {
    sub: String,
    exp: u64,
    access_token_hash: String,
}

#[derive(Debug)]
struct TokenData {
    access_token: String,
    sub: String,
    exp: u64,
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    expires_in: Option<u64>,
}

#[derive(serde::Deserialize)]
struct JwtClaims {
    sub: String,
    exp: Option<u64>,
}

impl JwtClaims {
    fn exp_or_default(&self) -> u64 {
        self.exp.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() + 3600)
                .unwrap_or(0)
        })
    }
}
