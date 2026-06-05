use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::json;

use crate::report::{
    elapsed_ms, sample_resources, summarize_durations, ResourceDelta, ScenarioReport,
};

pub(crate) fn run_large_snapshot_switch(
    iterations: u32,
    config: aeg_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = aeg_ir::bench::build_snapshot_switch_fixture(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut selected_backend_name = String::new();

    for _ in 0..iterations {
        let started = Instant::now();
        let mut next = fixture.next.clone();
        next.inherit_runtime_state_from(&fixture.current);
        let selected = next
            .select_backend(&fixture.probe_request)
            .context("snapshot switch probe request should resolve a backend")?;
        selected_backend_name = selected.backend_name;
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "large_snapshot_switch".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "listeners": fixture.current.listeners.len(),
            "routes": fixture.current.http_routes.len(),
            "backends": fixture.current.backends.len(),
            "expected_backend_name": fixture.expected_backend_name,
            "selected_backend_name": selected_backend_name,
        }),
    })
}

pub(crate) fn run_http_route_selection(
    iterations: u32,
    config: aeg_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = aeg_ir::bench::build_route_selection_fixture(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut selected_backend_name = String::new();

    for _ in 0..iterations {
        let started = Instant::now();
        let selected = fixture
            .snapshot
            .select_backend(&fixture.http_request)
            .context("http route selection benchmark should resolve a backend")?;
        selected_backend_name = selected.backend_name;
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "http_route_selection".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "listeners": fixture.snapshot.listeners.len(),
            "http_routes": fixture.snapshot.http_routes.len(),
            "listener_candidate_model": "single-pass-best-host-score",
            "expected_backend_name": fixture.expected_http_backend_name,
            "selected_backend_name": selected_backend_name,
        }),
    })
}

pub(crate) fn run_grpc_route_selection(
    iterations: u32,
    config: aeg_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = aeg_ir::bench::build_route_selection_fixture(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut selected_backend_name = String::new();

    for _ in 0..iterations {
        let started = Instant::now();
        let selected = fixture
            .snapshot
            .select_grpc_backend(&fixture.grpc_request)
            .context("grpc route selection benchmark should resolve a backend")?;
        selected_backend_name = selected.backend_name;
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "grpc_route_selection".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "listeners": fixture.snapshot.listeners.len(),
            "grpc_routes": fixture.snapshot.grpc_routes.len(),
            "listener_candidate_model": "single-pass-best-host-score",
            "expected_backend_name": fixture.expected_grpc_backend_name,
            "selected_backend_name": selected_backend_name,
        }),
    })
}

pub(crate) fn run_stream_route_selection(
    iterations: u32,
    config: aeg_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = aeg_ir::bench::build_route_selection_fixture(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut selected_backend_name = String::new();

    for _ in 0..iterations {
        let started = Instant::now();
        let selected = fixture
            .snapshot
            .select_stream_backend(
                fixture.stream_listener_name.as_str(),
                Some(fixture.stream_server_name.as_str()),
            )
            .context("stream route selection benchmark should resolve a backend")?;
        selected_backend_name = selected.backend_name;
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "stream_route_selection".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "listeners": fixture.snapshot.listeners.len(),
            "stream_routes": fixture.snapshot.stream_routes.len(),
            "listener_name": fixture.stream_listener_name,
            "server_name": fixture.stream_server_name,
            "expected_backend_name": fixture.expected_stream_backend_name,
            "selected_backend_name": selected_backend_name,
        }),
    })
}

pub(crate) fn run_xds_snapshot_parse(
    iterations: u32,
    config: aeg_ir::bench::SnapshotBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = aeg_ir::bench::build_proto_snapshot_fixture(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut decoded_routes = 0usize;
    let mut decoded_backends = 0usize;
    let mut runtime_indexes_ready = false;

    for _ in 0..iterations {
        let started = Instant::now();
        let snapshot = aeg_ir::Snapshot::from(fixture.snapshot.clone());
        decoded_routes =
            snapshot.http_routes.len() + snapshot.grpc_routes.len() + snapshot.stream_routes.len();
        decoded_backends = snapshot.backends.len();
        runtime_indexes_ready = snapshot.runtime_indexes_ready;
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    Ok(ScenarioReport {
        name: "xds_snapshot_parse".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "listeners": fixture.expected_listener_count,
            "http_routes": fixture.expected_http_routes,
            "grpc_routes": fixture.expected_grpc_routes,
            "stream_routes": fixture.expected_stream_routes,
            "decoded_routes": decoded_routes,
            "backends": fixture.expected_backends,
            "decoded_backends": decoded_backends,
            "runtime_indexes_ready": runtime_indexes_ready,
        }),
    })
}
