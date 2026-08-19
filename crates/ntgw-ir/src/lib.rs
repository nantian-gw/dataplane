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
mod types;

// Re-export all public types from the types module
pub use types::{
    AIServiceAuthConfig, AIServiceConfig, BackendCluster, BackendEndpoint, BackendPolicy,
    BackendRef, BackendServiceIndex, BackendSubjectAltName, BackendTlsConfig, BackendTlsValidation,
    CircuitBreakerConfig, ConsistentHashPolicy, FrontendClientCertificateRequirement,
    FrontendValidation, GrpcMatch, GrpcRoute, GrpcRule, HeaderMatch, HostnameRouteIndex, HttpMatch,
    HttpRoute, HttpRule, Listener, LoadBalancingPolicy, QueryMatch, RequestMaterializationHints,
    RequestMeta, RetryPolicy, RouteAttachmentListenerIndex, SecretMaterial, Snapshot, StreamMatch,
    StreamRoute, StreamRule, TlsConfig, TlsRouteMode, TokenPolicyConfig, WasmPluginConfig,
    WasmSandboxConfig,
    SecurityPolicyConfig, SecurityAuthNConfig, SecurityAuthZConfig, SecurityCorsConfig,
    JwtAuthConfig, OidcAuthConfig, BasicAuthConfig, ExternalAuthConfig,
    ExternalHttpAuth, ExternalGrpcAuth, RateLimitRule, SecurityIpConfig,
};

// Re-export internal types from other modules
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

// Re-export constants from types
pub(crate) use types::BACKEND_REF_META_VALID;
pub(crate) use types::{HttpBackendResolution, ResolvedHttpBackend, StreamMatchScore};
pub use types::{PASSIVE_EJECTION_CONSECUTIVE_FAILURES, PASSIVE_EJECTION_COOLDOWN};

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use parking_lot::{Condvar, Mutex};
use regex::Regex;
use tokio::sync::watch;

pub type SharedSnapshot = Arc<ArcSwap<Snapshot>>;
pub type SharedSnapshotSignal = Arc<SnapshotSignal>;

// ---------------------------------------------------------------------------
// SnapshotSignal
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// SelectionState
// ---------------------------------------------------------------------------

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
    pub(crate) fn next_backend_ticket(&self) -> u64 {
        self.backend_cursor.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn next_endpoint_ticket(&self) -> u64 {
        self.endpoint_cursor.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn next_mirror_ticket(&self) -> u64 {
        self.mirror_cursor.fetch_add(1, Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// EndpointRuntimeKey
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// EndpointRuntimeState
// ---------------------------------------------------------------------------

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
