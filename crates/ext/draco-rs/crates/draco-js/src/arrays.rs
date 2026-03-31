//! Draco typed array helpers exposed to JavaScript.
//!
//! These mirror the small DracoArray wrappers used by the reference
//! Emscripten bindings, but are backed by Rust `Vec<T>` storage.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

macro_rules! impl_draco_array {
    ($name:ident, $ty:ty) => {
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
        pub struct $name {
            values: Vec<$ty>,
        }

        #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
        #[allow(dead_code)] // JS bindings are only used on wasm32 targets.
        impl $name {
            #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
            pub fn new() -> Self {
                Self { values: Vec::new() }
            }

            #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetValue))]
            pub fn get_value(&self, index: usize) -> $ty {
                self.values[index]
            }

            #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = size))]
            pub fn size(&self) -> usize {
                self.values.len()
            }
        }

        #[allow(dead_code)] // Internal helpers used by wasm bindings and tests.
        impl $name {
            pub(crate) fn resize(&mut self, size: usize) {
                self.values.resize(size, <$ty>::default());
            }

            pub(crate) fn set_value(&mut self, index: usize, value: $ty) {
                self.values[index] = value;
            }

            pub(crate) fn move_data(&mut self, values: Vec<$ty>) {
                self.values = values;
            }

            pub(crate) fn set_values(&mut self, values: &[$ty]) {
                self.values.clear();
                self.values.extend_from_slice(values);
            }

            pub(crate) fn as_slice(&self) -> &[$ty] {
                &self.values
            }
        }
    };
}

impl_draco_array!(DracoFloat32Array, f32);
impl_draco_array!(DracoInt8Array, i8);
impl_draco_array!(DracoUInt8Array, u8);
impl_draco_array!(DracoInt16Array, i16);
impl_draco_array!(DracoUInt16Array, u16);
impl_draco_array!(DracoInt32Array, i32);
impl_draco_array!(DracoUInt32Array, u32);
