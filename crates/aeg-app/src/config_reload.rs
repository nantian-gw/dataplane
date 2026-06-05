use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{interval, MissedTickBehavior},
};
use tracing::{info, warn};

use aeg_config::{DataPlaneConfig, ReloadingDataPlaneConfig};
use aeg_http::SessionPersistenceOptions;
use aeg_observability::{
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

pub(crate) type SharedAdminConfig = Arc<RwLock<AdminRuntimeConfig>>;
pub(crate) type SharedCircuitBreakerController = Arc<RwLock<HttpCircuitBreakerController>>;
pub(crate) type SharedRateLimitController = Arc<RwLock<HttpRateLimitController>>;
pub(crate) type SharedRetryBudgetController = Arc<RwLock<RetryBudgetController>>;

#[derive(Clone)]
pub(crate) struct ConfigSnapshot {
    pub(crate) admin: AdminRuntimeConfig,
    pub(crate) http: aeg_http::ReloadableRuntimeConfig,
    pub(crate) shared_tls: aeg_shared_tls::ReloadableRuntimeConfig,
    pub(crate) stream: aeg_stream::ReloadableRuntimeConfig,
    pub(crate) xds: XdsRuntimeConfig,
    pub(crate) active_health: ReloadableProbeConfig,
    pub(crate) circuit_breaker: aeg_observability::HttpCircuitBreakerOptions,
    pub(crate) rate_limit: aeg_observability::HttpRateLimitOptions,
    pub(crate) retry_budget: aeg_observability::RetryBudgetOptions,
    pub(crate) request_mirror_max_concurrency: usize,
}

pub(crate) struct ReloadTargets {
    pub(crate) admin: SharedAdminConfig,
    pub(crate) http: watch::Sender<Arc<aeg_http::ReloadableRuntimeConfig>>,
    pub(crate) shared_tls: watch::Sender<Arc<aeg_shared_tls::ReloadableRuntimeConfig>>,
    pub(crate) stream: watch::Sender<Arc<aeg_stream::ReloadableRuntimeConfig>>,
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
    let http = aeg_http::ReloadableRuntimeConfig {
        runtime: to_http_runtime_options(cfg),
        access_log: to_access_log_options(&cfg.access_log),
        session_persistence,
    };
    let retry_budget = http.runtime.retry_budget.clone();
    let stream = aeg_stream::ReloadableRuntimeConfig {
        runtime: to_stream_runtime_options(cfg),
        access_log: to_access_log_options(&cfg.access_log),
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
        shared_tls: aeg_shared_tls::ReloadableRuntimeConfig {
            runtime: aeg_shared_tls::RuntimeOptions {
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
    aeg_http::configure_request_mirror_budget(snapshot.request_mirror_max_concurrency);
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
        let mut ticker = interval(CONFIG_RELOAD_POLL_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = shutdown.changed() => break,
            }

            let updated = match source.load_if_changed() {
                Ok(Some(cfg)) => cfg,
                Ok(None) => continue,
                Err(err) => {
                    warn!(error = %err, "failed to reload dataplane config file");
                    continue;
                }
            };

            let snapshot = match build_config_snapshot(&updated) {
                Ok(snapshot) => snapshot,
                Err(err) => {
                    warn!(error = %err, "ignored invalid dataplane config update");
                    continue;
                }
            };

            apply_config_snapshot(snapshot, &targets);
            info!("reloaded dataplane config");
        }
    })
}

#[cfg(test)]
mod tests;
