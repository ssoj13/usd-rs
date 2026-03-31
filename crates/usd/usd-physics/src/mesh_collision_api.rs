//! Physics Mesh Collision API schema.
//!
//! Provides attributes to control how a Mesh is made into a collider.
//! Can be applied only to a USDGeomMesh in addition to its PhysicsCollisionAPI.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/meshCollisionAPI.h` and `meshCollisionAPI.cpp`
//!
//! # Approximation Types
//!
//! - `none`: Use mesh geometry directly
//! - `convexDecomposition`: Decompose into convex meshes
//! - `convexHull`: Generate a convex hull
//! - `boundingSphere`: Use a bounding sphere
//! - `boundingCube`: Use a bounding box
//! - `meshSimplification`: Simplify the mesh
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::{CollisionAPI, MeshCollisionAPI};
//!
//! // Apply collision with mesh approximation
//! CollisionAPI::apply(&mesh_prim)?;
//! let mesh_collision = MeshCollisionAPI::apply(&mesh_prim)?;
//! mesh_collision.create_approximation_attr(Some("convexHull".into()))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics mesh collision API schema.
///
/// Attributes to control how a Mesh is made into a collider.
/// Can be applied to only a USDGeomMesh in addition to its PhysicsCollisionAPI.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsMeshCollisionAPI` class.
#[derive(Debug, Clone)]
pub struct MeshCollisionAPI {
    prim: Prim,
}

impl MeshCollisionAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsMeshCollisionAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a MeshCollisionAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a MeshCollisionAPI from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a MeshCollisionAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&Token::new(Self::SCHEMA_TYPE_NAME))
    }

    /// Applies this single-apply API schema to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    // =========================================================================
    // Approximation Attribute
    // =========================================================================

    /// Determines the mesh's collision approximation.
    ///
    /// Allowed values:
    /// - `none`: Mesh geometry used directly without approximation
    /// - `convexDecomposition`: Convex mesh decomposition
    /// - `convexHull`: Convex hull of the mesh
    /// - `boundingSphere`: Bounding sphere collider
    /// - `boundingCube`: Optimally fitting box collider
    /// - `meshSimplification`: Simplified triangle mesh
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token physics:approximation = "none"` |
    /// | C++ Type | TfToken |
    /// | Variability | Uniform |
    pub fn get_approximation_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_approximation.as_str())
    }

    /// Creates the approximation attribute.
    pub fn create_approximation_attr(&self, default_value: Option<Token>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("token"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_approximation.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![USD_PHYSICS_TOKENS.physics_approximation.clone()]
    }
}

impl MeshCollisionAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for MeshCollisionAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<MeshCollisionAPI> for Prim {
    fn from(api: MeshCollisionAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for MeshCollisionAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(MeshCollisionAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(
            MeshCollisionAPI::SCHEMA_TYPE_NAME,
            "PhysicsMeshCollisionAPI"
        );
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = MeshCollisionAPI::get_schema_attribute_names(false);
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:approximation")
        );
    }
}
