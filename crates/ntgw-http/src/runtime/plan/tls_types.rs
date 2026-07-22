/// TLS material types used by the runtime listener plan.
///
/// Extracted into a separate module to keep plan.rs under the size limit.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsMaterial {
    pub(crate) identities: Vec<TlsIdentity>,
    pub(crate) min_version: String,
    pub(crate) max_version: String,
    pub(crate) client_ca_bundle_pem: Option<String>,
    pub(crate) frontend_validation_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsIdentity {
    pub(crate) secret_ref: String,
    pub(crate) cert_pem: String,
    pub(crate) key_pem: String,
    pub(crate) match_names: Vec<String>,
}
