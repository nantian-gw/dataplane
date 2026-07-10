use std::collections::HashMap;
use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::json;

use ntgw_wasm::engine::create_engine;
use ntgw_wasm::plugin::{PluginManager, WasmHook, WasmSandboxConfig};

use crate::report::{
    ResourceDelta, ScenarioReport, elapsed_ms, sample_resources, summarize_durations,
};

/// Minimal plugin exporting an `on_request` hook that returns 0 (Continue),
/// plus the `memory`/`alloc` exports the host expects. Kept intentionally
/// trivial so the benchmark measures per-call Store creation + instantiation
/// cost, not guest execution.
const MINIMAL_PLUGIN_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "on_request") (result i32) i32.const 0)
  (func (export "alloc") (param i32) (result i32) i32.const 0)
)
"#;

const HOOK_PLUGIN_NAME: &str = "bench-hook-plugin";
const HEADER_COUNT: usize = 16;

struct WasmHookFixture {
    manager: PluginManager,
    header_template: HashMap<String, String>,
}

impl WasmHookFixture {
    fn build() -> Result<Self> {
        let engine = create_engine().context("create wasm engine")?;
        let manager = PluginManager::new(engine)
            .map_err(|e| anyhow::anyhow!("create plugin manager: {e}"))?;
        let wasm_bytes = wat::parse_str(MINIMAL_PLUGIN_WAT).context("compile bench plugin wat")?;
        manager
            .load_plugin(
                HOOK_PLUGIN_NAME,
                &wasm_bytes,
                json!({}),
                vec![WasmHook::OnRequest],
                WasmSandboxConfig::default(),
            )
            .map_err(|e| anyhow::anyhow!("load bench plugin: {e}"))?;

        let mut header_template = HashMap::with_capacity(HEADER_COUNT);
        for index in 0..HEADER_COUNT {
            header_template.insert(
                format!("x-bench-header-{index:02}"),
                format!("value-{index:02}"),
            );
        }

        Ok(Self {
            manager,
            header_template,
        })
    }

    fn invoke_empty(&self) -> Result<()> {
        self.manager
            .invoke_hook(
                HOOK_PLUGIN_NAME,
                &WasmHook::OnRequest,
                HashMap::new(),
                Vec::new(),
            )
            .map_err(|e| anyhow::anyhow!("invoke empty hook: {e}"))?;
        Ok(())
    }

    fn invoke_with_headers(&self) -> Result<()> {
        let headers = self.header_template.clone();
        self.manager
            .invoke_hook(HOOK_PLUGIN_NAME, &WasmHook::OnRequest, headers, Vec::new())
            .map_err(|e| anyhow::anyhow!("invoke header hook: {e}"))?;
        Ok(())
    }
}

pub(crate) fn run_wasm_hook_empty_invoke(iterations: u32) -> Result<ScenarioReport> {
    let fixture = WasmHookFixture::build().context("build wasm hook fixture")?;
    // Warm up one invocation so first-instantiate/module-cache cost is excluded.
    fixture.invoke_empty().context("warm up empty hook")?;

    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let started = Instant::now();
        fixture.invoke_empty()?;
        durations.push(elapsed_ms(started.elapsed()));
    }
    let after = sample_resources();

    Ok(ScenarioReport {
        name: "wasm_hook_empty_invoke".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "hook": "on_request",
            "header_count": 0,
            "body_bytes": 0,
            "evidence_type": "wasm-hook-fresh-store-instantiate",
            "note": "each invocation builds a fresh Store and instantiates via InstancePre",
        }),
    })
}

pub(crate) fn run_wasm_hook_header_heavy_invoke(iterations: u32) -> Result<ScenarioReport> {
    let fixture = WasmHookFixture::build().context("build wasm hook fixture")?;
    fixture
        .invoke_with_headers()
        .context("warm up header hook")?;

    let before = sample_resources();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let started = Instant::now();
        fixture.invoke_with_headers()?;
        durations.push(elapsed_ms(started.elapsed()));
    }
    let after = sample_resources();

    Ok(ScenarioReport {
        name: "wasm_hook_header_heavy_invoke".to_string(),
        iterations,
        timing: summarize_durations(&durations),
        resource_delta: ResourceDelta::between(&before, &after),
        resources_before: before.clone(),
        resources_after: after.clone(),
        details: json!({
            "hook": "on_request",
            "header_count": HEADER_COUNT,
            "body_bytes": 0,
            "evidence_type": "wasm-hook-fresh-store-instantiate-with-header-marshal",
            "note": "each invocation clones the header map into PluginContext then instantiates",
        }),
    })
}
