use anyhow::{Context, Result};
use wasmtime::{AsContext, AsContextMut, Caller};

use crate::engine::PluginContext;

/// Allocate memory inside the guest module and copy `data` into it.
///
/// Returns a packed `i64` where the high 32 bits are the pointer and the
/// low 32 bits are the length.
pub fn allocate_guest_memory(mut caller: Caller<'_, PluginContext>, data: &[u8]) -> Result<i64> {
    let alloc = caller
        .get_export("alloc")
        .and_then(|e| e.into_func())
        .with_context(|| "guest must export 'alloc' function")?;

    let len = data.len() as i32;
    let mut results = [wasmtime::Val::I32(0)];
    alloc
        .call(
            caller.as_context_mut(),
            &[wasmtime::Val::I32(len)],
            &mut results,
        )
        .map_err(|e| anyhow::anyhow!("failed to call guest alloc: {e}"))?;
    let ptr = results[0]
        .i32()
        .with_context(|| "alloc returned non-i32 value")?;

    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .with_context(|| "guest must export 'memory'")?;

    mem.write(caller.as_context_mut(), ptr as usize, data)
        .map_err(|e| anyhow::anyhow!("failed to write into guest memory: {e}"))?;

    let packed = ((ptr as i64) << 32) | (len as i64 & 0xFFFF_FFFF);
    Ok(packed)
}

/// Read `len` bytes from guest linear memory starting at `ptr`.
pub fn read_guest_memory(
    caller: &mut Caller<'_, PluginContext>,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>> {
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .with_context(|| "guest must export 'memory'")?;

    let mut buf = vec![0u8; len as usize];
    mem.read(caller.as_context(), ptr as usize, &mut buf)
        .map_err(|e| anyhow::anyhow!("failed to read from guest memory: {e}"))?;
    Ok(buf)
}

/// Unpack a packed pointer+length pair and read the corresponding guest memory.
///
/// The packed value uses the high 32 bits as the pointer and low 32 bits as the length.
pub fn read_packed_guest_memory(
    caller: &mut Caller<'_, PluginContext>,
    packed: i64,
) -> Result<Vec<u8>> {
    let ptr = (packed >> 32) as i32;
    let len = (packed & 0xFFFF_FFFF) as i32;
    if ptr == 0 || len == 0 {
        return Ok(Vec::new());
    }
    read_guest_memory(caller, ptr, len)
}
