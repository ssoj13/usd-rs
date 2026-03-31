//! Status wrapper for JavaScript bindings.
//!
//! Mirrors `draco::Status` accessors used by the Emscripten WebIDL bindings.

use crate::types::status_code_to_i32;
use draco_core::core::status::Status as CoreStatus;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Status {
    inner: CoreStatus,
}

impl Status {
    pub(crate) fn from_status(status: CoreStatus) -> Self {
        Self { inner: status }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Status {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = code))]
    pub fn code(&self) -> i32 {
        status_code_to_i32(self.inner.code())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = ok))]
    pub fn ok(&self) -> bool {
        self.inner.is_ok()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = error_msg))]
    pub fn error_msg(&self) -> String {
        self.inner.error_msg().to_string()
    }
}
