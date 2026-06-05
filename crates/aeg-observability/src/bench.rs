use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

use crate::access::{flush_access_log, render_access_log, write_access_log};
use crate::{
    shutdown_access_log_writer, AccessLogMode, AccessLogOptions, AccessLogRecord,
    SharedTrafficStats, TrafficObservation, TrafficObservationRef, TrafficRuntimeIds,
    TrafficSnapshot, TrafficTopology,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AccessLogBenchConfig {
    pub annotation_overrides: usize,
    pub enabled: bool,
    pub sample_rate: f64,
}

impl Default for AccessLogBenchConfig {
    fn default() -> Self {
        Self {
            annotation_overrides: 4,
            enabled: true,
            sample_rate: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogBenchStep {
    pub rendered_bytes: usize,
    pub file_bytes: u64,
    pub route_annotation_count: usize,
    pub mode: String,
    pub emitted: bool,
    pub enabled: bool,
    pub sample_rate: f64,
}

pub struct AccessLogFixture {
    path: PathBuf,
    options: AccessLogOptions,
    route_annotations: BTreeMap<String, String>,
    record: AccessLogRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrafficStatsBenchConfig {
    pub shard_count: usize,
}

impl Default for TrafficStatsBenchConfig {
    fn default() -> Self {
        Self { shard_count: 16 }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum TrafficStatsTopologyMode {
    ReusedTopology,
    NoRoute,
    BackendTopology,
}

impl TrafficStatsTopologyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReusedTopology => "reused_topology",
            Self::NoRoute => "no_route",
            Self::BackendTopology => "backend_topology",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStatsBenchStep {
    pub topology_mode: String,
    pub shard_count: usize,
    pub provided_topology: bool,
    pub has_backend_topology: bool,
    pub total_events: u64,
    pub total_request_events: u64,
    pub total_bytes_received: u64,
    pub total_bytes_sent: u64,
    pub node_count: usize,
    pub edge_count: usize,
    pub request_latency_histogram_count: usize,
    pub response_flag_count: usize,
}

pub struct TrafficStatsFixture {
    stats: SharedTrafficStats,
    observation: TrafficObservation,
    topology: Option<TrafficTopology>,
    topology_mode: TrafficStatsTopologyMode,
    shard_count: usize,
}

impl TrafficStatsFixture {
    pub fn build(config: TrafficStatsBenchConfig, topology_mode: TrafficStatsTopologyMode) -> Self {
        let observation = match topology_mode {
            TrafficStatsTopologyMode::NoRoute => TrafficObservation {
                listener_name: "default/gw/http".to_string(),
                protocol: "HTTP".to_string(),
                status: Some(404),
                latency_ms: 7,
                bytes_received: 96,
                bytes_sent: 256,
                response_flags: "NR".to_string(),
                ..TrafficObservation::default()
            },
            TrafficStatsTopologyMode::ReusedTopology => TrafficObservation {
                listener_name: "default/gw/http".to_string(),
                protocol: "HTTP".to_string(),
                route_namespace: "default".to_string(),
                route_name: "bench-route".to_string(),
                route_kind: "HTTPRoute".to_string(),
                status: Some(200),
                latency_ms: 3,
                bytes_received: 128,
                bytes_sent: 2048,
                upstream_pool_hits: 1,
                runtime_ids: TrafficRuntimeIds {
                    listener: Some(0x1001),
                    route: Some(0x2001),
                    backend: None,
                },
                ..TrafficObservation::default()
            },
            TrafficStatsTopologyMode::BackendTopology => TrafficObservation {
                listener_name: "default/gw/http".to_string(),
                protocol: "HTTP".to_string(),
                route_namespace: "default".to_string(),
                route_name: "bench-route".to_string(),
                route_kind: "HTTPRoute".to_string(),
                backend_name: "default/api:8080".to_string(),
                status: Some(200),
                latency_ms: 5,
                bytes_received: 256,
                bytes_sent: 4096,
                upstream_pool_hits: 1,
                runtime_ids: TrafficRuntimeIds {
                    listener: Some(0x1001),
                    route: Some(0x2001),
                    backend: Some(0x3001),
                },
                ..TrafficObservation::default()
            },
        };
        let topology = match topology_mode {
            TrafficStatsTopologyMode::NoRoute => Some(TrafficTopology::unmatched(
                observation.listener_name.as_str(),
            )),
            TrafficStatsTopologyMode::ReusedTopology
            | TrafficStatsTopologyMode::BackendTopology => Some(TrafficTopology::from_parts(
                observation.listener_name.as_str(),
                observation.route_kind.as_str(),
                observation.route_namespace.as_str(),
                observation.route_name.as_str(),
                observation.backend_name.as_str(),
            )),
        };

        Self {
            stats: SharedTrafficStats::with_shard_count(config.shard_count),
            observation,
            topology,
            topology_mode,
            shard_count: config.shard_count.max(1).next_power_of_two(),
        }
    }

    pub fn observe_once(&self) {
        self.stats.observe_ref_with_topology(
            TrafficObservationRef::from(&self.observation),
            self.topology.as_ref().map(TrafficTopology::as_ref),
        );
    }

    pub fn snapshot_step(&self) -> TrafficStatsBenchStep {
        let snapshot = self.stats.snapshot();
        traffic_stats_step_from_snapshot(
            self.topology_mode,
            self.shard_count,
            self.topology.is_some(),
            &snapshot,
        )
    }
}

impl AccessLogFixture {
    pub fn build(config: AccessLogBenchConfig) -> Self {
        let path = unique_log_path("aeg-access-log-bench");
        let mut route_annotations = BTreeMap::new();
        for index in 0..config.annotation_overrides {
            route_annotations.insert(
                format!("gateway.nantian.dev/access-log-extra-{index}"),
                format!("ignored-{index}"),
            );
        }
        route_annotations.insert(
            "gateway.nantian.dev/access-log-mode".to_string(),
            "json".to_string(),
        );

        Self {
            options: AccessLogOptions {
                path: path.display().to_string(),
                enabled: config.enabled,
                mode: AccessLogMode::Json,
                sample_rate: config.sample_rate,
                ..AccessLogOptions::default()
            },
            route_annotations,
            record: AccessLogRecord {
                event: "http_request".to_string(),
                timestamp: "2026-01-01T00:00:00.000Z".to_string(),
                start_time_unix_ms: 1_700_000_000_000,
                snapshot_version: "bench-v1".to_string(),
                listener: "default/gw/http".to_string(),
                protocol: "HTTP".to_string(),
                client_ip: "10.0.0.10".to_string(),
                host: "bench.example.com".to_string(),
                method: "GET".to_string(),
                path: "/bench/items".to_string(),
                request_id: "bench-request".to_string(),
                route_namespace: "default".to_string(),
                route_name: "bench-route".to_string(),
                route_kind: "Http".to_string(),
                backend: "default/bench-backend:8080".to_string(),
                status: Some(200),
                latency_ms: 12,
                bytes_sent: 512,
                bytes_received: 128,
                retry_attempts: 0,
                response_flags: String::new(),
                ..AccessLogRecord::default()
            },
            path,
        }
    }

    pub fn write_once(&self) -> Result<AccessLogBenchStep> {
        write_access_log(&self.options, &self.route_annotations, &self.record)?;
        let rendered = if self.options.enabled && self.options.sample_rate > 0.0 {
            flush_access_log(self.path.display().to_string().as_str())?;
            render_access_log(&self.options, &self.record)?.len()
        } else {
            0
        };
        let file_bytes = fs::metadata(&self.path)
            .map(|meta| meta.len())
            .unwrap_or_default();

        Ok(AccessLogBenchStep {
            rendered_bytes: rendered,
            file_bytes,
            route_annotation_count: self.route_annotations.len(),
            mode: "json".to_string(),
            emitted: file_bytes > 0,
            enabled: self.options.enabled,
            sample_rate: self.options.sample_rate,
        })
    }

    pub fn cleanup(&self) {
        shutdown_access_log_writer(self.path.display().to_string().as_str());
        let _ = fs::remove_file(&self.path);
    }
}

impl Drop for AccessLogFixture {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn traffic_stats_step_from_snapshot(
    topology_mode: TrafficStatsTopologyMode,
    shard_count: usize,
    provided_topology: bool,
    snapshot: &TrafficSnapshot,
) -> TrafficStatsBenchStep {
    TrafficStatsBenchStep {
        topology_mode: topology_mode.as_str().to_string(),
        shard_count,
        provided_topology,
        has_backend_topology: snapshot
            .edges
            .iter()
            .any(|edge| edge.edge_id.contains("backend")),
        total_events: snapshot.total_events,
        total_request_events: snapshot.total_request_events,
        total_bytes_received: snapshot.total_bytes_received,
        total_bytes_sent: snapshot.total_bytes_sent,
        node_count: snapshot.nodes.len(),
        edge_count: snapshot.edges.len(),
        request_latency_histogram_count: snapshot.request_latency_ms_histograms.len(),
        response_flag_count: snapshot.response_flags.len(),
    }
}

fn unique_log_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{unique}.log"))
}
