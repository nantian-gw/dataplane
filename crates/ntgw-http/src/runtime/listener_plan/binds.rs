use std::collections::BTreeSet;

use ntgw_ir::Listener;
use pingora::listeners::TcpSocketOptions;
use pingora::protocols::l4::ext::TcpKeepalive;

use super::super::{LISTENER_ADDRESSES_METADATA_KEY, RuntimeOptions};

pub(in crate::runtime) fn listener_bind_addrs(
    listener: &Listener,
    runtime: &RuntimeOptions,
) -> Vec<String> {
    let mut binds = BTreeSet::new();
    for host in listener_bind_hosts(listener, runtime) {
        for bind in bind_addrs(host.as_str(), listener.port, runtime.enable_ipv6) {
            binds.insert(bind);
        }
    }
    binds.into_iter().collect()
}

fn listener_bind_hosts(listener: &Listener, runtime: &RuntimeOptions) -> Vec<String> {
    let default_host = default_bind_host(runtime.default_listen_addr.as_str());
    let configured = listener_configured_addresses(listener);
    configured
        .into_iter()
        .map(|host| match default_host {
            Some(default_host) => listener_bind_host(host.as_str(), default_host).to_string(),
            None if host.is_empty() => "0.0.0.0".to_string(),
            None => host,
        })
        .collect()
}

fn listener_configured_addresses(listener: &Listener) -> Vec<String> {
    if !listener.addresses.is_empty() {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for value in listener
            .addresses
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if seen.insert(value.to_string()) {
                out.push(value.to_string());
            }
        }
        if !out.is_empty() {
            return out;
        }
    }

    if let Some(raw) = listener.metadata.get(LISTENER_ADDRESSES_METADATA_KEY) {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for value in raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if seen.insert(value.to_string()) {
                out.push(value.to_string());
            }
        }
        if !out.is_empty() {
            return out;
        }
    }

    if !listener.address.is_empty() {
        return vec![listener.address.clone()];
    }

    vec!["0.0.0.0".to_string()]
}

pub(in crate::runtime) fn bind_addrs(address: &str, port: u32, enable_ipv6: bool) -> Vec<String> {
    let host = if address.is_empty() {
        "0.0.0.0"
    } else {
        address
    };

    if enable_ipv6 && host == "0.0.0.0" {
        return vec![format!("0.0.0.0:{port}"), format!("[::]:{port}")];
    }

    vec![socket_addr(host, port)]
}

pub(in crate::runtime) fn bind_variants(bind: &str, enable_ipv6: bool) -> Vec<String> {
    if !enable_ipv6 {
        return vec![bind.to_string()];
    }

    if let Some(port) = bind.strip_prefix("0.0.0.0:") {
        return vec![bind.to_string(), format!("[::]:{port}")];
    }

    vec![bind.to_string()]
}

pub(in crate::runtime) fn tcp_socket_options_for_bind(
    bind: &str,
    tcp_keepalive: Option<TcpKeepalive>,
    reuse_port: Option<bool>,
    tcp_fastopen: Option<usize>,
    dscp: Option<u8>,
) -> TcpSocketOptions {
    let mut options = TcpSocketOptions::default();
    if bind.starts_with('[') {
        options.ipv6_only = Some(true);
    }
    options.tcp_keepalive = tcp_keepalive;
    options.so_reuseport = reuse_port;
    options.tcp_fastopen = tcp_fastopen;
    options.dscp = dscp;
    options
}

fn socket_addr(address: &str, port: u32) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

fn default_bind_host(bind: &str) -> Option<&str> {
    if bind.is_empty() {
        return None;
    }

    if let Some(bind) = bind.strip_prefix('[') {
        return bind.split_once("]:").map(|(host, _)| host);
    }

    bind.rsplit_once(':').map(|(host, _)| host)
}

fn listener_bind_host<'a>(listener_host: &'a str, default_host: &'a str) -> &'a str {
    if listener_host.is_empty() || is_wildcard_host(listener_host) {
        return default_host;
    }
    listener_host
}

fn is_wildcard_host(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "::" | "[::]")
}

pub(in crate::runtime) fn is_l7_protocol(protocol: &str) -> bool {
    is_plain_http_protocol(protocol) || is_https_protocol(protocol) || is_http3_protocol(protocol)
}

pub(in crate::runtime) fn is_plain_http_protocol(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTP" | "LISTENER_PROTOCOL_GRPC" | "HTTP" | "GRPC"
    )
}

pub(in crate::runtime) fn is_https_protocol(protocol: &str) -> bool {
    matches!(protocol, "LISTENER_PROTOCOL_HTTPS" | "HTTPS")
}

pub(in crate::runtime) fn is_http3_protocol(protocol: &str) -> bool {
    matches!(protocol, "LISTENER_PROTOCOL_HTTP3" | "HTTP3")
}
