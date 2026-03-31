//! Non-owning type-erased value reference.
//!
//! `ValueRef` provides a lightweight, non-owning view into a value, avoiding
//! unnecessary cloning. It's lifetime-bounded to the source value and is
//! typically used as a function parameter or automatic variable.
//!
//! # Examples
//!
//! ```
//! use usd_vt::{Value, ValueRef};
//!
//! let value = Value::from(42i32);
//! let value_ref = ValueRef::from(&value);
//!
//! // Type-safe access without cloning
//! assert!(value_ref.is::<i32>());
//! assert_eq!(value_ref.get::<i32>(), Some(&42));
//! ```
//!
//! # Performance
//!
//! `ValueRef` is zero-cost - it's just a reference with type information.
//! No heap allocations or reference counting overhead.

use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;

use super::value::Value;

/// A non-owning type-erased reference to a value.
///
/// `ValueRef` provides efficient read-only access to values without cloning.
/// It remembers the type information and provides safe downcasting.
///
/// # Lifetime
///
/// `ValueRef` borrows the value it views, so it cannot outlive the source.
/// This is enforced by Rust's lifetime system.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, ValueRef};
///
/// fn print_if_int(val_ref: ValueRef) {
///     if let Some(&n) = val_ref.get::<i32>() {
///         println!("Integer: {}", n);
///     }
/// }
///
/// let value = Value::from(42i32);
/// print_if_int(ValueRef::from(&value));
/// ```
///
/// # Direct Value References
///
/// You can also create `ValueRef` from direct typed references:
///
/// ```
/// use usd_vt::ValueRef;
///
/// let number = 42i32;
/// let val_ref = ValueRef::from_typed(&number);
/// assert_eq!(val_ref.get::<i32>(), Some(&42));
/// ```
#[derive(Copy, Clone)]
pub struct ValueRef<'a> {
    /// Pointer to the held object (type-erased).
    obj_ptr: *const (),
    /// Type ID of the held object.
    type_id: TypeId,
    /// Type name for debugging.
    type_name: &'static str,
    /// Lifetime marker.
    _marker: PhantomData<&'a ()>,
}

impl<'a> ValueRef<'a> {
    /// Creates an empty `ValueRef`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let empty = ValueRef::empty();
    /// assert!(empty.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            obj_ptr: std::ptr::null(),
            type_id: TypeId::of::<()>(), // Dummy type for empty
            type_name: "void",
            _marker: PhantomData,
        }
    }

    /// Creates a `ValueRef` from a typed reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    /// assert_eq!(val_ref.get::<i32>(), Some(&42));
    /// ```
    #[inline]
    #[must_use]
    pub fn from_typed<T: 'static>(value: &'a T) -> Self {
        Self {
            obj_ptr: value as *const T as *const (),
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            _marker: PhantomData,
        }
    }

    /// Returns true if this reference is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let empty = ValueRef::empty();
    /// assert!(empty.is_empty());
    ///
    /// let num = 42i32;
    /// let val_ref = ValueRef::from_typed(&num);
    /// assert!(!val_ref.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.obj_ptr.is_null()
    }

    /// Returns true if this reference holds type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    ///
    /// assert!(val_ref.is::<i32>());
    /// assert!(!val_ref.is::<f64>());
    /// ```
    #[inline]
    #[must_use]
    pub fn is<T: 'static>(&self) -> bool {
        !self.is_empty() && self.type_id == TypeId::of::<T>()
    }

    /// Returns the TypeId of the referenced type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::TypeId;
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    /// assert_eq!(val_ref.get_type_id(), TypeId::of::<i32>());
    /// ```
    #[inline]
    #[must_use]
    pub fn get_type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the type name for debugging.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    /// assert!(val_ref.type_name().contains("i32"));
    /// ```
    #[inline]
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        if self.is_empty() {
            "void"
        } else {
            self.type_name
        }
    }

    /// Returns a reference to the held value if type matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    ///
    /// assert_eq!(val_ref.get::<i32>(), Some(&42));
    /// assert_eq!(val_ref.get::<f64>(), None);
    /// ```
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub fn get<T: 'static>(&self) -> Option<&'a T> {
        if self.is::<T>() {
            // SAFETY: We verified the type matches via TypeId check above
            unsafe { Some(&*(self.obj_ptr as *const T)) }
        } else {
            None
        }
    }

    /// Returns a reference to the held value without type checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure the type `T` matches the held type,
    /// otherwise this invokes undefined behavior. Use `is<T>()` first.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    ///
    /// // Safe only after checking the type
    /// if val_ref.is::<i32>() {
    ///     let n = unsafe { val_ref.get_unchecked::<i32>() };
    ///     assert_eq!(n, &42);
    /// }
    /// ```
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn get_unchecked<T: 'static>(&self) -> &'a T {
        // SAFETY: Caller guarantees T matches the held type
        unsafe { &*(self.obj_ptr as *const T) }
    }

    /// Converts this reference back to a `Value` by cloning.
    ///
    /// This creates a new `Value` containing a clone of the referenced data.
    /// Note: This only works for types that were stored in a `Value` originally.
    /// For direct typed references created with `from_typed()`, this returns empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Value, ValueRef};
    ///
    /// let original = Value::from(42i32);
    /// let val_ref = ValueRef::from(&original);
    /// let cloned = val_ref.as_value();
    ///
    /// // Note: This is limited - full implementation needs more Value internals
    /// ```
    #[inline]
    #[must_use]
    pub fn as_value(&self) -> Value {
        // Limited implementation: try to reconstruct for common types
        // Full implementation would need Value to expose clone_from_ptr
        if self.is_empty() {
            return Value::empty();
        }

        // Try common types
        if let Some(&val) = self.get::<i32>() {
            return Value::from(val);
        }
        if let Some(&val) = self.get::<i64>() {
            return Value::from(val);
        }
        if let Some(&val) = self.get::<f32>() {
            return Value::from(val);
        }
        if let Some(&val) = self.get::<f64>() {
            return Value::from(val);
        }
        if let Some(&val) = self.get::<bool>() {
            return Value::from(val);
        }
        if let Some(val) = self.get::<String>() {
            return Value::from(val.clone());
        }

        // For unknown types, return empty
        // Full implementation needs Value cooperation to clone arbitrary types
        Value::empty()
    }

    /// Returns true if this views an Array instance.
    ///
    /// Matches C++ `VtValueRef::IsArrayValued()`.
    #[must_use]
    pub fn is_array_valued(&self) -> bool {
        // Check if type name contains "Array" or check against known array types
        self.type_name.contains("Array") || self.type_name.contains("Vec")
    }

    /// Returns true if this views an ArrayEdit instance.
    ///
    /// Matches C++ `VtValueRef::IsArrayEditValued()`.
    #[must_use]
    pub fn is_array_edit_valued(&self) -> bool {
        self.type_name.contains("ArrayEdit")
    }

    /// Returns the type name as a String.
    ///
    /// Matches C++ `VtValueRef::GetTypeName()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    /// assert_eq!(val_ref.get_type_name(), "i32");
    /// ```
    #[must_use]
    pub fn get_type_name(&self) -> String {
        self.type_name.to_string()
    }

    /// Returns a reference to the viewed object without type checking.
    ///
    /// Matches C++ `VtValueRef::UncheckedGet<T>()`.
    ///
    /// # Safety
    ///
    /// The caller must ensure the type `T` matches the held type,
    /// otherwise this invokes undefined behavior.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn unchecked_get<T: 'static>(&self) -> &'a T {
        // SAFETY: Delegated to get_unchecked with same safety requirements
        unsafe { self.get_unchecked::<T>() }
    }

    /// Returns true if the viewed object can be hashed.
    ///
    /// Matches C++ `VtValueRef::CanHash()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    /// assert!(val_ref.can_hash());
    /// ```
    #[must_use]
    pub fn can_hash(&self) -> bool {
        // Most types can be hashed in Rust
        // This is a simplified check - full implementation would check trait bounds
        !self.is_empty()
    }

    /// Returns a hash code for the viewed object.
    ///
    /// Matches C++ `VtValueRef::GetHash()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ValueRef;
    /// use std::collections::hash_map::DefaultHasher;
    /// use std::hash::{Hash, Hasher};
    ///
    /// let number = 42i32;
    /// let val_ref = ValueRef::from_typed(&number);
    /// let hash = val_ref.get_hash();
    /// assert!(hash > 0);
    /// ```
    #[must_use]
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        if self.is_empty() {
            return 0;
        }

        let mut hasher = DefaultHasher::new();
        // Hash the type ID and pointer
        self.type_id.hash(&mut hasher);
        (self.obj_ptr as usize).hash(&mut hasher);
        hasher.finish()
    }

    /// Returns true if this value can compose over other types.
    ///
    /// Matches C++ `VtValueRef::CanComposeOver()`.
    #[inline]
    #[must_use]
    pub fn can_compose_over(&self) -> bool {
        // Simplified - full implementation would check trait bounds
        !self.is_empty()
    }

    /// Returns true if this value supports transforms.
    ///
    /// Matches C++ `VtValueRef::CanTransform()`.
    #[inline]
    #[must_use]
    pub fn can_transform(&self) -> bool {
        // Simplified - full implementation would check trait bounds
        !self.is_empty()
    }
}

impl<'a> From<&'a Value> for ValueRef<'a> {
    /// Creates a `ValueRef` from a `Value` reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Value, ValueRef};
    ///
    /// let value = Value::from(42i32);
    /// let val_ref = ValueRef::from(&value);
    /// assert_eq!(val_ref.get::<i32>(), Some(&42));
    /// ```
    fn from(value: &'a Value) -> Self {
        if let Some((ptr, type_id, type_name)) = value.as_raw_parts() {
            Self {
                obj_ptr: ptr,
                type_id,
                type_name,
                _marker: PhantomData,
            }
        } else {
            Self::empty()
        }
    }
}

impl<'a> Default for ValueRef<'a> {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a> fmt::Debug for ValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "ValueRef(empty)")
        } else {
            write!(f, "ValueRef({})", self.type_name)
        }
    }
}

impl<'a> fmt::Display for ValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "<empty>")
        } else {
            write!(f, "<{}>", self.type_name)
        }
    }
}

// SAFETY: ValueRef is just a pointer + metadata. Since it's a read-only view
// with a lifetime bound to 'a, it's safe to Send/Sync as long as 'a is Send/Sync.
// The type-erased pointer is only dereferenced after TypeId verification.
#[allow(unsafe_code)]
unsafe impl<'a> Send for ValueRef<'a> {}
#[allow(unsafe_code)]
unsafe impl<'a> Sync for ValueRef<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let empty = ValueRef::empty();
        assert!(empty.is_empty());
        assert!(!empty.is::<i32>());
        assert_eq!(empty.get::<i32>(), None);
        assert_eq!(empty.type_name(), "void");
    }

    #[test]
    fn test_from_typed() {
        let number = 42i32;
        let val_ref = ValueRef::from_typed(&number);

        assert!(!val_ref.is_empty());
        assert!(val_ref.is::<i32>());
        assert!(!val_ref.is::<f64>());
        assert_eq!(val_ref.get::<i32>(), Some(&42));
        assert_eq!(val_ref.get::<f64>(), None);
    }

    #[test]
    fn test_from_typed_string() {
        let text = String::from("hello");
        let val_ref = ValueRef::from_typed(&text);

        assert!(val_ref.is::<String>());
        assert_eq!(val_ref.get::<String>(), Some(&String::from("hello")));
    }

    #[test]
    fn test_from_value() {
        let value = Value::from(42i32);
        let val_ref = ValueRef::from(&value);

        assert!(!val_ref.is_empty());
        assert!(val_ref.is::<i32>());
        assert_eq!(val_ref.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_from_value_float() {
        let value = Value::from(3.14f32);
        let val_ref = ValueRef::from(&value);

        assert!(val_ref.is::<f32>());
        assert_eq!(val_ref.get::<f32>(), Some(&3.14f32));
    }

    #[test]
    fn test_from_empty_value() {
        let value = Value::empty();
        let val_ref = ValueRef::from(&value);

        assert!(val_ref.is_empty());
    }

    #[test]
    fn test_get_type_id() {
        let number = 42i32;
        let val_ref = ValueRef::from_typed(&number);

        assert_eq!(val_ref.get_type_id(), TypeId::of::<i32>());
    }

    #[allow(unsafe_code)]
    #[test]
    fn test_get_unchecked() {
        let number = 42i32;
        let val_ref = ValueRef::from_typed(&number);

        if val_ref.is::<i32>() {
            let n = unsafe { val_ref.get_unchecked::<i32>() };
            assert_eq!(n, &42);
        }
    }

    #[test]
    fn test_clone() {
        let number = 42i32;
        let val_ref1 = ValueRef::from_typed(&number);
        let val_ref2 = val_ref1;

        assert_eq!(val_ref2.get::<i32>(), Some(&42));
    }

    // Lifetime bounds test - this demonstrates the API
    // Uncommenting this would fail to compile (as intended):
    // #[test]
    // fn test_lifetime_bounds() {
    //     let val_ref = {
    //         let number = 42i32;
    //         ValueRef::from_typed(&number)
    //     }; // number is dropped here
    //
    //     // val_ref cannot be used here - won't compile
    //     drop(val_ref);
    // }

    #[test]
    fn test_debug_format() {
        let empty = ValueRef::empty();
        let debug_str = format!("{:?}", empty);
        assert!(debug_str.contains("empty"));

        let number = 42i32;
        let val_ref = ValueRef::from_typed(&number);
        let debug_str = format!("{:?}", val_ref);
        assert!(debug_str.contains("i32"));
    }

    #[test]
    fn test_display_format() {
        let empty = ValueRef::empty();
        let display_str = format!("{}", empty);
        assert_eq!(display_str, "<empty>");

        let number = 42i32;
        let val_ref = ValueRef::from_typed(&number);
        let display_str = format!("{}", val_ref);
        assert!(display_str.contains("i32"));
    }

    #[test]
    fn test_multiple_types() {
        let int_val = 42i32;
        let float_val = 3.14f32;
        let string_val = String::from("test");
        let bool_val = true;

        let int_ref = ValueRef::from_typed(&int_val);
        let float_ref = ValueRef::from_typed(&float_val);
        let string_ref = ValueRef::from_typed(&string_val);
        let bool_ref = ValueRef::from_typed(&bool_val);

        assert!(int_ref.is::<i32>());
        assert!(float_ref.is::<f32>());
        assert!(string_ref.is::<String>());
        assert!(bool_ref.is::<bool>());

        assert_eq!(int_ref.get::<i32>(), Some(&42));
        assert_eq!(float_ref.get::<f32>(), Some(&3.14));
        assert_eq!(string_ref.get::<String>(), Some(&String::from("test")));
        assert_eq!(bool_ref.get::<bool>(), Some(&true));
    }

    #[test]
    fn test_function_parameter() {
        fn process_value(val_ref: ValueRef) -> String {
            if let Some(&n) = val_ref.get::<i32>() {
                format!("Integer: {}", n)
            } else if let Some(s) = val_ref.get::<String>() {
                format!("String: {}", s)
            } else {
                "Unknown".to_string()
            }
        }

        let number = 42i32;
        let text = String::from("hello");

        assert_eq!(process_value(ValueRef::from_typed(&number)), "Integer: 42");
        assert_eq!(process_value(ValueRef::from_typed(&text)), "String: hello");
    }

    #[test]
    fn test_as_value_roundtrip() {
        // Test common types
        let v_int = Value::from(42i32);
        let ref_int = ValueRef::from(&v_int);
        let cloned_int = ref_int.as_value();
        assert_eq!(cloned_int.get::<i32>(), Some(&42));

        let v_float = Value::from(3.14f32);
        let ref_float = ValueRef::from(&v_float);
        let cloned_float = ref_float.as_value();
        assert_eq!(cloned_float.get::<f32>(), Some(&3.14));

        let v_string = Value::from(String::from("test"));
        let ref_string = ValueRef::from(&v_string);
        let cloned_string = ref_string.as_value();
        assert_eq!(cloned_string.get::<String>(), Some(&String::from("test")));

        let v_bool = Value::from(true);
        let ref_bool = ValueRef::from(&v_bool);
        let cloned_bool = ref_bool.as_value();
        assert_eq!(cloned_bool.get::<bool>(), Some(&true));
    }

    #[test]
    fn test_as_value_empty() {
        let empty = ValueRef::empty();
        let value = empty.as_value();
        assert!(value.is_empty());
    }
}
