#![allow(unsafe_code)]
use crate::types::LogLevel;

/// Host function: log a message
///
/// # Safety
/// Called from the host via wasmtime linker
pub unsafe fn host_log(level: LogLevel, msg: &str) {
    #[link(wasm_import_module = "pingora")]
    // SAFETY: These are WASM host function imports. The wasmtime runtime
    // guarantees correct function signatures and calling conventions.
    unsafe extern "C" {
        fn log(level: i32, msg_ptr: *const u8, msg_len: i32);
    }
    // SAFETY: The log function is a valid WASM import with a matching
    // signature. The msg pointer and len reference a valid Rust &str,
    // so they are valid for the duration of this call.
    unsafe {
        log(level as i32, msg.as_ptr(), msg.len() as i32);
    }
}

/// Host function: get a request header value
///
/// Returns None if the header doesn't exist
///
/// # Safety
/// Calls host function that reads guest memory
pub unsafe fn host_get_header(name: &str) -> Option<String> {
    #[link(wasm_import_module = "pingora")]
    // SAFETY: These are WASM host function imports. The wasmtime runtime
    // guarantees correct function signatures and calling conventions.
    unsafe extern "C" {
        fn get_header(name_ptr: *const u8, name_len: i32) -> i64;
    }
    // SAFETY: The get_header function is a valid WASM import. The name
    // pointer and len reference a valid Rust &str, so they are valid
    // for the duration of this call.
    let packed = unsafe { get_header(name.as_ptr(), name.len() as i32) };
    if packed == 0 {
        return None;
    }
    let ptr = (packed >> 32) as i32;
    let len = (packed & 0xFFFF_FFFF) as i32;
    // Read from linear memory at [ptr..ptr+len]
    // SAFETY: The pointer and length are returned by the host from our own
    // guest memory. The host guarantees the range [ptr, ptr+len) is valid
    // within WASM linear memory for the pointer it returned.
    let data = unsafe {
        let slice = core::slice::from_raw_parts(ptr as *const u8, len as usize);
        core::str::from_utf8(slice).ok()?.to_string()
    };
    Some(data)
}

/// Host function: set a header value
///
/// # Safety
/// Calls host function with pointers into guest memory
pub unsafe fn host_set_header(name: &str, value: &str) {
    #[link(wasm_import_module = "pingora")]
    // SAFETY: These are WASM host function imports. The wasmtime runtime
    // guarantees correct function signatures and calling conventions.
    unsafe extern "C" {
        fn set_header(name_ptr: *const u8, name_len: i32, val_ptr: *const u8, val_len: i32);
    }
    // SAFETY: The set_header function is a valid WASM import with a
    // matching signature. All pointers and lengths reference valid
    // Rust &str values, so they are valid for the duration of this call.
    unsafe {
        set_header(
            name.as_ptr(),
            name.len() as i32,
            value.as_ptr(),
            value.len() as i32,
        );
    }
}

/// Simple bump allocator for host memory allocation requests.
///
/// This is a minimal allocator exported from the guest .wasm module.
/// It uses a static buffer and offset — suitable for single-threaded plugin use.
pub mod allocator {
    // Static buffer region that the host writes into via the returned pointer.
    // The buffer must be `static mut` so its address lands in the writable data
    // segment of the WASM linear memory. Accessed only via `addr_of!` (sound).
    static mut BUFFER: [u8; 65536] = [0; 65536];
    // The current bump offset. Using an AtomicUsize (rather than a raw static mut)
    // makes the offset update formally correct even though a single WASM instance
    // executes single-threaded. Relaxed ordering is sufficient because every hook
    // runs sequentially within one instance.
    static OFFSET: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

    /// Export this as "alloc" for the host to use.
    /// Returns the offset into the static buffer.
    #[unsafe(no_mangle)]
    pub extern "C" fn alloc(len: i32) -> i32 {
        let len = len as usize;
        let offset = OFFSET.load(core::sync::atomic::Ordering::Relaxed);
        let new_offset = offset + len;
        if new_offset > 65536 {
            return -1; // out of memory
        }
        // SAFETY: The pointer is returned to the host as an address into
        // WASM linear memory. The buffer is never read as a Rust value here.
        let addr = unsafe { (core::ptr::addr_of!(BUFFER) as *const u8).add(offset) as usize };
        OFFSET.store(new_offset, core::sync::atomic::Ordering::Relaxed);
        addr as i32
    }

    /// Reset the allocator (call at start of each request)
    pub fn reset() {
        OFFSET.store(0, core::sync::atomic::Ordering::Relaxed);
    }
}
