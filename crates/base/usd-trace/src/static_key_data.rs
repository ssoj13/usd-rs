//! Static key data for trace events.
//!
//! Port of pxr/base/trace/staticKeyData.h

use super::string_hash::StringHash;

/// Static key data for trace events.
///
/// This struct holds compile-time constant data for creating trace keys.
/// It's designed to be used as `const` static data in trace macros.
///
/// # Examples
///
/// ```
/// use usd_trace::StaticKeyData;
///
/// const KEY_DATA: StaticKeyData = StaticKeyData::new("my_scope");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticKeyData {
    /// Function name (optional).
    func_name: Option<&'static str>,
    /// Pretty function name with signature (optional).
    pretty_func_name: Option<&'static str>,
    /// Scope or marker name.
    name: &'static str,
    /// Cached hash of the full key string.
    hash: u32,
}

impl StaticKeyData {
    /// Creates a new static key data from a name.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::StaticKeyData;
    ///
    /// const KEY: StaticKeyData = StaticKeyData::new("my_marker");
    /// ```
    pub const fn new(name: &'static str) -> Self {
        Self {
            func_name: None,
            pretty_func_name: None,
            name,
            hash: StringHash::hash(name),
        }
    }

    /// Creates a new static key data for a function scope.
    ///
    /// # Arguments
    ///
    /// * `func` - Function name (e.g., "__FUNCTION__")
    /// * `pretty_func` - Pretty function name with signature
    /// * `scope_name` - Optional scope suffix
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::StaticKeyData;
    ///
    /// const KEY: StaticKeyData = StaticKeyData::new_function(
    ///     "my_function",
    ///     "void my_function()",
    ///     None,
    /// );
    /// ```
    pub const fn new_function(
        func: &'static str,
        pretty_func: &'static str,
        scope_name: Option<&'static str>,
    ) -> Self {
        // For hash, we use the pretty function name or regular name
        let hash = StringHash::hash(pretty_func);

        Self {
            func_name: Some(func),
            pretty_func_name: Some(pretty_func),
            name: if let Some(s) = scope_name { s } else { "" },
            hash,
        }
    }

    /// Returns the function name, if any.
    pub const fn func_name(&self) -> Option<&'static str> {
        self.func_name
    }

    /// Returns the pretty function name, if any.
    pub const fn pretty_func_name(&self) -> Option<&'static str> {
        self.pretty_func_name
    }

    /// Returns the scope/marker name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the cached hash value.
    pub const fn hash(&self) -> u32 {
        self.hash
    }

    /// Returns the full key string.
    ///
    /// This builds the complete key by combining function and scope names.
    pub fn get_string(&self) -> String {
        match (self.pretty_func_name, self.name) {
            (Some(func), name) if !name.is_empty() => {
                format!("{} [{}]", func, name)
            }
            (Some(func), _) => func.to_string(),
            (None, name) => name.to_string(),
        }
    }

    /// Returns the key as a static string, if possible.
    ///
    /// Returns `Some` only if there's no function name (simple scope/marker).
    pub const fn as_static_str(&self) -> Option<&'static str> {
        if self.func_name.is_none() && self.pretty_func_name.is_none() {
            Some(self.name)
        } else {
            None
        }
    }
}

impl Default for StaticKeyData {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_key() {
        const KEY: StaticKeyData = StaticKeyData::new("test_scope");
        assert_eq!(KEY.name(), "test_scope");
        assert_eq!(KEY.get_string(), "test_scope");
        assert!(KEY.func_name().is_none());
    }

    #[test]
    fn test_function_key() {
        const KEY: StaticKeyData =
            StaticKeyData::new_function("my_function", "void my_function(int x)", None);

        assert_eq!(KEY.func_name(), Some("my_function"));
        assert_eq!(KEY.pretty_func_name(), Some("void my_function(int x)"));
        assert_eq!(KEY.get_string(), "void my_function(int x)");
    }

    #[test]
    fn test_function_with_scope() {
        const KEY: StaticKeyData =
            StaticKeyData::new_function("process", "void process()", Some("phase1"));

        assert_eq!(KEY.get_string(), "void process() [phase1]");
    }

    #[test]
    fn test_hash_consistency() {
        const KEY1: StaticKeyData = StaticKeyData::new("test");
        const KEY2: StaticKeyData = StaticKeyData::new("test");
        assert_eq!(KEY1.hash(), KEY2.hash());
    }

    #[test]
    fn test_equality() {
        const KEY1: StaticKeyData = StaticKeyData::new("test");
        const KEY2: StaticKeyData = StaticKeyData::new("test");
        assert_eq!(KEY1, KEY2);
    }

    #[test]
    fn test_as_static_str() {
        const KEY1: StaticKeyData = StaticKeyData::new("simple");
        assert_eq!(KEY1.as_static_str(), Some("simple"));

        const KEY2: StaticKeyData = StaticKeyData::new_function("f", "f()", None);
        assert_eq!(KEY2.as_static_str(), None);
    }
}
