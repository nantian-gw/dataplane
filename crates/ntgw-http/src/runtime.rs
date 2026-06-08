use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use pingora::{
    apps::HttpServerOptions,
    listeners::tls::TlsSettings,
    protocols::l4::ext::TcpKeepalive,
    proxy::ProxyServiceBuilder,
    server::{RunArgs, Server, ShutdownSignal, ShutdownSignalWatch},
    tls::{pkey::PKey, ssl::SslVerifyMode, x509::X509},
};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use ntgw_ir::{
    Listener, SecretMaterial, SharedSnapshot, SharedSnapshotSignal, Snapshot, TlsConfig,
};
use ntgw_observability::{
    AccessLogOptions, HttpAdmissionController, HttpAdmissionOptions, HttpCircuitBreakerController,
    HttpCircuitBreakerOptions, HttpRateLimitController, HttpRateLimitOptions,
    RetryBudgetController, RetryBudgetOptions, RuntimeStatsSnapshot, SharedApplyStageRecorder,
    SharedOverloadStats, SharedRuntimeStats, SharedTrafficStats,
};

use crate::proxy::GatewayProxy;
use crate::session::SessionPersistenceOptions;

const LISTENER_ADDRESSES_METADATA_KEY: &str = "nantian.dev/listener-addresses";

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub default_listen_addr: String,
    pub enable_ipv6: bool,
    pub enable_http3: bool,
    pub tls_min_version: String,
    pub tls_max_version: String,
    pub tls_asset_dir: String,
    pub reload_retry_interval: Duration,
    pub downstream_read_timeout: Option<Duration>,
    pub downstream_max_connection_age: Option<Duration>,
    pub keepalive_request_limit: Option<u32>,
    pub capacity: HttpCapacityOptions,
    pub downstream_tcp_keepalive: Option<TcpKeepalive>,
    pub upstream_tcp_keepalive: Option<TcpKeepalive>,
    pub request_tracing_enabled: bool,
    pub admission: HttpAdmissionOptions,
    pub circuit_breaker: HttpCircuitBreakerOptions,
    pub rate_limit: HttpRateLimitOptions,
    pub retry_budget: RetryBudgetOptions,
    pub max_request_body_bytes: usize,
    pub max_request_header_bytes: usize,
    pub work_stealing: bool,
    pub downstream_tcp_fastopen: Option<usize>,
    pub downstream_dscp: Option<u8>,
    pub upstream_tcp_fast_open: bool,
    pub upstream_tcp_recv_buf: Option<usize>,
    pub upstream_connection_timeout: Option<Duration>,
    pub upstream_read_timeout: Option<Duration>,
    pub upstream_idle_timeout: Option<Duration>,
    pub upstream_dscp: Option<u8>,
    pub cache: std::sync::Arc<crate::cache::CacheManager>,
    pub experimental: ntgw_config::ExperimentalConfig,
}

#[derive(Clone)]
pub struct ReloadableRuntimeConfig {
    pub runtime: RuntimeOptions,
    pub access_log: AccessLogOptions,
    pub session_persistence: SessionPersistenceOptions,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            default_listen_addr: String::new(),
            enable_ipv6: true,
            enable_http3: false,
            tls_min_version: "1.2".to_string(),
            tls_max_version: "1.3".to_string(),
            tls_asset_dir: String::new(),
            reload_retry_interval: Duration::from_secs(1),
            downstream_read_timeout: Some(Duration::from_secs(60)),
            downstream_max_connection_age: None,
            keepalive_request_limit: None,
            capacity: HttpCapacityOptions::default(),
            downstream_tcp_keepalive: None,
            upstream_tcp_keepalive: None,
            request_tracing_enabled: false,
            admission: HttpAdmissionOptions::default(),
            circuit_breaker: HttpCircuitBreakerOptions::default(),
            rate_limit: HttpRateLimitOptions::default(),
            retry_budget: RetryBudgetOptions::default(),
            max_request_body_bytes: 0,
            max_request_header_bytes: 0,
            work_stealing: true,
            downstream_tcp_fastopen: None,
            downstream_dscp: None,
            upstream_tcp_fast_open: false,
            upstream_tcp_recv_buf: None,
            upstream_connection_timeout: None,
            upstream_read_timeout: None,
            upstream_idle_timeout: None,
            upstream_dscp: None,
            cache: crate::cache::CacheManager::new(crate::cache::CacheOptions {
                enabled: false,
                max_size_bytes: 0,
                default_ttl: Duration::from_secs(0),
            }),
            experimental: ntgw_config::ExperimentalConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListenerPlan {
    listeners: Vec<PlannedListener>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedListener {
    name: String,
    bind: String,
    protocol: ListenerProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ListenerProtocol {
    Plain,
    Tls(TlsMaterial),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TlsMaterial {
    identities: Vec<TlsIdentity>,
    min_version: String,
    max_version: String,
    client_ca_bundle_pem: Option<String>,
    frontend_validation_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TlsIdentity {
    secret_ref: String,
    cert_pem: String,
    key_pem: String,
    match_names: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ListenerUpdatePlan {
    start: Vec<PlannedListener>,
    stop: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ListenerPlanBuildResult {
    plan: Option<ListenerPlan>,
    retry_start: bool,
    deferred_binds: Vec<String>,
}

pub fn http3_available() -> bool {
    false
}

#[allow(clippy::too_many_arguments)]
pub fn spawn(
    snapshot: SharedSnapshot,
    updates: SharedSnapshotSignal,
    mut config: watch::Receiver<std::sync::Arc<ReloadableRuntimeConfig>>,
    runtime_stats: SharedRuntimeStats,
    traffic: SharedTrafficStats,
    overload: SharedOverloadStats,
    circuit_breaker: Arc<RwLock<HttpCircuitBreakerController>>,
    rate_limit: Arc<RwLock<HttpRateLimitController>>,
    retry_budget: Arc<RwLock<RetryBudgetController>>,
    stage_recorder: Option<SharedApplyStageRecorder>,
    shutdown: watch::Receiver<bool>,
) -> Result<thread::JoinHandle<()>> {
    let initial = config.borrow().clone();
    let asset_root = tls_asset_root(&initial.runtime);
    fs::create_dir_all(&asset_root)?;
    let handle = thread::spawn(move || {
        let shutdown = shutdown;
        let mut current = initial;
        if current.runtime.enable_http3 && !http3_available() {
            warn!("HTTP/3 is configured but unsupported by the current Nantian build");
        }

        let mut active = ListenerSet::default();
        let mut observed_generation = updates.generation();
        let mut refresh_runtime = true;
        let mut force_reload = true;
        loop {
            if *shutdown.borrow() {
                break;
            }

            if config.has_changed().unwrap_or(false) {
                current = config.borrow_and_update().clone();
                refresh_runtime = true;
                force_reload = true;
                if current.runtime.enable_http3 && !http3_available() {
                    warn!("HTTP/3 is configured but unsupported by the current Nantian build");
                }
            }

            let mut retry_start = false;
            if refresh_runtime {
                let runtime = current.runtime.clone();
                let access_log = current.access_log.clone();
                let session_persistence = current.session_persistence.clone();
                if session_persistence.uses_ephemeral_secret() {
                    warn!(
                        "session persistence is using an ephemeral, auto-generated secret — sessions will be invalidated on restart and cannot be shared across replicas; configure sharedSecret or sharedSecretFile"
                    );
                }
                let admission =
                    HttpAdmissionController::new(runtime.admission.clone(), overload.clone());
                let active_plan = active.active_bind_plan();
                let active_binds =
                    active_listener_binds_for_plan_build(active_plan.as_ref(), force_reload);
                let desired = {
                    let stage = Instant::now();
                    let current = snapshot.read();
                    let desired = build_listener_plan_for_runtime(
                        &current,
                        &runtime,
                        &active_binds,
                        &runtime_stats.snapshot(),
                    );
                    observe_reload_stage_elapsed(stage_recorder.as_deref(), "listener_plan", stage);
                    desired
                };
                let version = snapshot.read().id.clone();
                let active_circuit_breaker = circuit_breaker
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone();
                let active_rate_limit = rate_limit
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone();
                let active_retry_budget = retry_budget
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone();
                let result = active.replace(
                    desired.plan,
                    ListenerReplaceContext {
                        version: version.as_str(),
                        snapshot: snapshot.clone(),
                        runtime: runtime.clone(),
                        access_log,
                        session_persistence,
                        runtime_stats: &runtime_stats,
                        traffic: traffic.clone(),
                        admission,
                        circuit_breaker: active_circuit_breaker,
                        rate_limit: active_rate_limit,
                        retry_budget: active_retry_budget,
                        asset_root: &asset_root,
                        force_reload,
                        stage_recorder: stage_recorder.as_deref(),
                    },
                );
                retry_start = desired.retry_start || result.retry_start;
                if !desired.retry_start || !result.failures.is_empty() {
                    runtime_stats.observe_http_listener_reload_result(
                        version.as_str(),
                        &result.started_listeners,
                        &result.retained_listeners,
                        &result.failures,
                    );
                }
                force_reload = false;
            }

            let next_generation =
                updates.wait_timeout(observed_generation, current.runtime.reload_retry_interval);
            refresh_runtime = next_generation != observed_generation || retry_start;
            observed_generation = next_generation;
            if *shutdown.borrow() {
                break;
            }
            if !active.finished_tasks().is_empty() {
                refresh_runtime = true;
            }
        }

        active.shutdown_all();
    });

    Ok(handle)
}

pub(super) fn observe_reload_stage_elapsed(
    stage_recorder: Option<&dyn ntgw_observability::ApplyStageRecorder>,
    stage: &str,
    started_at: Instant,
) {
    if let Some(stage_recorder) = stage_recorder {
        stage_recorder.observe_apply_stage_duration(
            stage,
            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        );
    }
}

fn tls_asset_root(runtime: &RuntimeOptions) -> PathBuf {
    let configured = runtime.tls_asset_dir.trim();
    if !configured.is_empty() {
        return PathBuf::from(configured);
    }

    std::env::temp_dir()
        .join("nantian-gw")
        .join("http-listeners")
        .join(unique_asset_dir_name())
}

fn listener_updates(
    active: &BTreeMap<String, PlannedListener>,
    desired: Option<&ListenerPlan>,
    finished: &BTreeSet<String>,
) -> ListenerUpdatePlan {
    listener_updates_with_force_reload(active, desired, finished, false)
}

fn listener_updates_with_force_reload(
    active: &BTreeMap<String, PlannedListener>,
    desired: Option<&ListenerPlan>,
    finished: &BTreeSet<String>,
    force_reload: bool,
) -> ListenerUpdatePlan {
    let mut desired_by_bind = desired
        .map(|plan| {
            plan.listeners
                .iter()
                .cloned()
                .map(|listener| (listener.bind.clone(), listener))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut updates = ListenerUpdatePlan::default();

    for (bind, listener) in active {
        if force_reload && !finished.contains(bind) {
            if let Some(next) = desired_by_bind.remove(bind) {
                updates.stop.push(bind.clone());
                updates.start.push(next);
                continue;
            }
            updates.stop.push(bind.clone());
            continue;
        }
        match desired_by_bind.remove(bind) {
            Some(next) if !finished.contains(bind) && listener.protocol == next.protocol => {}
            Some(next) => {
                updates.stop.push(bind.clone());
                updates.start.push(next);
            }
            None => updates.stop.push(bind.clone()),
        }
    }

    updates.start.extend(desired_by_bind.into_values());
    updates
}

#[cfg(test)]
fn build_listener_plan(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_plan: Option<&ListenerPlan>,
) -> Option<ListenerPlan> {
    let active_binds = active_listener_binds(active_plan);
    build_listener_plan_with_bind_checker(snapshot, runtime, &active_binds, |_| Ok(()))
}

fn build_listener_plan_for_runtime(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    runtime_state: &RuntimeStatsSnapshot,
) -> ListenerPlanBuildResult {
    #[cfg(test)]
    {
        build_listener_plan_with_bind_checker_for_runtime(
            snapshot,
            runtime,
            active_binds,
            |_| Ok(()),
            runtime_state,
        )
    }

    #[cfg(not(test))]
    {
        build_listener_plan_with_bind_checker_for_runtime(
            snapshot,
            runtime,
            active_binds,
            probe_listener_bind,
            runtime_state,
        )
    }
}

fn active_listener_binds_for_plan_build(
    active_plan: Option<&ListenerPlan>,
    _force_reload: bool,
) -> BTreeSet<String> {
    active_listener_binds(active_plan)
}

#[cfg(test)]
fn build_listener_plan_with_bind_checker<F>(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    mut bind_checker: F,
) -> Option<ListenerPlan>
where
    F: FnMut(&str) -> io::Result<()>,
{
    build_listener_plan_with_bind_checker_inner(
        snapshot,
        runtime,
        active_binds,
        &mut bind_checker,
        &RuntimeStatsSnapshot::default(),
        true,
    )
    .plan
}

fn build_listener_plan_with_bind_checker_for_runtime<F>(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    mut bind_checker: F,
    runtime_state: &RuntimeStatsSnapshot,
) -> ListenerPlanBuildResult
where
    F: FnMut(&str) -> io::Result<()>,
{
    build_listener_plan_with_bind_checker_inner(
        snapshot,
        runtime,
        active_binds,
        &mut bind_checker,
        runtime_state,
        false,
    )
}

fn build_listener_plan_with_bind_checker_inner<F>(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    mut bind_checker: F,
    runtime_state: &RuntimeStatsSnapshot,
    include_tls_terminating_listeners: bool,
) -> ListenerPlanBuildResult
where
    F: FnMut(&str) -> io::Result<()>,
{
    let has_declared_l7 = snapshot
        .listeners
        .iter()
        .any(|listener| is_l7_protocol(&listener.protocol));
    let blocked_tls_binds = tls_passthrough_binds(snapshot, runtime);
    let mut endpoints: BTreeMap<String, PlannedListener> = BTreeMap::new();
    let mut result = ListenerPlanBuildResult::default();

    for listener in &snapshot.listeners {
        match desired_listener_protocol(listener, snapshot, runtime) {
            Some(protocol) => {
                if matches!(protocol, ListenerProtocol::Tls(_))
                    && !include_tls_terminating_listeners
                {
                    continue;
                }
                for bind in listener_bind_addrs(listener, runtime) {
                    if !active_binds.contains(&bind)
                        && let Err(err) = bind_checker(bind.as_str())
                    {
                        if should_defer_http_listener_bind_handoff(
                            snapshot,
                            runtime_state,
                            &protocol,
                            bind.as_str(),
                            &blocked_tls_binds,
                            &err,
                        ) {
                            if !result.deferred_binds.contains(&bind) {
                                result.deferred_binds.push(bind.clone());
                            }
                            result.retry_start = true;
                            info!(
                                listener = %listener.name,
                                bind = %bind,
                                version = %snapshot.id,
                                stream_last_good_version = %runtime_state.stream_last_good_reload_version,
                                "delaying tls-terminating http listener until stream runtime releases the shared bind"
                            );
                            continue;
                        }
                        if should_suppress_unavailable_bind_warning(bind.as_str(), &err) {
                            debug!(
                                listener = %listener.name,
                                bind = %bind,
                                error = %err,
                                "skipping http listener because IPv6 address family is unavailable"
                            );
                            continue;
                        }
                        warn!(
                            listener = %listener.name,
                            bind = %bind,
                            error = %err,
                            "skipping http listener because the bind address is unavailable"
                        );
                        continue;
                    }

                    if matches!(protocol, ListenerProtocol::Tls(_))
                        && blocked_tls_binds.contains(&bind)
                    {
                        warn!(
                            listener = %listener.name,
                            bind = %bind,
                            "skipping tls-terminating http listener because a tls passthrough listener claims the same bind address"
                        );
                        continue;
                    }

                    let planned = PlannedListener {
                        name: listener.name.clone(),
                        bind: bind.clone(),
                        protocol: protocol.clone(),
                    };

                    match endpoints.get(&bind) {
                        Some(existing) if existing.protocol != planned.protocol => {
                            warn!(
                                bind = %bind,
                                first_listener = %existing.name,
                                skipped_listener = %planned.name,
                                "conflicting http listener configuration on the same bind address; keeping the first listener"
                            );
                        }
                        Some(_) => {}
                        None => {
                            endpoints.insert(bind, planned);
                        }
                    }
                }
            }
            None if is_http3_protocol(&listener.protocol) => {
                warn!(
                    listener = %listener.name,
                    "HTTP/3 listener requested but unavailable in the current Nantian build"
                );
            }
            None => {}
        }
    }

    if endpoints.is_empty() && !has_declared_l7 && !runtime.default_listen_addr.is_empty() {
        for default_bind in bind_variants(runtime.default_listen_addr.as_str(), runtime.enable_ipv6)
        {
            if !active_binds.contains(&default_bind)
                && let Err(err) = bind_checker(default_bind.as_str())
            {
                if should_suppress_unavailable_bind_warning(default_bind.as_str(), &err) {
                    debug!(
                        bind = %default_bind,
                        error = %err,
                        "skipping default http listener because IPv6 address family is unavailable"
                    );
                    continue;
                }
                warn!(
                    bind = %default_bind,
                    error = %err,
                    "skipping default http listener because the bind address is unavailable"
                );
                continue;
            }

            endpoints.insert(
                default_bind.clone(),
                PlannedListener {
                    name: "runtime/default-http".to_string(),
                    bind: default_bind,
                    protocol: ListenerProtocol::Plain,
                },
            );
        }
    }

    result.plan = (!endpoints.is_empty()).then(|| ListenerPlan {
        listeners: endpoints.into_values().collect(),
    });
    result
}

fn active_listener_binds(active_plan: Option<&ListenerPlan>) -> BTreeSet<String> {
    active_plan
        .map(|plan| {
            plan.listeners
                .iter()
                .map(|listener| listener.bind.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(test))]
fn probe_listener_bind(bind: &str) -> io::Result<()> {
    use std::net::TcpListener as StdTcpListener;

    StdTcpListener::bind(bind).map(drop)
}

fn tls_passthrough_binds(snapshot: &Snapshot, runtime: &RuntimeOptions) -> BTreeSet<String> {
    snapshot
        .listeners
        .iter()
        .filter(|listener| {
            listener
                .tls
                .as_ref()
                .is_some_and(|tls| tls.enabled && tls.passthrough)
        })
        .flat_map(|listener| listener_bind_addrs(listener, runtime))
        .collect()
}

fn should_defer_http_listener_bind_handoff(
    snapshot: &Snapshot,
    runtime_state: &RuntimeStatsSnapshot,
    protocol: &ListenerProtocol,
    bind: &str,
    blocked_tls_binds: &BTreeSet<String>,
    err: &io::Error,
) -> bool {
    if !matches!(protocol, ListenerProtocol::Tls(_)) {
        return false;
    }
    if err.kind() != io::ErrorKind::AddrInUse {
        return false;
    }
    if snapshot.id.is_empty() || blocked_tls_binds.contains(bind) {
        return false;
    }

    let has_prior_stream_state = !runtime_state.stream_last_reload_attempt_version.is_empty()
        || !runtime_state.stream_last_good_reload_version.is_empty();
    has_prior_stream_state && runtime_state.stream_last_good_reload_version != snapshot.id
}

fn should_suppress_unavailable_bind_warning(bind: &str, err: &io::Error) -> bool {
    bind.starts_with('[') && is_address_family_not_supported(err)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_address_family_not_supported(err: &io::Error) -> bool {
    err.raw_os_error() == Some(97)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn is_address_family_not_supported(_err: &io::Error) -> bool {
    false
}

pub mod bench;
mod capacity;
mod listener_plan;
mod listener_set;
mod server;

pub use self::capacity::HttpCapacityOptions;
#[cfg(test)]
use self::listener_plan::materialize_tls_assets;
#[cfg(test)]
use self::listener_plan::wildcard_hostname_matches;
use self::listener_plan::{
    RuntimeListener, RuntimeListenerProtocol, RuntimePlan, TlsAssetWriteStats, bind_variants,
    cleanup_unused_tls_assets_in_dir, desired_listener_protocol, is_http3_protocol, is_l7_protocol,
    listener_bind_addrs, materialize_runtime_plan, materialize_tls_assets_in_dir,
    ordered_tls_identity_candidates, referenced_tls_asset_prefixes, tcp_socket_options_for_bind,
    unique_asset_dir_name,
};
use self::listener_set::{ActiveServer, ListenerReplaceContext, ListenerSet};
pub use self::server::{AcceptedHttpApp, build_http_app, process_accepted_stream};
#[cfg(test)]
use self::server::{
    listener_port_hint, plain_http_server_options, start_server, start_server_with_overload_stats,
};
use self::server::{start_server_with_asset_root, stop_server};

#[cfg(test)]
mod tests;
