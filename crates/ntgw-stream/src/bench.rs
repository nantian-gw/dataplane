use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::pool::TcpConnectionPool;
use crate::{
    DEFAULT_TCP_PROXY_BUFFER_BYTES, MAX_TCP_PROXY_BUFFER_BYTES, MIN_TCP_PROXY_BUFFER_BYTES,
    normalize_tcp_proxy_buffer_bytes,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StreamBenchConfig {
    pub tcp_buffer_samples: [usize; 7],
    pub udp_clients: usize,
    pub udp_payload_bytes: usize,
    pub udp_dispatcher_workers: usize,
    pub udp_session_shards: usize,
}

impl Default for StreamBenchConfig {
    fn default() -> Self {
        Self {
            tcp_buffer_samples: [
                0,
                MIN_TCP_PROXY_BUFFER_BYTES / 2,
                MIN_TCP_PROXY_BUFFER_BYTES,
                DEFAULT_TCP_PROXY_BUFFER_BYTES,
                64 * 1024,
                MAX_TCP_PROXY_BUFFER_BYTES,
                MAX_TCP_PROXY_BUFFER_BYTES * 2,
            ],
            udp_clients: 4096,
            udp_payload_bytes: 1200,
            udp_dispatcher_workers: crate::udp::UDP_DISPATCHER_WORKERS,
            udp_session_shards: crate::udp::UDP_SESSION_SHARDS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTcpBufferMatrixRow {
    pub requested_bytes: usize,
    pub effective_bytes: usize,
    pub used_default: bool,
    pub clamped_to_min: bool,
    pub clamped_to_max: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTcpBufferMatrixStep {
    pub default_bytes: usize,
    pub min_bytes: usize,
    pub max_bytes: usize,
    pub row_count: usize,
    pub rows: Vec<StreamTcpBufferMatrixRow>,
}

#[derive(Debug, Clone)]
pub struct StreamTcpBufferMatrixFixture {
    config: StreamBenchConfig,
}

impl StreamTcpBufferMatrixFixture {
    pub fn build(config: StreamBenchConfig) -> Self {
        Self { config }
    }

    pub fn evaluate_once(&self) -> StreamTcpBufferMatrixStep {
        let rows = self
            .config
            .tcp_buffer_samples
            .iter()
            .copied()
            .map(|requested_bytes| {
                let effective_bytes = normalize_tcp_proxy_buffer_bytes(requested_bytes);
                StreamTcpBufferMatrixRow {
                    requested_bytes,
                    effective_bytes,
                    used_default: requested_bytes == 0,
                    clamped_to_min: requested_bytes > 0
                        && requested_bytes < MIN_TCP_PROXY_BUFFER_BYTES,
                    clamped_to_max: requested_bytes > MAX_TCP_PROXY_BUFFER_BYTES,
                }
            })
            .collect::<Vec<_>>();

        StreamTcpBufferMatrixStep {
            default_bytes: DEFAULT_TCP_PROXY_BUFFER_BYTES,
            min_bytes: MIN_TCP_PROXY_BUFFER_BYTES,
            max_bytes: MAX_TCP_PROXY_BUFFER_BYTES,
            row_count: rows.len(),
            rows,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUdpDistributionStep {
    pub clients: usize,
    pub dispatcher_workers: usize,
    pub dispatcher_queue_capacity: usize,
    pub session_shards: usize,
    pub non_empty_dispatcher_workers: usize,
    pub non_empty_session_shards: usize,
    pub min_dispatcher_load: u64,
    pub max_dispatcher_load: u64,
    pub min_session_shard_load: u64,
    pub max_session_shard_load: u64,
}

#[derive(Debug, Clone)]
pub struct StreamUdpDistributionFixture {
    config: StreamBenchConfig,
}

impl StreamUdpDistributionFixture {
    pub fn build(config: StreamBenchConfig) -> Self {
        Self { config }
    }

    pub fn evaluate_once(&self) -> StreamUdpDistributionStep {
        let clients = self.config.udp_clients.max(1);
        let dispatcher_workers = self.config.udp_dispatcher_workers.max(1);
        let session_shards = self.config.udp_session_shards.max(1);
        let mut dispatcher_loads = vec![0u64; dispatcher_workers];
        let mut session_loads = vec![0u64; session_shards];
        let listener_name = Arc::<str>::from("default/gw/udp");
        let upstream_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)), 8080);

        for client_index in 0..clients {
            let client_addr = client_addr(client_index);
            let worker = crate::udp::udp_dispatcher_worker_index(
                listener_name.as_ref(),
                client_addr,
                upstream_addr,
                dispatcher_workers,
            );
            dispatcher_loads[worker] = dispatcher_loads[worker].saturating_add(1);
            let key = crate::udp::UdpSessionKey {
                listener_name: Arc::clone(&listener_name),
                client_addr,
                upstream_addr,
            };
            let shard = crate::udp::udp_session_shard_index(&key, session_shards);
            session_loads[shard] = session_loads[shard].saturating_add(1);
        }

        let (min_dispatcher_load, max_dispatcher_load, non_empty_dispatcher_workers) =
            load_summary(&dispatcher_loads);
        let (min_session_shard_load, max_session_shard_load, non_empty_session_shards) =
            load_summary(&session_loads);

        StreamUdpDistributionStep {
            clients,
            dispatcher_workers,
            dispatcher_queue_capacity: crate::udp::UDP_DISPATCHER_QUEUE_CAPACITY,
            session_shards,
            non_empty_dispatcher_workers,
            non_empty_session_shards,
            min_dispatcher_load,
            max_dispatcher_load,
            min_session_shard_load,
            max_session_shard_load,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUdpPayloadCopyStep {
    pub payload_bytes: usize,
    pub copied_bytes: usize,
    pub checksum: u64,
}

#[derive(Debug, Clone)]
pub struct StreamUdpPayloadCopyFixture {
    payload: Vec<u8>,
}

impl StreamUdpPayloadCopyFixture {
    pub fn build(config: StreamBenchConfig) -> Self {
        let payload_bytes = config.udp_payload_bytes.max(1);
        let payload = (0..payload_bytes)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        Self { payload }
    }

    pub fn copy_once(&self) -> StreamUdpPayloadCopyStep {
        let copied = self.payload.as_slice().to_vec();
        let checksum = copied.first().copied().unwrap_or_default() as u64
            + copied.last().copied().unwrap_or_default() as u64
            + copied.len() as u64;

        StreamUdpPayloadCopyStep {
            payload_bytes: self.payload.len(),
            copied_bytes: copied.len(),
            checksum,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TcpPoolContentionBenchConfig {
    pub threads: usize,
    pub prewarm_idle: usize,
}

impl Default for TcpPoolContentionBenchConfig {
    fn default() -> Self {
        Self {
            threads: 8,
            prewarm_idle: 16,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpPoolContentionStep {
    pub threads: usize,
    pub backend_keys: usize,
    pub prewarm_idle: usize,
    pub total_ops: u64,
    pub hits: u64,
    pub misses: u64,
    pub elapsed_ms: f64,
    pub throughput_ops_per_sec: f64,
}

struct WorkerStat {
    hits: u64,
    misses: u64,
    latencies: Vec<f64>,
}

pub struct TcpPoolContentionFixture {
    config: TcpPoolContentionBenchConfig,
    pool: Arc<TcpConnectionPool>,
    backends: Vec<(String, u16)>,
    backend_keys: usize,
    // Holds the echo-server accept loops alive for the fixture's lifetime; if this
    // runtime is dropped the peer sockets close and prewarmed idle conns stop hitting.
    // Wrapped in Option so Drop can shut it down without blocking: the bench harness
    // drops the fixture inside an async context, where a blocking runtime drop panics.
    server_runtime: Option<tokio::runtime::Runtime>,
}

impl Drop for TcpPoolContentionFixture {
    fn drop(&mut self) {
        if let Some(runtime) = self.server_runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl TcpPoolContentionFixture {
    pub fn build(config: TcpPoolContentionBenchConfig, backend_keys: usize) -> io::Result<Self> {
        // block_on must not run on an ambient runtime thread (the bench harness drives
        // scenarios inside a current_thread runtime), so do setup on a dedicated thread.
        std::thread::spawn(move || Self::build_inner(config, backend_keys))
            .join()
            .map_err(|_| io::Error::other("pool contention fixture build panicked"))?
    }

    fn build_inner(config: TcpPoolContentionBenchConfig, backend_keys: usize) -> io::Result<Self> {
        let backend_keys = backend_keys.max(1);
        let prewarm = config.prewarm_idle.max(1);
        let threads = config.threads.max(1);
        let max_idle = (prewarm + threads).clamp(1, 32_768);

        let server_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;

        let pool = Arc::new(TcpConnectionPool::new(max_idle, Duration::from_secs(300)));

        let backends: Vec<(String, u16)> = server_runtime.block_on(async {
            let mut backends = Vec::with_capacity(backend_keys);
            for _ in 0..backend_keys {
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
                let addr = listener.local_addr()?;
                tokio::spawn(echo_accept_loop(listener));
                backends.push((addr.ip().to_string(), addr.port()));
            }
            io::Result::Ok(backends)
        })?;

        // Prewarm each backend with `prewarm` simultaneously-idle connections so the
        // timed loop exercises the reuse (hit) path, not TcpStream::connect.
        server_runtime.block_on(async {
            for (addr, port) in &backends {
                let mut conns = Vec::with_capacity(prewarm);
                for _ in 0..prewarm {
                    let (conn, _) = pool.get_connection(addr, *port).await;
                    conns.push(conn?);
                }
                for conn in conns {
                    pool.return_connection(addr, *port, conn);
                }
            }
            io::Result::Ok(())
        })?;

        Ok(Self {
            config,
            pool,
            backends,
            backend_keys,
            server_runtime: Some(server_runtime),
        })
    }

    pub fn run(&self, ops_per_thread: usize) -> io::Result<(TcpPoolContentionStep, Vec<f64>)> {
        let ops_per_thread = ops_per_thread.max(1);
        let threads = self.config.threads.max(1);
        let start = Instant::now();
        let mut handles = Vec::with_capacity(threads);
        for thread_index in 0..threads {
            let pool = Arc::clone(&self.pool);
            let (addr, port) = self.backends[thread_index % self.backends.len()].clone();
            handles.push(std::thread::spawn(move || -> io::Result<WorkerStat> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async move {
                    let mut hits = 0u64;
                    let mut misses = 0u64;
                    let mut latencies = Vec::with_capacity(ops_per_thread);
                    for _ in 0..ops_per_thread {
                        let op_start = Instant::now();
                        let (conn, counters) = pool.get_connection(&addr, port).await;
                        let conn = conn?;
                        pool.return_connection(&addr, port, conn);
                        latencies.push(op_start.elapsed().as_secs_f64() * 1000.0);
                        hits += u64::from(counters.hits);
                        misses += u64::from(counters.misses);
                    }
                    io::Result::Ok(WorkerStat {
                        hits,
                        misses,
                        latencies,
                    })
                })
            }));
        }

        let mut hits = 0u64;
        let mut misses = 0u64;
        let mut latencies = Vec::with_capacity(threads.saturating_mul(ops_per_thread));
        for handle in handles {
            let stat = handle
                .join()
                .map_err(|_| io::Error::other("pool contention worker panicked"))??;
            hits += stat.hits;
            misses += stat.misses;
            latencies.extend(stat.latencies);
        }

        let elapsed = start.elapsed();
        let total_ops = hits + misses;
        let throughput = if elapsed.as_secs_f64() > 0.0 {
            total_ops as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let step = TcpPoolContentionStep {
            threads,
            backend_keys: self.backend_keys,
            prewarm_idle: self.config.prewarm_idle.max(1),
            total_ops,
            hits,
            misses,
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
            throughput_ops_per_sec: throughput,
        };
        Ok((step, latencies))
    }
}

async fn echo_accept_loop(listener: TcpListener) {
    loop {
        let Ok((socket, _)) = listener.accept().await else {
            break;
        };
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            loop {
                if socket.readable().await.is_err() {
                    break;
                }
                match socket.try_read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                    Err(_) => break,
                }
            }
        });
    }
}

fn client_addr(client_index: usize) -> SocketAddr {
    let octet_2 = ((client_index / 65_536) % 256) as u8;
    let octet_3 = ((client_index / 256) % 256) as u8;
    let octet_4 = (client_index % 256) as u8;
    let port = 10_000 + (client_index % 50_000) as u16;
    SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(10, octet_2, octet_3, octet_4)),
        port,
    )
}

fn load_summary(loads: &[u64]) -> (u64, u64, usize) {
    let min_load = loads.iter().copied().min().unwrap_or_default();
    let max_load = loads.iter().copied().max().unwrap_or_default();
    let non_empty = loads.iter().filter(|load| **load > 0).count();
    (min_load, max_load, non_empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_buffer_matrix_records_default_and_clamped_values() {
        let step =
            StreamTcpBufferMatrixFixture::build(StreamBenchConfig::default()).evaluate_once();

        assert_eq!(step.default_bytes, 16 * 1024);
        assert_eq!(step.min_bytes, 4 * 1024);
        assert_eq!(step.max_bytes, 256 * 1024);
        assert_eq!(step.row_count, 7);
        assert!(step.rows.iter().any(|row| row.used_default));
        assert!(step.rows.iter().any(|row| row.clamped_to_min));
        assert!(step.rows.iter().any(|row| row.clamped_to_max));
    }

    #[test]
    fn udp_distribution_records_dispatcher_and_session_spread() {
        let step =
            StreamUdpDistributionFixture::build(StreamBenchConfig::default()).evaluate_once();

        assert_eq!(step.clients, 4096);
        assert_eq!(step.dispatcher_workers, 16);
        assert_eq!(step.dispatcher_queue_capacity, 1024);
        assert_eq!(step.session_shards, 16);
        assert_eq!(step.non_empty_dispatcher_workers, 16);
        assert_eq!(step.non_empty_session_shards, 16);
        assert!(step.max_dispatcher_load >= step.min_dispatcher_load);
        assert!(step.max_session_shard_load >= step.min_session_shard_load);
    }

    #[test]
    fn udp_payload_copy_fixture_copies_full_payload() {
        let step = StreamUdpPayloadCopyFixture::build(StreamBenchConfig::default()).copy_once();

        assert_eq!(step.payload_bytes, 1200);
        assert_eq!(step.copied_bytes, 1200);
        assert!(step.checksum > 1200);
    }

    #[test]
    fn tcp_pool_contention_fixture_runs_and_reuses_connections() {
        let fixture = TcpPoolContentionFixture::build(
            TcpPoolContentionBenchConfig {
                threads: 2,
                prewarm_idle: 4,
            },
            1,
        )
        .expect("build pool contention fixture");

        let (step, latencies) = fixture.run(50).expect("run pool contention fixture");

        assert_eq!(step.threads, 2);
        assert_eq!(step.backend_keys, 1);
        assert_eq!(step.total_ops, 100);
        assert_eq!(latencies.len(), 100);
        assert!(step.hits > 0, "prewarmed pool should serve reuse hits");
    }
}
