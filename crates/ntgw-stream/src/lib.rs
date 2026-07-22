#![forbid(unsafe_code)]

mod access_log;
pub mod bench;
mod listener_plan;
pub mod pool;
mod sni;
mod tcp;
#[cfg(test)]
mod tests;
mod traffic;
mod udp;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use anyhow::Result;
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{Duration, sleep, timeout},
};
use tracing::{error, info, warn};

use listener_plan::{
    ListenerPlan, PlannedListener, StreamProtocol, build_listener_plan, listener_updates,
    listener_updates_with_force_reload,
};
use ntgw_ir::{SharedSnapshot, SharedSnapshotSignal};
use ntgw_observability::{
    AccessLogOptions, RuntimeListenerFailure, SharedOverloadStats, SharedRuntimeStats,
    SharedTrafficStats, SharedUdpSessionStats, TcpAdmissionController, TcpAdmissionOptions,
    UdpAdmissionController, UdpAdmissionOptions,
};
use pool::TcpConnectionPool;

const DEFAULT_TCP_PROXY_BUFFER_BYTES: usize = 16 * 1024;
const MIN_TCP_PROXY_BUFFER_BYTES: usize = 4 * 1024;
const MAX_TCP_PROXY_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub reload_retry_interval: Duration,
    pub udp_response_idle_timeout: Duration,
    pub tcp_proxy_buffer_bytes: usize,
    pub tcp_session_idle_timeout: Option<Duration>,
    pub tcp_max_connection_age: Option<Duration>,
    pub tcp_admission: TcpAdmissionOptions,
    pub udp_admission: UdpAdmissionOptions,
    pub stream_upstream_pool_size: usize,
    pub stream_upstream_pool_idle_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct ReloadableRuntimeConfig {
    pub runtime: RuntimeOptions,
    pub access_log: AccessLogOptions,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            reload_retry_interval: Duration::from_secs(1),
            udp_response_idle_timeout: Duration::from_millis(500),
            tcp_proxy_buffer_bytes: DEFAULT_TCP_PROXY_BUFFER_BYTES,
            tcp_session_idle_timeout: None,
            tcp_max_connection_age: None,
            tcp_admission: TcpAdmissionOptions::default(),
            udp_admission: UdpAdmissionOptions::default(),
            stream_upstream_pool_size: 0,
            stream_upstream_pool_idle_timeout: Duration::from_secs(30),
        }
    }
}

impl RuntimeOptions {
    fn effective_tcp_proxy_buffer_bytes(&self) -> usize {
        normalize_tcp_proxy_buffer_bytes(self.tcp_proxy_buffer_bytes)
    }
}

pub(crate) fn normalize_tcp_proxy_buffer_bytes(value: usize) -> usize {
    match value {
        0 => DEFAULT_TCP_PROXY_BUFFER_BYTES,
        value => value.clamp(MIN_TCP_PROXY_BUFFER_BYTES, MAX_TCP_PROXY_BUFFER_BYTES),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    snapshot: SharedSnapshot,
    updates: SharedSnapshotSignal,
    mut config: watch::Receiver<std::sync::Arc<ReloadableRuntimeConfig>>,
    runtime_stats: SharedRuntimeStats,
    traffic: SharedTrafficStats,
    udp_sessions: SharedUdpSessionStats,
    overload: SharedOverloadStats,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut active = ListenerSet::default();
    let mut subscription = updates.subscribe();
    let mut current = config.borrow().clone();
    let pool = Arc::new(TcpConnectionPool::new(
        current.runtime.stream_upstream_pool_size,
        current.runtime.stream_upstream_pool_idle_timeout,
    ));
    pool::register_global_pool(pool.clone());
    let mut force_reload = true;

    loop {
        if *shutdown.borrow() {
            break;
        }

        if force_reload {
            current = config.borrow_and_update().clone();
        }

        let (desired, version) = {
            let current = snapshot.load();
            (build_listener_plan(&current), current.id.clone())
        };

        if active.needs_reload(desired.as_ref(), force_reload) {
            let runtime = &current.runtime;
            pool.drain();
            let result = active
                .replace(
                    desired,
                    snapshot.clone(),
                    current.access_log.clone(),
                    traffic.clone(),
                    udp_sessions.clone(),
                    TcpAdmissionController::new(runtime.tcp_admission.clone(), overload.clone()),
                    UdpAdmissionController::new(runtime.udp_admission.clone(), overload.clone()),
                    runtime.udp_response_idle_timeout,
                    runtime.effective_tcp_proxy_buffer_bytes(),
                    runtime.tcp_session_idle_timeout,
                    runtime.tcp_max_connection_age,
                    pool.clone(),
                    force_reload,
                )
                .await;
            if !result.retry_start || !result.failures.is_empty() {
                runtime_stats.observe_stream_listener_reload_result(
                    version.as_str(),
                    &result.started_listeners,
                    &result.retained_listeners,
                    &result.failures,
                );
            }

            prewarm_stream_backends(&pool, &snapshot);
        } else if desired.is_some() {
            let retained_listeners = active
                .active_plan()
                .into_values()
                .map(|listener| listener.name)
                .collect::<Vec<_>>();
            runtime_stats.observe_stream_listener_reload_result(
                version.as_str(),
                &[],
                &retained_listeners,
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

#[derive(Default)]
struct ListenerSet {
    tasks: BTreeMap<String, ListenerTask>,
}

struct ListenerTask {
    listener: PlannedListener,
    shutdown: watch::Sender<bool>,
    join: JoinHandle<()>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ListenerReplaceResult {
    retry_start: bool,
    first_error: Option<String>,
    first_failed_listener: Option<String>,
    failures: Vec<RuntimeListenerFailure>,
    started_listeners: Vec<String>,
    retained_listeners: Vec<String>,
}

impl ListenerSet {
    fn needs_reload(&self, desired: Option<&ListenerPlan>, force_reload: bool) -> bool {
        let updates = if force_reload {
            listener_updates_with_force_reload(
                &self.active_plan(),
                desired,
                &self.finished_tasks(),
                true,
            )
        } else {
            listener_updates(&self.active_plan(), desired, &self.finished_tasks())
        };
        !updates.start.is_empty() || !updates.stop.is_empty()
    }

    #[allow(clippy::too_many_arguments)]
    async fn replace(
        &mut self,
        plan: Option<ListenerPlan>,
        snapshot: SharedSnapshot,
        access_log: AccessLogOptions,
        traffic: SharedTrafficStats,
        udp_sessions: SharedUdpSessionStats,
        tcp_admission: TcpAdmissionController,
        udp_admission: UdpAdmissionController,
        udp_response_idle_timeout: Duration,
        tcp_proxy_buffer_bytes: usize,
        tcp_session_idle_timeout: Option<Duration>,
        tcp_max_connection_age: Option<Duration>,
        pool: Arc<TcpConnectionPool>,
        force_reload: bool,
    ) -> ListenerReplaceResult {
        let updates = if force_reload {
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
            return ListenerReplaceResult::default();
        }

        let stopped = updates.stop.len();
        let attempted_starts = updates.start.len();
        let mut started = 0usize;
        let mut failed_starts = 0usize;
        let mut first_error = None;
        let mut first_failed_listener = None;
        let mut failures = Vec::with_capacity(updates.start.len());
        let mut started_listeners = Vec::with_capacity(updates.start.len());

        for name in updates.stop {
            if let Some(task) = self.tasks.remove(&name) {
                stop_listener_task(name.as_str(), task).await;
            }
        }

        for listener in updates.start {
            let name = listener.name.clone();
            match spawn_listener_task(
                listener.clone(),
                snapshot.clone(),
                access_log.clone(),
                traffic.clone(),
                udp_sessions.clone(),
                tcp_admission.clone(),
                udp_admission.clone(),
                udp_response_idle_timeout,
                tcp_proxy_buffer_bytes,
                tcp_session_idle_timeout,
                tcp_max_connection_age,
                pool.clone(),
            )
            .await
            {
                Ok(task) => {
                    self.tasks.insert(name.clone(), task);
                    started_listeners.push(name);
                    started += 1;
                }
                Err(err) => {
                    failed_starts += 1;
                    warn!(
                        listener = %name,
                        error = %err,
                        "failed to start stream listener"
                    );
                    let error_message = err.to_string();
                    failures.push(RuntimeListenerFailure {
                        listener: name.clone(),
                        message: error_message.clone(),
                    });
                    if first_error.is_none() {
                        first_error = Some(error_message);
                        first_failed_listener = Some(name.clone());
                    }
                }
            }
        }

        info!(
            active = self.tasks.len(),
            started, stopped, attempted_starts, failed_starts, "stream listeners applied"
        );

        let retained_listeners = self
            .active_plan()
            .into_values()
            .filter(|listener| !started_listeners.contains(&listener.name))
            .map(|listener| listener.name)
            .collect();

        ListenerReplaceResult {
            retry_start: false,
            first_error,
            first_failed_listener,
            failures,
            retained_listeners,
            started_listeners,
        }
    }

    fn active_plan(&self) -> BTreeMap<String, PlannedListener> {
        self.tasks
            .iter()
            .map(|(name, task)| (name.clone(), task.listener.clone()))
            .collect()
    }

    fn finished_tasks(&self) -> BTreeSet<String> {
        self.tasks
            .iter()
            .filter(|(_, task)| task.join.is_finished())
            .map(|(name, _)| name.clone())
            .collect()
    }

    async fn shutdown_all(&mut self) {
        for (name, task) in std::mem::take(&mut self.tasks) {
            stop_listener_task(name.as_str(), task).await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn spawn_listener_task(
    listener: PlannedListener,
    snapshot: SharedSnapshot,
    access_log: AccessLogOptions,
    traffic: SharedTrafficStats,
    udp_sessions: SharedUdpSessionStats,
    tcp_admission: TcpAdmissionController,
    udp_admission: UdpAdmissionController,
    udp_response_idle_timeout: Duration,
    tcp_proxy_buffer_bytes: usize,
    tcp_session_idle_timeout: Option<Duration>,
    tcp_max_connection_age: Option<Duration>,
    pool: Arc<TcpConnectionPool>,
) -> Result<ListenerTask> {
    let protocol = listener.protocol;
    let task_listener = listener.clone();
    let (shutdown, receiver) = watch::channel(false);
    let log_name = task_listener.name.clone();
    let task_snapshot = snapshot.clone();
    let task_access_log = access_log.clone();
    let task_traffic = traffic.clone();
    let join = match protocol {
        StreamProtocol::Tcp => {
            let bind = task_listener.bind.clone();
            let server = tcp::bind(&bind).await?;
            let admission = tcp_admission.clone();
            tokio::spawn(async move {
                let result = tcp::run_with_listener(
                    task_snapshot,
                    task_listener.name,
                    bind,
                    server,
                    receiver,
                    false,
                    task_access_log,
                    task_traffic,
                    admission,
                    tcp_proxy_buffer_bytes,
                    tcp_session_idle_timeout,
                    tcp_max_connection_age,
                    pool,
                )
                .await;

                if let Err(err) = result {
                    error!(listener = %log_name, error = %err, "stream listener exited");
                }
            })
        }
        StreamProtocol::Udp => {
            let bind = task_listener.bind.clone();
            let socket = udp::bind(&bind).await?;
            let admission = udp_admission;
            tokio::spawn(async move {
                let result = udp::run_with_socket(
                    task_snapshot,
                    task_listener.name,
                    bind,
                    std::sync::Arc::new(socket),
                    receiver,
                    task_access_log,
                    task_traffic,
                    admission,
                    udp_sessions,
                    udp_response_idle_timeout,
                )
                .await;

                if let Err(err) = result {
                    error!(listener = %log_name, error = %err, "stream listener exited");
                }
            })
        }
    };

    Ok(ListenerTask {
        listener,
        shutdown,
        join,
    })
}

async fn stop_listener_task(name: &str, task: ListenerTask) {
    let _ = task.shutdown.send(true);
    if let Err(err) = task.join.await {
        error!(listener = %name, error = %err, "failed to join stream listener");
    }
}

/// Spawn concurrent best-effort prewarm tasks — one connection per unique
/// backend endpoint in the current snapshot.  Prewarm happens after every
/// listener reload so the idle pool is always seeded for the active backends.
fn prewarm_stream_backends(pool: &Arc<TcpConnectionPool>, snapshot: &SharedSnapshot) {
    let snap = snapshot.load();
    let mut seen: HashSet<(String, u16)> = HashSet::new();

    for backend in &snap.backends {
        for endpoint in &backend.endpoints {
            let key = (endpoint.address.clone(), endpoint.port as u16);
            if seen.insert(key.clone()) {
                let p = pool.clone();
                let addr = endpoint.address.clone();
                let port = endpoint.port as u16;
                tokio::spawn(async move {
                    p.prewarm(addr, port, 1).await;
                });
            }
        }
    }
}

pub(crate) fn socket_addr(address: &str, port: u32) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

pub(crate) fn ephemeral_bind_addr(target: &str) -> &'static str {
    if target.contains(':') {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    }
}
