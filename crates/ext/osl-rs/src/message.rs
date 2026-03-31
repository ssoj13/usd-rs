//! Message passing — setmessage/getmessage runtime operations.
//!
//! Port of `opmessage.cpp`. Provides per-shading-point message storage
//! that shaders can use to communicate via `setmessage()` / `getmessage()`.
//! Supports layer validation (plan #45): messages may only be transferred
//! from layers that appear earlier in the shading network.

use std::collections::HashMap;

use crate::Float;
use crate::math::{Color3, Matrix44, Vec3};
use crate::ustring::UString;

/// A stored message value.
#[derive(Debug, Clone)]
pub enum MessageValue {
    Int(i32),
    Float(Float),
    String(UString),
    Color(Color3),
    Point(Vec3),
    Vector(Vec3),
    Normal(Vec3),
    Matrix(Matrix44),
    IntArray(Vec<i32>),
    FloatArray(Vec<Float>),
    StringArray(Vec<UString>),
    ColorArray(Vec<Color3>),
    PointArray(Vec<Vec3>),
    VectorArray(Vec<Vec3>),
    NormalArray(Vec<Vec3>),
    MatrixArray(Vec<Matrix44>),
}

/// Internal entry for validated messages (layeridx, queried-before-set).
#[derive(Debug, Clone)]
enum MessageEntry {
    Set(i32),
    Queried(i32),
}

/// Per-shading-point message store.
#[derive(Debug, Clone, Default)]
pub struct MessageStore {
    messages: HashMap<UString, MessageValue>,
    /// Track which messages have been set (legacy/simple path).
    set_flags: HashMap<UString, bool>,
    /// Validated entries (layeridx, Set vs Queried). When present, used for validation.
    validated: HashMap<UString, MessageEntry>,
}

impl MessageStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a message value (simple path, no validation).
    pub fn setmessage(&mut self, name: UString, value: MessageValue) {
        self.messages.insert(name.clone(), value);
        self.set_flags.insert(name, true);
    }

    /// Set with layer validation. Returns error string if validation fails.
    pub fn setmessage_validated(
        &mut self,
        name: UString,
        value: MessageValue,
        layeridx: i32,
        on_error: &mut impl FnMut(&str),
    ) {
        if let Some(entry) = self.validated.get(&name) {
            match entry {
                MessageEntry::Set(prev_layer) => {
                    on_error(&format!(
                        "message \"{}\" already exists (layer {}), cannot set again from layer {}",
                        name, prev_layer, layeridx
                    ));
                    return;
                }
                MessageEntry::Queried(prev_layer) => {
                    on_error(&format!(
                        "message \"{}\" was queried before being set (layer {}), \
                         setting from layer {} would lead to inconsistent results",
                        name, prev_layer, layeridx
                    ));
                    return;
                }
            }
        }
        self.validated
            .insert(name.clone(), MessageEntry::Set(layeridx));
        self.messages.insert(name.clone(), value);
        self.set_flags.insert(name, true);
    }

    /// Get a message value from the given source.
    /// `source` can be "trace" or empty (from the same shading point).
    pub fn getmessage(&self, source: &str, name: UString) -> Option<&MessageValue> {
        let _ = source;
        self.messages.get(&name)
    }

    /// Get with layer validation. Returns cloned value if found. On validation error, calls on_error.
    pub fn getmessage_validated(
        &mut self,
        name: UString,
        layeridx: i32,
        strict: bool,
        on_error: &mut impl FnMut(&str),
    ) -> Option<MessageValue> {
        if let Some(entry) = self.validated.get(&name) {
            match entry {
                MessageEntry::Set(msg_layeridx) => {
                    if *msg_layeridx > layeridx {
                        on_error(&format!(
                            "message \"{}\" was set by layer #{} but is being queried by layer #{} - \
                             messages may only be transferred from nodes that appear earlier",
                            name, msg_layeridx, layeridx
                        ));
                        return None;
                    }
                    return self.messages.get(&name).cloned();
                }
                MessageEntry::Queried(_) => return None,
            }
        }
        if strict {
            self.validated
                .insert(name.clone(), MessageEntry::Queried(layeridx));
        }
        None
    }

    /// Check if a message has been set.
    pub fn has_message(&self, name: UString) -> bool {
        self.set_flags.get(&name).copied().unwrap_or(false)
    }

    /// Clear all messages (done between shading points).
    pub fn clear(&mut self) {
        self.messages.clear();
        self.set_flags.clear();
        self.validated.clear();
    }

    /// Get message count.
    pub fn count(&self) -> usize {
        self.messages.len()
    }
}

/// Helper: set an int message.
pub fn setmessage_int(store: &mut MessageStore, name: UString, val: i32) {
    store.setmessage(name, MessageValue::Int(val));
}

/// Helper: set a float message.
pub fn setmessage_float(store: &mut MessageStore, name: UString, val: Float) {
    store.setmessage(name, MessageValue::Float(val));
}

/// Helper: set a string message.
pub fn setmessage_string(store: &mut MessageStore, name: UString, val: UString) {
    store.setmessage(name, MessageValue::String(val));
}

/// Helper: set a color message.
pub fn setmessage_color(store: &mut MessageStore, name: UString, val: Color3) {
    store.setmessage(name, MessageValue::Color(val));
}

/// Helper: get an int message.
pub fn getmessage_int(store: &MessageStore, source: &str, name: UString) -> Option<i32> {
    match store.getmessage(source, name)? {
        MessageValue::Int(v) => Some(*v),
        MessageValue::Float(v) => Some(*v as i32),
        _ => None,
    }
}

/// Helper: get a float message.
pub fn getmessage_float(store: &MessageStore, source: &str, name: UString) -> Option<Float> {
    match store.getmessage(source, name)? {
        MessageValue::Float(v) => Some(*v),
        MessageValue::Int(v) => Some(*v as Float),
        _ => None,
    }
}

/// Helper: get a string message.
pub fn getmessage_string(store: &MessageStore, source: &str, name: UString) -> Option<UString> {
    match store.getmessage(source, name)? {
        MessageValue::String(v) => Some(*v),
        _ => None,
    }
}

/// Helper: get a color message.
pub fn getmessage_color(store: &MessageStore, source: &str, name: UString) -> Option<Color3> {
    match store.getmessage(source, name)? {
        MessageValue::Color(v) => Some(*v),
        MessageValue::Point(v) | MessageValue::Vector(v) | MessageValue::Normal(v) => Some(*v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setmessage_getmessage() {
        let mut store = MessageStore::new();
        let name = UString::new("opacity");
        setmessage_float(&mut store, name, 0.75);

        assert!(store.has_message(name));
        let v = getmessage_float(&store, "", name);
        assert_eq!(v, Some(0.75));
    }

    #[test]
    fn test_string_message() {
        let mut store = MessageStore::new();
        let name = UString::new("label");
        let val = UString::new("diffuse");
        setmessage_string(&mut store, name, val);

        let result = getmessage_string(&store, "", name);
        assert_eq!(result, Some(val));
    }

    #[test]
    fn test_missing_message() {
        let store = MessageStore::new();
        let name = UString::new("nonexistent");
        assert!(!store.has_message(name));
        assert_eq!(getmessage_float(&store, "", name), None);
    }

    #[test]
    fn test_clear() {
        let mut store = MessageStore::new();
        let name = UString::new("test");
        setmessage_int(&mut store, name, 42);
        assert_eq!(store.count(), 1);
        store.clear();
        assert_eq!(store.count(), 0);
        assert!(!store.has_message(name));
    }

    #[test]
    fn test_color_message() {
        let mut store = MessageStore::new();
        let name = UString::new("albedo");
        setmessage_color(&mut store, name, Color3::new(0.5, 0.3, 0.1));

        let v = getmessage_color(&store, "", name).unwrap();
        assert!((v.x - 0.5).abs() < 1e-6);
        assert!((v.y - 0.3).abs() < 1e-6);
        assert!((v.z - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_overwrite_message() {
        let mut store = MessageStore::new();
        let name = UString::new("val");
        setmessage_int(&mut store, name, 1);
        setmessage_int(&mut store, name, 2);
        assert_eq!(getmessage_int(&store, "", name), Some(2));
    }
}
