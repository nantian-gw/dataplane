use crate::types::LogLevel;

/// Host function: log a message
///
/// # Safety
/// Called from the host via wasmtime linker
pub unsafe fn host_log(level: LogLevel, msg: &str) {
    #[link(wasm_import_module = "pingora")]
    unsafe extern "C" {
        fn log(level: i32, msg_ptr: *const u8, msg_len: i32);
    }
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
    unsafe extern "C" {
        fn get_header(name_ptr: *const u8, name_len: i32) -> i64;
    }
    let packed = unsafe { get_header(name.as_ptr(), name.len() as i32) };
    if packed == 0 {
        return None;
    }
    let ptr = (packed >> 32) as i32;
    let len = (packed & 0xFFFF_FFFF) as i32;
    // Read from linear memory at [ptr..ptr+len]
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
    unsafe extern "C" {
        fn set_header(name_ptr: *const u8, name_len: i32, val_ptr: *const u8, val_len: i32);
    }
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
    static mut BUFFER: [u8; 65536] = [0; 65536];
    static mut OFFSET: usize = 0;

    /// Export this as "alloc" for the host to use.
    /// Returns the offset into the static buffer.
    #[unsafe(no_mangle)]
    pub extern "C" fn alloc(len: i32) -> i32 {
        let len = len as usize;
        let offset = unsafe { OFFSET };
        let new_offset = offset + len;
        if new_offset > 65536 {
            return -1; // out of memory
        }
        // Return pointer relative to memory base (offset within BUFFER if it were at address 0).
        // In practice, wasm linear memory starts at 0, so this offset IS the address.
        // But BUFFER isn't at linear memory address 0 — we need to return the actual
        // address of BUFFER + offset in linear memory.
        let addr = unsafe { (core::ptr::addr_of!(BUFFER) as *const u8).add(offset) as usize };
        unsafe { OFFSET = new_offset };
        addr as i32
    }

    /// Reset the allocator (call at start of each request)
    pub fn reset() {
        unsafe {
            OFFSET = 0;
        }
    }
}
