//! Prototype scene index utilities.
//!
//! Port of pxr/usdImaging/usdImaging/prototypeSceneIndexUtils.h
//!
//! Utility functions for working with prototype scene indices, particularly
//! for native instancing support.

use usd_tf::Token;

// ============================================================================
// Renderable Prim Type Checking
// ============================================================================

/// Checks if a given prim type is renderable.
///
/// Determines whether a Hydra prim type represents geometry or other
/// renderable content (as opposed to non-renderable prims like instancers,
/// materials, cameras, etc.).
///
/// This is used by prototype scene indices to determine which prims should
/// be included in prototype subtrees for native instancing.
///
/// # Arguments
///
/// * `prim_type` - Hydra prim type token to check
///
/// # Returns
///
/// `true` if the prim type is renderable geometry, `false` otherwise.
///
/// # Examples
///
/// ```
/// use usd_tf::Token;
/// use usd_imaging::prototype_scene_index_utils::is_renderable_prim_type;
///
/// assert!(is_renderable_prim_type(&Token::new("mesh")));
/// assert!(is_renderable_prim_type(&Token::new("sphere")));
/// assert!(!is_renderable_prim_type(&Token::new("camera")));
/// assert!(!is_renderable_prim_type(&Token::new("material")));
/// ```
///
/// # Renderable Types
///
/// The following prim types are considered renderable:
///
/// ## Polygon Geometry
/// - `mesh` - Polygon mesh
/// - `tetMesh` - Tetrahedral mesh
/// - `geomSubset` - Geometry subset
///
/// ## Implicit Surfaces
/// - `sphere` - Sphere
/// - `cube` - Cube
/// - `cone` - Cone
/// - `cylinder` - Cylinder
/// - `capsule` - Capsule
/// - `plane` - Infinite plane
///
/// ## Curves
/// - `basisCurves` - Basis curves (cubic/linear)
/// - `nurbsCurves` - NURBS curves
/// - `nurbsPatch` - NURBS patch
///
/// ## Point Clouds
/// - `points` - Point cloud
///
/// ## Volumes
/// - `volume` - Volume primitive
///
/// # Non-Renderable Types
///
/// The following are NOT considered renderable:
/// - `camera` - Camera (state prim)
/// - `material` - Material (state prim)
/// - `light` - Lights (state prims)
/// - `instancer` - Point instancer (structural)
/// - `coordSys` - Coordinate system (state prim)
/// - `model` - Model reference (structural)
pub fn is_renderable_prim_type(prim_type: &Token) -> bool {
    let type_str = prim_type.as_str();

    match type_str {
        // Polygon meshes
        "mesh" | "tetMesh" | "geomSubset" => true,

        // Implicit surfaces
        "sphere" | "cube" | "cone" | "cylinder" | "capsule" | "plane" => true,

        // Curves
        "basisCurves" | "nurbsCurves" | "nurbsPatch" => true,

        // Points
        "points" => true,

        // Volumes
        "volume" => true,

        // Everything else is non-renderable
        // This includes:
        // - State prims: camera, material, light, coordSys
        // - Structural: instancer, model
        // - Unknown types
        _ => false,
    }
}

/// Checks if a prim type represents an implicit surface.
///
/// Implicit surfaces are primitives defined by mathematical equations
/// rather than explicit geometry (meshes/curves).
///
/// # Arguments
///
/// * `prim_type` - Hydra prim type token to check
///
/// # Returns
///
/// `true` if the prim is an implicit surface type.
///
/// # Examples
///
/// ```
/// use usd_tf::Token;
/// use usd_imaging::prototype_scene_index_utils::is_implicit_surface;
///
/// assert!(is_implicit_surface(&Token::new("sphere")));
/// assert!(is_implicit_surface(&Token::new("cube")));
/// assert!(!is_implicit_surface(&Token::new("mesh")));
/// ```
pub fn is_implicit_surface(prim_type: &Token) -> bool {
    matches!(
        prim_type.as_str(),
        "sphere" | "cube" | "cone" | "cylinder" | "capsule" | "plane"
    )
}

/// Checks if a prim type represents curves.
///
/// # Arguments
///
/// * `prim_type` - Hydra prim type token to check
///
/// # Returns
///
/// `true` if the prim is a curve type.
pub fn is_curve_type(prim_type: &Token) -> bool {
    matches!(
        prim_type.as_str(),
        "basisCurves" | "nurbsCurves" | "nurbsPatch"
    )
}

/// Checks if a prim type is a light.
///
/// # Arguments
///
/// * `prim_type` - Hydra prim type token to check
///
/// # Returns
///
/// `true` if the prim is a light type.
pub fn is_light_type(prim_type: &Token) -> bool {
    let type_str = prim_type.as_str();
    type_str.ends_with("Light") || type_str == "light"
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderable_mesh_types() {
        assert!(is_renderable_prim_type(&Token::new("mesh")));
        assert!(is_renderable_prim_type(&Token::new("tetMesh")));
        assert!(is_renderable_prim_type(&Token::new("geomSubset")));
    }

    #[test]
    fn test_renderable_implicit_surfaces() {
        assert!(is_renderable_prim_type(&Token::new("sphere")));
        assert!(is_renderable_prim_type(&Token::new("cube")));
        assert!(is_renderable_prim_type(&Token::new("cone")));
        assert!(is_renderable_prim_type(&Token::new("cylinder")));
        assert!(is_renderable_prim_type(&Token::new("capsule")));
        assert!(is_renderable_prim_type(&Token::new("plane")));
    }

    #[test]
    fn test_renderable_curves() {
        assert!(is_renderable_prim_type(&Token::new("basisCurves")));
        assert!(is_renderable_prim_type(&Token::new("nurbsCurves")));
        assert!(is_renderable_prim_type(&Token::new("nurbsPatch")));
    }

    #[test]
    fn test_renderable_points() {
        assert!(is_renderable_prim_type(&Token::new("points")));
    }

    #[test]
    fn test_renderable_volume() {
        assert!(is_renderable_prim_type(&Token::new("volume")));
    }

    #[test]
    fn test_non_renderable_state_prims() {
        assert!(!is_renderable_prim_type(&Token::new("camera")));
        assert!(!is_renderable_prim_type(&Token::new("material")));
        assert!(!is_renderable_prim_type(&Token::new("coordSys")));
    }

    #[test]
    fn test_non_renderable_lights() {
        assert!(!is_renderable_prim_type(&Token::new("light")));
        assert!(!is_renderable_prim_type(&Token::new("sphereLight")));
        assert!(!is_renderable_prim_type(&Token::new("distantLight")));
        assert!(!is_renderable_prim_type(&Token::new("domeLight")));
        assert!(!is_renderable_prim_type(&Token::new("rectLight")));
        assert!(!is_renderable_prim_type(&Token::new("diskLight")));
        assert!(!is_renderable_prim_type(&Token::new("cylinderLight")));
        assert!(!is_renderable_prim_type(&Token::new("meshLight")));
    }

    #[test]
    fn test_non_renderable_structural() {
        assert!(!is_renderable_prim_type(&Token::new("instancer")));
        assert!(!is_renderable_prim_type(&Token::new("model")));
    }

    #[test]
    fn test_non_renderable_unknown() {
        assert!(!is_renderable_prim_type(&Token::new("unknown")));
        assert!(!is_renderable_prim_type(&Token::new("")));
        assert!(!is_renderable_prim_type(&Token::new("customType")));
    }

    #[test]
    fn test_implicit_surface_detection() {
        // Implicit surfaces
        assert!(is_implicit_surface(&Token::new("sphere")));
        assert!(is_implicit_surface(&Token::new("cube")));
        assert!(is_implicit_surface(&Token::new("cone")));
        assert!(is_implicit_surface(&Token::new("cylinder")));
        assert!(is_implicit_surface(&Token::new("capsule")));
        assert!(is_implicit_surface(&Token::new("plane")));

        // Not implicit surfaces
        assert!(!is_implicit_surface(&Token::new("mesh")));
        assert!(!is_implicit_surface(&Token::new("basisCurves")));
        assert!(!is_implicit_surface(&Token::new("points")));
    }

    #[test]
    fn test_curve_detection() {
        assert!(is_curve_type(&Token::new("basisCurves")));
        assert!(is_curve_type(&Token::new("nurbsCurves")));
        assert!(is_curve_type(&Token::new("nurbsPatch")));

        assert!(!is_curve_type(&Token::new("mesh")));
        assert!(!is_curve_type(&Token::new("sphere")));
    }

    #[test]
    fn test_light_detection() {
        assert!(is_light_type(&Token::new("light")));
        assert!(is_light_type(&Token::new("sphereLight")));
        assert!(is_light_type(&Token::new("distantLight")));
        assert!(is_light_type(&Token::new("domeLight")));
        assert!(is_light_type(&Token::new("rectLight")));

        assert!(!is_light_type(&Token::new("mesh")));
        assert!(!is_light_type(&Token::new("camera")));
    }

    #[test]
    fn test_comprehensive_categorization() {
        // Every renderable type should have a category
        let renderable_types = [
            // Meshes
            "mesh",
            "tetMesh",
            "geomSubset",
            // Implicit surfaces
            "sphere",
            "cube",
            "cone",
            "cylinder",
            "capsule",
            "plane",
            // Curves
            "basisCurves",
            "nurbsCurves",
            "nurbsPatch",
            // Points
            "points",
            // Volume
            "volume",
        ];

        for type_str in renderable_types.iter() {
            let token = Token::new(type_str);
            assert!(
                is_renderable_prim_type(&token),
                "{} should be renderable",
                type_str
            );
        }

        // Non-renderable should not be in any renderable category
        let non_renderable = [
            "camera",
            "material",
            "light",
            "instancer",
            "model",
            "coordSys",
        ];

        for type_str in non_renderable.iter() {
            let token = Token::new(type_str);
            assert!(
                !is_renderable_prim_type(&token),
                "{} should not be renderable",
                type_str
            );
        }
    }
}
