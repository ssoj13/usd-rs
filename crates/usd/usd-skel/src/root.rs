//! UsdSkelRoot - boundable prim type used to identify a scope beneath which
//! skeletally-posed primitives are defined.
//!
//! Port of pxr/usd/usdSkel/root.h/cpp
//!
//! A SkelRoot must be defined at or above a skinned primitive for any skinning
//! behaviors in UsdSkel.

use super::tokens::tokens;
use usd_core::{Prim, Stage};
use usd_geom::boundable::Boundable;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// SkelRoot
// ============================================================================

/// Boundable prim type used to identify a scope beneath which
/// skeletally-posed primitives are defined.
///
/// A SkelRoot must be defined at or above a skinned primitive for any skinning
/// behaviors in UsdSkel.
///
/// Matches C++ `UsdSkelRoot`.
#[derive(Debug, Clone)]
pub struct SkelRoot {
    /// Base boundable schema.
    inner: Boundable,
}

impl SkelRoot {
    /// Creates a SkelRoot schema from a prim.
    ///
    /// Matches C++ `UsdSkelRoot(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Boundable::new(prim),
        }
    }

    /// Creates a SkelRoot schema from a Boundable schema.
    ///
    /// Matches C++ `UsdSkelRoot(const UsdSchemaBase& schemaObj)`.
    pub fn from_boundable(boundable: Boundable) -> Self {
        Self { inner: boundable }
    }

    /// Creates an invalid SkelRoot schema.
    pub fn invalid() -> Self {
        Self {
            inner: Boundable::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the boundable base.
    pub fn boundable(&self) -> &Boundable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        tokens().skel_root.clone()
    }

    /// Return a SkelRoot holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdSkelRoot::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdSkelRoot::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        Boundable::get_schema_attribute_names(include_inherited)
    }

    /// Returns the skel root at or above prim, or an invalid schema object
    /// if no ancestor prim is defined as a skel root.
    ///
    /// Matches C++ `UsdSkelRoot::Find(const UsdPrim& prim)`.
    pub fn find(prim: &Prim) -> Self {
        let mut p = prim.clone();
        while p.is_valid() {
            let type_name = p.type_name();
            if type_name == tokens().skel_root {
                return Self::new(p);
            }
            p = p.parent();
        }
        Self::invalid()
    }
}

impl PartialEq for SkelRoot {
    fn eq(&self, other: &Self) -> bool {
        self.prim() == other.prim()
    }
}

impl Eq for SkelRoot {}

impl std::hash::Hash for SkelRoot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim().hash(state);
    }
}
