//! Scene index prim representation.

use usd_tf::Token as TfToken;

// Use the data source types from the data_source module
pub use crate::data_source::{HdContainerDataSource, HdContainerDataSourceHandle};

/// A prim in the Hydra scene index.
///
/// Represents a single prim with its type and associated data source.
/// Per C++ `HdSceneIndexPrim::IsDefined()`, a prim is considered to exist
/// if and only if its data source is non-null (type alone is not enough).
#[derive(Clone)]
pub struct HdSceneIndexPrim {
    /// Type of the prim (e.g., "Mesh", "Camera", "Light")
    pub prim_type: TfToken,
    /// Container data source with prim's properties
    pub data_source: Option<HdContainerDataSourceHandle>,
}

impl HdSceneIndexPrim {
    /// Create a new scene index prim.
    pub fn new(prim_type: TfToken, data_source: Option<HdContainerDataSourceHandle>) -> Self {
        Self {
            prim_type,
            data_source,
        }
    }

    /// Create an empty/undefined prim.
    pub fn empty() -> Self {
        Self {
            prim_type: TfToken::empty(),
            data_source: None,
        }
    }

    /// Returns true if this prim exists in the scene index.
    ///
    /// Matches C++ `HdSceneIndexPrim::IsDefined()` which checks ONLY
    /// the data source pointer: `return bool(dataSource);`
    /// A prim with a type but no data source is NOT considered defined.
    pub fn is_defined(&self) -> bool {
        self.data_source.is_some()
    }
}

impl Default for HdSceneIndexPrim {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for HdSceneIndexPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdSceneIndexPrim")
            .field("prim_type", &self.prim_type)
            .field("has_data_source", &self.data_source.is_some())
            .finish()
    }
}

// Allow bool conversion for checking if prim exists
impl From<HdSceneIndexPrim> for bool {
    fn from(prim: HdSceneIndexPrim) -> bool {
        prim.is_defined()
    }
}

impl From<&HdSceneIndexPrim> for bool {
    fn from(prim: &HdSceneIndexPrim) -> bool {
        prim.is_defined()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_prim() {
        let prim = HdSceneIndexPrim::empty();
        assert!(!prim.is_defined());
        assert!(prim.prim_type.is_empty());
        assert!(prim.data_source.is_none());
    }

    #[test]
    fn test_default_prim() {
        let prim = HdSceneIndexPrim::default();
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_prim_with_type_but_no_data() {
        let prim = HdSceneIndexPrim::new(TfToken::new("Mesh"), None);
        // C++ IsDefined only checks dataSource, so type-only prim is NOT defined
        assert!(!prim.is_defined());
        assert_eq!(prim.prim_type.as_str(), "Mesh");
    }

    #[test]
    fn test_bool_conversion() {
        let empty_prim = HdSceneIndexPrim::empty();
        assert!(!bool::from(&empty_prim));

        // C++ IsDefined only checks dataSource; type-only is NOT defined
        let prim_with_type = HdSceneIndexPrim::new(TfToken::new("Mesh"), None);
        assert!(!bool::from(prim_with_type));
    }
}
