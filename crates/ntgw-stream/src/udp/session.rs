use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use tokio::{
    net::UdpSocket,
    sync::{
        Mutex,
        mpsc::{self, error::TrySendError},
    },
    time::{sleep, timeout},
};
use tracing::{debug, warn};

use ntgw_ir::{SelectedBackend, SelectedBackendRuntimeIds};
use ntgw_observability::{
    AccessLogOptions, SharedTrafficStats, SharedUdpSessionStats, UdpAdmissionPermit,
    UdpSessionStats,
};

use crate::{access_log::StreamAccessLogState, ephemeral_bind_addr};

use super::record_udp_datagram;

pub(crate) const UDP_SESSION_SHARDS: usize = 16;

type UdpSessionSender = mpsc::Sender<QueuedUdpSessionTask>;
type UdpSessionShard = Mutex<HashMap<UdpSessionKey, UdpSessionSender>>;
type UdpSessionShards = Arc<Vec<UdpSessionShard>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UdpSessionKey {
    pub(crate) listener_name: Arc<str>,
    pub(crate) client_addr: SocketAddr,
    pub(crate) upstream_addr: SocketAddr,
}

#[derive(Clone)]
pub(crate) struct UdpSessionRegistry {
    sessions: UdpSessionShards,
    stats: SharedUdpSessionStats,
}

pub(super) struct UdpSessionTask {
    pub(super) listener_name: Arc<str>,
    pub(super) selected: SelectedBackend,
    pub(super) runtime_ids: SelectedBackendRuntimeIds,
    pub(super) upstream_addr: SocketAddr,
    pub(super) client_addr: SocketAddr,
    pub(super) payload: Vec<u8>,
    pub(super) access_log: Option<AccessLogOptions>,
    pub(super) access_log_state: Option<StreamAccessLogState>,
    pub(super) traffic: SharedTrafficStats,
    pub(super) started_at: Instant,
    pub(super) _permit: UdpAdmissionPermit,
}

impl UdpSessionTask {
    pub(super) fn session_key(&self) -> UdpSessionKey {
        UdpSessionKey {
            listener_name: Arc::clone(&self.listener_name),
            client_addr: self.client_addr,
            upstream_addr: self.upstream_addr,
        }
    }
}

pub(crate) struct QueuedUdpSessionTask {
    task: UdpSessionTask,
    _queue_depth: UdpQueueDepthGuard,
}

struct ActiveUdpExchange {
    task: UdpSessionTask,
    total_response_bytes: usize,
}

impl ActiveUdpExchange {
    fn new(task: UdpSessionTask) -> Self {
        Self {
            task,
            total_response_bytes: 0,
        }
    }

    fn client_addr(&self) -> SocketAddr {
        self.task.client_addr
    }

    fn record(self) {
        record_udp_datagram(
            &self.task.traffic,
            self.task.access_log.as_ref(),
            self.task.access_log_state.as_ref(),
            &self.task.selected,
            self.task.runtime_ids,
            self.task.client_addr,
            self.task.payload.len(),
            self.total_response_bytes,
            self.task.started_at,
        );
    }
}

impl QueuedUdpSessionTask {
    fn new(task: UdpSessionTask, stats: SharedUdpSessionStats) -> Self {
        let listener = Arc::clone(&task.listener_name);
        stats.observe_queue_enqueued(listener.as_ref());
        Self {
            task,
            _queue_depth: UdpQueueDepthGuard { stats, listener },
        }
    }

    fn listener_name(&self) -> &str {
        self.task.selected.listener_name.as_str()
    }

    fn into_task(self) -> UdpSessionTask {
        let Self { task, _queue_depth } = self;
        task
    }
}

struct UdpQueueDepthGuard {
    stats: SharedUdpSessionStats,
    listener: Arc<str>,
}

impl Drop for UdpQueueDepthGuard {
    fn drop(&mut self) {
        self.stats.observe_queue_dequeued(self.listener.as_ref());
    }
}

impl UdpSessionRegistry {
    pub(crate) fn with_stats(stats: SharedUdpSessionStats) -> Self {
        let mut sessions = Vec::with_capacity(UDP_SESSION_SHARDS);
        for _ in 0..UDP_SESSION_SHARDS {
            sessions.push(Mutex::new(HashMap::new()));
        }
        Self {
            sessions: Arc::new(sessions),
            stats,
        }
    }

    #[cfg(test)]
    pub(super) fn shard_count(&self) -> usize {
        self.sessions.len()
    }

    pub(super) async fn dispatch(
        &self,
        downstream: Arc<UdpSocket>,
        mut task: UdpSessionTask,
        udp_response_idle_timeout: Duration,
    ) -> Result<()> {
        let key = task.session_key();
        loop {
            let sender = self
                .ensure_sender(
                    key.clone(),
                    task.selected.backend.address.as_str(),
                    downstream.clone(),
                    udp_response_idle_timeout,
                )
                .await?;
            let queued = QueuedUdpSessionTask::new(task, self.stats.clone());
            match sender.try_send(queued) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(queued)) => {
                    self.stats
                        .observe_queue_overflow_drop(queued.listener_name());
                    return Ok(());
                }
                Err(TrySendError::Closed(queued)) => {
                    self.remove(&key).await;
                    task = queued.into_task();
                }
            }
        }
    }

    async fn ensure_sender(
        &self,
        key: UdpSessionKey,
        upstream_host: &str,
        downstream: Arc<UdpSocket>,
        udp_response_idle_timeout: Duration,
    ) -> Result<mpsc::Sender<QueuedUdpSessionTask>> {
        let upstream_addr = key.upstream_addr;
        self.ensure_sender_with_factory(key, downstream, udp_response_idle_timeout, || async move {
            let upstream = UdpSocket::bind(ephemeral_bind_addr(upstream_host)).await?;
            upstream.connect(upstream_addr).await?;
            Ok::<UdpSocket, anyhow::Error>(upstream)
        })
        .await
    }

    pub(crate) async fn ensure_sender_with_factory<F, Fut>(
        &self,
        key: UdpSessionKey,
        downstream: Arc<UdpSocket>,
        udp_response_idle_timeout: Duration,
        create_upstream: F,
    ) -> Result<mpsc::Sender<QueuedUdpSessionTask>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<UdpSocket>>,
    {
        let shard = self.shard_for(&key);
        {
            let sessions = shard.lock().await;
            if let Some(existing) = sessions.get(&key) {
                return Ok(existing.clone());
            }
        }

        let (tx, rx) = mpsc::channel(32);
        {
            let mut sessions = shard.lock().await;
            if let Some(existing) = sessions.get(&key) {
                return Ok(existing.clone());
            }
            sessions.insert(key.clone(), tx.clone());
        }
        self.stats
            .observe_session_started(key.listener_name.as_ref());

        let upstream = match create_upstream().await {
            Ok(socket) => socket,
            Err(err) => {
                self.remove(&key).await;
                return Err(err);
            }
        };

        let registry = self.clone();
        tokio::spawn(async move {
            run_udp_session(
                registry,
                key,
                downstream,
                upstream,
                rx,
                udp_response_idle_timeout,
            )
            .await;
        });

        Ok(tx)
    }

    async fn remove(&self, key: &UdpSessionKey) {
        if self.shard_for(key).lock().await.remove(key).is_some() {
            self.stats.observe_session_ended(key.listener_name.as_ref());
        }
    }

    fn shard_for(
        &self,
        key: &UdpSessionKey,
    ) -> &Mutex<HashMap<UdpSessionKey, mpsc::Sender<QueuedUdpSessionTask>>> {
        &self.sessions[self.shard_index(key)]
    }

    fn shard_index(&self, key: &UdpSessionKey) -> usize {
        udp_session_shard_index(key, self.sessions.len())
    }
}

pub(crate) fn udp_session_shard_index(key: &UdpSessionKey, shard_count: usize) -> usize {
    assert!(
        shard_count > 0,
        "UDP session registry needs at least one shard"
    );
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % shard_count
}

async fn run_udp_session(
    registry: UdpSessionRegistry,
    key: UdpSessionKey,
    downstream: Arc<UdpSocket>,
    upstream: UdpSocket,
    mut rx: mpsc::Receiver<QueuedUdpSessionTask>,
    udp_response_idle_timeout: Duration,
) {
    let session_idle_timeout = udp_session_idle_timeout(udp_response_idle_timeout);
    let mut response_buffer = vec![0; 65_535];
    let mut current: Option<ActiveUdpExchange> = None;
    loop {
        if current.is_none() {
            let task = match timeout(session_idle_timeout, rx.recv()).await {
                Ok(Some(task)) => task.into_task(),
                Ok(None) => break,
                Err(_) => {
                    registry
                        .stats
                        .observe_idle_eviction(key.listener_name.as_ref());
                    break;
                }
            };
            match send_session_datagram(&upstream, task).await {
                Ok(exchange) => current = Some(exchange),
                Err(err) => {
                    warn!(
                        listener = %key.listener_name,
                        client = %key.client_addr,
                        backend = %key.upstream_addr,
                        error = %err,
                        "stream udp session failed"
                    );
                    break;
                }
            }
            continue;
        }

        tokio::select! {
            queued = rx.recv() => {
                let Some(queued) = queued else {
                    break;
                };
                if let Some(exchange) = current.take() {
                    exchange.record();
                }
                match send_session_datagram(&upstream, queued.into_task()).await {
                    Ok(exchange) => current = Some(exchange),
                    Err(err) => {
                        warn!(
                            listener = %key.listener_name,
                            client = %key.client_addr,
                            backend = %key.upstream_addr,
                            error = %err,
                            "stream udp session failed"
                        );
                        break;
                    }
                }
            }
            recv = upstream.recv(&mut response_buffer) => {
                match recv {
                    Ok(size) => {
                        if let Some(exchange) = current.as_mut() {
                            exchange.total_response_bytes += size;
                            if let Err(err) = downstream.send_to(&response_buffer[..size], exchange.client_addr()).await {
                                warn!(
                                    listener = %key.listener_name,
                                    client = %key.client_addr,
                                    backend = %key.upstream_addr,
                                    error = %err,
                                    "stream udp session failed"
                                );
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        warn!(
                            listener = %key.listener_name,
                            client = %key.client_addr,
                            backend = %key.upstream_addr,
                            error = %err,
                            "stream udp session failed"
                        );
                        break;
                    }
                }
            }
            _ = sleep(udp_response_idle_timeout) => {
                if let Some(exchange) = current.take() {
                    exchange.record();
                }
            }
        }
    }

    if let Some(exchange) = current.take() {
        exchange.record();
    }
    registry.remove(&key).await;
}

async fn send_session_datagram(
    upstream: &UdpSocket,
    task: UdpSessionTask,
) -> Result<ActiveUdpExchange> {
    upstream.send(&task.payload).await?;

    debug!(
        listener = %task.selected.listener_name,
        route = %task.selected.route_name,
        backend = %task.upstream_addr,
        "stream udp backend selected"
    );

    Ok(ActiveUdpExchange::new(task))
}

fn udp_session_idle_timeout(udp_response_idle_timeout: Duration) -> Duration {
    std::cmp::max(udp_response_idle_timeout, Duration::from_secs(1))
}

impl Default for UdpSessionRegistry {
    fn default() -> Self {
        Self::with_stats(UdpSessionStats::shared())
    }
}
