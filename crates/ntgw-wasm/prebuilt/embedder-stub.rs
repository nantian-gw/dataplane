// Stub embedder — returns fixed-size zero vector
// Exports: alloc, embed, get_embedding, reset

static mut BUFFER: [u8; 65536] = [0; 65536];
static OFFSET: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

// Last computed embedding (fixed 4-dimensional zero vector)
static mut EMBEDDING: [f32; 4] = [0.0; 4];
static mut EMBEDDING_DIM: i32 = 4;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    // SAFETY: The offset is only mutated by a single WASM instance.
    let offset = OFFSET.load(core::sync::atomic::Ordering::Relaxed);
    let new_offset = offset + len as usize;
    if new_offset > 65536 {
        return -1;
    }
    OFFSET.store(new_offset, core::sync::atomic::Ordering::Relaxed);
    // SAFETY: offset validated to be within BUFFER bounds by check above.
    unsafe { BUFFER.as_ptr().add(offset) as i32 }
}

#[no_mangle]
pub extern "C" fn embed(_ptr: i32, _len: i32) -> i32 {
    // Safety: single-threaded static mut read during init.
    unsafe { EMBEDDING_DIM }
}

#[no_mangle]
pub extern "C" fn get_embedding(buf_ptr: i32, buf_len: i32) {
    let dim = buf_len as usize;
    // Safety: buf_ptr and dim guaranteed valid by the host WASM runtime.
    let out = unsafe {
        core::slice::from_raw_parts_mut(buf_ptr as *mut u8, dim * 4)
    };
    // Safety: static mut slice read bounded by dim validated during init.
    let emb = unsafe { &EMBEDDING[..dim.min(4)] };
    for (i, v) in emb.iter().enumerate() {
        let bytes = v.to_le_bytes();
        out[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    // Zero-fill remaining
    for i in emb.len()..dim {
        out[i * 4..(i + 1) * 4].fill(0);
    }
}

#[no_mangle]
pub extern "C" fn reset() {
    OFFSET.store(0, core::sync::atomic::Ordering::Relaxed);
}