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
    on_request_plugin_names: Vec<String>,
    on_response_plugin_names: Vec<String>,
}

impl WasmPluginFilter {
    pub fn new(
        plugin_manager: Arc<PluginManager>,
        plugin_names: Vec<String>,
        max_concurrency: usize,
    ) -> Self {
        Self::new_with_hooks(plugin_manager, plugin_names, max_concurrency, true, true)
    }

    pub fn new_with_hooks(
        plugin_manager: Arc<PluginManager>,
        plugin_names: Vec<String>,
        max_concurrency: usize,
        has_on_request: bool,
        has_on_response: bool,
    ) -> Self {
        let on_request_plugin_names = if has_on_request {
            plugin_names.clone()
        } else {
            Vec::new()
        };
        let on_response_plugin_names = if has_on_response {
            plugin_names.clone()
        } else {
            Vec::new()
        };
        Self::new_with_hook_plugins(
            plugin_manager,
            plugin_names,
            max_concurrency,
            on_request_plugin_names,
            on_response_plugin_names,
        )
    }

    pub fn new_with_hook_plugins(
        plugin_manager: Arc<PluginManager>,
        plugin_names: Vec<String>,
        max_concurrency: usize,
        on_request_plugin_names: Vec<String>,
        on_response_plugin_names: Vec<String>,
    ) -> Self {
        Self {
            plugin_manager,
            plugin_names,
            concurrency_limit: Arc::new(Semaphore::new(max_concurrency)),
            on_request_plugin_names,
            on_response_plugin_names,
        }
    }

    pub fn has_on_request(&self) -> bool {
        !self.on_request_plugin_names.is_empty()
    }

    pub fn has_on_response(&self) -> bool {
        !self.on_response_plugin_names.is_empty()
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
        if !self.has_on_request() {
            return Ok(HashMap::new());
        }

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
        for name in &self.on_request_plugin_names {
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
        if !self.has_on_response() {
            return Ok(HashMap::new());
        }

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
        for name in &self.on_response_plugin_names {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_filter(has_on_request: bool, has_on_response: bool) -> WasmPluginFilter {
        let engine = ntgw_wasm::engine::create_engine().expect("engine");
        let manager = Arc::new(PluginManager::new(engine).expect("manager"));
        WasmPluginFilter::new_with_hooks(
            manager,
            vec!["plugin".to_string()],
            1,
            has_on_request,
            has_on_response,
        )
    }

    #[test]
    fn hook_capabilities_are_exposed() {
        let filter = test_filter(true, false);

        assert!(filter.has_on_request());
        assert!(!filter.has_on_response());
    }

    #[test]
    fn hook_specific_plugin_lists_drive_capabilities() {
        let engine = ntgw_wasm::engine::create_engine().expect("engine");
        let manager = Arc::new(PluginManager::new(engine).expect("manager"));
        let filter = WasmPluginFilter::new_with_hook_plugins(
            manager,
            vec!["request-plugin".to_string(), "response-plugin".to_string()],
            1,
            vec!["request-plugin".to_string()],
            Vec::new(),
        );

        assert!(filter.has_on_request());
        assert!(!filter.has_on_response());
    }

    #[tokio::test]
    async fn pre_process_skips_without_request_hook() {
        let filter = test_filter(false, true);

        let response_headers = filter
            .pre_process(
                HashMap::from([("x-test".to_string(), "1".to_string())]),
                vec![1],
            )
            .await
            .expect("skip without on_request hook");

        assert!(response_headers.is_empty());
    }

    #[tokio::test]
    async fn post_process_skips_without_response_hook() {
        let filter = test_filter(true, false);

        let response_headers = filter
            .post_process(
                HashMap::from([("x-test".to_string(), "1".to_string())]),
                vec![1],
            )
            .await
            .expect("skip without on_response hook");

        assert!(response_headers.is_empty());
    }
}
