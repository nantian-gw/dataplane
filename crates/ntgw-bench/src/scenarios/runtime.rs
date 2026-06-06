use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::json;

use crate::report::{
    elapsed_ms, sample_resources, summarize_durations, ResourceDelta, ScenarioReport,
};

pub(crate) fn run_http_capacity_matrix(
    iterations: u32,
    config: ntgw_http::runtime_bench::HttpCapacityMatrixBenchConfig,
) -> Result<ScenarioReport> {
    let fixture = ntgw_http::runtime_bench::HttpCapacityMatrixFixture::build(config);
    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    let mut last_step = None;

    for _ in 0..iterations {
        let started = Instant::now();
        last_step = Some(fixture.evaluate_once());
        durations.push(elapsed_ms(started.elapsed()));
    }

    let after = sample_resources();
    let step = last_step.context("http capacity matrix benchmark should execute")?;
    Ok(ScenarioReport {
        name: "http_capacity_matrix".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "row_count": step.row_count,
            "min_parallelism": step.min_parallelism,
            "max_parallelism": step.max_parallelism,
            "default_rows": step.default_rows,
            "tuned_rows": step.tuned_rows,
            "rows": step.rows,
            "evidence_type": "capacity-derivation-matrix",
        }),
    })
}
