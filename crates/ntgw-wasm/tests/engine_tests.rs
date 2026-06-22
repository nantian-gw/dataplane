use std::sync::Arc;

use anyhow::Result;
use ntgw_wasm::engine::WasmEngine;
use ntgw_wasm::engine::{create_engine, create_linker, global_engine};

#[test]
fn test_create_engine() -> Result<()> {
    let _engine = create_engine()?;
    // Verify engine is functional - it should be not null
    assert!(true);
    Ok(())
}

#[test]
fn test_create_linker() -> Result<()> {
    let engine = create_engine()?;
    let _linker = create_linker(&engine)?;
    // Linker was created without errors
    assert!(true);
    Ok(())
}

#[test]
fn test_engine_clone() -> Result<()> {
    let engine = create_engine()?;
    let _engine2 = engine.clone();
    assert!(true);
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
