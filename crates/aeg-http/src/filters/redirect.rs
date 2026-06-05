use pingora::{http::RequestHeader, proxy::Session, Error};

use aeg_ir::{MatchedHttpPath, PathModifier, RequestMeta, RequestRedirectFilter, UrlRewriteFilter};

use super::INVALID_ROUTE_FILTER;

pub(super) fn redirect_scheme(original_scheme: &str, redirect: &RequestRedirectFilter) -> String {
    if !redirect.scheme.is_empty() {
        return redirect.scheme.clone();
    }

    original_scheme.to_string()
}

pub(super) fn request_scheme(session: &Session) -> &'static str {
    if session
        .as_downstream()
        .digest()
        .and_then(|digest| digest.ssl_digest.as_ref())
        .is_some()
    {
        "https"
    } else {
        "http"
    }
}

pub(super) fn redirect_hostname(
    session: &Session,
    request: &RequestMeta,
    redirect: &RequestRedirectFilter,
) -> String {
    if !redirect.hostname.is_empty() {
        return redirect.hostname.clone();
    }

    if let Some(host) = &request.host {
        return host.clone();
    }

    session
        .as_downstream()
        .server_addr()
        .map(|addr| strip_port(&addr.to_string()))
        .unwrap_or_else(|| "localhost".to_string())
}

pub(super) fn request_port(session: &Session, request: &RequestMeta) -> u32 {
    if request.port != 0 {
        return request.port;
    }

    session
        .as_downstream()
        .server_addr()
        .and_then(|addr| addr.as_inet().map(|inet| inet.port()))
        .unwrap_or_default() as u32
}

pub(super) fn redirect_port(
    target_scheme: &str,
    original_scheme: &str,
    original_port: u32,
    redirect: &RequestRedirectFilter,
) -> u32 {
    if redirect.port != 0 {
        return redirect.port;
    }

    if original_port != 0 && target_scheme.eq_ignore_ascii_case(original_scheme) {
        return original_port;
    }

    default_port_for_scheme(target_scheme).unwrap_or(original_port)
}

pub(super) fn redirect_path_and_query(
    request_header: &RequestHeader,
    matched_http_path: &MatchedHttpPath,
    redirect: &RequestRedirectFilter,
) -> String {
    let path = request_header.uri.path();
    let query = request_header.uri.query();
    match &redirect.path {
        Some(modifier) => rewrite_path_and_query(path, query, modifier, Some(matched_http_path)),
        None => request_header
            .uri
            .path_and_query()
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| path.to_string()),
    }
}

pub(super) fn redirect_authority(scheme: &str, host: &str, port: u32) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };

    if (scheme.eq_ignore_ascii_case("http") && port == 80)
        || (scheme.eq_ignore_ascii_case("https") && port == 443)
        || port == 0
    {
        host
    } else {
        format!("{host}:{port}")
    }
}

pub(super) fn rewrite_path_and_query(
    path: &str,
    query: Option<&str>,
    modifier: &PathModifier,
    matched_http_path: Option<&MatchedHttpPath>,
) -> String {
    let rewritten_path = match modifier.modifier_type.as_str() {
        "ReplaceFullPath" if !modifier.replace_full_path.is_empty() => {
            modifier.replace_full_path.clone()
        }
        "ReplacePrefixMatch" => rewrite_prefix_path(
            path,
            matched_http_path
                .map(|item| item.path.as_str())
                .unwrap_or("/"),
            &modifier.replace_prefix_match,
        ),
        _ => path.to_string(),
    };

    match query {
        Some(query) if !query.is_empty() => format!("{rewritten_path}?{query}"),
        _ => rewritten_path,
    }
}

pub(super) fn apply_url_rewrite(
    request: &mut RequestHeader,
    rewrite: &UrlRewriteFilter,
    matched_http_path: Option<&MatchedHttpPath>,
) -> pingora::Result<()> {
    if !rewrite.hostname.is_empty() {
        request.insert_header("host".to_string(), rewrite.hostname.clone())?;
    }

    if let Some(modifier) = &rewrite.path {
        let rewritten = rewrite_path_and_query(
            request.uri.path(),
            request.uri.query(),
            modifier,
            matched_http_path,
        );
        request
            .set_raw_path(rewritten.as_bytes())
            .map_err(|err| Error::because(INVALID_ROUTE_FILTER, "invalid rewritten uri", err))?;
    }

    Ok(())
}

fn default_port_for_scheme(scheme: &str) -> Option<u32> {
    if scheme.eq_ignore_ascii_case("http") {
        Some(80)
    } else if scheme.eq_ignore_ascii_case("https") {
        Some(443)
    } else {
        None
    }
}

fn strip_port(host: &str) -> String {
    if host.starts_with('[') {
        return host
            .split_once(']')
            .map(|(value, _)| format!("{value}]"))
            .unwrap_or_else(|| host.to_string());
    }

    host.split(':').next().unwrap_or(host).to_string()
}

fn rewrite_prefix_path(path: &str, matched_prefix: &str, replacement: &str) -> String {
    let normalized_prefix = normalize_prefix(matched_prefix);
    let Some(remainder) = path_after_prefix(path, normalized_prefix) else {
        return path.to_string();
    };

    if replacement.is_empty() {
        return if remainder.is_empty() {
            "/".to_string()
        } else if remainder.starts_with('/') {
            remainder.to_string()
        } else {
            format!("/{remainder}")
        };
    }

    if remainder.is_empty() {
        return normalize_replacement(replacement);
    }

    let replacement = normalize_replacement(replacement);
    if replacement == "/" {
        remainder.to_string()
    } else if remainder == "/" {
        format!("{replacement}/")
    } else {
        format!("{replacement}{remainder}")
    }
}

fn normalize_prefix(prefix: &str) -> &str {
    if prefix == "/" {
        return prefix;
    }

    prefix.trim_end_matches('/')
}

fn path_after_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix == "/" {
        return Some(path);
    }

    if path == prefix {
        return Some("");
    }

    path.strip_prefix(prefix)
}

fn normalize_replacement(replacement: &str) -> String {
    if replacement.is_empty() {
        "/".to_string()
    } else if replacement.starts_with('/') {
        replacement.trim_end_matches('/').to_string().if_empty("/")
    } else {
        format!("/{}", replacement.trim_end_matches('/')).if_empty("/")
    }
}

trait StringExt {
    fn if_empty(self, fallback: &str) -> String;
}

impl StringExt for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}
