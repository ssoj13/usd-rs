//! HdTaskContext - Shared state container for task communication.
//!
//! The task context is an unordered map of token-value pairs used for
//! inter-task communication during Prepare and Execute phases.
//!
//! Data in the task context is not guaranteed to persist across calls to HdEngine::Execute().

use crate::render::driver::HdDriverVector;
use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::Value;

/// Token key for drivers stored in the task context map.
/// Matches C++ `HdTokens->drivers`.
fn drivers_token() -> Token {
    Token::new("drivers")
}

/// Shared state container for inter-task communication.
///
/// Matches C++ `HdTaskContext = std::unordered_map<TfToken, VtValue>`.
/// Drivers are stored IN the map under the key "drivers", just like C++.
///
/// # Example
/// ```ignore
/// use usd_hd::render::HdTaskContext;
/// use usd_tf::Token;
/// use usd_vt::Value;
///
/// let mut ctx = HdTaskContext::new();
///
/// // One task stores camera matrices
/// ctx.insert(Token::new("viewMatrix"), Value::from(view_matrix));
/// ctx.insert(Token::new("projMatrix"), Value::from(proj_matrix));
///
/// // Another task retrieves them
/// if let Some(view) = ctx.get(&Token::new("viewMatrix")) {
///     // Use view matrix for rendering
/// }
/// ```
#[derive(Debug, Default, Clone)]
pub struct HdTaskContext {
    data: HashMap<Token, Value>,
}

impl HdTaskContext {
    /// Create an empty task context.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert or update a value in the context.
    pub fn insert(&mut self, key: Token, value: Value) -> Option<Value> {
        self.data.insert(key, value)
    }

    /// Get a value from the context.
    pub fn get(&self, key: &Token) -> Option<&Value> {
        self.data.get(key)
    }

    /// Get a mutable reference to a value.
    pub fn get_mut(&mut self, key: &Token) -> Option<&mut Value> {
        self.data.get_mut(key)
    }

    /// Remove a value from the context.
    pub fn remove(&mut self, key: &Token) -> Option<Value> {
        self.data.remove(key)
    }

    /// Check if a key exists in the context.
    pub fn contains_key(&self, key: &Token) -> bool {
        self.data.contains_key(key)
    }

    /// Clear all entries from the context.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get number of entries in the context.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if context is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get an iterator over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&Token, &Value)> {
        self.data.iter()
    }

    /// Get an iterator over keys.
    pub fn keys(&self) -> impl Iterator<Item = &Token> {
        self.data.keys()
    }

    /// Get an iterator over values.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.data.values()
    }

    /// Set the driver vector in the map (injected by HdEngine before Execute).
    ///
    /// Matches C++: `_taskContext[HdTokens->drivers] = VtValue(index->GetDrivers())`.
    pub fn set_drivers(&mut self, drivers: HdDriverVector) {
        self.data.insert(drivers_token(), Value::new(drivers));
    }

    /// Get the driver vector from the map.
    ///
    /// Matches C++ `HdTask::_GetDriver` which reads from `ctx->find(HdTokens->drivers)`.
    pub fn get_drivers(&self) -> Option<&HdDriverVector> {
        self.data
            .get(&drivers_token())
            .and_then(|v| v.get::<HdDriverVector>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_context_basic() {
        let mut ctx = HdTaskContext::new();
        assert!(ctx.is_empty());

        let key = Token::new("testKey");
        let value = Value::from(42i32);

        ctx.insert(key.clone(), value);
        assert_eq!(ctx.len(), 1);
        assert!(ctx.contains_key(&key));

        let retrieved = ctx.get(&key);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_task_context_update() {
        let mut ctx = HdTaskContext::new();
        let key = Token::new("counter");

        ctx.insert(key.clone(), Value::from(1i32));
        ctx.insert(key.clone(), Value::from(2i32));

        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_task_context_remove() {
        let mut ctx = HdTaskContext::new();
        let key = Token::new("temp");

        ctx.insert(key.clone(), Value::from(100i32));
        assert!(ctx.contains_key(&key));

        let removed = ctx.remove(&key);
        assert!(removed.is_some());
        assert!(!ctx.contains_key(&key));
    }

    #[test]
    fn test_task_context_clear() {
        let mut ctx = HdTaskContext::new();

        ctx.insert(Token::new("key1"), Value::from(1i32));
        ctx.insert(Token::new("key2"), Value::from(2i32));
        assert_eq!(ctx.len(), 2);

        ctx.clear();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_task_context_iteration() {
        let mut ctx = HdTaskContext::new();

        ctx.insert(Token::new("a"), Value::from(1i32));
        ctx.insert(Token::new("b"), Value::from(2i32));
        ctx.insert(Token::new("c"), Value::from(3i32));

        let count = ctx.iter().count();
        assert_eq!(count, 3);

        let keys: Vec<_> = ctx.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys.len(), 3);
    }
}
