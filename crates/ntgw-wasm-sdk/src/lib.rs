//! Upstream Gateway Wasm Plugin SDK
//!
//! This SDK provides the host function bindings and macros needed
//! to write Wasm plugins for the Upstream Gateway.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use upstream_wasm_sdk::*;
//!
//! #[no_mangle]
//! pub extern "C" fn on_request() -> i32 {
//!     allocator::reset();
//!     let ctx = &mut RequestContext::new();
//!     ctx.log(LogLevel::Info, "Plugin on_request called");
//!     ctx.set_header("X-Custom", "hello");
//!     0
//! }
//! ```

pub mod host;
pub mod types;

pub use host::*;
pub use types::*;
