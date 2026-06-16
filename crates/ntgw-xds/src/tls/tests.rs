use std::time::Duration;

use rustls::crypto::CryptoProvider;

use super::ensure_rustls_provider;
use crate::{ClientTlsOptions, TransportOptions, normalize_endpoint};

#[test]
fn normalizes_plain_endpoint_with_tls_scheme() {
    assert_eq!(
        normalize_endpoint("controlplane.nantian-gw.svc:18080", true)
            .expect("endpoint should normalize"),
        "https://controlplane.nantian-gw.svc:18080"
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

#[test]
fn ensure_rustls_provider_installs_default() {
    ensure_rustls_provider();
    assert!(CryptoProvider::get_default().is_some());
}

#[test]
fn ensure_rustls_provider_is_idempotent() {
    ensure_rustls_provider();
    let first = CryptoProvider::get_default()
        .expect("provider should be installed")
        .clone();
    ensure_rustls_provider();
    let second = CryptoProvider::get_default()
        .expect("provider should stay installed")
        .clone();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
}
