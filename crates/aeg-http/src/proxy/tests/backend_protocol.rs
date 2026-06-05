use std::collections::BTreeMap;

use crate::proxy::UpstreamTuningOptions;
use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendPolicy, BackendTlsConfig, BackendTlsValidation,
    RouteKind, RouteTimeouts, SelectedBackend, Snapshot,
};
use pingora::prelude::HttpPeer;

use super::super::backend::{
    apply_backend_policy, apply_backend_protocol, backend_tls_service_name,
    build_upstream_peer_with_cached_config, build_upstream_peer_with_keepalive,
    is_tls_backend_protocol,
};
use super::super::context::UpstreamPeerAddress;
use super::super::selection::{
    selected_backend_config, selected_backend_config_cached,
    selected_backend_config_cached_for_fast_path, selected_backend_config_with_overrides,
    SelectedBackendConfigCache,
};
use super::super::DEFAULT_MAX_H2_UPSTREAM_STREAMS;
use super::{TEST_CLIENT_CERT_PEM, TEST_CLIENT_KEY_PEM};

include!("backend_protocol/policy.rs");
include!("backend_protocol/protocol.rs");
include!("backend_protocol/cached_config.rs");
include!("backend_protocol/keepalive.rs");
