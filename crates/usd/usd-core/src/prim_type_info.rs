//! Prim type information.
//!
//! Port of pxr/usd/usd/primTypeInfo.h
//!
//! Holds the full type information for a prim: the type name, applied API
//! schema names, and possibly a mapped schema type name. This is used to
//! cache and provide the "real" schema type for a prim's type name,
//! including fallback types for unrecognized prim type names.

use crate::prim_definition::PrimDefinition;
use std::sync::Arc;
use usd_tf::Token;

/// Uniquely identifies a prim type within the type info cache.
///
/// Consists of the authored type name, optional mapped type name (for
/// fallback handling), and the list of applied API schemas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PrimTypeId {
    /// Authored type name of the prim.
    pub prim_type_name: Token,
    /// Optional fallback type name for unrecognized prim types.
    pub mapped_type_name: Token,
    /// Applied API schemas authored on the prim.
    pub applied_api_schemas: Vec<Token>,
}

impl PrimTypeId {
    /// Creates an empty type ID.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a type ID from just a prim type name.
    pub fn from_type_name(name: Token) -> Self {
        Self {
            prim_type_name: name,
            ..Default::default()
        }
    }

    /// Returns true if this is an empty type ID.
    pub fn is_empty(&self) -> bool {
        self.prim_type_name.is_empty()
            && self.mapped_type_name.is_empty()
            && self.applied_api_schemas.is_empty()
    }
}

/// Full type information for a prim.
///
/// Caches the concrete schema type, schema type name, and prim definition
/// for a given combination of type name and applied API schemas.
pub struct PrimTypeInfo {
    /// The type identifier.
    type_id: PrimTypeId,
    /// The resolved schema type name.
    schema_type_name: Token,
    /// The prim definition (lazily resolved).
    prim_definition: Option<Arc<PrimDefinition>>,
}

impl PrimTypeInfo {
    /// Creates a new PrimTypeInfo from a type ID.
    pub fn new(type_id: PrimTypeId) -> Self {
        let schema_type_name = if type_id.mapped_type_name.is_empty() {
            type_id.prim_type_name.clone()
        } else {
            type_id.mapped_type_name.clone()
        };

        Self {
            type_id,
            schema_type_name,
            prim_definition: None,
        }
    }

    /// Returns the concrete prim type name.
    pub fn get_type_name(&self) -> &Token {
        &self.type_id.prim_type_name
    }

    /// Returns the list of applied API schemas directly authored on the prim.
    ///
    /// This does NOT include API schemas that may be defined in the concrete
    /// prim type's prim definition.
    pub fn get_applied_api_schemas(&self) -> &[Token] {
        &self.type_id.applied_api_schemas
    }

    /// Sets the composed list of applied API schemas.
    ///
    /// Called by `compose_prim_flags` after walking the prim index to
    /// collect and compose apiSchemas list op opinions.
    pub fn set_applied_api_schemas(&mut self, schemas: Vec<Token>) {
        self.type_id.applied_api_schemas = schemas;
    }

    /// Returns the schema type name.
    ///
    /// Typically the same as `get_type_name()` unless a fallback type is in use.
    pub fn get_schema_type_name(&self) -> &Token {
        &self.schema_type_name
    }

    /// Returns the full type ID.
    pub fn get_type_id(&self) -> &PrimTypeId {
        &self.type_id
    }

    /// Returns the prim definition associated with this type info.
    pub fn get_prim_definition(&self) -> Option<&Arc<PrimDefinition>> {
        self.prim_definition.as_ref()
    }

    /// Sets the prim definition.
    pub fn set_prim_definition(&mut self, def: Arc<PrimDefinition>) {
        self.prim_definition = Some(def);
    }

    /// Returns the empty prim type info (singleton pattern).
    pub fn get_empty_prim_type() -> Self {
        Self::new(PrimTypeId::default())
    }
}

impl PartialEq for PrimTypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for PrimTypeInfo {}

impl std::fmt::Debug for PrimTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimTypeInfo")
            .field("type_name", &self.type_id.prim_type_name)
            .field("schema_type_name", &self.schema_type_name)
            .field("applied_api_schemas", &self.type_id.applied_api_schemas)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_type_id() {
        let id = PrimTypeId::new();
        assert!(id.is_empty());
    }

    #[test]
    fn test_type_id_from_name() {
        let id = PrimTypeId::from_type_name(Token::from("Mesh"));
        assert!(!id.is_empty());
        assert_eq!(id.prim_type_name.as_str(), "Mesh");
    }

    #[test]
    fn test_prim_type_info() {
        let info = PrimTypeInfo::new(PrimTypeId::from_type_name(Token::from("Cube")));
        assert_eq!(info.get_type_name().as_str(), "Cube");
        assert_eq!(info.get_schema_type_name().as_str(), "Cube");
        assert!(info.get_applied_api_schemas().is_empty());
    }

    #[test]
    fn test_mapped_type() {
        let id = PrimTypeId {
            prim_type_name: Token::from("CustomType"),
            mapped_type_name: Token::from("Mesh"),
            applied_api_schemas: vec![],
        };
        let info = PrimTypeInfo::new(id);
        assert_eq!(info.get_type_name().as_str(), "CustomType");
        // Schema type name uses the mapped (fallback) type.
        assert_eq!(info.get_schema_type_name().as_str(), "Mesh");
    }
}
