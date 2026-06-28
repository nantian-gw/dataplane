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

    linker
        .func_wrap(
            "pingora",
            "get_body",
            |mut caller: wasmtime::Caller<'_, PluginContext>, buf_ptr: i32, buf_len: i32| -> i32 {
                let body_copy = caller.data().body.clone();
                let len = body_copy.len().min(buf_len as usize);
                if len == 0 {
                    return 0;
                }
                let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                    Some(m) => m,
                    None => {
                        warn!("get_body: guest must export 'memory'");
                        return 0;
                    }
                };
                match mem.write(&mut caller, buf_ptr as usize, &body_copy[..len]) {
                    Ok(_) => len as i32,
                    Err(e) => {
                        warn!(error = %e, "get_body: failed to write to guest memory");
                        0
                    }
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to register pingora::get_body: {e}"))?;

    linker
        .func_wrap(
            "pingora",
            "get_all_headers",
            |mut caller: wasmtime::Caller<'_, PluginContext>, buf_ptr: i32, buf_len: i32| -> i32 {
                let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                    Some(m) => m,
                    None => {
                        warn!("get_all_headers: guest must export 'memory'");
                        return -1;
                    }
                };
                let mut buf: Vec<u8> = Vec::new();
                let ctx = caller.data();
                for (k, v) in &ctx.request_headers {
                    buf.extend_from_slice(k.as_bytes());
                    buf.push(0);
                    buf.extend_from_slice(v.as_bytes());
                    buf.push(0);
                }
                let len = buf.len().min(buf_len as usize);
                if len == 0 {
                    return 0;
                }
                let _ = mem.write(&mut caller, buf_ptr as usize, &buf[..len]);
                len as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to register pingora::get_all_headers: {e}"))?;

    Ok(())
}
