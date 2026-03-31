//! Schema Traits for USD Schema System
//!
//! This module provides traits used by the `#[derive(UsdSchema)]` macro
//! from the `usd-derive-macros` crate.
//!
//! # Traits
//!
//! - [`UsdTyped`] - Implemented by typed (IsA) schemas
//! - [`UsdSchemaBase`] - Base trait for all schema types
//! - [`UsdAPISchema`] - Implemented by API schemas
//!
//! # Usage with Macros
//!
//! ```ignore
//! use usd_derive_macros::UsdSchema;
//!
//! #[derive(UsdSchema)]
//! #[usd_prim_type("MyMesh")]
//! pub struct MyMesh {
//!     prim: Prim,
//!     // ...
//! }
//!
//! // The macro generates:
//! impl UsdTyped for MyMesh { ... }
//! impl UsdSchemaBase for MyMesh { ... }
//! ```

use super::prim::Prim;

// ============================================================================
// UsdTyped Trait
// ============================================================================

/// Trait for typed (IsA) schemas.
///
/// Typed schemas define a prim's type. A prim can only have one type,
/// though it may also have API schemas applied.
///
/// This trait is automatically implemented by `#[derive(UsdSchema)]`.
pub trait UsdTyped: UsdSchemaBase {
    /// Returns the USD prim type name (e.g., "Mesh", "Xform", "Scope").
    fn get_schema_type_name() -> &'static str;

    /// Returns true if this is a typed schema.
    ///
    /// Always returns true for types implementing `UsdTyped`.
    fn is_typed() -> bool {
        true
    }
}

// ============================================================================
// UsdSchemaBase Trait
// ============================================================================

/// Base trait for all USD schema types.
///
/// Provides common functionality for accessing the underlying prim
/// and schema metadata.
///
/// This trait is automatically implemented by `#[derive(UsdSchema)]`.
pub trait UsdSchemaBase {
    /// Returns the schema kind ("concreteTyped", "abstractTyped", "api", etc.)
    fn get_schema_kind() -> &'static str;

    /// Returns the base schema type name ("UsdTyped", "UsdGeomGprim", etc.)
    fn get_schema_base_type() -> &'static str;

    /// Returns the schema documentation string.
    fn get_documentation() -> &'static str {
        ""
    }

    /// Returns a reference to the underlying prim.
    fn get_prim(&self) -> &Prim;
}

// ============================================================================
// UsdAPISchema Trait
// ============================================================================

/// Trait for API schemas.
///
/// API schemas add behaviors and data to prims without changing their type.
/// They are applied to prims using the `apiSchemas` metadata.
///
/// # Types
///
/// - **Single-apply**: Can be applied once per prim (e.g., `ModelAPI`)
/// - **Multiple-apply**: Can be applied multiple times with instance names
///   (e.g., `CollectionAPI:myCollection`)
pub trait UsdAPISchema: UsdSchemaBase {
    /// Returns the API schema name (e.g., "ModelAPI", "CollectionAPI").
    fn get_schema_name() -> &'static str;

    /// Returns true if this is an applied API schema.
    fn is_applied() -> bool {
        true
    }

    /// Returns true if this is a multiple-apply API schema.
    fn is_multiple_apply() -> bool {
        false
    }

    /// Returns the instance name for multiple-apply schemas.
    fn get_instance_name(&self) -> Option<&str> {
        None
    }
}

// ============================================================================
// SchemaAttrInfo - Attribute metadata for code generation
// ============================================================================

/// Metadata about a schema attribute.
///
/// Used by generated code to register attribute information.
#[derive(Debug, Clone)]
pub struct SchemaAttrInfo {
    /// The attribute name (e.g., "points", "normals").
    pub name: &'static str,

    /// The USD type name (e.g., "point3f[]", "normal3f[]").
    pub usd_type: &'static str,

    /// Default value as a string (e.g., "0.0", "(1, 0, 0)").
    pub default_value: &'static str,

    /// Interpolation mode ("vertex", "faceVarying", "uniform", "constant").
    pub interpolation: &'static str,

    /// Documentation string.
    pub doc: &'static str,
}

impl SchemaAttrInfo {
    /// Creates a new attribute info.
    pub const fn new(
        name: &'static str,
        usd_type: &'static str,
        default_value: &'static str,
        interpolation: &'static str,
        doc: &'static str,
    ) -> Self {
        Self {
            name,
            usd_type,
            default_value,
            interpolation,
            doc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_attr_info() {
        let info = SchemaAttrInfo::new("points", "point3f[]", "", "vertex", "The mesh vertices");

        assert_eq!(info.name, "points");
        assert_eq!(info.usd_type, "point3f[]");
        assert_eq!(info.interpolation, "vertex");
    }
}
