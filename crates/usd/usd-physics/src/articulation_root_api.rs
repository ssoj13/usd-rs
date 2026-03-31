//! Physics Articulation Root API schema.
//!
//! Marks a subtree for inclusion in reduced coordinate articulations.
//! Used for robotic systems and other articulated mechanisms.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/articulationRootAPI.h` and `articulationRootAPI.cpp`
//!
//! # Articulation Types
//!
//! - **Floating articulation**: Apply to the root body
//! - **Fixed articulation**: Apply to parent of root joint, or on the joint itself
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::ArticulationRootAPI;
//!
//! // Mark a robot arm as an articulation root
//! let articulation = ArticulationRootAPI::apply(&robot_base_prim)?;
//! ```

use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;

/// Physics articulation root API schema.
///
/// PhysicsArticulationRootAPI can be applied to a scene graph node,
/// and marks the subtree rooted here for inclusion in one or more reduced
/// coordinate articulations.
///
/// For floating articulations, this should be on the root body.
/// For fixed articulations (e.g. a robot arm bolted to the floor),
/// this API can be on a direct or indirect parent of the root joint
/// which is connected to the world, or on the joint itself.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsArticulationRootAPI` class.
#[derive(Debug, Clone)]
pub struct ArticulationRootAPI {
    prim: Prim,
}

impl ArticulationRootAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsArticulationRootAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct an ArticulationRootAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct an ArticulationRootAPI from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return an ArticulationRootAPI holding the prim at `path` on `stage`.
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
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// ArticulationRootAPI has no custom attributes beyond the base API schema.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }
}

impl ArticulationRootAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for ArticulationRootAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<ArticulationRootAPI> for Prim {
    fn from(api: ArticulationRootAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for ArticulationRootAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(ArticulationRootAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(
            ArticulationRootAPI::SCHEMA_TYPE_NAME,
            "PhysicsArticulationRootAPI"
        );
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = ArticulationRootAPI::get_schema_attribute_names(false);
        assert!(names.is_empty()); // No custom attributes
    }
}
