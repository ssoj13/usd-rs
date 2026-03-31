//! UsdSkelBinding - helper describing skeleton binding to skinnable objects.
//!
//! Port of pxr/usd/usdSkel/binding.h

use super::skeleton::Skeleton;
use super::skinning_query::SkinningQuery;

/// Helper object that describes the binding of a skeleton to a set of
/// skinnable objects.
///
/// The set of skinnable objects is given as SkinningQuery objects, which
/// can be used both to identify the skinned prim as well as compute
/// skinning properties of the prim.
///
/// Matches C++ `UsdSkelBinding`.
#[derive(Clone, Default)]
pub struct Binding {
    /// The bound skeleton.
    skeleton: Skeleton,
    /// The set of skinning targets.
    skinning_queries: Vec<SkinningQuery>,
}

impl Binding {
    /// Create an empty binding.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a binding from a skeleton and skinning queries.
    pub fn from_skeleton(skeleton: Skeleton, skinning_queries: Vec<SkinningQuery>) -> Self {
        Self {
            skeleton,
            skinning_queries,
        }
    }

    /// Returns the bound skeleton.
    pub fn get_skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// Returns the set of skinning targets.
    pub fn get_skinning_targets(&self) -> &[SkinningQuery] {
        &self.skinning_queries
    }

    /// Returns true if this binding has any skinning targets.
    pub fn has_skinning_targets(&self) -> bool {
        !self.skinning_queries.is_empty()
    }

    /// Returns the number of skinning targets.
    pub fn num_skinning_targets(&self) -> usize {
        self.skinning_queries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_binding() {
        let binding = Binding::new();
        assert!(!binding.has_skinning_targets());
        assert_eq!(binding.num_skinning_targets(), 0);
    }
}
