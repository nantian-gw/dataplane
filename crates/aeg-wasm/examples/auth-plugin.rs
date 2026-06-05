//! Auth Plugin: validates API key on each request
use aeg_wasm_sdk::host::allocator;
use aeg_wasm_sdk::types::{LogLevel, RequestContext};

#[no_mangle]
pub extern "C" fn on_request() -> i32 {
    allocator::reset();
    let mut ctx = RequestContext::new();

    ctx.log(LogLevel::Info, "auth-plugin: checking x-api-key");

    match ctx.get_header("x-api-key") {
        Some(key) if !key.is_empty() => {
            ctx.log(LogLevel::Info, "auth-plugin: valid API key present");
            ctx.set_header("X-Auth-Result", "allowed");
            0
        }
        _ => {
            ctx.log(LogLevel::Warn, "auth-plugin: missing or invalid x-api-key");
            ctx.set_header("X-Auth-Result", "denied");
            401
        }
    }
}

#[no_mangle]
pub extern "C" fn dealloc(_ptr: i32, _len: i32) {}
