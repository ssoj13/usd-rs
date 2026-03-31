//! UsdSkelAnimMapper - helper for remapping animation data between orderings.
//!
//! Port of pxr/usd/usdSkel/animMapper.h/cpp

use std::collections::HashMap;
use usd_gf::{Matrix4d, Matrix4f};
use usd_tf::Token;

/// Mapping flags for AnimMapper.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MapFlags(u32);

impl MapFlags {
    const NULL_MAP: u32 = 0;
    const SOME_SOURCE_VALUES_MAP_TO_TARGET: u32 = 0x1;
    const ALL_SOURCE_VALUES_MAP_TO_TARGET: u32 = 0x2;
    const SOURCE_OVERRIDES_ALL_TARGET_VALUES: u32 = 0x4;
    const ORDERED_MAP: u32 = 0x8;

    const IDENTITY_MAP: u32 = Self::ALL_SOURCE_VALUES_MAP_TO_TARGET
        | Self::SOURCE_OVERRIDES_ALL_TARGET_VALUES
        | Self::ORDERED_MAP;

    const NON_NULL_MAP: u32 =
        Self::SOME_SOURCE_VALUES_MAP_TO_TARGET | Self::ALL_SOURCE_VALUES_MAP_TO_TARGET;

    fn new(flags: u32) -> Self {
        Self(flags)
    }

    fn is_identity(&self) -> bool {
        (self.0 & Self::IDENTITY_MAP) == Self::IDENTITY_MAP
    }

    fn is_sparse(&self) -> bool {
        (self.0 & Self::SOURCE_OVERRIDES_ALL_TARGET_VALUES) == 0
    }

    fn is_null(&self) -> bool {
        (self.0 & Self::NON_NULL_MAP) == 0
    }

    fn is_ordered(&self) -> bool {
        (self.0 & Self::ORDERED_MAP) != 0
    }
}

/// Helper class for remapping vectorized animation data from
/// one ordering of tokens to another.
///
/// Matches C++ `UsdSkelAnimMapper`.
#[derive(Clone, Debug)]
pub struct AnimMapper {
    /// Size of the output map.
    target_size: usize,
    /// For ordered mappings, an offset into the output array.
    offset: usize,
    /// For unordered mappings, index map from source to target indices.
    index_map: Vec<i32>,
    /// Mapping flags.
    flags: MapFlags,
}

impl Default for AnimMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimMapper {
    /// Construct a null mapper.
    pub fn new() -> Self {
        Self {
            target_size: 0,
            offset: 0,
            index_map: Vec::new(),
            flags: MapFlags::new(MapFlags::NULL_MAP),
        }
    }

    /// Construct an identity mapper for remapping a range of `size` elements.
    /// An identity mapper indicates no remapping is required.
    pub fn identity(size: usize) -> Self {
        Self {
            target_size: size,
            offset: 0,
            index_map: Vec::new(),
            flags: MapFlags::new(MapFlags::IDENTITY_MAP),
        }
    }

    /// Construct a mapper for mapping data from `source_order` to `target_order`.
    pub fn from_orders(source_order: &[Token], target_order: &[Token]) -> Self {
        let source_size = source_order.len();
        let target_size = target_order.len();

        if source_size == 0 || target_size == 0 {
            return Self {
                target_size,
                offset: 0,
                index_map: Vec::new(),
                flags: MapFlags::new(MapFlags::NULL_MAP),
            };
        }

        // Determine if this is an ordered mapping with a simple offset
        // (includes identity maps)
        if let Some(pos) = target_order.iter().position(|t| t == &source_order[0]) {
            if (pos + source_size) <= target_size {
                // Check if source matches target starting at pos
                let matches = source_order
                    .iter()
                    .zip(target_order[pos..].iter())
                    .all(|(s, t)| s == t);

                if matches {
                    let mut flags =
                        MapFlags::ORDERED_MAP | MapFlags::ALL_SOURCE_VALUES_MAP_TO_TARGET;

                    if pos == 0 && source_size == target_size {
                        flags |= MapFlags::SOURCE_OVERRIDES_ALL_TARGET_VALUES;
                    }

                    return Self {
                        target_size,
                        offset: pos,
                        index_map: Vec::new(),
                        flags: MapFlags::new(flags),
                    };
                }
            }
        }

        // No ordered mapping - create an unordered indexed mapping
        let mut target_map: HashMap<&Token, i32> = HashMap::new();
        for (i, token) in target_order.iter().enumerate() {
            target_map.insert(token, i as i32);
        }

        let mut index_map = Vec::with_capacity(source_size);
        let mut mapped_count = 0usize;
        let mut target_mapped = vec![false; target_size];

        for token in source_order {
            if let Some(&idx) = target_map.get(token) {
                index_map.push(idx);
                target_mapped[idx as usize] = true;
                mapped_count += 1;
            } else {
                index_map.push(-1);
            }
        }

        let mut flags = if mapped_count == source_size {
            MapFlags::ALL_SOURCE_VALUES_MAP_TO_TARGET
        } else {
            MapFlags::SOME_SOURCE_VALUES_MAP_TO_TARGET
        };

        if target_mapped.iter().all(|&v| v) {
            flags |= MapFlags::SOURCE_OVERRIDES_ALL_TARGET_VALUES;
        }

        Self {
            target_size,
            offset: 0,
            index_map,
            flags: MapFlags::new(flags),
        }
    }

    /// Returns true if this is an identity map.
    /// The source and target orders of an identity map are identical.
    pub fn is_identity(&self) -> bool {
        self.flags.is_identity()
    }

    /// Returns true if this is a sparse mapping.
    /// A sparse mapping means not all target values will be overridden by source values.
    pub fn is_sparse(&self) -> bool {
        self.flags.is_sparse()
    }

    /// Returns true if this is a null mapping.
    /// No source elements of a null map are mapped to the target.
    pub fn is_null(&self) -> bool {
        self.flags.is_null()
    }

    /// Get the size of the output array that this mapper expects.
    pub fn size(&self) -> usize {
        self.target_size
    }

    /// Remap data from source to target array.
    ///
    /// The `source` array provides a run of `element_size` for each path in the
    /// source order. These elements are remapped and copied to the `target` array.
    ///
    /// Prior to remapping, the `target` array is resized to target_size * element_size.
    /// Only NEW elements (beyond current size) are initialized to `default_value`.
    /// Existing elements are preserved (matching C++ VtArray::resize behavior).
    /// This allows callers to pre-fill the target with fallback values (e.g. rest poses)
    /// before calling remap, which then overlays only the mapped values.
    pub fn remap<T: Clone + Default>(
        &self,
        source: &[T],
        target: &mut Vec<T>,
        element_size: usize,
        default_value: Option<&T>,
    ) -> bool {
        if element_size == 0 {
            eprintln!("Invalid element_size [0]: size must be greater than zero.");
            return false;
        }

        let target_array_size = self.target_size * element_size;

        if self.is_identity() && source.len() == target_array_size {
            // Can make copy of the array
            *target = source.to_vec();
            return true;
        }

        // Resize target array to expected size - only extend, don't shrink or clear.
        // This matches C++ _ResizeContainer which only sets new elements to default.
        let default = default_value.cloned().unwrap_or_default();
        if target.len() < target_array_size {
            target.resize(target_array_size, default);
        } else if target.len() > target_array_size {
            target.truncate(target_array_size);
        }

        if self.is_null() {
            return true;
        }

        if self.flags.is_ordered() {
            // Ordered mapping with offset
            let copy_count = std::cmp::min(
                source.len(),
                target_array_size.saturating_sub(self.offset * element_size),
            );
            let target_start = self.offset * element_size;
            for i in 0..copy_count {
                target[target_start + i] = source[i].clone();
            }
        } else {
            // Unordered mapping using index map
            let copy_count = std::cmp::min(source.len() / element_size, self.index_map.len());

            for i in 0..copy_count {
                let target_idx = self.index_map[i];
                if target_idx >= 0 && (target_idx as usize) < self.target_size {
                    let src_start = i * element_size;
                    let tgt_start = (target_idx as usize) * element_size;

                    for j in 0..element_size {
                        if src_start + j < source.len() && tgt_start + j < target.len() {
                            target[tgt_start + j] = source[src_start + j].clone();
                        }
                    }
                }
            }
        }
        true
    }

    /// Convenience method for remapping transform arrays.
    /// Sets the matrix identity as the default value.
    pub fn remap_transforms_4d(
        &self,
        source: &[Matrix4d],
        target: &mut Vec<Matrix4d>,
        element_size: usize,
    ) -> bool {
        let identity = Matrix4d::identity();
        self.remap(source, target, element_size, Some(&identity))
    }

    /// Convenience method for remapping transform arrays (f32).
    pub fn remap_transforms_4f(
        &self,
        source: &[Matrix4f],
        target: &mut Vec<Matrix4f>,
        element_size: usize,
    ) -> bool {
        let identity = Matrix4f::identity();
        self.remap(source, target, element_size, Some(&identity))
    }
}

impl PartialEq for AnimMapper {
    fn eq(&self, other: &Self) -> bool {
        self.target_size == other.target_size
            && self.offset == other.offset
            && self.flags == other.flags
            && self.index_map == other.index_map
    }
}

impl Eq for AnimMapper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_mapper() {
        let mapper = AnimMapper::new();
        assert!(mapper.is_null());
        assert!(!mapper.is_identity());
        assert_eq!(mapper.size(), 0);
    }

    #[test]
    fn test_identity_mapper() {
        let mapper = AnimMapper::identity(5);
        assert!(mapper.is_identity());
        assert!(!mapper.is_null());
        assert!(!mapper.is_sparse());
        assert_eq!(mapper.size(), 5);
    }

    #[test]
    fn test_identity_remap() {
        let mapper = AnimMapper::identity(3);
        let source = vec![1.0f32, 2.0, 3.0];
        let mut target = Vec::new();

        assert!(mapper.remap(&source, &mut target, 1, None));
        assert_eq!(target, source);
    }

    #[test]
    fn test_ordered_offset_mapping() {
        let source_order = vec![Token::new("B"), Token::new("C")];
        let target_order = vec![
            Token::new("A"),
            Token::new("B"),
            Token::new("C"),
            Token::new("D"),
        ];

        let mapper = AnimMapper::from_orders(&source_order, &target_order);
        assert!(!mapper.is_identity());
        assert!(!mapper.is_null());
        assert!(mapper.is_sparse()); // Not all target values overridden
        assert_eq!(mapper.size(), 4);

        let source = vec![10.0f32, 20.0];
        let mut target = Vec::new();

        assert!(mapper.remap(&source, &mut target, 1, Some(&0.0)));
        assert_eq!(target, vec![0.0, 10.0, 20.0, 0.0]);
    }

    #[test]
    fn test_unordered_mapping() {
        let source_order = vec![Token::new("C"), Token::new("A")];
        let target_order = vec![Token::new("A"), Token::new("B"), Token::new("C")];

        let mapper = AnimMapper::from_orders(&source_order, &target_order);
        assert!(!mapper.is_identity());
        assert!(!mapper.is_null());
        assert_eq!(mapper.size(), 3);

        let source = vec![30.0f32, 10.0]; // C=30, A=10
        let mut target = Vec::new();

        assert!(mapper.remap(&source, &mut target, 1, Some(&0.0)));
        // A=10, B=0, C=30
        assert_eq!(target, vec![10.0, 0.0, 30.0]);
    }

    #[test]
    fn test_partial_mapping() {
        let source_order = vec![Token::new("X"), Token::new("A")]; // X not in target
        let target_order = vec![Token::new("A"), Token::new("B")];

        let mapper = AnimMapper::from_orders(&source_order, &target_order);
        assert!(!mapper.is_null());
        assert!(mapper.is_sparse());

        let source = vec![100.0f32, 10.0];
        let mut target = Vec::new();

        assert!(mapper.remap(&source, &mut target, 1, Some(&0.0)));
        // X doesn't map, A maps to index 0
        assert_eq!(target, vec![10.0, 0.0]);
    }
}
