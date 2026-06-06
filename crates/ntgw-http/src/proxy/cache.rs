use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::{Arc, OnceLock},
};

use parking_lot::RwLock;
use pingora::protocols::tls::HandshakeCompleteHook;
use pingora::utils::tls::CertKey;
use sha2::{Digest, Sha256};

use crate::session::{ResolvedSession, SessionManager};
use ntgw_ir::{BackendTlsValidation, SessionPersistence};

pub(crate) static BACKEND_CLIENT_CERT_CACHE: OnceLock<RwLock<ClientCertCache>> = OnceLock::new();
pub(crate) static BACKEND_TLS_VALIDATION_CACHE: OnceLock<RwLock<BackendTlsValidationCache>> =
    OnceLock::new();

#[derive(Default)]
pub(crate) struct ClientCertCache {
    pub(crate) entries: HashMap<ClientCertCacheKey, Result<Arc<CertKey>, String>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ClientCertCacheKey {
    namespace: String,
    name: String,
    cert_pem_sha256: [u8; 32],
    key_pem_sha256: [u8; 32],
}

#[derive(Default)]
pub(crate) struct BackendTlsValidationCache {
    pub(crate) entries:
        HashMap<BackendTlsValidationCacheKey, Result<CachedBackendTlsValidation, String>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct BackendTlsValidationCacheKey {
    hostname: String,
    use_system_ca_certificates: bool,
    ca_bundle_sha256: [u8; 32],
    subject_alt_names_sha256: [u8; 32],
    min_version: String,
    max_version: String,
}

#[derive(Clone, Default)]
pub(crate) struct CachedBackendTlsValidation {
    pub(crate) ca_certificates: Option<Arc<pingora::protocols::tls::CaType>>,
    pub(crate) verify_hostname: bool,
    pub(crate) alternative_cn: Option<String>,
    pub(crate) group_key: u64,
    pub(crate) subject_alt_name_validation_hook: Option<HandshakeCompleteHook>,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct SessionCacheKey {
    session_name: String,
    is_cookie: bool,
}

pub(crate) struct SessionResolutionCache<'a> {
    manager: &'a SessionManager,
    headers: &'a BTreeMap<String, Vec<String>>,
    entries: RefCell<HashMap<SessionCacheKey, Option<ResolvedSession>>>,
}

impl ClientCertCacheKey {
    pub(crate) fn new(secret: &ntgw_ir::SecretMaterial) -> Self {
        Self {
            namespace: secret.namespace.clone(),
            name: secret.name.clone(),
            cert_pem_sha256: digest_bytes(secret.cert_pem.as_bytes()),
            key_pem_sha256: digest_bytes(secret.key_pem.as_bytes()),
        }
    }
}

impl BackendTlsValidationCacheKey {
    pub(crate) fn new(validation: &BackendTlsValidation) -> Self {
        Self {
            hostname: validation.hostname.clone(),
            use_system_ca_certificates: validation.use_system_ca_certificates,
            ca_bundle_sha256: digest_text_segments(validation.ca_pems.iter().map(String::as_str)),
            subject_alt_names_sha256: digest_subject_alt_names(&validation.subject_alt_names),
            min_version: validation.min_version.clone(),
            max_version: validation.max_version.clone(),
        }
    }
}

fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn digest_text_segments<'a>(segments: impl IntoIterator<Item = &'a str>) -> [u8; 32] {
    let mut digest = Sha256::new();
    for segment in segments {
        digest.update((segment.len() as u64).to_le_bytes());
        digest.update(segment.as_bytes());
    }
    digest.finalize().into()
}

fn digest_subject_alt_names(items: &[ntgw_ir::BackendSubjectAltName]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for item in items {
        digest.update((item.kind.len() as u64).to_le_bytes());
        digest.update(item.kind.as_bytes());
        digest.update((item.value.len() as u64).to_le_bytes());
        digest.update(item.value.as_bytes());
    }
    digest.finalize().into()
}

impl SessionCacheKey {
    fn from_policy(policy: &SessionPersistence) -> Self {
        Self {
            session_name: policy.session_name.clone(),
            is_cookie: policy.is_cookie(),
        }
    }
}

impl<'a> SessionResolutionCache<'a> {
    pub(crate) fn new(
        manager: &'a SessionManager,
        headers: &'a BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self {
            manager,
            headers,
            entries: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn resolve_target(
        &self,
        policy: &SessionPersistence,
    ) -> Option<ntgw_ir::PersistentSessionTarget> {
        self.resolve(policy).map(|session| session.target.clone())
    }

    pub(crate) fn resolved_session(&self, policy: &SessionPersistence) -> Option<ResolvedSession> {
        self.resolve(policy)
    }

    fn resolve(&self, policy: &SessionPersistence) -> Option<ResolvedSession> {
        let key = SessionCacheKey::from_policy(policy);
        if let Some(cached) = self.entries.borrow().get(&key) {
            return cached.clone();
        }

        let resolved = self
            .manager
            .resolve_request_session_headers(self.headers, policy);
        self.entries.borrow_mut().insert(key, resolved.clone());
        resolved
    }
}
