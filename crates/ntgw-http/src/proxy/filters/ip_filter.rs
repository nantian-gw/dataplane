use std::net::IpAddr;

use pingora::http::ResponseHeader;
use pingora::prelude::Session;

use super::super::{GatewayProxy, RequestContext, SelectedHttpRoute};

/// Handles IP-based access control for a selected HTTP route.
/// Returns `Ok(true)` if the request was denied (403),
/// `Ok(false)` if the IP is allowed and processing should continue.
pub(super) async fn handle_ip_filter(
    _proxy: &GatewayProxy,
    session: &mut Session,
    _ctx: &mut RequestContext,
    route: &SelectedHttpRoute,
) -> pingora::Result<bool> {
    let ip_config = match route.security_policy.as_ref().and_then(|sp| sp.ip.as_ref()) {
        Some(config) => config,
        None => return Ok(false),
    };

    // If both allow and deny lists are empty, skip IP filtering
    if ip_config.allow_cidrs.is_empty() && ip_config.deny_cidrs.is_empty() {
        return Ok(false);
    }

    // Extract client IP from the session
    let client_ip_str = match session.client_addr() {
        Some(addr) => addr.to_string(),
        None => {
            // If we can't determine the client IP, deny the request
            send_403(session, "IP not allowed").await?;
            return Ok(true);
        }
    };

    // Parse the client IP (strip port if present)
    let ip_str = if let Some(pos) = client_ip_str.rfind(':') {
        &client_ip_str[..pos]
    } else {
        &client_ip_str
    };

    let client_ip: IpAddr = match ip_str.parse() {
        Ok(ip) => ip,
        Err(_) => {
            // If we can't parse the IP, deny the request
            send_403(session, "IP not allowed").await?;
            return Ok(true);
        }
    };

    // Check allow list first (allow takes precedence over deny)
    if !ip_config.allow_cidrs.is_empty() {
        let mut allowed = false;
        for cidr_str in &ip_config.allow_cidrs {
            if let Ok(cidr) = cidr_str.parse::<ipnet::IpNet>()
                && cidr.contains(&client_ip)
            {
                allowed = true;
                break;
            }
        }
        if !allowed {
            send_403(session, "IP not allowed").await?;
            return Ok(true);
        }
    }

    // Check deny list
    for cidr_str in &ip_config.deny_cidrs {
        if let Ok(cidr) = cidr_str.parse::<ipnet::IpNet>()
            && cidr.contains(&client_ip)
        {
            send_403(session, "IP not allowed").await?;
            return Ok(true);
        }
    }

    Ok(false)
}

async fn send_403(session: &mut Session, message: &str) -> pingora::Result<()> {
    let body = bytes::Bytes::from(message.to_string());
    let mut response = ResponseHeader::build(403, None)?;
    response.insert_header("content-type", "text/plain")?;
    response.insert_header("content-length", body.len().to_string())?;
    session
        .write_response_header(Box::new(response), false)
        .await?;
    session.write_response_body(Some(body), true).await?;
    Ok(())
}
