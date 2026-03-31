//! Base class for all USD schema classes.

use super::common::SchemaKind;
use super::object::Stage;
use super::prim::Prim;
use super::schema_registry::SchemaRegistry;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// SchemaBase
// ============================================================================

/// Base class for all schema classes.
///
/// USD schemas provide typed access to prims with specific structure.
/// SchemaBase is the abstract base that all schema types derive from.
///
/// There are two main categories of schemas:
/// - **IsA schemas** (UsdTyped): Define a prim's type (e.g., Mesh, Xform)
/// - **API schemas** (UsdAPISchemaBase): Add additional functionality to prims
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::UsdStage;
/// use usd_geom::Mesh;
///
/// let stage = UsdStage::open("scene.usda")?;
/// let prim = stage.get_prim_at_path("/World/Cube")?;
///
/// // Check if prim is a Mesh
/// if let Some(mesh) = Mesh::get(&prim) {
///     let points = mesh.get_points_attr();
///     // ...
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SchemaBase {
    /// The prim this schema wraps.
    prim: Prim,
}

impl SchemaBase {
    /// Creates a schema from a prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Creates an invalid schema.
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Returns true if this schema is valid (wraps a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.prim.path()
    }

    /// Returns the stage.
    pub fn stage(&self) -> Option<std::sync::Arc<Stage>> {
        self.prim.stage()
    }

    /// Returns the schema type name (static - empty for base class).
    ///
    /// Derived schema classes should override this. In Rust, each concrete
    /// schema type provides its own static implementation.
    pub fn schema_type_name() -> Token {
        Token::new("")
    }

    /// Returns the schema type name for this instance by querying the prim.
    ///
    /// Since Rust doesn't have virtual static dispatch, this instance method
    /// returns the prim's type name as the effective schema type.
    pub fn schema_type(&self) -> Token {
        self.prim.type_name()
    }

    /// Returns true if this is an API schema.
    pub fn is_api_schema() -> bool {
        false
    }

    /// Returns true if this is an applied API schema.
    pub fn is_applied_api_schema() -> bool {
        false
    }

    /// Returns true if this is a multiple-apply API schema.
    pub fn is_multiple_apply_api_schema() -> bool {
        false
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // SchemaBase doesn't add any attributes itself
        Vec::new()
    }

    /// Returns the schema kind of this schema.
    ///
    /// Internal helper that queries the schema registry based on the prim's type.
    fn _get_schema_kind(&self) -> SchemaKind {
        if !self.prim.is_valid() {
            return SchemaKind::Invalid;
        }
        let type_name = self.prim.type_name();
        SchemaRegistry::get_schema_kind_from_name(&type_name)
    }

    /// Returns true if this is a concrete typed schema (instantiable prim type).
    ///
    /// Matches C++ `UsdSchemaBase::IsConcrete()`.
    pub fn is_concrete(&self) -> bool {
        self._get_schema_kind() == SchemaKind::ConcreteTyped
    }

    /// Returns true if this schema inherits from UsdTyped.
    ///
    /// A typed schema is either ConcreteTyped or AbstractTyped.
    /// API schemas return false.
    ///
    /// Matches C++ `UsdSchemaBase::IsTyped()`.
    pub fn is_typed(&self) -> bool {
        let kind = self._get_schema_kind();
        kind == SchemaKind::ConcreteTyped || kind == SchemaKind::AbstractTyped
    }
}

impl PartialEq for SchemaBase {
    fn eq(&self, other: &Self) -> bool {
        self.prim == other.prim
    }
}

impl Eq for SchemaBase {}

impl std::hash::Hash for SchemaBase {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim.hash(state);
    }
}

// ============================================================================
// APISchemaBase
// ============================================================================

/// Base class for API schemas.
///
/// API schemas add behaviors and data to prims without changing their type.
/// They are applied to prims using the `apiSchemas` metadata.
#[derive(Debug, Clone)]
pub struct APISchemaBase {
    /// The prim this schema wraps.
    prim: Prim,
    /// Instance name for multiple-apply schemas.
    instance_name: Option<Token>,
}

impl APISchemaBase {
    /// Creates an API schema from a prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            prim,
            instance_name: None,
        }
    }

    /// Creates a multiple-apply API schema with instance name.
    pub fn new_with_instance(prim: Prim, instance_name: Token) -> Self {
        Self {
            prim,
            instance_name: Some(instance_name),
        }
    }

    /// Creates an invalid API schema.
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
            instance_name: None,
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns the wrapped prim (alias for prim()).
    ///
    /// Matches C++ `GetPrim()`.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns the path to this schema's prim.
    ///
    /// Matches C++ `GetPath()`.
    pub fn path(&self) -> &Path {
        self.prim.path()
    }

    /// Returns the instance name (for multiple-apply schemas).
    pub fn instance_name(&self) -> Option<&Token> {
        self.instance_name.as_ref()
    }

    /// Returns true if this is a multiple-apply schema instance.
    pub fn is_multiple_apply_instance(&self) -> bool {
        self.instance_name.is_some()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // APISchemaBase doesn't add any attributes itself
        Vec::new()
    }
}

impl PartialEq for APISchemaBase {
    fn eq(&self, other: &Self) -> bool {
        self.prim == other.prim && self.instance_name == other.instance_name
    }
}

impl Eq for APISchemaBase {}

impl std::hash::Hash for APISchemaBase {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim.hash(state);
        self.instance_name.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_schema() {
        let schema = SchemaBase::invalid();
        assert!(!schema.is_valid());
    }

    #[test]
    fn test_invalid_api_schema() {
        let schema = APISchemaBase::invalid();
        assert!(!schema.is_valid());
    }

    // M10: schema_type() returns prim type_name
    #[test]
    fn test_schema_type_returns_prim_type() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let schema = SchemaBase::new(prim);
        assert_eq!(schema.schema_type().get_text(), "Xform");
    }

    // Static schema_type_name() returns empty for base class
    #[test]
    fn test_static_schema_type_name_empty() {
        assert!(SchemaBase::schema_type_name().is_empty());
    }

    // New test: is_concrete returns true for ConcreteTyped schema
    #[test]
    fn test_is_concrete() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::{SchemaInfo, register_schema};
        use super::super::stage::Stage;
        use usd_tf::Token;

        // Register a concrete typed schema for testing
        let schema_info = SchemaInfo {
            identifier: Token::new("Xform"),
            type_name: "UsdGeomXform".to_string(),
            family: Token::new("Xform"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: Vec::new(),
            auto_apply_to: Vec::new(),
            can_only_apply_to: Vec::new(),
            allowed_instance_names: None,
            prim_definition: None,
        };
        register_schema(schema_info);

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();
        let schema = SchemaBase::new(prim);

        assert!(schema.is_concrete());
    }

    // New test: is_typed returns true for ConcreteTyped and AbstractTyped
    #[test]
    fn test_is_typed() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::{SchemaInfo, register_schema};
        use super::super::stage::Stage;
        use usd_tf::Token;

        // Register an abstract typed schema
        let schema_info = SchemaInfo {
            identifier: Token::new("Gprim"),
            type_name: "UsdGeomGprim".to_string(),
            family: Token::new("Gprim"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: Vec::new(),
            auto_apply_to: Vec::new(),
            can_only_apply_to: Vec::new(),
            allowed_instance_names: None,
            prim_definition: None,
        };
        register_schema(schema_info);

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Abstract", "Gprim").unwrap();
        let schema = SchemaBase::new(prim);

        assert!(schema.is_typed());
        assert!(!schema.is_concrete()); // abstract, not concrete
    }

    // New test: is_typed returns false for API schema
    #[test]
    fn test_is_typed_false_for_api() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::{SchemaInfo, register_schema};
        use super::super::stage::Stage;
        use usd_tf::Token;

        // Register a single-apply API schema
        let schema_info = SchemaInfo {
            identifier: Token::new("CollectionAPI"),
            type_name: "UsdCollectionAPI".to_string(),
            family: Token::new("CollectionAPI"),
            version: 0,
            kind: SchemaKind::SingleApplyAPI,
            base_type_names: Vec::new(),
            auto_apply_to: Vec::new(),
            can_only_apply_to: Vec::new(),
            allowed_instance_names: None,
            prim_definition: None,
        };
        register_schema(schema_info);

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/API", "CollectionAPI").unwrap();
        let schema = SchemaBase::new(prim);

        assert!(!schema.is_typed()); // API schemas are not typed
        assert!(!schema.is_concrete());
    }
}
