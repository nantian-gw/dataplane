use std::{fs, time::Instant};

use anyhow::{Context, Result};
use serde_json::json;

use crate::report::{
    elapsed_ms, sample_resources, summarize_durations, unique_temp_dir, ResourceDelta,
    ScenarioReport,
};

pub(crate) fn run_tls_asset_rotation(
    iterations: u32,
    config: ntgw_http::runtime_bench::TlsRotationBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::runtime_bench::TlsRotationFixture::build(config);
    let asset_dir = unique_temp_dir("dataplane-reload-bench");
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_initial = None;
    let mut last_rotated = None;
    let mut remaining_files = 0usize;

    for _ in 0..iterations {
        let started = Instant::now();
        last_initial = Some(fixture.materialize_initial(&asset_dir).with_context(|| {
            format!(
                "materializing initial tls assets in {}",
                asset_dir.display()
            )
        })?);
        last_rotated = Some(
            fixture
                .rotate(&asset_dir)
                .with_context(|| format!("rotating tls assets in {}", asset_dir.display()))?,
        );
        remaining_files = fixture
            .cleanup_rotated(&asset_dir)
            .with_context(|| format!("cleaning up tls assets in {}", asset_dir.display()))?;
        durations.push(elapsed_ms(started.elapsed()));
    }

    let _ = fs::remove_dir_all(&asset_dir);
    let after = sample_resources();
    let initial =
        last_initial.context("tls rotation benchmark should materialize an initial step")?;
    let rotated =
        last_rotated.context("tls rotation benchmark should materialize a rotated step")?;

    Ok(ScenarioReport {
        name: "tls_asset_rotation".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "listener_count": initial.listener_count,
            "initial_unique_asset_prefixes": initial.unique_asset_prefixes,
            "initial_reused_assets": initial.reused_assets,
            "rotated_unique_asset_prefixes": rotated.unique_asset_prefixes,
            "rotated_reused_assets": rotated.reused_assets,
            "remaining_files_after_cleanup": remaining_files,
        }),
    })
}

pub(crate) async fn run_high_frequency_apply(
    iterations: u32,
    config: ntgw_xds::bench::ApplyBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_xds::bench::ReloadBench::new(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_outcome = None;

    for index in 0..iterations {
        let version = format!("bench-success-{}", index + 1);
        let started = Instant::now();
        let outcome = fixture
            .apply_success(version.as_str())
            .await
            .map_err(anyhow::Error::msg)
            .context("xds high-frequency apply benchmark should ACK the current version")?;
        durations.push(elapsed_ms(started.elapsed()));
        last_outcome = Some(outcome);
    }

    let after = sample_resources();
    let outcome = last_outcome.context("xds apply benchmark should produce an outcome")?;
    Ok(ScenarioReport {
        name: "high_frequency_apply".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "ready": outcome.ready,
            "version": outcome.version,
            "message": outcome.message,
        }),
    })
}

pub(crate) async fn run_last_good_fallback(
    iterations: u32,
    config: ntgw_xds::bench::ApplyBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_xds::bench::ReloadBench::new(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_outcome = None;

    for index in 0..iterations {
        let last_good = format!("bench-good-{}", index + 1);
        let rejected = format!("bench-bad-{}", index + 1);
        let started = Instant::now();
        let outcome = fixture
            .apply_failure_with_last_good(last_good.as_str(), rejected.as_str())
            .await;
        durations.push(elapsed_ms(started.elapsed()));
        last_outcome = Some(outcome);
    }

    let after = sample_resources();
    let outcome = last_outcome.context("last-good fallback benchmark should produce an outcome")?;
    Ok(ScenarioReport {
        name: "last_good_fallback".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "ready": outcome.ready,
            "rejected_version": outcome.rejected_version,
            "last_good_version": outcome.last_good_version,
            "message": outcome.message,
            "apply_error": outcome.apply_error,
        }),
    })
}
