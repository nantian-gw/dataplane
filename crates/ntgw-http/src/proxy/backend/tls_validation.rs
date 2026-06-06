use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ntgw_ir::{BackendTlsValidation, SelectedBackend};
use parking_lot::RwLock;
use pingora::prelude::HttpPeer;
use pingora::{Error, ErrorType};
use tracing::warn;

use super::super::cache::{
    BackendTlsValidationCache, BackendTlsValidationCacheKey, CachedBackendTlsValidation,
    BACKEND_TLS_VALIDATION_CACHE,
};

pub(crate) fn resolve_backend_tls_validation(
    validation: Option<&BackendTlsValidation>,
) -> pingora::Result<Option<CachedBackendTlsValidation>> {
    let Some(validation) = validation else {
        return Ok(None);
    };

    if validation.hostname.is_empty() {
        return Err(unsupported_backend_tls_validation(
            "backend TLS validation hostname must not be empty",
        ));
    }

    cached_backend_tls_validation(validation).map(Some)
}

pub(crate) fn apply_cached_backend_tls_validation(
    peer: &mut HttpPeer,
    cached: Option<&CachedBackendTlsValidation>,
) {
    let Some(cached) = cached else {
        return;
    };

    peer.options.ca = cached.ca_certificates.clone();
    peer.group_key = cached.group_key;
    peer.options.alternative_cn = cached.alternative_cn.clone();
    peer.options.upstream_tls_handshake_complete_hook =
        cached.subject_alt_name_validation_hook.clone();
    peer.options.verify_hostname = cached.verify_hostname;
    peer.options.verify_cert = true;
}

fn cached_backend_tls_validation(
    validation: &BackendTlsValidation,
) -> pingora::Result<CachedBackendTlsValidation> {
    let cache_key = BackendTlsValidationCacheKey::new(validation);
    let cache = BACKEND_TLS_VALIDATION_CACHE
        .get_or_init(|| RwLock::new(BackendTlsValidationCache::default()));
    if let Some(parsed) = lookup_cached_backend_tls_validation(cache, &cache_key) {
        return parsed.map_err(unsupported_backend_tls_validation);
    }

    let parsed = build_cached_backend_tls_validation(validation).map_err(|err| err.to_string());
    let mut state = cache.write();
    let cached = state
        .entries
        .entry(cache_key)
        .or_insert_with(|| parsed.clone())
        .clone();
    cached.map_err(unsupported_backend_tls_validation)
}

fn build_cached_backend_tls_validation(
    validation: &BackendTlsValidation,
) -> pingora::Result<CachedBackendTlsValidation> {
    let ca_certificates = if validation.use_system_ca_certificates {
        None
    } else {
        if validation.ca_pems.is_empty() {
            return Err(unsupported_backend_tls_validation(
                "backend TLS validation must provide either system CA certificates or a custom CA bundle",
            ));
        }
        Some(parse_backend_ca_certificates(&validation.ca_pems)?)
    };

    if !validation.min_version.is_empty() || !validation.max_version.is_empty() {
        return Err(unsupported_backend_tls_validation(
            "backend TLS version bounds are not supported by the upstream Nantian/OpenSSL runtime",
        ));
    }

    let subject_alt_name_validation_hook = (!validation.subject_alt_names.is_empty())
        .then(|| build_backend_tls_subject_alt_name_validation_hook(&validation.subject_alt_names));
    if validation.subject_alt_names.is_empty() && validation.use_system_ca_certificates {
        warn!(
            "BackendTLSPolicy has no subjectAltNames and uses system CA — hostname verification is disabled"
        );
    }
    Ok(CachedBackendTlsValidation {
        ca_certificates,
        verify_hostname: validation.subject_alt_names.is_empty(),
        alternative_cn: None,
        group_key: backend_tls_validation_group_key(validation),
        subject_alt_name_validation_hook,
    })
}

fn lookup_cached_backend_tls_validation(
    cache: &RwLock<BackendTlsValidationCache>,
    cache_key: &BackendTlsValidationCacheKey,
) -> Option<Result<CachedBackendTlsValidation, String>> {
    cache.read().entries.get(cache_key).cloned()
}

pub(crate) fn backend_tls_sni_name(
    endpoint: &SelectedBackend,
    validation: Option<&BackendTlsValidation>,
) -> Option<String> {
    if let Some(validation) = validation {
        if !validation.hostname.is_empty() {
            return Some(validation.hostname.clone());
        }
    }

    backend_tls_service_name(&endpoint.backend_name)
}

pub(crate) fn backend_tls_service_name(backend_name: &str) -> Option<String> {
    let (namespace, cluster_name) = backend_name.split_once('/')?;
    let (service_name, _) = cluster_name.rsplit_once(':')?;
    if namespace.is_empty() || service_name.is_empty() {
        return None;
    }

    Some(format!("{service_name}.{namespace}.svc"))
}

fn unsupported_backend_tls_validation(message: impl Into<String>) -> Box<Error> {
    Error::new(ErrorType::new("UnsupportedBackendTlsValidation")).more_context(message.into())
}

fn invalid_backend_tls_subject_alt_name(message: impl Into<String>) -> Box<Error> {
    Error::new(ErrorType::new("InvalidBackendTlsSubjectAltName")).more_context(message.into())
}

fn parse_backend_ca_certificates(
    ca_pems: &[String],
) -> pingora::Result<Arc<pingora::protocols::tls::CaType>> {
    let mut certificates = Vec::new();
    for ca_pem in ca_pems {
        let items = pingora::tls::x509::X509::stack_from_pem(ca_pem.as_bytes()).map_err(|err| {
            unsupported_backend_tls_validation(format!(
                "failed to parse backend TLS CA bundle: {err}"
            ))
        })?;
        if items.is_empty() {
            return Err(unsupported_backend_tls_validation(
                "backend TLS CA bundle does not contain any certificates",
            ));
        }

        certificates.extend(items);
    }

    if certificates.is_empty() {
        return Err(unsupported_backend_tls_validation(
            "backend TLS validation requires at least one CA certificate",
        ));
    }

    Ok(Arc::new(certificates.into_boxed_slice()))
}

pub(crate) fn backend_certificate_matches_subject_alt_names(
    certificate: &pingora::tls::x509::X509Ref,
    subject_alt_names: &[ntgw_ir::BackendSubjectAltName],
) -> Result<bool, String> {
    if subject_alt_names.is_empty() {
        return Ok(true);
    }

    let mut presented_hostnames = Vec::new();
    let mut presented_uris = Vec::new();

    if let Some(items) = certificate.subject_alt_names() {
        for item in items {
            if let Some(hostname) = item.dnsname() {
                let normalized = normalize_backend_tls_hostname(hostname);
                if !normalized.is_empty() {
                    presented_hostnames.push(normalized);
                }
            }
            if let Some(uri) = item.uri() {
                presented_uris.push(uri.to_string());
            }
        }
    }

    for item in subject_alt_names {
        if item.value.is_empty() {
            return Err("backend TLS subjectAltNames entries must not be empty".to_string());
        }

        match item.kind.as_str() {
            "Hostname" => {
                let expected = normalize_backend_tls_hostname(&item.value);
                if expected.is_empty() {
                    return Err(
                        "backend TLS Hostname subjectAltName entries must not be empty".to_string(),
                    );
                }
                if presented_hostnames
                    .iter()
                    .any(|presented| hostname_subject_alt_name_matches(presented, &expected))
                {
                    return Ok(true);
                }
            }
            "URI" => {
                if presented_uris
                    .iter()
                    .any(|presented| presented == &item.value)
                {
                    return Ok(true);
                }
            }
            _ => {
                return Err(format!(
                    "unsupported backend TLS subjectAltName type: {}",
                    item.kind
                ));
            }
        }
    }

    Ok(false)
}

fn build_backend_tls_subject_alt_name_validation_hook(
    subject_alt_names: &[ntgw_ir::BackendSubjectAltName],
) -> pingora::protocols::tls::HandshakeCompleteHook {
    let expected = Arc::new(subject_alt_names.to_vec());
    Arc::new(move |ssl: &pingora::tls::ssl::SslRef| {
        let failure = match ssl.peer_certificate() {
            Some(certificate) => {
                match backend_certificate_matches_subject_alt_names(&certificate, expected.as_ref())
                {
                    Ok(true) => None,
                    Ok(false) => Some(
                        "backend certificate subjectAltName entries did not match any configured BackendTLSPolicy subjectAltNames"
                            .to_string(),
                    ),
                    Err(message) => Some(message),
                }
            }
            None => Some("backend TLS handshake did not present a peer certificate".to_string()),
        }?;

        Some(Arc::new(BackendTlsSubjectAltNameValidationFailure {
            message: failure,
        }))
    })
}

fn backend_tls_validation_group_key(validation: &BackendTlsValidation) -> u64 {
    let mut hasher = DefaultHasher::new();
    BackendTlsValidationCacheKey::new(validation).hash(&mut hasher);
    let value = hasher.finish();
    if value == 0 {
        1
    } else {
        value
    }
}

fn normalize_backend_tls_hostname(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn hostname_subject_alt_name_matches(pattern: &str, expected: &str) -> bool {
    pattern == expected || wildcard_backend_tls_hostname_matches(pattern, expected)
}

fn wildcard_backend_tls_hostname_matches(pattern: &str, host: &str) -> bool {
    let Some(suffix) = pattern.strip_prefix("*.") else {
        return false;
    };

    host != suffix
        && host.ends_with(suffix)
        && host
            .as_bytes()
            .get(host.len().saturating_sub(suffix.len() + 1))
            .is_some_and(|item| *item == b'.')
}

#[derive(Debug)]
struct BackendTlsSubjectAltNameValidationFailure {
    message: String,
}

pub(crate) fn validate_backend_tls_subject_alt_name_result(
    peer: &HttpPeer,
    digest: Option<&pingora::protocols::Digest>,
) -> pingora::Result<()> {
    if peer.options.upstream_tls_handshake_complete_hook.is_none() {
        return Ok(());
    }

    let Some(ssl_digest) = digest.and_then(|item| item.ssl_digest.as_ref()) else {
        return Err(invalid_backend_tls_subject_alt_name(
            "backend TLS subjectAltName validation expected a TLS digest but none was recorded",
        ));
    };

    if let Some(failure) = ssl_digest
        .extension
        .get::<BackendTlsSubjectAltNameValidationFailure>()
    {
        return Err(invalid_backend_tls_subject_alt_name(
            failure.message.clone(),
        ));
    }

    Ok(())
}
