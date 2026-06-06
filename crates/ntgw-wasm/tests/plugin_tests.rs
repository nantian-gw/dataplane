use std::collections::HashMap;

use ntgw_wasm::engine::create_engine;
use ntgw_wasm::plugin::{HookResult, PluginManager, WasmHook, WasmSandboxConfig};
use anyhow::Result;

/// Minimal WAT for a plugin that exports `on_request`, `alloc`, and `memory`.
///
/// `on_request` always returns 0 (Continue).
/// `alloc` is a trivial allocator returning `addr` directly.
const MINIMAL_PLUGIN_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "on_request") (result i32) i32.const 0)
  (func (export "on_response") (result i32) i32.const 0)
  (func (export "alloc") (param i32) (result i32) i32.const 0)
)
"#;

fn compile_wat(wat: &str) -> Result<Vec<u8>> {
    Ok(wat::parse_str(wat)?)
}

#[test]
fn test_plugin_manager_new() -> Result<()> {
    let engine = create_engine()?;
    let manager = PluginManager::new(engine)?;
    assert!(!manager.has_plugin("test"));
    Ok(())
}

#[test]
fn test_load_and_invoke_plugin() -> Result<()> {
    let engine = create_engine()?;
    let manager = PluginManager::new(engine)?;

    let wasm_bytes = compile_wat(MINIMAL_PLUGIN_WAT)?;

    manager.load_plugin(
        "test-plugin",
        &wasm_bytes,
        serde_json::json!({"key": "value"}),
        vec![WasmHook::OnRequest, WasmHook::OnResponse],
        WasmSandboxConfig::default(),
    )?;

    assert!(manager.has_plugin("test-plugin"));

    let result = manager.invoke_hook(
        "test-plugin",
        &WasmHook::OnRequest,
        HashMap::new(),
        Vec::new(),
    )?;

    assert_eq!(result, HookResult::Continue);
    Ok(())
}

#[test]
fn test_load_twice_overwrites() -> Result<()> {
    let engine = create_engine()?;
    let manager = PluginManager::new(engine)?;

    let wasm_bytes = compile_wat(MINIMAL_PLUGIN_WAT)?;

    manager
        .load_plugin(
            "test-plugin",
            &wasm_bytes,
            serde_json::json!({}),
            vec![WasmHook::OnRequest],
            WasmSandboxConfig::default(),
        )
        .expect("first load should succeed");

    manager
        .load_plugin(
            "test-plugin",
            &wasm_bytes,
            serde_json::json!({"updated": true}),
            vec![WasmHook::OnRequest],
            WasmSandboxConfig::default(),
        )
        .expect("second load should succeed");

    assert!(manager.has_plugin("test-plugin"));
    Ok(())
}

#[test]
fn test_unload_plugin() -> Result<()> {
    let engine = create_engine()?;
    let manager = PluginManager::new(engine)?;

    let wasm_bytes = compile_wat(MINIMAL_PLUGIN_WAT)?;

    manager.load_plugin(
        "test-plugin",
        &wasm_bytes,
        serde_json::json!({}),
        vec![WasmHook::OnRequest],
        WasmSandboxConfig::default(),
    )?;

    assert!(manager.has_plugin("test-plugin"));

    manager.unload_plugin("test-plugin");
    assert!(!manager.has_plugin("test-plugin"));
    Ok(())
}

#[test]
fn test_invoke_nonexistent_plugin() -> Result<()> {
    let engine = create_engine()?;
    let manager = PluginManager::new(engine)?;

    let result = manager.invoke_hook(
        "nonexistent",
        &WasmHook::OnRequest,
        HashMap::new(),
        Vec::new(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        format!("{err}").contains("not found"),
        "expected 'not found' error, got: {err}"
    );
    Ok(())
}

#[test]
fn test_invoke_missing_hook() -> Result<()> {
    let engine = create_engine()?;
    let manager = PluginManager::new(engine)?;

    let wasm_bytes = compile_wat(MINIMAL_PLUGIN_WAT)?;

    manager.load_plugin(
        "test-plugin",
        &wasm_bytes,
        serde_json::json!({}),
        vec![WasmHook::OnRequest],
        WasmSandboxConfig::default(),
    )?;

    // on_stream_chunk is not registered by the plugin, so it's silently skipped.
    let result = manager.invoke_hook(
        "test-plugin",
        &WasmHook::OnStreamChunk,
        HashMap::new(),
        Vec::new(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), HookResult::Continue);
    Ok(())
}

#[test]
fn test_sandbox_config_custom() {
    let cfg = WasmSandboxConfig {
        max_memory_bytes: 1024,
        max_execution_ms: 100,
    };
    assert_eq!(cfg.max_memory_bytes, 1024);
    assert_eq!(cfg.max_execution_ms, 100);
}

#[test]
fn test_hook_result_equality() {
    assert_eq!(HookResult::Continue, HookResult::Continue);
    assert_eq!(HookResult::Reject(403), HookResult::Reject(403));
    assert_ne!(HookResult::Continue, HookResult::Reject(403));
}
