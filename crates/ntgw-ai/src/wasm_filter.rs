use std::collections::HashMap;
use std::sync::Arc;

use ntgw_wasm::plugin::WasmHook;
use ntgw_wasm::{HookResult, PluginManager, WasmError};
use tokio::sync::{Semaphore, TryAcquireError};
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
    pub fn new(plugin_manager: Arc<PluginManager>, plugin_names: Vec<String>, max_concurrency: usize) -> Self {
        Self {
            plugin_manager,
            plugin_names,
            concurrency_limit: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    /// Pre-request: execute all plugins' on_request hook.
    ///
    /// Each plugin receives request headers and the raw body. If any plugin
    /// returns a rejection code the entire pre-process phase fails.
    ///
    /// On success, returns the response headers set by plugins via `set_header`,
    /// so the caller can propagate them to the actual HTTP response.
    pub async fn pre_process(
        &self,
        request_headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HashMap<String, String>, WasmError> {
        let _permit = match self.concurrency_limit.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => {
                tracing::warn!(target: "wasm_filter", "wasm concurrency limit reached, returning 503");
                return Err(WasmError::PluginExecution(
                    "wasm_filter".to_string(),
                    "concurrency limit reached".to_string(),
                ));
            }
            Err(TryAcquireError::Closed) => {
                return Err(WasmError::PluginExecution(
                    "wasm_filter".to_string(),
                    "concurrency limit closed".to_string(),
                ));
            }
        };
        let headers = Arc::new(request_headers);
        let body = Arc::new(body);
        let mut all_response_headers: HashMap<String, String> = HashMap::new();
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
            .map_err(|e| WasmError::PluginExecution(name.clone(), format!("join error: {e}")))??;
            match result {
                HookResult::Continue { response_headers } => {
                    all_response_headers.extend(response_headers);
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

        Ok(all_response_headers)
    }

    /// Post-response: execute all plugins' on_response hook.
    ///
    /// Returns the response headers set by plugins so the caller can apply them
    /// to the ongoing response before it is committed to the client.
    pub async fn post_process(
        &self,
        request_headers: HashMap<String, String>,
        response_body: Vec<u8>,
    ) -> Result<HashMap<String, String>, WasmError> {
        let _permit = match self.concurrency_limit.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => {
                tracing::warn!(target: "wasm_filter", "wasm concurrency limit reached, returning 503");
                return Err(WasmError::PluginExecution(
                    "wasm_filter".to_string(),
                    "concurrency limit reached".to_string(),
                ));
            }
            Err(TryAcquireError::Closed) => {
                return Err(WasmError::PluginExecution(
                    "wasm_filter".to_string(),
                    "concurrency limit closed".to_string(),
                ));
            }
        };
        let headers = Arc::new(request_headers);
        let body = Arc::new(response_body);
        let mut all_response_headers: HashMap<String, String> = HashMap::new();
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
            .map_err(|e| WasmError::PluginExecution(name.clone(), format!("join error: {e}")))??;
            match result {
                HookResult::Continue { response_headers } => {
                    all_response_headers.extend(response_headers);
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

        Ok(all_response_headers)
    }
}