//! Trace keys for identifying events.
//!
//! Keys are used to identify and group trace events. There are two types:
//! - [`StaticKey`] - Known at compile time, zero overhead
//! - [`DynamicKey`] - Created at runtime, some allocation overhead

use std::borrow::Cow;
use std::hash::{Hash, Hasher};

/// A trait for trace keys.
pub trait Key {
    /// Returns the key as a string slice.
    fn as_str(&self) -> &str;

    /// Returns a hash of the key.
    fn hash_value(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.as_str().hash(&mut hasher);
        hasher.finish()
    }
}

/// A static trace key known at compile time.
///
/// Static keys have zero runtime overhead for the key itself since
/// the string is stored in the binary.
///
/// # Examples
///
/// ```
/// use usd_trace::StaticKey;
///
/// const MY_KEY: StaticKey = StaticKey::new("my_operation");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticKey {
    /// The key name.
    name: &'static str,
    /// Optional pretty name for display.
    pretty_name: Option<&'static str>,
}

impl StaticKey {
    /// Creates a new static key.
    ///
    /// # Arguments
    ///
    /// * `name` - The key name
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            pretty_name: None,
        }
    }

    /// Creates a new static key with a pretty name.
    ///
    /// # Arguments
    ///
    /// * `name` - The key name
    /// * `pretty_name` - A human-readable display name
    pub const fn with_pretty_name(name: &'static str, pretty_name: &'static str) -> Self {
        Self {
            name,
            pretty_name: Some(pretty_name),
        }
    }

    /// Returns the key name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the pretty name, or the key name if not set.
    pub const fn pretty_name(&self) -> &'static str {
        match self.pretty_name {
            Some(p) => p,
            None => self.name,
        }
    }
}

impl Key for StaticKey {
    fn as_str(&self) -> &str {
        self.name
    }
}

impl std::fmt::Display for StaticKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pretty_name())
    }
}

/// A dynamic trace key created at runtime.
///
/// Dynamic keys are useful when the key name isn't known until runtime,
/// such as keys based on parameter values.
///
/// # Examples
///
/// ```
/// use usd_trace::DynamicKey;
///
/// fn trace_file_operation(filename: &str) {
///     let key = DynamicKey::new(format!("read_file:{}", filename));
///     // Use key for tracing...
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DynamicKey {
    /// The key name.
    name: Cow<'static, str>,
}

impl DynamicKey {
    /// Creates a new dynamic key from an owned string.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Cow::Owned(name.into()),
        }
    }

    /// Creates a new dynamic key from a static string.
    ///
    /// This avoids allocation when the string is static.
    pub const fn from_static(name: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
        }
    }

    /// Returns the key name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Key for DynamicKey {
    fn as_str(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for DynamicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<&'static str> for DynamicKey {
    fn from(s: &'static str) -> Self {
        Self::from_static(s)
    }
}

impl From<String> for DynamicKey {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Creates a static key at compile time.
///
/// # Examples
///
/// ```
/// use usd_trace::static_key;
///
/// static_key!(MY_OPERATION);
/// // Creates: const MY_OPERATION: StaticKey = StaticKey::new("MY_OPERATION");
/// ```
#[macro_export]
macro_rules! static_key {
    ($name:ident) => {
        const $name: $crate::StaticKey = $crate::StaticKey::new(stringify!($name));
    };
    ($name:ident, $display:expr) => {
        const $name: $crate::StaticKey =
            $crate::StaticKey::with_pretty_name(stringify!($name), $display);
    };
}

pub use static_key;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_key() {
        const KEY: StaticKey = StaticKey::new("test_key");
        assert_eq!(KEY.name(), "test_key");
        assert_eq!(KEY.pretty_name(), "test_key");
        assert_eq!(KEY.as_str(), "test_key");
    }

    #[test]
    fn test_static_key_with_pretty_name() {
        const KEY: StaticKey = StaticKey::with_pretty_name("internal_name", "Display Name");
        assert_eq!(KEY.name(), "internal_name");
        assert_eq!(KEY.pretty_name(), "Display Name");
    }

    #[test]
    fn test_dynamic_key() {
        let key = DynamicKey::new("dynamic_key".to_string());
        assert_eq!(key.name(), "dynamic_key");
        assert_eq!(key.as_str(), "dynamic_key");
    }

    #[test]
    fn test_dynamic_key_from_static() {
        let key = DynamicKey::from_static("static_string");
        assert_eq!(key.name(), "static_string");
    }

    #[test]
    fn test_dynamic_key_from_string() {
        let key: DynamicKey = "test".into();
        assert_eq!(key.name(), "test");

        let key: DynamicKey = String::from("test2").into();
        assert_eq!(key.name(), "test2");
    }

    #[test]
    fn test_key_hash() {
        let key1 = StaticKey::new("same");
        let key2 = DynamicKey::from_static("same");
        assert_eq!(key1.hash_value(), key2.hash_value());
    }

    #[test]
    fn test_static_key_macro() {
        static_key!(TEST_KEY);
        assert_eq!(TEST_KEY.name(), "TEST_KEY");
    }

    #[test]
    fn test_static_key_macro_with_display() {
        static_key!(INTERNAL_KEY, "User Friendly Name");
        assert_eq!(INTERNAL_KEY.name(), "INTERNAL_KEY");
        assert_eq!(INTERNAL_KEY.pretty_name(), "User Friendly Name");
    }
}
