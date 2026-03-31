//! Options utilities.
//! Reference: `_ref/draco/src/draco/core/options.h` + `.cc`.

use std::collections::BTreeMap;
use std::ffi::c_char;

#[derive(Clone, Debug, Default)]
pub struct Options {
    options: BTreeMap<String, String>,
}

fn with_c_string_terminator<R>(value: &str, f: impl FnOnce(*const c_char) -> R) -> R {
    let bytes = value
        .as_bytes()
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or_default();
    let mut c_value = Vec::with_capacity(bytes.len() + 1);
    c_value.extend_from_slice(bytes);
    c_value.push(0);
    f(c_value.as_ptr().cast())
}

fn parse_int_like_c(value: &str) -> i32 {
    with_c_string_terminator(value, |c_value| unsafe { libc::atoi(c_value) })
}

fn parse_float_like_c(value: &str) -> f32 {
    with_c_string_terminator(value, |c_value| unsafe { libc::atof(c_value) as f32 })
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge_and_replace(&mut self, other: &Options) {
        for (k, v) in other.options.iter() {
            self.options.insert(k.clone(), v.clone());
        }
    }

    pub fn set_int(&mut self, name: &str, val: i32) {
        self.options.insert(name.to_string(), val.to_string());
    }

    pub fn set_float(&mut self, name: &str, val: f32) {
        self.options.insert(name.to_string(), val.to_string());
    }

    pub fn set_bool(&mut self, name: &str, val: bool) {
        self.options
            .insert(name.to_string(), if val { "1" } else { "0" }.to_string());
    }

    pub fn set_string(&mut self, name: &str, val: &str) {
        self.options.insert(name.to_string(), val.to_string());
    }

    pub fn set_vector<T: std::fmt::Display>(&mut self, name: &str, vec: &[T]) {
        let mut out = String::new();
        for (i, v) in vec.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&format!("{}", v));
        }
        self.options.insert(name.to_string(), out);
    }

    pub fn get_int(&self, name: &str) -> i32 {
        self.get_int_or(name, -1)
    }

    pub fn get_int_or(&self, name: &str, default_val: i32) -> i32 {
        match self.options.get(name) {
            Some(v) => parse_int_like_c(v),
            None => default_val,
        }
    }

    pub fn get_float(&self, name: &str) -> f32 {
        self.get_float_or(name, -1.0)
    }

    pub fn get_float_or(&self, name: &str, default_val: f32) -> f32 {
        match self.options.get(name) {
            Some(v) => parse_float_like_c(v),
            None => default_val,
        }
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.get_bool_or(name, false)
    }

    pub fn get_bool_or(&self, name: &str, default_val: bool) -> bool {
        let ret = self.get_int_or(name, -1);
        if ret == -1 {
            return default_val;
        }
        ret != 0
    }

    pub fn get_string(&self, name: &str) -> String {
        self.get_string_or(name, "")
    }

    pub fn get_string_or(&self, name: &str, default_val: &str) -> String {
        match self.options.get(name) {
            Some(v) => v.clone(),
            None => default_val.to_string(),
        }
    }

    pub fn get_vector<T: std::str::FromStr + Copy>(&self, name: &str, out_val: &mut [T]) -> bool {
        let Some(value) = self.options.get(name) else {
            return false;
        };
        if value.is_empty() {
            return true;
        }
        let mut idx = 0;
        for token in value.split_whitespace() {
            if idx >= out_val.len() {
                break;
            }
            if let Ok(val) = token.parse::<T>() {
                out_val[idx] = val;
                idx += 1;
            } else {
                break;
            }
        }
        true
    }

    pub fn is_option_set(&self, name: &str) -> bool {
        self.options.contains_key(name)
    }
}
