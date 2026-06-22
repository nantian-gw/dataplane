use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use wasmtime::{Config, Engine, Linker, OptLevel};

fn global_engine_config() -> Config {
    let mut config = Config::default();
    config.epoch_interruption(true);
    config.wasm_multi_memory(true);
    config.wasm_component_model(true);
    config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
    config.cranelift_opt_level(OptLevel::Speed);
    config
}

static GLOBAL_ENGINE: OnceLock<Result<Arc<Engine>, String>> = OnceLock::new();

pub fn create_engine() -> Result<Engine> {
    Engine::new(&global_engine_config()).map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn global_engine() -> Result<Arc<Engine>> {
    match GLOBAL_ENGINE.get_or_init(|| create_engine().map(Arc::new).map_err(|e| e.to_string())) {
        Ok(engine) => Ok(Arc::clone(engine)),
        Err(error) => Err(anyhow::anyhow!(error.clone())),
    }
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
    pub engine: Arc<Engine>,
}

impl WasmEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            engine: create_engine().map(Arc::new)?,
        })
    }

    pub fn global() -> Result<Self> {
        Ok(Self {
            engine: global_engine()?,
        })
    }
}
