//! Metadata helpers for JavaScript bindings.
//!
//! These mirror the Emscripten `Metadata`, `MetadataQuerier`, and
//! `MetadataBuilder` wrappers to keep the JS API compatible.

use crate::arrays::DracoInt32Array;
use draco_core::metadata::metadata::{Metadata as CoreMetadata, MetadataString};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Metadata {
    inner: CoreMetadata,
}

impl Metadata {
    pub(crate) fn from_core(metadata: CoreMetadata) -> Self {
        Self { inner: metadata }
    }

    pub(crate) fn inner(&self) -> &CoreMetadata {
        &self.inner
    }

    pub(crate) fn inner_mut(&mut self) -> &mut CoreMetadata {
        &mut self.inner
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Metadata {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: CoreMetadata::new(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct MetadataBuilder;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl MetadataBuilder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        MetadataBuilder
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddStringEntry))]
    pub fn add_string_entry(
        &self,
        metadata: &mut Metadata,
        entry_name: &str,
        entry_value: &str,
    ) -> bool {
        metadata
            .inner_mut()
            .add_entry_string(entry_name, entry_value);
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddIntEntry))]
    pub fn add_int_entry(
        &self,
        metadata: &mut Metadata,
        entry_name: &str,
        entry_value: i32,
    ) -> bool {
        metadata.inner_mut().add_entry_int(entry_name, entry_value);
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddIntEntryArray))]
    pub fn add_int_entry_array(
        &self,
        metadata: &mut Metadata,
        entry_name: &str,
        entry_values: &[i32],
        num_values: i32,
    ) -> bool {
        let num_values = num_values.max(0) as usize;
        let count = num_values.min(entry_values.len());
        metadata
            .inner_mut()
            .add_entry_int_array(entry_name, &entry_values[..count]);
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddDoubleEntry))]
    pub fn add_double_entry(
        &self,
        metadata: &mut Metadata,
        entry_name: &str,
        entry_value: f64,
    ) -> bool {
        metadata
            .inner_mut()
            .add_entry_double(entry_name, entry_value);
        true
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct MetadataQuerier {
    entry_names_cache: Vec<String>,
    last_metadata_ptr: Option<usize>,
    last_string_returned: MetadataString,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl MetadataQuerier {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            entry_names_cache: Vec::new(),
            last_metadata_ptr: None,
            last_string_returned: MetadataString::default(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = HasEntry))]
    pub fn has_entry(&self, metadata: &Metadata, entry_name: &str) -> bool {
        metadata.inner().entries().contains_key(entry_name)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetIntEntry))]
    pub fn get_int_entry(&self, metadata: &Metadata, entry_name: &str) -> i32 {
        let mut value = 0i32;
        let _ = metadata.inner().get_entry_int(entry_name, &mut value);
        value
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetIntEntryArray))]
    pub fn get_int_entry_array(
        &self,
        metadata: &Metadata,
        entry_name: &str,
        out_values: &mut DracoInt32Array,
    ) {
        let mut values = Vec::new();
        let _ = metadata
            .inner()
            .get_entry_int_array(entry_name, &mut values);
        out_values.move_data(values);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetDoubleEntry))]
    pub fn get_double_entry(&self, metadata: &Metadata, entry_name: &str) -> f64 {
        let mut value = 0.0f64;
        let _ = metadata.inner().get_entry_double(entry_name, &mut value);
        value
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetStringEntry))]
    pub fn get_string_entry(&mut self, metadata: &Metadata, entry_name: &str) -> Option<String> {
        self.last_string_returned.clear();
        if metadata
            .inner()
            .get_entry_string(entry_name, &mut self.last_string_returned)
        {
            return Some(self.last_string_returned.to_utf8_lossy().into_owned());
        }
        None
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = NumEntries))]
    pub fn num_entries(&self, metadata: &Metadata) -> i32 {
        metadata.inner().num_entries() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetEntryName))]
    pub fn get_entry_name(&mut self, metadata: &Metadata, entry_id: i32) -> Option<String> {
        if entry_id < 0 {
            return None;
        }
        let metadata_ptr = metadata as *const Metadata as usize;
        if self.last_metadata_ptr != Some(metadata_ptr) {
            self.entry_names_cache = metadata
                .inner()
                .entries()
                .keys()
                .map(|name| name.to_utf8_lossy().into_owned())
                .collect();
            self.last_metadata_ptr = Some(metadata_ptr);
        }
        let entry_id = entry_id as usize;
        self.entry_names_cache.get(entry_id).cloned()
    }
}
