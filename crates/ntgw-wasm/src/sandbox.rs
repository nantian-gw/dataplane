use std::collections::HashMap;
use std::fmt;

use anyhow::Result;
use wasmtime::{Engine, Linker, Module, ResourceLimiter, Store};

use crate::engine::PluginContext;
use crate::error::WasmError;

/// Resource limiter for AI sandbox to prevent OOM from malicious WASM modules.
///
/// Limits memory growth to 64 MiB and table elements to 1M entries.
/// This is a defense-in-depth measure complementing the epoch-deadline timeout.
struct AISandboxLimiter {
    max_memory_bytes: usize,
    max_table_elements: u32,
}

impl AISandboxLimiter {
    fn new(max_memory_bytes: usize, max_table_elements: u32) -> Self {
        Self {
            max_memory_bytes,
            max_table_elements,
        }
    }
}

impl ResourceLimiter for AISandboxLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok(desired <= self.max_memory_bytes)
    }

    fn table_growing(
        &mut self,
        _current: u32,
        desired: u32,
        _maximum: Option<u32>,
    ) -> anyhow::Result<bool> {
        Ok(desired <= self.max_table_elements)
    }
}


/// AI inference sandbox for running LLM-related operations in Wasm.
///
/// Provides tokenization and embedding via sandboxed Wasm modules.
/// Each module is expected to export `alloc`, `memory`, and an
/// operation-specific function (`tokenize` or `embed`).
pub struct AISandbox {
    engine: Engine,
    #[allow(dead_code)]
    linker: Linker<PluginContext>,
    modules: HashMap<String, Module>,
}

impl fmt::Debug for AISandbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AISandbox")
            .field("engine", &"<wasmtime::Engine>")
            .field("modules", &self.modules.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Clone for AISandbox {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            linker: Linker::new(&self.engine),
            modules: self.modules.clone(),
        }
    }
}

impl AISandbox {
    pub fn new(engine: Engine) -> Result<Self> {
        let linker = Linker::new(&engine);
        Ok(Self {
            engine,
            linker,
            modules: HashMap::new(),
        })
    }

    /// Load a sandbox module from raw Wasm bytes.
    pub fn load_module(&mut self, name: &str, wasm_bytes: &[u8]) -> Result<(), WasmError> {
        let module =
            Module::from_binary(&self.engine, wasm_bytes).map_err(|e| WasmError::LoadFailed {
                name: name.to_string(),
                reason: format!("sandbox compilation error: {e}"),
            })?;
        self.modules.insert(name.to_string(), module);
        Ok(())
    }

    /// Tokenize `text` using the named sandbox module.
    ///
    /// The module must export:
    /// - `alloc(len: i32) -> i32` — allocate `len` bytes, return pointer
    /// - `memory` — the linear memory
    /// - `tokenize(ptr: i32, len: i32) -> i32` — returns token count
    pub fn tokenize(&self, model: &str, text: &str) -> Result<Vec<u32>, WasmError> {
        let module = self
            .modules
            .get(model)
            .ok_or_else(|| WasmError::SandboxModuleNotFound(model.to_string()))?;

        let mut store = Store::new(&self.engine, PluginContext::default());
        store.set_epoch_deadline(100); // 100 ticks ≈ 100 ms with 1 ms epoch thread
        store.limiter(|_| AISandboxLimiter::new(64 * 1024 * 1024, 1_000_000));

        let instance = self.linker.instantiate(&mut store, module).map_err(|e| {
            WasmError::PluginExecution(
                model.to_string(),
                format!("sandbox instantiation failed: {e}"),
            )
        })?;

        // Write text to guest memory via alloc
        let text_bytes = text.as_bytes();
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|_| {
                WasmError::PluginExecution(
                    model.to_string(),
                    "sandbox module missing alloc export".to_string(),
                )
            })?;
        let ptr = alloc
            .call(&mut store, text_bytes.len() as i32)
            .map_err(|e| {
                WasmError::PluginExecution(model.to_string(), format!("alloc failed: {e}"))
            })?;

        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
            WasmError::Memory(anyhow::anyhow!("sandbox module has no memory export"))
        })?;
        memory
            .write(&mut store, ptr as usize, text_bytes)
            .map_err(|e| WasmError::Memory(anyhow::anyhow!("memory write failed: {e}")))?;

        // Call tokenize(ptr, len) -> token count
        let tokenize_func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "tokenize")
            .map_err(|_| {
                WasmError::PluginExecution(
                    model.to_string(),
                    "sandbox module missing tokenize export".to_string(),
                )
            })?;
        let token_count = tokenize_func
            .call(&mut store, (ptr, text_bytes.len() as i32))
            .map_err(|e| {
                if e.is::<wasmtime::Trap>() {
                    WasmError::PluginTimeout(format!("{model} tokenize"))
                } else {
                    WasmError::PluginExecution(
                        model.to_string(),
                        format!("tokenize call failed: {e}"),
                    )
                }
            })?;

        Ok(vec![token_count as u32])
    }

    /// Compute an embedding vector for `text` using the named sandbox module.
    ///
    /// The module must export:
    /// - `alloc(len: i32) -> i32`
    /// - `memory`
    /// - `embed(ptr: i32, len: i32) -> i32` — returns embedding dimension
    /// - `get_embedding(buf_ptr: i32, buf_len: i32)` — writes floats into buffer
    pub fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>, WasmError> {
        let module = self
            .modules
            .get(model)
            .ok_or_else(|| WasmError::SandboxModuleNotFound(model.to_string()))?;

        let mut store = Store::new(&self.engine, PluginContext::default());
        store.set_epoch_deadline(100);
        store.limiter(|_| AISandboxLimiter::new(64 * 1024 * 1024, 1_000_000));

        let instance = self.linker.instantiate(&mut store, module).map_err(|e| {
            WasmError::PluginExecution(
                model.to_string(),
                format!("sandbox instantiation failed: {e}"),
            )
        })?;

        let text_bytes = text.as_bytes();
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|_| {
                WasmError::PluginExecution(
                    model.to_string(),
                    "sandbox module missing alloc export".to_string(),
                )
            })?;
        let ptr = alloc
            .call(&mut store, text_bytes.len() as i32)
            .map_err(|e| {
                WasmError::PluginExecution(model.to_string(), format!("alloc failed: {e}"))
            })?;

        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
            WasmError::Memory(anyhow::anyhow!("sandbox module has no memory export"))
        })?;
        memory
            .write(&mut store, ptr as usize, text_bytes)
            .map_err(|e| WasmError::Memory(anyhow::anyhow!("memory write failed: {e}")))?;

        let embed_func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "embed")
            .map_err(|_| {
                WasmError::PluginExecution(
                    model.to_string(),
                    "sandbox module missing embed export".to_string(),
                )
            })?;
        let dim = embed_func
            .call(&mut store, (ptr, text_bytes.len() as i32))
            .map_err(|e| {
                if e.is::<wasmtime::Trap>() {
                    WasmError::PluginTimeout(format!("{model} embed"))
                } else {
                    WasmError::PluginExecution(model.to_string(), format!("embed call failed: {e}"))
                }
            })?;

        if dim <= 0 {
            return Ok(vec![]);
        }

        // Allocate buffer for embedding output and call get_embedding
        let buf_ptr = alloc
            .call(&mut store, dim * 4) // dim * sizeof(f32)
            .map_err(|e| {
                WasmError::PluginExecution(
                    model.to_string(),
                    format!("alloc for embedding buffer failed: {e}"),
                )
            })?;

        let get_emb = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "get_embedding")
            .map_err(|_| {
                WasmError::PluginExecution(
                    model.to_string(),
                    "sandbox module missing get_embedding export".to_string(),
                )
            })?;
        get_emb.call(&mut store, (buf_ptr, dim)).map_err(|e| {
            WasmError::PluginExecution(model.to_string(), format!("get_embedding call failed: {e}"))
        })?;

        // Read back the embedding vector
        let mut f32_bytes = vec![0u8; dim as usize * 4];
        memory
            .read(&store, buf_ptr as usize, &mut f32_bytes)
            .map_err(|e| WasmError::Memory(anyhow::anyhow!("read embedding failed: {e}")))?;

        let floats: Vec<f32> = f32_bytes
            .chunks_exact(4)
            .map(|chunk| {
                let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                f32::from_le_bytes(arr)
            })
            .collect();

        Ok(floats)
    }
}
