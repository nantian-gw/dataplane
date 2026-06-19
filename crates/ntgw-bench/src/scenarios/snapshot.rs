use std::sync::Arc;
use std::{hint::black_box, time::Instant};

use anyhow::Result;
use arc_swap::ArcSwap;
use serde_json::json;

use crate::report::{
    ResourceDelta, ScenarioReport, elapsed_ms, sample_resources, summarize_durations,
};

pub(crate) fn run_snapshot_read_rwlock(
    iterations: u32,
    config: ntgw_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_ir::bench::build_route_selection_fixture(config);
    let shared = ntgw_ir::Snapshot::shared();
    shared.store(Arc::new(fixture.snapshot.clone()));
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut observed_routes = 0usize;

    for _ in 0..iterations {
        let started = Instant::now();
        let snapshot = shared.load();
        observed_routes = snapshot.http_routes.len();
        black_box((
            snapshot.id.len(),
            snapshot.listeners.len(),
            snapshot.http_routes.len(),
            snapshot.backends.len(),
        ));
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "snapshot_read_rwlock".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "snapshot_model": "Arc<RwLock<Snapshot>>",
            "listeners": fixture.snapshot.listeners.len(),
            "http_routes": observed_routes,
            "backends": fixture.snapshot.backends.len(),
        }),
    })
}

pub(crate) fn run_snapshot_read_arc_swap(
    iterations: u32,
    config: ntgw_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_ir::bench::build_route_selection_fixture(config);
    let shared = ArcSwap::from_pointee(fixture.snapshot.clone());
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut observed_routes = 0usize;

    for _ in 0..iterations {
        let started = Instant::now();
        let snapshot = shared.load();
        observed_routes = snapshot.http_routes.len();
        black_box((
            snapshot.id.len(),
            snapshot.listeners.len(),
            snapshot.http_routes.len(),
            snapshot.backends.len(),
        ));
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "snapshot_read_arc_swap".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "snapshot_model": "ArcSwap<Snapshot>",
            "listeners": fixture.snapshot.listeners.len(),
            "http_routes": observed_routes,
            "backends": fixture.snapshot.backends.len(),
        }),
    })
}

pub(crate) fn run_runtime_index_rebuild_route_only(
    iterations: u32,
    config: ntgw_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    run_runtime_index_rebuild(iterations, config, RuntimeIndexMutation::Route)
}

pub(crate) fn run_runtime_index_rebuild_endpoint_only(
    iterations: u32,
    config: ntgw_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    run_runtime_index_rebuild(iterations, config, RuntimeIndexMutation::Endpoint)
}

pub(crate) fn run_runtime_index_rebuild_secret_only(
    iterations: u32,
    config: ntgw_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    run_runtime_index_rebuild(iterations, config, RuntimeIndexMutation::Secret)
}

#[derive(Clone, Copy)]
enum RuntimeIndexMutation {
    Route,
    Endpoint,
    Secret,
}

impl RuntimeIndexMutation {
    fn scenario_name(self) -> &'static str {
        match self {
            Self::Route => "runtime_index_rebuild_route_only",
            Self::Endpoint => "runtime_index_rebuild_endpoint_only",
            Self::Secret => "runtime_index_rebuild_secret_only",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Route => "route-only",
            Self::Endpoint => "endpoint-only",
            Self::Secret => "secret-only",
        }
    }
}

fn run_runtime_index_rebuild(
    iterations: u32,
    config: ntgw_ir::bench::SnapshotBenchConfig,
    mutation: RuntimeIndexMutation,
) -> Result<ScenarioReport> {
    let fixture = ntgw_ir::bench::build_snapshot_switch_fixture(config);
    let mut snapshot = fixture.current;
    apply_runtime_index_mutation(&mut snapshot, mutation);

    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);

    for _ in 0..iterations {
        let started = Instant::now();
        snapshot.rebuild_runtime_indexes();
        black_box((
            snapshot.runtime_indexes_ready,
            snapshot.backend_index.len(),
            snapshot.secret_index.len(),
            snapshot.workload_namespace_index.len(),
            snapshot.http_listener_indices.len(),
            snapshot.grpc_listener_indices.len(),
            snapshot.stream_listener_route_index.len(),
        ));
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: mutation.scenario_name().to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "mutation": mutation.label(),
            "index_strategy": "full-rebuild-baseline",
            "listeners": snapshot.listeners.len(),
            "http_routes": snapshot.http_routes.len(),
            "grpc_routes": snapshot.grpc_routes.len(),
            "stream_routes": snapshot.stream_routes.len(),
            "backends": snapshot.backends.len(),
            "secrets": snapshot.secrets.len(),
            "workloads": snapshot.workloads.len(),
            "backend_index_entries": snapshot.backend_index.len(),
            "secret_index_entries": snapshot.secret_index.len(),
            "workload_index_entries": snapshot.workload_namespace_index.len(),
        }),
    })
}

fn apply_runtime_index_mutation(snapshot: &mut ntgw_ir::Snapshot, mutation: RuntimeIndexMutation) {
    snapshot.id = format!("bench-runtime-index-{}", mutation.label());

    match mutation {
        RuntimeIndexMutation::Route => {
            if let Some(route) = snapshot.http_routes.first_mut() {
                route
                    .hostnames
                    .push("route-only-bench.example.com".to_string());
                if let Some(rule) = route.rules.first_mut()
                    && let Some(route_match) = rule.matches.first_mut()
                {
                    route_match.path = "/route-only-bench".to_string();
                    route_match.path_type = "PathPrefix".to_string();
                }
            }
        }
        RuntimeIndexMutation::Endpoint => {
            if let Some(endpoint) = snapshot
                .backends
                .first_mut()
                .and_then(|backend| backend.endpoints.first_mut())
            {
                endpoint.address = "10.255.0.1".to_string();
            }
        }
        RuntimeIndexMutation::Secret => {
            if let Some(secret) = snapshot.secrets.first_mut() {
                secret.cert_pem.push_str("-rotated");
                secret.key_pem.push_str("-rotated");
            }
        }
    }
}
