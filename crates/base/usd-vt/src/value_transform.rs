//! Value transformation utilities.
//!
//! This module provides a trait-based system for transforming `Value` instances
//! and typed `Array<T>` elements. Transformations can be composed and chained
//! for complex operations.
//!
//! # Overview
//!
//! The `ValueTransform` trait defines the core interface for value transformations.
//! Several built-in transformations are provided:
//!
//! - `IdentityTransform` - Returns values unchanged
//! - `MapTransform<F>` - Applies a custom function
//! - `ScaleTransform` - Multiplies numeric values by a factor
//! - `OffsetTransform` - Adds an offset to numeric values
//! - `ClampTransform` - Clamps numeric values to a range
//! - `TransformChain` - Composes multiple transformations
//!
//! # Examples
//!
//! ## Basic Transformations
//!
//! ```
//! use usd_vt::{Value, value_transform::{ValueTransform, ScaleTransform, OffsetTransform}};
//!
//! let val = Value::from(10.0_f64);
//!
//! // Scale by 2.0
//! let scale = ScaleTransform { factor: 2.0 };
//! let scaled = scale.transform(&val).unwrap();
//! assert_eq!(scaled.get::<f64>(), Some(&20.0));
//!
//! // Add offset
//! let offset = OffsetTransform { offset: 5.0 };
//! let result = offset.transform(&val).unwrap();
//! assert_eq!(result.get::<f64>(), Some(&15.0));
//! ```
//!
//! ## Composing Transformations
//!
//! ```
//! use usd_vt::value_transform::{ValueTransform, TransformChain, ScaleTransform, OffsetTransform, ClampTransform};
//! use usd_vt::Value;
//!
//! let val = Value::from(10.0_f64);
//!
//! // Chain: scale by 2, add 5, clamp to [0, 20]
//! let mut chain = TransformChain::new();
//! chain.push(ScaleTransform { factor: 2.0 });
//! chain.push(OffsetTransform { offset: 5.0 });
//! chain.push(ClampTransform { min: 0.0, max: 20.0 });
//!
//! let result = chain.transform(&val).unwrap();
//! assert_eq!(result.get::<f64>(), Some(&20.0)); // 10 * 2 + 5 = 25, clamped to 20
//! ```
//!
//! ## Custom Transformations
//!
//! ```
//! use usd_vt::{Value, value_transform::{ValueTransform, MapTransform}};
//!
//! // Create a custom transform that negates numbers
//! let negate = MapTransform::new(|val: &Value| {
//!     if let Some(&n) = val.get::<f64>() {
//!         Some(Value::from(-n))
//!     } else if let Some(&n) = val.get::<i32>() {
//!         Some(Value::from(-n))
//!     } else {
//!         None
//!     }
//! });
//!
//! let val = Value::from(42.0_f64);
//! let result = negate.transform(&val).unwrap();
//! assert_eq!(result.get::<f64>(), Some(&-42.0));
//! ```
//!
//! ## Transforming Arrays
//!
//! ```
//! use usd_vt::{Array, value_transform::transform_array};
//!
//! let arr: Array<f32> = Array::from(vec![1.0, 2.0, 3.0, 4.0]);
//!
//! // Double each element
//! let doubled = transform_array(&arr, |x| x * 2.0);
//! assert_eq!(doubled[0], 2.0);
//! assert_eq!(doubled[3], 8.0);
//! ```

use super::array::Array;
use super::value::Value;

/// Trait for value transformations.
///
/// Implementors define how to transform a `Value` into another `Value`.
/// Transformations may be type-specific (e.g., only work on numbers) or
/// general-purpose.
///
/// # Return Value
///
/// Returns `Some(Value)` if the transformation was successful, or `None` if
/// the transformation does not apply to the given value type.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_transform::ValueTransform};
///
/// struct DoubleNumbers;
///
/// impl ValueTransform for DoubleNumbers {
///     fn transform(&self, value: &Value) -> Option<Value> {
///         if let Some(&n) = value.get::<f64>() {
///             Some(Value::from(n * 2.0))
///         } else if let Some(&n) = value.get::<i32>() {
///             Some(Value::from(n * 2))
///         } else {
///             None
///         }
///     }
/// }
///
/// let val = Value::from(21.0_f64);
/// let doubled = DoubleNumbers.transform(&val).unwrap();
/// assert_eq!(doubled.get::<f64>(), Some(&42.0));
/// ```
pub trait ValueTransform {
    /// Transform a value, returning a new value or `None` if not applicable.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to transform
    ///
    /// # Returns
    ///
    /// * `Some(Value)` - The transformed value
    /// * `None` - The transformation does not apply to this value type
    fn transform(&self, value: &Value) -> Option<Value>;
}

/// Identity transform - returns the value unchanged.
///
/// This transformation always succeeds and returns a clone of the input value.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_transform::{ValueTransform, IdentityTransform}};
///
/// let val = Value::from(42i32);
/// let identity = IdentityTransform;
/// let result = identity.transform(&val).unwrap();
/// assert_eq!(result.get::<i32>(), Some(&42));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IdentityTransform;

impl ValueTransform for IdentityTransform {
    #[inline]
    fn transform(&self, value: &Value) -> Option<Value> {
        Some(value.clone())
    }
}

/// Map transform - applies a custom function to values.
///
/// This is a flexible transformation that allows you to provide any function
/// that maps `&Value` to `Option<Value>`.
///
/// # Type Parameters
///
/// * `F` - A function type implementing `Fn(&Value) -> Option<Value>`
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_transform::{ValueTransform, MapTransform}};
///
/// // Transform that converts i32 to f64
/// let int_to_float = MapTransform::new(|val: &Value| {
///     val.get::<i32>().map(|&n| Value::from(n as f64))
/// });
///
/// let val = Value::from(42i32);
/// let result = int_to_float.transform(&val).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&42.0));
/// ```
pub struct MapTransform<F> {
    /// The transformation function.
    func: F,
}

impl<F> MapTransform<F>
where
    F: Fn(&Value) -> Option<Value>,
{
    /// Creates a new map transform with the given function.
    ///
    /// # Arguments
    ///
    /// * `func` - Function to apply to values
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{ValueTransform, MapTransform};
    /// use usd_vt::Value;
    ///
    /// let square = MapTransform::new(|val: &Value| {
    ///     val.get::<i32>().map(|&n| Value::from(n * n))
    /// });
    ///
    /// let val = Value::from(5i32);
    /// let result = square.transform(&val).unwrap();
    /// assert_eq!(result.get::<i32>(), Some(&25));
    /// ```
    #[inline]
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

impl<F> ValueTransform for MapTransform<F>
where
    F: Fn(&Value) -> Option<Value>,
{
    #[inline]
    fn transform(&self, value: &Value) -> Option<Value> {
        (self.func)(value)
    }
}

/// Scale transform - multiplies numeric values by a factor.
///
/// Supports the following numeric types:
/// - `i32`, `i64`
/// - `f32`, `f64`
/// - `u32`, `u64`
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_transform::{ValueTransform, ScaleTransform}};
///
/// let val = Value::from(10.0_f64);
/// let scale = ScaleTransform { factor: 2.5 };
/// let result = scale.transform(&val).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&25.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleTransform {
    /// The scaling factor to multiply by.
    pub factor: f64,
}

impl ScaleTransform {
    /// Creates a new scale transform with the given factor.
    ///
    /// # Arguments
    ///
    /// * `factor` - The scaling factor
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{ValueTransform, ScaleTransform};
    /// use usd_vt::Value;
    ///
    /// let scale = ScaleTransform::new(3.0);
    /// let val = Value::from(5.0_f64);
    /// let result = scale.transform(&val).unwrap();
    /// assert_eq!(result.get::<f64>(), Some(&15.0));
    /// ```
    #[inline]
    pub fn new(factor: f64) -> Self {
        Self { factor }
    }
}

impl ValueTransform for ScaleTransform {
    fn transform(&self, value: &Value) -> Option<Value> {
        // Try each numeric type
        if let Some(&n) = value.get::<f64>() {
            Some(Value::from(n * self.factor))
        } else if let Some(&n) = value.get::<f32>() {
            Some(Value::from(n * self.factor as f32))
        } else if let Some(&n) = value.get::<i32>() {
            Some(Value::from((n as f64 * self.factor) as i32))
        } else if let Some(&n) = value.get::<i64>() {
            Some(Value::from((n as f64 * self.factor) as i64))
        } else if let Some(&n) = value.get::<u32>() {
            Some(Value::from((n as f64 * self.factor) as u32))
        } else if let Some(&n) = value.get::<u64>() {
            Some(Value::from((n as f64 * self.factor) as u64))
        } else {
            None
        }
    }
}

/// Offset transform - adds an offset to numeric values.
///
/// Supports the following numeric types:
/// - `i32`, `i64`
/// - `f32`, `f64`
/// - `u32`, `u64`
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_transform::{ValueTransform, OffsetTransform}};
///
/// let val = Value::from(10.0_f64);
/// let offset = OffsetTransform { offset: 5.0 };
/// let result = offset.transform(&val).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&15.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffsetTransform {
    /// The offset value to add.
    pub offset: f64,
}

impl OffsetTransform {
    /// Creates a new offset transform with the given offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset value to add
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{ValueTransform, OffsetTransform};
    /// use usd_vt::Value;
    ///
    /// let offset = OffsetTransform::new(10.0);
    /// let val = Value::from(5.0_f64);
    /// let result = offset.transform(&val).unwrap();
    /// assert_eq!(result.get::<f64>(), Some(&15.0));
    /// ```
    #[inline]
    pub fn new(offset: f64) -> Self {
        Self { offset }
    }
}

impl ValueTransform for OffsetTransform {
    fn transform(&self, value: &Value) -> Option<Value> {
        // Try each numeric type
        if let Some(&n) = value.get::<f64>() {
            Some(Value::from(n + self.offset))
        } else if let Some(&n) = value.get::<f32>() {
            Some(Value::from(n + self.offset as f32))
        } else if let Some(&n) = value.get::<i32>() {
            Some(Value::from((n as f64 + self.offset) as i32))
        } else if let Some(&n) = value.get::<i64>() {
            Some(Value::from((n as f64 + self.offset) as i64))
        } else if let Some(&n) = value.get::<u32>() {
            Some(Value::from((n as f64 + self.offset) as u32))
        } else if let Some(&n) = value.get::<u64>() {
            Some(Value::from((n as f64 + self.offset) as u64))
        } else {
            None
        }
    }
}

/// Clamp transform - clamps numeric values to a range.
///
/// Values below `min` are set to `min`, values above `max` are set to `max`.
///
/// Supports the following numeric types:
/// - `i32`, `i64`
/// - `f32`, `f64`
/// - `u32`, `u64`
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, value_transform::{ValueTransform, ClampTransform}};
///
/// let clamp = ClampTransform { min: 0.0, max: 10.0 };
///
/// let low = Value::from(-5.0_f64);
/// let result = clamp.transform(&low).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&0.0));
///
/// let high = Value::from(15.0_f64);
/// let result = clamp.transform(&high).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&10.0));
///
/// let mid = Value::from(5.0_f64);
/// let result = clamp.transform(&mid).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&5.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClampTransform {
    /// Minimum value (inclusive).
    pub min: f64,
    /// Maximum value (inclusive).
    pub max: f64,
}

impl ClampTransform {
    /// Creates a new clamp transform with the given range.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum value (inclusive)
    /// * `max` - Maximum value (inclusive)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{ValueTransform, ClampTransform};
    /// use usd_vt::Value;
    ///
    /// let clamp = ClampTransform::new(-1.0, 1.0);
    /// let val = Value::from(5.0_f64);
    /// let result = clamp.transform(&val).unwrap();
    /// assert_eq!(result.get::<f64>(), Some(&1.0));
    /// ```
    #[inline]
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }
}

impl ValueTransform for ClampTransform {
    fn transform(&self, value: &Value) -> Option<Value> {
        // Try each numeric type
        if let Some(&n) = value.get::<f64>() {
            Some(Value::from(n.clamp(self.min, self.max)))
        } else if let Some(&n) = value.get::<f32>() {
            Some(Value::from(n.clamp(self.min as f32, self.max as f32)))
        } else if let Some(&n) = value.get::<i32>() {
            Some(Value::from((n as f64).clamp(self.min, self.max) as i32))
        } else if let Some(&n) = value.get::<i64>() {
            Some(Value::from((n as f64).clamp(self.min, self.max) as i64))
        } else if let Some(&n) = value.get::<u32>() {
            Some(Value::from((n as f64).clamp(self.min, self.max) as u32))
        } else if let Some(&n) = value.get::<u64>() {
            Some(Value::from((n as f64).clamp(self.min, self.max) as u64))
        } else {
            None
        }
    }
}

/// Chain of transformations applied sequentially.
///
/// Transformations are applied in the order they were added. If any
/// transformation returns `None`, the entire chain returns `None`.
///
/// # Examples
///
/// ```
/// use usd_vt::value_transform::{ValueTransform, TransformChain, ScaleTransform, OffsetTransform};
/// use usd_vt::Value;
///
/// let mut chain = TransformChain::new();
/// chain.push(ScaleTransform::new(2.0));
/// chain.push(OffsetTransform::new(3.0));
///
/// let val = Value::from(10.0_f64);
/// let result = chain.transform(&val).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&23.0)); // (10 * 2) + 3
/// ```
pub struct TransformChain {
    /// The sequence of transformations to apply.
    transforms: Vec<Box<dyn ValueTransform>>,
}

impl TransformChain {
    /// Creates a new empty transformation chain.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::TransformChain;
    ///
    /// let chain = TransformChain::new();
    /// assert_eq!(chain.len(), 0);
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    /// Creates a transformation chain with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Number of transforms to pre-allocate space for
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::TransformChain;
    ///
    /// let chain = TransformChain::with_capacity(10);
    /// assert_eq!(chain.len(), 0);
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            transforms: Vec::with_capacity(capacity),
        }
    }

    /// Adds a transformation to the end of the chain.
    ///
    /// # Arguments
    ///
    /// * `transform` - The transformation to add
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{ValueTransform, TransformChain, ScaleTransform};
    ///
    /// let mut chain = TransformChain::new();
    /// chain.push(ScaleTransform::new(2.0));
    /// assert_eq!(chain.len(), 1);
    /// ```
    pub fn push(&mut self, transform: impl ValueTransform + 'static) {
        self.transforms.push(Box::new(transform));
    }

    /// Returns the number of transformations in the chain.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{ValueTransform, TransformChain, IdentityTransform};
    ///
    /// let mut chain = TransformChain::new();
    /// assert_eq!(chain.len(), 0);
    ///
    /// chain.push(IdentityTransform);
    /// assert_eq!(chain.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.transforms.len()
    }

    /// Returns true if the chain contains no transformations.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::TransformChain;
    ///
    /// let chain = TransformChain::new();
    /// assert!(chain.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }

    /// Clears all transformations from the chain.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::value_transform::{TransformChain, IdentityTransform};
    ///
    /// let mut chain = TransformChain::new();
    /// chain.push(IdentityTransform);
    /// assert_eq!(chain.len(), 1);
    ///
    /// chain.clear();
    /// assert_eq!(chain.len(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.transforms.clear();
    }
}

impl Default for TransformChain {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueTransform for TransformChain {
    fn transform(&self, value: &Value) -> Option<Value> {
        let mut result = value.clone();
        for transform in &self.transforms {
            result = transform.transform(&result)?;
        }
        Some(result)
    }
}

/// Transform a value using the given transformation.
///
/// This is a convenience function equivalent to calling `transform.transform(value)`.
///
/// # Arguments
///
/// * `value` - The value to transform
/// * `transform` - The transformation to apply
///
/// # Returns
///
/// * `Some(Value)` - The transformed value
/// * `None` - The transformation does not apply to this value type
///
/// # Examples
///
/// ```
/// use usd_vt::value_transform::{ValueTransform, transform_value, ScaleTransform};
/// use usd_vt::Value;
///
/// let val = Value::from(10.0_f64);
/// let scale = ScaleTransform::new(3.0);
/// let result = transform_value(&val, &scale).unwrap();
/// assert_eq!(result.get::<f64>(), Some(&30.0));
/// ```
#[inline]
pub fn transform_value(value: &Value, transform: &dyn ValueTransform) -> Option<Value> {
    transform.transform(value)
}

/// Transform array elements using the given function.
///
/// Applies the transformation function to each element of the array,
/// creating a new array with the transformed values.
///
/// # Type Parameters
///
/// * `T` - Element type (must be `Clone + Send + Sync + 'static`)
/// * `F` - Function type implementing `Fn(&T) -> T`
///
/// # Arguments
///
/// * `array` - The array to transform
/// * `func` - The transformation function to apply to each element
///
/// # Returns
///
/// A new array containing the transformed elements.
///
/// # Examples
///
/// ```
/// use usd_vt::{Array, value_transform::transform_array};
///
/// let arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5]);
/// let doubled = transform_array(&arr, |x| x * 2);
///
/// assert_eq!(doubled[0], 2);
/// assert_eq!(doubled[4], 10);
/// ```
pub fn transform_array<T, F>(array: &Array<T>, func: F) -> Array<T>
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> T,
{
    Array::from(array.iter().map(func).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_transform() {
        let val = Value::from(42i32);
        let identity = IdentityTransform;
        let result = identity.transform(&val).unwrap();
        assert_eq!(result.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_identity_preserves_all_types() {
        let identity = IdentityTransform;

        let int_val = Value::from(123i32);
        assert_eq!(
            identity.transform(&int_val).unwrap().get::<i32>(),
            Some(&123)
        );

        let float_val = Value::from(3.14_f64);
        assert_eq!(
            identity.transform(&float_val).unwrap().get::<f64>(),
            Some(&3.14)
        );

        let str_val = Value::from("hello".to_string());
        assert_eq!(
            identity.transform(&str_val).unwrap().get::<String>(),
            Some(&"hello".to_string())
        );
    }

    #[test]
    fn test_map_transform() {
        let square = MapTransform::new(|val: &Value| val.get::<i32>().map(|&n| Value::from(n * n)));

        let val = Value::from(5i32);
        let result = square.transform(&val).unwrap();
        assert_eq!(result.get::<i32>(), Some(&25));
    }

    #[test]
    fn test_map_transform_returns_none() {
        let int_only = MapTransform::new(|val: &Value| val.get::<i32>().map(|&n| Value::from(n)));

        let val = Value::from(3.14_f64);
        assert!(int_only.transform(&val).is_none());
    }

    #[test]
    fn test_map_transform_type_conversion() {
        let int_to_float =
            MapTransform::new(|val: &Value| val.get::<i32>().map(|&n| Value::from(n as f64)));

        let val = Value::from(42i32);
        let result = int_to_float.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&42.0));
    }

    #[test]
    fn test_scale_transform_f64() {
        let val = Value::from(10.0_f64);
        let scale = ScaleTransform::new(2.5);
        let result = scale.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&25.0));
    }

    #[test]
    fn test_scale_transform_f32() {
        let val = Value::from(5.0_f32);
        let scale = ScaleTransform::new(3.0);
        let result = scale.transform(&val).unwrap();
        assert_eq!(result.get::<f32>(), Some(&15.0));
    }

    #[test]
    fn test_scale_transform_i32() {
        let val = Value::from(10i32);
        let scale = ScaleTransform::new(3.0);
        let result = scale.transform(&val).unwrap();
        assert_eq!(result.get::<i32>(), Some(&30));
    }

    #[test]
    fn test_scale_transform_i64() {
        let val = Value::from(100i64);
        let scale = ScaleTransform::new(0.5);
        let result = scale.transform(&val).unwrap();
        assert_eq!(result.get::<i64>(), Some(&50));
    }

    #[test]
    fn test_scale_transform_non_numeric() {
        let val = Value::from("hello".to_string());
        let scale = ScaleTransform::new(2.0);
        assert!(scale.transform(&val).is_none());
    }

    #[test]
    fn test_offset_transform_f64() {
        let val = Value::from(10.0_f64);
        let offset = OffsetTransform::new(5.0);
        let result = offset.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&15.0));
    }

    #[test]
    fn test_offset_transform_negative() {
        let val = Value::from(10.0_f64);
        let offset = OffsetTransform::new(-3.0);
        let result = offset.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&7.0));
    }

    #[test]
    fn test_offset_transform_i32() {
        let val = Value::from(20i32);
        let offset = OffsetTransform::new(15.0);
        let result = offset.transform(&val).unwrap();
        assert_eq!(result.get::<i32>(), Some(&35));
    }

    #[test]
    fn test_clamp_transform_below_min() {
        let val = Value::from(-5.0_f64);
        let clamp = ClampTransform::new(0.0, 10.0);
        let result = clamp.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&0.0));
    }

    #[test]
    fn test_clamp_transform_above_max() {
        let val = Value::from(15.0_f64);
        let clamp = ClampTransform::new(0.0, 10.0);
        let result = clamp.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&10.0));
    }

    #[test]
    fn test_clamp_transform_within_range() {
        let val = Value::from(5.0_f64);
        let clamp = ClampTransform::new(0.0, 10.0);
        let result = clamp.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&5.0));
    }

    #[test]
    fn test_clamp_transform_i32() {
        let val = Value::from(100i32);
        let clamp = ClampTransform::new(0.0, 50.0);
        let result = clamp.transform(&val).unwrap();
        assert_eq!(result.get::<i32>(), Some(&50));
    }

    #[test]
    fn test_transform_chain_empty() {
        let chain = TransformChain::new();
        let val = Value::from(10.0_f64);
        let result = chain.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&10.0));
    }

    #[test]
    fn test_transform_chain_single() {
        let mut chain = TransformChain::new();
        chain.push(ScaleTransform::new(2.0));

        let val = Value::from(10.0_f64);
        let result = chain.transform(&val).unwrap();
        assert_eq!(result.get::<f64>(), Some(&20.0));
    }

    #[test]
    fn test_transform_chain_multiple() {
        let mut chain = TransformChain::new();
        chain.push(ScaleTransform::new(2.0));
        chain.push(OffsetTransform::new(5.0));

        let val = Value::from(10.0_f64);
        let result = chain.transform(&val).unwrap();
        // (10 * 2) + 5 = 25
        assert_eq!(result.get::<f64>(), Some(&25.0));
    }

    #[test]
    fn test_transform_chain_with_clamp() {
        let mut chain = TransformChain::new();
        chain.push(ScaleTransform::new(2.0));
        chain.push(OffsetTransform::new(5.0));
        chain.push(ClampTransform::new(0.0, 20.0));

        let val = Value::from(10.0_f64);
        let result = chain.transform(&val).unwrap();
        // (10 * 2) + 5 = 25, clamped to 20
        assert_eq!(result.get::<f64>(), Some(&20.0));
    }

    #[test]
    fn test_transform_chain_fails_on_incompatible() {
        let mut chain = TransformChain::new();
        chain.push(ScaleTransform::new(2.0));

        let val = Value::from("hello".to_string());
        assert!(chain.transform(&val).is_none());
    }

    #[test]
    fn test_transform_chain_methods() {
        let mut chain = TransformChain::new();
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());

        chain.push(IdentityTransform);
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());

        chain.push(IdentityTransform);
        assert_eq!(chain.len(), 2);

        chain.clear();
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_transform_value_function() {
        let val = Value::from(10.0_f64);
        let scale = ScaleTransform::new(3.0);
        let result = transform_value(&val, &scale).unwrap();
        assert_eq!(result.get::<f64>(), Some(&30.0));
    }

    #[test]
    fn test_transform_array() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5]);
        let doubled = transform_array(&arr, |x| x * 2);

        assert_eq!(doubled.len(), 5);
        assert_eq!(doubled[0], 2);
        assert_eq!(doubled[1], 4);
        assert_eq!(doubled[2], 6);
        assert_eq!(doubled[3], 8);
        assert_eq!(doubled[4], 10);
    }

    #[test]
    fn test_transform_array_f32() {
        let arr: Array<f32> = Array::from(vec![1.0, 2.0, 3.0]);
        let scaled = transform_array(&arr, |x| x * 2.5);

        assert_eq!(scaled.len(), 3);
        assert_eq!(scaled[0], 2.5);
        assert_eq!(scaled[1], 5.0);
        assert_eq!(scaled[2], 7.5);
    }

    #[test]
    fn test_transform_array_with_closure() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3, 4]);
        let factor = 10;
        let multiplied = transform_array(&arr, |x| x * factor);

        assert_eq!(multiplied[0], 10);
        assert_eq!(multiplied[3], 40);
    }

    #[test]
    fn test_complex_chain_example() {
        // Real-world scenario: normalize values to [0, 1] range
        // Input is in range [0, 255], scale by 1/255, then clamp
        let mut normalize = TransformChain::new();
        normalize.push(ScaleTransform::new(1.0 / 255.0));
        normalize.push(ClampTransform::new(0.0, 1.0));

        let val = Value::from(128.0_f64);
        let result = normalize.transform(&val).unwrap();
        let normalized = result.get::<f64>().unwrap();
        assert!((*normalized - 0.5019607843137255).abs() < 1e-10);
    }

    #[test]
    fn test_scale_multiple_types() {
        let scale = ScaleTransform::new(2.0);

        // Test u32
        let u32_val = Value::from(10u32);
        assert_eq!(scale.transform(&u32_val).unwrap().get::<u32>(), Some(&20));

        // Test u64
        let u64_val = Value::from(50u64);
        assert_eq!(scale.transform(&u64_val).unwrap().get::<u64>(), Some(&100));
    }

    #[test]
    fn test_offset_multiple_types() {
        let offset = OffsetTransform::new(5.0);

        // Test u32
        let u32_val = Value::from(10u32);
        assert_eq!(offset.transform(&u32_val).unwrap().get::<u32>(), Some(&15));

        // Test f32
        let f32_val = Value::from(2.5_f32);
        assert_eq!(offset.transform(&f32_val).unwrap().get::<f32>(), Some(&7.5));
    }

    #[test]
    fn test_clamp_edge_cases() {
        let clamp = ClampTransform::new(0.0, 100.0);

        // Exactly at min
        let at_min = Value::from(0.0_f64);
        assert_eq!(clamp.transform(&at_min).unwrap().get::<f64>(), Some(&0.0));

        // Exactly at max
        let at_max = Value::from(100.0_f64);
        assert_eq!(clamp.transform(&at_max).unwrap().get::<f64>(), Some(&100.0));
    }

    #[test]
    fn test_transform_chain_capacity() {
        let chain = TransformChain::with_capacity(5);
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());
    }
}
