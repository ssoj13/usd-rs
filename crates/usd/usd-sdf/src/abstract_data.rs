//! Abstract data storage interface for USD.
//!
//! This module provides the core trait and types for data storage in SDF layers.
//! `AbstractData` defines the interface for storing specs (objects) and fields
//! (key-value pairs) in a layer.

use std::collections::{BTreeMap, HashMap};

use usd_tf::Token;

use super::path::Path;
use super::types::{SpecType, TimeSamples};

// ============================================================================
// Value Type
// ============================================================================

/// Type-erased value for field storage.
///
/// This is an alias to `vt::Value`, the universal type-erased container.
pub type Value = usd_vt::Value;

// ============================================================================
// Abstract Data Trait
// ============================================================================

/// Interface for scene description data storage.
///
/// This is an anonymous container holding scene description values.
/// It's like an STL container, but specialized for holding scene description.
///
/// For any given `Path`, `AbstractData` can hold one or more key/value pairs
/// called Fields. Most of the API accesses or modifies the value stored in a
/// field for a particular path and field name.
///
/// `AbstractData` does not provide undo, change notification, or any strong
/// consistency guarantees. Instead, it's a basis for building those features.
pub trait AbstractData: Send + Sync {
    // ========================================================================
    // Basic properties
    // ========================================================================

    /// Returns true if this data streams to/from its serialized store on demand.
    ///
    /// Sdf treats layers with streaming data differently to avoid pulling in
    /// data unnecessarily. For example, reloading a streaming layer will not
    /// perform fine-grained change notification.
    fn streams_data(&self) -> bool;

    /// Returns true if this data is detached from its serialized store.
    ///
    /// A detached data object must not be affected by external changes to the
    /// serialized data. The default returns !streams_data().
    fn is_detached(&self) -> bool {
        !self.streams_data()
    }

    /// Returns true if this data contains no specs.
    fn is_empty(&self) -> bool;

    // ========================================================================
    // Spec operations
    // ========================================================================

    /// Create a new spec at path with the given spec type.
    ///
    /// If the spec already exists, the spec type will be changed.
    fn create_spec(&mut self, path: &Path, spec_type: SpecType);

    /// Returns true if this data has a spec for path.
    fn has_spec(&self, path: &Path) -> bool;

    /// Erase the spec at path and any fields on it.
    ///
    /// Note: This does not erase child specs.
    fn erase_spec(&mut self, path: &Path);

    /// Move the spec at old_path to new_path, including all fields.
    ///
    /// Note: This does not move child specs.
    fn move_spec(&mut self, old_path: &Path, new_path: &Path);

    /// Returns the spec type for the spec at path.
    ///
    /// Returns `SpecType::Unknown` if the spec doesn't exist.
    fn get_spec_type(&self, path: &Path) -> SpecType;

    /// Visit every spec with the given visitor.
    ///
    /// The order in which specs are visited is undefined.
    /// The visitor may not modify the data object.
    fn visit_specs(&self, visitor: &mut dyn SpecVisitor);

    // ========================================================================
    // Field operations
    // ========================================================================

    /// Returns whether a value exists for the given path and field name.
    fn has_field(&self, path: &Path, field_name: &Token) -> bool;

    /// Returns the value for the given path and field name.
    ///
    /// Returns `None` if no value is set.
    fn get_field(&self, path: &Path, field_name: &Token) -> Option<Value>;

    /// Sets the value for the given path and field name.
    ///
    /// It's an error to set a field on a spec that does not exist.
    fn set_field(&mut self, path: &Path, field_name: &Token, value: Value);

    /// Removes the field at path and field_name.
    fn erase_field(&mut self, path: &Path, field_name: &Token);

    /// Returns the names of all fields that are set at path.
    fn list_fields(&self, path: &Path) -> Vec<Token>;

    /// Returns whether both spec exists and field exists.
    ///
    /// Equivalent to:
    /// ```ignore
    /// let spec_type = get_spec_type(path);
    /// spec_type != SpecType::Unknown && has_field(path, field_name)
    /// ```
    fn has_spec_and_field(&self, path: &Path, field_name: &Token) -> (bool, SpecType) {
        let spec_type = self.get_spec_type(path);
        let has = spec_type != SpecType::Unknown && self.has_field(path, field_name);
        (has, spec_type)
    }

    // ========================================================================
    // Dictionary field operations
    // ========================================================================

    /// Returns true if the field is dictionary-valued and contains key_path.
    fn has_dict_key(&self, path: &Path, field_name: &Token, key_path: &Token) -> bool {
        if let Some(dict_value) = self.get_field(path, field_name) {
            // Check if value is a dictionary containing key_path
            if let Some(dict) = dict_value.get::<BTreeMap<String, Value>>() {
                return dict.contains_key(key_path.as_str());
            }
        }
        false
    }

    /// Returns the element at key_path in the dictionary-valued field.
    fn get_dict_value_by_key(
        &self,
        path: &Path,
        field_name: &Token,
        key_path: &Token,
    ) -> Option<Value> {
        let dict_value = self.get_field(path, field_name)?;
        let dict = dict_value.get::<BTreeMap<String, Value>>()?;
        dict.get(key_path.as_str()).cloned()
    }

    /// Sets the element at key_path in the dictionary-valued field.
    fn set_dict_value_by_key(
        &mut self,
        path: &Path,
        field_name: &Token,
        key_path: &Token,
        value: Value,
    ) {
        // Get existing dict or create new one
        let mut dict = if let Some(existing) = self.get_field(path, field_name) {
            existing
                .get::<BTreeMap<String, Value>>()
                .cloned()
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };

        dict.insert(key_path.as_str().to_string(), value);
        self.set_field(path, field_name, Value::new(dict));
    }

    /// Removes the element at key_path from the dictionary-valued field.
    fn erase_dict_value_by_key(&mut self, path: &Path, field_name: &Token, key_path: &Token) {
        if let Some(dict_value) = self.get_field(path, field_name) {
            if let Some(dict) = dict_value.get::<BTreeMap<String, Value>>() {
                let mut dict = dict.clone();
                dict.remove(key_path.as_str());
                if dict.is_empty() {
                    self.erase_field(path, field_name);
                } else {
                    self.set_field(path, field_name, Value::new(dict));
                }
            }
        }
    }

    /// Returns the keys in the dictionary field at key_path.
    fn list_dict_keys(&self, path: &Path, field_name: &Token, key_path: &Token) -> Vec<Token> {
        if let Some(value) = self.get_dict_value_by_key(path, field_name, key_path) {
            if let Some(dict) = value.get::<BTreeMap<String, Value>>() {
                return dict.keys().map(|k| Token::new(k)).collect();
            }
        }
        Vec::new()
    }

    // ========================================================================
    // Time sample operations
    // ========================================================================

    /// Returns all time sample times in this data.
    ///
    /// Matches C++ `std::set<double> ListAllTimeSamples() const`.
    fn list_all_time_samples(&self) -> TimeSamples;

    /// Returns all time sample times for the given path.
    ///
    /// Matches C++ `std::set<double> ListTimeSamplesForPath(const SdfPath&) const`.
    fn list_time_samples_for_path(&self, path: &Path) -> TimeSamples;

    /// Returns the number of time samples for the given path.
    fn get_num_time_samples_for_path(&self, path: &Path) -> usize {
        self.list_time_samples_for_path(path).len()
    }

    /// Returns all time sample times as Vec<f64> (convenience method).
    ///
    /// Converts from TimeSamples (BTreeSet<OrderedFloat<f64>>) to Vec<f64>.
    fn list_all_time_samples_vec(&self) -> Vec<f64> {
        self.list_all_time_samples()
            .iter()
            .map(|of| of.into_inner())
            .collect()
    }

    /// Returns time sample times for path as Vec<f64> (convenience method).
    ///
    /// Converts from TimeSamples (BTreeSet<OrderedFloat<f64>>) to Vec<f64>.
    fn list_time_samples_for_path_vec(&self, path: &Path) -> Vec<f64> {
        self.list_time_samples_for_path(path)
            .iter()
            .map(|of| of.into_inner())
            .collect()
    }

    /// Returns the bracketing time samples around the given time.
    ///
    /// Returns (lower, upper) where lower <= time <= upper.
    /// If there's an exact match, both values will be equal to time.
    fn get_bracketing_time_samples(&self, time: f64) -> Option<(f64, f64)>;

    /// Returns the bracketing time samples for the given path.
    fn get_bracketing_time_samples_for_path(&self, path: &Path, time: f64) -> Option<(f64, f64)>;

    /// Returns the previous time sample before the given time.
    fn get_previous_time_sample_for_path(&self, path: &Path, time: f64) -> Option<f64> {
        use ordered_float::OrderedFloat;
        let time_ord = OrderedFloat(time);
        let samples = self.list_time_samples_for_path(path);
        samples
            .into_iter()
            .filter(|&of| of < time_ord)
            .max()
            .map(|of| of.into_inner())
    }

    /// Queries whether a time sample exists at the given time.
    ///
    /// Returns the value if it exists.
    fn query_time_sample(&self, path: &Path, time: f64) -> Option<Value>;

    /// Sets a time sample value at the given time.
    fn set_time_sample(&mut self, path: &Path, time: f64, value: Value);

    /// Erases the time sample at the given time.
    fn erase_time_sample(&mut self, path: &Path, time: f64);

    // ========================================================================
    // Utility operations
    // ========================================================================

    /// Copies data from source into this data object.
    fn copy_from(&mut self, source: &dyn AbstractData)
    where
        Self: Sized,
    {
        struct CopyVisitor<'a> {
            dest: &'a mut dyn AbstractData,
            source: &'a dyn AbstractData,
        }

        impl<'a> SpecVisitor for CopyVisitor<'a> {
            fn visit_spec(&mut self, path: &Path) -> bool {
                let spec_type = self.source.get_spec_type(path);
                self.dest.create_spec(path, spec_type);

                // Copy all fields
                for field_name in self.source.list_fields(path) {
                    if let Some(value) = self.source.get_field(path, &field_name) {
                        self.dest.set_field(path, &field_name, value);
                    }
                }

                // Copy time samples
                for time_ord in self.source.list_time_samples_for_path(path) {
                    let time = time_ord.into_inner();
                    if let Some(value) = self.source.query_time_sample(path, time) {
                        self.dest.set_time_sample(path, time, value);
                    }
                }

                true
            }

            fn done(&mut self) {}
        }

        let mut visitor = CopyVisitor { dest: self, source };
        source.visit_specs(&mut visitor);
    }

    /// Returns self as `&dyn Any` for downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns true if this data equals another data object.
    ///
    /// Performs spec-by-spec, field-by-field comparison.
    fn equals(&self, other: &dyn AbstractData) -> bool {
        struct EqualityVisitor<'a> {
            other: &'a dyn AbstractData,
            equal: bool,
        }

        impl<'a> SpecVisitor for EqualityVisitor<'a> {
            fn visit_spec(&mut self, path: &Path) -> bool {
                // Check spec type
                if self.other.get_spec_type(path) == SpecType::Unknown {
                    self.equal = false;
                    return false;
                }

                // Note: Deep field comparison would require value comparison
                // which isn't possible without type information
                self.equal &= true;
                true
            }

            fn done(&mut self) {}
        }

        let mut visitor = EqualityVisitor { other, equal: true };
        self.visit_specs(&mut visitor);
        visitor.equal
    }
}

// ============================================================================
// Spec Visitor Trait
// ============================================================================

/// Visitor for traversing specs in an `AbstractData` object.
///
/// The visitor pattern allows external code to iterate over all specs
/// without exposing the internal data structure.
pub trait SpecVisitor {
    /// Called for each spec.
    ///
    /// Returns `false` to stop iteration early, `true` to continue.
    fn visit_spec(&mut self, path: &Path) -> bool;

    /// Called after all specs have been visited.
    ///
    /// This is called even if iteration was stopped early.
    fn done(&mut self);
}

// ============================================================================
// Data Visitor Trait
// ============================================================================

/// Extended visitor that receives both path and spec data.
pub trait DataVisitor {
    /// Called for each spec with its data.
    ///
    /// Returns `false` to stop iteration, `true` to continue.
    fn visit(&mut self, path: &Path, data: &dyn AbstractData) -> bool;

    /// Called after visitation is complete.
    fn done(&mut self, data: &dyn AbstractData);
}

// ============================================================================
// Simple Implementation
// ============================================================================

/// Simple in-memory implementation of `AbstractData`.
///
/// This is a basic implementation using standard collections, suitable for
/// testing and small datasets.
pub struct SimpleData {
    /// Map from path to (spec_type, fields)
    specs: BTreeMap<Path, (SpecType, BTreeMap<Token, Value>)>,
    /// Map from path to time samples (using HashMap for f64 keys)
    time_samples: BTreeMap<Path, HashMap<ordered_float::OrderedFloat<f64>, Value>>,
}

impl SimpleData {
    /// Creates a new empty data object.
    pub fn new() -> Self {
        Self {
            specs: BTreeMap::new(),
            time_samples: BTreeMap::new(),
        }
    }

    /// Converts f64 to ordered float for use as HashMap key.
    fn to_ordered(time: f64) -> ordered_float::OrderedFloat<f64> {
        ordered_float::OrderedFloat(time)
    }
}

impl Default for SimpleData {
    fn default() -> Self {
        Self::new()
    }
}

impl AbstractData for SimpleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn streams_data(&self) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    fn create_spec(&mut self, path: &Path, spec_type: SpecType) {
        self.specs
            .entry(path.clone())
            .and_modify(|(st, _)| *st = spec_type)
            .or_insert((spec_type, BTreeMap::new()));
    }

    fn has_spec(&self, path: &Path) -> bool {
        self.specs.contains_key(path)
    }

    fn erase_spec(&mut self, path: &Path) {
        self.specs.remove(path);
        self.time_samples.remove(path);
    }

    fn move_spec(&mut self, old_path: &Path, new_path: &Path) {
        if let Some(spec_data) = self.specs.remove(old_path) {
            self.specs.insert(new_path.clone(), spec_data);
        }
        if let Some(samples) = self.time_samples.remove(old_path) {
            self.time_samples.insert(new_path.clone(), samples);
        }
    }

    fn get_spec_type(&self, path: &Path) -> SpecType {
        self.specs
            .get(path)
            .map(|(st, _)| *st)
            .unwrap_or(SpecType::Unknown)
    }

    fn visit_specs(&self, visitor: &mut dyn SpecVisitor) {
        for path in self.specs.keys() {
            if !visitor.visit_spec(path) {
                break;
            }
        }
        visitor.done();
    }

    fn has_field(&self, path: &Path, field_name: &Token) -> bool {
        self.specs
            .get(path)
            .map(|(_, fields)| fields.contains_key(field_name))
            .unwrap_or(false)
    }

    fn get_field(&self, path: &Path, field_name: &Token) -> Option<Value> {
        self.specs
            .get(path)
            .and_then(|(_, fields)| fields.get(field_name).cloned())
    }

    fn set_field(&mut self, path: &Path, field_name: &Token, value: Value) {
        if let Some((_, fields)) = self.specs.get_mut(path) {
            fields.insert(field_name.clone(), value);
        }
    }

    fn erase_field(&mut self, path: &Path, field_name: &Token) {
        if let Some((_, fields)) = self.specs.get_mut(path) {
            fields.remove(field_name);
        }
        // C++ SdfData stores timeSamples as a field; we store them separately.
        // When erasing "timeSamples", also clear the time_samples storage.
        if field_name.get_text() == "timeSamples" {
            self.time_samples.remove(path);
        }
    }

    fn list_fields(&self, path: &Path) -> Vec<Token> {
        self.specs
            .get(path)
            .map(|(_, fields)| fields.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn list_all_time_samples(&self) -> TimeSamples {
        let mut all_times = TimeSamples::new();
        for samples in self.time_samples.values() {
            for &time in samples.keys() {
                all_times.insert(time); // time is already OrderedFloat<f64>
            }
        }
        all_times
    }

    fn list_time_samples_for_path(&self, path: &Path) -> TimeSamples {
        self.time_samples
            .get(path)
            .map(|samples| samples.keys().copied().collect())
            .unwrap_or_default()
    }

    fn get_bracketing_time_samples(&self, time: f64) -> Option<(f64, f64)> {
        use ordered_float::OrderedFloat;
        let time_ord = OrderedFloat(time);
        let all_times = self.list_all_time_samples();
        if all_times.is_empty() {
            return None;
        }

        // Find lower and upper bounds using OrderedFloat comparison
        let lower = all_times
            .iter()
            .filter(|&&of| of <= time_ord)
            .max()
            .copied();
        let upper = all_times
            .iter()
            .filter(|&&of| of >= time_ord)
            .min()
            .copied();

        match (lower, upper) {
            (Some(l), Some(u)) => Some((l.into_inner(), u.into_inner())),
            (Some(l), None) => {
                let val = l.into_inner();
                Some((val, val))
            }
            (None, Some(u)) => {
                let val = u.into_inner();
                Some((val, val))
            }
            (None, None) => None,
        }
    }

    fn get_bracketing_time_samples_for_path(&self, path: &Path, time: f64) -> Option<(f64, f64)> {
        use ordered_float::OrderedFloat;
        let time_ord = OrderedFloat(time);
        let times = self.list_time_samples_for_path(path);
        if times.is_empty() {
            return None;
        }

        // Find lower and upper bounds using OrderedFloat comparison
        let lower = times.iter().filter(|&&of| of <= time_ord).max().copied();
        let upper = times.iter().filter(|&&of| of >= time_ord).min().copied();

        match (lower, upper) {
            (Some(l), Some(u)) => Some((l.into_inner(), u.into_inner())),
            (Some(l), None) => {
                let val = l.into_inner();
                Some((val, val))
            }
            (None, Some(u)) => {
                let val = u.into_inner();
                Some((val, val))
            }
            (None, None) => None,
        }
    }

    fn query_time_sample(&self, path: &Path, time: f64) -> Option<Value> {
        self.time_samples
            .get(path)
            .and_then(|samples| samples.get(&Self::to_ordered(time)).cloned())
    }

    fn set_time_sample(&mut self, path: &Path, time: f64, value: Value) {
        self.time_samples
            .entry(path.clone())
            .or_default()
            .insert(Self::to_ordered(time), value);
    }

    fn erase_time_sample(&mut self, path: &Path, time: f64) {
        if let Some(samples) = self.time_samples.get_mut(path) {
            samples.remove(&Self::to_ordered(time));
            if samples.is_empty() {
                self.time_samples.remove(path);
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a new simple data object.
pub fn create_simple_data() -> Box<dyn AbstractData> {
    Box::new(SimpleData::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_data_creation() {
        let data = SimpleData::new();
        assert!(!data.streams_data());
        assert!(data.is_detached());
        assert!(data.is_empty());
    }

    #[test]
    fn test_spec_creation() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World").unwrap();

        data.create_spec(&path, SpecType::Prim);
        assert!(data.has_spec(&path));
        assert_eq!(data.get_spec_type(&path), SpecType::Prim);
        assert!(!data.is_empty());
    }

    #[test]
    fn test_spec_erase() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World").unwrap();

        data.create_spec(&path, SpecType::Prim);
        assert!(data.has_spec(&path));

        data.erase_spec(&path);
        assert!(!data.has_spec(&path));
        assert!(data.is_empty());
    }

    #[test]
    fn test_spec_move() {
        let mut data = SimpleData::new();
        let old_path = Path::from_string("/World").unwrap();
        let new_path = Path::from_string("/NewWorld").unwrap();

        data.create_spec(&old_path, SpecType::Prim);
        data.move_spec(&old_path, &new_path);

        assert!(!data.has_spec(&old_path));
        assert!(data.has_spec(&new_path));
        assert_eq!(data.get_spec_type(&new_path), SpecType::Prim);
    }

    #[test]
    fn test_field_operations() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World").unwrap();
        let field_name = Token::new("active");

        data.create_spec(&path, SpecType::Prim);

        // Set field
        data.set_field(&path, &field_name, Value::new(true));
        assert!(data.has_field(&path, &field_name));

        // Get field
        let value = data.get_field(&path, &field_name).unwrap();
        assert_eq!(*value.get::<bool>().unwrap(), true);

        // List fields
        let fields = data.list_fields(&path);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0], field_name);

        // Erase field
        data.erase_field(&path, &field_name);
        assert!(!data.has_field(&path, &field_name));
    }

    #[test]
    fn test_has_spec_and_field() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World").unwrap();
        let field_name = Token::new("active");

        // No spec
        let (has, spec_type) = data.has_spec_and_field(&path, &field_name);
        assert!(!has);
        assert_eq!(spec_type, SpecType::Unknown);

        // Spec but no field
        data.create_spec(&path, SpecType::Prim);
        let (has, spec_type) = data.has_spec_and_field(&path, &field_name);
        assert!(!has);
        assert_eq!(spec_type, SpecType::Prim);

        // Spec and field
        data.set_field(&path, &field_name, Value::new(true));
        let (has, spec_type) = data.has_spec_and_field(&path, &field_name);
        assert!(has);
        assert_eq!(spec_type, SpecType::Prim);
    }

    #[test]
    fn test_dict_operations() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World").unwrap();
        let field_name = Token::new("customData");
        let key_path = Token::new("myKey");

        data.create_spec(&path, SpecType::Prim);

        // Set dict value
        data.set_dict_value_by_key(&path, &field_name, &key_path, Value::new(42));
        assert!(data.has_dict_key(&path, &field_name, &key_path));

        // Get dict value
        let value = data
            .get_dict_value_by_key(&path, &field_name, &key_path)
            .unwrap();
        assert_eq!(*value.get::<i32>().unwrap(), 42);

        // List dict keys
        let keys = data.list_dict_keys(&path, &field_name, &Token::new(""));
        assert!(keys.is_empty()); // No nested dict at root level

        // Erase dict value
        data.erase_dict_value_by_key(&path, &field_name, &key_path);
        assert!(!data.has_dict_key(&path, &field_name, &key_path));
    }

    #[test]
    fn test_time_sample_operations() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World.xform").unwrap();

        data.create_spec(&path, SpecType::Attribute);

        // Set time samples
        data.set_time_sample(&path, 1.0, Value::from_f64(10.0));
        data.set_time_sample(&path, 2.0, Value::from_f64(20.0));
        data.set_time_sample(&path, 3.0, Value::from_f64(30.0));

        // List time samples
        let times = data.list_time_samples_for_path(&path);
        assert_eq!(times.len(), 3);
        let times_vec: Vec<f64> = times.iter().map(|of| of.into_inner()).collect();
        assert_eq!(times_vec, vec![1.0, 2.0, 3.0]);

        // Get num time samples
        assert_eq!(data.get_num_time_samples_for_path(&path), 3);

        // Query time sample
        let value = data.query_time_sample(&path, 2.0).unwrap();
        assert_eq!(*value.get::<f64>().unwrap(), 20.0);

        // Get bracketing samples
        let (lower, upper) = data
            .get_bracketing_time_samples_for_path(&path, 2.5)
            .unwrap();
        assert_eq!(lower, 2.0);
        assert_eq!(upper, 3.0);

        // Get previous sample
        let prev = data.get_previous_time_sample_for_path(&path, 2.5).unwrap();
        assert_eq!(prev, 2.0);

        // Erase time sample
        data.erase_time_sample(&path, 2.0);
        assert_eq!(data.get_num_time_samples_for_path(&path), 2);
        assert!(data.query_time_sample(&path, 2.0).is_none());
    }

    #[test]
    fn test_list_all_time_samples() {
        let mut data = SimpleData::new();
        let path1 = Path::from_string("/World.x").unwrap();
        let path2 = Path::from_string("/World.y").unwrap();

        data.create_spec(&path1, SpecType::Attribute);
        data.create_spec(&path2, SpecType::Attribute);

        data.set_time_sample(&path1, 1.0, Value::from_f64(1.0));
        data.set_time_sample(&path1, 2.0, Value::from_f64(2.0));
        data.set_time_sample(&path2, 2.0, Value::from_f64(2.0));
        data.set_time_sample(&path2, 3.0, Value::from_f64(3.0));

        let all_times = data.list_all_time_samples();
        assert_eq!(all_times.len(), 3);
        let all_times_vec: Vec<f64> = all_times.iter().map(|of| of.into_inner()).collect();
        assert_eq!(all_times_vec, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_bracketing_time_samples() {
        let mut data = SimpleData::new();
        let path = Path::from_string("/World.x").unwrap();

        data.create_spec(&path, SpecType::Attribute);
        data.set_time_sample(&path, 1.0, Value::from_f64(1.0));
        data.set_time_sample(&path, 3.0, Value::from_f64(3.0));
        data.set_time_sample(&path, 5.0, Value::from_f64(5.0));

        // Exact match
        let (lower, upper) = data
            .get_bracketing_time_samples_for_path(&path, 3.0)
            .unwrap();
        assert_eq!(lower, 3.0);
        assert_eq!(upper, 3.0);

        // Between samples
        let (lower, upper) = data
            .get_bracketing_time_samples_for_path(&path, 2.0)
            .unwrap();
        assert_eq!(lower, 1.0);
        assert_eq!(upper, 3.0);

        // Before first
        let (lower, upper) = data
            .get_bracketing_time_samples_for_path(&path, 0.5)
            .unwrap();
        assert_eq!(lower, 1.0);
        assert_eq!(upper, 1.0);

        // After last
        let (lower, upper) = data
            .get_bracketing_time_samples_for_path(&path, 10.0)
            .unwrap();
        assert_eq!(lower, 5.0);
        assert_eq!(upper, 5.0);
    }

    #[test]
    fn test_visitor_pattern() {
        let mut data = SimpleData::new();
        let path1 = Path::from_string("/World").unwrap();
        let path2 = Path::from_string("/World/Cube").unwrap();

        data.create_spec(&path1, SpecType::Prim);
        data.create_spec(&path2, SpecType::Prim);

        struct CountVisitor {
            count: usize,
        }

        impl SpecVisitor for CountVisitor {
            fn visit_spec(&mut self, _path: &Path) -> bool {
                self.count += 1;
                true
            }

            fn done(&mut self) {}
        }

        let mut visitor = CountVisitor { count: 0 };
        data.visit_specs(&mut visitor);
        assert_eq!(visitor.count, 2);
    }

    #[test]
    fn test_visitor_early_termination() {
        let mut data = SimpleData::new();
        for i in 0..10 {
            let path = Path::from_string(&format!("/Prim{}", i)).unwrap();
            data.create_spec(&path, SpecType::Prim);
        }

        struct StopAfterThreeVisitor {
            count: usize,
        }

        impl SpecVisitor for StopAfterThreeVisitor {
            fn visit_spec(&mut self, _path: &Path) -> bool {
                self.count += 1;
                self.count < 3
            }

            fn done(&mut self) {}
        }

        let mut visitor = StopAfterThreeVisitor { count: 0 };
        data.visit_specs(&mut visitor);
        assert_eq!(visitor.count, 3);
    }

    #[test]
    fn test_copy_from() {
        let mut source = SimpleData::new();
        let path = Path::from_string("/World").unwrap();

        source.create_spec(&path, SpecType::Prim);
        source.set_field(&path, &Token::new("active"), Value::new(true));
        source.set_time_sample(&path, 1.0, Value::from_f64(10.0));

        let mut dest = SimpleData::new();
        dest.copy_from(&source);

        assert!(dest.has_spec(&path));
        assert_eq!(dest.get_spec_type(&path), SpecType::Prim);
        assert!(dest.has_field(&path, &Token::new("active")));
        assert!(dest.query_time_sample(&path, 1.0).is_some());
    }

    #[test]
    fn test_value_type() {
        let v1 = Value::new(42i32);
        assert!(v1.is::<i32>());
        assert!(!v1.is::<f64>());
        assert_eq!(*v1.get::<i32>().unwrap(), 42);

        let v2 = Value::new("hello".to_string());
        assert!(v2.is::<String>());
        assert_eq!(v2.get::<String>().unwrap(), "hello");

        let v3 = Value::new(vec![1, 2, 3]);
        assert!(v3.is::<Vec<i32>>());
        let vec = v3.get::<Vec<i32>>().cloned().unwrap();
        assert_eq!(vec, vec![1, 2, 3]);
    }

    #[test]
    fn test_create_simple_data() {
        let data = create_simple_data();
        assert!(!data.streams_data());
        assert!(data.is_empty());
    }
}
