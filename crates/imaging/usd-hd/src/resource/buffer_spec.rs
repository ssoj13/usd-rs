//! Buffer specification for describing GPU buffer layouts.

use crate::types::HdTupleType;
use std::collections::HashSet;
use usd_tf::Token;

/// Vector of buffer specifications.
pub type HdBufferSpecVector = Vec<HdBufferSpec>;

/// Describes a single named resource in a buffer array.
///
/// Specifies the buffer's value type as `HdTupleType`, which includes
/// the data type, number of components, and array size.
///
/// # Example
/// ```ignore
/// use usd_tf::Token;
/// use usd_hd::types::{HdTupleType, HdType};
/// use usd_hd::resource::HdBufferSpec;
///
/// let spec = HdBufferSpec::new(
///     Token::new("points"),
///     HdTupleType::single(HdType::FloatVec3)
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdBufferSpec {
    /// Name of the buffer resource
    pub name: Token,

    /// Data type and array size
    pub tuple_type: HdTupleType,
}

impl HdBufferSpec {
    /// Create a new buffer specification.
    pub fn new(name: Token, tuple_type: HdTupleType) -> Self {
        Self { name, tuple_type }
    }

    /// Get buffer specs from a collection of sources.
    ///
    /// Only collects specs from valid sources.
    pub fn get_specs_from_sources<T, S>(sources: T) -> HdBufferSpecVector
    where
        T: IntoIterator<Item = S>,
        S: AsRef<dyn HdBufferSourceTrait>,
    {
        let mut specs = Vec::new();
        for source in sources {
            if source.as_ref().is_valid() {
                source.as_ref().add_buffer_specs(&mut specs);
            }
        }
        specs
    }

    /// Check if `subset` is a subset of `superset`.
    ///
    /// An empty set is considered a valid subset.
    pub fn is_subset(subset: &HdBufferSpecVector, superset: &HdBufferSpecVector) -> bool {
        let superset_set: HashSet<_> = superset.iter().collect();
        subset.iter().all(|spec| superset_set.contains(spec))
    }

    /// Compute the union of two spec vectors.
    ///
    /// Duplicates are removed. Items from `spec1` appear first,
    /// preserving their relative order, followed by new items from `spec2`.
    pub fn compute_union(
        spec1: &HdBufferSpecVector,
        spec2: &HdBufferSpecVector,
    ) -> HdBufferSpecVector {
        let mut result = spec1.clone();
        let existing: HashSet<_> = spec1.iter().collect();

        for spec in spec2 {
            if !existing.contains(spec) {
                result.push(spec.clone());
            }
        }

        result
    }

    /// Compute the difference between two spec vectors.
    ///
    /// Returns specs in `spec1` that are not in `spec2`.
    /// Duplicates are removed. Order from `spec1` is preserved.
    pub fn compute_difference(
        spec1: &HdBufferSpecVector,
        spec2: &HdBufferSpecVector,
    ) -> HdBufferSpecVector {
        let spec2_set: HashSet<_> = spec2.iter().collect();
        let mut result = Vec::new();
        let mut seen = HashSet::new();

        for spec in spec1 {
            if !spec2_set.contains(spec) && seen.insert(spec) {
                result.push(spec.clone());
            }
        }

        result
    }

    /// Debug output for a vector of buffer specs.
    pub fn dump(specs: &HdBufferSpecVector) {
        println!("HdBufferSpec dump ({} specs):", specs.len());
        for (i, spec) in specs.iter().enumerate() {
            println!(
                "  [{}] name={}, type={:?}, count={}",
                i,
                spec.name.as_str(),
                spec.tuple_type.type_,
                spec.tuple_type.count
            );
        }
    }

    /// Compute hash value for this spec.
    pub fn hash_value(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl PartialOrd for HdBufferSpec {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HdBufferSpec {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.name.as_str().cmp(other.name.as_str()) {
            std::cmp::Ordering::Equal => self.tuple_type.cmp(&other.tuple_type),
            ord => ord,
        }
    }
}

/// Trait for buffer data sources.
///
/// Provides interface for buffer sources to validate themselves
/// and contribute their buffer specifications to a collection.
///
/// # Note
///
/// This is a temporary trait that will be replaced with the actual
/// buffer source implementation from OpenUSD.
pub trait HdBufferSourceTrait {
    /// Check if this buffer source is valid.
    fn is_valid(&self) -> bool;

    /// Add buffer specifications to the provided vector.
    fn add_buffer_specs(&self, specs: &mut HdBufferSpecVector);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HdType;

    #[test]
    fn test_buffer_spec_creation() {
        let spec = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));

        assert_eq!(spec.name.as_str(), "points");
        assert_eq!(spec.tuple_type.type_, HdType::FloatVec3);
        assert_eq!(spec.tuple_type.count, 1);
    }

    #[test]
    fn test_buffer_spec_equality() {
        let spec1 = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));
        let spec2 = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));
        let spec3 = HdBufferSpec::new(
            Token::new("normals"),
            HdTupleType::new(HdType::FloatVec3, 1),
        );

        assert_eq!(spec1, spec2);
        assert_ne!(spec1, spec3);
    }

    #[test]
    fn test_buffer_spec_ordering() {
        let spec1 = HdBufferSpec::new(
            Token::new("normals"),
            HdTupleType::new(HdType::FloatVec3, 1),
        );
        let spec2 = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));

        assert!(spec1 < spec2);
    }

    #[test]
    fn test_is_subset() {
        let spec1 = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));
        let spec2 = HdBufferSpec::new(
            Token::new("normals"),
            HdTupleType::new(HdType::FloatVec3, 1),
        );

        let subset = vec![spec1.clone()];
        let superset = vec![spec1.clone(), spec2.clone()];

        assert!(HdBufferSpec::is_subset(&subset, &superset));
        assert!(!HdBufferSpec::is_subset(&superset, &subset));
        assert!(HdBufferSpec::is_subset(&vec![], &superset));
    }

    #[test]
    fn test_compute_union() {
        let spec1 = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));
        let spec2 = HdBufferSpec::new(
            Token::new("normals"),
            HdTupleType::new(HdType::FloatVec3, 1),
        );
        let spec3 = HdBufferSpec::new(Token::new("colors"), HdTupleType::new(HdType::FloatVec4, 1));

        let vec1 = vec![spec1.clone(), spec2.clone()];
        let vec2 = vec![spec2.clone(), spec3.clone()];

        let union = HdBufferSpec::compute_union(&vec1, &vec2);

        assert_eq!(union.len(), 3);
        assert!(union.contains(&spec1));
        assert!(union.contains(&spec2));
        assert!(union.contains(&spec3));
    }

    #[test]
    fn test_compute_difference() {
        let spec1 = HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1));
        let spec2 = HdBufferSpec::new(
            Token::new("normals"),
            HdTupleType::new(HdType::FloatVec3, 1),
        );
        let spec3 = HdBufferSpec::new(Token::new("colors"), HdTupleType::new(HdType::FloatVec4, 1));

        let vec1 = vec![spec1.clone(), spec2.clone(), spec3.clone()];
        let vec2 = vec![spec2.clone()];

        let diff = HdBufferSpec::compute_difference(&vec1, &vec2);

        assert_eq!(diff.len(), 2);
        assert!(diff.contains(&spec1));
        assert!(diff.contains(&spec3));
        assert!(!diff.contains(&spec2));
    }
}
