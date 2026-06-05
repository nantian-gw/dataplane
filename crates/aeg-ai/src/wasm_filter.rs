use std::collections::HashMap;
use std::sync::Arc;

use aeg_wasm::plugin::WasmHook;
use aeg_wasm::{HookResult, PluginManager, WasmError};
use tracing;

/// Wasm filter, integrated into AI Gateway filter pipeline.
///
/// Executes Wasm plugins at configured hooks (on_request, on_response).
pub struct WasmPluginFilter {
    pub plugin_manager: Arc<PluginManager>,
    pub plugin_names: Vec<String>,
}

impl WasmPluginFilter {
    pub fn new(plugin_manager: Arc<PluginManager>, plugin_names: Vec<String>) -> Self {
        Self {
            plugin_manager,
            plugin_names,
        }
    }

    /// Pre-request: execute all plugins' on_request hook.
    ///
    /// Each plugin receives request headers and the raw body. If any plugin
    /// returns a rejection code the entire pre-process phase fails.
    pub fn pre_process(
        &self,
        request_headers: &HashMap<String, String>,
        body: &[u8],
    ) -> Result<(), WasmError> {
        for name in &self.plugin_names {
            tracing::debug!(
                target: "wasm_filter",
                plugin = %name,
                "invoking onRequest"
            );
            match self.plugin_manager.invoke_hook(
                name,
                &WasmHook::OnRequest,
                request_headers.clone(),
                body.to_vec(),
            )? {
                HookResult::Continue => {}
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
    pub fn post_process(
        &self,
        request_headers: &HashMap<String, String>,
        response_body: &[u8],
    ) -> Result<(), WasmError> {
        for name in &self.plugin_names {
            tracing::debug!(
                target: "wasm_filter",
                plugin = %name,
                "invoking onResponse"
            );
            self.plugin_manager.invoke_hook(
                name,
                &WasmHook::OnResponse,
                request_headers.clone(),
                response_body.to_vec(),
            )?;
        }

        Ok(())
    }
}
