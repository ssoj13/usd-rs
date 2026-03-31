#![allow(dead_code)]

//! Primvar output buffer source for GPU external computations.
//!
//! A "null" buffer source that reserves space in a buffer array range for
//! the output of an ExtComputation GPU kernel. It does not provide CPU data;
//! the actual values are written by the GPU compute dispatch.
//!
//! Matches C++ `HdStExtCompGpuPrimvarBufferSource`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

use crate::flat_normals::HdBufferSpec;
use usd_hd::types::HdType;

/// GPU primvar buffer source for external computations.
///
/// Acts as a placeholder that tells the resource registry to allocate
/// buffer space for a primvar that will be filled by a GPU computation.
/// The source itself contains no CPU data -- it merely describes the
/// layout (name, type, element count).
///
/// Used during the resolve phase to reserve output slots in the
/// destination buffer array range.
#[derive(Debug, Clone)]
pub struct ExtCompGpuPrimvarBufferSource {
    /// Primvar name
    name: Token,
    /// Tuple type (data type + count)
    tuple_type: HdType,
    /// Number of elements to allocate
    num_elements: usize,
    /// Source computation prim path (for debugging)
    comp_id: SdfPath,
}

impl ExtCompGpuPrimvarBufferSource {
    /// Create a new GPU primvar buffer source.
    pub fn new(name: Token, tuple_type: HdType, num_elements: usize, comp_id: SdfPath) -> Self {
        Self {
            name,
            tuple_type,
            num_elements,
            comp_id,
        }
    }

    /// Compute a content hash for deduplication.
    ///
    /// Per C++ reference: hash only comp_id + primvar name.
    /// Intentionally excludes num_elements to disable primvar sharing
    /// for computed primvars (matching C++ `TfHash::Combine(_compId, _name)`).
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.comp_id.hash(&mut hasher);
        self.name.as_str().hash(&mut hasher);
        hasher.finish()
    }

    /// Resolve this source.
    ///
    /// A null buffer source is always considered resolved since it
    /// carries no CPU data to upload.
    pub fn resolve(&self) -> bool {
        true
    }

    /// Get the primvar name.
    pub fn get_name(&self) -> &Token {
        &self.name
    }

    /// Get the number of output elements.
    pub fn get_num_elements(&self) -> usize {
        self.num_elements
    }

    /// Get the data type.
    pub fn get_tuple_type(&self) -> HdType {
        self.tuple_type
    }

    /// Get the buffer spec for this primvar.
    pub fn get_buffer_specs(&self) -> Vec<HdBufferSpec> {
        vec![HdBufferSpec {
            name: self.name.clone(),
            data_type: self.tuple_type,
        }]
    }

    /// Check validity.
    ///
    /// Valid if the tuple type is not Invalid and element count > 0.
    pub fn is_valid(&self) -> bool {
        self.tuple_type != HdType::Invalid && self.num_elements > 0
    }

    /// Get the source computation path.
    pub fn get_comp_id(&self) -> &SdfPath {
        &self.comp_id
    }
}

/// Shared pointer alias.
pub type ExtCompGpuPrimvarBufferSourceSharedPtr = Arc<ExtCompGpuPrimvarBufferSource>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let src = ExtCompGpuPrimvarBufferSource::new(
            Token::new("points"),
            HdType::FloatVec3,
            100,
            SdfPath::from_string("/comp").unwrap(),
        );

        assert_eq!(src.get_name(), &Token::new("points"));
        assert_eq!(src.get_num_elements(), 100);
        assert_eq!(src.get_tuple_type(), HdType::FloatVec3);
        assert!(src.is_valid());
    }

    #[test]
    fn test_resolve_always_true() {
        let src = ExtCompGpuPrimvarBufferSource::new(
            Token::new("normals"),
            HdType::FloatVec3,
            50,
            SdfPath::from_string("/comp").unwrap(),
        );
        assert!(src.resolve());
    }

    #[test]
    fn test_buffer_specs() {
        let src = ExtCompGpuPrimvarBufferSource::new(
            Token::new("normals"),
            HdType::FloatVec3,
            50,
            SdfPath::from_string("/comp").unwrap(),
        );
        let specs = src.get_buffer_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, Token::new("normals"));
        assert_eq!(specs[0].data_type, HdType::FloatVec3);
    }

    #[test]
    fn test_invalid_type() {
        let src = ExtCompGpuPrimvarBufferSource::new(
            Token::new("bad"),
            HdType::Invalid,
            100,
            SdfPath::from_string("/comp").unwrap(),
        );
        assert!(!src.is_valid());
    }

    #[test]
    fn test_zero_elements() {
        let src = ExtCompGpuPrimvarBufferSource::new(
            Token::new("empty"),
            HdType::FloatVec3,
            0,
            SdfPath::from_string("/comp").unwrap(),
        );
        assert!(!src.is_valid());
    }

    #[test]
    fn test_hash_differs() {
        let s1 = ExtCompGpuPrimvarBufferSource::new(
            Token::new("a"),
            HdType::FloatVec3,
            100,
            SdfPath::from_string("/comp1").unwrap(),
        );
        let s2 = ExtCompGpuPrimvarBufferSource::new(
            Token::new("b"),
            HdType::FloatVec3,
            100,
            SdfPath::from_string("/comp2").unwrap(),
        );
        assert_ne!(s1.compute_hash(), s2.compute_hash());
    }
}
