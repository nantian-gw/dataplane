use std::collections::BTreeMap;

use aeg_ir::{
    BackendEndpoint, BackendPolicy, BackendTlsValidation, RouteKind, SelectedBackend, Snapshot,
};

use super::super::backend::{backend_certificate_matches_subject_alt_names, build_upstream_peer};
use super::{TEST_CLIENT_CERT_PEM, TEST_SERVER_SAN_CERT_PEM};

mod cache_and_errors;
mod peer_tls;
mod subject_alt_names;
