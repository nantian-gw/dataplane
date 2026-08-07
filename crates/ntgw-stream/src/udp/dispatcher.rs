use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use crate::StreamError;
use tokio::{net::UdpSocket, sync::mpsc};
use tracing::warn;

use super::session::{UdpSessionRegistry, UdpSessionTask};

pub(crate) const UDP_DISPATCHER_WORKERS: usize = 16;
pub(crate) const UDP_DISPATCHER_QUEUE_CAPACITY: usize = 1024;

#[derive(Clone)]
pub(super) struct UdpDatagramDispatcher {
    workers: Arc<Vec<mpsc::Sender<UdpSessionTask>>>,
}

impl UdpDatagramDispatcher {
    pub(super) fn new(
        downstream: Arc<UdpSocket>,
        sessions: UdpSessionRegistry,
        udp_response_idle_timeout: Duration,
    ) -> Self {
        Self::with_capacity(
            downstream,
            sessions,
            udp_response_idle_timeout,
            UDP_DISPATCHER_WORKERS,
            UDP_DISPATCHER_QUEUE_CAPACITY,
        )
    }

    fn with_capacity(
        downstream: Arc<UdpSocket>,
        sessions: UdpSessionRegistry,
        udp_response_idle_timeout: Duration,
        worker_count: usize,
        queue_capacity: usize,
    ) -> Self {
        assert!(worker_count > 0, "UDP dispatcher needs at least one worker");
        assert!(
            queue_capacity > 0,
            "UDP dispatcher queue capacity must be positive"
        );

        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (tx, rx) = mpsc::channel(queue_capacity);
            let worker = UdpDatagramDispatchWorker {
                downstream: Arc::clone(&downstream),
                sessions: sessions.clone(),
                rx,
                udp_response_idle_timeout,
            };
            tokio::spawn(worker.run());
            workers.push(tx);
        }

        Self {
            workers: Arc::new(workers),
        }
    }

    pub(super) async fn dispatch(&self, task: UdpSessionTask) -> std::result::Result<(), StreamError> {
        let worker = self.worker_index_for_task(&task);
        self.workers[worker]
            .send(task)
            .await
            .map_err(|_| StreamError::Dispatch("udp dispatcher stopped".to_string()))
    }

    fn worker_index_for_task(&self, task: &UdpSessionTask) -> usize {
        udp_dispatcher_worker_index(
            task.selected.listener_name.as_str(),
            task.client_addr,
            task.upstream_addr,
            self.workers.len(),
        )
    }
}

pub(crate) fn udp_dispatcher_worker_index(
    listener_name: &str,
    client_addr: std::net::SocketAddr,
    upstream_addr: std::net::SocketAddr,
    worker_count: usize,
) -> usize {
    assert!(worker_count > 0, "UDP dispatcher needs at least one worker");
    let mut hasher = DefaultHasher::new();
    listener_name.hash(&mut hasher);
    client_addr.hash(&mut hasher);
    upstream_addr.hash(&mut hasher);
    (hasher.finish() as usize) % worker_count
}

struct UdpDatagramDispatchWorker {
    downstream: Arc<UdpSocket>,
    sessions: UdpSessionRegistry,
    rx: mpsc::Receiver<UdpSessionTask>,
    udp_response_idle_timeout: Duration,
}

impl UdpDatagramDispatchWorker {
    async fn run(mut self) {
        while let Some(task) = self.rx.recv().await {
            let listener = Arc::clone(&task.listener_name);
            let client = task.client_addr;
            if let Err(err) = self
                .sessions
                .dispatch(
                    Arc::clone(&self.downstream),
                    task,
                    self.udp_response_idle_timeout,
                )
                .await
            {
                warn!(
                    listener = %listener.as_ref(),
                    client = %client,
                    error = %err,
                    "stream udp datagram failed"
                );
            }
        }
    }
}
