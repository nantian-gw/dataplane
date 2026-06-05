use super::*;
use aeg_observability::HttpAdmissionController;
use aeg_observability::HttpCircuitBreakerController;
use aeg_observability::HttpRateLimitController;
use aeg_observability::RetryBudgetController;
use aeg_observability::RuntimeListenerFailure;
use tracing::{error, info, warn};

#[derive(Debug, Default)]
pub(super) struct ListenerReplaceResult {
    pub(super) retry_start: bool,
    pub(super) first_error: Option<String>,
    pub(super) first_failed_listener: Option<String>,
    pub(super) failures: Vec<RuntimeListenerFailure>,
    pub(super) started_listeners: Vec<String>,
    pub(super) retained_listeners: Vec<String>,
}

pub(super) struct ActiveServer {
    pub(super) plan: ListenerPlan,
    pub(super) shutdown: watch::Sender<bool>,
    pub(super) join: thread::JoinHandle<()>,
    pub(super) asset_dir: Option<PathBuf>,
}

impl ActiveServer {
    fn is_finished(&self) -> bool {
        self.join.is_finished()
    }
}

#[derive(Default)]
pub(super) struct ListenerSet {
    tasks: BTreeMap<String, ActiveServer>,
}

pub(super) struct ListenerReplaceContext<'a> {
    pub(super) version: &'a str,
    pub(super) snapshot: SharedSnapshot,
    pub(super) runtime: RuntimeOptions,
    pub(super) access_log: AccessLogOptions,
    pub(super) session_persistence: SessionPersistenceOptions,
    pub(super) runtime_stats: &'a SharedRuntimeStats,
    pub(super) traffic: SharedTrafficStats,
    pub(super) admission: HttpAdmissionController,
    pub(super) circuit_breaker: HttpCircuitBreakerController,
    pub(super) rate_limit: HttpRateLimitController,
    pub(super) retry_budget: RetryBudgetController,
    pub(super) asset_root: &'a Path,
    pub(super) force_reload: bool,
    pub(super) stage_recorder: Option<&'a dyn aeg_observability::ApplyStageRecorder>,
}

impl ListenerSet {
    pub(super) fn replace(
        &mut self,
        plan: Option<ListenerPlan>,
        ctx: ListenerReplaceContext<'_>,
    ) -> ListenerReplaceResult {
        let updates = if ctx.force_reload {
            listener_updates_with_force_reload(
                &self.active_plan(),
                plan.as_ref(),
                &self.finished_tasks(),
                true,
            )
        } else {
            listener_updates(&self.active_plan(), plan.as_ref(), &self.finished_tasks())
        };
        if updates.start.is_empty() && updates.stop.is_empty() {
            return ListenerReplaceResult {
                retained_listeners: self
                    .active_bind_plan()
                    .unwrap_or_else(|| ListenerPlan {
                        listeners: Vec::new(),
                    })
                    .listeners
                    .iter()
                    .map(|listener| listener.name.clone())
                    .collect(),
                ..ListenerReplaceResult::default()
            };
        }

        let mut result = ListenerReplaceResult::default();
        let stop_binds = updates.stop.into_iter().collect::<BTreeSet<_>>();
        let (restart_after_stop, start_before_stop): (Vec<_>, Vec<_>) = updates
            .start
            .into_iter()
            .partition(|listener| stop_binds.contains(&listener.bind));
        let mut started = 0usize;
        let mut stopped = 0usize;
        let mut preflight_started_binds = Vec::new();

        for listener in start_before_stop {
            if let Some(bind) = self.start_listener(listener, &ctx, &mut result, &mut started) {
                preflight_started_binds.push(bind);
            }
        }

        if result.first_error.is_some() {
            for bind in preflight_started_binds {
                if let Some(server) = self.tasks.remove(&bind) {
                    stop_server(server);
                    started = started.saturating_sub(1);
                }
            }
            result.started_listeners.clear();
            result.retained_listeners = self
                .active_bind_plan()
                .unwrap_or_else(|| ListenerPlan {
                    listeners: Vec::new(),
                })
                .listeners
                .iter()
                .map(|listener| listener.name.clone())
                .collect();
            info!(
                active = self.tasks.len(),
                started,
                stopped,
                version = ctx.version,
                "http listeners applied"
            );
            return result;
        }

        for bind in stop_binds {
            if let Some(server) = self.tasks.remove(&bind) {
                stop_server(server);
                stopped += 1;
            }
        }

        for listener in restart_after_stop {
            self.start_listener(listener, &ctx, &mut result, &mut started);
        }

        info!(
            active = self.tasks.len(),
            started,
            stopped,
            version = ctx.version,
            "http listeners applied"
        );

        let active_plan = self.active_bind_plan().unwrap_or_else(|| ListenerPlan {
            listeners: Vec::new(),
        });
        let started_names = result
            .started_listeners
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        result.retained_listeners = active_plan
            .listeners
            .iter()
            .filter(|listener| !started_names.contains(&listener.name))
            .map(|listener| listener.name.clone())
            .collect();
        let referenced_prefixes = referenced_tls_asset_prefixes(&active_plan);
        if let Err(err) = cleanup_unused_tls_assets_in_dir(ctx.asset_root, &referenced_prefixes) {
            warn!(
                asset_root = %ctx.asset_root.display(),
                error = %err,
                "failed to prune unused tls listener assets"
            );
        }

        result
    }

    fn start_listener(
        &mut self,
        listener: PlannedListener,
        ctx: &ListenerReplaceContext<'_>,
        result: &mut ListenerReplaceResult,
        started: &mut usize,
    ) -> Option<String> {
        let bind = listener.bind.clone();
        match start_server_with_asset_root(
            ListenerPlan {
                listeners: vec![listener.clone()],
            },
            ctx.snapshot.clone(),
            ctx.runtime.clone(),
            ctx.access_log.clone(),
            ctx.session_persistence.clone(),
            ctx.traffic.clone(),
            ctx.admission.clone(),
            ctx.circuit_breaker.clone(),
            ctx.rate_limit.clone(),
            ctx.retry_budget.clone(),
            ctx.asset_root,
            ctx.stage_recorder,
        ) {
            Ok((server, asset_stats)) => {
                self.tasks.insert(bind.clone(), server);
                ctx.runtime_stats
                    .observe_http_tls_asset_reuses(asset_stats.reused);
                result.started_listeners.push(listener.name.clone());
                *started += 1;
                Some(bind)
            }
            Err(err) => {
                result.retry_start = true;
                let error_message = err.to_string();
                result.failures.push(RuntimeListenerFailure {
                    listener: listener.name.clone(),
                    message: error_message.clone(),
                });
                if result.first_error.is_none() {
                    result.first_error = Some(error_message.clone());
                    result.first_failed_listener = Some(listener.name.clone());
                }
                error!(
                    listener = %listener.name,
                    bind = %listener.bind,
                    error = %error_message,
                    "failed to start nantian http listener"
                );
                None
            }
        }
    }

    pub(super) fn active_bind_plan(&self) -> Option<ListenerPlan> {
        let listeners = self
            .tasks
            .values()
            .filter_map(|server| server.plan.listeners.first().cloned())
            .collect::<Vec<_>>();

        (!listeners.is_empty()).then_some(ListenerPlan { listeners })
    }

    fn active_plan(&self) -> BTreeMap<String, PlannedListener> {
        self.tasks
            .iter()
            .filter_map(|(bind, server)| {
                server
                    .plan
                    .listeners
                    .first()
                    .cloned()
                    .map(|listener| (bind.clone(), listener))
            })
            .collect()
    }

    pub(super) fn finished_tasks(&self) -> BTreeSet<String> {
        self.tasks
            .iter()
            .filter(|(_, server)| server.is_finished())
            .map(|(bind, _)| bind.clone())
            .collect()
    }

    pub(super) fn shutdown_all(&mut self) {
        for server in std::mem::take(&mut self.tasks).into_values() {
            stop_server(server);
        }
    }
}
