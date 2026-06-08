use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

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
}
