use crate::{
    SharedTlsError,
    listener_plan::{SharedTlsIdentity, TerminateSurface},
};
use async_trait::async_trait;
use ntgw_http::DownstreamTlsInfo;
use pingora::{
    listeners::TlsAccept,
    protocols::{l4::stream::Stream as L4Stream, tls::server::handshake_with_callback},
    tls::{
        ext,
        pkey::PKey,
        ssl::{
            AlpnError, SslAcceptor, SslAcceptorBuilder, SslMethod, SslRef, SslSessionCacheMode,
            SslVerifyMode, select_next_proto,
        },
        x509::X509,
    },
};
use std::{any::Any, sync::Arc};
use tracing::warn;

const SHARED_TLS_ALPN_H2_H1: &[u8] = b"\x02h2\x08http/1.1";

pub(super) async fn terminate_tls(
    downstream: L4Stream,
    terminate: &TerminateSurface,
) -> Result<pingora::protocols::tls::SslStream<L4Stream>, SharedTlsError> {
    let acceptor = build_tls_acceptor(terminate)?;
    let callbacks: pingora::listeners::TlsAcceptCallbacks =
        Box::new(DynamicTlsCertificates::new(terminate.identities.clone()));
    Ok(handshake_with_callback(&acceptor, downstream, &callbacks).await?)
}

fn build_tls_acceptor(terminate: &TerminateSurface) -> Result<SslAcceptor, SharedTlsError> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls())
        .map_err(|e| SharedTlsError::Certificate(format!("build shared tls acceptor: {e}")))?;
    configure_alpn(&mut builder);
    builder.set_session_cache_mode(SslSessionCacheMode::SERVER);
    configure_frontend_client_validation(&mut builder, terminate)?;
    Ok(builder.build())
}

fn configure_alpn(builder: &mut SslAcceptorBuilder) {
    builder.set_alpn_select_callback(|_, client_protocols| {
        select_next_proto(SHARED_TLS_ALPN_H2_H1, client_protocols).ok_or(AlpnError::NOACK)
    });
}

fn configure_frontend_client_validation(
    builder: &mut SslAcceptorBuilder,
    terminate: &TerminateSurface,
) -> Result<(), SharedTlsError> {
    let Some(client_ca_bundle_pem) = terminate.client_ca_bundle_pem.as_deref() else {
        return Ok(());
    };

    let ca_certs = X509::stack_from_pem(client_ca_bundle_pem.as_bytes()).map_err(|e| {
        SharedTlsError::Certificate(format!("parse frontend client CA bundle: {e}"))
    })?;
    if ca_certs.is_empty() {
        return Err(SharedTlsError::Certificate(
            "frontend client CA bundle did not contain certificates".to_string(),
        ));
    }

    for cert in ca_certs {
        builder.cert_store_mut().add_cert(cert).map_err(|e| {
            SharedTlsError::Certificate(format!("add frontend client CA to trust store: {e}"))
        })?;
    }

    if matches!(
        terminate.frontend_validation_mode.as_deref(),
        Some("AllowInsecureFallback")
    ) {
        builder.set_verify_callback(SslVerifyMode::PEER, |_preverify_ok, _ctx| true);
    } else {
        builder.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
    }
    builder
        .set_session_id_context(b"ntgw-shared-tls")
        .map_err(|e| {
            SharedTlsError::Certificate(format!("set shared TLS session id context: {e}"))
        })?;
    Ok(())
}

struct DynamicTlsCertificates {
    identities: Vec<SharedTlsIdentity>,
}

impl DynamicTlsCertificates {
    fn new(identities: Vec<SharedTlsIdentity>) -> Self {
        Self { identities }
    }
}

#[async_trait]
impl TlsAccept for DynamicTlsCertificates {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        let server_name = ssl
            .servername(pingora::tls::ssl::NameType::HOST_NAME)
            .map(str::to_string);
        for identity in ordered_tls_identity_candidates(&self.identities, server_name.as_deref()) {
            if apply_dynamic_tls_identity(ssl, identity).is_ok() {
                return;
            }

            warn!(
                secret = %identity.secret_ref,
                sni = server_name.as_deref().unwrap_or_default(),
                "failed to apply shared tls identity during handshake, trying next certificate"
            );
        }

        warn!(
            sni = server_name.as_deref().unwrap_or_default(),
            "failed to apply any shared tls identity during handshake"
        );
    }

    async fn handshake_complete_callback(
        &self,
        ssl: &SslRef,
    ) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(Arc::new(DownstreamTlsInfo {
            server_name: ssl
                .servername(pingora::tls::ssl::NameType::HOST_NAME)
                .map(str::to_string)
                .unwrap_or_default(),
            client_certificate_present: ssl.peer_certificate().is_some(),
        }) as Arc<dyn Any + Send + Sync>)
    }
}

fn apply_dynamic_tls_identity(
    ssl: &mut SslRef,
    identity: &SharedTlsIdentity,
) -> Result<(), SharedTlsError> {
    let certs = X509::stack_from_pem(identity.cert_pem.as_bytes())
        .map_err(|e| SharedTlsError::Certificate(format!("parse certificate PEM: {e}")))?;
    let Some(leaf) = certs.first() else {
        return Err(SharedTlsError::Certificate(
            "no certificates found in PEM".to_string(),
        ));
    };
    let key = PKey::private_key_from_pem(identity.key_pem.as_bytes())
        .map_err(|e| SharedTlsError::Certificate(format!("parse private key PEM: {e}")))?;

    ext::ssl_use_certificate(ssl, leaf)
        .map_err(|e| SharedTlsError::Certificate(format!("load leaf certificate: {e}")))?;
    for cert in certs.iter().skip(1) {
        ext::ssl_add_chain_cert(ssl, cert)
            .map_err(|e| SharedTlsError::Certificate(format!("load certificate chain: {e}")))?;
    }
    ext::ssl_use_private_key(ssl, &key)
        .map_err(|e| SharedTlsError::Certificate(format!("load private key: {e}")))?;
    Ok(())
}

fn ordered_tls_identity_candidates<'a>(
    identities: &'a [SharedTlsIdentity],
    server_name: Option<&str>,
) -> Vec<&'a SharedTlsIdentity> {
    if identities.is_empty() {
        return Vec::new();
    }

    let mut scored = Vec::new();
    let mut fallback = Vec::new();

    for (index, identity) in identities.iter().enumerate() {
        if let Some(server_name) = server_name
            && let Some(rank) = tls_identity_match_rank(identity, server_name)
        {
            scored.push((rank, index, identity));
            continue;
        }
        fallback.push((index, identity));
    }

    scored.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));

    let mut ordered = Vec::with_capacity(identities.len());
    ordered.extend(scored.into_iter().map(|(_, _, identity)| identity));
    ordered.extend(fallback.into_iter().map(|(_, identity)| identity));
    ordered
}

fn tls_identity_match_rank(identity: &SharedTlsIdentity, server_name: &str) -> Option<u8> {
    let normalized = normalize_tls_server_name(server_name);
    let mut best = None;

    for pattern in &identity.match_names {
        if pattern == &normalized {
            return Some(2);
        }
        if wildcard_hostname_matches(pattern, &normalized) {
            best = Some(best.unwrap_or(1));
        }
    }

    best
}

fn normalize_tls_server_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn wildcard_hostname_matches(pattern: &str, host: &str) -> bool {
    let Some(suffix) = pattern.strip_prefix("*.") else {
        return false;
    };

    if host == suffix || !host.ends_with(suffix) {
        return false;
    }

    let Some(prefix) = host
        .strip_suffix(suffix)
        .and_then(|value| value.strip_suffix('.'))
    else {
        return false;
    };

    !prefix.is_empty() && !prefix.contains('.')
}
