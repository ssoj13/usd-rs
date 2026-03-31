//! Generic per-attribute options for encoding/decoding.
//! Reference: `_ref/draco/src/draco/compression/config/draco_options.h`.

use std::collections::BTreeMap;

use draco_core::core::options::Options;

/// Base option class used to control encoding and decoding.
#[derive(Clone, Debug)]
pub struct DracoOptions<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    global_options: Options,
    attribute_options: BTreeMap<AttributeKeyT, Options>,
}

impl<AttributeKeyT> Default for DracoOptions<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    fn default() -> Self {
        Self {
            global_options: Options::new(),
            attribute_options: BTreeMap::new(),
        }
    }
}

impl<AttributeKeyT> DracoOptions<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_attribute_int(&self, att_key: &AttributeKeyT, name: &str, default_val: i32) -> i32 {
        if let Some(att_options) = self.attribute_options.get(att_key) {
            if att_options.is_option_set(name) {
                return att_options.get_int_or(name, default_val);
            }
        }
        self.global_options.get_int_or(name, default_val)
    }

    pub fn set_attribute_int(&mut self, att_key: &AttributeKeyT, name: &str, val: i32) {
        self.get_attribute_options(att_key).set_int(name, val);
    }

    pub fn get_attribute_float(
        &self,
        att_key: &AttributeKeyT,
        name: &str,
        default_val: f32,
    ) -> f32 {
        if let Some(att_options) = self.attribute_options.get(att_key) {
            if att_options.is_option_set(name) {
                return att_options.get_float_or(name, default_val);
            }
        }
        self.global_options.get_float_or(name, default_val)
    }

    pub fn set_attribute_float(&mut self, att_key: &AttributeKeyT, name: &str, val: f32) {
        self.get_attribute_options(att_key).set_float(name, val);
    }

    pub fn get_attribute_bool(
        &self,
        att_key: &AttributeKeyT,
        name: &str,
        default_val: bool,
    ) -> bool {
        if let Some(att_options) = self.attribute_options.get(att_key) {
            if att_options.is_option_set(name) {
                return att_options.get_bool_or(name, default_val);
            }
        }
        self.global_options.get_bool_or(name, default_val)
    }

    pub fn set_attribute_bool(&mut self, att_key: &AttributeKeyT, name: &str, val: bool) {
        self.get_attribute_options(att_key).set_bool(name, val);
    }

    pub fn get_attribute_vector<T: std::str::FromStr + Copy>(
        &self,
        att_key: &AttributeKeyT,
        name: &str,
        num_dims: i32,
        val: &mut [T],
    ) -> bool {
        if num_dims < 0 {
            return false;
        }
        if let Some(att_options) = self.attribute_options.get(att_key) {
            if att_options.is_option_set(name) {
                return att_options.get_vector(name, val);
            }
        }
        self.global_options.get_vector(name, val)
    }

    pub fn set_attribute_vector<T: std::fmt::Display>(
        &mut self,
        att_key: &AttributeKeyT,
        name: &str,
        num_dims: i32,
        val: &[T],
    ) {
        let _ = num_dims; // kept for API parity with reference
        self.get_attribute_options(att_key).set_vector(name, val);
    }

    pub fn is_attribute_option_set(&self, att_key: &AttributeKeyT, name: &str) -> bool {
        if let Some(att_options) = self.attribute_options.get(att_key) {
            return att_options.is_option_set(name);
        }
        self.global_options.is_option_set(name)
    }

    pub fn get_global_int(&self, name: &str, default_val: i32) -> i32 {
        self.global_options.get_int_or(name, default_val)
    }

    pub fn set_global_int(&mut self, name: &str, val: i32) {
        self.global_options.set_int(name, val);
    }

    pub fn get_global_float(&self, name: &str, default_val: f32) -> f32 {
        self.global_options.get_float_or(name, default_val)
    }

    pub fn set_global_float(&mut self, name: &str, val: f32) {
        self.global_options.set_float(name, val);
    }

    pub fn get_global_bool(&self, name: &str, default_val: bool) -> bool {
        self.global_options.get_bool_or(name, default_val)
    }

    pub fn set_global_bool(&mut self, name: &str, val: bool) {
        self.global_options.set_bool(name, val);
    }

    pub fn get_global_vector<T: std::str::FromStr + Copy>(
        &self,
        name: &str,
        _num_dims: i32,
        val: &mut [T],
    ) -> bool {
        self.global_options.get_vector(name, val)
    }

    pub fn set_global_vector<T: std::fmt::Display>(
        &mut self,
        name: &str,
        _num_dims: i32,
        val: &[T],
    ) {
        self.global_options.set_vector(name, val);
    }

    pub fn is_global_option_set(&self, name: &str) -> bool {
        self.global_options.is_option_set(name)
    }

    pub fn set_attribute_options(&mut self, att_key: &AttributeKeyT, options: Options) {
        self.attribute_options.insert(att_key.clone(), options);
    }

    pub fn set_global_options(&mut self, options: Options) {
        self.global_options = options;
    }

    pub fn find_attribute_options(&self, att_key: &AttributeKeyT) -> Option<&Options> {
        self.attribute_options.get(att_key)
    }

    pub fn global_options(&self) -> &Options {
        &self.global_options
    }

    fn get_attribute_options(&mut self, att_key: &AttributeKeyT) -> &mut Options {
        if !self.attribute_options.contains_key(att_key) {
            self.attribute_options
                .insert(att_key.clone(), Options::new());
        }
        self.attribute_options
            .get_mut(att_key)
            .expect("attribute options")
    }
}
