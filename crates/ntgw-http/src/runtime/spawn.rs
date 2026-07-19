use std::{fs, path::PathBuf, sync::Arc, thread, time::Instant};

use anyhow::Result;
use parking_lot::RwLock;
use tokio::sync::watch;
use tracing::warn;

use ntgw_ir::{SharedSnapshot, SharedSnapshotSignal};
use ntgw_observability::{
    HttpAdmissionController, HttpCircuitBreakerController, HttpRateLimitController,
    RetryBudgetController, SharedApplyStageRecorder, SharedOverloadStats, SharedRuntimeStats,
    SharedTrafficStats,
};

use super::http3_available;
use super::listener_plan::unique_asset_dir_name;
use super::listener_set::{ListenerReplaceContext, ListenerSet};
use super::options::{ReloadableRuntimeConfig, RuntimeOptions};
use super::plan::{active_listener_binds_for_plan_build, build_listener_plan_for_runtime};

pub(crate) fn observe_reload_stage_elapsed(
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

pub(crate) fn tls_asset_root(runtime: &RuntimeOptions) -> PathBuf {
    let configured = runtime.tls_asset_dir.trim();
    if !configured.is_empty() {
        return PathBuf::from(configured);
    }

    std::env::temp_dir()
        .join("nantian-gw")
        .join("http-listeners")
        .join(unique_asset_dir_name())
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
                    let current = snapshot.load();
                    let desired = build_listener_plan_for_runtime(
                        &current,
                        &runtime,
                        &active_binds,
                        &runtime_stats.snapshot(),
                    );
                    observe_reload_stage_elapsed(stage_recorder.as_deref(), "listener_plan", stage);
                    desired
                };
                let version = snapshot.load().id.clone();
                let active_circuit_breaker = circuit_breaker.read().clone();
                let active_rate_limit = rate_limit.read().clone();
                let active_retry_budget = retry_budget.read().clone();
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
