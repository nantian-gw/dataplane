use std::{
    path::Path,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use notify::event::ModifyKind;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};
use tracing::{info, warn};

use ntgw_config::{DataPlaneConfig, ReloadingDataPlaneConfig};
use ntgw_http::SessionPersistenceOptions;
use ntgw_observability::{
    HttpCircuitBreakerController, HttpRateLimitController, RetryBudgetController,
};

use crate::{
    active_health::ReloadableProbeConfig,
    admin::AdminRuntimeConfig,
    config_mapping::{
        to_access_log_options, to_http_circuit_breaker_options, to_http_rate_limit_options,
        to_http_runtime_options, to_stream_runtime_options, to_xds_runtime_config,
    },
    xds_runtime::XdsRuntimeConfig,
};

pub(crate) const CONFIG_RELOAD_POLL_INTERVAL: Duration = Duration::from_millis(250);
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) type SharedAdminConfig = Arc<RwLock<AdminRuntimeConfig>>;
pub(crate) type SharedCircuitBreakerController = Arc<RwLock<HttpCircuitBreakerController>>;
pub(crate) type SharedRateLimitController = Arc<RwLock<HttpRateLimitController>>;
pub(crate) type SharedRetryBudgetController = Arc<RwLock<RetryBudgetController>>;

#[derive(Clone)]
pub(crate) struct ConfigSnapshot {
    pub(crate) admin: AdminRuntimeConfig,
    pub(crate) http: ntgw_http::ReloadableRuntimeConfig,
    pub(crate) shared_tls: ntgw_shared_tls::ReloadableRuntimeConfig,
    pub(crate) stream: ntgw_stream::ReloadableRuntimeConfig,
    pub(crate) xds: XdsRuntimeConfig,
    pub(crate) active_health: ReloadableProbeConfig,
    pub(crate) circuit_breaker: ntgw_observability::HttpCircuitBreakerOptions,
    pub(crate) rate_limit: ntgw_observability::HttpRateLimitOptions,
    pub(crate) retry_budget: ntgw_observability::RetryBudgetOptions,
    pub(crate) request_mirror_max_concurrency: usize,
}

impl std::fmt::Debug for ConfigSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigSnapshot").finish_non_exhaustive()
    }
}

pub(crate) struct ReloadTargets {
    pub(crate) admin: SharedAdminConfig,
    pub(crate) http: watch::Sender<Arc<ntgw_http::ReloadableRuntimeConfig>>,
    pub(crate) shared_tls: watch::Sender<Arc<ntgw_shared_tls::ReloadableRuntimeConfig>>,
    pub(crate) stream: watch::Sender<Arc<ntgw_stream::ReloadableRuntimeConfig>>,
    pub(crate) xds: watch::Sender<Arc<XdsRuntimeConfig>>,
    pub(crate) active_health: watch::Sender<Arc<ReloadableProbeConfig>>,
    pub(crate) circuit_breaker: SharedCircuitBreakerController,
    pub(crate) rate_limit: SharedRateLimitController,
    pub(crate) retry_budget: SharedRetryBudgetController,
}

pub(crate) fn build_config_snapshot(cfg: &DataPlaneConfig) -> Result<ConfigSnapshot> {
    let session_persistence =
        if let Some(shared_secret) = cfg.session_persistence.resolve_shared_secret() {
            SessionPersistenceOptions::build(Some(shared_secret), None)?
        } else {
            SessionPersistenceOptions::build(
                cfg.session_persistence.resolve_secret()?,
                (!cfg.session_persistence.secret_key_file.trim().is_empty())
                    .then(|| cfg.session_persistence.secret_key_file.clone()),
            )?
        };
    let session_persistence_uses_ephemeral_secret = session_persistence.uses_ephemeral_secret();
    let http = ntgw_http::ReloadableRuntimeConfig {
        runtime: to_http_runtime_options(cfg),
        access_log: to_access_log_options(&cfg.access_log)?,
        session_persistence,
    };
    let retry_budget = http.runtime.retry_budget.clone();
    let stream = ntgw_stream::ReloadableRuntimeConfig {
        runtime: to_stream_runtime_options(cfg),
        access_log: to_access_log_options(&cfg.access_log)?,
    };

    Ok(ConfigSnapshot {
        admin: AdminRuntimeConfig {
            admin_bearer_token: (!cfg.admin_auth.bearer_token.trim().is_empty())
                .then(|| cfg.admin_auth.bearer_token.clone()),
            admin_bearer_token_file: (!cfg.admin_auth.bearer_token_file.trim().is_empty())
                .then(|| cfg.admin_auth.bearer_token_file.clone()),
            cluster: cfg.cluster.clone(),
            http3_configured: cfg.runtime.enable_http3,
            node_id: cfg.node_id.clone(),
            session_persistence_uses_ephemeral_secret,
            snapshot_freshness_timeout: cfg.xds_transport.snapshot_freshness_timeout(),
        },
        shared_tls: ntgw_shared_tls::ReloadableRuntimeConfig {
            runtime: ntgw_shared_tls::RuntimeOptions {
                enable_ipv6: cfg.runtime.enable_ipv6,
                reload_retry_interval: std::cmp::min(
                    cfg.runtime_tuning.http_reload_retry_interval(),
                    cfg.runtime_tuning.stream_reload_retry_interval(),
                ),
            },
            http: http.clone(),
            stream: stream.clone(),
        },
        http,
        stream,
        xds: to_xds_runtime_config(cfg),
        active_health: ReloadableProbeConfig {
            enabled: cfg.runtime_tuning.active_health_check_enabled(),
            probe_interval: cfg.runtime_tuning.active_health_check_interval(),
            probe_timeout: cfg.runtime_tuning.active_health_check_timeout(),
            unhealthy_threshold: cfg.runtime_tuning.active_health_check_unhealthy_threshold(),
        },
        circuit_breaker: to_http_circuit_breaker_options(&cfg.runtime_protection),
        rate_limit: to_http_rate_limit_options(&cfg.runtime_protection),
        retry_budget,
        request_mirror_max_concurrency: cfg.runtime_tuning.request_mirror_max_concurrency,
    })
}

pub(crate) fn apply_config_snapshot(snapshot: ConfigSnapshot, targets: &ReloadTargets) {
    ntgw_http::configure_request_mirror_budget(snapshot.request_mirror_max_concurrency);
    *targets.admin.write().unwrap_or_else(|err| err.into_inner()) = snapshot.admin;
    *targets
        .circuit_breaker
        .write()
        .unwrap_or_else(|err| err.into_inner()) =
        HttpCircuitBreakerController::new(snapshot.circuit_breaker);
    *targets
        .rate_limit
        .write()
        .unwrap_or_else(|err| err.into_inner()) = HttpRateLimitController::new(snapshot.rate_limit);
    *targets
        .retry_budget
        .write()
        .unwrap_or_else(|err| err.into_inner()) = RetryBudgetController::new(snapshot.retry_budget);
    let _ = targets.http.send(Arc::new(snapshot.http));
    let _ = targets.shared_tls.send(Arc::new(snapshot.shared_tls));
    let _ = targets.stream.send(Arc::new(snapshot.stream));
    let _ = targets.xds.send(Arc::new(snapshot.xds));
    let _ = targets.active_health.send(Arc::new(snapshot.active_health));
}

pub(crate) fn spawn_config_reload_loop(
    source: ReloadingDataPlaneConfig,
    targets: ReloadTargets,
    mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let config_path = source.path().to_path_buf();
        let parent_dir = match config_path.parent() {
            Some(dir) => dir,
            None => {
                warn!(
                    "config file path has no parent directory, falling back to watchdog-only reload"
                );
                run_watchdog_loop(source, targets, shutdown).await;
                return;
            }
        };

        // Bridge: notify uses std::sync::mpsc, we need tokio::sync::mpsc for select!
        let (std_tx, std_rx) = std::sync::mpsc::channel();
        let (tokio_tx, mut event_rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            for event in std_rx {
                if tokio_tx.send(event).is_err() {
                    break;
                }
            }
        });

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = std_tx.send(event);
                }
            },
            Config::default(),
        ) {
            Ok(w) => w,
            Err(err) => {
                warn!(error = %err, "failed to create file-system watcher, falling back to watchdog-only reload");
                run_watchdog_loop(source, targets, shutdown).await;
                return;
            }
        };

        if let Err(err) = watcher.watch(parent_dir, RecursiveMode::NonRecursive) {
            warn!(error = %err, "failed to watch config directory");
            run_watchdog_loop(source, targets, shutdown).await;
            return;
        }

        let mut watchdog = interval(WATCHDOG_INTERVAL);
        watchdog.set_missed_tick_behavior(MissedTickBehavior::Delay);

        info!(path = %config_path.display(), "watching config file for changes");

        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    if !is_config_modify_event(&event, &config_path) {
                        continue;
                    }
                    // Brief debounce to let the write complete
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                _ = watchdog.tick() => {}
                _ = shutdown.changed() => break,
            }

            if let Err(err) = try_reload(&source, &targets) {
                warn!(error = %err, "config reload failed");
            }
        }
    })
}

fn is_config_modify_event(event: &notify::Event, config_path: &Path) -> bool {
    if !matches!(event.kind, EventKind::Modify(ModifyKind::Data(_))) {
        return false;
    }
    event.paths.iter().any(|p| p == config_path)
}

fn try_reload(source: &ReloadingDataPlaneConfig, targets: &ReloadTargets) -> Result<()> {
    let updated = match source.load_if_changed() {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return Ok(()),
        Err(err) => {
            return Err(anyhow::anyhow!(
                "failed to reload dataplane config file: {err}"
            ));
        }
    };

    let snapshot = build_config_snapshot(&updated)
        .map_err(|err| anyhow::anyhow!("ignored invalid dataplane config update: {err}"))?;

    apply_config_snapshot(snapshot, targets);
    info!("reloaded dataplane config");
    Ok(())
}

async fn run_watchdog_loop(
    source: ReloadingDataPlaneConfig,
    targets: ReloadTargets,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut watchdog = interval(WATCHDOG_INTERVAL);
    watchdog.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = watchdog.tick() => {}
            _ = shutdown.changed() => break,
        }

        if let Err(err) = try_reload(&source, &targets) {
            warn!(error = %err, "config reload failed");
        }
    }
}

#[cfg(test)]
mod tests;
