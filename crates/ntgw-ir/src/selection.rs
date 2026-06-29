use std::collections::BTreeMap;

use ntgw_config::RoutePolicyConfig;

use crate::{
    BackendEndpoint, BackendTlsConfig, Filter, MatchedHttpPath, RetryPolicy, RouteTimeouts,
    SessionPersistence,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    Http,
    Grpc,
    Tcp,
    Udp,
    Tls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendSelectionError {
    InvalidBackendRefs,
    NoHealthyBackends,
}

#[derive(Debug, Clone)]
pub struct SelectedBackend {
    pub route_kind: RouteKind,
    pub route_name: String,
    pub route_namespace: String,
    pub rule_index: Option<usize>,
    pub route_annotations: BTreeMap<String, String>,
    pub listener_name: String,
    pub listener_protocol: String,
    pub backend: BackendEndpoint,
    pub backend_name: String,
    pub filters: Vec<Filter>,
    pub matched_http_path: Option<MatchedHttpPath>,
    pub timeouts: Option<RouteTimeouts>,
    pub retry: Option<RetryPolicy>,
    pub session_persistence: Option<SessionPersistence>,
    pub backend_tls: Option<BackendTlsConfig>,
}

#[derive(Debug, Clone)]
pub struct RequestMirrorContext {
    pub route_kind: RouteKind,
    pub route_name: String,
    pub route_namespace: String,
    pub rule_index: Option<usize>,
    pub filters: Vec<Filter>,
    pub matched_http_path: Option<MatchedHttpPath>,
    pub timeouts: Option<RouteTimeouts>,
    pub backend_tls: Option<BackendTlsConfig>,
}

#[derive(Debug, Clone)]
pub struct SelectedHttpRoute {
    pub route_name: String,
    pub route_namespace: String,
    pub rule_index: Option<usize>,
    pub route_annotations: BTreeMap<String, String>,
    pub listener_name: String,
    pub listener_protocol: String,
    pub filters: Vec<Filter>,
    pub matched_http_path: MatchedHttpPath,
    pub backend: Option<BackendEndpoint>,
    pub backend_name: Option<String>,
    pub backend_error: Option<BackendSelectionError>,
    pub timeouts: Option<RouteTimeouts>,
    pub retry: Option<RetryPolicy>,
    pub session_persistence: Option<SessionPersistence>,
    pub backend_tls: Option<BackendTlsConfig>,
    pub route_policy: Option<RoutePolicyConfig>,
}
