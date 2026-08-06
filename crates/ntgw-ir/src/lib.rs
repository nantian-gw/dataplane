#![forbid(unsafe_code)]

pub mod bench;
mod endpoint_runtime;
mod filters;
mod http_fast_path;
mod http_selection;
mod matching;
mod mesh;
mod proto;
mod runtime_id;
mod selection;
mod session;
mod snapshot;
mod stream_fast_path;
#[cfg(test)]
mod tests;
mod timeouts;

use std::{
    collections::{BTreeMap, HashMap, HashSet, hash_map::Entry},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use form_urlencoded::parse;
use parking_lot::{Condvar, Mutex};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

pub(crate) use endpoint_runtime::EndpointRuntimeStore;
pub use endpoint_runtime::{EndpointRuntimeHandle, EndpointRuntimeSnapshot};
pub use filters::{
    ClaimToHeader, CorsFilter, DirectResponseFilter, ExtensionFilter, ExternalAuthFilter,
    ExternalGRPCAuthConfig, ExternalHTTPAuthConfig, Filter, Fraction, HeaderModifier,
    HeaderOperation, JwtAuthFilter, MatchedHttpPath, PathModifier, RequestMirrorFilter,
    RequestRedirectFilter, UrlRewriteFilter,
};
pub use http_fast_path::{CompiledSelectedHttpBackend, HttpFastPathPlan, HttpFastPathRequest};
pub(crate) use matching::{
    GrpcPath, best_stream_rule_match_with_tls_mode, default_http_path_match,
    filters_without_request_mirror, has_non_backend_http_filter, hostname_matches, is_grpc_request,
    matches_grpc_rule, matches_http_rule, normalize_host_ref, normalize_http_path_match,
    parse_grpc_path,
};
pub use mesh::{ParentRef, Workload};
pub use runtime_id::{RuntimeId, RuntimeIdIndex, RuntimeResourceRef, SelectedBackendRuntimeIds};
pub use selection::{
    BackendSelectionError, RequestMirrorContext, RouteKind, SelectedBackend, SelectedHttpRoute,
};
pub use session::{CookieConfig, PersistentSessionTarget, SessionPersistence};
pub use stream_fast_path::StreamFastPathPlan;
pub use timeouts::RouteTimeouts;

pub type SharedSnapshot = Arc<ArcSwap<Snapshot>>;
pub type SharedSnapshotSignal = Arc<SnapshotSignal>;

const BACKEND_REF_META_VALID: &str = "nantian.dev/backend-ref-valid";
#[doc(hidden)]
pub const PASSIVE_EJECTION_CONSECUTIVE_FAILURES: u32 = 3;
#[doc(hidden)]
pub const PASSIVE_EJECTION_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub struct SnapshotSignal {
    generation: AtomicU64,
    state: Mutex<u64>,
    ready: Condvar,
    tx: watch::Sender<u64>,
}

impl Default for SnapshotSignal {
    fn default() -> Self {
        let (tx, _rx) = watch::channel(0);

        Self {
            generation: AtomicU64::new(0),
            state: Mutex::new(0),
            ready: Condvar::new(),
            tx,
        }
    }
}

impl SnapshotSignal {
    pub fn shared() -> SharedSnapshotSignal {
        Arc::new(Self::default())
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn notify_changed(&self) -> u64 {
        let next = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self.state.lock() = next;
        self.ready.notify_all();
        let _ = self.tx.send(next);
        next
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.tx.subscribe()
    }

    pub fn wait_timeout(&self, observed: u64, timeout: Duration) -> u64 {
        let mut state = self.state.lock();
        while *state <= observed {
            if self.ready.wait_for(&mut state, timeout).timed_out() {
                break;
            }
        }
        *state
    }
}

#[derive(Debug, Clone)]
pub struct SelectionState {
    backend_cursor: Arc<AtomicU64>,
    endpoint_cursor: Arc<AtomicU64>,
    mirror_cursor: Arc<AtomicU64>,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            backend_cursor: Arc::new(AtomicU64::new(0)),
            endpoint_cursor: Arc::new(AtomicU64::new(0)),
            mirror_cursor: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl SelectionState {
    fn next_backend_ticket(&self) -> u64 {
        self.backend_cursor.fetch_add(1, Ordering::Relaxed)
    }

    fn next_endpoint_ticket(&self) -> u64 {
        self.endpoint_cursor.fetch_add(1, Ordering::Relaxed)
    }

    fn next_mirror_ticket(&self) -> u64 {
        self.mirror_cursor.fetch_add(1, Ordering::Relaxed)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct EndpointRuntimeKey {
    backend_name: String,
    address: String,
    port: u32,
}

impl EndpointRuntimeKey {
    pub(crate) fn new(backend_name: &str, endpoint: &BackendEndpoint) -> Self {
        Self {
            backend_name: backend_name.to_string(),
            address: endpoint.address.clone(),
            port: endpoint.port,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct EndpointRuntimeState {
    consecutive_failures: u32,
    ejected_until: Option<Instant>,
    recovery_started_at: Option<Instant>,
    active_consecutive_failures: u32,
    active_unhealthy: bool,
}

impl EndpointRuntimeState {
    pub(crate) fn is_default_state(&self) -> bool {
        self.consecutive_failures == 0
            && self.ejected_until.is_none()
            && self.recovery_started_at.is_none()
            && self.active_consecutive_failures == 0
            && !self.active_unhealthy
    }

    pub(crate) fn is_ejected_at(&self, now: Instant) -> bool {
        self.ejected_until.is_some_and(|until| until > now)
    }

    pub(crate) fn ejection_expired_at(&self, now: Instant) -> bool {
        self.ejected_until.is_some_and(|until| until <= now)
    }

    pub(crate) fn record_failure(&mut self, now: Instant) {
        if self.ejection_expired_at(now) {
            *self = Self::default();
        }

        let was_ejected = self.is_ejected_at(now);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
            if !was_ejected {
                self.recovery_started_at.get_or_insert(now);
            }
            self.ejected_until = Some(now + PASSIVE_EJECTION_COOLDOWN);
        }
    }

    pub(crate) fn record_success(&mut self) -> Option<u64> {
        let recovery_latency_ms = self.recovery_latency_ms(Instant::now());
        *self = Self::default();
        recovery_latency_ms
    }

    pub(crate) fn record_active_probe_failure(&mut self, unhealthy_threshold: u32) {
        let now = Instant::now();
        let threshold = unhealthy_threshold.max(1);
        let was_unhealthy = self.active_unhealthy;
        self.active_consecutive_failures = self.active_consecutive_failures.saturating_add(1);
        if self.active_consecutive_failures >= threshold {
            if !was_unhealthy {
                self.recovery_started_at.get_or_insert(now);
            }
            self.active_unhealthy = true;
        }
    }

    pub(crate) fn is_active_unhealthy(&self) -> bool {
        self.active_unhealthy
    }

    pub(crate) fn record_active_probe_success(&mut self) -> Option<u64> {
        let recovery_latency_ms = self.recovery_latency_ms(Instant::now());
        *self = Self::default();
        recovery_latency_ms
    }

    fn recovery_latency_ms(&self, now: Instant) -> Option<u64> {
        self.recovery_started_at.map(|started| {
            now.saturating_duration_since(started)
                .as_millis()
                .min(u64::MAX as u128) as u64
        })
    }
}

#[derive(Debug, Default)]
struct HttpBackendResolution {
    selected: Option<ResolvedHttpBackend>,
    error: Option<BackendSelectionError>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct StreamMatchScore {
    hostname_rank: u8,
    hostname_length: usize,
}

#[derive(Debug, Clone)]
struct ResolvedHttpBackend {
    endpoint: BackendEndpoint,
    backend_name: String,
    filters: Vec<Filter>,
    session_persistence: Option<SessionPersistence>,
}

#[derive(Debug, Clone, Default)]
pub struct HostnameRouteIndex {
    catch_all: Vec<usize>,
    exact: HashMap<String, Vec<usize>>,
    wildcard_suffix: HashMap<String, Vec<usize>>,
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
            let host = host.to_ascii_lowercase();
            if let Some(indices) = self.exact.get(&host) {
                next = min_candidate_index(next, first_index_after(indices, last));
            }
            for suffix in wildcard_hostname_suffixes(&host) {
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
    pub selection_state: SelectionState,
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
    pub service_frontend_index: HashMap<String, mesh::ServiceFrontendRef>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRoute {
    pub name: String,
    pub namespace: String,
    pub hostnames: Vec<String>,
    pub parent_refs: Vec<ParentRef>,
    pub rules: Vec<HttpRule>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpcRoute {
    pub name: String,
    pub namespace: String,
    pub hostnames: Vec<String>,
    pub parent_refs: Vec<ParentRef>,
    pub rules: Vec<GrpcRule>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamRoute {
    pub name: String,
    pub namespace: String,
    pub kind: String,
    pub parent_refs: Vec<ParentRef>,
    pub rules: Vec<StreamRule>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
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
    pub auth: Option<AIServiceAuthConfig>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIServiceAuthConfig {
    pub auth_type: String,
    pub secret_ref: String,
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
    pub connect_timeout: Option<Duration>,
    pub request_timeout: Option<Duration>,
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
    pub backoff: Option<Duration>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretMaterial {
    pub namespace: String,
    pub name: String,
    pub cert_pem: String,
    pub key_pem: String,
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

fn default_backend_weight() -> u32 {
    1
}
