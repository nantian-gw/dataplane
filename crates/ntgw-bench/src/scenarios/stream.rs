use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::json;

use crate::report::{
    ResourceDelta, ScenarioReport, elapsed_ms, sample_resources, summarize_durations,
};

pub(crate) fn run_stream_tcp_buffer_matrix(
    iterations: u32,
    config: ntgw_stream::bench::StreamBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_stream::bench::StreamTcpBufferMatrixFixture::build(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.evaluate_once());
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("stream tcp buffer matrix benchmark should execute")?;
    Ok(ScenarioReport {
        name: "stream_tcp_buffer_matrix".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "default_bytes": step.default_bytes,
            "min_bytes": step.min_bytes,
            "max_bytes": step.max_bytes,
            "row_count": step.row_count,
            "rows": step.rows,
            "evidence_type": "tcp-buffer-tuning-matrix",
        }),
    })
}

pub(crate) fn run_stream_udp_dispatcher_distribution(
    iterations: u32,
    config: ntgw_stream::bench::StreamBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_stream::bench::StreamUdpDistributionFixture::build(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.evaluate_once());
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("stream udp distribution benchmark should execute")?;
    Ok(ScenarioReport {
        name: "stream_udp_dispatcher_distribution".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "clients": step.clients,
            "dispatcher_workers": step.dispatcher_workers,
            "dispatcher_queue_capacity": step.dispatcher_queue_capacity,
            "session_shards": step.session_shards,
            "non_empty_dispatcher_workers": step.non_empty_dispatcher_workers,
            "non_empty_session_shards": step.non_empty_session_shards,
            "min_dispatcher_load": step.min_dispatcher_load,
            "max_dispatcher_load": step.max_dispatcher_load,
            "min_session_shard_load": step.min_session_shard_load,
            "max_session_shard_load": step.max_session_shard_load,
            "evidence_type": "udp-dispatcher-session-shard-distribution",
        }),
    })
}

pub(crate) fn run_stream_pool_contention_hot_key(
    iterations: u32,
    config: ntgw_stream::bench::TcpPoolContentionBenchConfig,
) -> Result<ScenarioReport> {
    run_pool_contention("stream_pool_contention_hot_key", iterations, config, 1)
}

pub(crate) fn run_stream_pool_contention_spread(
    iterations: u32,
    config: ntgw_stream::bench::TcpPoolContentionBenchConfig,
) -> Result<ScenarioReport> {
    let backend_keys = config.threads.max(1);
    run_pool_contention(
        "stream_pool_contention_spread",
        iterations,
        config,
        backend_keys,
    )
}

fn run_pool_contention(
    name: &str,
    iterations: u32,
    config: ntgw_stream::bench::TcpPoolContentionBenchConfig,
    backend_keys: usize,
) -> Result<ScenarioReport> {
    let fixture = ntgw_stream::bench::TcpPoolContentionFixture::build(config, backend_keys)
        .context("build tcp pool contention fixture")?;
    let before = sample_resources();
    let (step, latencies) = fixture
        .run(iterations as usize)
        .context("run tcp pool contention fixture")?;
    let after = sample_resources();

    Ok(ScenarioReport {
        name: name.to_string(),
        iterations: step.total_ops.min(u64::from(u32::MAX)) as u32,
        timing: summarize_durations(&latencies),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "threads": step.threads,
            "backend_keys": step.backend_keys,
            "prewarm_idle": step.prewarm_idle,
            "total_ops": step.total_ops,
            "hits": step.hits,
            "misses": step.misses,
            "elapsed_ms": step.elapsed_ms,
            "throughput_ops_per_sec": step.throughput_ops_per_sec,
            "evidence_type": "tcp-pool-get-return-contention",
        }),
    })
}

pub(crate) fn run_stream_udp_payload_copy(
    iterations: u32,
    config: ntgw_stream::bench::StreamBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_stream::bench::StreamUdpPayloadCopyFixture::build(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.copy_once());
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("stream udp payload copy benchmark should execute")?;
    Ok(ScenarioReport {
        name: "stream_udp_payload_copy".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "payload_bytes": step.payload_bytes,
            "copied_bytes": step.copied_bytes,
            "checksum": step.checksum,
            "evidence_type": "udp-payload-copy-hot-path",
        }),
    })
}
