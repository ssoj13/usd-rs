//! Buffer and allocator helpers for JavaScript bindings.
//!
//! Includes `DecoderBuffer` (deprecated Draco API) and `_malloc`/`_free`
//! for pointer-style APIs mirrored from the Emscripten build.

use std::alloc::{alloc, dealloc, Layout};

use draco_core::core::decoder_buffer::DecoderBuffer as CoreDecoderBuffer;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct DecoderBuffer {
    data: Vec<u8>,
    position: usize,
    bitstream_version: u16,
}

impl DecoderBuffer {
    pub(crate) fn with_core_buffer<R>(&mut self, f: impl FnOnce(&mut CoreDecoderBuffer) -> R) -> R {
        let mut buffer = CoreDecoderBuffer::new();
        buffer.set_bitstream_version(self.bitstream_version);
        buffer.init_with_version(&self.data, self.bitstream_version);
        buffer.start_decoding_from(self.position as i64);
        let result = f(&mut buffer);
        self.position = buffer.position();
        self.bitstream_version = buffer.bitstream_version();
        result
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl DecoderBuffer {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            position: 0,
            bitstream_version: 0,
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = Init))]
    pub fn init(&mut self, data: &[u8], data_size: usize) {
        let count = data_size.min(data.len());
        self.data.clear();
        self.data.extend_from_slice(&data[..count]);
        self.position = 0;
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = _malloc))]
pub fn draco_malloc(size: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }
    // Store allocation size just before the returned pointer so `_free` can
    // reclaim the block without an explicit size parameter.
    let header = std::mem::size_of::<usize>();
    let total = size + header;
    let layout = Layout::from_size_align(total, std::mem::align_of::<usize>()).unwrap();
    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            return ptr;
        }
        (ptr as *mut usize).write(size);
        ptr.add(header)
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = _free))]
pub fn draco_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let header = std::mem::size_of::<usize>();
    unsafe {
        let base = ptr.sub(header);
        let size = (base as *const usize).read();
        let total = size + header;
        let layout = Layout::from_size_align(total, std::mem::align_of::<usize>()).unwrap();
        dealloc(base, layout);
    }
}
