//! Accessor helpers - traits and utilities for spec field access.
//!
//! Provides Rust-idiomatic helpers for accessing and modifying spec fields,
//! replacing the C++ accessor macros with trait-based approach.

use usd_tf::Token;
use usd_vt::Value as VtValue;

use super::schema::SchemaBase;
use super::{LayerHandle, Path};

// ============================================================================
// Field Accessor Trait
// ============================================================================

/// Trait for accessing spec fields.
///
/// Provides generic get/set/has/clear operations for any field on a spec.
/// This is the Rust equivalent of the C++ SDF_ACCESSOR_* macros.
pub trait FieldAccessor {
    /// Gets the schema for this spec.
    fn get_schema(&self) -> &SchemaBase;

    /// Gets a field value.
    fn get_field(&self, key: &Token) -> Option<VtValue>;

    /// Sets a field value.
    fn set_field(&mut self, key: &Token, value: VtValue) -> bool;

    /// Checks if a field has a value.
    fn has_field(&self, key: &Token) -> bool;

    /// Clears a field value.
    fn clear_field(&mut self, key: &Token);

    /// Gets the layer handle for this spec.
    fn get_layer_handle(&self) -> LayerHandle;

    /// Gets the path for this spec.
    fn get_path(&self) -> &Path;
}

// ============================================================================
// Typed Field Accessors
// ============================================================================

/// Trait for typed field access with automatic conversion.
///
/// Extends `FieldAccessor` with typed get/set methods that handle
/// conversion to/from VtValue automatically.
pub trait TypedFieldAccessor: FieldAccessor {
    /// Gets a field value with a specific type, returning `None` if neither
    /// the field value nor the schema fallback holds the requested type.
    ///
    /// This is the safe variant. C++ equivalent raises `TF_CODING_ERROR` (non-fatal)
    /// when the fallback is missing; here we propagate `None` instead of panicking.
    fn get_field_opt<T>(&self, key: &Token) -> Option<T>
    where
        T: Clone + 'static,
    {
        // Try authored field value first.
        if let Some(value) = self.get_field(key) {
            if let Some(typed) = value.get::<T>() {
                return Some(typed.clone());
            }
        }
        // Fall back to schema default.
        let schema = self.get_schema();
        let fallback = schema.get_fallback(key);
        fallback.get::<T>().cloned()
    }

    /// Gets a field value with a specific type, with fallback to schema default.
    ///
    /// Returns the schema fallback if the field is not set or has wrong type.
    /// If no fallback exists either, logs a warning and returns `T::default()`.
    /// Matches C++ `TF_CODING_ERROR` (non-fatal).
    fn get_field_with_fallback<T>(&self, key: &Token) -> T
    where
        T: Clone + Default + 'static,
    {
        if let Some(v) = self.get_field_opt::<T>(key) {
            return v;
        }
        // C++ uses TF_CODING_ERROR (non-fatal). Log and return default.
        eprintln!(
            "[usd-sdf] SdfAccessorHelpers: no fallback value for field '{}' of type '{}'",
            key.as_str(),
            std::any::type_name::<T>()
        );
        T::default()
    }

    /// Gets a typed field value.
    ///
    /// Returns None if field is not set or has wrong type.
    fn get_typed_field<T>(&self, key: &Token) -> Option<T>
    where
        T: Clone + 'static,
    {
        self.get_field(key).and_then(|v| v.get::<T>().cloned())
    }

    /// Sets a typed field value.
    fn set_typed_field<T>(&mut self, key: &Token, value: T) -> bool
    where
        T: Clone + Send + Sync + std::fmt::Debug + PartialEq + std::hash::Hash + 'static,
    {
        self.set_field(key, VtValue::new(value))
    }

    /// Gets a boolean field value.
    fn is_field_set<T>(&self, key: &Token) -> bool
    where
        T: Clone + 'static,
    {
        self.get_typed_field::<bool>(key).unwrap_or(false)
    }
}

// Blanket implementation for all FieldAccessors
impl<T: FieldAccessor> TypedFieldAccessor for T {}

// ============================================================================
// Read/Write Predicates
// ============================================================================

/// Trait for checking if fields can be read.
pub trait ReadPredicate {
    /// Returns true if the field can be read.
    fn can_read(&self, key: &Token) -> bool;
}

/// Trait for checking if fields can be written.
pub trait WritePredicate {
    /// Returns true if the field can be written.
    fn can_write(&self, key: &Token) -> bool;
}

/// Default read predicate that allows all reads.
#[derive(Debug, Clone, Copy)]
pub struct AllowAllReads;

impl ReadPredicate for AllowAllReads {
    fn can_read(&self, _key: &Token) -> bool {
        true
    }
}

/// Default write predicate that allows all writes.
#[derive(Debug, Clone, Copy)]
pub struct AllowAllWrites;

impl WritePredicate for AllowAllWrites {
    fn can_write(&self, _key: &Token) -> bool {
        true
    }
}

// ============================================================================
// Accessor Builder
// ============================================================================

/// Builder for creating field accessors with custom predicates.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::accessor_helpers::*;
///
/// let accessor = AccessorBuilder::new(spec)
///     .with_read_predicate(custom_read_pred)
///     .with_write_predicate(custom_write_pred)
///     .build();
/// ```
#[derive(Debug)]
pub struct AccessorBuilder<S, R, W> {
    spec: S,
    read_pred: R,
    write_pred: W,
}

impl<S> AccessorBuilder<S, AllowAllReads, AllowAllWrites> {
    /// Creates a new accessor builder with default predicates.
    pub fn new(spec: S) -> Self {
        Self {
            spec,
            read_pred: AllowAllReads,
            write_pred: AllowAllWrites,
        }
    }
}

impl<S, R, W> AccessorBuilder<S, R, W> {
    /// Sets a custom read predicate.
    pub fn with_read_predicate<NewR>(self, pred: NewR) -> AccessorBuilder<S, NewR, W>
    where
        NewR: ReadPredicate,
    {
        AccessorBuilder {
            spec: self.spec,
            read_pred: pred,
            write_pred: self.write_pred,
        }
    }

    /// Sets a custom write predicate.
    pub fn with_write_predicate<NewW>(self, pred: NewW) -> AccessorBuilder<S, R, NewW>
    where
        NewW: WritePredicate,
    {
        AccessorBuilder {
            spec: self.spec,
            read_pred: self.read_pred,
            write_pred: pred,
        }
    }

    /// Builds the accessor.
    pub fn build(self) -> GuardedAccessor<S, R, W> {
        GuardedAccessor {
            spec: self.spec,
            read_pred: self.read_pred,
            write_pred: self.write_pred,
        }
    }
}

// ============================================================================
// Guarded Accessor
// ============================================================================

/// Accessor with read/write guards.
///
/// Wraps a spec with predicates that control field access.
#[derive(Debug)]
pub struct GuardedAccessor<S, R, W> {
    spec: S,
    read_pred: R,
    write_pred: W,
}

impl<S, R, W> GuardedAccessor<S, R, W>
where
    S: FieldAccessor,
    R: ReadPredicate,
    W: WritePredicate,
{
    /// Gets a field value if allowed.
    pub fn get_field(&self, key: &Token) -> Option<VtValue> {
        if self.read_pred.can_read(key) {
            self.spec.get_field(key)
        } else {
            None
        }
    }

    /// Sets a field value if allowed.
    pub fn set_field(&mut self, key: &Token, value: VtValue) -> bool {
        if self.write_pred.can_write(key) {
            self.spec.set_field(key, value)
        } else {
            false
        }
    }

    /// Checks if a field has a value (if reading is allowed).
    pub fn has_field(&self, key: &Token) -> bool {
        if self.read_pred.can_read(key) {
            self.spec.has_field(key)
        } else {
            false
        }
    }

    /// Clears a field (if writing is allowed).
    pub fn clear_field(&mut self, key: &Token) {
        if self.write_pred.can_write(key) {
            self.spec.clear_field(key);
        }
    }
}

// ============================================================================
// Convenience Macros (Optional Rust-style)
// ============================================================================

/// Generates typed accessor methods for a field.
///
/// This is a Rust replacement for the C++ SDF_DEFINE_GET_SET macros.
///
/// # Examples
///
/// ```ignore
/// impl MySpec {
///     // Generates: get_name() and set_name(value)
///     define_accessor!(name, Name, String, "name");
/// }
/// ```
#[macro_export]
macro_rules! define_accessor {
    ($field:ident, $Name:ident, $Type:ty, $key:expr) => {
        paste::paste! {
            /// Gets the field value.
            pub fn [<get_ $field>](&self) -> $Type {
                use $crate::accessor_helpers::TypedFieldAccessor;
                let key = $usd_tf::Token::new($key);
                self.get_field_with_fallback(&key)
            }

            /// Sets the field value.
            pub fn [<set_ $field>](&mut self, value: $Type) {
                use $crate::accessor_helpers::TypedFieldAccessor;
                let key = $usd_tf::Token::new($key);
                self.set_typed_field(&key, value);
            }

            /// Checks if the field has a value.
            pub fn [<has_ $field>](&self) -> bool {
                let key = $usd_tf::Token::new($key);
                self.has_field(&key)
            }

            /// Clears the field value.
            pub fn [<clear_ $field>](&mut self) {
                let key = $usd_tf::Token::new($key);
                self.clear_field(&key);
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_all_predicates() {
        let read_pred = AllowAllReads;
        let write_pred = AllowAllWrites;

        let key = Token::new("test");
        assert!(read_pred.can_read(&key));
        assert!(write_pred.can_write(&key));
    }

    #[test]
    fn test_accessor_builder_pattern() {
        // Just test that the builder pattern compiles
        struct DummySpec;

        let _builder = AccessorBuilder::new(DummySpec)
            .with_read_predicate(AllowAllReads)
            .with_write_predicate(AllowAllWrites);
    }
}
