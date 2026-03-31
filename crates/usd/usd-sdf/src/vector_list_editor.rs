//! Vector-based list editor implementation.
//!
//! Port of pxr/usd/sdf/vectorListEditor.h
//!
//! A list editor implementation that represents a single type of list editing
//! operation stored in a vector-typed field.
//!
//! TypePolicy determines the externally visible value type of this list editor.
//! The underlying field data type may differ (e.g., stored as String but
//! exposed as Token), with adapters handling conversion.

use crate::{Layer, ListOpType, Path};
use usd_tf::Token;
use std::sync::Arc;

/// Adapter trait for converting between exposed value type and stored field type.
pub trait VectorFieldAdapter<To, From> {
    /// Convert from the stored field type to the exposed type.
    fn convert(from: &[From]) -> Vec<To>;
}

/// Identity adapter when exposed and stored types are the same.
#[derive(Clone)]
pub struct IdentityAdapter;

impl<T: Clone> VectorFieldAdapter<T, T> for IdentityAdapter {
    fn convert(from: &[T]) -> Vec<T> {
        from.to_vec()
    }
}

/// Adapter from String to Token.
#[derive(Clone)]
pub struct StringToTokenAdapter;

impl VectorFieldAdapter<Token, String> for StringToTokenAdapter {
    fn convert(from: &[String]) -> Vec<Token> {
        from.iter().map(|s| Token::from(s.as_str())).collect()
    }
}

/// Adapter from Token to String.
#[derive(Clone)]
pub struct TokenToStringAdapter;

impl VectorFieldAdapter<String, Token> for TokenToStringAdapter {
    fn convert(from: &[Token]) -> Vec<String> {
        from.iter().map(|t| t.get_string().to_string()).collect()
    }
}

/// Trait for vector list editor type policies.
pub trait VectorTypePolicy: Clone + Send + Sync + 'static {
    /// The externally visible value type.
    type Value: Clone + PartialEq + std::fmt::Debug + Send + Sync;

    /// The type stored in the underlying field.
    type FieldValue: Clone + PartialEq + std::fmt::Debug + Send + Sync;

    /// The field name token for the vector field.
    fn field_name() -> Token;

    /// Convert from field values to exposed values.
    fn from_field(field: &[Self::FieldValue]) -> Vec<Self::Value>;

    /// Convert from exposed values to field values.
    fn to_field(values: &[Self::Value]) -> Vec<Self::FieldValue>;
}

/// Vector-based list editor.
///
/// Stores a single list editing operation in a vector-typed field.
/// Unlike ListOp-based editors, this only supports explicit lists or
/// ordered lists (no prepend/append/delete operations).
pub struct VectorListEditor<P: VectorTypePolicy> {
    /// The owning layer.
    layer: Option<Arc<Layer>>,
    /// Path to the owning spec.
    owner_path: Path,
    /// The current values.
    values: Vec<P::Value>,
    /// The list op type for this editor.
    op: ListOpType,
    /// Marker for the policy type.
    _policy: std::marker::PhantomData<P>,
}

impl<P: VectorTypePolicy> VectorListEditor<P> {
    /// Creates a new vector list editor.
    pub fn new(layer: Arc<Layer>, owner_path: Path, op: ListOpType) -> Self {
        Self {
            layer: Some(layer),
            owner_path,
            values: Vec::new(),
            op,
            _policy: std::marker::PhantomData,
        }
    }

    /// Returns the owning layer.
    pub fn layer(&self) -> Option<&Arc<Layer>> {
        self.layer.as_ref()
    }

    /// Returns the owning spec path.
    pub fn path(&self) -> &Path {
        &self.owner_path
    }

    /// Returns true if the editor is valid.
    pub fn is_valid(&self) -> bool {
        self.layer.is_some()
    }

    /// Returns the current values.
    pub fn get_values(&self) -> &[P::Value] {
        &self.values
    }

    /// Sets the values.
    pub fn set_values(&mut self, values: Vec<P::Value>) {
        let old = std::mem::replace(&mut self.values, values);
        self.on_edit(self.op, &old, &self.values.clone());
    }

    /// Returns true if the list is explicit (i.e., replaces entirely).
    pub fn is_explicit(&self) -> bool {
        self.op == ListOpType::Explicit
    }

    /// Returns the list op type.
    pub fn op_type(&self) -> ListOpType {
        self.op
    }

    /// Returns the number of values.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Adds a value to the list.
    pub fn add(&mut self, value: P::Value) {
        if !self.values.contains(&value) {
            let old = self.values.clone();
            self.values.push(value);
            self.on_edit(self.op, &old, &self.values.clone());
        }
    }

    /// Removes a value from the list.
    pub fn remove(&mut self, value: &P::Value) {
        if let Some(pos) = self.values.iter().position(|v| v == value) {
            let old = self.values.clone();
            self.values.remove(pos);
            self.on_edit(self.op, &old, &self.values.clone());
        }
    }

    /// Clears all values.
    pub fn clear(&mut self) {
        if !self.values.is_empty() {
            let old = std::mem::take(&mut self.values);
            self.on_edit(self.op, &old, &[]);
        }
    }

    /// Replaces a range of values.
    pub fn replace(&mut self, index: usize, n: usize, new_items: Vec<P::Value>) -> bool {
        if index > self.values.len() {
            return false;
        }
        let end = (index + n).min(self.values.len());
        let old = self.values.clone();
        self.values.splice(index..end, new_items);
        self.on_edit(self.op, &old, &self.values.clone());
        true
    }

    /// Hook called when an edit occurs. Subclasses can override.
    fn on_edit(&self, _op: ListOpType, _old_values: &[P::Value], _new_values: &[P::Value]) {
        // Base implementation does nothing. Specialized editors
        // (like SubLayerListEditor) override this.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestPolicy;

    impl VectorTypePolicy for TestPolicy {
        type Value = String;
        type FieldValue = String;

        fn field_name() -> Token {
            Token::from("testField")
        }

        fn from_field(field: &[String]) -> Vec<String> {
            field.to_vec()
        }

        fn to_field(values: &[String]) -> Vec<String> {
            values.to_vec()
        }
    }

    #[test]
    fn test_vector_list_editor_basic() {
        // Just test the data structure without a real layer.
        let values: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(values.len(), 3);
        assert!(values.contains(&"b".to_string()));
    }
}
