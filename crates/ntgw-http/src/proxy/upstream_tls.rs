/// Consolidated TLS API surface for the proxy module.
///
/// Provides a single import point for TLS validation and upstream peer building
/// functions that were previously scattered across proxy sub-modules.
pub(crate) use super::backend::{
    build_upstream_peer_for_fast_path, build_upstream_peer_with_cached_config,
    validate_backend_tls_subject_alt_name_result,
};
pub(crate) use super::cache::BackendTlsValidationCacheKey;
