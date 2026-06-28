use std::collections::HashMap;
use std::sync::Arc;

use ntgw_wasm::plugin::WasmHook;
use ntgw_wasm::{HookResult, PluginManager, WasmError};
use tokio::sync::Semaphore;
use tokio::task;
use tracing;

/// Wasm filter, integrated into AI Gateway filter pipeline.
///
/// Executes Wasm plugins at configured hooks (on_request, on_response).
pub struct WasmPluginFilter {
    pub plugin_manager: Arc<PluginManager>,
    pub plugin_names: Vec<String>,
    concurrency_limit: Arc<Semaphore>,
}

impl WasmPluginFilter {
    pub fn new(plugin_manager: Arc<PluginManager>, plugin_names: Vec<String>) -> Self {
        Self {
            plugin_manager,
            plugin_names,
            concurrency_limit: Arc::new(Semaphore::new(1024)),
        }
    }

    /// Pre-request: execute all plugins' on_request hook.
    ///
    /// Each plugin receives request headers and the raw body. If any plugin
    /// returns a rejection code the entire pre-process phase fails.
    pub async fn pre_process(
        &self,
        request_headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(), WasmError> {
        let _permit = self.concurrency_limit.acquire().await.map_err(|_| {
            WasmError::PluginExecution("wasm_filter".to_string(), "concurrency limit closed".to_string())
        })?;
        let headers = Arc::new(request_headers);
        let body = Arc::new(body);
        for name in &self.plugin_names {
            tracing::debug!(
                target: "wasm_filter",
                plugin = %name,
                "invoking onRequest"
            );
            let name_for_spawn = name.clone();
            let plugin_manager = Arc::clone(&self.plugin_manager);
            let headers = Arc::clone(&headers);
            let body = Arc::clone(&body);
            let result = task::spawn_blocking(move || {
                plugin_manager.invoke_hook(
                    &name_for_spawn,
                    &WasmHook::OnRequest,
                    (*headers).clone(),
                    (*body).clone(),
                )
            })
            .await
            .map_err(|e| {
                WasmError::PluginExecution(
                    name.clone(),
                    format!("join error: {e}"),
                )
            })??;
            match result {
                HookResult::Continue { response_headers } => {
                    if !response_headers.is_empty() {
                        tracing::warn!(
                            target: "wasm_filter",
                            plugin = %name,
                            "guest set response headers but they are not propagated to the caller"
                        );
                    }
                }
                HookResult::Reject(code) => {
                    tracing::warn!(
                        target: "wasm_filter",
                        plugin = %name,
                        code,
                        "onRequest rejected"
                    );
                    return Err(WasmError::PluginRejected(name.clone(), code));
                }
            }
        }

        Ok(())
    }

    /// Post-response: execute all plugins' on_response hook.
    pub async fn post_process(
        &self,
        request_headers: HashMap<String, String>,
        response_body: Vec<u8>,
    ) -> Result<(), WasmError> {
        let _permit = self.concurrency_limit.acquire().await.map_err(|_| {
            WasmError::PluginExecution("wasm_filter".to_string(), "concurrency limit closed".to_string())
        })?;
        let headers = Arc::new(request_headers);
        let body = Arc::new(response_body);
        for name in &self.plugin_names {
            tracing::debug!(
                target: "wasm_filter",
                plugin = %name,
                "invoking onResponse"
            );
            let name_for_spawn = name.clone();
            let plugin_manager = Arc::clone(&self.plugin_manager);
            let headers = Arc::clone(&headers);
            let body = Arc::clone(&body);
            let result = task::spawn_blocking(move || {
                plugin_manager.invoke_hook(
                    &name_for_spawn,
                    &WasmHook::OnResponse,
                    (*headers).clone(),
                    (*body).clone(),
                )
            })
            .await
            .map_err(|e| {
                WasmError::PluginExecution(
                    name.clone(),
                    format!("join error: {e}"),
                )
            })??;
            match result {
                HookResult::Continue { response_headers } => {
                    if !response_headers.is_empty() {
                        tracing::warn!(
                            target: "wasm_filter",
                            plugin = %name,
                            "guest set response headers but they are not propagated to the caller"
                        );
                    }
                }
                HookResult::Reject(code) => {
                    tracing::warn!(
                        target: "wasm_filter",
                        plugin = %name,
                        code,
                        "onResponse rejected"
                    );
                    return Err(WasmError::PluginRejected(name.clone(), code));
                }
            }
        }

        Ok(())
    }
}
