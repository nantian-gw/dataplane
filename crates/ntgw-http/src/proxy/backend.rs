use super::*;
#[cfg(test)]
use ntgw_ir::Snapshot;
use pingora::utils::tls::CertKey;
use std::sync::Arc;
use std::time::Duration;

mod client_cert;
mod tls_validation;

pub(crate) use self::client_cert::resolve_backend_client_cert_key;

#[derive(Debug, Clone, Default)]
pub struct UpstreamTuningOptions {
    pub tcp_fast_open: bool,
    pub tcp_recv_buf: Option<usize>,
    pub connection_timeout: Option<Duration>,
    pub read_timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub dscp: Option<u8>,
}
use self::tls_validation::apply_cached_backend_tls_validation;
#[cfg(test)]
pub(crate) use self::tls_validation::{
    backend_certificate_matches_subject_alt_names, backend_tls_service_name,
};
pub(crate) use self::tls_validation::{
    backend_tls_sni_name, resolve_backend_tls_validation,
    validate_backend_tls_subject_alt_name_result,
};
use super::context::{SelectedBackendConfig, UpstreamPeerAddress};
#[cfg(test)]
use super::selection::selected_backend_config_with_overrides;

#[cfg(test)]
pub(crate) fn build_upstream_peer(
    snapshot: &Snapshot,
    endpoint: &SelectedBackend,
    protocol: Option<&str>,
    policy: Option<&BackendPolicy>,
) -> pingora::Result<HttpPeer> {
    let config = selected_backend_config_with_overrides(snapshot, endpoint, protocol, policy)?;
    build_upstream_peer_with_keepalive(snapshot, endpoint, &config, None)
}

pub(crate) fn build_upstream_peer_with_cached_config(
    endpoint: &SelectedBackend,
    config: &SelectedBackendConfig,
    tcp_keepalive: Option<pingora::protocols::l4::ext::TcpKeepalive>,
    tuning: &UpstreamTuningOptions,
) -> pingora::Result<HttpPeer> {
    build_upstream_peer_from_cached_parts(
        &endpoint.backend_name,
        &endpoint.backend,
        config,
        tcp_keepalive,
        tuning,
    )
}

pub(crate) fn build_upstream_peer_for_fast_path(
    selected: &ntgw_ir::CompiledSelectedHttpBackend,
    config: &SelectedBackendConfig,
    tcp_keepalive: Option<pingora::protocols::l4::ext::TcpKeepalive>,
    tuning: &UpstreamTuningOptions,
) -> pingora::Result<HttpPeer> {
    build_upstream_peer_from_cached_parts(
        &selected.backend_name,
        &selected.backend,
        config,
        tcp_keepalive,
        tuning,
    )
}

#[cfg(test)]
pub(crate) fn build_upstream_peer_with_keepalive(
    _snapshot: &Snapshot,
    endpoint: &SelectedBackend,
    config: &SelectedBackendConfig,
    tcp_keepalive: Option<pingora::protocols::l4::ext::TcpKeepalive>,
) -> pingora::Result<HttpPeer> {
    build_upstream_peer_from_cached_parts(
        &endpoint.backend_name,
        &endpoint.backend,
        config,
        tcp_keepalive,
        &UpstreamTuningOptions::default(),
    )
}

fn build_upstream_peer_from_cached_parts(
    _backend_name: &str,
    _backend: &ntgw_ir::BackendEndpoint,
    config: &SelectedBackendConfig,
    tcp_keepalive: Option<pingora::protocols::l4::ext::TcpKeepalive>,
    tuning: &UpstreamTuningOptions,
) -> pingora::Result<HttpPeer> {
    let mut peer = if config.tls_enabled {
        if let Some(client_cert_key) = config.client_cert_key.clone() {
            new_http_peer(
                &config.peer_address,
                config.peer_port,
                true,
                config.sni.clone(),
                Some(client_cert_key),
            )
        } else {
            new_http_peer(
                &config.peer_address,
                config.peer_port,
                true,
                config.sni.clone(),
                None,
            )
        }
    } else {
        new_http_peer(
            &config.peer_address,
            config.peer_port,
            false,
            String::new(),
            None,
        )
    };

    apply_backend_protocol_config(&mut peer, config.use_http2);
    apply_cached_backend_tls_validation(&mut peer, config.backend_tls_validation.as_ref());
    apply_precomputed_backend_policy(&mut peer, config);
    peer.options.tcp_keepalive = tcp_keepalive;
    if !peer.options.tcp_fast_open {
        peer.options.tcp_fast_open = tuning.tcp_fast_open;
    }
    if peer.options.tcp_recv_buf.is_none() {
        peer.options.tcp_recv_buf = tuning.tcp_recv_buf;
    }
    if peer.options.connection_timeout.is_none() {
        peer.options.connection_timeout = tuning.connection_timeout;
    }
    if peer.options.read_timeout.is_none() {
        peer.options.read_timeout = tuning.read_timeout;
    }
    if peer.options.idle_timeout.is_none() {
        peer.options.idle_timeout = tuning.idle_timeout;
    }
    if peer.options.dscp.is_none() {
        peer.options.dscp = tuning.dscp;
    }

    Ok(peer)
}

fn new_http_peer(
    address: &UpstreamPeerAddress,
    port: u16,
    tls_enabled: bool,
    sni: String,
    client_cert_key: Option<Arc<CertKey>>,
) -> HttpPeer {
    match (address, client_cert_key) {
        (UpstreamPeerAddress::Ip(ip_addr), Some(client_cert_key)) => {
            HttpPeer::new_mtls((*ip_addr, port), sni, client_cert_key)
        }
        (UpstreamPeerAddress::Ip(ip_addr), None) => {
            HttpPeer::new((*ip_addr, port), tls_enabled, sni)
        }
        (UpstreamPeerAddress::Host(host), Some(client_cert_key)) => {
            HttpPeer::new_mtls((host.as_str(), port), sni, client_cert_key)
        }
        (UpstreamPeerAddress::Host(host), None) => {
            HttpPeer::new((host.as_str(), port), tls_enabled, sni)
        }
    }
}

#[cfg(test)]
pub(crate) fn apply_backend_protocol(
    peer: &mut HttpPeer,
    route_kind: &RouteKind,
    protocol: Option<&str>,
) {
    if matches!(route_kind, RouteKind::Grpc) || is_http2_backend_protocol(protocol) {
        apply_backend_protocol_config(peer, true);
    }
}

fn apply_backend_protocol_config(peer: &mut HttpPeer, use_http2: bool) {
    if use_http2 {
        peer.options.set_http_version(2, 2);
        peer.options.max_h2_streams = DEFAULT_MAX_H2_UPSTREAM_STREAMS;
    }
}

#[cfg(test)]
pub(crate) fn apply_backend_policy(
    peer: &mut HttpPeer,
    policy: Option<&BackendPolicy>,
    route_timeouts: Option<&RouteTimeouts>,
) {
    let Some(policy) = policy else {
        if let Some(timeout) = effective_request_timeout(None, route_timeouts) {
            peer.options.read_timeout = Some(timeout);
            peer.options.write_timeout = Some(timeout);
        }
        return;
    };

    peer.options.connection_timeout = policy.connect_timeout;
    peer.options.total_connection_timeout = policy.connect_timeout;
    let request_timeout = effective_request_timeout(Some(policy), route_timeouts);
    peer.options.read_timeout = request_timeout;
    peer.options.write_timeout = request_timeout;
}

pub(crate) fn effective_request_timeout(
    policy: Option<&BackendPolicy>,
    route_timeouts: Option<&RouteTimeouts>,
) -> Option<std::time::Duration> {
    if let Some(route_timeout) = route_timeouts.and_then(route_request_timeout) {
        return (!route_timeout.is_zero()).then_some(route_timeout);
    }

    policy
        .and_then(|item| item.request_timeout)
        .filter(|timeout| !timeout.is_zero())
}

fn apply_precomputed_backend_policy(peer: &mut HttpPeer, config: &SelectedBackendConfig) {
    peer.options.connection_timeout = config.connect_timeout;
    peer.options.total_connection_timeout = config.connect_timeout;
    peer.options.read_timeout = config.request_timeout;
    peer.options.write_timeout = config.request_timeout;
}

pub(crate) fn route_request_timeout(timeouts: &RouteTimeouts) -> Option<std::time::Duration> {
    timeouts.backend_request.or(timeouts.request)
}

pub(crate) fn is_http2_backend_protocol(protocol: Option<&str>) -> bool {
    matches!(
        protocol.unwrap_or_default().to_ascii_uppercase().as_str(),
        "GRPC" | "GRPCS" | "H2C" | "HTTP2" | "HTTP/2"
    )
}

pub(crate) fn is_tls_backend_protocol(protocol: Option<&str>) -> bool {
    matches!(
        protocol.unwrap_or_default().to_ascii_uppercase().as_str(),
        "HTTPS" | "GRPCS"
    )
}

pub(crate) fn error_for_backend_selection(error: BackendSelectionError) -> Box<Error> {
    match error {
        BackendSelectionError::InvalidBackendRefs => {
            Error::new(ErrorType::new("InvalidBackendRefs"))
        }
        BackendSelectionError::NoHealthyBackends => Error::new(ErrorType::new("NoHealthyBackend")),
    }
}
