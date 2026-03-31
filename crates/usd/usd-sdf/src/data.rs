//! Concrete in-memory data storage implementation.
//!
//! `Data` provides efficient storage for scene description data. It uses
//! optimized data structures for spec and field storage, matching the
//! reference OpenUSD implementation.

use std::collections::{BTreeMap, HashMap};

use ordered_float::OrderedFloat;

use usd_tf::Token;

use super::abstract_data::{AbstractData, SpecVisitor, Value};
use super::path::Path;

use super::types::{SpecType, TimeSamples};

// ============================================================================
// SpecData - Storage for a single spec
// ============================================================================

/// Storage for a single spec's data.
///
/// Stores spec type and fields as a vector of (field_name, value) pairs.
/// Using a vector is more cache-friendly than a HashMap for the typical
/// small number of fields per spec.
#[derive(Debug, Clone)]
struct SpecData {
    /// The type of this spec.
    spec_type: SpecType,
    /// Fields stored as (name, value) pairs.
    /// Using Vec instead of HashMap for better cache locality
    /// with small numbers of fields (typical case).
    fields: Vec<(Token, Value)>,
}

impl SpecData {
    /// Creates a new spec data with the given type.
    fn new(spec_type: SpecType) -> Self {
        Self {
            spec_type,
            fields: Vec::new(),
        }
    }

    /// Finds a field by name, returning immutable reference.
    fn get_field(&self, field_name: &Token) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(name, _)| name == field_name)
            .map(|(_, value)| value)
    }

    /// Finds a field by name, returning mutable reference.
    fn get_field_mut(&mut self, field_name: &Token) -> Option<&mut Value> {
        self.fields
            .iter_mut()
            .find(|(name, _)| name == field_name)
            .map(|(_, value)| value)
    }

    /// Sets a field value, creating it if it doesn't exist.
    fn set_field(&mut self, field_name: &Token, value: Value) {
        if let Some(existing) = self.get_field_mut(field_name) {
            *existing = value;
        } else {
            self.fields.push((field_name.clone(), value));
        }
    }

    /// Removes a field by name.
    fn erase_field(&mut self, field_name: &Token) -> bool {
        if let Some(pos) = self.fields.iter().position(|(name, _)| name == field_name) {
            self.fields.remove(pos);
            true
        } else {
            false
        }
    }

    /// Returns true if field exists.
    fn has_field(&self, field_name: &Token) -> bool {
        self.fields.iter().any(|(name, _)| name == field_name)
    }

    /// Returns list of all field names.
    fn list_fields(&self) -> Vec<Token> {
        self.fields.iter().map(|(name, _)| name.clone()).collect()
    }
}

// ============================================================================
// TimeSampleMap - Storage for time samples
// ============================================================================

/// Time samples storage for a single path.
///
/// Uses BTreeMap for efficient ordered time lookup and range queries.
/// OrderedFloat wrapper allows f64 to be used as map keys.
type TimeSampleMap = BTreeMap<OrderedFloat<f64>, Value>;

// ============================================================================
// Data - Main storage implementation
// ============================================================================

/// Default in-memory storage for scene description data.
///
/// `Data` provides efficient storage using hash tables and ordered maps.
/// This matches the OpenUSD SdfData implementation with:
///
/// - HashMap for spec lookup by path (O(1) access)
/// - Vector-based field storage per spec (cache-friendly for small field counts)
/// - BTreeMap for time samples (efficient ordered time queries)
///
/// # Thread Safety
///
/// `Data` is `Send` but not `Sync` - it should only be accessed from one
/// thread at a time. Layers typically handle synchronization.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::{Data, Path, SpecType, Value};
/// use usd_tf::Token;
///
/// let mut data = Data::new();
///
/// // Create a prim spec
/// let path = Path::from_string("/World").unwrap();
/// data.create_spec(&path, SpecType::Prim);
///
/// // Set fields
/// data.set_field(&path, &Token::new("active"), Value::new(true));
///
/// // Query data
/// assert!(data.has_spec(&path));
/// assert!(data.has_field(&path, &Token::new("active")));
/// ```
/// Data implements Clone for layer content transfer.
#[derive(Clone)]
pub struct Data {
    /// Spec storage: path -> (spec_type, fields)
    specs: HashMap<Path, SpecData>,
    /// Time sample storage: path -> (time -> value)
    time_samples: HashMap<Path, TimeSampleMap>,
}

impl Data {
    /// Creates a new data object with pseudo-root spec.
    ///
    /// The pseudo-root spec at "/" must always exist for proper layer operation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Data;
    ///
    /// let data = Data::new();
    /// assert!(data.has_spec(&usd_sdf::Path::absolute_root()));
    /// ```
    pub fn new() -> Self {
        let mut specs = HashMap::new();
        // The pseudo-root spec must always exist (matches OpenUSD behavior)
        specs.insert(Path::absolute_root(), SpecData::new(SpecType::PseudoRoot));
        Self {
            specs,
            time_samples: HashMap::new(),
        }
    }

    /// Returns true if this data contains no user content.
    ///
    /// A Data with only the pseudo-root spec is considered empty
    /// (pseudo-root is always present but doesn't count as user content).
    pub fn is_empty(&self) -> bool {
        let len = self.specs.len();
        if len == 0 {
            return true;
        }
        if len == 1 {
            return self.specs.contains_key(&Path::absolute_root());
        }
        false
    }

    /// Returns the number of specs stored.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Clears all data.
    pub fn clear(&mut self) {
        self.specs.clear();
        self.time_samples.clear();
    }

    // ========================================================================
    // Internal field access helpers
    // ========================================================================

    /// Gets immutable reference to field value.
    fn get_field_value(&self, path: &Path, field_name: &Token) -> Option<&Value> {
        self.specs
            .get(path)
            .and_then(|spec| spec.get_field(field_name))
    }

    /// Gets spec type and field value in one lookup.
    fn get_spec_type_and_field(
        &self,
        path: &Path,
        field_name: &Token,
    ) -> (SpecType, Option<&Value>) {
        if let Some(spec) = self.specs.get(path) {
            (spec.spec_type, spec.get_field(field_name))
        } else {
            (SpecType::Unknown, None)
        }
    }

    // ========================================================================
    // Time sample helpers
    // ========================================================================

    /// Converts f64 time to OrderedFloat for use as map key.
    #[inline]
    fn to_ordered_time(time: f64) -> OrderedFloat<f64> {
        OrderedFloat(time)
    }

    /// Gets time sample map for path, creating if needed.
    fn get_or_create_time_samples(&mut self, path: &Path) -> &mut TimeSampleMap {
        self.time_samples.entry(path.clone()).or_default()
    }

    /// Implements bracketing time sample search.
    ///
    /// Returns (lower, upper) where lower <= time <= upper.
    /// If exact match, both values are equal.
    fn find_bracketing_times(samples: &TimeSampleMap, time: f64) -> Option<(f64, f64)> {
        if samples.is_empty() {
            return None;
        }

        let ordered_time = Self::to_ordered_time(time);

        // Get first and last sample times (safe: checked is_empty above)
        let first_time = samples.keys().next().expect("samples not empty").0;
        let last_time = samples.keys().next_back().expect("samples not empty").0;

        if time <= first_time {
            // Time is at or before first sample
            Some((first_time, first_time))
        } else if time >= last_time {
            // Time is at or after last sample
            Some((last_time, last_time))
        } else {
            // Time is between samples
            // Find first sample >= time
            if let Some((&upper_key, _)) = samples.range(ordered_time..).next() {
                if upper_key.0 == time {
                    // Exact match
                    Some((time, time))
                } else {
                    // Find previous sample
                    if let Some((&lower_key, _)) = samples.range(..ordered_time).next_back() {
                        Some((lower_key.0, upper_key.0))
                    } else {
                        // Shouldn't happen if time > first_time
                        Some((upper_key.0, upper_key.0))
                    }
                }
            } else {
                // Shouldn't happen if time < last_time
                Some((last_time, last_time))
            }
        }
    }
}

impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// AbstractData Implementation
// ============================================================================

impl AbstractData for Data {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn streams_data(&self) -> bool {
        false
    }

    fn is_detached(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        // Delegate to the inherent method
        Data::is_empty(self)
    }

    fn create_spec(&mut self, path: &Path, spec_type: SpecType) {
        self.specs
            .entry(path.clone())
            .and_modify(|spec| spec.spec_type = spec_type)
            .or_insert_with(|| SpecData::new(spec_type));
    }

    fn has_spec(&self, path: &Path) -> bool {
        self.specs.contains_key(path)
    }

    fn erase_spec(&mut self, path: &Path) {
        self.specs.remove(path);
        self.time_samples.remove(path);
    }

    fn move_spec(&mut self, old_path: &Path, new_path: &Path) {
        // Collect old_path and all descendant specs to move
        let old_prefix = old_path.as_str();
        let new_prefix = new_path.as_str();

        // Collect all paths that start with old_path (self + descendants)
        let paths_to_move: Vec<(Path, Path)> = self
            .specs
            .keys()
            .filter(|p| {
                let s = p.as_str();
                s == old_prefix
                    || s.starts_with(old_prefix)
                        && s.as_bytes().get(old_prefix.len()) == Some(&b'/')
            })
            .map(|p| {
                let suffix = &p.as_str()[old_prefix.len()..];
                let new_p = Path::from_string(&format!("{}{}", new_prefix, suffix))
                    .unwrap_or_else(Path::empty);
                (p.clone(), new_p)
            })
            .collect();

        for (old_p, new_p) in paths_to_move {
            if new_p.is_empty() {
                continue;
            }
            if let Some(spec_data) = self.specs.remove(&old_p) {
                self.specs.insert(new_p.clone(), spec_data);
            }
            if let Some(samples) = self.time_samples.remove(&old_p) {
                self.time_samples.insert(new_p, samples);
            }
        }
    }

    fn get_spec_type(&self, path: &Path) -> SpecType {
        self.specs
            .get(path)
            .map(|spec| spec.spec_type)
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
            .map(|spec| spec.has_field(field_name))
            .unwrap_or(false)
    }

    fn get_field(&self, path: &Path, field_name: &Token) -> Option<Value> {
        self.get_field_value(path, field_name).cloned()
    }

    fn set_field(&mut self, path: &Path, field_name: &Token, value: Value) {
        if let Some(spec) = self.specs.get_mut(path) {
            spec.set_field(field_name, value);
        }
    }

    fn erase_field(&mut self, path: &Path, field_name: &Token) {
        if let Some(spec) = self.specs.get_mut(path) {
            spec.erase_field(field_name);
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
            .map(|spec| spec.list_fields())
            .unwrap_or_default()
    }

    fn has_spec_and_field(&self, path: &Path, field_name: &Token) -> (bool, SpecType) {
        let (spec_type, field_value) = self.get_spec_type_and_field(path, field_name);
        let has = spec_type != SpecType::Unknown && field_value.is_some();
        (has, spec_type)
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

    fn get_num_time_samples_for_path(&self, path: &Path) -> usize {
        self.time_samples
            .get(path)
            .map(|samples| samples.len())
            .unwrap_or(0)
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
        self.time_samples
            .get(path)
            .and_then(|samples| Self::find_bracketing_times(samples, time))
    }

    fn get_previous_time_sample_for_path(&self, path: &Path, time: f64) -> Option<f64> {
        use ordered_float::OrderedFloat;
        // O(log n) via BTreeMap::range instead of enumerating all samples
        self.time_samples
            .get(path)
            .and_then(|samples| {
                samples
                    .range(..OrderedFloat(time))
                    .next_back()
                    .map(|(k, _)| k.into_inner())
            })
    }

    fn query_time_sample(&self, path: &Path, time: f64) -> Option<Value> {
        self.time_samples
            .get(path)
            .and_then(|samples| samples.get(&Self::to_ordered_time(time)).cloned())
    }

    fn set_time_sample(&mut self, path: &Path, time: f64, value: Value) {
        let samples = self.get_or_create_time_samples(path);
        samples.insert(Self::to_ordered_time(time), value);
    }

    fn erase_time_sample(&mut self, path: &Path, time: f64) {
        if let Some(samples) = self.time_samples.get_mut(path) {
            samples.remove(&Self::to_ordered_time(time));
            if samples.is_empty() {
                self.time_samples.remove(path);
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a new `Data` object.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::create_data;
///
/// let data = create_data();
/// assert!(data.is_empty());
/// ```
pub fn create_data() -> Box<dyn AbstractData> {
    Box::new(Data::new())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_creation() {
        let data = Data::new();
        assert!(!data.streams_data());
        assert!(data.is_detached());
        // Data with only pseudo-root spec is considered empty (for user content purposes)
        assert!(data.is_empty());
        assert_eq!(data.len(), 1); // but len() still shows 1 spec
        assert!(data.has_spec(&Path::absolute_root()));
        assert_eq!(
            data.get_spec_type(&Path::absolute_root()),
            SpecType::PseudoRoot
        );
    }

    #[test]
    fn test_spec_operations() {
        let mut data = Data::new();
        let path = Path::from_string("/World").unwrap();

        // Create spec (Data already has pseudo-root)
        data.create_spec(&path, SpecType::Prim);
        assert!(data.has_spec(&path));
        assert_eq!(data.get_spec_type(&path), SpecType::Prim);
        assert_eq!(data.len(), 2); // pseudo-root + /World
        assert!(!data.is_empty()); // Now has user content

        // Change spec type
        data.create_spec(&path, SpecType::PseudoRoot);
        assert_eq!(data.get_spec_type(&path), SpecType::PseudoRoot);
        assert_eq!(data.len(), 2); // Still two specs

        // Erase spec
        data.erase_spec(&path);
        assert!(!data.has_spec(&path));
        assert_eq!(data.get_spec_type(&path), SpecType::Unknown);
        // Pseudo-root still exists, but data is "empty" (no user content)
        assert!(data.is_empty());
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_move_spec() {
        let mut data = Data::new();
        let old_path = Path::from_string("/Old").unwrap();
        let new_path = Path::from_string("/New").unwrap();
        let field_name = Token::new("active");

        data.create_spec(&old_path, SpecType::Prim);
        data.set_field(&old_path, &field_name, Value::new(true));
        data.set_time_sample(&old_path, 1.0, Value::from_f64(10.0));

        // Move spec
        data.move_spec(&old_path, &new_path);

        // Old path should be gone
        assert!(!data.has_spec(&old_path));

        // New path should have all data
        assert!(data.has_spec(&new_path));
        assert_eq!(data.get_spec_type(&new_path), SpecType::Prim);
        assert!(data.has_field(&new_path, &field_name));
        assert_eq!(data.get_num_time_samples_for_path(&new_path), 1);
    }

    #[test]
    fn test_field_operations() {
        let mut data = Data::new();
        let path = Path::from_string("/World").unwrap();
        let field_name = Token::new("active");

        data.create_spec(&path, SpecType::Prim);

        // Set field
        data.set_field(&path, &field_name, Value::new(true));
        assert!(data.has_field(&path, &field_name));

        // Get field
        let value = data.get_field(&path, &field_name).unwrap();
        assert_eq!(*value.downcast::<bool>().unwrap(), true);

        // Update field
        data.set_field(&path, &field_name, Value::new(false));
        let value = data.get_field(&path, &field_name).unwrap();
        assert_eq!(*value.downcast::<bool>().unwrap(), false);

        // List fields
        let fields = data.list_fields(&path);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0], field_name);

        // Erase field
        data.erase_field(&path, &field_name);
        assert!(!data.has_field(&path, &field_name));
        assert!(data.list_fields(&path).is_empty());
    }

    #[test]
    fn test_multiple_fields() {
        let mut data = Data::new();
        let path = Path::from_string("/World").unwrap();

        data.create_spec(&path, SpecType::Prim);

        // Set multiple fields
        data.set_field(&path, &Token::new("active"), Value::new(true));
        data.set_field(&path, &Token::new("kind"), Value::new("group".to_string()));
        data.set_field(
            &path,
            &Token::new("typeName"),
            Value::new("Xform".to_string()),
        );

        // List all fields
        let fields = data.list_fields(&path);
        assert_eq!(fields.len(), 3);

        // Verify each field
        assert!(data.has_field(&path, &Token::new("active")));
        assert!(data.has_field(&path, &Token::new("kind")));
        assert!(data.has_field(&path, &Token::new("typeName")));
    }

    #[test]
    fn test_has_spec_and_field() {
        let mut data = Data::new();
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
    fn test_time_samples() {
        let mut data = Data::new();
        let path = Path::from_string("/World.xform").unwrap();

        data.create_spec(&path, SpecType::Attribute);

        // Set time samples
        data.set_time_sample(&path, 1.0, Value::from_f64(10.0));
        data.set_time_sample(&path, 2.0, Value::from_f64(20.0));
        data.set_time_sample(&path, 3.0, Value::from_f64(30.0));

        // Count samples
        assert_eq!(data.get_num_time_samples_for_path(&path), 3);

        // List samples
        let times = data.list_time_samples_for_path(&path);
        let times_vec: Vec<f64> = times.iter().map(|of| of.into_inner()).collect();
        assert_eq!(times_vec, vec![1.0, 2.0, 3.0]);

        // Query sample
        let value = data.query_time_sample(&path, 2.0).unwrap();
        assert_eq!(*value.downcast::<f64>().unwrap(), 20.0);

        // Erase sample
        data.erase_time_sample(&path, 2.0);
        assert_eq!(data.get_num_time_samples_for_path(&path), 2);
        assert!(data.query_time_sample(&path, 2.0).is_none());
    }

    #[test]
    fn test_bracketing_time_samples() {
        let mut data = Data::new();
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

        // Between second and third
        let (lower, upper) = data
            .get_bracketing_time_samples_for_path(&path, 4.0)
            .unwrap();
        assert_eq!(lower, 3.0);
        assert_eq!(upper, 5.0);
    }

    #[test]
    fn test_get_previous_time_sample() {
        let mut data = Data::new();
        let path = Path::from_string("/World.x").unwrap();

        data.create_spec(&path, SpecType::Attribute);
        data.set_time_sample(&path, 1.0, Value::from_f64(1.0));
        data.set_time_sample(&path, 3.0, Value::from_f64(3.0));
        data.set_time_sample(&path, 5.0, Value::from_f64(5.0));

        // Previous sample exists
        assert_eq!(
            data.get_previous_time_sample_for_path(&path, 3.5),
            Some(3.0)
        );
        assert_eq!(
            data.get_previous_time_sample_for_path(&path, 6.0),
            Some(5.0)
        );

        // No previous sample
        assert_eq!(data.get_previous_time_sample_for_path(&path, 0.5), None);
        assert_eq!(data.get_previous_time_sample_for_path(&path, 1.0), None);
    }

    #[test]
    fn test_list_all_time_samples() {
        let mut data = Data::new();
        let path1 = Path::from_string("/A.x").unwrap();
        let path2 = Path::from_string("/B.y").unwrap();

        data.create_spec(&path1, SpecType::Attribute);
        data.create_spec(&path2, SpecType::Attribute);

        data.set_time_sample(&path1, 1.0, Value::from_f64(1.0));
        data.set_time_sample(&path1, 2.0, Value::from_f64(2.0));
        data.set_time_sample(&path2, 2.0, Value::from_f64(2.0)); // Duplicate time
        data.set_time_sample(&path2, 3.0, Value::from_f64(3.0));

        let all_times = data.list_all_time_samples();
        let all_times_vec: Vec<f64> = all_times.iter().map(|of| of.into_inner()).collect();
        assert_eq!(all_times_vec, vec![1.0, 2.0, 3.0]); // Unique times, sorted
    }

    #[test]
    fn test_get_bracketing_time_samples_global() {
        let mut data = Data::new();
        let path1 = Path::from_string("/A.x").unwrap();
        let path2 = Path::from_string("/B.y").unwrap();

        data.create_spec(&path1, SpecType::Attribute);
        data.create_spec(&path2, SpecType::Attribute);

        data.set_time_sample(&path1, 1.0, Value::from_f64(1.0));
        data.set_time_sample(&path2, 5.0, Value::from_f64(5.0));

        // Should bracket across all paths
        let (lower, upper) = data.get_bracketing_time_samples(3.0).unwrap();
        assert_eq!(lower, 1.0);
        assert_eq!(upper, 5.0);
    }

    #[test]
    fn test_visitor_pattern() {
        let mut data = Data::new();
        let path1 = Path::from_string("/World").unwrap();
        let path2 = Path::from_string("/World/Cube").unwrap();
        let path3 = Path::from_string("/World/Sphere").unwrap();

        data.create_spec(&path1, SpecType::Prim);
        data.create_spec(&path2, SpecType::Prim);
        data.create_spec(&path3, SpecType::Prim);

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
        assert_eq!(visitor.count, 4); // 3 prims + pseudo-root
    }

    #[test]
    fn test_visitor_early_termination() {
        let mut data = Data::new();
        for i in 0..10 {
            let path = Path::from_string(&format!("/Prim{}", i)).unwrap();
            data.create_spec(&path, SpecType::Prim);
        }

        struct StopAfterThree {
            count: usize,
        }

        impl SpecVisitor for StopAfterThree {
            fn visit_spec(&mut self, _path: &Path) -> bool {
                self.count += 1;
                self.count < 3
            }

            fn done(&mut self) {}
        }

        let mut visitor = StopAfterThree { count: 0 };
        data.visit_specs(&mut visitor);
        assert_eq!(visitor.count, 3);
    }

    #[test]
    fn test_clear() {
        let mut data = Data::new();
        let path = Path::from_string("/World").unwrap();

        data.create_spec(&path, SpecType::Prim);
        data.set_field(&path, &Token::new("active"), Value::new(true));
        data.set_time_sample(&path, 1.0, Value::from_f64(1.0));

        assert!(!data.is_empty());
        assert_eq!(data.len(), 2); // pseudo-root + /World

        data.clear();

        // clear() removes everything including pseudo-root
        assert!(data.is_empty());
        assert_eq!(data.len(), 0);
        assert!(!data.has_spec(&path));
    }

    #[test]
    fn test_erase_spec_removes_time_samples() {
        let mut data = Data::new();
        let path = Path::from_string("/World.x").unwrap();

        data.create_spec(&path, SpecType::Attribute);
        data.set_time_sample(&path, 1.0, Value::from_f64(1.0));
        data.set_time_sample(&path, 2.0, Value::from_f64(2.0));

        assert_eq!(data.get_num_time_samples_for_path(&path), 2);

        data.erase_spec(&path);

        assert!(!data.has_spec(&path));
        assert_eq!(data.get_num_time_samples_for_path(&path), 0);
    }

    #[test]
    fn test_create_data_helper() {
        let data = create_data();
        assert!(!data.streams_data());
        // Data with only pseudo-root spec is considered empty (for user content purposes)
        assert!(data.is_empty());
    }

    #[test]
    fn test_field_update_vs_insert() {
        let mut data = Data::new();
        let path = Path::from_string("/World").unwrap();
        let field_name = Token::new("count");

        data.create_spec(&path, SpecType::Prim);

        // First insert
        data.set_field(&path, &field_name, Value::new(1));
        assert_eq!(data.list_fields(&path).len(), 1);

        // Update existing - should not duplicate
        data.set_field(&path, &field_name, Value::new(2));
        assert_eq!(data.list_fields(&path).len(), 1);

        let value = data.get_field(&path, &field_name).unwrap();
        assert_eq!(*value.downcast::<i32>().unwrap(), 2);
    }

    #[test]
    fn test_time_sample_overwrite() {
        let mut data = Data::new();
        let path = Path::from_string("/World.x").unwrap();

        data.create_spec(&path, SpecType::Attribute);

        // Set initial value
        data.set_time_sample(&path, 1.0, Value::from_f64(10.0));
        assert_eq!(data.get_num_time_samples_for_path(&path), 1);

        // Overwrite same time
        data.set_time_sample(&path, 1.0, Value::from_f64(20.0));
        assert_eq!(data.get_num_time_samples_for_path(&path), 1);

        let value = data.query_time_sample(&path, 1.0).unwrap();
        assert_eq!(*value.downcast::<f64>().unwrap(), 20.0);
    }

    #[test]
    fn test_empty_path_operations() {
        let data = Data::new();
        let path = Path::from_string("/").unwrap();

        // Pseudo-root is created automatically
        assert!(data.has_spec(&path));
        assert_eq!(data.get_spec_type(&path), SpecType::PseudoRoot);
    }
}
