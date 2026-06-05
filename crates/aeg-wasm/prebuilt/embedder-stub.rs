// Stub embedder — returns fixed-size zero vector
// Exports: alloc, embed, get_embedding, reset

static mut BUFFER: [u8; 65536] = [0; 65536];
static mut OFFSET: usize = 0;

// Last computed embedding (fixed 4-dimensional zero vector)
static mut EMBEDDING: [f32; 4] = [0.0; 4];
static mut EMBEDDING_DIM: i32 = 4;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    let offset = unsafe { OFFSET };
    let new_offset = offset + len as usize;
    if new_offset > 65536 {
        return -1;
    }
    unsafe { OFFSET = new_offset; }
    unsafe { BUFFER.as_ptr().add(offset) as i32 }
}

#[no_mangle]
pub extern "C" fn embed(_ptr: i32, _len: i32) -> i32 {
    unsafe { EMBEDDING_DIM }
}

#[no_mangle]
pub extern "C" fn get_embedding(buf_ptr: i32, buf_len: i32) {
    let dim = buf_len as usize;
    let out = unsafe {
        core::slice::from_raw_parts_mut(buf_ptr as *mut u8, dim * 4)
    };
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
    unsafe { OFFSET = 0; }
}