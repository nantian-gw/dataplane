use std::time::Instant;

use anyhow::Result;
use serde_json::json;

use crate::report::{
    ResourceDelta, ScenarioReport, elapsed_ms, sample_resources, summarize_durations,
};

pub(crate) fn run_traffic_observe_reused_topology(
    iterations: u32,
    config: ntgw_observability::bench::TrafficStatsBenchConfig,
) -> Result<ScenarioReport> {
    run_traffic_observe_path(
        "traffic_observe_reused_topology",
        iterations,
        config,
        ntgw_observability::bench::TrafficStatsTopologyMode::ReusedTopology,
    )
}

pub(crate) fn run_traffic_observe_no_route(
    iterations: u32,
    config: ntgw_observability::bench::TrafficStatsBenchConfig,
) -> Result<ScenarioReport> {
    run_traffic_observe_path(
        "traffic_observe_no_route",
        iterations,
        config,
        ntgw_observability::bench::TrafficStatsTopologyMode::NoRoute,
    )
}

pub(crate) fn run_traffic_observe_backend_topology_4_shards(
    iterations: u32,
    mut config: ntgw_observability::bench::TrafficStatsBenchConfig,
) -> Result<ScenarioReport> {
    config.shard_count = 4;
    run_traffic_observe_path(
        "traffic_observe_backend_topology_4_shards",
        iterations,
        config,
        ntgw_observability::bench::TrafficStatsTopologyMode::BackendTopology,
    )
}

pub(crate) fn run_traffic_observe_backend_topology_64_shards(
    iterations: u32,
    mut config: ntgw_observability::bench::TrafficStatsBenchConfig,
) -> Result<ScenarioReport> {
    config.shard_count = 64;
    run_traffic_observe_path(
        "traffic_observe_backend_topology_64_shards",
        iterations,
        config,
        ntgw_observability::bench::TrafficStatsTopologyMode::BackendTopology,
    )
}

fn run_traffic_observe_path(
    name: &str,
    iterations: u32,
    config: ntgw_observability::bench::TrafficStatsBenchConfig,
    topology_mode: ntgw_observability::bench::TrafficStatsTopologyMode,
) -> Result<ScenarioReport> {
    let fixture = ntgw_observability::bench::TrafficStatsFixture::build(config, topology_mode);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);

    for _ in 0..iterations {
        let started = Instant::now();
        fixture.observe_once();
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = fixture.snapshot_step();
    if step.total_events == 0 {
        anyhow::bail!("traffic stats benchmark should execute at least one iteration");
    }
    let expected_events = u64::from(iterations);
    let expected_request_events = expected_events;

    Ok(ScenarioReport {
        name: name.to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "topology_mode": step.topology_mode,
            "shard_count": step.shard_count,
            "provided_topology": step.provided_topology,
            "has_backend_topology": step.has_backend_topology,
            "total_events": step.total_events,
            "total_request_events": step.total_request_events,
            "expected_events": expected_events,
            "expected_request_events": expected_request_events,
            "total_bytes_received": step.total_bytes_received,
            "total_bytes_sent": step.total_bytes_sent,
            "node_count": step.node_count,
            "edge_count": step.edge_count,
            "request_latency_histogram_count": step.request_latency_histogram_count,
            "response_flag_count": step.response_flag_count,
        }),
    })
}
