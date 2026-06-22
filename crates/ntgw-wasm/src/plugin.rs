use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::OnceLock;
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
    sha256: Option<String>,
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
            sha256: None,
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

    /// Load a new plugin or update an existing one in-place.
    ///
    /// Returns Ok(true) if the plugin was newly loaded or its WASM bytes changed,
    /// Ok(false) if the plugin was unchanged (same SHA256), or an error on failure.
    pub fn load_or_update(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        config: serde_json::Value,
        hooks: Vec<WasmHook>,
        sandbox: WasmSandboxConfig,
        sha256: Option<&str>,
    ) -> Result<bool, WasmError> {
        // Skip if SHA256 hasn't changed and plugin already loaded.
        if let Some(sha) = sha256 {
            let plugins = self.plugins.read();
            if let Some(existing) = plugins.get(name)
                && existing.sha256.as_deref() == Some(sha)
            {
                debug!(
                    plugin = name,
                    sha256 = sha,
                    "plugin unchanged, skipping reload"
                );
                return Ok(false);
            }
        }

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
            sha256: sha256.map(String::from),
        };

        let was_update = {
            let mut plugins = self.plugins.write();
            let existed = plugins.contains_key(name);
            plugins.insert(name.to_string(), loaded);
            existed
        };

        if was_update {
            info!(plugin = name, "reloaded plugin (wasm bytes changed)");
        } else {
            info!(plugin = name, "loaded new plugin");
        }
        Ok(true)
    }

    /// Synchronize the plugin set to match the desired state.
    ///
    /// - Loads or updates plugins present in `desired`
    /// - Unloads plugins not present in `desired`
    /// - Skips plugins whose SHA256 hasn't changed
    ///
    /// Returns counts of (loaded, updated, skipped, unloaded) plugins.
    pub fn diff_and_apply(&self, desired: &[WasmPluginSpec]) -> (usize, usize, usize, usize) {
        let mut loaded = 0;
        let mut updated = 0;
        let mut skipped = 0;
        let mut unloaded = 0;

        let desired_names: HashSet<&str> = desired
            .iter()
            .map(|(name, _, _, _, _, _)| name.as_str())
            .collect();

        // Unload plugins not in the desired set.
        {
            let plugins = self.plugins.read();
            let to_remove: Vec<String> = plugins
                .keys()
                .filter(|name| !desired_names.contains(name.as_str()))
                .cloned()
                .collect();
            drop(plugins);

            for name in &to_remove {
                self.unload_plugin(name);
                unloaded += 1;
            }
        }

        // Load or update desired plugins.
        for (name, wasm_bytes, config, hooks, sandbox, sha256) in desired {
            match self.load_or_update(
                name,
                wasm_bytes,
                config.clone(),
                hooks.clone(),
                sandbox.clone(),
                sha256.as_deref(),
            ) {
                Ok(true) => {
                    if self.plugins.read().contains_key(name.as_str())
                        && !self.plugins.read().is_empty()
                    {
                        updated += 1;
                    } else {
                        loaded += 1;
                    }
                }
                Ok(false) => skipped += 1,
                Err(e) => {
                    tracing::warn!(
                        target: "wasm",
                        plugin = %name,
                        error = %e,
                        "failed to load/reload plugin, keeping previous version"
                    );
                }
            }
        }

        (loaded, updated, skipped, unloaded)
    }

    /// Returns the names of all currently loaded plugins.
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.read().keys().cloned().collect()
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

/// Specification for loading or updating a WASM plugin.
pub type WasmPluginSpec = (
    String,
    Vec<u8>,
    serde_json::Value,
    Vec<WasmHook>,
    WasmSandboxConfig,
    Option<String>,
);

static GLOBAL_PLUGIN_MANAGER: OnceLock<Result<Arc<PluginManager>, String>> = OnceLock::new();

/// Returns the global PluginManager, creating it from the global Engine if needed.
pub fn global_plugin_manager() -> Result<Arc<PluginManager>, WasmError> {
    match GLOBAL_PLUGIN_MANAGER.get_or_init(|| {
        let engine = crate::engine::global_engine().map_err(|e| e.to_string())?;
        PluginManager::new((*engine).clone())
            .map(Arc::new)
            .map_err(|e| e.to_string())
    }) {
        Ok(manager) => Ok(Arc::clone(manager)),
        Err(error) => Err(WasmError::RuntimeInit(error.clone())),
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
