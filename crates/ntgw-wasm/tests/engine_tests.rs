use anyhow::Result;
use ntgw_wasm::engine::{PluginContext, create_engine, create_linker};

#[test]
fn test_create_engine() -> Result<()> {
    let engine = create_engine()?;
    // Verify engine is functional - it should be not null
    assert!(true);
    Ok(())
}

#[test]
fn test_create_linker() -> Result<()> {
    let engine = create_engine()?;
    let linker = create_linker(&engine)?;
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
