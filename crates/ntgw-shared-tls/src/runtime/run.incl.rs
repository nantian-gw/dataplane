#[allow(clippy::too_many_arguments)]
pub async fn run(
    snapshot: SharedSnapshot,
    updates: SharedSnapshotSignal,
    mut config: watch::Receiver<Arc<ReloadableRuntimeConfig>>,
    runtime_stats: SharedRuntimeStats,
    traffic: SharedTrafficStats,
    overload: SharedOverloadStats,
    circuit_breaker: Arc<RwLock<HttpCircuitBreakerController>>,
    rate_limit: Arc<RwLock<HttpRateLimitController>>,
    retry_budget: Arc<RwLock<RetryBudgetController>>,
    stage_recorder: Option<SharedApplyStageRecorder>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), SharedTlsError> {
    let mut active = SharedTlsBindSet::default();
    let mut subscription = updates.subscribe();
    let mut current = config.borrow().clone();
    let mut force_reload = true;

    loop {
        if *shutdown.borrow() {
            break;
        }

        if force_reload {
            current = config.borrow_and_update().clone();
        }

        let (desired, version, failures, tls_required) = {
            let stage = Instant::now();
            let current_snapshot = snapshot.load();
            let version = current_snapshot.id.clone();
            let tls_required = current_snapshot
                .listeners
                .iter()
                .any(shared_tls_listener_protocol);
            let result = match desired_listener_plan(&current_snapshot, &current.runtime) {
                Ok(plan) => (plan, version, Vec::new(), tls_required),
                Err(err) => {
                    let message = err.to_string();
                    let failures = shared_tls_listener_failures(&current_snapshot, &message);
                    (None, version, failures, tls_required)
                }
            };
            observe_reload_stage_elapsed(stage_recorder.as_deref(), "listener_plan", stage);
            result
        };

        if desired.is_none() && !failures.is_empty() {
            runtime_stats
                .observe_tls_listener_reload_failures(version.as_str(), failures.as_slice());
            force_reload = false;
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = config.changed() => {
                    force_reload = true;
                }
                result = timeout(current.runtime.reload_retry_interval, subscription.changed()) => match result {
                    Ok(Ok(())) | Err(_) => {}
                    Ok(Err(_)) => sleep(current.runtime.reload_retry_interval).await,
                }
            }
            continue;
        }

        if active.needs_reload(desired.as_ref(), force_reload) {
            let http_app = build_runtime_http_app(
                snapshot.clone(),
                current.as_ref(),
                traffic.clone(),
                overload.clone(),
                circuit_breaker.clone(),
                rate_limit.clone(),
                retry_budget.clone(),
            )?;
            let result = active
                .replace(
                    desired,
                    snapshot.clone(),
                    http_app,
                    shutdown.clone(),
                    force_reload,
                )
                .await;
            runtime_stats.observe_tls_listener_reload_result(
                version.as_str(),
                &result.started_listeners,
                &result.retained_listeners,
                &result.failures,
            );
        } else if desired.is_some() || tls_required {
            runtime_stats.observe_tls_listener_reload_result(
                version.as_str(),
                &[],
                &active.active_listener_names(),
                &[],
            );
        }
        force_reload = false;

        tokio::select! {
            _ = shutdown.changed() => break,
            _ = config.changed() => {
                force_reload = true;
            }
            result = timeout(current.runtime.reload_retry_interval, subscription.changed()) => match result {
                Ok(Ok(())) | Err(_) => {}
                Ok(Err(_)) => sleep(current.runtime.reload_retry_interval).await,
            }
        }
    }

    active.shutdown_all().await;
    Ok(())
}
