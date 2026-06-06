use std::collections::BTreeMap;

use ntgw_ir::{BackendEndpoint, BackendTlsConfig, RouteKind, SelectedBackend, Snapshot};

use super::super::backend::build_upstream_peer;
use super::{
    TEST_CLIENT_CERT_PEM, TEST_CLIENT_KEY_PEM, TEST_SERVER_SAN_CERT_PEM, TEST_SERVER_SAN_KEY_PEM,
};

include!("backend_client_cert/basic.rs");
include!("backend_client_cert/rotation.rs");
include!("backend_client_cert/cache_reuse.rs");
include!("backend_client_cert/errors.rs");
