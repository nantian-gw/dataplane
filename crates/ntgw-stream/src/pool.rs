use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::net::TcpStream;
use tracing::{debug, warn};

use crate::traffic::StreamPoolCounters;

const MAX_CONNECTION_COUNT: usize = 32_768;

type PoolKey = (String, u16);

/// Snapshot of cumulative connection pool counters at a point in time.
#[derive(Debug, Clone, Copy, Default)]
pub struct PoolCountersSnapshot {
    /// Connections currently held by callers (not in idle pool).
    pub active_connections: u64,
    /// Connections currently sitting idle in the pool.
    pub idle_connections: u64,
    /// Cumulative number of successful pool reuses.
    pub connection_hits: u64,
    /// Cumulative number of new connections created (pool misses).
    pub connection_misses: u64,
    /// Peak active connections observed since the pool was created.
    pub peak_active_connections: u64,
}

pub(crate) struct TcpConnectionPool {
    idle: Arc<DashMap<PoolKey, Vec<IdleConnection>>>,
    pub(super) max_idle_per_backend: AtomicUsize,
    idle_timeout: Duration,
    /// Cumulative pool-level counters (best-effort, relaxed ordering).
    active_connections: AtomicU64,
    idle_connections: AtomicU64,
    connection_hits: AtomicU64,
    connection_misses: AtomicU64,
    peak_active_connections: AtomicU64,
}

struct IdleConnection {
    stream: TcpStream,
    since: Instant,
}

impl TcpConnectionPool {
    pub(crate) fn new(max_idle_per_backend: usize, idle_timeout: Duration) -> Self {
        let max_idle_per_backend = max_idle_per_backend.clamp(0, MAX_CONNECTION_COUNT);
        Self {
            idle: Arc::new(DashMap::new()),
            max_idle_per_backend: AtomicUsize::new(max_idle_per_backend),
            idle_timeout,
            active_connections: AtomicU64::new(0),
            idle_connections: AtomicU64::new(0),
            connection_hits: AtomicU64::new(0),
            connection_misses: AtomicU64::new(0),
            peak_active_connections: AtomicU64::new(0),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.max_idle_per_backend.load(Ordering::Relaxed) > 0
    }

    pub(crate) fn set_max_idle_per_backend(&self, size: usize) {
        let clamped = size.clamp(0, MAX_CONNECTION_COUNT);
        let old = self.max_idle_per_backend.swap(clamped, Ordering::Relaxed);
        if old != clamped {
            debug!(old, new = clamped, "pool max_idle_per_backend updated");
        }
    }

    pub(crate) async fn get_connection(
        &self,
        addr: String,
        port: u16,
    ) -> (std::io::Result<TcpStream>, StreamPoolCounters) {
        if !self.is_enabled() {
            let conn = TcpStream::connect((addr.as_str(), port)).await;
            return (conn, StreamPoolCounters::default());
        }

        let key = (addr, port);

        loop {
            // Hold the shard lock only for the O(1) pop, then release it before the
            // try_read liveness probe so concurrent get/return on this backend do not
            // serialize behind a syscall.
            let idle = {
                let Some(mut entry) = self.idle.get_mut(&key) else {
                    break;
                };
                match entry.pop() {
                    Some(idle) => {
                        self.idle_connections.fetch_sub(1, Ordering::Relaxed);
                        idle
                    }
                    None => break,
                }
            };

            if idle.since.elapsed() >= self.idle_timeout {
                debug!(backend = ?key, "pool eviction: idle timeout");
                continue;
            }

            let mut buf = [0u8; 1];
            match idle.stream.try_read(&mut buf) {
                Ok(0) => {
                    debug!(backend = ?key, "pool eviction: stream closed");
                    continue;
                }
                Ok(_) => {
                    debug!(backend = ?key, "pool eviction: unexpected data pending");
                    continue;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    debug!(backend = ?key, "pool hit");
                    self.connection_hits.fetch_add(1, Ordering::Relaxed);
                    self.active_connections.fetch_add(1, Ordering::Relaxed);
                    return (Ok(idle.stream), StreamPoolCounters { hits: 1, misses: 0 });
                }
                Err(_) => {
                    debug!(backend = ?key, "pool eviction: error");
                    continue;
                }
            }
        }

        debug!(backend = ?key, "pool miss");
        let idle_now = self.idle_connections.load(Ordering::Relaxed);
        if idle_now == 0 {
            debug!(backend = ?key, "pool exhausted: all idle connections used");
        }
        self.connection_misses.fetch_add(1, Ordering::Relaxed);
        let conn = TcpStream::connect((key.0.as_str(), port)).await;
        if conn.is_ok() {
            self.active_connections.fetch_add(1, Ordering::Relaxed);
            self.peak_active_connections.fetch_max(
                self.active_connections.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
        }
        (conn, StreamPoolCounters { hits: 0, misses: 1 })
    }

    pub(crate) fn return_connection(&self, addr: String, port: u16, stream: TcpStream) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        if !self.is_enabled() {
            return;
        }
        let backend_addr = addr.clone();
        let key = (addr, port);
        let mut entry = self.idle.entry(key).or_default();

        if entry.len() >= self.max_idle_per_backend.load(Ordering::Relaxed) {
            debug!(
                backend.addr = backend_addr,
                backend.port = port,
                "pool full, dropping returned connection"
            );
            return;
        }

        self.idle_connections.fetch_add(1, Ordering::Relaxed);
        entry.push(IdleConnection {
            stream,
            since: Instant::now(),
        });
        debug!(
            backend.addr = backend_addr,
            backend.port = port,
            count = entry.len(),
            "pool return"
        );
    }

    /// Best-effort pre-establish idle connections to a backend so the first
    /// request does not pay TCP handshake latency.
    ///
    /// Respects `max_idle_per_backend` — never exceeds it.  Failures are
    /// logged at warn level and do not propagate.
    pub(crate) async fn prewarm(&self, addr: String, port: u16, count: usize) {
        if !self.is_enabled() {
            return;
        }

        let key = (addr.clone(), port);

        let available = {
            let entry = self.idle.entry(key.clone()).or_default();
            self.max_idle_per_backend
                .load(Ordering::Relaxed)
                .saturating_sub(entry.len())
        };

        let effective = count.min(available);
        if effective == 0 {
            return;
        }

        let mut warmed = 0usize;
        for _ in 0..effective {
            match TcpStream::connect((addr.as_str(), port)).await {
                Ok(stream) => {
                    let mut entry = self.idle.entry(key.clone()).or_default();
                    entry.push(IdleConnection {
                        stream,
                        since: Instant::now(),
                    });
                    warmed += 1;
                }
                Err(err) => {
                    warn!(
                        backend.addr = %addr,
                        backend.port = port,
                        error = %err,
                        "prewarm: connection failed"
                    );
                }
            }
        }

        if warmed > 0 {
            self.idle_connections
                .fetch_add(warmed as u64, Ordering::Relaxed);
            debug!(
                backend.addr = %addr,
                backend.port = port,
                count = warmed,
                "prewarmed {warmed} connections to {addr}:{port}"
            );
        }
    }

    pub(crate) fn drain(&self) {
        let count = self.idle.len();
        self.idle.clear();
        self.idle_connections.store(0, Ordering::Relaxed);
        debug!(count, "pool drained");
    }


    pub(crate) fn counter_snapshot(&self) -> PoolCountersSnapshot {
        PoolCountersSnapshot {
            active_connections: self.active_connections.load(Ordering::Relaxed),
            idle_connections: self.idle_connections.load(Ordering::Relaxed),
            connection_hits: self.connection_hits.load(Ordering::Relaxed),
            connection_misses: self.connection_misses.load(Ordering::Relaxed),
            peak_active_connections: self.peak_active_connections.load(Ordering::Relaxed),
        }
    }

    #[cfg(test)]
    fn idle_count(&self, addr: &str, port: u16) -> usize {
        let key = (addr.to_string(), port);
        self.idle.get(&key).map_or(0, |e| e.len())
    }
}

static GLOBAL_POOL: OnceLock<Arc<TcpConnectionPool>> = OnceLock::new();

pub(crate) fn register_global_pool(pool: Arc<TcpConnectionPool>) {
    let _ = GLOBAL_POOL.set(pool);
}

#[must_use]
pub fn global_pool_snapshot() -> Option<PoolCountersSnapshot> {
    GLOBAL_POOL.get().map(|pool| pool.counter_snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn echo_server() -> (String, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ip = addr.ip().to_string();
        let port = addr.port();
        tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = [0; 1024];
                    let _ = socket.try_read(&mut buf);
                });
            }
        });
        (ip, port)
    }

    #[tokio::test]
    async fn pool_returns_new_connection_when_empty() {
        let (ip, port) = echo_server().await;
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

        let (result, counters) = pool.get_connection(ip.clone(), port).await;
        assert!(result.is_ok(), "should connect to server");
        assert_eq!(counters.hits, 0);
        assert_eq!(counters.misses, 1);
    }

    #[tokio::test]
    async fn pool_reuses_returned_connection() {
        let (ip, port) = echo_server().await;
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

        let (conn1, c1) = pool.get_connection(ip.clone(), port).await;
        assert!(conn1.is_ok());
        assert_eq!(c1.misses, 1);

        pool.return_connection(ip.clone(), port, conn1.unwrap());
        assert_eq!(pool.idle_count(&ip, port), 1);

        let (conn2, c2) = pool.get_connection(ip.clone(), port).await;
        assert!(conn2.is_ok());
        assert_eq!(c2.hits, 1);
        assert_eq!(c2.misses, 0);
    }

    #[test]
    fn cumulative_counters_track_hits_misses_and_active_idle() {
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

        let snap = pool.counter_snapshot();
        assert_eq!(snap.active_connections, 0);
        assert_eq!(snap.idle_connections, 0);
        assert_eq!(snap.connection_hits, 0);
        assert_eq!(snap.connection_misses, 0);
    }

    #[tokio::test]
    async fn counters_track_miss_and_active() {
        let (ip, port) = echo_server().await;
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

        let (result, _) = pool.get_connection(ip.clone(), port).await;
        assert!(result.is_ok());

        let snap = pool.counter_snapshot();
        assert_eq!(snap.connection_misses, 1);
        assert_eq!(snap.connection_hits, 0);
        assert_eq!(snap.active_connections, 1);
        assert_eq!(snap.idle_connections, 0);
    }

    #[tokio::test]
    async fn counters_track_hit_and_active_idle_transitions() {
        let (ip, port) = echo_server().await;
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

        // First connection: miss, becomes active
        let (conn1, _) = pool.get_connection(ip.clone(), port).await;
        assert!(conn1.is_ok());

        let snap = pool.counter_snapshot();
        assert_eq!(snap.connection_misses, 1);
        assert_eq!(snap.active_connections, 1);

        // Return to pool: active→idle
        pool.return_connection(ip.clone(), port, conn1.unwrap());

        let snap = pool.counter_snapshot();
        assert_eq!(snap.active_connections, 0);
        assert_eq!(snap.idle_connections, 1);
        assert_eq!(snap.connection_hits, 0);

        // Second connection: hit, idle→active
        let (conn2, c2) = pool.get_connection(ip.clone(), port).await;
        assert!(conn2.is_ok());
        assert_eq!(c2.hits, 1);

        let snap = pool.counter_snapshot();
        assert_eq!(snap.connection_hits, 1);
        assert_eq!(snap.connection_misses, 1);
        assert_eq!(snap.active_connections, 1);
        assert_eq!(snap.idle_connections, 0);

        drop(conn2);
    }

    #[test]
    fn drain_resets_idle_counters() {
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));
        pool.drain();

        let snap = pool.counter_snapshot();
        assert_eq!(snap.idle_connections, 0);
        assert_eq!(snap.active_connections, 0);
    }

    #[tokio::test]
    async fn counter_snapshot_is_clone_send() {
        let (ip, port) = echo_server().await;
        let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

        let (result, _) = pool.get_connection(ip.clone(), port).await;
        assert!(result.is_ok());

        let snap = pool.counter_snapshot();
        let snap2 = snap;
        assert_eq!(snap2.connection_misses, snap.connection_misses);

        let handle = tokio::spawn(async move {
            let _ = snap;
        });
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn dynamic_resize_respects_new_limit() {
        let (ip, port) = echo_server().await;
        let pool = TcpConnectionPool::new(5, Duration::from_secs(30));

        let (conn1, _) = pool.get_connection(ip.clone(), port).await;
        assert!(conn1.is_ok());

        pool.return_connection(ip.clone(), port, conn1.unwrap());
        assert_eq!(
            pool.idle_count(&ip, port),
            1,
            "pool should have 1 idle after first return"
        );

        pool.set_max_idle_per_backend(0);
        assert!(!pool.is_enabled(), "pool should be disabled when size is 0");

        let (conn2, _) = pool.get_connection(ip.clone(), port).await;
        assert!(conn2.is_ok());

        pool.return_connection(ip.clone(), port, conn2.unwrap());
        assert_eq!(
            pool.idle_count(&ip, port),
            1,
            "disabled pool drops returned connections, idle stays at 1"
        );

        pool.set_max_idle_per_backend(1);
        assert!(
            pool.is_enabled(),
            "pool should be enabled after increasing size"
        );

        let (conn3, _) = pool.get_connection(ip.clone(), port).await;
        assert!(conn3.is_ok());
        pool.return_connection(ip.clone(), port, conn3.unwrap());
        assert_eq!(
            pool.idle_count(&ip, port),
            1,
            "pool respects new limit of 1"
        );
    }
}
