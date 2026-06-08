use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::RwLock;
use serde::Deserialize;
use tracing::{debug, info};
use wasmtime::{Module, Store, Val};

use crate::engine::{self, PluginContext};
use crate::error::WasmError;

/// Possible hook points where a plugin can execute.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum WasmHook {
    /// Executed when a request is received (before upstream proxy).
    #[serde(rename = "on_request")]
    OnRequest,
    /// Executed when a response is received (before sending to client).
    #[serde(rename = "on_response")]
    OnResponse,
    /// Executed for each streamed chunk.
    #[serde(rename = "on_stream_chunk")]
    OnStreamChunk,
}

impl WasmHook {
    /// Returns the exported function name for this hook.
    pub fn export_name(&self) -> &'static str {
        match self {
            WasmHook::OnRequest => "on_request",
            WasmHook::OnResponse => "on_response",
            WasmHook::OnStreamChunk => "on_stream_chunk",
        }
    }
}

/// Sandbox configuration for a loaded plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct WasmSandboxConfig {
    /// Maximum heap memory in bytes.
    pub max_memory_bytes: usize,
    /// Maximum execution time in milliseconds.
    pub max_execution_ms: u64,
}

impl Default for WasmSandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024, // 16 MiB
            max_execution_ms: 10,               // 10 ms
        }
    }
}

/// Return value from a hook invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult {
    /// Allow the request/response to proceed.
    Continue,
    /// Reject the request/response with the given HTTP status code.
    Reject(i32),
}

/// A loaded and compiled plugin module.
struct LoadedPlugin {
    module: Module,
    config: serde_json::Value,
    hooks: Vec<WasmHook>,
    sandbox: WasmSandboxConfig,
}

/// Manages the lifecycle of all loaded Wasm plugins.
///
/// Responsible for compiling, storing, invoking, and unloading plugins.
/// Maintains a shared engine, linker, and an epoch counter for timeout enforcement.
pub struct PluginManager {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<PluginContext>,
    plugins: RwLock<HashMap<String, LoadedPlugin>>,
    epoch_deadline: Arc<AtomicU64>,
}

impl PluginManager {
    /// Create a new PluginManager with the given engine.
    ///
    /// Spawns a background thread that increments the engine epoch every 1 ms
    /// so that guest timeouts are enforced.
    pub fn new(engine: wasmtime::Engine) -> Result<Self, WasmError> {
        let linker = engine::create_linker(&engine).map_err(|e| WasmError::LoadFailed {
            name: "linker".to_string(),
            reason: format!("{e}"),
        })?;
        let epoch_deadline = Arc::new(AtomicU64::new(0));

        // Spawn epoch incrementer
        let engine_ptr = engine.clone();
        let epoch_deadline_clone = Arc::clone(&epoch_deadline);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(1));
                let current = epoch_deadline_clone.load(Ordering::Relaxed);
                engine_ptr.increment_epoch();
                epoch_deadline_clone.store(current + 1, Ordering::Release);
            }
        });

        Ok(Self {
            engine,
            linker,
            plugins: RwLock::new(HashMap::new()),
            epoch_deadline,
        })
    }

    /// Load a plugin from raw Wasm bytes.
    pub fn load_plugin(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        config: serde_json::Value,
        hooks: Vec<WasmHook>,
        sandbox: WasmSandboxConfig,
    ) -> Result<(), WasmError> {
        let module = wasmtime::Module::from_binary(&self.engine, wasm_bytes).map_err(|e| {
            WasmError::LoadFailed {
                name: name.to_string(),
                reason: format!("compilation error: {e}"),
            }
        })?;

        let loaded = LoadedPlugin {
            module,
            config,
            hooks,
            sandbox,
        };

        let mut plugins = self.plugins.write();
        plugins.insert(name.to_string(), loaded);

        info!(plugin = name, "loaded plugin");
        Ok(())
    }

    /// Remove a plugin by name.
    pub fn unload_plugin(&self, name: &str) {
        let mut plugins = self.plugins.write();
        if plugins.remove(name).is_some() {
            info!(plugin = name, "unloaded plugin");
        } else {
            debug!(plugin = name, "plugin not found for unload");
        }
    }

    /// Check if a plugin with the given name is loaded.
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.read().contains_key(name)
    }

    /// Invoke a hook on a loaded plugin.
    ///
    /// Creates a fresh Store with the sandbox config applied as a resource
    /// limiter, instantiates the module, calls the hook export, and returns
    /// the result.
    pub fn invoke_hook(
        &self,
        name: &str,
        hook: &WasmHook,
        request_headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HookResult, WasmError> {
        let plugins = self.plugins.read();
        let plugin = plugins
            .get(name)
            .ok_or_else(|| WasmError::PluginNotFound(name.to_string()))?;

        if !plugin.hooks.contains(hook) {
            debug!(plugin = name, hook = ?hook, "plugin does not register hook, skipping");
            return Ok(HookResult::Continue);
        }

        let module = plugin.module.clone();
        let config = plugin.config.clone();
        let sandbox = plugin.sandbox.clone();
        drop(plugins);

        let mut store = Store::new(
            &self.engine,
            PluginContext {
                config,
                request_headers,
                response_headers: HashMap::new(),
                body,
                memory_limit: sandbox.max_memory_bytes,
            },
        );

        // Apply resource limiting through the PluginContext itself
        store.limiter(|ctx| ctx);

        // Set epoch deadline for timeout enforcement
        let max_ticks = sandbox.max_execution_ms;
        let current = self.epoch_deadline.load(Ordering::Acquire);
        store.set_epoch_deadline(current + max_ticks);

        // Instantiate the plugin
        let instance = self.linker.instantiate(&mut store, &module).map_err(|e| {
            WasmError::PluginExecution(name.to_string(), format!("instantiation error: {e}"))
        })?;

        // Call the hook export
        let export_name = hook.export_name();
        let func = instance.get_func(&mut store, export_name).ok_or_else(|| {
            WasmError::InvalidHook(format!("plugin '{name}' missing export '{export_name}'"))
        })?;

        let mut results = [Val::I32(0)];
        match func.call(&mut store, &[], &mut results) {
            Ok(_) => {
                let code = results.first().and_then(|v| v.i32()).unwrap_or(0);
                if code == 0 {
                    Ok(HookResult::Continue)
                } else {
                    Ok(HookResult::Reject(code))
                }
            }
            Err(e) => {
                if let Some(trap) = e.downcast_ref::<wasmtime::Trap>()
                    && matches!(trap, wasmtime::Trap::Interrupt)
                {
                    return Err(WasmError::PluginTimeout(name.to_string()));
                }
                Err(WasmError::PluginExecution(name.to_string(), format!("{e}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_hook_export_names() {
        assert_eq!(WasmHook::OnRequest.export_name(), "on_request");
        assert_eq!(WasmHook::OnResponse.export_name(), "on_response");
        assert_eq!(WasmHook::OnStreamChunk.export_name(), "on_stream_chunk");
    }

    #[test]
    fn test_sandbox_config_defaults() {
        let cfg = WasmSandboxConfig::default();
        assert_eq!(cfg.max_memory_bytes, 16 * 1024 * 1024);
        assert_eq!(cfg.max_execution_ms, 10);
    }
}
