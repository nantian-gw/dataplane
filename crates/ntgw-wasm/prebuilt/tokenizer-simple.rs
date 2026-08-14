// Simple word-based tokenizer for testing
// Exports: alloc, tokenize, reset
// tokenize(ptr, len) returns word count

static mut BUFFER: [u8; 65536] = [0; 65536];
static mut OFFSET: usize = 0;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    // Safety: single-threaded static mut read during init.
    let offset = unsafe { OFFSET };
    let new_offset = offset + len as usize;
    if new_offset > 65536 {
        return -1;
    }
    // Safety: single-threaded static mut write during init.
    unsafe { OFFSET = new_offset; }
    // Safety: offset validated by init to be within BUFFER bounds.
    unsafe { BUFFER.as_ptr().add(offset) as i32 }
}

/// Count alphanumeric words from memory at [ptr..ptr+len]
#[no_mangle]
pub extern "C" fn tokenize(ptr: i32, len: i32) -> i32 {
    // Safety: ptr and len guaranteed valid by the host WASM runtime.
    let data = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };
    let text = match core::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut count = 0;
    let mut in_word = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    count
}

#[no_mangle]
pub extern "C" fn reset() {
    OFFSET.store(0, core::sync::atomic::Ordering::Relaxed);
}