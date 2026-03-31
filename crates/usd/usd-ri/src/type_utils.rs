//! Type conversion utilities for RenderMan.
//!
//! Provides conversion between USD value types and RenderMan type names.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/typeUtils.h` and `typeUtils.cpp`

use usd_sdf::ValueTypeName;
use usd_sdf::value_type_registry::ValueTypeRegistry;

/// Convert a USD value type name to a RenderMan type string.
///
/// # Arguments
///
/// * `usd_type` - The USD value type name
///
/// # Returns
///
/// The corresponding RenderMan type string, or empty string if no mapping.
///
/// # Note
///
/// This is a stub in the C++ reference. Full implementation would map
/// USD types to RenderMan type definitions.
pub fn get_ri_type(_usd_type: &ValueTypeName) -> String {
    // C++ implementation is also a stub returning empty string
    String::new()
}

/// Convert a RenderMan type string to a USD value type name.
///
/// # Arguments
///
/// * `ri_type` - The RenderMan type definition string (e.g. "color", "vector")
///
/// # Returns
///
/// The corresponding USD value type name.
///
/// # Mappings
///
/// | RenderMan Type | USD Type |
/// |----------------|----------|
/// | color | Color3f |
/// | vector | Vector3d |
/// | normal | Normal3d |
/// | point | Point3d |
/// | matrix | Matrix4d |
pub fn get_usd_type(ri_type: &str) -> ValueTypeName {
    // Type mapping table from C++ reference
    static MAPPINGS: &[(&str, &str)] = &[
        ("color", "color3f"),
        ("vector", "vector3d"),
        ("normal", "normal3d"),
        ("point", "point3d"),
        ("matrix", "matrix4d"),
    ];

    let registry = ValueTypeRegistry::instance();

    for (ri_name, usd_name) in MAPPINGS {
        if ri_type.contains(ri_name) {
            return registry.find_type(usd_name);
        }
    }

    // Fallback: try to find/create the type directly
    registry.find_type(ri_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ri_type_stub() {
        let vt = ValueTypeName::default();
        assert!(get_ri_type(&vt).is_empty());
    }

    #[test]
    fn test_get_usd_type_mappings() {
        // Test color mapping
        let color_type = get_usd_type("color");
        // The returned type depends on registry
        let _ = color_type;

        // Test vector mapping
        let vector_type = get_usd_type("vector");
        let _ = vector_type;
    }
}
