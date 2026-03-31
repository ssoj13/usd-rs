//! Sparse value writing utilities.
//!
//! Provides utilities for authoring time-varying attribute values with
//! basic run-length encoding to skip redundant time-samples.

use std::collections::HashMap;
use usd_core::attribute::Attribute;
use usd_sdf::time_code::TimeCode;
use usd_vt::value::Value;

/// Epsilon values for comparing floating point values.
/// C++ reference uses distinct thresholds per type:
/// - half: 1e-2, float: 1e-6, double: 1e-12
#[allow(dead_code)] // Reserved for future half-float support
const EPSILON_HALF: f64 = 1e-2;
const EPSILON_FLOAT: f64 = 1e-6;
const EPSILON_DOUBLE: f64 = 1e-12;

/// Check if two f64 values are within epsilon.
#[inline]
fn is_close_f64(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON_DOUBLE
}

/// Check if two f32 values are within epsilon.
#[inline]
fn is_close_f32(a: f32, b: f32) -> bool {
    (a - b).abs() < EPSILON_FLOAT as f32
}

/// A utility class for authoring time-varying attribute values with
/// simple run-length encoding.
///
/// Time-samples that are close enough to each other (with relative difference
/// smaller than a fixed epsilon) are considered equivalent.
pub struct SparseAttrValueWriter {
    /// The attribute being written to.
    attr: Attribute,
    /// The time at which the previous time-sample was authored.
    prev_time: TimeCode,
    /// The value at the previously authored time-sample.
    prev_value: Option<Value>,
    /// Whether a time-sample was written at prev_time.
    did_write_prev_value: bool,
}

impl SparseAttrValueWriter {
    /// Creates a new sparse attribute value writer.
    pub fn new(attr: Attribute, default_value: Option<Value>) -> Self {
        let mut writer = Self {
            attr,
            prev_time: TimeCode::default(),
            prev_value: None,
            did_write_prev_value: true,
        };

        writer.initialize_sparse_authoring(default_value);
        writer
    }

    /// Initializes the sparse authoring scheme.
    fn initialize_sparse_authoring(&mut self, default_value: Option<Value>) {
        if let Some(value) = default_value {
            // Set default value on the attribute
            let _ = self.attr.set(value.clone(), TimeCode::default());
            self.prev_value = Some(value);
        } else {
            // Use existing default if available
            self.prev_value = self.attr.get(TimeCode::default());
        }
    }

    /// Sets a new time-sample on the attribute.
    ///
    /// The time-sample is only authored if it's different from the previously
    /// set time-sample.
    pub fn set_time_sample(&mut self, value: Value, time: TimeCode) -> bool {
        self.set_time_sample_impl(value, time)
    }

    /// Internal implementation of set_time_sample.
    fn set_time_sample_impl(&mut self, value: Value, time: TimeCode) -> bool {
        let is_close = if let Some(ref prev) = self.prev_value {
            Self::values_are_close(prev, &value)
        } else {
            false
        };

        if is_close {
            self.prev_time = time;
            self.did_write_prev_value = false;
            true
        } else {
            // Write the previous value if we skipped it
            if !self.did_write_prev_value {
                if let Some(ref prev) = self.prev_value {
                    if !self.attr.set(prev.clone(), self.prev_time) {
                        return false;
                    }
                }
            }

            // Write the new value
            if !self.attr.set(value.clone(), time) {
                return false;
            }

            self.prev_time = time;
            self.prev_value = Some(value);
            self.did_write_prev_value = true;
            true
        }
    }

    /// Returns the attribute held by this writer.
    pub fn get_attr(&self) -> &Attribute {
        &self.attr
    }

    /// Checks if two values are close enough to be considered equivalent.
    ///
    /// Uses type-appropriate epsilon thresholds matching C++ reference:
    /// - f64/double: 1e-12
    /// - f32/float: 1e-6
    /// - half::f16: 1e-2
    /// For non-floating-point types, falls back to direct equality.
    fn values_are_close(a: &Value, b: &Value) -> bool {
        // Check for empty values
        if a.is_empty() || b.is_empty() {
            return false;
        }

        // f64 scalar
        if let (Some(&va), Some(&vb)) = (a.get::<f64>(), b.get::<f64>()) {
            return is_close_f64(va, vb);
        }

        // f32 scalar
        if let (Some(&va), Some(&vb)) = (a.get::<f32>(), b.get::<f32>()) {
            return is_close_f32(va, vb);
        }

        // Vec<f64> (VtDoubleArray)
        if let (Some(va), Some(vb)) = (a.get::<Vec<f64>>(), b.get::<Vec<f64>>()) {
            return va.len() == vb.len()
                && va.iter().zip(vb.iter()).all(|(&x, &y)| is_close_f64(x, y));
        }

        // Vec<f32> (VtFloatArray)
        if let (Some(va), Some(vb)) = (a.get::<Vec<f32>>(), b.get::<Vec<f32>>()) {
            return va.len() == vb.len()
                && va.iter().zip(vb.iter()).all(|(&x, &y)| is_close_f32(x, y));
        }

        // For other types, use direct equality
        a == b
    }
}

/// Utility class that manages sparse authoring of a set of USD attributes.
pub struct SparseValueWriter {
    /// Map from attribute to its sparse value writer.
    attr_value_writers: HashMap<AttributeKey, SparseAttrValueWriter>,
}

/// Key type for the attribute map (using attribute path as key).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AttributeKey {
    path: String,
}

impl From<&Attribute> for AttributeKey {
    fn from(attr: &Attribute) -> Self {
        Self {
            path: attr.path().as_str().to_string(),
        }
    }
}

impl SparseValueWriter {
    /// Creates a new sparse value writer.
    pub fn new() -> Self {
        Self {
            attr_value_writers: HashMap::new(),
        }
    }

    /// Sets the value of an attribute at a given time.
    pub fn set_attribute(&mut self, attr: &Attribute, value: Value, time: TimeCode) -> bool {
        let key = AttributeKey::from(attr);

        let writer = self.attr_value_writers.entry(key).or_insert_with(|| {
            let default_value = if time.is_default() {
                Some(value.clone())
            } else {
                None
            };
            SparseAttrValueWriter::new(attr.clone(), default_value)
        });

        if time.is_default() {
            return true;
        }

        writer.set_time_sample(value, time)
    }

    /// Clears the internal map.
    pub fn clear(&mut self) {
        self.attr_value_writers.clear();
    }

    /// Returns the number of attributes being tracked.
    pub fn len(&self) -> usize {
        self.attr_value_writers.len()
    }

    /// Returns true if no attributes are being tracked.
    pub fn is_empty(&self) -> bool {
        self.attr_value_writers.is_empty()
    }

    /// Returns all sparse attribute value writers currently tracked.
    ///
    /// C++ equivalent: `UsdUtilsSparseValueWriter::GetSparseAttrValueWriters()`
    /// Note: C++ returns by value (vector copy); Rust returns references.
    pub fn get_writers(&self) -> Vec<&SparseAttrValueWriter> {
        self.attr_value_writers.values().collect()
    }
}

impl Default for SparseValueWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_value_writer_new() {
        let writer = SparseValueWriter::new();
        assert!(writer.is_empty());
        assert_eq!(writer.len(), 0);
    }

    #[test]
    fn test_sparse_value_writer_get_writers_empty() {
        let writer = SparseValueWriter::new();
        let writers = writer.get_writers();
        assert!(writers.is_empty());
    }

    #[test]
    fn test_is_close_f64_within_epsilon() {
        assert!(is_close_f64(1.0, 1.0 + 1e-13));
        assert!(!is_close_f64(1.0, 1.0 + 1e-11));
    }

    #[test]
    fn test_is_close_f32_within_epsilon() {
        // f32 uses 1e-6 epsilon, not 1e-12
        assert!(is_close_f32(1.0, 1.0 + 1e-7));
        assert!(!is_close_f32(1.0, 1.0 + 1e-5));
    }

    #[test]
    fn test_values_are_close_f64() {
        let a = Value::from(1.0_f64);
        let b = Value::from(1.0_f64 + 1e-13);
        assert!(SparseAttrValueWriter::values_are_close(&a, &b));

        let c = Value::from(1.0_f64 + 1e-11);
        assert!(!SparseAttrValueWriter::values_are_close(&a, &c));
    }

    #[test]
    fn test_values_are_close_f32() {
        // With old code, f32 used 1e-12 epsilon (too strict, everything was "different")
        // Now uses 1e-6, which matches C++ behavior
        let a = Value::from(1.0_f32);
        let b = Value::from(1.0_f32 + 1e-7);
        assert!(SparseAttrValueWriter::values_are_close(&a, &b));

        let c = Value::from(1.0_f32 + 1e-5);
        assert!(!SparseAttrValueWriter::values_are_close(&a, &c));
    }

    #[test]
    fn test_values_are_close_empty() {
        let a = Value::default();
        let b = Value::from(1.0_f64);
        assert!(!SparseAttrValueWriter::values_are_close(&a, &b));
        assert!(!SparseAttrValueWriter::values_are_close(&b, &a));
    }

    #[test]
    fn test_values_are_close_vec_f64() {
        let a = Value::from_no_hash(vec![1.0_f64, 2.0, 3.0]);
        let b = Value::from_no_hash(vec![1.0_f64 + 1e-13, 2.0 + 1e-13, 3.0]);
        assert!(SparseAttrValueWriter::values_are_close(&a, &b));

        let c = Value::from_no_hash(vec![1.0_f64 + 1e-11, 2.0, 3.0]);
        assert!(!SparseAttrValueWriter::values_are_close(&a, &c));
    }

    #[test]
    fn test_values_are_close_vec_different_len() {
        let a = Value::from_no_hash(vec![1.0_f64, 2.0]);
        let b = Value::from_no_hash(vec![1.0_f64, 2.0, 3.0]);
        assert!(!SparseAttrValueWriter::values_are_close(&a, &b));
    }
}
