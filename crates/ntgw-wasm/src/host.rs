use anyhow::Result;
use tracing::{debug, warn};
use wasmtime::Linker;

use crate::engine::PluginContext;
use crate::mem;

/// Register all host functions available to guest plugins under the "upstream" module.
pub fn register_host_functions(linker: &mut Linker<PluginContext>) -> Result<()> {
    linker
        .func_wrap(
            "pingora",
            "log",
            |mut caller: wasmtime::Caller<'_, PluginContext>,
             level: i32,
             msg_ptr: i32,
             msg_len: i32| {
                let msg =
                    mem::read_guest_memory(&mut caller, msg_ptr, msg_len).unwrap_or_else(|e| {
                        warn!(error = %e, "failed to read guest log message");
                        Vec::new()
                    });
                let msg_str = String::from_utf8_lossy(&msg);
                match level {
                    0 => debug!(target: "ntgw_wasm", "{}", msg_str),
                    1..=2 => warn!(target: "ntgw_wasm", "{}", msg_str),
                    _ => debug!(target: "ntgw_wasm", level = level, "{}", msg_str),
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to register pingora::log: {e}"))?;

    linker
        .func_wrap(
            "pingora",
            "get_header",
            |mut caller: wasmtime::Caller<'_, PluginContext>,
             name_ptr: i32,
             name_len: i32|
             -> i64 {
                let name_buf = match mem::read_guest_memory(&mut caller, name_ptr, name_len) {
                    Ok(buf) => buf,
                    Err(_) => return 0,
                };
                let name = match String::from_utf8(name_buf) {
                    Ok(s) => s,
                    Err(_) => return 0,
                };

                let value = {
                    let ctx = caller.data();
                    ctx.request_headers.get(&name).cloned()
                };

                match value {
                    Some(val) => {
                        mem::allocate_guest_memory(caller, val.as_bytes()).unwrap_or_default()
                    }
                    None => 0,
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to register pingora::get_header: {e}"))?;

    linker
        .func_wrap(
            "pingora",
            "set_header",
            |mut caller: wasmtime::Caller<'_, PluginContext>,
             name_ptr: i32,
             name_len: i32,
             val_ptr: i32,
             val_len: i32| {
                let name = match mem::read_guest_memory(&mut caller, name_ptr, name_len) {
                    Ok(buf) => String::from_utf8_lossy(&buf).to_string(),
                    Err(_) => return,
                };
                let value = match mem::read_guest_memory(&mut caller, val_ptr, val_len) {
                    Ok(buf) => String::from_utf8_lossy(&buf).to_string(),
                    Err(_) => return,
                };
                let ctx = caller.data_mut();
                ctx.response_headers.insert(name, value);
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to register pingora::set_header: {e}"))?;

    Ok(())
}
