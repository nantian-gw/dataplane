use std::sync::Arc;

use anyhow::Result;
use ntgw_wasm::engine::{PluginContext, WasmEngine};
use ntgw_wasm::engine::{create_engine, create_linker, global_engine};
use wasmtime::{Module, Store};

#[test]
fn test_create_engine() -> Result<()> {
    let engine = create_engine()?;
    let module = Module::new(&engine, "(module)")?;
    assert_eq!(module.imports().len(), 0);
    Ok(())
}

#[test]
fn test_create_linker() -> Result<()> {
    let engine = create_engine()?;
    let linker = create_linker(&engine)?;
    let module = Module::new(&engine, "(module)")?;
    let mut store = Store::new(&engine, PluginContext::default());
    let instance = linker.instantiate(&mut store, &module)?;
    assert_eq!(instance.exports(&mut store).len(), 0);
    Ok(())
}

#[test]
fn test_engine_clone() -> Result<()> {
    let engine = create_engine()?;
    let engine2 = engine.clone();
    let module = Module::new(&engine2, "(module)")?;
    assert_eq!(module.imports().len(), 0);
    Ok(())
}

#[test]
fn test_global_engine_returns_reusable_result() -> Result<()> {
    let first = global_engine()?;
    let second = global_engine()?;
    assert!(Arc::ptr_eq(&first, &second));
    Ok(())
}

#[test]
fn test_wasm_engine_global_returns_result() -> Result<()> {
    let first = WasmEngine::global()?;
    let second = WasmEngine::global()?;
    assert!(Arc::ptr_eq(&first.engine, &second.engine));
    Ok(())
}
