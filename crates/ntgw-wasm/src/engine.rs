use std::collections::HashMap;

use anyhow::Result;
use wasmtime::{Config, Engine, Linker, OptLevel};

pub fn create_engine() -> Result<Engine> {
    let mut config = Config::default();
    config.epoch_interruption(true);
    config.wasm_multi_memory(true);
    config.wasm_component_model(true);
    config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
    config.cranelift_opt_level(OptLevel::Speed);
    Engine::new(&config).map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(Default)]
pub struct PluginContext {
    pub config: serde_json::Value,
    pub request_headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub memory_limit: usize,
}

impl wasmtime::ResourceLimiter for PluginContext {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        Ok(desired <= self.memory_limit)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        Ok(true)
    }

    fn instances(&self) -> usize {
        1
    }
}

pub fn create_linker(engine: &Engine) -> Result<Linker<PluginContext>> {
    let mut linker = Linker::new(engine);
    crate::host::register_host_functions(&mut linker)?;
    Ok(linker)
}

pub struct WasmEngine {
    pub engine: Engine,
}

impl WasmEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            engine: create_engine()?,
        })
    }
}
