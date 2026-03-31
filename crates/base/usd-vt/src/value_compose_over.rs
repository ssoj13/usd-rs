//! Value composition for USD layer composition.
//!
//! This module provides composition semantics for `Value` types, which is
//! critical for USD's layer composition system. When stronger layers compose
//! over weaker layers, values are combined according to type-specific rules.
//!
//! # Composition Rules
//!
//! - **Scalars**: Stronger value always wins
//! - **Arrays**: Stronger array completely replaces weaker array
//! - **Dictionaries**: Recursive merge, stronger keys override weaker keys
//! - **Empty values**: Empty stronger returns weaker, empty weaker returns stronger
//!
//! # Examples
//!
//! ```
//! use usd_vt::{Value, Dictionary, value_compose_over};
//!
//! // Scalars - stronger wins
//! let stronger = Value::from(42i32);
//! let weaker = Value::from(10i32);
//! let result = value_compose_over(&stronger, &weaker);
//! assert_eq!(result.get::<i32>(), Some(&42));
//!
//! // Dictionaries - recursive merge
//! let mut dict1 = Dictionary::new();
//! dict1.insert("a", 1i32);
//! dict1.insert("b", 2i32);
//!
//! let mut dict2 = Dictionary::new();
//! dict2.insert("b", 20i32);
//! dict2.insert("c", 30i32);
//!
//! let v1 = Value::new(dict1);
//! let v2 = Value::new(dict2);
//! let result = value_compose_over(&v1, &v2);
//!
//! if let Some(dict) = result.get::<Dictionary>() {
//!     assert_eq!(dict.get_as::<i32>("a"), Some(&1));  // from stronger
//!     assert_eq!(dict.get_as::<i32>("b"), Some(&2));  // stronger wins
//!     assert_eq!(dict.get_as::<i32>("c"), Some(&30)); // from weaker
//! }
//! ```

use super::{Array, ArrayEdit, Dictionary, Value};
use std::any::TypeId;

// ============================================================================
// BackgroundType sentinel (C++ VtBackgroundType / VtBackground)
// ============================================================================

/// Sentinel type for value composition indicating "background" (default) layer.
///
/// Matches C++ `VtBackgroundType` from `valueComposeOver.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BackgroundType;

/// Global sentinel matching C++ `inline constexpr VtBackgroundType VtBackground`.
pub const VT_BACKGROUND: BackgroundType = BackgroundType;

/// Trait for types that support composition.
///
/// Types implementing this trait can be composed together, with a "stronger"
/// value composing over a "weaker" value. This is used extensively in USD's
/// layer composition system.
///
/// # Associativity
///
/// Implementations MUST be associative:
/// ```text
/// ((A over B) over C) == (A over (B over C))
/// ```
///
/// # Examples
///
/// ```
/// use usd_vt::{ValueComposable, Dictionary};
///
/// let mut dict1 = Dictionary::new();
/// dict1.insert("x", 1i32);
///
/// let mut dict2 = Dictionary::new();
/// dict2.insert("y", 2i32);
///
/// let result = dict1.compose_over(dict2);
/// assert!(result.contains_key("x"));
/// assert!(result.contains_key("y"));
/// ```
pub trait ValueComposable: Sized {
    /// Returns true if this value always dominates in composition.
    ///
    /// If this returns true, `compose_over(self, weaker)` always returns `self`
    /// regardless of `weaker`.
    fn can_compose_over(&self) -> bool {
        true
    }

    /// Composes this value over a weaker value.
    ///
    /// Returns the result of composing `self` (stronger) over `weaker`.
    /// The stronger value generally takes precedence, but type-specific
    /// rules may apply (e.g., dictionary merging).
    fn compose_over(self, weaker: Self) -> Self;
}

/// Composes a stronger value over a weaker value.
///
/// This is the main entry point for value composition. Returns the result
/// of composing `stronger` over `weaker` according to type-specific rules:
///
/// - If `stronger` is empty, returns clone of `weaker`
/// - If `weaker` is empty, returns clone of `stronger`
/// - If types match, applies type-specific composition
/// - If types differ, returns clone of `stronger`
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_compose_over};
///
/// let stronger = Value::from(100i32);
/// let weaker = Value::from(50i32);
/// let result = value_compose_over(&stronger, &weaker);
/// assert_eq!(result.get::<i32>(), Some(&100));
///
/// // Empty stronger returns weaker
/// let empty = Value::empty();
/// let result = value_compose_over(&empty, &weaker);
/// assert_eq!(result.get::<i32>(), Some(&50));
/// ```
pub fn value_compose_over(stronger: &Value, weaker: &Value) -> Value {
    // If stronger is empty, return weaker
    if stronger.is_empty() {
        return weaker.clone();
    }

    // ArrayEdit<T> composes over Array<T>, ArrayEdit<T>, or empty
    // Must check BEFORE type_id comparison since ArrayEdit<T> != Array<T>
    if stronger.is_array_edit_valued() {
        if let Some(result) = try_compose_array_edit(stronger, weaker) {
            return result;
        }
    }

    // If weaker is empty, return stronger
    if weaker.is_empty() {
        return stronger.clone();
    }

    // If types don't match, stronger wins
    if stronger.held_type_id() != weaker.held_type_id() {
        return stronger.clone();
    }

    // Dictionary has special recursive merge behavior
    if let Some(dict_s) = stronger.get::<Dictionary>() {
        if let Some(dict_w) = weaker.get::<Dictionary>() {
            return Value::new(dict_s.clone().compose_over(dict_w.clone()));
        }
    }

    // For all other types (including Array), stronger replaces weaker
    stronger.clone()
}

/// Returns true if value can compose over other values.
///
/// This is a fast check to determine if `value_compose_over(val, x)` would
/// return `val` for all `x`. Empty values can always compose over.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_can_compose_over};
///
/// let v = Value::from(42i32);
/// assert!(value_can_compose_over(&v));
///
/// let empty = Value::empty();
/// assert!(value_can_compose_over(&empty));
/// ```
pub fn value_can_compose_over(val: &Value) -> bool {
    // Empty values can compose over
    if val.is_empty() {
        return true;
    }

    // Dictionaries and ArrayEdits compose non-trivially
    if val.is::<Dictionary>() || val.is_array_edit_valued() {
        return true;
    }
    true
}

/// Attempts to compose two values if they can compose non-trivially.
///
/// Returns `Some(result)` if composition is non-trivial, `None` if
/// `stronger` would simply replace `weaker`.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, Dictionary, value_try_compose_over};
///
/// // Dictionaries compose non-trivially
/// let mut d1 = Dictionary::new();
/// d1.insert("a", 1i32);
/// let mut d2 = Dictionary::new();
/// d2.insert("b", 2i32);
///
/// let v1 = Value::new(d1);
/// let v2 = Value::new(d2);
/// assert!(value_try_compose_over(&v1, &v2).is_some());
///
/// // Scalars don't compose non-trivially
/// let v3 = Value::from(42i32);
/// let v4 = Value::from(10i32);
/// assert!(value_try_compose_over(&v3, &v4).is_none());
/// ```
pub fn value_try_compose_over(stronger: &Value, weaker: &Value) -> Option<Value> {
    // If stronger is empty, composition is non-trivial (returns weaker)
    if stronger.is_empty() && !weaker.is_empty() {
        return Some(weaker.clone());
    }

    // ArrayEdit composes non-trivially over Array, ArrayEdit, or empty
    // Must check BEFORE type_id comparison since ArrayEdit<T> != Array<T>
    if stronger.is_array_edit_valued() {
        if let Some(result) = try_compose_array_edit(stronger, weaker) {
            return Some(result);
        }
    }

    // If types don't match or weaker is empty, composition is trivial
    if weaker.is_empty() || stronger.held_type_id() != weaker.held_type_id() {
        return None;
    }

    // Dictionaries compose non-trivially
    if stronger.is::<Dictionary>() && weaker.is::<Dictionary>() {
        return Some(value_compose_over(stronger, weaker));
    }

    // All other types: composition is trivial (stronger wins)
    None
}

/// Returns true if values of the given type may compose non-trivially.
///
/// Matches C++ `VtValueTypeCanComposeOver(std::type_info const&)`.
pub fn value_type_can_compose_over(type_id: TypeId) -> bool {
    if type_id == TypeId::of::<Dictionary>() {
        return true;
    }

    macro_rules! matches_array_edit_type {
        ($elem:ty) => {
            if type_id == TypeId::of::<ArrayEdit<$elem>>() {
                return true;
            }
        };
    }

    matches_array_edit_type!(bool);
    matches_array_edit_type!(u8);
    matches_array_edit_type!(i32);
    matches_array_edit_type!(u32);
    matches_array_edit_type!(i64);
    matches_array_edit_type!(u64);
    matches_array_edit_type!(f32);
    matches_array_edit_type!(f64);
    matches_array_edit_type!(String);
    matches_array_edit_type!(usd_tf::Token);
    matches_array_edit_type!(usd_gf::Vec2f);
    matches_array_edit_type!(usd_gf::Vec3f);
    matches_array_edit_type!(usd_gf::Vec4f);
    matches_array_edit_type!(usd_gf::Vec2d);
    matches_array_edit_type!(usd_gf::Vec3d);
    matches_array_edit_type!(usd_gf::Vec4d);
    matches_array_edit_type!(usd_gf::Vec2i);
    matches_array_edit_type!(usd_gf::Vec3i);
    matches_array_edit_type!(usd_gf::Vec4i);
    matches_array_edit_type!(usd_gf::Quatf);
    matches_array_edit_type!(usd_gf::Quatd);
    matches_array_edit_type!(usd_gf::Quath);
    matches_array_edit_type!(usd_gf::Matrix2d);
    matches_array_edit_type!(usd_gf::Matrix3d);
    matches_array_edit_type!(usd_gf::Matrix4d);

    false
}

// ============================================================================
// ValueComposable implementations
// ============================================================================

impl ValueComposable for Dictionary {
    fn can_compose_over(&self) -> bool {
        // Dictionaries always compose non-trivially
        false
    }

    fn compose_over(self, weaker: Self) -> Self {
        compose_dictionaries(self, weaker)
    }
}

impl<T: Clone + Send + Sync + 'static> ValueComposable for Array<T> {
    fn can_compose_over(&self) -> bool {
        // Arrays always replace, so they can compose over
        true
    }

    fn compose_over(self, _weaker: Self) -> Self {
        // Stronger array completely replaces weaker
        self
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Recursively composes two dictionaries.
///
/// Performs a recursive merge where:
/// - Keys only in stronger are kept
/// - Keys only in weaker are kept
/// - Keys in both: if both values are dictionaries, recursively merge;
///   otherwise stronger wins
fn compose_dictionaries(stronger: Dictionary, weaker: Dictionary) -> Dictionary {
    let mut result = weaker.clone();

    for (key, strong_val) in stronger.iter() {
        if let Some(weak_val) = weaker.get(key) {
            // Both have this key
            if let (Some(strong_dict), Some(weak_dict)) =
                (strong_val.get::<Dictionary>(), weak_val.get::<Dictionary>())
            {
                // Both are dictionaries - recursive merge
                let merged = compose_dictionaries(strong_dict.clone(), weak_dict.clone());
                result.insert_value(key.clone(), Value::new(merged));
            } else {
                // Not both dictionaries - stronger wins
                result.insert_value(key.clone(), strong_val.clone());
            }
        } else {
            // Only in stronger
            result.insert_value(key.clone(), strong_val.clone());
        }
    }

    result
}

// ============================================================================
// ArrayEdit composition (matches C++ VtRegisterComposeOver for VtArrayEdit)
// ============================================================================

/// Try to compose ArrayEdit<T> over Array<T> or ArrayEdit<T>.
/// Returns Some(result) if composition succeeded, None if types didn't match.
macro_rules! try_compose_edit_for_type {
    ($stronger:expr, $weaker:expr, $elem:ty) => {
        if let Some(edit) = $stronger.get::<ArrayEdit<$elem>>() {
            // ArrayEdit<T> over Array<T> -> apply edit, return Array<T>
            if let Some(arr) = $weaker.get::<Array<$elem>>() {
                let mut result = arr.clone();
                edit.apply(&mut result);
                return Some(Value::from_no_hash(result));
            }
            // ArrayEdit<T> over ArrayEdit<T> -> compose edits
            if let Some(weak_edit) = $weaker.get::<ArrayEdit<$elem>>() {
                return Some(Value::from_no_hash(edit.compose_over(weak_edit)));
            }
            // ArrayEdit<T> over empty/background -> apply to empty array
            if $weaker.is_empty() {
                let mut result = Array::<$elem>::default();
                edit.apply(&mut result);
                return Some(Value::from_no_hash(result));
            }
        }
    };
}

fn try_compose_array_edit(stronger: &Value, weaker: &Value) -> Option<Value> {
    // Try all scalar element types matching C++ VT_SCALAR_VALUE_TYPES
    try_compose_edit_for_type!(stronger, weaker, bool);
    try_compose_edit_for_type!(stronger, weaker, u8);
    try_compose_edit_for_type!(stronger, weaker, i32);
    try_compose_edit_for_type!(stronger, weaker, u32);
    try_compose_edit_for_type!(stronger, weaker, i64);
    try_compose_edit_for_type!(stronger, weaker, u64);
    try_compose_edit_for_type!(stronger, weaker, f32);
    try_compose_edit_for_type!(stronger, weaker, f64);
    try_compose_edit_for_type!(stronger, weaker, String);
    try_compose_edit_for_type!(stronger, weaker, usd_tf::Token);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec2f);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec3f);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec4f);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec2d);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec3d);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec4d);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec2i);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec3i);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Vec4i);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Quatf);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Quatd);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Quath);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Matrix2d);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Matrix3d);
    try_compose_edit_for_type!(stronger, weaker, usd_gf::Matrix4d);
    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_compose_stronger_wins() {
        let stronger = Value::from(42i32);
        let weaker = Value::from(10i32);
        let result = value_compose_over(&stronger, &weaker);
        assert_eq!(result.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_empty_stronger_returns_weaker() {
        let stronger = Value::empty();
        let weaker = Value::from(10i32);
        let result = value_compose_over(&stronger, &weaker);
        assert_eq!(result.get::<i32>(), Some(&10));
    }

    #[test]
    fn test_empty_weaker_returns_stronger() {
        let stronger = Value::from(42i32);
        let weaker = Value::empty();
        let result = value_compose_over(&stronger, &weaker);
        assert_eq!(result.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_both_empty_returns_empty() {
        let stronger = Value::empty();
        let weaker = Value::empty();
        let result = value_compose_over(&stronger, &weaker);
        assert!(result.is_empty());
    }

    #[test]
    fn test_type_mismatch_stronger_wins() {
        let stronger = Value::from(42i32);
        let weaker = Value::from(3.14f64);
        let result = value_compose_over(&stronger, &weaker);
        assert_eq!(result.get::<i32>(), Some(&42));
        assert!(result.get::<f64>().is_none());
    }

    #[test]
    fn test_array_stronger_replaces_weaker() {
        let stronger = Array::from(vec![1i32, 2, 3]);
        let weaker = Array::from(vec![10i32, 20, 30, 40]);

        let v_strong = Value::new(stronger);
        let v_weak = Value::new(weaker);
        let result = value_compose_over(&v_strong, &v_weak);

        if let Some(arr) = result.get::<Array<i32>>() {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], 1);
            assert_eq!(arr[1], 2);
            assert_eq!(arr[2], 3);
        } else {
            panic!("Expected Array<i32>");
        }
    }

    #[test]
    fn test_dictionary_merge_disjoint_keys() {
        let mut dict1 = Dictionary::new();
        dict1.insert("a", 1i32);
        dict1.insert("b", 2i32);

        let mut dict2 = Dictionary::new();
        dict2.insert("c", 30i32);
        dict2.insert("d", 40i32);

        let result = dict1.compose_over(dict2);

        assert_eq!(result.get_as::<i32>("a"), Some(&1));
        assert_eq!(result.get_as::<i32>("b"), Some(&2));
        assert_eq!(result.get_as::<i32>("c"), Some(&30));
        assert_eq!(result.get_as::<i32>("d"), Some(&40));
    }

    #[test]
    fn test_dictionary_merge_overlapping_keys() {
        let mut dict1 = Dictionary::new();
        dict1.insert("a", 1i32);
        dict1.insert("b", 2i32);

        let mut dict2 = Dictionary::new();
        dict2.insert("b", 20i32);
        dict2.insert("c", 30i32);

        let result = dict1.compose_over(dict2);

        assert_eq!(result.get_as::<i32>("a"), Some(&1)); // from stronger
        assert_eq!(result.get_as::<i32>("b"), Some(&2)); // stronger wins
        assert_eq!(result.get_as::<i32>("c"), Some(&30)); // from weaker
    }

    #[test]
    fn test_dictionary_recursive_merge() {
        // Create nested dictionaries
        let mut inner_strong = Dictionary::new();
        inner_strong.insert("x", 1i32);

        let mut inner_weak = Dictionary::new();
        inner_weak.insert("y", 2i32);

        let mut outer_strong = Dictionary::new();
        outer_strong.insert_value("nested", Value::new(inner_strong));

        let mut outer_weak = Dictionary::new();
        outer_weak.insert_value("nested", Value::new(inner_weak));

        let result = outer_strong.compose_over(outer_weak);

        // Check that nested dictionaries were merged
        if let Some(nested_val) = result.get("nested") {
            if let Some(nested) = nested_val.get::<Dictionary>() {
                assert_eq!(nested.get_as::<i32>("x"), Some(&1)); // from stronger
                assert_eq!(nested.get_as::<i32>("y"), Some(&2)); // from weaker
            } else {
                panic!("Expected nested Dictionary");
            }
        } else {
            panic!("Expected 'nested' key");
        }
    }

    #[test]
    fn test_dictionary_value_compose_over() {
        let mut dict1 = Dictionary::new();
        dict1.insert("a", 1i32);

        let mut dict2 = Dictionary::new();
        dict2.insert("b", 2i32);

        let v1 = Value::new(dict1);
        let v2 = Value::new(dict2);
        let result = value_compose_over(&v1, &v2);

        if let Some(dict) = result.get::<Dictionary>() {
            assert_eq!(dict.get_as::<i32>("a"), Some(&1));
            assert_eq!(dict.get_as::<i32>("b"), Some(&2));
        } else {
            panic!("Expected Dictionary");
        }
    }

    #[test]
    fn test_can_compose_over_empty() {
        let empty = Value::empty();
        assert!(value_can_compose_over(&empty));
    }

    #[test]
    fn test_can_compose_over_scalar() {
        let v = Value::from(42i32);
        assert!(value_can_compose_over(&v));
    }

    #[test]
    fn test_can_compose_over_array() {
        let arr = Array::from(vec![1i32, 2, 3]);
        let v = Value::new(arr);
        assert!(value_can_compose_over(&v));
    }

    #[test]
    fn test_try_compose_over_scalars_is_none() {
        let v1 = Value::from(42i32);
        let v2 = Value::from(10i32);
        assert!(value_try_compose_over(&v1, &v2).is_none());
    }

    #[test]
    fn test_try_compose_over_arrays_is_none() {
        let arr1 = Array::from(vec![1i32, 2]);
        let arr2 = Array::from(vec![10i32, 20]);
        let v1 = Value::new(arr1);
        let v2 = Value::new(arr2);
        assert!(value_try_compose_over(&v1, &v2).is_none());
    }

    #[test]
    fn test_try_compose_over_dicts_is_some() {
        let mut dict1 = Dictionary::new();
        dict1.insert("a", 1i32);

        let mut dict2 = Dictionary::new();
        dict2.insert("b", 2i32);

        let v1 = Value::new(dict1);
        let v2 = Value::new(dict2);
        let result = value_try_compose_over(&v1, &v2);
        assert!(result.is_some());

        if let Some(res) = result {
            if let Some(dict) = res.get::<Dictionary>() {
                assert_eq!(dict.len(), 2);
            }
        }
    }

    #[test]
    fn test_try_compose_over_empty_stronger_is_some() {
        let empty = Value::empty();
        let v = Value::from(42i32);
        let result = value_try_compose_over(&empty, &v);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get::<i32>(), Some(&42));
    }

    #[test]
    fn test_try_compose_over_type_mismatch_is_none() {
        let v1 = Value::from(42i32);
        let v2 = Value::from(3.14f64);
        assert!(value_try_compose_over(&v1, &v2).is_none());
    }

    #[test]
    fn test_float_composition() {
        let stronger = Value::from(3.14f64);
        let weaker = Value::from(2.71f64);
        let result = value_compose_over(&stronger, &weaker);
        assert_eq!(result.get::<f64>(), Some(&3.14f64));
    }

    #[test]
    fn test_string_composition() {
        let stronger = Value::from("stronger".to_string());
        let weaker = Value::from("weaker".to_string());
        let result = value_compose_over(&stronger, &weaker);
        assert_eq!(result.get::<String>(), Some(&"stronger".to_string()));
    }

    #[test]
    fn test_complex_nested_dictionaries() {
        // Build: { "root": { "level1": { "data": 1 } } }
        let mut level1_strong = Dictionary::new();
        level1_strong.insert("data", 1i32);

        let mut root_strong = Dictionary::new();
        root_strong.insert_value("level1", Value::new(level1_strong));

        let mut outer_strong = Dictionary::new();
        outer_strong.insert_value("root", Value::new(root_strong));

        // Build: { "root": { "level1": { "other": 2 }, "sibling": 3 } }
        let mut level1_weak = Dictionary::new();
        level1_weak.insert("other", 2i32);

        let mut root_weak = Dictionary::new();
        root_weak.insert_value("level1", Value::new(level1_weak));
        root_weak.insert("sibling", 3i32);

        let mut outer_weak = Dictionary::new();
        outer_weak.insert_value("root", Value::new(root_weak));

        let result = outer_strong.compose_over(outer_weak);

        // Verify structure
        if let Some(root_val) = result.get("root") {
            if let Some(root) = root_val.get::<Dictionary>() {
                // Check sibling from weaker
                assert_eq!(root.get_as::<i32>("sibling"), Some(&3));

                // Check merged level1
                if let Some(level1_val) = root.get("level1") {
                    if let Some(level1) = level1_val.get::<Dictionary>() {
                        assert_eq!(level1.get_as::<i32>("data"), Some(&1));
                        assert_eq!(level1.get_as::<i32>("other"), Some(&2));
                    } else {
                        panic!("Expected level1 Dictionary");
                    }
                } else {
                    panic!("Expected level1 key");
                }
            } else {
                panic!("Expected root Dictionary");
            }
        } else {
            panic!("Expected root key");
        }
    }

    #[test]
    fn test_dictionary_mixed_types() {
        let mut dict1 = Dictionary::new();
        dict1.insert("int", 42i32);
        dict1.insert("str", "hello".to_string());

        let mut dict2 = Dictionary::new();
        dict2.insert("float", 3.14f64);
        dict2.insert("int", 100i32); // Will be overridden

        let result = dict1.compose_over(dict2);

        assert_eq!(result.get_as::<i32>("int"), Some(&42)); // stronger wins
        assert_eq!(result.get_as::<String>("str"), Some(&"hello".to_string()));
        assert_eq!(result.get_as::<f64>("float"), Some(&3.14f64));
    }

    #[test]
    fn test_array_trait_compose() {
        let arr1 = Array::from(vec![1i32, 2]);
        let arr2 = Array::from(vec![10i32, 20, 30]);
        let result = arr1.compose_over(arr2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_dictionary_trait_can_compose() {
        let dict = Dictionary::new();
        assert!(!dict.can_compose_over()); // Dictionaries compose non-trivially
    }

    #[test]
    fn test_array_trait_can_compose() {
        let arr = Array::<i32>::new();
        assert!(arr.can_compose_over()); // Arrays always replace
    }
}
