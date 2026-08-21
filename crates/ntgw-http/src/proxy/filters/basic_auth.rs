use base64::Engine;
use pingora::http::ResponseHeader;
use pingora::prelude::Session;

use super::super::{GatewayProxy, RequestContext, SelectedHttpRoute};

/// Handles HTTP Basic Authentication for a selected HTTP route.
/// Returns `Ok(true)` if the request was handled (early return, e.g. denied),
/// `Ok(false)` if processing should continue past BasicAuth validation.
pub(super) async fn handle_basic_auth(
    proxy: &GatewayProxy,
    session: &mut Session,
    _ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
) -> pingora::Result<bool> {
    let auth_config = match route
        .security_policy
        .as_ref()
        .and_then(|sp| sp.authn.as_ref())
        .and_then(|an| an.basic_auth.as_ref())
    {
        Some(config) => config,
        None => return Ok(false),
    };

    let realm = if auth_config.realm.is_empty() {
        "Restricted"
    } else {
        &auth_config.realm
    };

    let header_value = match session.req_header().headers.get("authorization") {
        Some(v) => v.to_str().unwrap_or(""),
        None => {
            send_401(session, realm).await?;
            return Ok(true);
        }
    };

    if !header_value.starts_with("Basic ") {
        send_401(session, realm).await?;
        return Ok(true);
    }

    let encoded = &header_value[6..];
    let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded) {
        Ok(d) => d,
        Err(_) => {
            send_401(session, realm).await?;
            return Ok(true);
        }
    };

    let decoded_str = match std::str::from_utf8(&decoded) {
        Ok(s) => s,
        Err(_) => {
            send_401(session, realm).await?;
            return Ok(true);
        }
    };

    let (user, password) = match decoded_str.split_once(':') {
        Some((u, p)) => (u, p),
        None => {
            send_401(session, realm).await?;
            return Ok(true);
        }
    };

    // Load htpasswd from secrets
    let snap = proxy.snapshot.load();
    let htpasswd_content = snap
        .secrets
        .iter()
        .find(|s| s.name == auth_config.htpasswd_ref)
        .map(|s| s.htpasswd.as_str())
        .unwrap_or("");
    if htpasswd_content.is_empty() {
        send_401(session, realm).await?;
        return Ok(true);
    }

    // Parse htpasswd lines: each line is "user:hash" (bcrypt hash)
    for line in htpasswd_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((stored_user, stored_hash)) = line.split_once(':')
            && stored_user == user
        {
            match bcrypt::verify(password, stored_hash) {
                Ok(true) => return Ok(false),
                _ => continue,
            }
        }
    }

    send_401(session, realm).await?;
    Ok(true)
}

async fn send_401(session: &mut Session, realm: &str) -> pingora::Result<()> {
    let mut response = ResponseHeader::build(401, None)?;
    response.insert_header("www-authenticate", format!("Basic realm=\"{realm}\""))?;
    response.insert_header("content-length", "0")?;
    session
        .write_response_header(Box::new(response), false)
        .await?;
    session.write_response_body(None, true).await?;
    Ok(())
}
