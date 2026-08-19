use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet, hash_map::Entry},
    sync::Arc,
};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::endpoint_runtime::EndpointRuntimeStore;
use crate::filters::Filter;
use crate::http_fast_path::HttpFastPathPlan;
use crate::matching::normalize_host_ref;
use crate::mesh::{ParentRef, ServiceFrontendRef, Workload};
use crate::runtime_id::RuntimeIdIndex;
use crate::selection::BackendSelectionError;
use crate::session::SessionPersistence;
use crate::stream_fast_path::StreamFastPathPlan;
use crate::timeouts::RouteTimeouts;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub(crate) const BACKEND_REF_META_VALID: &str = "nantian.dev/backend-ref-valid";
#[doc(hidden)]
pub const PASSIVE_EJECTION_CONSECUTIVE_FAILURES: u32 = 3;
#[doc(hidden)]
pub const PASSIVE_EJECTION_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Helper: default backend weight
// ---------------------------------------------------------------------------

fn default_backend_weight() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Intermediate types used during resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub(crate) struct HttpBackendResolution {
    pub(crate) selected: Option<ResolvedHttpBackend>,
    pub(crate) error: Option<BackendSelectionError>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StreamMatchScore {
    pub(crate) hostname_rank: u8,
    pub(crate) hostname_length: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedHttpBackend {
    pub(crate) endpoint: BackendEndpoint,
    pub(crate) backend_name: String,
    pub(crate) filters: Vec<Filter>,
    pub(crate) session_persistence: Option<SessionPersistence>,
}

// ---------------------------------------------------------------------------
// FrontendClientCertificateRequirement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FrontendClientCertificateRequirement {
    #[default]
    None,
    Reject,
    Require,
}

impl FrontendClientCertificateRequirement {
    pub fn closes_connection_without_valid_client_certificate(
        self,
        client_certificate_present: bool,
    ) -> bool {
        match self {
            Self::None => false,
            Self::Reject => true,
            Self::Require => !client_certificate_present,
        }
    }
}

// ---------------------------------------------------------------------------
// RequestMaterializationHints
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequestMaterializationHints {
    pub http_route_headers: bool,
    pub grpc_route_headers: bool,
    pub session_headers: bool,
    pub backend_hash_headers: bool,
    pub source_ip: bool,
}

impl RequestMaterializationHints {
    pub fn requires_full_headers(&self) -> bool {
        self.http_route_headers
            || self.grpc_route_headers
            || self.session_headers
            || self.backend_hash_headers
    }
}

// ---------------------------------------------------------------------------
// HostnameRouteIndex
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct HostnameRouteIndex {
    pub(crate) catch_all: Vec<usize>,
    pub(crate) exact: HashMap<String, Vec<usize>>,
    pub(crate) wildcard_suffix: HashMap<String, Vec<usize>>,
}

impl HostnameRouteIndex {
    #[cfg(test)]
    pub(crate) fn candidate_indices(&self, request_host: Option<&str>) -> Vec<usize> {
        let mut candidate_indices = Vec::new();
        self.visit_candidate_indices(request_host, |index| {
            candidate_indices.push(index);
            true
        });
        candidate_indices
    }

    pub(crate) fn visit_candidate_indices(
        &self,
        request_host: Option<&str>,
        mut visit: impl FnMut(usize) -> bool,
    ) -> bool {
        let request_host = request_host.map(normalize_host_ref);
        let mut last = None;
        while let Some(index) = self.next_candidate_index_after_normalized(request_host, last) {
            if !visit(index) {
                return false;
            }
            last = Some(index);
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn next_candidate_index_after(
        &self,
        request_host: Option<&str>,
        last: Option<usize>,
    ) -> Option<usize> {
        self.next_candidate_index_after_normalized(request_host.map(normalize_host_ref), last)
    }

    fn next_candidate_index_after_normalized(
        &self,
        request_host: Option<&str>,
        last: Option<usize>,
    ) -> Option<usize> {
        let mut next = first_index_after(&self.catch_all, last);

        if let Some(host) = request_host {
            // Avoid allocation on the hot path: most hostnames are already
            // lowercase. Only call to_ascii_lowercase() when there are uppercase
            // characters (rare in practice).
            let host_ref: Cow<'_, str> = if host.bytes().any(|b| b.is_ascii_uppercase()) {
                Cow::Owned(host.to_ascii_lowercase())
            } else {
                Cow::Borrowed(host)
            };
            if let Some(indices) = self.exact.get(host_ref.as_ref()) {
                next = min_candidate_index(next, first_index_after(indices, last));
            }
            for suffix in wildcard_hostname_suffixes(host_ref.as_ref()) {
                if let Some(indices) = self.wildcard_suffix.get(suffix) {
                    next = min_candidate_index(next, first_index_after(indices, last));
                }
            }
        }

        next
    }
}

fn wildcard_hostname_suffixes(host: &str) -> impl Iterator<Item = &str> {
    host.match_indices('.')
        .filter_map(|(index, _)| host.get(index + 1..))
}

fn first_index_after(indices: &[usize], last: Option<usize>) -> Option<usize> {
    let start = last.map_or(0, |last| indices.partition_point(|index| *index <= last));
    indices.get(start).copied()
}

fn min_candidate_index(current: Option<usize>, candidate: Option<usize>) -> Option<usize> {
    match (current, candidate) {
        (Some(current), Some(candidate)) => Some(current.min(candidate)),
        (Some(current), None) => Some(current),
        (None, Some(candidate)) => Some(candidate),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// RouteAttachmentListenerIndex
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct RouteAttachmentListenerIndex {
    routes: HashMap<String, HashMap<String, Vec<usize>>>,
}

impl RouteAttachmentListenerIndex {
    pub(crate) fn from_listeners(listeners: &[Listener]) -> Self {
        let mut routes = HashMap::<String, HashMap<String, Vec<usize>>>::new();

        for (listener_index, listener) in listeners.iter().enumerate() {
            for attached_route in &listener.attached_routes {
                let Some((namespace, name)) = attached_route.split_once('/') else {
                    continue;
                };
                routes
                    .entry(namespace.to_string())
                    .or_default()
                    .entry(name.to_string())
                    .or_default()
                    .push(listener_index);
            }
        }

        for routes_by_name in routes.values_mut() {
            for listener_indices in routes_by_name.values_mut() {
                listener_indices.sort_unstable();
                listener_indices.dedup();
            }
        }

        Self { routes }
    }

    pub(crate) fn listeners_for_route(&self, namespace: &str, name: &str) -> Option<&[usize]> {
        self.routes
            .get(namespace)
            .and_then(|routes| routes.get(name).map(Vec::as_slice))
    }

    pub(crate) fn contains_listener(
        &self,
        namespace: &str,
        name: &str,
        listener_index: usize,
    ) -> bool {
        self.listeners_for_route(namespace, name)
            .is_some_and(|listeners| listeners.binary_search(&listener_index).is_ok())
    }
}

// ---------------------------------------------------------------------------
// BackendServiceIndex
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct BackendServiceIndex {
    services: HashMap<String, BackendServiceBucket>,
}

#[derive(Debug, Clone)]
enum BackendServiceBucket {
    Single(BackendServiceEntry),
    Many(Vec<BackendServiceEntry>),
}

#[derive(Debug, Clone)]
struct BackendServiceEntry {
    namespace: String,
    port: u32,
    index: usize,
}

impl BackendServiceIndex {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            services: HashMap::with_capacity(capacity),
        }
    }

    pub(crate) fn insert(&mut self, namespace: &str, name: &str, port: u32, index: usize) {
        let entry = BackendServiceEntry {
            namespace: namespace.to_string(),
            port,
            index,
        };

        match self.services.entry(name.to_string()) {
            Entry::Vacant(bucket) => {
                bucket.insert(BackendServiceBucket::Single(entry));
            }
            Entry::Occupied(mut bucket) => {
                bucket.get_mut().insert(entry);
            }
        }
    }

    pub(crate) fn index_for(&self, namespace: &str, name: &str, port: u32) -> Option<usize> {
        self.services
            .get(name)
            .and_then(|bucket| bucket.index_for(namespace, port))
    }

    pub(crate) fn unique_namespace(&self, name: &str, port: u32) -> Option<&str> {
        self.services
            .get(name)
            .and_then(|bucket| bucket.unique_namespace(port))
    }

    #[cfg(test)]
    pub(crate) fn service_name_count(&self) -> usize {
        self.services.len()
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.services.values().map(BackendServiceBucket::len).sum()
    }
}

impl BackendServiceBucket {
    fn insert(&mut self, entry: BackendServiceEntry) {
        match self {
            Self::Single(existing) if existing.matches(entry.namespace.as_str(), entry.port) => {
                existing.index = entry.index;
            }
            Self::Single(existing) => {
                let first = existing.clone();
                *self = Self::Many(vec![first, entry]);
            }
            Self::Many(entries) => {
                if let Some(existing) = entries
                    .iter_mut()
                    .find(|item| item.matches(entry.namespace.as_str(), entry.port))
                {
                    existing.index = entry.index;
                } else {
                    entries.push(entry);
                }
            }
        }
    }

    fn index_for(&self, namespace: &str, port: u32) -> Option<usize> {
        match self {
            Self::Single(entry) => entry.matches(namespace, port).then_some(entry.index),
            Self::Many(entries) => entries
                .iter()
                .find(|entry| entry.matches(namespace, port))
                .map(|entry| entry.index),
        }
    }

    fn unique_namespace(&self, port: u32) -> Option<&str> {
        match self {
            Self::Single(entry) => (entry.port == port).then_some(entry.namespace.as_str()),
            Self::Many(entries) => {
                let mut namespace = None;
                for entry in entries.iter().filter(|entry| entry.port == port) {
                    match namespace {
                        Some(current) if current != entry.namespace => return None,
                        Some(_) => {}
                        None => namespace = Some(entry.namespace.as_str()),
                    }
                }
                namespace
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Many(entries) => entries.len(),
        }
    }
}

impl BackendServiceEntry {
    fn matches(&self, namespace: &str, port: u32) -> bool {
        self.namespace == namespace && self.port == port
    }
}

// ---------------------------------------------------------------------------
// Snapshot — the main IR type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub listeners: Vec<Listener>,
    pub http_routes: Vec<HttpRoute>,
    pub grpc_routes: Vec<GrpcRoute>,
    pub stream_routes: Vec<StreamRoute>,
    pub backends: Vec<BackendCluster>,
    pub backend_policies: BTreeMap<String, BackendPolicy>,
    pub secrets: Vec<SecretMaterial>,
    pub workloads: Vec<Workload>,
    #[serde(skip)]
    pub selection_state: crate::SelectionState,
    #[doc(hidden)]
    #[serde(skip)]
    pub endpoint_runtime: EndpointRuntimeStore,
    #[doc(hidden)]
    #[serde(skip)]
    pub runtime_ids: RuntimeIdIndex,
    #[doc(hidden)]
    #[serde(skip)]
    pub backend_names: Vec<Arc<str>>,
    #[doc(hidden)]
    #[serde(skip)]
    pub backend_index: HashMap<Arc<str>, usize>,
    #[doc(hidden)]
    #[serde(skip)]
    pub backend_service_index: BackendServiceIndex,
    #[doc(hidden)]
    #[serde(skip)]
    pub secret_index: HashMap<String, usize>,
    #[doc(hidden)]
    #[serde(skip)]
    pub workload_namespace_index: HashMap<String, String>,
    #[doc(hidden)]
    #[serde(skip)]
    pub runtime_indexes_ready: bool,
    #[doc(hidden)]
    #[serde(skip)]
    pub listener_name_index: HashMap<String, usize>,
    #[doc(hidden)]
    #[serde(skip)]
    pub http_listener_indices: Vec<usize>,
    #[doc(hidden)]
    #[serde(skip)]
    pub grpc_listener_indices: Vec<usize>,
    #[doc(hidden)]
    #[serde(skip)]
    pub http_listener_port_index: HashMap<u32, Vec<usize>>,
    #[doc(hidden)]
    #[serde(skip)]
    pub grpc_listener_port_index: HashMap<u32, Vec<usize>>,
    #[doc(hidden)]
    #[serde(skip)]
    pub frontend_client_certificate_index: HashMap<String, FrontendClientCertificateRequirement>,
    #[doc(hidden)]
    #[serde(skip)]
    pub service_frontend_index: HashMap<String, ServiceFrontendRef>,
    #[doc(hidden)]
    #[serde(skip)]
    pub service_frontend_listener_port_index: HashMap<u32, Vec<usize>>,
    #[doc(hidden)]
    #[serde(skip)]
    pub service_frontend_attachment_index: HashMap<String, HashSet<String>>,
    #[doc(hidden)]
    #[serde(skip)]
    pub route_attachment_listener_index: RouteAttachmentListenerIndex,
    #[doc(hidden)]
    #[serde(skip)]
    pub http_route_hostname_index: HostnameRouteIndex,
    #[doc(hidden)]
    #[serde(skip)]
    pub grpc_route_hostname_index: HostnameRouteIndex,
    #[doc(hidden)]
    #[serde(skip)]
    pub stream_listener_route_index: HashMap<String, Vec<usize>>,
    #[doc(hidden)]
    #[serde(skip)]
    pub request_materialization: RequestMaterializationHints,
    #[doc(hidden)]
    #[serde(skip)]
    pub http_fast_path: HttpFastPathPlan,
    #[doc(hidden)]
    #[serde(skip)]
    pub stream_fast_path: StreamFastPathPlan,
}

// ---------------------------------------------------------------------------
// Listener and TLS types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Listener {
    pub name: String,
    pub address: String,
    pub addresses: Vec<String>,
    pub port: u32,
    pub protocol: String,
    pub hostnames: Vec<String>,
    pub attached_routes: Vec<String>,
    pub tls: Option<TlsConfig>,
    pub backend_tls: Option<BackendTlsConfig>,
    pub metadata: BTreeMap<String, String>,
    pub security_policy: Option<SecurityPolicyConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub passthrough: bool,
    pub secret_refs: Vec<String>,
    pub sni_hosts: Vec<String>,
    pub min_version: String,
    pub max_version: String,
    pub frontend_validation: Option<FrontendValidation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrontendValidation {
    pub ca_pems: Vec<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendTlsConfig {
    pub client_certificate_ref: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendTlsValidation {
    pub hostname: String,
    pub use_system_ca_certificates: bool,
    pub ca_pems: Vec<String>,
    pub subject_alt_names: Vec<BackendSubjectAltName>,
    pub min_version: String,
    pub max_version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendSubjectAltName {
    pub kind: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// HTTP route types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRoute {
    pub name: String,
    pub namespace: String,
    pub hostnames: Vec<String>,
    pub parent_refs: Vec<ParentRef>,
    pub rules: Vec<HttpRule>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub security_policy: Option<SecurityPolicyConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRule {
    pub name: String,
    pub matches: Vec<HttpMatch>,
    pub filters: Vec<Filter>,
    pub backend_refs: Vec<BackendRef>,
    pub timeouts: Option<RouteTimeouts>,
    pub retry: Option<RetryPolicy>,
    pub session_persistence: Option<SessionPersistence>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpMatch {
    pub path: String,
    pub path_type: String,
    pub method: String,
    pub headers: Vec<HeaderMatch>,
    pub query_params: Vec<QueryMatch>,
    #[serde(skip)]
    #[doc(hidden)]
    pub compiled_path_regex: Option<Arc<Regex>>,
}

// ---------------------------------------------------------------------------
// gRPC route types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpcRoute {
    pub name: String,
    pub namespace: String,
    pub hostnames: Vec<String>,
    pub parent_refs: Vec<ParentRef>,
    pub rules: Vec<GrpcRule>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub security_policy: Option<SecurityPolicyConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpcRule {
    pub name: String,
    pub matches: Vec<GrpcMatch>,
    pub filters: Vec<Filter>,
    pub backend_refs: Vec<BackendRef>,
    pub session_persistence: Option<SessionPersistence>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpcMatch {
    pub service: String,
    pub method: String,
    pub match_type: String,
    pub headers: Vec<HeaderMatch>,
    #[serde(skip)]
    #[doc(hidden)]
    pub compiled_service_regex: Option<Arc<Regex>>,
    #[serde(skip)]
    #[doc(hidden)]
    pub compiled_method_regex: Option<Arc<Regex>>,
}

// ---------------------------------------------------------------------------
// Stream route types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamRoute {
    pub name: String,
    pub namespace: String,
    pub kind: String,
    pub parent_refs: Vec<ParentRef>,
    pub rules: Vec<StreamRule>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub security_policy: Option<SecurityPolicyConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamRule {
    pub name: String,
    pub matches: Vec<StreamMatch>,
    pub backend_refs: Vec<BackendRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TlsRouteMode {
    #[default]
    Passthrough,
    Terminate,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamMatch {
    pub port: u32,
    pub sni_hostname: String,
    #[serde(default)]
    pub mode: TlsRouteMode,
}

// ---------------------------------------------------------------------------
// Backend types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendRef {
    pub group: String,
    pub kind: String,
    pub namespace: String,
    pub name: String,
    pub port: u32,
    #[serde(default = "default_backend_weight")]
    pub weight: u32,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default)]
    pub filters: Vec<Filter>,
}

impl Default for BackendRef {
    fn default() -> Self {
        Self {
            group: String::new(),
            kind: String::new(),
            namespace: String::new(),
            name: String::new(),
            port: 0,
            weight: default_backend_weight(),
            metadata: BTreeMap::new(),
            filters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendCluster {
    pub name: String,
    pub namespace: String,
    pub protocol: String,
    pub endpoints: Vec<BackendEndpoint>,
    pub wasm_plugin: Option<WasmPluginConfig>,
    pub ai_service: Option<AIServiceConfig>,
    pub token_policy: Option<TokenPolicyConfig>,
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    pub security_policy: Option<SecurityPolicyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub max_inflight_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginConfig {
    pub name: String,
    pub namespace: String,
    pub wasm_bytes: Vec<u8>,
    pub sha256: String,
    pub hooks: Vec<String>,
    pub config_json: String,
    pub sandbox: WasmSandboxConfig,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSandboxConfig {
    pub max_memory_bytes: u64,
    pub max_execution_time_ms: u64,
    pub allow_network: bool,
    pub allow_file_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIServiceConfig {
    pub provider: String,
    pub format: String,
    pub model: String,
    pub endpoint: String,
    pub auth: Option<AIServiceAuthConfig>,
    pub timeout_secs: Option<u64>,
    pub retry_max_retries: u32,
    pub retry_backoff_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIServiceAuthConfig {
    pub auth_type: String,
    pub secret_ref: String,
    pub key: String,
    pub header: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPolicyConfig {
    pub tokens_per_minute: u64,
    pub tokens_per_hour: u64,
    pub requests_per_minute: u64,
    pub scope: String,
    pub burst: f64,
    pub on_limit: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendEndpoint {
    pub address: String,
    pub port: u32,
    pub healthy: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendPolicy {
    pub connect_timeout: Option<std::time::Duration>,
    pub request_timeout: Option<std::time::Duration>,
    pub tls_validation: Option<BackendTlsValidation>,
    pub session_persistence: Option<SessionPersistence>,
    pub load_balancing: Option<LoadBalancingPolicy>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoadBalancingPolicy {
    pub policy_type: String,
    pub consistent_hash: Option<ConsistentHashPolicy>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsistentHashPolicy {
    pub key_type: String,
    pub header_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default)]
    pub codes: Vec<u32>,
    pub attempts: u32,
    pub backoff: Option<std::time::Duration>,
}

// ---------------------------------------------------------------------------
// Secret, Header, Query, RequestMeta types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretMaterial {
    pub namespace: String,
    pub name: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub htpasswd: String,
    pub oidc_client_secret: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeaderMatch {
    pub name: String,
    pub value: String,
    pub match_type: String,
    #[serde(skip)]
    #[doc(hidden)]
    pub compiled_regex: Option<Arc<Regex>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryMatch {
    pub name: String,
    pub value: String,
    pub match_type: String,
    #[serde(skip)]
    #[doc(hidden)]
    pub compiled_regex: Option<Arc<Regex>>,
}

#[derive(Debug, Clone, Default)]
pub struct RequestMeta {
    pub host: Option<String>,
    pub port: u32,
    pub path: String,
    pub method: String,
    pub source_ip: Option<String>,
    pub headers: BTreeMap<String, Vec<String>>,
    pub query_params: BTreeMap<String, Vec<String>>,
}

// ---------------------------------------------------------------------------
// Security Policy types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityPolicyConfig {
    pub authn: Option<SecurityAuthNConfig>,
    pub authz: Option<SecurityAuthZConfig>,
    pub cors: Option<SecurityCorsConfig>,
    #[serde(default)]
    pub rate_limit: Vec<RateLimitRule>,
    pub ip: Option<SecurityIpConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityAuthNConfig {
    pub jwt: Option<JwtAuthConfig>,
    pub oidc: Option<OidcAuthConfig>,
    pub basic_auth: Option<BasicAuthConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityAuthZConfig {
    pub external: Option<ExternalAuthConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JwtAuthConfig {
    pub issuer: String,
    pub jwks_url: String,
    pub audience: String,
    pub header_name: String,
    pub token_prefix: String,
    #[serde(default)]
    pub claims_to_headers: BTreeMap<String, String>,
    pub cache_ttl_secs: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OidcAuthConfig {
    pub provider_authorization_url: String,
    pub provider_token_url: String,
    pub provider_jwks_url: String,
    pub provider_userinfo_url: String,
    pub client_id: String,
    pub client_secret_ref: String,
    pub callback_path: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_url: String,
    pub session_signing_key_ref: String,
    pub session_cookie_name: String,
    pub session_ttl_secs: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BasicAuthConfig {
    pub htpasswd_ref: String,
    pub bcrypt: bool,
    pub realm: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalAuthConfig {
    pub protocol: String,
    pub backend_ref: Option<BackendRef>,
    pub http: Option<ExternalHttpAuth>,
    pub grpc: Option<ExternalGrpcAuth>,
    pub forward_body_max_size: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalHttpAuth {
    pub path_prefix: String,
    #[serde(default)]
    pub headers_to_add: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalGrpcAuth {
    pub grpc_service: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityCorsConfig {
    #[serde(default)]
    pub allow_origins: Vec<String>,
    #[serde(default)]
    pub allow_methods: Vec<String>,
    #[serde(default)]
    pub allow_headers: Vec<String>,
    #[serde(default)]
    pub expose_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimitRule {
    pub scope: String,
    pub requests_per_second: u32,
    pub burst: u32,
    pub key_type: String,
    pub on_limit: String,
    pub key_header_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityIpConfig {
    #[serde(default)]
    pub allow_cidrs: Vec<String>,
    #[serde(default)]
    pub deny_cidrs: Vec<String>,
}
