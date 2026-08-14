use crate::error::WasmError;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use wasmtime::{Config, Engine, Linker, OptLevel};

/// Maximum accepted plugin/sandbox Wasm module size, checked before compilation.
pub const MAX_WASM_MODULE_BYTES: usize = 32 * 1024 * 1024;

/// Maximum number of table elements a guest may grow to at runtime.
pub const MAX_WASM_TABLE_ELEMENTS: usize = 10_000;
/// Maximum number of Store objects to keep in the reuse pool.
pub const STORE_POOL_MAX_SIZE: usize = 32;

fn global_engine_config() -> Config {
    let mut config = Config::default();
    config.epoch_interruption(true);
    // The plugin/sandbox loader only builds core modules with a single "memory"
    // export, so keep the component model and multi-memory off to shrink the
    // guest-reachable attack surface and compilation cost.
    config.wasm_multi_memory(false);
    config.wasm_component_model(false);
    config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Disable);
    config.cranelift_opt_level(OptLevel::Speed);
    config.parallel_compilation(true);
    // cache_config_load_default removed in wasmtime 30+
    config
}

static GLOBAL_ENGINE: OnceLock<Result<Arc<Engine>, String>> = OnceLock::new();

pub fn create_engine() -> Result<Engine, WasmError> {
    Engine::new(&global_engine_config()).map_err(|e| WasmError::RuntimeInit(e.to_string()))
}

pub fn global_engine() -> Result<Arc<Engine>, WasmError> {
    match GLOBAL_ENGINE.get_or_init(|| create_engine().map(Arc::new).map_err(|e| e.to_string())) {
        Ok(engine) => Ok(Arc::clone(engine)),
        Err(error) => Err(WasmError::RuntimeInit(error.clone())),
    }
}

#[derive(Default)]
pub struct PluginContext {
    pub config: Arc<serde_json::Value>,
    pub request_headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub memory_limit: usize,
    pub table_elements_limit: usize,
}
impl PluginContext {
    /// Reset the context for reuse with new request data.
    /// Used by the Store pool to avoid re-allocation.
    pub fn reset(
        &mut self,
        config: Arc<serde_json::Value>,
        request_headers: HashMap<String, String>,
        body: Vec<u8>,
        memory_limit: usize,
        table_elements_limit: usize,
    ) {
        self.config = config;
        self.request_headers = request_headers;
        self.response_headers = HashMap::new();
        self.body = body;
        self.memory_limit = memory_limit;
        self.table_elements_limit = table_elements_limit;
    }
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
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        Ok(desired <= self.table_elements_limit)
    }

    fn instances(&self) -> usize {
        1
    }
}

pub fn create_linker(engine: &Engine) -> Result<Linker<PluginContext>, WasmError> {
    let mut linker = Linker::new(engine);
    crate::host::register_host_functions(&mut linker)?;
    Ok(linker)
}

pub struct WasmEngine {
    pub engine: Arc<Engine>,
}

impl WasmEngine {
    pub fn new() -> Result<Self, WasmError> {
        Ok(Self {
            engine: create_engine().map(Arc::new)?,
        })
    }

    pub fn global() -> Result<Self, WasmError> {
        Ok(Self {
            engine: global_engine()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::ResourceLimiter;

    #[test]
    fn table_growing_respects_limit() {
        let mut ctx = PluginContext {
            table_elements_limit: 8,
            ..PluginContext::default()
        };
        assert!(ctx.table_growing(0, 8, None).unwrap());
        assert!(!ctx.table_growing(0, 9, None).unwrap());
    }

    #[test]
    fn memory_growing_respects_limit() {
        let mut ctx = PluginContext {
            memory_limit: 100,
            ..PluginContext::default()
        };
        assert!(ctx.memory_growing(0, 100, None).unwrap());
        assert!(!ctx.memory_growing(0, 101, None).unwrap());
    }
}
