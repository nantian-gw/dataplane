use std::{hint::black_box, time::Instant};

use anyhow::{Context, Result};
use serde_json::json;

use crate::report::{
    elapsed_ms, sample_resources, summarize_durations, ResourceDelta, ScenarioReport,
};

pub(crate) fn run_header_filter_chain(
    iterations: u32,
    config: ntgw_http::bench::FilterChainBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::bench::FilterChainFixture::build(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.apply()?);
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("filter chain benchmark should execute at least one iteration")?;
    Ok(ScenarioReport {
        name: "header_filter_chain".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "request_filters": config.request_filters,
            "response_filters": config.response_filters,
            "header_ops_per_filter": config.header_ops_per_filter,
            "request_header_count": step.request_header_count,
            "response_header_count": step.response_header_count,
            "request_marker": step.request_marker,
            "response_marker": step.response_marker,
        }),
    })
}

pub(crate) fn run_request_meta_header_heavy(
    iterations: u32,
    config: ntgw_http::bench::RequestMetaBuildBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::bench::RequestMetaBuildFixture::build(config)?;
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.materialize()?);
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("request meta benchmark should execute at least one iteration")?;
    Ok(ScenarioReport {
        name: "request_meta_header_heavy".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "configured_header_count": config.header_count,
            "configured_values_per_header": config.values_per_header,
            "configured_query_params": config.query_params,
            "configured_header_value_bytes": config.header_value_bytes,
            "path": step.path,
            "header_name_count": step.header_name_count,
            "header_value_count": step.header_value_count,
            "header_name_bytes": step.header_name_bytes,
            "header_value_bytes": step.header_value_bytes,
            "request_header_bytes": step.request_header_bytes,
            "query_param_count": step.query_param_count,
            "query_value_count": step.query_value_count,
            "request_id": step.request_id,
            "content_length": step.content_length,
        }),
    })
}

pub(crate) fn run_request_view_header_heavy(
    iterations: u32,
    config: ntgw_http::bench::RequestViewBuildBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::bench::RequestViewBuildFixture::build(config)?;
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);

    for _ in 0..iterations {
        let started = Instant::now();
        let ctx = fixture.capture_context();
        black_box((
            ctx.host.len(),
            ctx.path.len(),
            ctx.method.len(),
            ctx.request_id.len(),
            ctx.bytes_received,
        ));
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = fixture.capture()?;
    Ok(ScenarioReport {
        name: "request_view_header_heavy".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "configured_header_count": config.header_count,
            "configured_values_per_header": config.values_per_header,
            "configured_query_params": config.query_params,
            "configured_header_value_bytes": config.header_value_bytes,
            "host": step.host,
            "path": step.path,
            "method": step.method,
            "client_ip": step.client_ip,
            "header_name_count": step.header_name_count,
            "header_value_count": step.header_value_count,
            "header_name_bytes": step.header_name_bytes,
            "header_value_bytes": step.header_value_bytes,
            "request_header_bytes": step.request_header_bytes,
            "query_param_count": step.query_param_count,
            "query_value_count": step.query_value_count,
            "request_id": step.request_id,
            "content_length": step.content_length,
        }),
    })
}

pub(crate) fn run_request_fast_path_selection(
    iterations: u32,
    config: ntgw_http::bench::RequestMetaBuildBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::bench::FastPathSelectionFixture::build(config)?;
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_selected = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_selected = Some(fixture.select()?);
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let selected = last_selected.context("fast path selection benchmark should execute")?;
    Ok(ScenarioReport {
        name: "request_fast_path_selection".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "route_name": selected.route_name,
            "route_namespace": selected.route_namespace,
            "selected_backend": selected.backend_name,
            "endpoint": format!("{}:{}", selected.backend.address, selected.backend.port),
        }),
    })
}

pub(crate) fn run_session_persistence(
    iterations: u32,
    config: ntgw_http::bench::SessionBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::bench::SessionBenchFixture::build(config)?;
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.encode_decode_cycle()?);
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step =
        last_step.context("session persistence benchmark should execute at least one iteration")?;
    Ok(ScenarioReport {
        name: "session_persistence".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "backend_name": step.backend_name,
            "endpoint_address": step.endpoint_address,
            "endpoint_port": step.endpoint_port,
            "token_len": step.token_len,
            "absolute_timeout_secs": config.absolute_timeout_secs,
            "idle_timeout_secs": config.idle_timeout_secs,
        }),
    })
}

pub(crate) fn run_access_log_write_path(
    iterations: u32,
    config: ntgw_observability::bench::AccessLogBenchConfig,
) -> Result<ScenarioReport> {
    run_access_log_path("access_log_write_path", iterations, config)
}

pub(crate) fn run_access_log_disabled_path(
    iterations: u32,
    config: ntgw_observability::bench::AccessLogBenchConfig,
) -> Result<ScenarioReport> {
    run_access_log_path(
        "access_log_disabled_path",
        iterations,
        ntgw_observability::bench::AccessLogBenchConfig {
            enabled: false,
            ..config
        },
    )
}

pub(crate) fn run_access_log_sampled_out_path(
    iterations: u32,
    config: ntgw_observability::bench::AccessLogBenchConfig,
) -> Result<ScenarioReport> {
    run_access_log_path(
        "access_log_sampled_out_path",
        iterations,
        ntgw_observability::bench::AccessLogBenchConfig {
            sample_rate: 0.0,
            ..config
        },
    )
}

fn run_access_log_path(
    name: &str,
    iterations: u32,
    config: ntgw_observability::bench::AccessLogBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_observability::bench::AccessLogFixture::build(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.write_once()?);
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("access log benchmark should execute at least one iteration")?;
    Ok(ScenarioReport {
        name: name.to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "rendered_bytes": step.rendered_bytes,
            "file_bytes": step.file_bytes,
            "route_annotation_count": step.route_annotation_count,
            "mode": step.mode,
            "enabled": step.enabled,
            "sample_rate": step.sample_rate,
            "emitted": step.emitted,
        }),
    })
}
