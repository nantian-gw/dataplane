use std::sync::Arc;

use ntgw_ir::{BackendTlsConfig, Snapshot};
use parking_lot::RwLock;
use pingora::utils::tls::CertKey;
use pingora::{Error, ErrorType};

use super::super::cache::{ClientCertCache, ClientCertCacheKey, BACKEND_CLIENT_CERT_CACHE};

pub(crate) fn resolve_backend_client_cert_key(
    snapshot: &Snapshot,
    backend_tls: Option<&BackendTlsConfig>,
) -> pingora::Result<Option<Arc<CertKey>>> {
    let Some(backend_tls) = backend_tls else {
        return Ok(None);
    };

    let (namespace, name) = backend_tls
        .client_certificate_ref
        .split_once('/')
        .ok_or_else(|| {
            invalid_backend_client_certificate("client certificate ref must use namespace/name")
        })?;
    let secret = snapshot.secret_material(namespace, name).ok_or_else(|| {
        invalid_backend_client_certificate(format!(
            "client certificate secret {namespace}/{name} was not found in snapshot"
        ))
    })?;

    cached_client_cert_key(secret).map(Some)
}

fn parse_client_cert_key(secret: &ntgw_ir::SecretMaterial) -> pingora::Result<Arc<CertKey>> {
    if secret.cert_pem.is_empty() || secret.key_pem.is_empty() {
        return Err(invalid_backend_client_certificate(format!(
            "secret {}/{} does not contain tls.crt and tls.key",
            secret.namespace, secret.name
        )));
    }

    let certificates = pingora::tls::x509::X509::stack_from_pem(secret.cert_pem.as_bytes())
        .map_err(|err| {
            invalid_backend_client_certificate(format!(
                "failed to parse client certificate PEM for {}/{}: {}",
                secret.namespace, secret.name, err
            ))
        })?;
    if certificates.is_empty() {
        return Err(invalid_backend_client_certificate(format!(
            "client certificate secret {}/{} does not contain any certificates",
            secret.namespace, secret.name
        )));
    }

    let private_key = pingora::tls::pkey::PKey::private_key_from_pem(secret.key_pem.as_bytes())
        .map_err(|err| {
            invalid_backend_client_certificate(format!(
                "failed to parse client private key PEM for {}/{}: {}",
                secret.namespace, secret.name, err
            ))
        })?;

    Ok(Arc::new(CertKey::new(certificates, private_key)))
}

pub(crate) fn invalid_backend_client_certificate(message: impl Into<String>) -> Box<Error> {
    Error::new(ErrorType::new("InvalidBackendClientCertificate")).more_context(message.into())
}

fn cached_client_cert_key(secret: &ntgw_ir::SecretMaterial) -> pingora::Result<Arc<CertKey>> {
    let cache_key = ClientCertCacheKey::new(secret);
    let cache = BACKEND_CLIENT_CERT_CACHE.get_or_init(|| RwLock::new(ClientCertCache::default()));
    if let Some(parsed) = lookup_cached_client_cert_key(cache, &cache_key) {
        return parsed.map_err(invalid_backend_client_certificate);
    }

    let parsed = parse_client_cert_key(secret).map_err(|err| err.to_string());
    let mut state = cache.write();
    let cached = state
        .entries
        .entry(cache_key)
        .or_insert_with(|| parsed.clone())
        .clone();
    cached.map_err(invalid_backend_client_certificate)
}

fn lookup_cached_client_cert_key(
    cache: &RwLock<ClientCertCache>,
    cache_key: &ClientCertCacheKey,
) -> Option<Result<Arc<CertKey>, String>> {
    cache.read().entries.get(cache_key).cloned()
}
