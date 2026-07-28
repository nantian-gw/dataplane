use std::{collections::BTreeMap, net::IpAddr};

use http::header::CONTENT_TYPE;
use pingora::http::RequestHeader;
use pingora::prelude::Session;

pub(crate) fn request_host_value(req: &RequestHeader) -> Option<&str> {
    req.headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .or_else(|| req.uri.authority().map(|authority| authority.as_str()))
}

pub(crate) fn client_ip(session: &Session) -> Option<String> {
    session
        .client_addr()
        .or_else(|| {
            session
                .as_downstream()
                .digest()
                .and_then(|digest| digest.socket_digest.as_ref())
                .and_then(|socket| socket.peer_addr())
        })
        .map(|addr| {
            addr.as_inet()
                .map(|inet| normalize_ip(inet.ip()))
                .unwrap_or_else(|| addr.to_string())
        })
}

pub(crate) fn normalize_ip(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(ipv4) => ipv4.to_string(),
        IpAddr::V6(ipv6) => ipv6
            .to_ipv4_mapped()
            .map(|ipv4| ipv4.to_string())
            .unwrap_or_else(|| ipv6.to_string()),
    }
}

pub(crate) fn request_headers(req: &RequestHeader) -> BTreeMap<String, Vec<String>> {
    req.headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .fold(BTreeMap::new(), |mut acc, (name, value)| {
            acc.entry(name).or_default().push(value);
            acc
        })
}

pub(crate) fn grpc_content_type_headers(req: &RequestHeader) -> BTreeMap<String, Vec<String>> {
    let values = req
        .headers
        .get_all(CONTENT_TYPE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter(|value| value.starts_with("application/grpc"))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if values.is_empty() {
        BTreeMap::new()
    } else {
        BTreeMap::from([("content-type".to_string(), values)])
    }
}

pub(crate) fn request_header_bytes_from_header(req: &RequestHeader) -> usize {
    req.headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| name.as_str().len().saturating_add(value.len()))
        })
        .sum()
}

pub(crate) fn request_id_from_headers(headers: &BTreeMap<String, Vec<String>>) -> &str {
    [
        "x-request-id",
        "x-correlation-id",
        "traceparent",
        "grpc-trace-bin",
    ]
    .into_iter()
    .find_map(|name| headers.get(name).and_then(|values| values.first()))
    .map(|value| value.as_str())
    .unwrap_or_default()
}

pub(crate) fn request_id_from_header(req: &RequestHeader) -> &str {
    [
        "x-request-id",
        "x-correlation-id",
        "traceparent",
        "grpc-trace-bin",
    ]
    .into_iter()
    .find_map(|name| req.headers.get(name).and_then(|value| value.to_str().ok()))
    .unwrap_or_default()
}

pub(crate) fn request_content_length(headers: &BTreeMap<String, Vec<String>>) -> usize {
    headers
        .get("content-length")
        .and_then(|values| values.first())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default()
}

pub(crate) fn request_content_length_from_header(req: &RequestHeader) -> usize {
    req.headers
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default()
}
