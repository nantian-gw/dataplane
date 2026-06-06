//! Hello Plugin: adds x-custom header to requests
use ntgw_wasm_sdk::host::allocator;
use ntgw_wasm_sdk::types::{LogLevel, RequestContext};

#[no_mangle]
pub extern "C" fn on_request() -> i32 {
    allocator::reset();
    let mut ctx = RequestContext::new();
    ctx.log(LogLevel::Info, "hello-plugin: processing request");
    ctx.set_header("X-Custom", "hello from wasm");
    0
}

#[no_mangle]
pub extern "C" fn dealloc(_ptr: i32, _len: i32) {}
