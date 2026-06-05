use std::time::Duration;

use crate::{normalize_endpoint, ClientTlsOptions, TransportOptions};

#[test]
fn normalizes_plain_endpoint_with_tls_scheme() {
    assert_eq!(
        normalize_endpoint("controlplane.aether-gateway.svc:18080", true)
            .expect("endpoint should normalize"),
        "https://controlplane.aether-gateway.svc:18080"
    );
}

#[test]
fn upgrades_http_endpoint_when_tls_enabled() {
    assert_eq!(
        normalize_endpoint("http://127.0.0.1:18080", true).expect("endpoint should normalize"),
        "https://127.0.0.1:18080"
    );
}

#[test]
fn empty_endpoint_is_rejected() {
    assert!(normalize_endpoint("   ", false).is_err());
}

#[test]
fn tls_options_default_to_disabled_material() {
    let tls = ClientTlsOptions::default();
    assert!(tls.ca_path.is_empty());
    assert!(tls.cert_path.is_empty());
    assert!(tls.key_path.is_empty());
}

#[test]
fn transport_options_default_include_stale_stream_timeout() {
    assert_eq!(
        TransportOptions::default().stale_stream_timeout,
        Duration::from_secs(30)
    );
}
