use std::collections::BTreeMap;

use pingora::http::ResponseHeader;

use ntgw_ir::CorsFilter;

pub(super) fn apply_cors_filter(
    response: &mut ResponseHeader,
    cors: &CorsFilter,
    request_method: Option<&str>,
    request_headers: Option<&BTreeMap<String, Vec<String>>>,
) -> pingora::Result<()> {
    let Some(request_headers) = request_headers else {
        return Ok(());
    };
    let Some(origin) = request_headers
        .get("origin")
        .and_then(|values| values.first())
    else {
        return Ok(());
    };

    let Some(allow_origin) = cors_allow_origin(cors, origin, request_headers) else {
        return Ok(());
    };

    response.insert_header("access-control-allow-origin", allow_origin)?;
    if allow_origin != "*" {
        response.append_header("vary", "Origin").map(|_| ())?;
    }
    if cors.allow_credentials {
        response.insert_header("access-control-allow-credentials", "true")?;
    } else {
        response.remove_header("access-control-allow-credentials");
    }
    if !cors.expose_headers.is_empty() {
        response.insert_header(
            "access-control-expose-headers",
            join_header_values(&cors.expose_headers),
        )?;
    } else {
        response.remove_header("access-control-expose-headers");
    }
    if is_cors_preflight_request(request_method, request_headers) {
        if let Some(value) = cors_allow_methods(cors, request_headers) {
            response.insert_header("access-control-allow-methods", value)?;
        } else {
            response.remove_header("access-control-allow-methods");
        }
        if let Some(value) = cors_allow_headers(cors, request_headers) {
            response.insert_header("access-control-allow-headers", value)?;
        } else {
            response.remove_header("access-control-allow-headers");
        }
        if let Some(max_age) = cors.max_age {
            response.insert_header("access-control-max-age", max_age.to_string())?;
        } else {
            response.remove_header("access-control-max-age");
        }
    } else {
        response.remove_header("access-control-allow-methods");
        response.remove_header("access-control-allow-headers");
        response.remove_header("access-control-max-age");
    }

    Ok(())
}

pub(super) fn is_cors_preflight_request(
    request_method: Option<&str>,
    request_headers: &BTreeMap<String, Vec<String>>,
) -> bool {
    request_method.is_some_and(|method| method.eq_ignore_ascii_case("OPTIONS"))
        && cors_request_header(request_headers, "access-control-request-method").is_some()
}

fn cors_request_header<'a>(
    request_headers: &'a BTreeMap<String, Vec<String>>,
    name: &str,
) -> Option<&'a str> {
    request_headers
        .get(name)
        .and_then(|values| values.first())
        .map(|value| value.as_str())
        .filter(|value| !value.is_empty())
}

fn cors_allow_origin<'a>(
    cors: &'a CorsFilter,
    origin: &'a str,
    request_headers: &BTreeMap<String, Vec<String>>,
) -> Option<&'a str> {
    if cors.allow_origins.iter().any(|allowed| allowed == "*") {
        if cors.allow_credentials || request_has_credentials(request_headers) {
            return Some(origin);
        }
        return Some("*");
    }

    cors.allow_origins
        .iter()
        .any(|allowed| origin_matches(allowed, origin))
        .then_some(origin)
}

fn cors_allow_methods(
    cors: &CorsFilter,
    request_headers: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    if cors.allow_methods.is_empty() {
        return None;
    }
    if cors.allow_credentials && cors.allow_methods.iter().any(|method| method == "*") {
        return cors_request_header(request_headers, "access-control-request-method")
            .map(ToOwned::to_owned);
    }
    Some(join_header_values(&cors.allow_methods))
}

fn cors_allow_headers(
    cors: &CorsFilter,
    request_headers: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    if cors.allow_headers.is_empty() {
        return None;
    }
    if cors.allow_credentials && cors.allow_headers.iter().any(|header| header == "*") {
        return cors_request_header(request_headers, "access-control-request-headers")
            .map(ToOwned::to_owned);
    }
    Some(join_header_values(&cors.allow_headers))
}

fn request_has_credentials(request_headers: &BTreeMap<String, Vec<String>>) -> bool {
    ["cookie", "authorization", "proxy-authorization"]
        .into_iter()
        .any(|name| cors_request_header(request_headers, name).is_some())
}

fn origin_matches(allowed: &str, origin: &str) -> bool {
    if allowed.eq_ignore_ascii_case(origin) {
        return true;
    }

    let Some(allowed) = ParsedOrigin::parse(allowed) else {
        return false;
    };
    let Some(origin) = ParsedOrigin::parse(origin) else {
        return false;
    };
    if !allowed.scheme.eq_ignore_ascii_case(origin.scheme) || allowed.port != origin.port {
        return false;
    }

    host_pattern_matches(allowed.host, origin.host)
}

struct ParsedOrigin<'a> {
    scheme: &'a str,
    host: &'a str,
    port: u16,
}

impl<'a> ParsedOrigin<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let (scheme, authority) = value.split_once("://")?;
        if !matches_ignore_ascii_case(scheme, "http") && !matches_ignore_ascii_case(scheme, "https")
        {
            return None;
        }
        if authority.is_empty() || authority.contains('/') || authority.contains('?') {
            return None;
        }

        let (host, port) = split_host_port(authority, default_port(scheme)?)?;
        if host.is_empty() {
            return None;
        }

        Some(Self { scheme, host, port })
    }
}

fn split_host_port(authority: &str, default_port: u16) -> Option<(&str, u16)> {
    let Some((host, port)) = authority.rsplit_once(':') else {
        return Some((authority, default_port));
    };
    if host.is_empty() || port.is_empty() || !port.bytes().all(|item| item.is_ascii_digit()) {
        return Some((authority, default_port));
    }

    let port = port.parse::<u16>().ok()?;
    Some((host, port))
}

fn default_port(scheme: &str) -> Option<u16> {
    if matches_ignore_ascii_case(scheme, "http") {
        Some(80)
    } else if matches_ignore_ascii_case(scheme, "https") {
        Some(443)
    } else {
        None
    }
}

fn host_pattern_matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern.eq_ignore_ascii_case(host);
    }

    let pattern = pattern.to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return false;
    };
    host.starts_with(prefix) && host.ends_with(suffix) && host.len() > prefix.len() + suffix.len()
}

fn matches_ignore_ascii_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn join_header_values(values: &[String]) -> String {
    values.join(", ")
}
