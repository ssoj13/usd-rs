//! UsdGeomXform - transform geometry schema.
//!
//! Port of pxr/usd/usdGeom/xform.h/cpp
//!
//! Concrete prim schema for a transform, which implements Xformable.

use super::xformable::Xformable;
use usd_core::{Prim, Stage};
use usd_tf::Token;

// ============================================================================
// Xform
// ============================================================================

/// Transform geometry schema.
///
/// Concrete prim schema for a transform, which implements Xformable.
///
/// Matches C++ `UsdGeomXform`.
#[derive(Debug, Clone)]
pub struct Xform {
    /// Base xformable schema.
    inner: Xformable,
}

impl Xform {
    /// Creates a Xform schema from a prim.
    ///
    /// Matches C++ `UsdGeomXform(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Xformable::new(prim),
        }
    }

    /// Creates a Xform schema from a Xformable schema.
    ///
    /// Matches C++ `UsdGeomXform(const UsdSchemaBase& schemaObj)`.
    pub fn from_xformable(xformable: Xformable) -> Self {
        Self { inner: xformable }
    }

    /// Creates an invalid Xform schema.
    pub fn invalid() -> Self {
        Self {
            inner: Xformable::invalid(),
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

    /// Returns the xformable base.
    pub fn xformable(&self) -> &Xformable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Xform")
    }

    /// Return a Xform holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomXform::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomXform::Define(const UsdStagePtr &stage, const SdfPath &path)`.
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
        Xformable::get_schema_attribute_names(include_inherited)
    }
}

impl PartialEq for Xform {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Xform {}
