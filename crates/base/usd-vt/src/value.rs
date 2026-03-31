//! Type-erased value container.
//!
//! `Value` provides a container that can hold any type, with support for
//! introspection, casting, and comparison. This is the Rust equivalent of
//! OpenUSD's `VtValue`.
//!
//! # Examples
//!
//! ```
//! use usd_vt::Value;
//!
//! // Create values of different types
//! let int_val = Value::from(42i32);
//! let float_val = Value::from(3.14f64);
//! let string_val = Value::from("hello".to_string());
//!
//! // Check and extract types
//! assert!(int_val.is::<i32>());
//! assert_eq!(int_val.get::<i32>(), Some(&42));
//!
//! // Empty value
//! let empty = Value::empty();
//! assert!(empty.is_empty());
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// ============================================================================
// Inline storage for small Copy types (H2: Local Storage Optimization)
// ============================================================================

/// Maximum size for inline-stored values (16 bytes covers all primitives).
const INLINE_MAX_SIZE: usize = 16;

/// Aligned storage for inline values using MaybeUninit for safe uninitialized memory.
/// This reduces unsafe code by using MaybeUninit instead of raw byte arrays.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct InlineData {
    data: std::mem::MaybeUninit<[u8; INLINE_MAX_SIZE]>,
}

impl InlineData {
    /// Creates new zeroed inline data.
    #[inline]
    const fn zeroed() -> Self {
        Self {
            data: std::mem::MaybeUninit::new([0u8; INLINE_MAX_SIZE]),
        }
    }

    /// Stores a value into inline data.
    ///
    /// SAFETY INVARIANT: T must fit in INLINE_MAX_SIZE, be Copy, and align <= 16.
    /// This is the ONLY method that writes to inline storage.
    ///
    /// # Safety
    /// The caller must ensure:
    /// - sizeof(T) <= INLINE_MAX_SIZE
    /// - alignof(T) <= 16
    /// - T is Copy (required for memcpy-like behavior)
    #[inline]
    #[allow(unsafe_code)]
    unsafe fn store<T: Copy>(&mut self, val: T) {
        debug_assert!(
            std::mem::size_of::<T>() <= INLINE_MAX_SIZE,
            "Type too large for inline storage"
        );
        debug_assert!(
            std::mem::align_of::<T>() <= 16,
            "Type alignment exceeds inline storage alignment"
        );

        // SAFETY: Caller guarantees T fits. We use ptr::write to avoid drop.
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.data.as_mut_ptr() as *mut T;
            ptr.write(val);
        }
    }

    /// Loads a reference to the stored value.
    ///
    /// SAFETY INVARIANT: This is one of TWO methods that read from inline storage.
    /// Type safety is enforced by TypeId checks in InlineValue::get/get_mut.
    ///
    /// # Safety
    /// The caller must ensure T matches the type that was stored via `store()`.
    #[inline]
    #[allow(unsafe_code)]
    unsafe fn load<T>(&self) -> &T {
        // SAFETY: Caller guarantees type matches. Alignment guaranteed by repr(C, align(16)).
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.data.as_ptr() as *const T;
            &*ptr
        }
    }

    /// Loads a mutable reference to the stored value.
    ///
    /// SAFETY INVARIANT: This is one of TWO methods that read from inline storage.
    /// Type safety is enforced by TypeId checks in InlineValue::get/get_mut.
    ///
    /// # Safety
    /// The caller must ensure T matches the type that was stored via `store()`.
    #[inline]
    #[allow(unsafe_code)]
    unsafe fn load_mut<T>(&mut self) -> &mut T {
        // SAFETY: Caller guarantees type matches. Alignment guaranteed by repr(C, align(16)).
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.data.as_mut_ptr() as *mut T;
            &mut *ptr
        }
    }
}

/// Vtable for inline-stored types, providing type-erased operations.
struct InlineVtable {
    type_name: &'static str,
    eq_fn: fn(&InlineData, &InlineData) -> bool,
    debug_fn: fn(&InlineData, &mut fmt::Formatter<'_>) -> fmt::Result,
    /// Raw value formatting for stream_out (no "Value()" wrapper).
    stream_out_fn: fn(&InlineData) -> String,
    compute_hash_fn: fn(&InlineData) -> u64,
}

/// Inline value: type_id + data + vtable pointer (no heap allocation).
#[derive(Clone, Copy)]
struct InlineValue {
    type_id: TypeId,
    data: InlineData,
    vtable: &'static InlineVtable,
}

/// Creates a static vtable for a type that implements Hash.
///
/// SAFETY: All closures use InlineData::load() which is safe because:
/// - Type is enforced by InlineValue's TypeId check before vtable is called
/// - InlineData guarantees alignment and size constraints
macro_rules! inline_vtable_hash {
    ($T:ty, $name:expr) => {{
        #[allow(unsafe_code)]
        static VTABLE: InlineVtable = InlineVtable {
            type_name: $name,
            eq_fn: |a, b| {
                // SAFETY: TypeId check in InlineValue ensures type correctness
                unsafe {
                    let a_val: &$T = a.load();
                    let b_val: &$T = b.load();
                    a_val == b_val
                }
            },
            debug_fn: |data, f| {
                // SAFETY: TypeId check in InlineValue ensures type correctness
                unsafe {
                    let val: &$T = data.load();
                    write!(f, "Value({:?})", val)
                }
            },
            stream_out_fn: |data| {
                // SAFETY: TypeId check in InlineValue ensures type correctness
                unsafe {
                    let val: &$T = data.load();
                    format!("{:?}", val)
                }
            },
            compute_hash_fn: |data| {
                // SAFETY: TypeId check in InlineValue ensures type correctness
                unsafe {
                    use std::collections::hash_map::DefaultHasher;
                    let val: &$T = data.load();
                    let mut hasher = DefaultHasher::new();
                    TypeId::of::<$T>().hash(&mut hasher);
                    val.hash(&mut hasher);
                    hasher.finish()
                }
            },
        };
        &VTABLE
    }};
}

/// Creates a static vtable for f32 (uses to_bits for hashing).
///
/// SAFETY: All closures use InlineData::load() which is safe because:
/// - Type is enforced by InlineValue's TypeId check before vtable is called
/// - InlineData guarantees alignment and size constraints
macro_rules! inline_vtable_f32 {
    () => {{
        #[allow(unsafe_code)]
        static VTABLE: InlineVtable = InlineVtable {
            type_name: "f32",
            eq_fn: |a, b| {
                // SAFETY: TypeId check ensures type is f32
                unsafe {
                    let a_val: &f32 = a.load();
                    let b_val: &f32 = b.load();
                    a_val == b_val
                }
            },
            debug_fn: |data, f| {
                // SAFETY: TypeId check ensures type is f32
                unsafe {
                    let val: &f32 = data.load();
                    write!(f, "Value({:?})", val)
                }
            },
            stream_out_fn: |data| {
                // SAFETY: TypeId check ensures type is f32
                unsafe {
                    let val: &f32 = data.load();
                    format!("{}", val)
                }
            },
            compute_hash_fn: |data| {
                // SAFETY: TypeId check ensures type is f32
                unsafe {
                    use std::collections::hash_map::DefaultHasher;
                    let val: &f32 = data.load();
                    let mut hasher = DefaultHasher::new();
                    TypeId::of::<f32>().hash(&mut hasher);
                    val.to_bits().hash(&mut hasher);
                    hasher.finish()
                }
            },
        };
        &VTABLE
    }};
}

/// Creates a static vtable for f64 (uses to_bits for hashing).
///
/// SAFETY: All closures use InlineData::load() which is safe because:
/// - Type is enforced by InlineValue's TypeId check before vtable is called
/// - InlineData guarantees alignment and size constraints
macro_rules! inline_vtable_f64 {
    () => {{
        #[allow(unsafe_code)]
        static VTABLE: InlineVtable = InlineVtable {
            type_name: "f64",
            eq_fn: |a, b| {
                // SAFETY: TypeId check ensures type is f64
                unsafe {
                    let a_val: &f64 = a.load();
                    let b_val: &f64 = b.load();
                    a_val == b_val
                }
            },
            debug_fn: |data, f| {
                // SAFETY: TypeId check ensures type is f64
                unsafe {
                    let val: &f64 = data.load();
                    write!(f, "Value({:?})", val)
                }
            },
            stream_out_fn: |data| {
                // SAFETY: TypeId check ensures type is f64
                unsafe {
                    let val: &f64 = data.load();
                    format!("{}", val)
                }
            },
            compute_hash_fn: |data| {
                // SAFETY: TypeId check ensures type is f64
                unsafe {
                    use std::collections::hash_map::DefaultHasher;
                    let val: &f64 = data.load();
                    let mut hasher = DefaultHasher::new();
                    TypeId::of::<f64>().hash(&mut hasher);
                    val.to_bits().hash(&mut hasher);
                    hasher.finish()
                }
            },
        };
        &VTABLE
    }};
}

/// Generates a typed constructor for InlineValue.
///
/// SAFETY: The store() call is safe because:
/// - Macro is only used for primitive types that fit in INLINE_MAX_SIZE
/// - All primitive types have alignment <= 16
/// - T is Copy (required by store's signature)
macro_rules! impl_inline_new {
    ($fn_name:ident, $T:ty, $vtable_macro:ident) => {
        #[inline]
        fn $fn_name(val: $T) -> Self {
            let mut data = InlineData::zeroed();
            // SAFETY: Primitive type guaranteed to fit, align, and be Copy
            #[allow(unsafe_code)]
            unsafe {
                data.store(val);
            }
            Self {
                type_id: TypeId::of::<$T>(),
                data,
                vtable: $vtable_macro!($T, stringify!($T)),
            }
        }
    };
}

impl InlineValue {
    impl_inline_new!(new_bool, bool, inline_vtable_hash);
    impl_inline_new!(new_i8, i8, inline_vtable_hash);
    impl_inline_new!(new_i16, i16, inline_vtable_hash);
    impl_inline_new!(new_i32, i32, inline_vtable_hash);
    impl_inline_new!(new_i64, i64, inline_vtable_hash);
    impl_inline_new!(new_u8, u8, inline_vtable_hash);
    impl_inline_new!(new_u16, u16, inline_vtable_hash);
    impl_inline_new!(new_u32, u32, inline_vtable_hash);
    impl_inline_new!(new_u64, u64, inline_vtable_hash);

    /// Creates an inline value for f32.
    #[inline]
    fn new_f32(val: f32) -> Self {
        let mut data = InlineData::zeroed();
        // SAFETY: f32 (4 bytes) fits in INLINE_MAX_SIZE (16 bytes), align=4 <= 16, Copy
        #[allow(unsafe_code)]
        unsafe {
            data.store(val);
        }
        Self {
            type_id: TypeId::of::<f32>(),
            data,
            vtable: inline_vtable_f32!(),
        }
    }

    /// Creates an inline value for f64.
    #[inline]
    fn new_f64(val: f64) -> Self {
        let mut data = InlineData::zeroed();
        // SAFETY: f64 (8 bytes) fits in INLINE_MAX_SIZE (16 bytes), align=8 <= 16, Copy
        #[allow(unsafe_code)]
        unsafe {
            data.store(val);
        }
        Self {
            type_id: TypeId::of::<f64>(),
            data,
            vtable: inline_vtable_f64!(),
        }
    }

    /// Returns a reference to the stored value if type matches.
    ///
    /// Type safety is enforced by comparing TypeId before calling load().
    #[inline]
    fn get<T: 'static>(&self) -> Option<&T> {
        if self.type_id == TypeId::of::<T>() {
            // SAFETY: TypeId check ensures T matches stored type
            #[allow(unsafe_code)]
            Some(unsafe { self.data.load::<T>() })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the stored value if type matches.
    ///
    /// Type safety is enforced by comparing TypeId before calling load_mut().
    #[inline]
    fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.type_id == TypeId::of::<T>() {
            // SAFETY: TypeId check ensures T matches stored type
            #[allow(unsafe_code)]
            Some(unsafe { self.data.load_mut::<T>() })
        } else {
            None
        }
    }
}

// ============================================================================
// Heap storage (for complex types, unchanged from before)
// ============================================================================

/// Internal trait for type-erased value storage.
trait ValueHolder: Send + Sync {
    /// Returns the TypeId of the held value.
    fn held_type_id(&self) -> TypeId;

    /// Returns the type name for debugging.
    fn held_type_name(&self) -> &'static str;

    /// Returns self as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns self as mutable Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Clone this holder into a new boxed trait object (for Arc::make_mut).
    fn clone_holder(&self) -> Box<dyn ValueHolder>;

    /// Debug format.
    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    /// Raw value format for stream_out (no "Value()" wrapper).
    fn stream_out_str(&self) -> String;

    /// Compare for equality.
    fn eq(&self, other: &dyn ValueHolder) -> bool;

    /// Compute a hash of the value.
    fn compute_hash(&self) -> u64;

    /// Returns true if this is a proxy value.
    fn is_proxy(&self) -> bool {
        false
    }

    /// For proxy holders: get the proxied value as a Value.
    fn get_proxied_value(&self) -> Option<Value> {
        None
    }

    /// For proxy holders: check if the proxied type matches T.
    fn proxy_holds_type(&self, _type_id: TypeId) -> bool {
        false
    }
}

impl Clone for Box<dyn ValueHolder> {
    fn clone(&self) -> Self {
        self.clone_holder()
    }
}

/// Wrapper struct that implements ValueHolder for hashable types.
struct Holder<T: Clone + Send + Sync + 'static> {
    value: T,
}

impl<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static> ValueHolder for Holder<T> {
    fn held_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn held_type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn as_any(&self) -> &dyn Any {
        &self.value
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.value
    }

    fn clone_holder(&self) -> Box<dyn ValueHolder> {
        Box::new(Holder {
            value: self.value.clone(),
        })
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value({:?})", self.value)
    }

    fn stream_out_str(&self) -> String {
        format!("{:?}", self.value)
    }

    fn eq(&self, other: &dyn ValueHolder) -> bool {
        if self.held_type_id() != other.held_type_id() {
            return false;
        }
        if let Some(other_val) = other.as_any().downcast_ref::<T>() {
            self.value == *other_val
        } else {
            false
        }
    }

    fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.held_type_id().hash(&mut hasher);
        self.value.hash(&mut hasher);
        hasher.finish()
    }
}

/// Wrapper for Dictionary (used by from_dictionary for backward compat).
/// Stores Dictionary so TypeId matches Value::get::<Dictionary>() downcasts.
struct DictHolder {
    value: crate::dictionary::Dictionary,
}

impl ValueHolder for DictHolder {
    fn held_type_id(&self) -> TypeId {
        TypeId::of::<crate::dictionary::Dictionary>()
    }

    fn held_type_name(&self) -> &'static str {
        "Dictionary"
    }

    fn as_any(&self) -> &dyn Any {
        &self.value
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.value
    }

    fn clone_holder(&self) -> Box<dyn ValueHolder> {
        Box::new(DictHolder {
            value: self.value.clone(),
        })
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value(Dict{{...}}")
    }

    fn stream_out_str(&self) -> String {
        "Dict{...}".to_string()
    }

    fn eq(&self, other: &dyn ValueHolder) -> bool {
        if self.held_type_id() != other.held_type_id() {
            return false;
        }
        if let Some(other_val) = other
            .as_any()
            .downcast_ref::<crate::dictionary::Dictionary>()
        {
            self.value == *other_val
        } else {
            false
        }
    }

    fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.held_type_id().hash(&mut hasher);
        self.value.hash(&mut hasher);
        hasher.finish()
    }
}

/// Wrapper struct for types that don't implement Hash (e.g., types containing floats).
/// Uses debug representation for hashing.
struct NoHashHolder<T: Clone + Send + Sync + fmt::Debug + PartialEq + 'static> {
    value: T,
}

impl<T: Clone + Send + Sync + fmt::Debug + PartialEq + 'static> ValueHolder for NoHashHolder<T> {
    fn held_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn held_type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn as_any(&self) -> &dyn Any {
        &self.value
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.value
    }

    fn clone_holder(&self) -> Box<dyn ValueHolder> {
        Box::new(NoHashHolder {
            value: self.value.clone(),
        })
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value({:?})", self.value)
    }

    fn stream_out_str(&self) -> String {
        format!("{:?}", self.value)
    }

    fn eq(&self, other: &dyn ValueHolder) -> bool {
        if self.held_type_id() != other.held_type_id() {
            return false;
        }
        if let Some(other_val) = other.as_any().downcast_ref::<T>() {
            self.value == *other_val
        } else {
            false
        }
    }

    fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.held_type_id().hash(&mut hasher);
        // Use debug representation for hashing
        format!("{:?}", self.value).hash(&mut hasher);
        hasher.finish()
    }
}

// ============================================================================
// Proxy holders (H3: Proxy Support)
// ============================================================================

/// Holder for typed value proxies.
///
/// A typed proxy knows the proxied type at compile time.
struct TypedProxyHolder<P>
where
    P: super::traits::TypedValueProxyBase
        + super::traits::GetProxiedObject
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + PartialEq
        + 'static,
    <P as super::traits::GetProxiedObject>::Proxied:
        Clone + Send + Sync + fmt::Debug + PartialEq + 'static,
{
    proxy: P,
}

impl<P> ValueHolder for TypedProxyHolder<P>
where
    P: super::traits::TypedValueProxyBase
        + super::traits::GetProxiedObject
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + PartialEq
        + 'static,
    <P as super::traits::GetProxiedObject>::Proxied:
        Clone + Send + Sync + fmt::Debug + PartialEq + 'static,
{
    fn held_type_id(&self) -> TypeId {
        // Report the proxy type itself
        TypeId::of::<P>()
    }

    fn held_type_name(&self) -> &'static str {
        std::any::type_name::<P>()
    }

    fn as_any(&self) -> &dyn Any {
        &self.proxy
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.proxy
    }

    fn clone_holder(&self) -> Box<dyn ValueHolder> {
        Box::new(TypedProxyHolder {
            proxy: self.proxy.clone(),
        })
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value(Proxy({:?}))", self.proxy)
    }

    fn stream_out_str(&self) -> String {
        format!("{:?}", self.proxy)
    }

    fn eq(&self, other: &dyn ValueHolder) -> bool {
        if self.held_type_id() != other.held_type_id() {
            return false;
        }
        if let Some(other_proxy) = other.as_any().downcast_ref::<P>() {
            self.proxy == *other_proxy
        } else {
            false
        }
    }

    fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.held_type_id().hash(&mut hasher);
        format!("{:?}", self.proxy).hash(&mut hasher);
        hasher.finish()
    }

    fn is_proxy(&self) -> bool {
        true
    }

    fn get_proxied_value(&self) -> Option<Value> {
        let proxied = self.proxy.get_proxied();
        Some(Value::from_no_hash(proxied.clone()))
    }

    fn proxy_holds_type(&self, type_id: TypeId) -> bool {
        type_id == TypeId::of::<<P as super::traits::GetProxiedObject>::Proxied>()
    }
}

/// Holder for erased value proxies.
///
/// An erased proxy determines its proxied type at runtime.
struct ErasedProxyHolder<P>
where
    P: super::traits::ErasedValueProxyBase + Clone + Send + Sync + fmt::Debug + PartialEq + 'static,
{
    proxy: P,
}

impl<P> ValueHolder for ErasedProxyHolder<P>
where
    P: super::traits::ErasedValueProxyBase + Clone + Send + Sync + fmt::Debug + PartialEq + 'static,
{
    fn held_type_id(&self) -> TypeId {
        TypeId::of::<P>()
    }

    fn held_type_name(&self) -> &'static str {
        std::any::type_name::<P>()
    }

    fn as_any(&self) -> &dyn Any {
        &self.proxy
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.proxy
    }

    fn clone_holder(&self) -> Box<dyn ValueHolder> {
        Box::new(ErasedProxyHolder {
            proxy: self.proxy.clone(),
        })
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value(ErasedProxy({:?}))", self.proxy)
    }

    fn stream_out_str(&self) -> String {
        format!("{:?}", self.proxy)
    }

    fn eq(&self, other: &dyn ValueHolder) -> bool {
        if self.held_type_id() != other.held_type_id() {
            return false;
        }
        if let Some(other_proxy) = other.as_any().downcast_ref::<P>() {
            self.proxy == *other_proxy
        } else {
            false
        }
    }

    fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.held_type_id().hash(&mut hasher);
        format!("{:?}", self.proxy).hash(&mut hasher);
        hasher.finish()
    }

    fn is_proxy(&self) -> bool {
        true
    }

    fn get_proxied_value(&self) -> Option<Value> {
        Some(self.proxy.get_erased_proxied_value())
    }

    fn proxy_holds_type(&self, type_id: TypeId) -> bool {
        // For erased proxies, resolve the value and check its type
        if let Some(resolved) = self.get_proxied_value() {
            resolved.held_type_id() == Some(type_id)
        } else {
            false
        }
    }
}

// ============================================================================
// Value struct with dual storage
// ============================================================================

/// Internal storage for Value.
enum ValueStorage {
    /// No value stored.
    Empty,
    /// Small Copy type stored inline (no heap allocation).
    Inline(InlineValue),
    /// Complex type stored on the heap via Arc.
    Heap(Arc<dyn ValueHolder>),
}

impl Clone for ValueStorage {
    fn clone(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Inline(iv) => Self::Inline(*iv),
            Self::Heap(arc) => Self::Heap(Arc::clone(arc)),
        }
    }
}

/// A type-erased value container.
///
/// `Value` can hold any type that implements `Clone + Send + Sync + 'static`.
/// It provides type introspection, safe casting, and comparison operations.
///
/// # Storage Optimization
///
/// Small Copy types (bool, integers, floats) are stored inline without heap
/// allocation, matching C++ VtValue's local storage optimization. Larger or
/// non-Copy types use heap storage via Arc.
///
/// # Thread Safety
///
/// `Value` is `Send + Sync`, allowing it to be shared across threads.
///
/// # Examples
///
/// ```
/// use usd_vt::Value;
///
/// // Create values
/// let num = Value::from(42i32);
/// let name = Value::from("MyObject".to_string());
///
/// // Type checking
/// assert!(num.is::<i32>());
/// assert!(name.is::<String>());
///
/// // Safe extraction
/// if let Some(&n) = num.get::<i32>() {
///     println!("Number: {}", n);
/// }
/// ```
#[derive(Clone)]
pub struct Value {
    storage: ValueStorage,
}

impl Value {
    /// Creates an empty value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::empty();
    /// assert!(v.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            storage: ValueStorage::Empty,
        }
    }

    /// Creates a new value holding the given data (heap-allocated).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::new(42i32);
    /// assert_eq!(v.get::<i32>(), Some(&42));
    /// ```
    #[inline]
    pub fn new<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static>(value: T) -> Self {
        Self {
            storage: ValueStorage::Heap(Arc::new(Holder { value })),
        }
    }

    /// Creates a new value holding an f32 (inline storage).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::from_f32(3.14f32);
    /// assert_eq!(v.get::<f32>(), Some(&3.14f32));
    /// ```
    #[inline]
    #[must_use]
    pub fn from_f32(value: f32) -> Self {
        Self {
            storage: ValueStorage::Inline(InlineValue::new_f32(value)),
        }
    }

    /// Creates a new value holding an f64 (inline storage).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::from_f64(3.14159f64);
    /// assert_eq!(v.get::<f64>(), Some(&3.14159f64));
    /// ```
    #[inline]
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        Self {
            storage: ValueStorage::Inline(InlineValue::new_f64(value)),
        }
    }

    /// Creates a new value from a HashMap (converts to Dictionary internally).
    ///
    /// This is useful for storing VtDictionary data. The HashMap is converted
    /// to a Dictionary so that `Value::get::<Dictionary>()` works correctly.
    #[inline]
    #[must_use]
    pub fn from_dictionary(value: HashMap<String, Value>) -> Self {
        let mut dict = crate::dictionary::Dictionary::new();
        for (k, v) in value {
            dict.insert_value(k, v);
        }
        Self {
            storage: ValueStorage::Heap(Arc::new(DictHolder { value: dict })),
        }
    }

    /// Creates a value from a type that doesn't implement Hash.
    ///
    /// Use this for types containing floats (Vec2d, Matrix4d, etc.) which
    /// cannot implement Hash directly.
    #[inline]
    #[must_use]
    pub fn from_no_hash<T: Clone + Send + Sync + fmt::Debug + PartialEq + 'static>(
        value: T,
    ) -> Self {
        Self {
            storage: ValueStorage::Heap(Arc::new(NoHashHolder { value })),
        }
    }

    /// Creates a new value holding a SplineValue.
    ///
    /// Splines are animation curves that can vary over time.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Value, spline::{SplineValue, SplineCurveType}};
    ///
    /// let spline = SplineValue::new(SplineCurveType::Bezier);
    /// let v = Value::from_spline(spline);
    /// assert!(v.is::<SplineValue>());
    /// ```
    #[inline]
    #[must_use]
    pub fn from_spline(value: crate::spline::SplineValue) -> Self {
        Self::from_no_hash(value)
    }

    /// Creates a value holding a typed proxy.
    ///
    /// The proxy will be unwrapped transparently when accessing the
    /// proxied value.
    #[inline]
    pub fn from_typed_proxy<P>(proxy: P) -> Self
    where
        P: super::traits::TypedValueProxyBase
            + super::traits::GetProxiedObject
            + Clone
            + Send
            + Sync
            + fmt::Debug
            + PartialEq
            + 'static,
        <P as super::traits::GetProxiedObject>::Proxied:
            Clone + Send + Sync + fmt::Debug + PartialEq + 'static,
    {
        Self {
            storage: ValueStorage::Heap(Arc::new(TypedProxyHolder { proxy })),
        }
    }

    /// Creates a value holding an erased proxy.
    ///
    /// The proxy resolves to a Value at runtime.
    #[inline]
    pub fn from_erased_proxy<P>(proxy: P) -> Self
    where
        P: super::traits::ErasedValueProxyBase
            + Clone
            + Send
            + Sync
            + fmt::Debug
            + PartialEq
            + 'static,
    {
        Self {
            storage: ValueStorage::Heap(Arc::new(ErasedProxyHolder { proxy })),
        }
    }

    /// Returns the dictionary if this value holds a HashMap<String, Value>
    /// or a Dictionary (stored by `from_dictionary`).
    ///
    /// Returns None if the value is not a dictionary type.
    #[inline]
    #[must_use]
    pub fn as_dictionary(&self) -> Option<HashMap<String, Value>> {
        // Try direct HashMap first
        if let Some(map) = self.get::<HashMap<String, Value>>() {
            return Some(map.clone());
        }
        // Try Dictionary (stored by from_dictionary / DictHolder)
        if let Some(dict) = self.get::<crate::dictionary::Dictionary>() {
            return Some(dict.to_hash_map());
        }
        None
    }

    /// Creates a clone of this value (identity conversion).
    ///
    /// This method exists for API compatibility where conversion from
    /// Value to Value is needed.
    #[inline]
    #[must_use]
    pub fn from_value(value: &Self) -> Self {
        value.clone()
    }

    /// Returns true if this value is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let empty = Value::empty();
    /// let full = Value::from(42i32);
    ///
    /// assert!(empty.is_empty());
    /// assert!(!full.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.storage, ValueStorage::Empty)
    }

    /// Returns true if this value holds type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::new(42i32);
    /// assert!(v.is::<i32>());
    /// assert!(!v.is::<f64>());
    /// ```
    #[inline]
    #[must_use]
    pub fn is<T: 'static>(&self) -> bool {
        let target = TypeId::of::<T>();
        match &self.storage {
            ValueStorage::Empty => false,
            ValueStorage::Inline(iv) => iv.type_id == target,
            ValueStorage::Heap(holder) => {
                if holder.held_type_id() == target {
                    true
                } else if holder.is_proxy() {
                    // Check if the proxy holds the requested type
                    holder.proxy_holds_type(target)
                } else {
                    false
                }
            }
        }
    }

    /// Returns the TypeId of the held type, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::TypeId;
    /// use usd_vt::Value;
    ///
    /// let v = Value::new(42i32);
    /// assert_eq!(v.held_type_id(), Some(TypeId::of::<i32>()));
    ///
    /// let empty = Value::empty();
    /// assert_eq!(empty.held_type_id(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn held_type_id(&self) -> Option<TypeId> {
        match &self.storage {
            ValueStorage::Empty => None,
            ValueStorage::Inline(iv) => Some(iv.type_id),
            ValueStorage::Heap(holder) => Some(holder.held_type_id()),
        }
    }

    /// Returns the type name for debugging purposes.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::new(42i32);
    /// assert!(v.type_name().unwrap().contains("i32"));
    /// ```
    #[inline]
    #[must_use]
    pub fn type_name(&self) -> Option<&'static str> {
        match &self.storage {
            ValueStorage::Empty => None,
            ValueStorage::Inline(iv) => Some(iv.vtable.type_name),
            ValueStorage::Heap(holder) => Some(holder.held_type_name()),
        }
    }

    /// Returns a reference to the held value if it matches type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::from(42i32);
    /// assert_eq!(v.get::<i32>(), Some(&42));
    /// assert_eq!(v.get::<f64>(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        match &self.storage {
            ValueStorage::Empty => None,
            ValueStorage::Inline(iv) => iv.get::<T>(),
            ValueStorage::Heap(holder) => holder.as_any().downcast_ref::<T>(),
        }
    }

    /// Returns a mutable reference to the held value if it matches type `T`.
    ///
    /// For inline-stored values (small Copy types), provides direct access.
    /// For heap-stored values, clones on write if the Arc is shared,
    /// then provides mutable access. Matches C++ in-place mutation.
    pub fn get_mut<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static>(
        &mut self,
    ) -> Option<&mut T> {
        match &mut self.storage {
            ValueStorage::Empty => None,
            ValueStorage::Inline(iv) => iv.get_mut::<T>(),
            ValueStorage::Heap(arc) => {
                // Ensure unique ownership: if shared, clone the holder
                if Arc::strong_count(arc) > 1 || Arc::weak_count(arc) > 0 {
                    let cloned = arc.clone_holder();
                    *arc = Arc::<dyn ValueHolder>::from(cloned);
                }
                // Now we have unique ownership, get mutable ref
                Arc::get_mut(arc).and_then(|holder| holder.as_any_mut().downcast_mut::<T>())
            }
        }
    }

    /// Alias for `get` - attempts to downcast to the specified type.
    ///
    /// Returns a reference to the held value if it matches type `T`.
    #[inline]
    #[must_use]
    pub fn downcast<T: 'static>(&self) -> Option<&T> {
        self.get::<T>()
    }

    /// Clones the underlying value if it's the specified type.
    ///
    /// Returns `None` if the value doesn't hold type `T`.
    /// Handles Dictionary <-> HashMap<String, Value> transparent conversion
    /// since `from(HashMap)` stores as Dictionary internally.
    #[must_use]
    pub fn downcast_clone<T: Clone + 'static>(&self) -> Option<T> {
        // Fast path: exact type match
        if let Some(val) = self.get::<T>() {
            return Some(val.clone());
        }
        // Dictionary -> HashMap<String, Value> transparent conversion
        if TypeId::of::<T>() == TypeId::of::<HashMap<String, Value>>() {
            if let Some(dict) = self.get::<crate::dictionary::Dictionary>() {
                let hash_map: HashMap<String, Value> = dict.to_hash_map();
                let boxed: Box<dyn Any> = Box::new(hash_map);
                return boxed.downcast::<T>().ok().map(|b| *b);
            }
        }
        None
    }

    /// Returns a cloned `Vec<T>` regardless of whether the value holds
    /// a `Vec<T>` or a `VtArray<T>` (`Array<T>`).
    ///
    /// USDC stores all geometry arrays as `Array<T>` (matching C++ `VtArray`),
    /// but callers often request `Vec<T>`. This bridges that gap without
    /// requiring callers to know which concrete type is stored.
    #[must_use]
    pub fn as_vec_clone<T: Clone + Send + Sync + 'static>(&self) -> Option<Vec<T>> {
        // Fast path: already a Vec<T>
        if let Some(v) = self.get::<Vec<T>>() {
            return Some(v.clone());
        }
        // Fallback: VtArray<T> — convert via slice
        if let Some(arr) = self.get::<crate::Array<T>>() {
            return Some(arr.as_slice().to_vec());
        }
        // USDA may store homogeneous arrays as Vec<Value>. Bridge common scalar
        // array cases so higher layers don't need format-specific fallbacks.
        if let Some(values) = self.get::<Vec<Value>>() {
            if TypeId::of::<T>() == TypeId::of::<usd_tf::Token>() {
                let tokens: Vec<usd_tf::Token> = values
                    .iter()
                    .map(|value| {
                        value
                            .get::<usd_tf::Token>()
                            .cloned()
                            .or_else(|| value.get::<String>().map(|text| usd_tf::Token::new(text)))
                    })
                    .collect::<Option<Vec<_>>>()?;
                let boxed: Box<dyn Any> = Box::new(tokens);
                return boxed.downcast::<Vec<T>>().ok().map(|boxed| *boxed);
            }
            if TypeId::of::<T>() == TypeId::of::<String>() {
                let strings: Vec<String> = values
                    .iter()
                    .map(|value| {
                        value
                            .get::<String>()
                            .cloned()
                            .or_else(|| value.get::<usd_tf::Token>().map(|token| token.to_string()))
                    })
                    .collect::<Option<Vec<_>>>()?;
                let boxed: Box<dyn Any> = Box::new(strings);
                return boxed.downcast::<Vec<T>>().ok().map(|boxed| *boxed);
            }
        }
        None
    }

    /// Extracts a clone of the value if type matches.
    ///
    /// Returns `Err(self)` if type doesn't match.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let v = Value::new(vec![1i32, 2, 3]);
    /// match v.try_into_inner::<Vec<i32>>() {
    ///     Ok(vec) => println!("Got vector: {:?}", vec),
    ///     Err(_) => println!("Could not extract"),
    /// }
    /// ```
    pub fn try_into_inner<T: Clone + 'static>(self) -> Result<T, Self> {
        if let Some(val) = self.get::<T>() {
            Ok(val.clone())
        } else {
            Err(self)
        }
    }

    /// Clears the value, making it empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let mut v = Value::from(42i32);
    /// assert!(!v.is_empty());
    ///
    /// v.clear();
    /// assert!(v.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.storage = ValueStorage::Empty;
    }

    /// Swaps the contents with another Value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let mut v1 = Value::from(1i32);
    /// let mut v2 = Value::from(2i32);
    ///
    /// v1.swap(&mut v2);
    ///
    /// assert_eq!(v1.get::<i32>(), Some(&2));
    /// assert_eq!(v2.get::<i32>(), Some(&1));
    /// ```
    #[inline]
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.storage, &mut other.storage);
    }

    /// Take ownership of a value, moving it into a Value.
    ///
    /// Matches C++ `VtValue::Take<T>(T &obj)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let mut obj = 42i32;
    /// let value = Value::take(&mut obj);
    /// assert_eq!(value.get::<i32>(), Some(&42));
    /// ```
    pub fn take<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static>(
        obj: &mut T,
    ) -> Self {
        let tmp = obj.clone();
        Self::new(tmp)
    }

    /// Swap the held value with a typed value.
    ///
    /// If this value is holding a T, swap with rhs. If this value is not
    /// holding a T, replace the held value with a value-initialized T instance
    /// first, then swap.
    ///
    /// Matches C++ `VtValue::Swap<T>(T &rhs)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let mut value = Value::from(1i32);
    /// let mut x = 2i32;
    /// value.swap_with(&mut x);
    /// assert_eq!(value.get::<i32>(), Some(&2));
    /// assert_eq!(x, 1);
    /// ```
    pub fn swap_with<T: Clone + Default + Send + Sync + fmt::Debug + PartialEq + Hash + 'static>(
        &mut self,
        rhs: &mut T,
    ) {
        if !self.is::<T>() {
            *self = Self::new(T::default());
        }
        self.unchecked_swap_with(rhs);
    }

    /// Swap the held value with a typed value.
    ///
    /// This Value must be holding an object of type T. If it does not,
    /// this invokes undefined behavior. Use `swap_with` if this Value
    /// is not known to contain an object of type T.
    ///
    /// Matches C++ `VtValue::UncheckedSwap<T>(T &rhs)`.
    pub fn unchecked_swap_with<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static>(
        &mut self,
        rhs: &mut T,
    ) {
        // In-place swap via get_mut (avoids clone for unique values)
        if let Some(held) = self.get_mut::<T>() {
            std::mem::swap(held, rhs);
        }
    }

    /// Make this value empty and return the held T instance.
    ///
    /// If this value does not hold a T instance, make this value empty and
    /// return a default-constructed T.
    ///
    /// Matches C++ `VtValue::Remove<T>()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let mut value = Value::from(42i32);
    /// let result: i32 = value.remove();
    /// assert_eq!(result, 42);
    /// assert!(value.is_empty());
    /// ```
    pub fn remove<T: Clone + Default + Send + Sync + fmt::Debug + PartialEq + Hash + 'static>(
        &mut self,
    ) -> T {
        let mut result = T::default();
        self.swap_with(&mut result);
        self.clear();
        result
    }

    /// Make this value empty and return the held T instance.
    ///
    /// If this value does not hold a T instance, this method invokes undefined behavior.
    /// Use `remove` if this Value is not known to contain an object of type T.
    ///
    /// Matches C++ `VtValue::UncheckedRemove<T>()`.
    pub fn unchecked_remove<
        T: Clone + Default + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
    >(
        &mut self,
    ) -> T {
        // Swap out value in-place (matching C++ UncheckedRemove which uses UncheckedSwap)
        let mut result = T::default();
        self.unchecked_swap_with(&mut result);
        self.clear();
        result
    }

    /// If this value holds an object of type T, invoke mutate_fn, passing
    /// it a mutable reference to the held object and return true.
    /// Otherwise do nothing and return false.
    ///
    /// Matches C++ `VtValue::Mutate<T, Fn>(Fn &&mutateFn)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Value;
    ///
    /// let mut value = Value::from(10i32);
    /// let mutated = value.mutate::<i32, _>(|x| *x *= 2);
    /// assert!(mutated);
    /// assert_eq!(value.get::<i32>(), Some(&20));
    /// ```
    pub fn mutate<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static, F>(
        &mut self,
        mutate_fn: F,
    ) -> bool
    where
        F: FnOnce(&mut T),
    {
        if !self.is::<T>() {
            return false;
        }
        self.unchecked_mutate::<T, _>(mutate_fn);
        true
    }

    /// Invoke mutate_fn, passing it a mutable reference to the held object
    /// which must be of type T. If the held object is not of type T,
    /// this function invokes undefined behavior.
    ///
    /// Matches C++ `VtValue::UncheckedMutate<T, Fn>(Fn &&mutateFn)`.
    pub fn unchecked_mutate<T: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static, F>(
        &mut self,
        mutate_fn: F,
    ) where
        F: FnOnce(&mut T),
    {
        // In-place mutation via get_mut (avoids clone for unique values)
        let held = self.get_mut::<T>().expect("Value must hold T");
        mutate_fn(held);
    }

    /// Internal method: Get raw pointer to held value for ValueRef.
    ///
    /// Returns (ptr, type_id, type_name) tuple if not empty.
    /// This is used internally by ValueRef to create efficient non-owning views.
    #[doc(hidden)]
    #[inline]
    pub fn as_raw_parts(&self) -> Option<(*const (), TypeId, &'static str)> {
        match &self.storage {
            ValueStorage::Empty => None,
            ValueStorage::Inline(iv) => {
                // Get pointer to inline data for debugging (safe operation on MaybeUninit)
                let ptr = iv.data.data.as_ptr() as *const ();
                Some((ptr, iv.type_id, iv.vtable.type_name))
            }
            ValueStorage::Heap(holder) => {
                let ptr = holder.as_any() as *const dyn Any as *const ();
                let type_id = holder.held_type_id();
                let type_name = holder.held_type_name();
                Some((ptr, type_id, type_name))
            }
        }
    }

    /// Returns true if this value holds a proxy object.
    ///
    /// Proxy objects lazily resolve to their underlying value.
    #[inline]
    #[must_use]
    pub fn is_proxy(&self) -> bool {
        match &self.storage {
            ValueStorage::Heap(holder) => holder.is_proxy(),
            _ => false,
        }
    }

    /// If this is a proxy, returns the resolved proxied value.
    ///
    /// For non-proxy values, returns None.
    #[inline]
    #[must_use]
    pub fn get_proxied_value(&self) -> Option<Value> {
        match &self.storage {
            ValueStorage::Heap(holder) if holder.is_proxy() => holder.get_proxied_value(),
            _ => None,
        }
    }

    /// Returns true if the value uses inline (stack) storage.
    #[inline]
    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(self.storage, ValueStorage::Inline(_))
    }

    // =========================================================================
    // C++ VtValue parity: IsArrayValued, IsArrayEditValued, GetArraySize,
    // Ref, GetKnownValueTypeIndex, CanTransform
    // =========================================================================

    /// Returns a non-owning reference to this value.
    ///
    /// Matches C++ `VtValue::Ref()`.
    #[inline]
    #[must_use]
    pub fn as_ref(&self) -> super::ValueRef<'_> {
        super::ValueRef::from(self)
    }

    /// Returns true if this value holds an array type.
    ///
    /// Matches C++ `VtValue::IsArrayValued()`.
    #[inline]
    #[must_use]
    pub fn is_array_valued(&self) -> bool {
        self.as_ref().is_array_valued()
    }

    /// Returns true if this value holds an ArrayEdit instance.
    ///
    /// Matches C++ `VtValue::IsArrayEditValued()`.
    #[inline]
    #[must_use]
    pub fn is_array_edit_valued(&self) -> bool {
        self.as_ref().is_array_edit_valued()
    }

    /// Returns the number of elements if this holds an array, 0 otherwise.
    ///
    /// Matches C++ `VtValue::GetArraySize()`.
    #[must_use]
    pub fn array_size(&self) -> usize {
        use super::visit_value::{ArraySizeVisitor, visit_value};
        visit_value(self, &mut ArraySizeVisitor)
    }

    /// Returns the known value type index if this type is in the USD type map.
    ///
    /// Matches C++ `VtValue::GetKnownValueTypeIndex()`. Returns `None` if
    /// the type is not a known USD value type (C++ returns -1).
    #[inline]
    #[must_use]
    pub fn get_known_value_type_index(&self) -> Option<usize> {
        self.held_type_id()
            .and_then(super::types::get_known_type_index_by_id)
    }

    /// Returns true if this value supports transforms.
    ///
    /// Matches C++ `VtValueRef::CanTransform()`.
    #[inline]
    #[must_use]
    pub fn can_transform(&self) -> bool {
        self.as_ref().can_transform()
    }

    /// Returns a copy of the held value if it is type T, otherwise returns `def`.
    ///
    /// Matches C++ `VtValue::GetWithDefault<T>(T const &def)`.
    #[inline]
    #[must_use]
    pub fn get_with_default<T: Clone + 'static>(&self, def: &T) -> T {
        self.get::<T>().cloned().unwrap_or_else(|| def.clone())
    }

    /// Returns the held value of type T, or the provided default if type doesn't match.
    ///
    /// Convenience owned-default variant of `get_with_default`.
    /// Returns the held value cloned if it matches type `T`, otherwise returns `default` as-is.
    #[inline]
    #[must_use]
    pub fn get_or<T: Clone + 'static>(&self, default: T) -> T {
        self.get::<T>().cloned().unwrap_or(default)
    }

    /// Returns true if this value can be hashed.
    ///
    /// All non-empty values support hashing. Matches C++ `VtValue::CanHash()`.
    #[inline]
    #[must_use]
    pub fn can_hash(&self) -> bool {
        !self.is_empty()
    }
}

impl Default for Value {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

// Inline From implementations for small Copy types
macro_rules! impl_value_from_inline {
    ($t:ty, $ctor:ident) => {
        impl From<$t> for Value {
            #[inline]
            fn from(value: $t) -> Self {
                Self {
                    storage: ValueStorage::Inline(InlineValue::$ctor(value)),
                }
            }
        }
    };
}

impl_value_from_inline!(bool, new_bool);
impl_value_from_inline!(i8, new_i8);
impl_value_from_inline!(i16, new_i16);
impl_value_from_inline!(i32, new_i32);
impl_value_from_inline!(i64, new_i64);
impl_value_from_inline!(u8, new_u8);
impl_value_from_inline!(u16, new_u16);
impl_value_from_inline!(u32, new_u32);
impl_value_from_inline!(u64, new_u64);

// Larger integer types go to heap (i128/u128 are 16 bytes, isize/usize vary)
macro_rules! impl_value_from_heap {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for Value {
                #[inline]
                fn from(value: $t) -> Self {
                    Self::new(value)
                }
            }
        )*
    };
}

impl_value_from_heap!(i128, isize, u128, usize, String);

impl From<usd_gf::half::Half> for Value {
    #[inline]
    fn from(value: usd_gf::half::Half) -> Self {
        Self::new(value)
    }
}

// Special From implementations for float types (inline)
impl From<f32> for Value {
    #[inline]
    fn from(value: f32) -> Self {
        Self::from_f32(value)
    }
}

impl From<f64> for Value {
    #[inline]
    fn from(value: f64) -> Self {
        Self::from_f64(value)
    }
}

impl From<&str> for Value {
    #[inline]
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

// From<usd_ar::ResolverContext> impl lives in usd-ar (avoids vt -> ar -> vt cycle).

impl From<crate::Dictionary> for Value {
    #[inline]
    fn from(value: crate::Dictionary) -> Self {
        Self::new(value)
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + PartialEq + std::hash::Hash + 'static>
    From<crate::Array<T>> for Value
{
    #[inline]
    fn from(value: crate::Array<T>) -> Self {
        Self::new(value)
    }
}

// From implementation for dictionary type (HashMap doesn't implement Hash)
impl From<HashMap<String, Value>> for Value {
    #[inline]
    fn from(value: HashMap<String, Value>) -> Self {
        Self::from_dictionary(value)
    }
}

// From implementation for Vec<String> (common metadata type)
impl From<Vec<String>> for Value {
    #[inline]
    fn from(value: Vec<String>) -> Self {
        Self::new(value)
    }
}

// From implementation for HashMap<String, String> (common metadata type like variantSelection)
impl From<HashMap<String, String>> for Value {
    #[inline]
    fn from(value: HashMap<String, String>) -> Self {
        // HashMap doesn't implement Hash, so use from_no_hash
        Self::from_no_hash(value)
    }
}

impl From<usd_tf::Token> for Value {
    #[inline]
    fn from(value: usd_tf::Token) -> Self {
        Self::new(value)
    }
}

impl From<crate::AssetPath> for Value {
    #[inline]
    fn from(value: crate::AssetPath) -> Self {
        Self::new(value)
    }
}

impl From<crate::TimeCode> for Value {
    #[inline]
    fn from(value: crate::TimeCode) -> Self {
        // TimeCode wraps f64 and uses bit-level hash; store without float hash special-casing.
        Self::new(value)
    }
}

impl From<Vec<usd_tf::Token>> for Value {
    #[inline]
    fn from(value: Vec<usd_tf::Token>) -> Self {
        Self::new(value)
    }
}

impl From<crate::spline::SplineValue> for Value {
    #[inline]
    fn from(value: crate::spline::SplineValue) -> Self {
        Self::from_spline(value)
    }
}

// From<usd_sdf::PathExpression> impl lives in usd-sdf (avoids vt -> sdf -> vt cycle).

// ArrayEdit<T> -> Value (no Hash on ArrayEdit, use from_no_hash)
impl<T: Clone + Send + Sync + Default + std::fmt::Debug + PartialEq + 'static>
    From<crate::ArrayEdit<T>> for Value
{
    #[inline]
    fn from(value: crate::ArrayEdit<T>) -> Self {
        Self::from_no_hash(value)
    }
}

// From implementations for Gf types (contain floats, so use from_no_hash)
macro_rules! impl_value_from_no_hash {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for Value {
                #[inline]
                fn from(value: $t) -> Self {
                    Self::from_no_hash(value)
                }
            }
        )*
    };
}

impl_value_from_no_hash!(
    // Vec2 types
    usd_gf::Vec2d,
    usd_gf::Vec2f,
    usd_gf::Vec2h,
    // Vec3 types
    usd_gf::Vec3d,
    usd_gf::Vec3f,
    usd_gf::Vec3h,
    // Vec4 types
    usd_gf::Vec4d,
    usd_gf::Vec4f,
    usd_gf::Vec4h,
    // Matrix types
    usd_gf::Matrix2d,
    usd_gf::Matrix2f,
    usd_gf::Matrix3d,
    usd_gf::Matrix3f,
    usd_gf::Matrix4d,
    usd_gf::Matrix4f,
    // Quaternion types
    usd_gf::Quatd,
    usd_gf::Quatf,
    usd_gf::Quath,
    usd_gf::Quaternion,
    // Dual quaternion types
    usd_gf::DualQuatd,
    usd_gf::DualQuatf,
    usd_gf::DualQuath,
    // Range types
    usd_gf::Range1d,
    usd_gf::Range1f,
    usd_gf::Range2d,
    usd_gf::Range2f,
    usd_gf::Range3d,
    usd_gf::Range3f,
    // Geometric types
    usd_gf::BBox3d,
    usd_gf::Rotation,
    usd_gf::Transform,
    usd_gf::Frustum,
    usd_gf::Plane,
    usd_gf::Ray,
    usd_gf::Line,
    usd_gf::LineSeg,
    usd_gf::Interval,
    usd_gf::MultiInterval,
    usd_gf::Camera,
    usd_gf::Color,
);

// Vec2i/Vec3i/Vec4i implement Hash (no floats)
impl From<usd_gf::Vec2i> for Value {
    #[inline]
    fn from(value: usd_gf::Vec2i) -> Self {
        Self::new(value)
    }
}

impl From<usd_gf::Vec3i> for Value {
    #[inline]
    fn from(value: usd_gf::Vec3i) -> Self {
        Self::new(value)
    }
}

impl From<usd_gf::Vec4i> for Value {
    #[inline]
    fn from(value: usd_gf::Vec4i) -> Self {
        Self::new(value)
    }
}

// Array From implementations for Gf types (for skeleton animation, etc.)
macro_rules! impl_value_from_vec_no_hash {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<Vec<$t>> for Value {
                #[inline]
                fn from(value: Vec<$t>) -> Self {
                    Self::from_no_hash(value)
                }
            }
        )*
    };
}

impl_value_from_vec_no_hash!(
    // Vec2 array types
    usd_gf::Vec2d,
    usd_gf::Vec2f,
    usd_gf::Vec2h,
    // Vec3 array types (for positions, translations, scales)
    usd_gf::Vec3d,
    usd_gf::Vec3f,
    usd_gf::Vec3h,
    // Vec4 array types (for dual quaternion skinning)
    usd_gf::Vec4d,
    usd_gf::Vec4f,
    usd_gf::Vec4h,
    // Quaternion array types (for rotations)
    usd_gf::Quatd,
    usd_gf::Quatf,
    usd_gf::Quath,
    // Matrix array types
    usd_gf::Matrix3f,
    usd_gf::Matrix4d,
    usd_gf::Matrix4f,
);

// Primitive and std vec types for instancer/primvar data sources
impl From<Vec<i32>> for Value {
    #[inline]
    fn from(value: Vec<i32>) -> Self {
        Self::new(value)
    }
}

impl From<Vec<f32>> for Value {
    #[inline]
    fn from(value: Vec<f32>) -> Self {
        Self::from_no_hash(value)
    }
}

impl From<Vec<bool>> for Value {
    #[inline]
    fn from(value: Vec<bool>) -> Self {
        Self::new(value)
    }
}

impl From<Vec<usd_gf::Vec2i>> for Value {
    #[inline]
    fn from(value: Vec<usd_gf::Vec2i>) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.storage {
            ValueStorage::Empty => write!(f, "Value(empty)"),
            ValueStorage::Inline(iv) => (iv.vtable.debug_fn)(&iv.data, f),
            ValueStorage::Heap(holder) => holder.debug_fmt(f),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.storage {
            ValueStorage::Empty => write!(f, "<empty>"),
            ValueStorage::Inline(iv) => (iv.vtable.debug_fn)(&iv.data, f),
            ValueStorage::Heap(holder) => holder.debug_fmt(f),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (&self.storage, &other.storage) {
            (ValueStorage::Empty, ValueStorage::Empty) => true,
            (ValueStorage::Inline(a), ValueStorage::Inline(b)) => {
                a.type_id == b.type_id && (a.vtable.eq_fn)(&a.data, &b.data)
            }
            (ValueStorage::Heap(a), ValueStorage::Heap(b)) => a.eq(b.as_ref()),
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.storage {
            ValueStorage::Empty => 0u64.hash(state),
            ValueStorage::Inline(iv) => {
                (iv.vtable.compute_hash_fn)(&iv.data).hash(state);
            }
            ValueStorage::Heap(holder) => {
                holder.compute_hash().hash(state);
            }
        }
    }
}

// SAFETY: Value's inline storage only holds Copy + Send + Sync types.
// Heap storage uses Arc<dyn ValueHolder> where ValueHolder: Send + Sync.
// The MaybeUninit wrapper doesn't change thread safety of stored types.
#[allow(unsafe_code)]
unsafe impl Send for Value {}

#[allow(unsafe_code)]
unsafe impl Sync for Value {}

// ============================================================================
// VtStreamOut - Type-erased value formatting
// Matches C++ VtStreamOut / Vt_StreamOutGeneric / VtStreamOutArray.
// ============================================================================

impl Value {
    /// Stream out this value as a string, matching C++ `VtStreamOut`.
    ///
    /// - For types that implement Display, outputs the raw value.
    /// - For empty values, returns "<empty>".
    /// - Fallback: `<'TypeName' @ 0xADDR>` matching C++ `Vt_StreamOutGeneric`.
    #[must_use]
    pub fn stream_out(&self) -> String {
        match &self.storage {
            ValueStorage::Empty => "<empty>".to_string(),
            ValueStorage::Inline(iv) => (iv.vtable.stream_out_fn)(&iv.data),
            ValueStorage::Heap(holder) => holder.stream_out_str(),
        }
    }
}

/// Generic stream-out fallback: `<'type_name' @ 0xADDR>`.
///
/// Matches C++ `Vt_StreamOutGeneric(type_info, addr, stream)`.
#[must_use]
pub fn stream_out_generic(type_name: &str, addr: *const ()) -> String {
    format!("<'{}' @ {:p}>", type_name, addr)
}

/// Pretty-print an array of displayable values as `[v1, v2, v3, ...]`.
///
/// If the array is longer than `max_items`, truncates with `...`.
/// Matches C++ `VtStreamOutArray` recursive shape formatting.
#[must_use]
pub fn stream_out_array<T: fmt::Display>(values: &[T], max_items: usize) -> String {
    let mut s = String::from('[');
    let len = values.len();
    let show = len.min(max_items);
    for (i, v) in values.iter().take(show).enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&format!("{}", v));
    }
    if len > max_items {
        s.push_str(", ...");
    }
    s.push(']');
    s
}

/// Pretty-print a shaped multi-dimensional array.
///
/// `values` is a flat buffer; `shape` describes dimensions (outer-to-inner).
/// Matches C++ `VtStreamOutArray` with `Vt_ShapeData` recursive formatting.
#[must_use]
pub fn stream_out_array_shaped<T: fmt::Display>(values: &[T], shape: &[usize]) -> String {
    if shape.is_empty() || values.is_empty() {
        return "[]".to_string();
    }
    let mut idx = 0;
    fn recurse<T: fmt::Display>(
        values: &[T],
        shape: &[usize],
        dim: usize,
        idx: &mut usize,
    ) -> String {
        let mut s = String::from('[');
        if dim == shape.len() - 1 {
            // Innermost dimension
            for j in 0..shape[dim] {
                if j > 0 {
                    s.push_str(", ");
                }
                if *idx < values.len() {
                    s.push_str(&format!("{}", values[*idx]));
                    *idx += 1;
                }
            }
        } else {
            for j in 0..shape[dim] {
                if j > 0 {
                    s.push_str(", ");
                }
                s.push_str(&recurse(values, shape, dim + 1, idx));
            }
        }
        s.push(']');
        s
    }
    recurse(values, shape, 0, &mut idx)
}

// ============================================================================
// Type Cast System
// Matches C++ Vt_CastRegistry singleton with runtime registration.
// ============================================================================

use std::sync::{LazyLock, RwLock};

use usd_gf::half::Half;
use usd_tf::Token;

/// Type alias for a cast function.
type CastFn = Box<dyn Fn(&Value) -> Option<Value> + Send + Sync>;

/// Global registry of cast functions. Auto-initialized with builtin numeric
/// and Token<->String casts on first access (matches C++ `_RegisterBuiltinCasts`).
static CAST_REGISTRY: LazyLock<RwLock<HashMap<(TypeId, TypeId), CastFn>>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    init_builtin_casts(&mut map);
    RwLock::new(map)
});

impl Value {
    /// Registers a cast function from type `From` to type `To`.
    ///
    /// The cast function takes a Value containing From and returns a Value
    /// containing To.
    ///
    /// Matches C++ `VtValue::RegisterCast<From, To>()`.
    pub fn register_cast<From, To, F>(cast_fn: F)
    where
        From: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
        To: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
        F: Fn(&From) -> To + Send + Sync + 'static,
    {
        let from_type = TypeId::of::<From>();
        let to_type = TypeId::of::<To>();

        let boxed_fn: CastFn = Box::new(move |value: &Value| {
            value.get::<From>().map(|from| {
                let result = cast_fn(from);
                Value::new(result)
            })
        });

        CAST_REGISTRY
            .write()
            .expect("rwlock poisoned")
            .insert((from_type, to_type), boxed_fn);
    }

    /// Registers a simple cast that uses From's Into<To> implementation.
    ///
    /// Matches C++ `VtValue::RegisterSimpleCast<From, To>()`.
    pub fn register_simple_cast<From, To>()
    where
        From: Clone + Into<To> + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
        To: Clone + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
    {
        Self::register_cast::<From, To, _>(|from: &From| from.clone().into());
    }

    /// Registers bidirectional simple casts between From and To.
    ///
    /// Matches C++ `VtValue::RegisterSimpleBidirectionalCast<From, To>()`.
    pub fn register_simple_bidirectional_cast<From, To>()
    where
        From: Clone + Into<To> + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
        To: Clone + Into<From> + Send + Sync + fmt::Debug + PartialEq + Hash + 'static,
    {
        Self::register_simple_cast::<From, To>();
        Self::register_cast::<To, From, _>(|to: &To| to.clone().into());
    }

    /// Returns whether this Value can be cast to type T.
    ///
    /// Matches C++ `VtValue::CanCast<T>()`.
    pub fn can_cast<T: 'static>(&self) -> bool {
        if self.is::<T>() {
            return true;
        }

        let Some(from_type) = self.held_type_id() else {
            return false;
        };
        let to_type = TypeId::of::<T>();

        CAST_REGISTRY
            .read()
            .expect("rwlock poisoned")
            .contains_key(&(from_type, to_type))
    }

    /// Returns whether a cast is registered from type From to type To.
    ///
    /// Matches C++ `VtValue::CanCastFromTypeidToTypeid()`.
    pub fn can_cast_types<From: 'static, To: 'static>() -> bool {
        if TypeId::of::<From>() == TypeId::of::<To>() {
            return true;
        }

        let from_type = TypeId::of::<From>();
        let to_type = TypeId::of::<To>();

        CAST_REGISTRY
            .read()
            .expect("rwlock poisoned")
            .contains_key(&(from_type, to_type))
    }

    /// Casts this Value to type T, returning a new Value.
    ///
    /// Returns None if the cast is not possible.
    ///
    /// Matches C++ `VtValue::Cast<T>()`.
    pub fn cast<T: 'static>(&self) -> Option<Value> {
        // If already the right type, return clone
        if self.is::<T>() {
            return Some(self.clone());
        }

        let Some(from_type) = self.held_type_id() else {
            return None;
        };
        let to_type = TypeId::of::<T>();

        // Look up cast function
        let registry = CAST_REGISTRY.read().expect("rwlock poisoned");
        let cast_fn = registry.get(&(from_type, to_type))?;
        cast_fn(self)
    }

    /// Casts this Value to the same type as `other`.
    ///
    /// Matches C++ `VtValue::CastToTypeOf(const VtValue &other)`.
    pub fn cast_to_type_of(&self, other: &Value) -> Option<Value> {
        let Some(to_type) = other.held_type_id() else {
            return None;
        };

        if self.held_type_id() == Some(to_type) {
            return Some(self.clone());
        }

        let Some(from_type) = self.held_type_id() else {
            return None;
        };

        let registry = CAST_REGISTRY.read().expect("rwlock poisoned");
        let cast_fn = registry.get(&(from_type, to_type))?;
        cast_fn(self)
    }

    /// Casts this Value to type T in place.
    ///
    /// Returns a mutable reference to self for chaining.
    /// If the cast fails, self becomes empty.
    ///
    /// Matches C++ `VtValue::Cast<T>()` (mutating version).
    pub fn cast_in_place<T: 'static>(&mut self) -> &mut Self {
        if self.is::<T>() {
            return self;
        }

        match self.cast::<T>() {
            Some(casted) => {
                *self = casted;
            }
            None => {
                self.clear();
            }
        }
        self
    }

    /// Casts this Value in place to the type identified by `type_id`.
    ///
    /// If the current type already matches, this is a no-op.
    /// If the cast fails or no cast is registered, self becomes empty.
    ///
    /// Matches C++ `VtValue::CastToTypeid(std::type_info const &type)`.
    pub fn cast_to_typeid(&mut self, type_id: TypeId) -> &mut Self {
        // No-op if already the right type
        if self.held_type_id() == Some(type_id) {
            return self;
        }

        let Some(from_type) = self.held_type_id() else {
            self.clear();
            return self;
        };

        let result = {
            let registry = CAST_REGISTRY.read().expect("rwlock poisoned");
            registry.get(&(from_type, type_id)).and_then(|f| f(self))
        };

        match result {
            Some(casted) => *self = casted,
            None => self.clear(),
        }
        self
    }

    /// Returns whether a cast is registered from `from` TypeId to `to` TypeId.
    ///
    /// Same types always return true (no cast needed).
    ///
    /// Matches C++ `VtValue::CanCastFromTypeidToTypeid(std::type_info const &from, std::type_info const &to)`.
    pub fn can_cast_from_typeid_to_typeid(from: TypeId, to: TypeId) -> bool {
        if from == to {
            return true;
        }
        CAST_REGISTRY
            .read()
            .expect("rwlock poisoned")
            .contains_key(&(from, to))
    }
}

/// Registers a one-way `as` cast into the map.
/// Registers a one-way cast: when Value holds A, produce B via `f` then wrap via `wrap`.
/// `wrap` converts B -> Value (e.g. `Value::from`).
fn reg_cast_with<A, B, F, W>(map: &mut HashMap<(TypeId, TypeId), CastFn>, f: F, wrap: W)
where
    A: Clone + Send + Sync + 'static,
    B: 'static,
    F: Fn(&A) -> B + Send + Sync + 'static,
    W: Fn(B) -> Value + Send + Sync + 'static,
{
    let cast_fn: CastFn = Box::new(move |v: &Value| v.get::<A>().map(|a| wrap(f(a))));
    map.insert((TypeId::of::<A>(), TypeId::of::<B>()), cast_fn);
}

/// Registers a one-way `as` cast. B must implement `Into<Value>`.
fn reg_as_cast<A, B, F>(map: &mut HashMap<(TypeId, TypeId), CastFn>, f: F)
where
    A: Clone + Send + Sync + 'static,
    B: Into<Value> + 'static,
    F: Fn(&A) -> B + Send + Sync + 'static,
{
    reg_cast_with(map, f, B::into);
}

/// Registers a bidirectional `as` cast pair into the map.
fn reg_as_pair<A, B>(map: &mut HashMap<(TypeId, TypeId), CastFn>, ab: fn(&A) -> B, ba: fn(&B) -> A)
where
    A: Into<Value> + Clone + Send + Sync + 'static,
    B: Into<Value> + Clone + Send + Sync + 'static,
{
    reg_as_cast::<A, B, _>(map, ab);
    reg_as_cast::<B, A, _>(map, ba);
}

/// Initialize builtin casts matching C++ `Vt_CastRegistry::_RegisterBuiltinCasts()`.
///
/// Registers exhaustive bidirectional numeric casts between:
/// bool, i8, u8, i16, u16, i32, u32, i64, u64, Half, f32, f64
/// Plus Token<->String conversions.
fn init_builtin_casts(map: &mut HashMap<(TypeId, TypeId), CastFn>) {
    // Macro for bidirectional `as` casts between two primitive types.
    macro_rules! reg_as {
        ($a:ty, $b:ty) => {
            reg_as_pair::<$a, $b>(map, |v| *v as $b, |v| *v as $a);
        };
    }

    // -- bool <-> all integer types (C++ _RegisterNumericCasts<bool, *>) --
    // Can't use reg_as! because Rust doesn't support `int as bool`.
    macro_rules! reg_bool_int {
        ($int:ty) => {
            reg_as_cast::<bool, $int, _>(map, |v| *v as $int);
            reg_as_cast::<$int, bool, _>(map, |v| *v != 0);
        };
    }
    reg_bool_int!(i8);
    reg_bool_int!(u8);
    reg_bool_int!(i16);
    reg_bool_int!(u16);
    reg_bool_int!(i32);
    reg_bool_int!(u32);
    reg_bool_int!(i64);
    reg_bool_int!(u64);
    // bool <-> Half
    reg_as_cast::<bool, Half, _>(map, |v| Half::from(*v as i32));
    reg_as_cast::<Half, bool, _>(map, |v| f32::from(*v) != 0.0);
    // bool <-> f32
    reg_as_cast::<bool, f32, _>(map, |v| if *v { 1.0 } else { 0.0 });
    reg_as_cast::<f32, bool, _>(map, |v| *v != 0.0);
    // bool <-> f64
    reg_as_cast::<bool, f64, _>(map, |v| if *v { 1.0 } else { 0.0 });
    reg_as_cast::<f64, bool, _>(map, |v| *v != 0.0);

    // -- i8 <-> all remaining types --
    reg_as!(i8, u8);
    reg_as!(i8, i16);
    reg_as!(i8, u16);
    reg_as!(i8, i32);
    reg_as!(i8, u32);
    reg_as!(i8, i64);
    reg_as!(i8, u64);
    reg_as_cast::<i8, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, i8, _>(map, |v| f32::from(*v) as i8);
    reg_as_cast::<i8, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, i8, _>(map, |v| *v as i8);
    reg_as_cast::<i8, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, i8, _>(map, |v| *v as i8);

    // -- u8 <-> remaining --
    reg_as!(u8, i16);
    reg_as!(u8, u16);
    reg_as!(u8, i32);
    reg_as!(u8, u32);
    reg_as!(u8, i64);
    reg_as!(u8, u64);
    reg_as_cast::<u8, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, u8, _>(map, |v| f32::from(*v) as u8);
    reg_as_cast::<u8, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, u8, _>(map, |v| *v as u8);
    reg_as_cast::<u8, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, u8, _>(map, |v| *v as u8);

    // -- i16 <-> remaining --
    reg_as!(i16, u16);
    reg_as!(i16, i32);
    reg_as!(i16, u32);
    reg_as!(i16, i64);
    reg_as!(i16, u64);
    reg_as_cast::<i16, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, i16, _>(map, |v| f32::from(*v) as i16);
    reg_as_cast::<i16, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, i16, _>(map, |v| *v as i16);
    reg_as_cast::<i16, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, i16, _>(map, |v| *v as i16);

    // -- u16 <-> remaining --
    reg_as!(u16, i32);
    reg_as!(u16, u32);
    reg_as!(u16, i64);
    reg_as!(u16, u64);
    reg_as_cast::<u16, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, u16, _>(map, |v| f32::from(*v) as u16);
    reg_as_cast::<u16, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, u16, _>(map, |v| *v as u16);
    reg_as_cast::<u16, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, u16, _>(map, |v| *v as u16);

    // -- i32 <-> remaining --
    reg_as!(i32, u32);
    reg_as!(i32, i64);
    reg_as!(i32, u64);
    reg_as_cast::<i32, Half, _>(map, |v| Half::from(*v));
    reg_as_cast::<Half, i32, _>(map, |v| f32::from(*v) as i32);
    reg_as_cast::<i32, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, i32, _>(map, |v| *v as i32);
    reg_as_cast::<i32, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, i32, _>(map, |v| *v as i32);

    // -- u32 <-> remaining --
    reg_as!(u32, i64);
    reg_as!(u32, u64);
    reg_as_cast::<u32, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, u32, _>(map, |v| f32::from(*v) as u32);
    reg_as_cast::<u32, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, u32, _>(map, |v| *v as u32);
    reg_as_cast::<u32, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, u32, _>(map, |v| *v as u32);

    // -- i64 <-> remaining --
    reg_as!(i64, u64);
    reg_as_cast::<i64, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, i64, _>(map, |v| f32::from(*v) as i64);
    reg_as_cast::<i64, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, i64, _>(map, |v| *v as i64);
    reg_as_cast::<i64, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, i64, _>(map, |v| *v as i64);

    // -- u64 <-> remaining --
    reg_as_cast::<u64, Half, _>(map, |v| Half::from(*v as f32));
    reg_as_cast::<Half, u64, _>(map, |v| f32::from(*v) as u64);
    reg_as_cast::<u64, f32, _>(map, |v| *v as f32);
    reg_as_cast::<f32, u64, _>(map, |v| *v as u64);
    reg_as_cast::<u64, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, u64, _>(map, |v| *v as u64);

    // -- Half <-> f32, f64 --
    reg_as_cast::<Half, f32, _>(map, |v| f32::from(*v));
    reg_as_cast::<f32, Half, _>(map, |v| Half::from(*v));
    reg_as_cast::<Half, f64, _>(map, |v| f64::from(f32::from(*v)));
    reg_as_cast::<f64, Half, _>(map, |v| Half::from(*v as f32));

    // -- f32 <-> f64 --
    reg_as_cast::<f32, f64, _>(map, |v| *v as f64);
    reg_as_cast::<f64, f32, _>(map, |v| *v as f32);

    // -- Token <-> String (matches C++ TfToken<->string) --
    reg_as_cast::<Token, String, _>(map, |v: &Token| v.as_str().to_owned());
    reg_as_cast::<String, Token, _>(map, |v: &String| Token::new(v));

    // -- String <-> numeric (convenient, not in C++ but useful for Rust) --
    reg_as_cast::<i32, String, _>(map, |v| v.to_string());
    reg_as_cast::<i64, String, _>(map, |v| v.to_string());
    reg_as_cast::<u32, String, _>(map, |v| v.to_string());
    reg_as_cast::<u64, String, _>(map, |v| v.to_string());
    reg_as_cast::<bool, String, _>(map, |v| v.to_string());
}

/// Initialize common type casts (triggers lazy init of CAST_REGISTRY).
///
/// This is a convenience function that ensures the registry is initialized.
/// The registry auto-initializes on first access, so calling this is optional.
pub fn register_common_casts() {
    // Force lazy init - all builtin casts are registered in the LazyLock.
    let _guard = CAST_REGISTRY.read();
    drop(_guard);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let v = Value::empty();
        assert!(v.is_empty());
        assert!(!v.is::<i32>());
        assert_eq!(v.get::<i32>(), None);
        assert_eq!(v.held_type_id(), None);
    }

    #[test]
    fn test_from_int() {
        let v = Value::from(42i32);
        assert!(!v.is_empty());
        assert!(v.is::<i32>());
        assert!(!v.is::<f64>());
        assert_eq!(v.get::<i32>(), Some(&42));
        assert_eq!(v.get::<f64>(), None);
    }

    #[test]
    fn test_from_float() {
        let v32 = Value::from(3.14f32);
        assert!(v32.is::<f32>());
        assert_eq!(v32.get::<f32>(), Some(&3.14f32));

        let v64 = Value::from(3.14159f64);
        assert!(v64.is::<f64>());
        assert_eq!(v64.get::<f64>(), Some(&3.14159f64));
    }

    #[test]
    fn test_from_string() {
        let v = Value::from("hello".to_string());
        assert!(v.is::<String>());
        assert_eq!(v.get::<String>(), Some(&"hello".to_string()));
    }

    #[test]
    fn test_clone() {
        let v1 = Value::from(42i32);
        let v2 = v1.clone();
        assert_eq!(v1.get::<i32>(), v2.get::<i32>());
    }

    #[test]
    fn test_equality() {
        let v1 = Value::from(42i32);
        let v2 = Value::from(42i32);
        let v3 = Value::from(43i32);
        let v4 = Value::from(42.0f64);

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
        assert_ne!(v1, v4); // Different types
    }

    #[test]
    fn test_float_equality() {
        let v1 = Value::from(3.14f32);
        let v2 = Value::from(3.14f32);
        let v3 = Value::from(2.71f32);

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_empty_equality() {
        let e1 = Value::empty();
        let e2 = Value::empty();
        let v = Value::from(42i32);

        assert_eq!(e1, e2);
        assert_ne!(e1, v);
    }

    #[test]
    fn test_clear() {
        let mut v = Value::from(42i32);
        assert!(!v.is_empty());
        v.clear();
        assert!(v.is_empty());
    }

    #[test]
    fn test_vt_value_parity_methods() {
        use super::super::Array;

        let scalar = Value::from(42i32);
        assert!(!scalar.is_array_valued());
        assert!(!scalar.is_array_edit_valued());
        assert_eq!(scalar.array_size(), 0);
        assert_eq!(scalar.get::<i32>(), scalar.as_ref().get::<i32>());
        assert_eq!(scalar.get_known_value_type_index(), Some(5)); // i32 in types map
        assert!(scalar.can_transform());

        let arr: Array<i32> = vec![1, 2, 3].into();
        let arr_val = Value::from(arr);
        assert!(arr_val.is_array_valued());
        assert_eq!(arr_val.array_size(), 3);
        assert!(arr_val.as_ref().get::<Array<i32>>().is_some());

        let empty = Value::empty();
        assert!(empty.as_ref().is_empty());
        assert_eq!(empty.get_known_value_type_index(), None);
    }

    #[test]
    fn test_swap() {
        let mut v1 = Value::from(1i32);
        let mut v2 = Value::from("hello".to_string());

        v1.swap(&mut v2);

        assert!(v1.is::<String>());
        assert!(v2.is::<i32>());
    }

    #[test]
    fn test_try_into_inner() {
        let v = Value::new(vec![1i32, 2, 3]);
        let result = v.try_into_inner::<Vec<i32>>();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_hash() {
        use std::collections::hash_map::DefaultHasher;

        let v1 = Value::from(42i32);
        let v2 = Value::from(42i32);

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();

        v1.hash(&mut h1);
        v2.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_float_hash() {
        use std::collections::hash_map::DefaultHasher;

        let v1 = Value::from(3.14f64);
        let v2 = Value::from(3.14f64);

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();

        v1.hash(&mut h1);
        v2.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_spline_value() {
        use crate::spline::{SplineCurveType, SplineExtrapolation, SplineKnot, SplineValue};

        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.set_pre_extrapolation(SplineExtrapolation::Held);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        let v = Value::from(spline.clone());
        assert!(v.is::<SplineValue>());
        assert_eq!(v.get::<SplineValue>().unwrap().num_knots(), 2);

        // Test from_spline method
        let v2 = Value::from_spline(spline.clone());
        assert!(v2.is::<SplineValue>());
        assert_eq!(v2.get::<SplineValue>().unwrap().num_knots(), 2);

        // Test equality
        assert_eq!(v, v2);
    }

    // H2: Inline storage tests
    #[test]
    fn test_inline_bool() {
        let v = Value::from(true);
        assert!(v.is_inline());
        assert!(v.is::<bool>());
        assert_eq!(v.get::<bool>(), Some(&true));
    }

    #[test]
    fn test_inline_i32() {
        let v = Value::from(42i32);
        assert!(v.is_inline());
        assert!(v.is::<i32>());
        assert_eq!(v.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_inline_f64() {
        let v = Value::from(3.14159f64);
        assert!(v.is_inline());
        assert!(v.is::<f64>());
        assert_eq!(v.get::<f64>(), Some(&3.14159f64));
    }

    #[test]
    fn test_inline_clone() {
        let v1 = Value::from(42i32);
        let v2 = v1.clone();
        assert!(v2.is_inline());
        assert_eq!(v1, v2);
        assert_eq!(v1.get::<i32>(), v2.get::<i32>());
    }

    #[test]
    fn test_inline_equality() {
        let a = Value::from(100u64);
        let b = Value::from(100u64);
        let c = Value::from(200u64);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_inline_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;

        let v1 = Value::from(77i64);
        let v2 = Value::from(77i64);

        let hash1 = {
            let mut h = DefaultHasher::new();
            v1.hash(&mut h);
            h.finish()
        };
        let hash2 = {
            let mut h = DefaultHasher::new();
            v2.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_inline_different_types_not_equal() {
        let vi = Value::from(42i32);
        let vu = Value::from(42u32);
        assert_ne!(vi, vu);
    }

    #[test]
    fn test_heap_string_not_inline() {
        let v = Value::from("test".to_string());
        assert!(!v.is_inline());
        assert!(v.is::<String>());
    }

    // H3: Proxy tests
    #[test]
    fn test_typed_proxy() {
        use crate::traits::{GetProxiedObject, TypedValueProxyBase};

        // Define a test typed proxy
        #[derive(Clone, Debug, PartialEq)]
        struct MyProxy {
            inner: i32,
        }
        impl TypedValueProxyBase for MyProxy {}
        impl GetProxiedObject for MyProxy {
            type Proxied = i32;
            fn get_proxied(&self) -> &i32 {
                &self.inner
            }
        }

        let proxy = MyProxy { inner: 42 };
        let v = Value::from_typed_proxy(proxy);

        assert!(v.is_proxy());
        assert!(v.is::<MyProxy>());
        // Check that proxy_holds_type detects the proxied type
        assert!(v.is::<i32>());

        // Get the proxied value
        let resolved = v.get_proxied_value().unwrap();
        assert_eq!(resolved.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_erased_proxy() {
        use crate::traits::ErasedValueProxyBase;

        #[derive(Clone, Debug, PartialEq)]
        struct LazyValue {
            resolved: Value,
        }
        impl ErasedValueProxyBase for LazyValue {
            fn get_erased_proxied_value(&self) -> Value {
                self.resolved.clone()
            }
        }

        let proxy = LazyValue {
            resolved: Value::from(99i32),
        };
        let v = Value::from_erased_proxy(proxy);

        assert!(v.is_proxy());
        let resolved = v.get_proxied_value().unwrap();
        assert_eq!(resolved.get::<i32>(), Some(&99));
    }

    #[test]
    fn test_non_proxy_is_not_proxy() {
        let v = Value::from(42i32);
        assert!(!v.is_proxy());
        assert_eq!(v.get_proxied_value(), None);
    }

    // =====================================================================
    // H4: Cast registry tests
    // =====================================================================

    #[test]
    fn test_cast_i32_to_i64() {
        let v = Value::from(42i32);
        let casted = v.cast::<i64>().unwrap();
        assert_eq!(casted.get::<i64>(), Some(&42i64));
    }

    #[test]
    fn test_cast_i64_to_i32() {
        let v = Value::from(100i64);
        let casted = v.cast::<i32>().unwrap();
        assert_eq!(casted.get::<i32>(), Some(&100i32));
    }

    #[test]
    fn test_cast_f32_to_f64() {
        let v = Value::from(3.14f32);
        let casted = v.cast::<f64>().unwrap();
        let result = *casted.get::<f64>().unwrap();
        assert!((result - 3.14f64).abs() < 0.001);
    }

    #[test]
    fn test_cast_f64_to_f32() {
        let v = Value::from(2.71f64);
        let casted = v.cast::<f32>().unwrap();
        let result = *casted.get::<f32>().unwrap();
        assert!((result - 2.71f32).abs() < 0.001);
    }

    #[test]
    fn test_cast_bool_to_i32() {
        let v = Value::from(true);
        let casted = v.cast::<i32>().unwrap();
        assert_eq!(casted.get::<i32>(), Some(&1i32));

        let v2 = Value::from(false);
        let casted2 = v2.cast::<i32>().unwrap();
        assert_eq!(casted2.get::<i32>(), Some(&0i32));
    }

    #[test]
    fn test_cast_i32_to_bool() {
        let v = Value::from(0i32);
        let casted = v.cast::<bool>().unwrap();
        assert_eq!(casted.get::<bool>(), Some(&false));

        let v2 = Value::from(42i32);
        let casted2 = v2.cast::<bool>().unwrap();
        assert_eq!(casted2.get::<bool>(), Some(&true));
    }

    #[test]
    fn test_cast_half_to_f32() {
        use usd_gf::half::Half;
        let h = Half::from(1.5f32);
        let v = Value::new(h);
        let casted = v.cast::<f32>().unwrap();
        assert_eq!(casted.get::<f32>(), Some(&1.5f32));
    }

    #[test]
    fn test_cast_f32_to_half() {
        use usd_gf::half::Half;
        let v = Value::from(2.0f32);
        let casted = v.cast::<Half>().unwrap();
        assert_eq!(casted.get::<Half>().unwrap().to_f32(), 2.0f32);
    }

    #[test]
    fn test_cast_half_to_f64() {
        use usd_gf::half::Half;
        let h = Half::from(3.0f32);
        let v = Value::new(h);
        let casted = v.cast::<f64>().unwrap();
        assert_eq!(casted.get::<f64>(), Some(&3.0f64));
    }

    #[test]
    fn test_cast_token_to_string() {
        use usd_tf::Token;
        let tok = Token::new("hello");
        let v = Value::new(tok);
        let casted = v.cast::<String>().unwrap();
        assert_eq!(casted.get::<String>(), Some(&"hello".to_string()));
    }

    #[test]
    fn test_cast_string_to_token() {
        use usd_tf::Token;
        let v = Value::from("world".to_string());
        let casted = v.cast::<Token>().unwrap();
        assert_eq!(casted.get::<Token>().unwrap().as_str(), "world");
    }

    #[test]
    fn test_cast_bidirectional_token_string() {
        use usd_tf::Token;
        let tok = Token::new("round_trip");
        let v = Value::new(tok);
        let as_string = v.cast::<String>().unwrap();
        let back = as_string.cast::<Token>().unwrap();
        assert_eq!(back.get::<Token>().unwrap().as_str(), "round_trip");
    }

    #[test]
    fn test_can_cast_builtin() {
        let v = Value::from(42i32);
        assert!(v.can_cast::<i64>());
        assert!(v.can_cast::<f64>());
        assert!(v.can_cast::<bool>());
        assert!(v.can_cast::<i32>()); // same type
    }

    #[test]
    fn test_can_cast_types_builtin() {
        assert!(Value::can_cast_types::<i32, i64>());
        assert!(Value::can_cast_types::<f32, f64>());
        assert!(Value::can_cast_types::<bool, i32>());
        assert!(Value::can_cast_types::<i32, i32>()); // same type
    }

    #[test]
    fn test_cast_empty_returns_none() {
        let v = Value::empty();
        assert!(v.cast::<i32>().is_none());
    }

    #[test]
    fn test_cast_same_type_returns_clone() {
        let v = Value::from(42i32);
        let casted = v.cast::<i32>().unwrap();
        assert_eq!(casted.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_cast_unregistered_returns_none() {
        // Vec<i32> -> i32 is not registered
        let v = Value::new(vec![1i32, 2, 3]);
        assert!(v.cast::<i32>().is_none());
    }

    #[test]
    fn test_cast_in_place_success() {
        let mut v = Value::from(42i32);
        v.cast_in_place::<i64>();
        assert!(v.is::<i64>());
        assert_eq!(v.get::<i64>(), Some(&42i64));
    }

    #[test]
    fn test_cast_in_place_failure_clears() {
        let mut v = Value::new(vec![1i32, 2, 3]);
        v.cast_in_place::<i32>();
        assert!(v.is_empty());
    }

    #[test]
    fn test_cast_to_type_of() {
        let v1 = Value::from(42i32);
        let v2 = Value::from(0i64);
        let casted = v1.cast_to_type_of(&v2).unwrap();
        assert!(casted.is::<i64>());
        assert_eq!(casted.get::<i64>(), Some(&42i64));
    }

    #[test]
    fn test_register_custom_cast() {
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        struct Celsius(i64);
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        struct Fahrenheit(i64);

        Value::register_cast::<Celsius, Fahrenheit, _>(|c| Fahrenheit(c.0 * 9 / 5 + 32));

        let c = Value::new(Celsius(100));
        let f = c.cast::<Fahrenheit>().unwrap();
        assert_eq!(f.get::<Fahrenheit>().unwrap().0, 212);
    }

    #[test]
    fn test_register_bidirectional_custom_cast() {
        #[derive(Clone, Debug, PartialEq, Hash)]
        struct Meters(i64);
        #[derive(Clone, Debug, PartialEq, Hash)]
        struct Feet(i64);

        Value::register_cast::<Meters, Feet, _>(|m| Feet(m.0 * 3));
        Value::register_cast::<Feet, Meters, _>(|f| Meters(f.0 / 3));

        let m = Value::new(Meters(10));
        let f = m.cast::<Feet>().unwrap();
        assert_eq!(f.get::<Feet>().unwrap().0, 30);

        let back = f.cast::<Meters>().unwrap();
        assert_eq!(back.get::<Meters>().unwrap().0, 10);
    }

    #[test]
    fn test_builtin_i32_to_string() {
        let v = Value::from(42i32);
        let s = v.cast::<String>().unwrap();
        assert_eq!(s.get::<String>(), Some(&"42".to_string()));
    }

    #[test]
    fn test_cast_u8_to_u64() {
        let v = Value::new(255u8);
        let casted = v.cast::<u64>().unwrap();
        assert_eq!(casted.get::<u64>(), Some(&255u64));
    }

    #[test]
    fn test_cast_i16_to_f64() {
        let v = Value::new(1000i16);
        let casted = v.cast::<f64>().unwrap();
        assert_eq!(casted.get::<f64>(), Some(&1000.0f64));
    }

    // VtStreamOut tests
    #[test]
    fn test_stream_out_primitives() {
        let v_int = Value::from(42i32);
        assert_eq!(v_int.stream_out(), "42");

        let v_float = Value::from(3.14f64);
        assert!(v_float.stream_out().starts_with("3.14"));

        let v_bool = Value::from(true);
        assert_eq!(v_bool.stream_out(), "true");

        let v_empty = Value::empty();
        assert_eq!(v_empty.stream_out(), "<empty>");
    }

    #[test]
    fn test_stream_out_string() {
        let v = Value::from("hello".to_string());
        let s = v.stream_out();
        assert!(s.contains("hello"));
    }

    #[test]
    fn test_stream_out_generic() {
        use super::stream_out_generic;
        let x = 42i32;
        let addr = &x as *const i32 as *const ();
        let result = stream_out_generic("i32", addr);
        assert!(result.starts_with("<'i32' @ 0x"));
        assert!(result.ends_with('>'));
    }

    #[test]
    fn test_stream_out_array_basic() {
        use super::stream_out_array;
        let vals = [1, 2, 3, 4, 5];
        assert_eq!(stream_out_array(&vals, 10), "[1, 2, 3, 4, 5]");
        assert_eq!(stream_out_array(&vals, 3), "[1, 2, 3, ...]");
        assert_eq!(stream_out_array::<i32>(&[], 10), "[]");
    }

    #[test]
    fn test_stream_out_array_shaped() {
        use super::stream_out_array_shaped;
        // 2x3 matrix
        let vals = [1, 2, 3, 4, 5, 6];
        let result = stream_out_array_shaped(&vals, &[2, 3]);
        assert_eq!(result, "[[1, 2, 3], [4, 5, 6]]");

        // 1D
        let result_1d = stream_out_array_shaped(&[10, 20], &[2]);
        assert_eq!(result_1d, "[10, 20]");

        // Empty
        assert_eq!(stream_out_array_shaped::<i32>(&[], &[]), "[]");
    }

    // =========================================================================
    // M-vt: get_mut - in-place mutation without clone
    // =========================================================================

    #[test]
    fn test_get_mut_inline() {
        // Inline (Copy) types should support direct mutation
        let mut v = Value::from(42i32);
        if let Some(val) = v.get_mut::<i32>() {
            *val = 100;
        }
        assert_eq!(v.get::<i32>(), Some(&100));
    }

    #[test]
    fn test_get_mut_heap() {
        // Heap types (String) should also support mutation
        let mut v = Value::from("hello".to_string());
        if let Some(val) = v.get_mut::<String>() {
            val.push_str(" world");
        }
        assert_eq!(v.get::<String>(), Some(&"hello world".to_string()));
    }

    #[test]
    fn test_get_mut_wrong_type() {
        let mut v = Value::from(42i32);
        assert!(v.get_mut::<i64>().is_none());
    }

    #[test]
    fn test_unchecked_swap_with_no_clone() {
        // Verifies swap works via get_mut path
        let mut v = Value::from(10i32);
        let mut rhs = 20i32;
        v.unchecked_swap_with(&mut rhs);
        assert_eq!(v.get::<i32>(), Some(&20));
        assert_eq!(rhs, 10);
    }

    #[test]
    fn test_unchecked_swap_with_heap() {
        let mut v = Value::from("abc".to_string());
        let mut rhs = "xyz".to_string();
        v.unchecked_swap_with(&mut rhs);
        assert_eq!(v.get::<String>(), Some(&"xyz".to_string()));
        assert_eq!(rhs, "abc");
    }

    #[test]
    fn test_unchecked_mutate_in_place() {
        let mut v = Value::from(7i32);
        v.unchecked_mutate::<i32, _>(|x| *x *= 3);
        assert_eq!(v.get::<i32>(), Some(&21));
    }

    #[test]
    fn test_unchecked_remove_no_clone() {
        let mut v = Value::from(42i32);
        let result: i32 = v.unchecked_remove();
        assert_eq!(result, 42);
        assert!(v.is_empty());
    }

    #[test]
    fn test_remove_heap_type() {
        let mut v = Value::from("test".to_string());
        let result: String = v.remove();
        assert_eq!(result, "test");
        assert!(v.is_empty());
    }
}
