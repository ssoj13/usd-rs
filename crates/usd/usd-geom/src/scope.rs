//! UsdGeomScope - scope geometry schema.
//!
//! Port of pxr/usd/usdGeom/scope.h/cpp
//!
//! Scope is the simplest grouping primitive, and does not carry the baggage of transformability.

use super::imageable::Imageable;
use usd_core::{Prim, Stage};
use usd_tf::Token;

// ============================================================================
// Scope
// ============================================================================

/// Scope geometry schema.
///
/// Scope is the simplest grouping primitive, and does not carry the baggage of transformability.
///
/// Matches C++ `UsdGeomScope`.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Base imageable schema.
    inner: Imageable,
}

impl Scope {
    /// Creates a Scope schema from a prim.
    ///
    /// Matches C++ `UsdGeomScope(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Imageable::new(prim),
        }
    }

    /// Creates a Scope schema from an Imageable schema.
    ///
    /// Matches C++ `UsdGeomScope(const UsdSchemaBase& schemaObj)`.
    pub fn from_imageable(imageable: Imageable) -> Self {
        Self { inner: imageable }
    }

    /// Creates an invalid Scope schema.
    pub fn invalid() -> Self {
        Self {
            inner: Imageable::invalid(),
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

    /// Returns the imageable base.
    pub fn imageable(&self) -> &Imageable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Scope")
    }

    /// Return a Scope holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomScope::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomScope::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
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
        Imageable::get_schema_attribute_names(include_inherited)
    }
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Scope {}
