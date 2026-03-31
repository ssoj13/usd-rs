//! Visitor pattern for type-erased Value inspection.
//!
//! This module provides a visitor pattern for efficiently dispatching on the
//! concrete type held within a `Value`. This is the Rust equivalent of OpenUSD's
//! `VtVisitValue` functionality.
//!
//! The visitor pattern allows handling different types without repeated downcasting
//! or type checking. It's more efficient than chained `if value.is::<T>()` calls
//! and allows handling related types with a single method.
//!
//! # Examples
//!
//! ## Basic Visitor
//!
//! ```
//! use usd_vt::{Value, visit_value, ValueVisitor};
//!
//! struct TypePrinter;
//!
//! impl ValueVisitor for TypePrinter {
//!     type Output = String;
//!
//!     fn visit_bool(&mut self, v: bool) -> Self::Output {
//!         format!("bool: {}", v)
//!     }
//!
//!     fn visit_int(&mut self, v: i32) -> Self::Output {
//!         format!("int: {}", v)
//!     }
//!
//!     fn visit_string(&mut self, v: &str) -> Self::Output {
//!         format!("string: {}", v)
//!     }
//!
//!     fn visit_unknown(&mut self, _v: &Value) -> Self::Output {
//!         "unknown type".to_string()
//!     }
//! }
//!
//! let val = Value::from(42i32);
//! let result = visit_value(&val, &mut TypePrinter);
//! assert_eq!(result, "int: 42");
//! ```
//!
//! ## Array Size Visitor
//!
//! ```
//! use usd_vt::{Value, Array, visit_value, ValueVisitor};
//!
//! struct ArraySizeVisitor;
//!
//! impl ValueVisitor for ArraySizeVisitor {
//!     type Output = Option<usize>;
//!
//!     fn visit_array<T: Clone + Send + Sync + 'static>(
//!         &mut self,
//!         arr: &Array<T>,
//!     ) -> Self::Output {
//!         Some(arr.len())
//!     }
//!
//!     fn visit_unknown(&mut self, _v: &Value) -> Self::Output {
//!         None
//!     }
//! }
//!
//! let arr: Array<i32> = vec![1, 2, 3, 4, 5].into();
//! let val = Value::from(arr);
//! let size = visit_value(&val, &mut ArraySizeVisitor);
//! assert_eq!(size, Some(5));
//! ```

use crate::{Array, AssetPath, Dictionary, TimeCode, Value};
use usd_gf::{
    DualQuatd, DualQuatf, DualQuath, Half, Interval, Matrix2d, Matrix2f, Matrix3d, Matrix3f,
    Matrix4d, Matrix4f, Quatd, Quaternion, Quatf, Quath, Range1d, Range1f, Range2d, Range2f,
    Range3d, Range3f, Rect2i, Vec2d, Vec2f, Vec2h, Vec2i, Vec3d, Vec3f, Vec3h, Vec3i, Vec4d, Vec4f,
    Vec4h, Vec4i,
};
use usd_tf::Token;

/// Visitor trait for inspecting Value contents.
///
/// Implement this trait to handle different types held within a `Value`.
/// Each method corresponds to a common USD type. The `visit_unknown` method
/// is called when the Value contains a type not explicitly handled.
///
/// All visit methods have default implementations that delegate to `visit_unknown`,
/// so you only need to implement the types you care about.
///
/// # Type Parameter
///
/// - `Output`: The return type of visit methods. Can be `()`, or any other type
///   depending on what you want to collect/compute during visitation.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, ValueVisitor};
///
/// struct IsNumeric;
///
/// impl ValueVisitor for IsNumeric {
///     type Output = bool;
///
///     fn visit_int(&mut self, _: i32) -> bool { true }
///     fn visit_int64(&mut self, _: i64) -> bool { true }
///     fn visit_float(&mut self, _: f32) -> bool { true }
///     fn visit_double(&mut self, _: f64) -> bool { true }
///     fn visit_unknown(&mut self, _: &Value) -> bool { false }
/// }
/// ```
pub trait ValueVisitor {
    /// The return type of visit methods.
    type Output;

    // =========================================================================
    // Scalar types
    // =========================================================================

    /// Visit bool value.
    #[allow(unused_variables)]
    fn visit_bool(&mut self, v: bool) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit i8 (char) value.
    #[allow(unused_variables)]
    fn visit_char(&mut self, v: i8) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit u8 (unsigned char) value.
    #[allow(unused_variables)]
    fn visit_uchar(&mut self, v: u8) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit i16 (short) value.
    #[allow(unused_variables)]
    fn visit_short(&mut self, v: i16) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit u16 (unsigned short) value.
    #[allow(unused_variables)]
    fn visit_ushort(&mut self, v: u16) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit i32 (int) value.
    #[allow(unused_variables)]
    fn visit_int(&mut self, v: i32) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit u32 (unsigned int) value.
    #[allow(unused_variables)]
    fn visit_uint(&mut self, v: u32) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit i64 value.
    #[allow(unused_variables)]
    fn visit_int64(&mut self, v: i64) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit u64 value.
    #[allow(unused_variables)]
    fn visit_uint64(&mut self, v: u64) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit f32 (float) value.
    #[allow(unused_variables)]
    fn visit_float(&mut self, v: f32) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit f64 (double) value.
    #[allow(unused_variables)]
    fn visit_double(&mut self, v: f64) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit half-precision float value.
    #[allow(unused_variables)]
    fn visit_half(&mut self, v: Half) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit string value.
    #[allow(unused_variables)]
    fn visit_string(&mut self, v: &str) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Token value.
    #[allow(unused_variables)]
    fn visit_token(&mut self, v: &Token) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit AssetPath value.
    #[allow(unused_variables)]
    fn visit_asset_path(&mut self, v: &AssetPath) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit TimeCode value.
    #[allow(unused_variables)]
    fn visit_time_code(&mut self, v: TimeCode) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    // =========================================================================
    // Vector types
    // =========================================================================

    /// Visit Vec2i value.
    #[allow(unused_variables)]
    fn visit_vec2i(&mut self, v: Vec2i) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec2f value.
    #[allow(unused_variables)]
    fn visit_vec2f(&mut self, v: Vec2f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec2d value.
    #[allow(unused_variables)]
    fn visit_vec2d(&mut self, v: Vec2d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec2h value.
    #[allow(unused_variables)]
    fn visit_vec2h(&mut self, v: Vec2h) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec3i value.
    #[allow(unused_variables)]
    fn visit_vec3i(&mut self, v: Vec3i) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec3f value.
    #[allow(unused_variables)]
    fn visit_vec3f(&mut self, v: Vec3f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec3d value.
    #[allow(unused_variables)]
    fn visit_vec3d(&mut self, v: Vec3d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec3h value.
    #[allow(unused_variables)]
    fn visit_vec3h(&mut self, v: Vec3h) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec4i value.
    #[allow(unused_variables)]
    fn visit_vec4i(&mut self, v: Vec4i) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec4f value.
    #[allow(unused_variables)]
    fn visit_vec4f(&mut self, v: Vec4f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec4d value.
    #[allow(unused_variables)]
    fn visit_vec4d(&mut self, v: Vec4d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Vec4h value.
    #[allow(unused_variables)]
    fn visit_vec4h(&mut self, v: Vec4h) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    // =========================================================================
    // Matrix types
    // =========================================================================

    /// Visit Matrix2f value.
    #[allow(unused_variables)]
    fn visit_matrix2f(&mut self, v: &Matrix2f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Matrix2d value.
    #[allow(unused_variables)]
    fn visit_matrix2d(&mut self, v: &Matrix2d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Matrix3f value.
    #[allow(unused_variables)]
    fn visit_matrix3f(&mut self, v: &Matrix3f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Matrix3d value.
    #[allow(unused_variables)]
    fn visit_matrix3d(&mut self, v: &Matrix3d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Matrix4f value.
    #[allow(unused_variables)]
    fn visit_matrix4f(&mut self, v: &Matrix4f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Matrix4d value.
    #[allow(unused_variables)]
    fn visit_matrix4d(&mut self, v: &Matrix4d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    // =========================================================================
    // Quaternion types
    // =========================================================================

    /// Visit Quatf value.
    #[allow(unused_variables)]
    fn visit_quatf(&mut self, v: Quatf) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Quatd value.
    #[allow(unused_variables)]
    fn visit_quatd(&mut self, v: Quatd) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Quath value.
    #[allow(unused_variables)]
    fn visit_quath(&mut self, v: Quath) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit DualQuatf value.
    #[allow(unused_variables)]
    fn visit_dual_quatf(&mut self, v: DualQuatf) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit DualQuatd value.
    #[allow(unused_variables)]
    fn visit_dual_quatd(&mut self, v: DualQuatd) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit DualQuath value.
    #[allow(unused_variables)]
    fn visit_dual_quath(&mut self, v: DualQuath) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit legacy Quaternion value.
    #[allow(unused_variables)]
    fn visit_quaternion(&mut self, v: Quaternion) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    // =========================================================================
    // Range types
    // =========================================================================

    /// Visit Range1f value.
    #[allow(unused_variables)]
    fn visit_range1f(&mut self, v: Range1f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Range1d value.
    #[allow(unused_variables)]
    fn visit_range1d(&mut self, v: Range1d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Range2f value.
    #[allow(unused_variables)]
    fn visit_range2f(&mut self, v: Range2f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Range2d value.
    #[allow(unused_variables)]
    fn visit_range2d(&mut self, v: Range2d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Range3f value.
    #[allow(unused_variables)]
    fn visit_range3f(&mut self, v: Range3f) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Range3d value.
    #[allow(unused_variables)]
    fn visit_range3d(&mut self, v: Range3d) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Interval value.
    #[allow(unused_variables)]
    fn visit_interval(&mut self, v: Interval) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Rect2i value.
    #[allow(unused_variables)]
    fn visit_rect2i(&mut self, v: Rect2i) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    // =========================================================================
    // Container types
    // =========================================================================

    /// Visit typed Array value.
    ///
    /// This is a generic method that handles all Array<T> types.
    /// Override this to handle arrays without caring about element type.
    #[allow(unused_variables)]
    fn visit_array<T: Clone + Send + Sync + 'static>(&mut self, arr: &Array<T>) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    /// Visit Dictionary value.
    #[allow(unused_variables)]
    fn visit_dictionary(&mut self, dict: &Dictionary) -> Self::Output {
        self.visit_unknown(&Value::empty())
    }

    // =========================================================================
    // Fallback
    // =========================================================================

    /// Visit unknown or unhandled type.
    ///
    /// This method must be implemented. It's called when:
    /// - The Value is empty
    /// - The Value contains a type not explicitly handled
    /// - A default visit method is invoked
    fn visit_unknown(&mut self, v: &Value) -> Self::Output;
}

/// Dispatch to the appropriate visitor method based on the Value's held type.
///
/// This function inspects the concrete type held by `value` and calls the
/// corresponding method on `visitor`. If the type is not recognized or the
/// Value is empty, `visitor.visit_unknown()` is called.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, visit_value, ValueVisitor};
///
/// struct Counter {
///     count: usize,
/// }
///
/// impl ValueVisitor for Counter {
///     type Output = ();
///
///     fn visit_int(&mut self, _: i32) {
///         self.count += 1;
///     }
///
///     fn visit_unknown(&mut self, _: &Value) {}
/// }
///
/// let mut counter = Counter { count: 0 };
/// visit_value(&Value::from(42i32), &mut counter);
/// visit_value(&Value::from(100i32), &mut counter);
/// assert_eq!(counter.count, 2);
/// ```
pub fn visit_value<V: ValueVisitor>(value: &Value, visitor: &mut V) -> V::Output {
    use std::any::TypeId;

    if value.is_empty() {
        return visitor.visit_unknown(value);
    }

    let type_id = value.held_type_id().expect("not empty");

    // Macro to simplify type dispatch
    macro_rules! dispatch {
        ($ty:ty, $method:ident) => {
            if type_id == TypeId::of::<$ty>() {
                if let Some(val) = value.get::<$ty>() {
                    return visitor.$method(*val);
                }
            }
        };
        ($ty:ty, $method:ident, ref) => {
            if type_id == TypeId::of::<$ty>() {
                if let Some(val) = value.get::<$ty>() {
                    return visitor.$method(val);
                }
            }
        };
    }

    // Scalar types
    dispatch!(bool, visit_bool);
    dispatch!(i8, visit_char);
    dispatch!(u8, visit_uchar);
    dispatch!(i16, visit_short);
    dispatch!(u16, visit_ushort);
    dispatch!(i32, visit_int);
    dispatch!(u32, visit_uint);
    dispatch!(i64, visit_int64);
    dispatch!(u64, visit_uint64);
    dispatch!(f32, visit_float);
    dispatch!(f64, visit_double);
    dispatch!(Half, visit_half);
    dispatch!(String, visit_string, ref);
    dispatch!(Token, visit_token, ref);
    dispatch!(AssetPath, visit_asset_path, ref);
    dispatch!(TimeCode, visit_time_code);

    // Vector types
    dispatch!(Vec2i, visit_vec2i);
    dispatch!(Vec2f, visit_vec2f);
    dispatch!(Vec2d, visit_vec2d);
    dispatch!(Vec2h, visit_vec2h);
    dispatch!(Vec3i, visit_vec3i);
    dispatch!(Vec3f, visit_vec3f);
    dispatch!(Vec3d, visit_vec3d);
    dispatch!(Vec3h, visit_vec3h);
    dispatch!(Vec4i, visit_vec4i);
    dispatch!(Vec4f, visit_vec4f);
    dispatch!(Vec4d, visit_vec4d);
    dispatch!(Vec4h, visit_vec4h);

    // Matrix types
    dispatch!(Matrix2f, visit_matrix2f, ref);
    dispatch!(Matrix2d, visit_matrix2d, ref);
    dispatch!(Matrix3f, visit_matrix3f, ref);
    dispatch!(Matrix3d, visit_matrix3d, ref);
    dispatch!(Matrix4f, visit_matrix4f, ref);
    dispatch!(Matrix4d, visit_matrix4d, ref);

    // Quaternion types
    dispatch!(Quatf, visit_quatf);
    dispatch!(Quatd, visit_quatd);
    dispatch!(Quath, visit_quath);
    dispatch!(DualQuatf, visit_dual_quatf);
    dispatch!(DualQuatd, visit_dual_quatd);
    dispatch!(DualQuath, visit_dual_quath);
    dispatch!(Quaternion, visit_quaternion);

    // Range types
    dispatch!(Range1f, visit_range1f);
    dispatch!(Range1d, visit_range1d);
    dispatch!(Range2f, visit_range2f);
    dispatch!(Range2d, visit_range2d);
    dispatch!(Range3f, visit_range3f);
    dispatch!(Range3d, visit_range3d);
    dispatch!(Interval, visit_interval);
    dispatch!(Rect2i, visit_rect2i);

    // Container types
    dispatch!(Dictionary, visit_dictionary, ref);

    // Array types - check all common array types
    macro_rules! dispatch_array {
        ($elem_ty:ty) => {
            if type_id == TypeId::of::<Array<$elem_ty>>() {
                if let Some(arr) = value.get::<Array<$elem_ty>>() {
                    return visitor.visit_array(arr);
                }
            }
        };
    }

    // Scalar arrays
    dispatch_array!(bool);
    dispatch_array!(i8);
    dispatch_array!(u8);
    dispatch_array!(i16);
    dispatch_array!(u16);
    dispatch_array!(i32);
    dispatch_array!(u32);
    dispatch_array!(i64);
    dispatch_array!(u64);
    dispatch_array!(f32);
    dispatch_array!(f64);
    dispatch_array!(Half);
    dispatch_array!(String);
    dispatch_array!(Token);
    dispatch_array!(AssetPath);
    dispatch_array!(TimeCode);

    // Vector arrays
    dispatch_array!(Vec2i);
    dispatch_array!(Vec2f);
    dispatch_array!(Vec2d);
    dispatch_array!(Vec2h);
    dispatch_array!(Vec3i);
    dispatch_array!(Vec3f);
    dispatch_array!(Vec3d);
    dispatch_array!(Vec3h);
    dispatch_array!(Vec4i);
    dispatch_array!(Vec4f);
    dispatch_array!(Vec4d);
    dispatch_array!(Vec4h);

    // Matrix arrays
    dispatch_array!(Matrix2f);
    dispatch_array!(Matrix2d);
    dispatch_array!(Matrix3f);
    dispatch_array!(Matrix3d);
    dispatch_array!(Matrix4f);
    dispatch_array!(Matrix4d);

    // Quaternion arrays
    dispatch_array!(Quatf);
    dispatch_array!(Quatd);
    dispatch_array!(Quath);
    dispatch_array!(DualQuatf);
    dispatch_array!(DualQuatd);
    dispatch_array!(DualQuath);
    dispatch_array!(Quaternion);

    // Range arrays
    dispatch_array!(Range1f);
    dispatch_array!(Range1d);
    dispatch_array!(Range2f);
    dispatch_array!(Range2d);
    dispatch_array!(Range3f);
    dispatch_array!(Range3d);
    dispatch_array!(Interval);
    dispatch_array!(Rect2i);

    // Unknown type - call fallback
    visitor.visit_unknown(value)
}

// =============================================================================
// Example Visitors
// =============================================================================

/// Visitor that returns the array size for array values, 0 for non-arrays.
///
/// Matches C++ `VtValue::GetArraySize()`.
///
/// # Examples
///
/// ```
/// use usd_vt::{Array, Value, visit_value, ArraySizeVisitor};
///
/// let arr: Array<i32> = vec![1, 2, 3].into();
/// let val = Value::from(arr);
/// assert_eq!(visit_value(&val, &mut ArraySizeVisitor), 3);
///
/// let scalar = Value::from(42i32);
/// assert_eq!(visit_value(&scalar, &mut ArraySizeVisitor), 0);
/// ```
pub struct ArraySizeVisitor;

impl ValueVisitor for ArraySizeVisitor {
    type Output = usize;

    fn visit_array<T: Clone + Send + Sync + 'static>(&mut self, arr: &Array<T>) -> Self::Output {
        arr.len()
    }

    fn visit_unknown(&mut self, _: &Value) -> Self::Output {
        0
    }
}

/// Example visitor that prints values to a string.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, visit_value, PrintVisitor};
///
/// let mut visitor = PrintVisitor::new();
/// let result = visit_value(&Value::from(42i32), &mut visitor);
/// assert_eq!(result, "42");
/// ```
pub struct PrintVisitor;

impl PrintVisitor {
    /// Creates a new PrintVisitor.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrintVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueVisitor for PrintVisitor {
    type Output = String;

    fn visit_bool(&mut self, v: bool) -> String {
        v.to_string()
    }

    fn visit_int(&mut self, v: i32) -> String {
        v.to_string()
    }

    fn visit_int64(&mut self, v: i64) -> String {
        v.to_string()
    }

    fn visit_uint(&mut self, v: u32) -> String {
        v.to_string()
    }

    fn visit_uint64(&mut self, v: u64) -> String {
        v.to_string()
    }

    fn visit_float(&mut self, v: f32) -> String {
        v.to_string()
    }

    fn visit_double(&mut self, v: f64) -> String {
        v.to_string()
    }

    fn visit_string(&mut self, v: &str) -> String {
        format!("\"{}\"", v)
    }

    fn visit_token(&mut self, v: &Token) -> String {
        format!("Token({})", v.as_str())
    }

    fn visit_array<T: Clone + Send + Sync + 'static>(&mut self, arr: &Array<T>) -> String {
        format!("Array[{}]", arr.len())
    }

    fn visit_dictionary(&mut self, dict: &Dictionary) -> String {
        format!("Dictionary{{{}entries}}", dict.len())
    }

    fn visit_unknown(&mut self, v: &Value) -> String {
        if v.is_empty() {
            "empty".to_string()
        } else {
            format!("<{}>", v.type_name().unwrap_or("unknown"))
        }
    }
}

/// Example visitor that collects type names.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, visit_value, TypeCollectorVisitor};
///
/// let mut visitor = TypeCollectorVisitor::new();
/// visit_value(&Value::from(42i32), &mut visitor);
/// visit_value(&Value::from(3.14f64), &mut visitor);
/// assert_eq!(visitor.types(), &["i32", "f64"]);
/// ```
pub struct TypeCollectorVisitor {
    types: Vec<String>,
}

impl TypeCollectorVisitor {
    /// Creates a new TypeCollectorVisitor.
    pub fn new() -> Self {
        Self { types: Vec::new() }
    }

    /// Returns the collected type names.
    pub fn types(&self) -> &[String] {
        &self.types
    }
}

impl Default for TypeCollectorVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueVisitor for TypeCollectorVisitor {
    type Output = ();

    fn visit_bool(&mut self, _: bool) {
        self.types.push("bool".to_string());
    }

    fn visit_int(&mut self, _: i32) {
        self.types.push("i32".to_string());
    }

    fn visit_int64(&mut self, _: i64) {
        self.types.push("i64".to_string());
    }

    fn visit_uint(&mut self, _: u32) {
        self.types.push("u32".to_string());
    }

    fn visit_uint64(&mut self, _: u64) {
        self.types.push("u64".to_string());
    }

    fn visit_float(&mut self, _: f32) {
        self.types.push("f32".to_string());
    }

    fn visit_double(&mut self, _: f64) {
        self.types.push("f64".to_string());
    }

    fn visit_string(&mut self, _: &str) {
        self.types.push("String".to_string());
    }

    fn visit_token(&mut self, _: &Token) {
        self.types.push("Token".to_string());
    }

    fn visit_array<T: Clone + Send + Sync + 'static>(&mut self, _: &Array<T>) {
        self.types
            .push(format!("Array<{}>", std::any::type_name::<T>()));
    }

    fn visit_dictionary(&mut self, _: &Dictionary) {
        self.types.push("Dictionary".to_string());
    }

    fn visit_unknown(&mut self, v: &Value) {
        if v.is_empty() {
            self.types.push("empty".to_string());
        } else {
            self.types
                .push(format!("unknown<{}>", v.type_name().unwrap_or("?")));
        }
    }
}

/// Example visitor that computes a hash of the value.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, visit_value, HashVisitor};
///
/// let val1 = Value::from(42i32);
/// let val2 = Value::from(42i32);
/// let val3 = Value::from(43i32);
///
/// let hash1 = visit_value(&val1, &mut HashVisitor::new());
/// let hash2 = visit_value(&val2, &mut HashVisitor::new());
/// let hash3 = visit_value(&val3, &mut HashVisitor::new());
///
/// assert_eq!(hash1, hash2);
/// assert_ne!(hash1, hash3);
/// ```
pub struct HashVisitor {
    hash: u64,
}

impl HashVisitor {
    /// Creates a new HashVisitor.
    pub fn new() -> Self {
        Self { hash: 0 }
    }

    /// Updates the hash with a value.
    fn update<H: std::hash::Hash>(&mut self, v: &H) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        self.hash ^= hasher.finish();
    }
}

impl Default for HashVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueVisitor for HashVisitor {
    type Output = u64;

    fn visit_bool(&mut self, v: bool) -> u64 {
        self.update(&v);
        self.hash
    }

    fn visit_int(&mut self, v: i32) -> u64 {
        self.update(&v);
        self.hash
    }

    fn visit_int64(&mut self, v: i64) -> u64 {
        self.update(&v);
        self.hash
    }

    fn visit_uint(&mut self, v: u32) -> u64 {
        self.update(&v);
        self.hash
    }

    fn visit_uint64(&mut self, v: u64) -> u64 {
        self.update(&v);
        self.hash
    }

    fn visit_float(&mut self, v: f32) -> u64 {
        self.update(&v.to_bits());
        self.hash
    }

    fn visit_double(&mut self, v: f64) -> u64 {
        self.update(&v.to_bits());
        self.hash
    }

    fn visit_string(&mut self, v: &str) -> u64 {
        self.update(&v);
        self.hash
    }

    fn visit_unknown(&mut self, _: &Value) -> u64 {
        self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visit_scalars() {
        let mut visitor = PrintVisitor::new();

        assert_eq!(visit_value(&Value::from(true), &mut visitor), "true");
        assert_eq!(visit_value(&Value::from(42i32), &mut visitor), "42");
        assert_eq!(visit_value(&Value::from(100i64), &mut visitor), "100");
        assert_eq!(visit_value(&Value::from(3.14f32), &mut visitor), "3.14");
        assert_eq!(visit_value(&Value::from(2.71f64), &mut visitor), "2.71");
        assert_eq!(
            visit_value(&Value::from("hello".to_string()), &mut visitor),
            "\"hello\""
        );
    }

    #[test]
    fn test_visit_array() {
        let arr: Array<i32> = vec![1, 2, 3, 4, 5].into();
        let val = Value::from(arr);

        let mut visitor = PrintVisitor::new();
        let result = visit_value(&val, &mut visitor);
        assert_eq!(result, "Array[5]");
    }

    #[test]
    fn test_visit_empty() {
        let val = Value::empty();
        let mut visitor = PrintVisitor::new();
        let result = visit_value(&val, &mut visitor);
        assert_eq!(result, "empty");
    }

    #[test]
    fn test_type_collector() {
        let mut visitor = TypeCollectorVisitor::new();

        visit_value(&Value::from(42i32), &mut visitor);
        visit_value(&Value::from(3.14f64), &mut visitor);
        visit_value(&Value::from("test".to_string()), &mut visitor);

        let types = visitor.types();
        assert_eq!(types.len(), 3);
        assert_eq!(types[0], "i32");
        assert_eq!(types[1], "f64");
        assert_eq!(types[2], "String");
    }

    #[test]
    fn test_hash_visitor() {
        let val1 = Value::from(42i32);
        let val2 = Value::from(42i32);
        let val3 = Value::from(43i32);

        let hash1 = visit_value(&val1, &mut HashVisitor::new());
        let hash2 = visit_value(&val2, &mut HashVisitor::new());
        let hash3 = visit_value(&val3, &mut HashVisitor::new());

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_asset_path_dispatch() {
        // Verify AssetPath and TimeCode are properly dispatched by visit_value.
        struct TypeIdCollector(Vec<&'static str>);

        impl ValueVisitor for TypeIdCollector {
            type Output = ();
            fn visit_asset_path(&mut self, _: &AssetPath) -> () {
                self.0.push("AssetPath");
            }
            fn visit_time_code(&mut self, _: TimeCode) -> () {
                self.0.push("TimeCode");
            }
            fn visit_unknown(&mut self, _: &Value) -> () {
                self.0.push("unknown");
            }
        }

        let ap = AssetPath::new("model.usd");
        let val = Value::from(ap);
        let mut visitor = TypeIdCollector(vec![]);
        visit_value(&val, &mut visitor);
        assert_eq!(visitor.0, ["AssetPath"]);

        let tc = TimeCode::new(42.0);
        let val = Value::from(tc);
        let mut visitor = TypeIdCollector(vec![]);
        visit_value(&val, &mut visitor);
        assert_eq!(visitor.0, ["TimeCode"]);
    }

    #[test]
    fn test_custom_visitor() {
        struct IsNumeric;

        impl ValueVisitor for IsNumeric {
            type Output = bool;

            fn visit_int(&mut self, _: i32) -> bool {
                true
            }

            fn visit_int64(&mut self, _: i64) -> bool {
                true
            }

            fn visit_float(&mut self, _: f32) -> bool {
                true
            }

            fn visit_double(&mut self, _: f64) -> bool {
                true
            }

            fn visit_unknown(&mut self, _: &Value) -> bool {
                false
            }
        }

        assert!(visit_value(&Value::from(42i32), &mut IsNumeric));
        assert!(visit_value(&Value::from(3.14f64), &mut IsNumeric));
        assert!(!visit_value(
            &Value::from("not a number".to_string()),
            &mut IsNumeric
        ));
    }

    #[test]
    fn test_array_size_visitor() {
        struct ArraySizeVisitor;

        impl ValueVisitor for ArraySizeVisitor {
            type Output = Option<usize>;

            fn visit_array<T: Clone + Send + Sync + 'static>(
                &mut self,
                arr: &Array<T>,
            ) -> Option<usize> {
                Some(arr.len())
            }

            fn visit_unknown(&mut self, _: &Value) -> Option<usize> {
                None
            }
        }

        let arr: Array<i32> = vec![1, 2, 3, 4, 5].into();
        let val = Value::from(arr);
        assert_eq!(visit_value(&val, &mut ArraySizeVisitor), Some(5));

        let not_array = Value::from(42i32);
        assert_eq!(visit_value(&not_array, &mut ArraySizeVisitor), None);
    }
}
