//! UsdAPISchemaBase - Base class for all API schemas.
//!
//! An API schema provides an interface to a prim's qualities, but does not
//! specify a typeName for the underlying prim. The prim's qualities include
//! its inheritance structure, attributes, relationships etc.
//!
//! API schemas are classified into applied and non-applied API schemas.
//! Applied API schemas are further classified into single-apply and
//! multiple-apply API schemas.

use super::common::SchemaKind;
use super::prim::Prim;
use usd_tf::Token;

// ============================================================================
// UsdAPISchemaBase
// ============================================================================

/// The base class for all API schemas.
///
/// An API schema provides an interface to a prim's qualities, but does not
/// specify a typeName for the underlying prim. Since it cannot provide a
/// typeName, an API schema is considered to be non-concrete.
///
/// # Applied vs Non-Applied API Schemas
///
/// - **Non-applied**: Provide interface to set metadata (like UsdModelAPI,
///   UsdClipsAPI). These can be used without being recorded on the prim.
///
/// - **Applied**: Need to discover/record whether a prim subscribes to the
///   schema. Applied schemas add properties to a prim and must be applied
///   via an `apply()` method.
///
/// # Single vs Multiple Apply
///
/// - **Single-apply**: Can only be applied once to a prim.
///
/// - **Multiple-apply**: Can be applied multiple times with different
///   instance names (e.g., UsdCollectionAPI for collections).
///
/// # Example
///
/// ```ignore
/// // Check if API schema is applied
/// if prim.has_api::<UsdCollectionAPI>("myCollection") {
///     let api = UsdCollectionAPI::new(&prim, "myCollection");
///     // Use the API...
/// }
/// ```
#[derive(Debug, Clone)]
pub struct APISchemaBase {
    /// The underlying prim.
    prim: Prim,
    /// Instance name for multiple-apply schemas.
    instance_name: Token,
}

impl APISchemaBase {
    /// Schema kind for API schema base (abstract base).
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::AbstractBase;

    /// Constructs an APISchemaBase on the given prim.
    pub fn new(prim: &Prim) -> Self {
        Self {
            prim: prim.clone(),
            instance_name: Token::new(""),
        }
    }

    /// Constructs a multiple-apply APISchemaBase with the given instance name.
    pub fn with_instance_name(prim: &Prim, instance_name: &Token) -> Self {
        Self {
            prim: prim.clone(),
            instance_name: instance_name.clone(),
        }
    }

    /// Constructs from another schema object.
    pub fn from_schema(schema: &super::schema_base::SchemaBase) -> Self {
        Self {
            prim: schema.prim().clone(),
            instance_name: Token::new(""),
        }
    }

    /// Constructs from another schema object with instance name.
    pub fn from_schema_with_instance(
        schema: &super::schema_base::SchemaBase,
        instance_name: &Token,
    ) -> Self {
        Self {
            prim: schema.prim().clone(),
            instance_name: instance_name.clone(),
        }
    }

    /// Returns the instance name for multiple-apply schemas.
    ///
    /// Returns an empty token for non-applied and single-apply schemas.
    pub fn instance_name(&self) -> &Token {
        &self.instance_name
    }

    /// Returns true if this API schema is compatible with the current prim.
    ///
    /// For applied API schemas, this checks if the schema is applied to the
    /// prim with the correct instance name.
    pub fn is_compatible(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        // For non-applied schemas, just check prim validity
        if self.instance_name.is_empty() {
            return true;
        }

        // For applied schemas, would need to check has_api
        // This base class doesn't know the schema type, so derived classes
        // should override
        true
    }

    /// Returns the schema kind (always AbstractBase for this class).
    pub fn schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns schema attribute names.
    ///
    /// API schema base has no attributes, derived classes override this.
    pub fn schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }

    /// Returns instance names for a multiple-apply schema on a prim.
    ///
    /// # Arguments
    ///
    /// * `prim` - The prim to query
    /// * `schema_identifier` - The schema's API schema identifier (e.g., "CollectionAPI")
    ///
    /// # Returns
    ///
    /// Vector of instance names for the schema applied to this prim.
    pub fn get_multiple_apply_instance_names(prim: &Prim, schema_identifier: &Token) -> Vec<Token> {
        if !prim.is_valid() {
            return Vec::new();
        }

        let mut instances = Vec::new();
        let prefix = format!("{}:", schema_identifier.get_text());

        for schema in prim.get_applied_schemas() {
            let schema_text = schema.get_text();
            if let Some(instance_part) = schema_text.strip_prefix(&prefix) {
                instances.push(Token::new(instance_part));
            }
        }

        instances
    }

    /// Creates the API schema name for application.
    ///
    /// For single-apply: returns the identifier as-is.
    /// For multiple-apply: returns "SchemaName:instanceName".
    pub fn make_schema_identifier_for_apply(
        schema_identifier: &Token,
        instance_name: Option<&Token>,
    ) -> Token {
        match instance_name {
            Some(name) if !name.is_empty() => Token::new(&format!(
                "{}:{}",
                schema_identifier.get_text(),
                name.get_text()
            )),
            _ => schema_identifier.clone(),
        }
    }
}

impl APISchemaBase {
    /// Returns the prim this API schema wraps.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns true if this API schema object is valid.
    pub fn is_valid(&self) -> bool {
        self.is_compatible()
    }
}

impl Default for APISchemaBase {
    fn default() -> Self {
        Self {
            prim: Prim::invalid(),
            instance_name: Token::new(""),
        }
    }
}

impl PartialEq for APISchemaBase {
    fn eq(&self, other: &Self) -> bool {
        self.prim == other.prim && self.instance_name == other.instance_name
    }
}

impl Eq for APISchemaBase {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let schema = APISchemaBase::default();
        assert!(!schema.is_valid());
        assert!(schema.instance_name().is_empty());
    }

    #[test]
    fn test_schema_kind() {
        let schema = APISchemaBase::default();
        assert_eq!(schema.schema_kind(), SchemaKind::AbstractBase);
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = APISchemaBase::schema_attribute_names(true);
        assert!(names.is_empty());
    }

    #[test]
    fn test_make_schema_identifier_single_apply() {
        let id = Token::new("ModelAPI");
        let result = APISchemaBase::make_schema_identifier_for_apply(&id, None);
        assert_eq!(result.get_text(), "ModelAPI");
    }

    #[test]
    fn test_make_schema_identifier_multiple_apply() {
        let id = Token::new("CollectionAPI");
        let instance = Token::new("myCollection");
        let result = APISchemaBase::make_schema_identifier_for_apply(&id, Some(&instance));
        assert_eq!(result.get_text(), "CollectionAPI:myCollection");
    }
}
