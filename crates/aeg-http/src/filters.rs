use pingora::{
    http::{RequestHeader, ResponseHeader},
    proxy::Session,
    Error, ErrorType,
};
use std::collections::BTreeMap;

use crate::extensions::extension_filters_supported;
use aeg_ir::{Filter, MatchedHttpPath, RequestMeta, RequestRedirectFilter};

mod cors;
mod headers;
mod redirect;

use self::{
    cors::{apply_cors_filter, is_cors_preflight_request},
    headers::apply_header_modifier,
    redirect::{
        apply_url_rewrite, redirect_authority, redirect_hostname, redirect_path_and_query,
        redirect_port, redirect_scheme, request_port, request_scheme,
    },
};

const INVALID_ROUTE_FILTER: ErrorType = ErrorType::new("InvalidRouteFilter");
const UNSUPPORTED_ROUTE_FILTER: ErrorType = ErrorType::new("UnsupportedRouteFilter");

pub(crate) fn request_redirect_filter(filters: &[Filter]) -> Option<&RequestRedirectFilter> {
    filters
        .iter()
        .find_map(|filter| filter.request_redirect.as_ref())
}

pub(crate) fn ensure_supported_filters(filters: &[Filter]) -> pingora::Result<()> {
    if filters
        .iter()
        .any(|filter| !is_supported_filter_type(&filter.filter_type))
    {
        return Err(Error::new(UNSUPPORTED_ROUTE_FILTER));
    }
    if !extension_filters_supported(filters) {
        return Err(Error::new(UNSUPPORTED_ROUTE_FILTER));
    }
    if filters.iter().any(|filter| {
        filter.external_auth.as_ref().is_some_and(|auth| {
            !(auth.protocol.eq_ignore_ascii_case("HTTP")
                || auth.protocol.eq_ignore_ascii_case("GRPC"))
        })
    }) {
        return Err(Error::new(UNSUPPORTED_ROUTE_FILTER));
    }

    Ok(())
}

pub(crate) fn build_redirect_response(
    status_code: u16,
    location: &str,
) -> pingora::Result<ResponseHeader> {
    let mut response = ResponseHeader::build(status_code, None)?;
    response.insert_header("location", location.to_string())?;
    response.insert_header("content-length", "0")?;
    Ok(response)
}

pub(crate) fn build_cors_preflight_response(
    filters: &[Filter],
    request_method: &str,
    request_headers: &BTreeMap<String, Vec<String>>,
) -> pingora::Result<Option<ResponseHeader>> {
    if !is_cors_preflight_request(Some(request_method), request_headers)
        || !filters.iter().any(|filter| filter.filter_type == "CORS")
    {
        return Ok(None);
    }

    let mut response = ResponseHeader::build(204, None)?;
    response.insert_header("content-length", "0")?;
    apply_response_filters(
        &mut response,
        filters,
        Some(request_method),
        Some(request_headers),
    )?;
    Ok(Some(response))
}

pub(crate) fn build_redirect_location(
    session: &Session,
    request: &RequestMeta,
    matched_http_path: &MatchedHttpPath,
    redirect: &RequestRedirectFilter,
) -> String {
    let request_header = session.req_header();
    let original_scheme = request_scheme(session);
    let scheme = redirect_scheme(original_scheme, redirect);
    let host = redirect_hostname(session, request, redirect);
    let port = redirect_port(
        &scheme,
        original_scheme,
        request_port(session, request),
        redirect,
    );
    let path_and_query = redirect_path_and_query(request_header, matched_http_path, redirect);
    let authority = redirect_authority(&scheme, &host, port);

    format!("{scheme}://{authority}{path_and_query}")
}

pub(crate) fn apply_request_filters(
    request: &mut RequestHeader,
    filters: &[Filter],
    matched_http_path: Option<&MatchedHttpPath>,
) -> pingora::Result<()> {
    for filter in filters {
        if let Some(rewrite) = &filter.url_rewrite {
            apply_url_rewrite(request, rewrite, matched_http_path)?;
            continue;
        }

        if filter.filter_type == "RequestHeaderModifier" {
            if let Some(modifier) = &filter.header_modifier {
                apply_header_modifier(request, modifier)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn apply_response_filters(
    response: &mut ResponseHeader,
    filters: &[Filter],
    request_method: Option<&str>,
    request_headers: Option<&BTreeMap<String, Vec<String>>>,
) -> pingora::Result<()> {
    for filter in filters {
        if filter.filter_type == "ResponseHeaderModifier" {
            if let Some(modifier) = &filter.header_modifier {
                apply_header_modifier(response, modifier)?;
            }
            continue;
        }

        if filter.filter_type == "CORS" {
            if let Some(cors) = &filter.cors {
                apply_cors_filter(response, cors, request_method, request_headers)?;
            }
        }
    }

    Ok(())
}

fn is_supported_filter_type(filter_type: &str) -> bool {
    matches!(
        filter_type,
        "RequestHeaderModifier"
            | "ResponseHeaderModifier"
            | "CORS"
            | "RequestRedirect"
            | "URLRewrite"
            | "RequestMirror"
            | "ExternalAuth"
            | "ExtensionRef"
    )
}

#[cfg(test)]
mod tests;
