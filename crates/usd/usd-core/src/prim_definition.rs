//! Prim Definition - builtin definition of a prim given registered schemas.
//!
//! Port of pxr/usd/usd/primDefinition.h/cpp
//!
//! UsdPrimDefinition provides access to the builtin properties and metadata
//! of a prim whose type is defined by this definition. Instances can only be
//! created by the UsdSchemaRegistry.

use std::collections::HashMap;
use std::sync::Arc;

use super::schema_registry::SchemaRegistry;
use usd_sdf::value_type_name::ValueTypeName;
use usd_sdf::{Layer, Path, SpecType, Specifier, Variability};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// LayerAndPath (internal)
// ============================================================================

/// Internal structure for storing property access via layer and path.
///
/// Matches C++ `UsdPrimDefinition::_LayerAndPath`.
#[derive(Clone)]
pub(crate) struct LayerAndPath {
    /// Raw pointer to the layer (for efficiency).
    /// The schema registry ensures layers stay alive.
    layer: Option<Arc<Layer>>,
    /// Path to the property spec on the layer.
    path: Path,
}

impl LayerAndPath {
    /// Creates a new LayerAndPath.
    fn new(layer: Arc<Layer>, path: Path) -> Self {
        Self {
            layer: Some(layer),
            path,
        }
    }

    /// Creates an invalid LayerAndPath.
    fn invalid() -> Self {
        Self {
            layer: None,
            path: Path::default(),
        }
    }

    /// Returns true if this is valid.
    pub(crate) fn is_valid(&self) -> bool {
        self.layer.is_some()
    }

    /// Gets a field value from the layer.
    fn has_field<T>(&self, field_name: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        if let Some(ref layer) = self.layer {
            if let Some(val) = layer.get_field(&self.path, field_name) {
                if let Ok(v) = T::try_from(&val) {
                    *value = v;
                    return true;
                }
            }
        }
        false
    }

    /// Gets a dictionary key value from the layer.
    fn has_field_dict_key<T>(&self, field_name: &Token, key_path: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        if let Some(ref layer) = self.layer {
            let data = layer.data();
            if let Some(val) = data.get_dict_value_by_key(&self.path, field_name, key_path) {
                if let Ok(v) = T::try_from(&val) {
                    *value = v;
                    return true;
                }
            }
        }
        false
    }

    /// Gets a field value as Value.
    fn get_field(&self, field_name: &Token) -> Option<Value> {
        if let Some(ref layer) = self.layer {
            layer.get_field(&self.path, field_name)
        } else {
            None
        }
    }

    /// Gets a field value as a specific type T.
    ///
    /// Note: This method has strict trait bounds that most types don't satisfy.
    /// It's primarily useful for testing and for types with explicit TryFrom<&Value> impls.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn get_field_as<T>(&self, field_name: &Token) -> Option<T>
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        self.get_field(field_name)
            .and_then(|v| T::try_from(&v).ok())
    }
}

// ============================================================================
// Property
// ============================================================================

/// Accessor to a property's definition in the prim definition.
///
/// Matches C++ `UsdPrimDefinition::Property`.
#[derive(Clone)]
pub struct Property {
    /// The name of the property.
    name: Token,
    /// The layer and path for accessing the property spec.
    layer_and_path: Option<LayerAndPath>,
}

impl Property {
    /// Creates an invalid property.
    pub fn invalid() -> Self {
        Self {
            name: Token::new(""),
            layer_and_path: None,
        }
    }

    /// Creates a property from name and layer/path.
    pub(crate) fn new(name: Token, layer_and_path: Option<LayerAndPath>) -> Self {
        Self {
            name,
            layer_and_path,
        }
    }

    /// Returns the name of the requested property.
    ///
    /// Matches C++ `GetName()`.
    pub fn name(&self) -> &Token {
        &self.name
    }

    /// Returns true if this represents a valid property.
    ///
    /// Matches C++ `explicit operator bool()`.
    pub fn is_valid(&self) -> bool {
        self.layer_and_path.is_some()
    }

    /// Returns true if the property is an attribute.
    ///
    /// Matches C++ `IsAttribute()`.
    pub fn is_attribute(&self) -> bool {
        self.spec_type() == SpecType::Attribute
    }

    /// Returns true if the property is a relationship.
    ///
    /// Matches C++ `IsRelationship()`.
    pub fn is_relationship(&self) -> bool {
        self.spec_type() == SpecType::Relationship
    }

    /// Returns the spec type of this property.
    ///
    /// Matches C++ `GetSpecType()`.
    pub fn spec_type(&self) -> SpecType {
        if let Some(ref lap) = self.layer_and_path {
            if let Some(ref layer) = lap.layer {
                let data = layer.data();
                return data.get_spec_type(&lap.path);
            }
        }
        SpecType::Unknown
    }

    /// Returns the list of names of metadata fields defined for this property.
    ///
    /// Matches C++ `ListMetadataFields()`.
    pub fn list_metadata_fields(&self) -> Vec<Token> {
        if let Some(ref lap) = self.layer_and_path {
            if let Some(ref layer) = lap.layer {
                // Get the list of fields from the schematics for the property (or prim)
                // path and remove the fields that we don't allow fallbacks for.
                let mut fields = layer.list_fields(&lap.path);
                fields.retain(|field| !SchemaRegistry::is_disallowed_field(field));
                return fields;
            }
        }
        Vec::new()
    }

    /// Retrieves the fallback value for the metadata field.
    ///
    /// Matches C++ template `GetMetadata()`.
    pub fn get_metadata<T>(&self, key: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        if let Some(ref lap) = self.layer_and_path {
            // Check if field is disallowed
            if SchemaRegistry::is_disallowed_field(key) {
                return false;
            }
            return lap.has_field(key, value);
        }
        false
    }

    /// Retrieves the value at keyPath from dictionary metadata field.
    ///
    /// Matches C++ template `GetMetadataByDictKey()`.
    pub fn get_metadata_by_dict_key<T>(&self, key: &Token, key_path: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        if let Some(ref lap) = self.layer_and_path {
            // Check if field is disallowed
            if SchemaRegistry::is_disallowed_field(key) {
                return false;
            }
            return lap.has_field_dict_key(key, key_path, value);
        }
        false
    }

    /// Returns the variability of this property.
    ///
    /// Matches C++ `GetVariability()`.
    pub fn variability(&self) -> Variability {
        if let Some(ref lap) = self.layer_and_path {
            // Read variability field - it's stored as Variability enum
            if let Some(val) = lap.get_field(&Token::new("variability")) {
                // Try to extract Variability from value
                // Variability is stored directly as enum in Value
                if let Some(var) = val.get::<Variability>() {
                    return *var;
                }
            }
        }
        Variability::Varying
    }

    /// Returns the documentation metadata for this property.
    ///
    /// Matches C++ `GetDocumentation()`.
    pub fn documentation(&self) -> String {
        if let Some(ref lap) = self.layer_and_path {
            // Check validity first
            if !lap.is_valid() {
                return String::new();
            }
            if let Some(val) = lap.get_field(&Token::new("documentation")) {
                // Try to extract String from value
                if let Some(s) = val.get::<String>() {
                    return s.clone();
                }
                // Try Token as fallback
                if let Some(token) = val.get::<Token>() {
                    return token.as_str().to_string();
                }
            }
        }
        String::new()
    }
}

impl std::ops::Deref for Property {
    type Target = Token;

    fn deref(&self) -> &Self::Target {
        &self.name
    }
}

// ============================================================================
// Attribute
// ============================================================================

/// Accessor to an attribute's definition in the prim definition.
///
/// Matches C++ `UsdPrimDefinition::Attribute`.
#[derive(Clone)]
pub struct Attribute {
    /// The underlying property.
    property: Property,
}

impl Attribute {
    /// Creates an invalid attribute.
    pub fn invalid() -> Self {
        Self {
            property: Property::invalid(),
        }
    }

    /// Creates an attribute from a property.
    pub fn from_property(property: Property) -> Self {
        Self { property }
    }

    /// Returns true if this represents a valid attribute.
    ///
    /// Matches C++ `explicit operator bool()`.
    pub fn is_valid(&self) -> bool {
        self.property.is_valid() && self.property.is_attribute()
    }

    /// Returns the value type name of this attribute.
    ///
    /// Matches C++ `GetTypeName()`.
    ///
    /// Uses the global ValueTypeRegistry instance to look up the type name token.
    pub fn type_name(&self) -> ValueTypeName {
        let type_token = self.type_name_token();
        if type_token.is_empty() {
            return ValueTypeName::invalid();
        }
        use usd_sdf::ValueTypeRegistry;
        ValueTypeRegistry::instance().find_type_by_token(&type_token)
    }

    /// Returns the token value of the type name.
    ///
    /// Matches C++ `GetTypeNameToken()`.
    pub fn type_name_token(&self) -> Token {
        if let Some(ref lap) = self.property.layer_and_path {
            if let Some(val) = lap.get_field(&Token::new("typeName")) {
                if let Some(token) = val.get::<Token>() {
                    return token.clone();
                }
                // Try String as fallback and convert to Token
                if let Some(s) = val.get::<String>() {
                    return Token::new(s);
                }
            }
        }
        Token::new("")
    }

    /// Retrieves the fallback value of type T for this attribute.
    ///
    /// Matches C++ template `GetFallbackValue()`.
    pub fn get_fallback_value<T>(&self, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        if let Some(ref lap) = self.property.layer_and_path {
            return lap.has_field(&Token::new("default"), value);
        }
        false
    }

    /// Gets the fallback value as Value (for cases where T=Value).
    ///
    /// This is a helper method for getting fallback values when the type
    /// is Value itself, which doesn't implement TryFrom<&Value>.
    pub(crate) fn get_fallback_value_as_value(&self) -> Option<Value> {
        if let Some(ref lap) = self.property.layer_and_path {
            return lap.get_field(&Token::new("default"));
        }
        None
    }
}

impl From<Property> for Attribute {
    fn from(property: Property) -> Self {
        Self::from_property(property)
    }
}

// ============================================================================
// Relationship
// ============================================================================

/// Accessor to a relationship's definition in the prim definition.
///
/// Matches C++ `UsdPrimDefinition::Relationship`.
#[derive(Clone)]
pub struct Relationship {
    /// The underlying property.
    property: Property,
}

impl Relationship {
    /// Creates an invalid relationship.
    pub fn invalid() -> Self {
        Self {
            property: Property::invalid(),
        }
    }

    /// Creates a relationship from a property.
    pub fn from_property(property: Property) -> Self {
        Self { property }
    }

    /// Returns true if this represents a valid relationship.
    ///
    /// Matches C++ `explicit operator bool()`.
    pub fn is_valid(&self) -> bool {
        self.property.is_valid() && self.property.is_relationship()
    }
}

impl From<Property> for Relationship {
    fn from(property: Property) -> Self {
        Self::from_property(property)
    }
}

// ============================================================================
// PrimDefinition
// ============================================================================

/// Class representing the builtin definition of a prim given the schemas
/// registered in the schema registry.
///
/// Matches C++ `UsdPrimDefinition`.
#[derive(Clone)]
pub struct PrimDefinition {
    /// Path to the prim in the schematics for this prim definition.
    prim_layer_and_path: LayerAndPath,
    /// Map for caching the paths to each property spec in the schematics by property name.
    prop_layer_and_path_map: HashMap<Token, LayerAndPath>,
    /// List of applied API schemas.
    applied_api_schemas: Vec<Token>,
    /// Cached list of property names.
    properties: Vec<Token>,
    /// Layer that may be created for this prim definition if it's necessary to
    /// compose any new property specs for this definition from multiple
    /// property specs from other definitions.
    ///
    /// Reserved for future composition implementation. Currently unused but will be
    /// needed when composing API schemas with overlapping properties.
    _composed_property_layer: Option<Arc<Layer>>,
}

impl PrimDefinition {
    /// Creates a new empty prim definition.
    ///
    /// Matches C++ default constructor (private).
    pub(crate) fn new() -> Self {
        Self {
            prim_layer_and_path: LayerAndPath::invalid(),
            prop_layer_and_path_map: HashMap::new(),
            applied_api_schemas: Vec::new(),
            properties: Vec::new(),
            _composed_property_layer: None,
        }
    }

    /// Returns the list of names of builtin properties for this prim definition,
    /// ordered by this prim definition's propertyOrder.
    ///
    /// Matches C++ `GetPropertyNames()`.
    pub fn property_names(&self) -> &[Token] {
        &self.properties
    }

    /// Returns the list of names of the API schemas that have been applied to
    /// this prim definition in order.
    ///
    /// Matches C++ `GetAppliedAPISchemas()`.
    pub fn applied_api_schemas(&self) -> &[Token] {
        &self.applied_api_schemas
    }

    /// Returns a property accessor for the property named propName if it is
    /// defined by this prim definition.
    ///
    /// Matches C++ `GetPropertyDefinition()`.
    pub fn get_property_definition(&self, prop_name: &Token) -> Property {
        // For Typed schemas, the empty property is mapped to the prim path to
        // access prim metadata. We make sure that this can't be accessed via
        // the public GetPropertyDefinition since we only want this returning
        // true properties.
        if prop_name.is_empty() {
            return Property::invalid();
        }
        let layer_and_path = self.prop_layer_and_path_map.get(prop_name).cloned();
        Property::new(prop_name.clone(), layer_and_path)
    }

    /// Returns an attribute accessor for the property named attrName if it is
    /// defined by this prim definition and is an attribute.
    ///
    /// Matches C++ `GetAttributeDefinition()`.
    pub fn get_attribute_definition(&self, attr_name: &Token) -> Attribute {
        Attribute::from_property(self.get_property_definition(attr_name))
    }

    /// Returns a relationship accessor for the property named relName if it is
    /// defined by this prim definition and is a relationship.
    ///
    /// Matches C++ `GetRelationshipDefinition()`.
    pub fn get_relationship_definition(&self, rel_name: &Token) -> Relationship {
        Relationship::from_property(self.get_property_definition(rel_name))
    }

    /// Returns the SdfSpecType for propName if it is a builtin property of
    /// the prim type represented by this prim definition.
    ///
    /// Matches C++ `GetSpecType()`.
    pub fn get_spec_type(&self, prop_name: &Token) -> SpecType {
        let prop = self.get_property_definition(prop_name);
        if prop.is_valid() {
            return prop.spec_type();
        }
        SpecType::Unknown
    }

    /// Returns the list of names of metadata fields that are defined by this
    /// prim definition for the prim itself.
    ///
    /// Matches C++ `ListMetadataFields()`.
    pub fn list_metadata_fields(&self) -> Vec<Token> {
        // Prim metadata for Typed schema definitions is stored specially as an
        // empty named property which will not be returned by GetPropertyDefinition.
        // But we can still access it via the empty token mapping.
        if let Some(layer_and_path) = self.prop_layer_and_path_map.get(&Token::new("")) {
            if let Some(ref layer) = layer_and_path.layer {
                // Get the list of fields from the schematics for the prim path
                // and remove the fields that we don't allow fallbacks for.
                let mut fields = layer.list_fields(&layer_and_path.path);
                fields.retain(|field| !SchemaRegistry::is_disallowed_field(field));
                return fields;
            }
        }
        Vec::new()
    }

    /// Retrieves the fallback value for the metadata field named key.
    ///
    /// Matches C++ template `GetMetadata()`.
    pub fn get_metadata<T>(&self, key: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        // Check if field is disallowed
        if SchemaRegistry::is_disallowed_field(key) {
            return false;
        }
        self.prim_layer_and_path.has_field(key, value)
    }

    /// Retrieves the value at keyPath from the fallback dictionary value.
    ///
    /// Matches C++ template `GetMetadataByDictKey()`.
    pub fn get_metadata_by_dict_key<T>(&self, key: &Token, key_path: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        // Check if field is disallowed
        if SchemaRegistry::is_disallowed_field(key) {
            return false;
        }
        self.prim_layer_and_path
            .has_field_dict_key(key, key_path, value)
    }

    /// Returns the documentation metadata defined by the prim definition for
    /// the prim itself.
    ///
    /// Matches C++ `GetDocumentation()`.
    pub fn documentation(&self) -> String {
        // Special case for prim documentation. Pure API schemas don't map their
        // prim spec paths to the empty token. To get documentation for an API
        // schema, we have to get the documentation field from the schematics
        // for the prim path.
        let prop = Property::new(Token::new(""), Some(self.prim_layer_and_path.clone()));
        prop.documentation()
    }

    /// Returns the list of names of metadata fields that are defined by this
    /// prim definition for property propName if a property named propName exists.
    ///
    /// Matches C++ `ListPropertyMetadataFields()`.
    pub fn list_property_metadata_fields(&self, prop_name: &Token) -> Vec<Token> {
        let prop = self.get_property_definition(prop_name);
        if prop.is_valid() {
            return prop.list_metadata_fields();
        }
        Vec::new()
    }

    /// Retrieves the fallback value for the metadata field named key for the
    /// property named propName.
    ///
    /// Matches C++ template `GetPropertyMetadata()`.
    pub fn get_property_metadata<T>(&self, prop_name: &Token, key: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        let prop = self.get_property_definition(prop_name);
        if prop.is_valid() {
            return prop.get_metadata(key, value);
        }
        false
    }

    /// Retrieves the value at keyPath from the fallback dictionary value for
    /// the dictionary metadata field named key for the property named propName.
    ///
    /// Matches C++ template `GetPropertyMetadataByDictKey()`.
    pub fn get_property_metadata_by_dict_key<T>(
        &self,
        prop_name: &Token,
        key: &Token,
        key_path: &Token,
        value: &mut T,
    ) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        let prop = self.get_property_definition(prop_name);
        if prop.is_valid() {
            return prop.get_metadata_by_dict_key(key, key_path, value);
        }
        false
    }

    /// Returns the documentation metadata defined by the prim definition for
    /// the property named propName if it exists.
    ///
    /// Matches C++ `GetPropertyDocumentation()`.
    pub fn get_property_documentation(&self, prop_name: &Token) -> String {
        let prop = self.get_property_definition(prop_name);
        if prop.is_valid() {
            return prop.documentation();
        }
        String::new()
    }

    /// Retrieves the fallback value for the attribute named attrName.
    ///
    /// Matches C++ template `GetAttributeFallbackValue()`.
    pub fn get_attribute_fallback_value<T>(&self, attr_name: &Token, value: &mut T) -> bool
    where
        T: Clone + for<'a> TryFrom<&'a Value, Error = ()>,
    {
        let attr = self.get_attribute_definition(attr_name);
        if attr.is_valid() {
            return attr.get_fallback_value(value);
        }
        false
    }

    /// \deprecated Use GetPropertyDefinition instead.
    ///
    /// Returns the property spec that defines the fallback for the property
    /// named propName on prims of this prim definition's type. Returns None
    /// if there is no such property spec.
    ///
    /// Matches C++ `GetSchemaPropertySpec()`.
    #[deprecated(note = "Use get_property_definition instead")]
    pub fn get_schema_property_spec(&self, prop_name: &Token) -> Option<usd_sdf::PropertySpec> {
        if let Some(layer_and_path) = self.get_property_layer_and_path(prop_name) {
            if let Some(ref layer) = layer_and_path.layer {
                return layer.get_property_at_path(&layer_and_path.path);
            }
        }
        None
    }

    /// \deprecated Use GetAttributeDefinition instead.
    ///
    /// Returns the attribute spec that defines the fallback for the attribute
    /// named attrName on prims of this prim definition's type. Returns None
    /// if there is no such attribute spec.
    ///
    /// Matches C++ `GetSchemaAttributeSpec()`.
    #[deprecated(note = "Use get_attribute_definition instead")]
    pub fn get_schema_attribute_spec(&self, attr_name: &Token) -> Option<usd_sdf::AttributeSpec> {
        if let Some(layer_and_path) = self.get_property_layer_and_path(attr_name) {
            if let Some(ref layer) = layer_and_path.layer {
                return layer.get_attribute_at_path(&layer_and_path.path);
            }
        }
        None
    }

    /// \deprecated Use GetRelationshipDefinition instead.
    ///
    /// Returns the relationship spec that defines the fallback for the relationship
    /// named relName on prims of this prim definition's type. Returns None
    /// if there is no such relationship spec.
    ///
    /// Matches C++ `GetSchemaRelationshipSpec()`.
    #[deprecated(note = "Use get_relationship_definition instead")]
    pub fn get_schema_relationship_spec(
        &self,
        rel_name: &Token,
    ) -> Option<usd_sdf::RelationshipSpec> {
        if let Some(layer_and_path) = self.get_property_layer_and_path(rel_name) {
            if let Some(ref layer) = layer_and_path.layer {
                return layer.get_relationship_at_path(&layer_and_path.path);
            }
        }
        None
    }

    /// Copies the contents of this prim definition to a prim spec on the
    /// given layer at the given path. This includes the entire property
    /// spec for each of this definition's built-in properties as well as all of
    /// this definition's prim metadata.
    ///
    /// Matches C++ `FlattenTo(const SdfLayerHandle &layer, const SdfPath &path, SdfSpecifier newSpecSpecifier)`.
    pub fn flatten_to(
        &self,
        layer: &Arc<Layer>,
        path: &Path,
        new_spec_specifier: Specifier,
    ) -> bool {
        use usd_sdf::copy_utils::copy_spec;

        // Find or create the target prim spec at the target layer.
        let target_spec = layer.get_prim_at_path(path);

        if target_spec.is_some() {
            // If the target spec already exists, clear its properties and schema
            // allowed metadata. This does not clear non-schema metadata fields like
            // children, composition arc, clips, specifier, etc.

            // Note: In C++ this uses SetProperties(empty vector), but in Rust
            // we don't have a direct way to clear all properties. The properties
            // will be overwritten when we copy new ones below.

            // Clear schema-allowed metadata fields
            if let Some(ref spec) = target_spec {
                let info_keys = spec.spec().list_info_keys();
                for field_name in info_keys {
                    if !SchemaRegistry::is_disallowed_field(&field_name) {
                        layer.erase_field(path, &field_name);
                    }
                }
            }
        } else {
            // Otherwise create a new target spec and set its specifier.
            let new_spec = layer.create_prim_spec(path, new_spec_specifier, "");
            if new_spec.is_none() {
                eprintln!(
                    "Failed to create prim spec at path '{}' in layer '{}'",
                    path,
                    layer.identifier()
                );
                return false;
            }
        }

        // Copy all properties.
        for prop_name in self.property_names() {
            if let Some(layer_and_path) = self.get_property_layer_and_path(prop_name) {
                if let Some(ref src_layer) = layer_and_path.layer {
                    let prop_path = path.append_property(prop_name.as_str());
                    if prop_path.is_none() {
                        continue;
                    }
                    let prop_path = prop_path.expect("checked above");

                    // Copy the property spec
                    if !copy_spec(src_layer, &layer_and_path.path, layer, &prop_path) {
                        eprintln!(
                            "Failed to copy prim definition property '{}' to prim spec at path '{}' in layer '{}'.",
                            prop_name.as_str(),
                            path,
                            layer.identifier()
                        );
                    }
                }
            }
        }

        // Copy prim metadata
        for field_name in self.list_metadata_fields() {
            // Get the field value directly from the prim layer
            if let Some(field_value) = self.prim_layer_and_path.get_field(&field_name) {
                layer.set_field(path, &field_name, field_value);
            }
        }

        // Explicitly set the full list of applied API schemas in metadata
        // The apiSchemas field copied from prim metadata will only contain the
        // built-in API schemas of the underlying typed schemas but not any
        // additional API schemas that may have been applied to this definition.
        use usd_sdf::TokenListOp;
        let api_schemas_op = TokenListOp::create_explicit(self.applied_api_schemas().to_vec());
        layer.set_field(path, &Token::new("apiSchemas"), Value::from(api_schemas_op));

        // Also explicitly set the documentation string. This is necessary when
        // flattening an API schema prim definition as GetMetadata doesn't return
        // the documentation as metadata for API schemas.
        let doc = self.documentation();
        if !doc.is_empty() {
            layer.set_field(path, &Token::new("documentation"), Value::from(doc));
        }

        true
    }

    /// \overload
    /// Copies the contents of this prim definition to a prim spec at the
    /// current edit target for a prim with the given name under the prim parent.
    ///
    /// Matches C++ `FlattenTo(const UsdPrim &parent, const TfToken &name, SdfSpecifier newSpecSpecifier)`.
    pub fn flatten_to_prim(
        &self,
        parent: &crate::Prim,
        name: &Token,
        new_spec_specifier: Specifier,
    ) -> Option<crate::Prim> {
        // Create the path of the prim we're flattening to.
        let prim_path = parent.path().append_child(name.as_str())?;

        // Map the target prim to the edit target.
        let stage = parent.stage()?;
        let edit_target = stage.edit_target();
        let Some(target_layer) = edit_target.layer() else {
            return None;
        };
        let target_path = edit_target.map_to_spec_path(&prim_path);

        if target_path.is_empty() {
            return None;
        }

        if !self.flatten_to(target_layer, &target_path, new_spec_specifier) {
            return None;
        }

        stage.get_prim_at_path(&prim_path)
    }

    /// \overload
    /// Copies the contents of this prim definition to a prim spec at the
    /// current edit target for the given prim.
    ///
    /// Matches C++ `FlattenTo(const UsdPrim &prim, SdfSpecifier newSpecSpecifier)`.
    pub fn flatten_to_prim_direct(
        &self,
        prim: &crate::Prim,
        new_spec_specifier: Specifier,
    ) -> Option<crate::Prim> {
        let parent = prim.parent();
        let name = prim.name();
        self.flatten_to_prim(&parent, &name, new_spec_specifier)
    }

    // ========================================================================
    // Internal methods (for SchemaRegistry)
    // ========================================================================

    /// Gets the property layer and path for the given property name.
    pub(crate) fn get_property_layer_and_path(&self, prop_name: &Token) -> Option<&LayerAndPath> {
        self.prop_layer_and_path_map.get(prop_name)
    }

    /// Sets the prim layer and path.
    #[allow(dead_code)] // Internal API - prim definition building
    pub(crate) fn set_prim_layer_and_path(&mut self, layer: Arc<Layer>, path: Path) {
        self.prim_layer_and_path = LayerAndPath::new(layer, path);
    }

    /// Adds a property to the definition.
    #[allow(dead_code)] // Internal API - prim definition building
    pub(crate) fn add_property(&mut self, prop_name: Token, layer: Arc<Layer>, path: Path) {
        let layer_and_path = LayerAndPath::new(layer, path);
        if self
            .prop_layer_and_path_map
            .insert(prop_name.clone(), layer_and_path)
            .is_none()
        {
            self.properties.push(prop_name);
        }
    }

    /// Sets the applied API schemas.
    #[allow(dead_code)] // Internal API - prim definition building
    pub(crate) fn set_applied_api_schemas(&mut self, schemas: Vec<Token>) {
        self.applied_api_schemas = schemas;
    }

    /// Applies property order metadata.
    ///
    /// Matches C++ `_ApplyPropertyOrder()`.
    ///
    /// Uses the same algorithm as `UsdPrim::ApplyPropertyOrder`: iterates through
    /// the order list and rotates matching properties to the front of the list.
    pub(crate) fn apply_property_order(&mut self) {
        // Read propertyOrder metadata from prim
        if let Some(val) = self
            .prim_layer_and_path
            .get_field(&Token::new("propertyOrder"))
        {
            if let Some(prop_order) = val.get::<Vec<Token>>() {
                // If order is empty or properties is empty, nothing to do.
                if prop_order.is_empty() || self.properties.is_empty() {
                    return;
                }

                // Perf note: this walks 'order' and linear searches 'properties' to find each
                // element, for O(M*N) operations. This matches the C++ implementation which
                // uses linear search for efficiency (TfToken pointer comparisons vs string comparisons).

                let mut names_rest = 0;

                for o_name in prop_order {
                    // Look for this name from 'order' in the rest of 'properties'.
                    if let Some(i) = self.properties[names_rest..]
                        .iter()
                        .position(|p| p == o_name)
                    {
                        let actual_idx = names_rest + i;
                        // Found. Move to the front by rotating the sub-range.
                        // This matches C++ std::rotate behavior.
                        self.properties[names_rest..=actual_idx].rotate_right(1);
                        names_rest += 1;
                    }
                }
            }
        }
    }

    /// Initializes this prim definition for a typed schema.
    ///
    /// Matches C++ `_IntializeForTypedSchema()`.
    pub(crate) fn initialize_for_typed_schema(
        &mut self,
        schematics_layer: Arc<Layer>,
        schematics_prim_path: Path,
        properties_to_ignore: &[Token],
    ) -> bool {
        self.prim_layer_and_path =
            LayerAndPath::new(schematics_layer, schematics_prim_path.clone());

        // Verify the layer and path is valid before proceeding
        if !self.prim_layer_and_path.is_valid() {
            return false;
        }

        if self.map_schematics_property_paths(properties_to_ignore) {
            // Prim definition for Typed schemas use the prim spec to provide prim
            // level metadata, so we map the empty property name to the prim path
            // in the schematics for the field accessor functions. This mapping aids
            // the efficiency of value resolution by allowing UsdStage to access
            // fallback metadata from both prims and properties through the same
            // code path without extra conditionals.
            self.prop_layer_and_path_map
                .insert(Token::new(""), self.prim_layer_and_path.clone());

            // Store properties according to the propertyOrder metadata
            self.apply_property_order();
            return true;
        }
        false
    }

    /// Initializes this prim definition for an API schema.
    ///
    /// Matches C++ `_IntializeForAPISchema()`.
    pub(crate) fn initialize_for_api_schema(
        &mut self,
        api_schema_name: Token,
        schematics_layer: Arc<Layer>,
        schematics_prim_path: Path,
        properties_to_ignore: &[Token],
    ) -> bool {
        // We always include the API schema itself as the first applied API
        // schema in its prim definition.
        self.applied_api_schemas = vec![api_schema_name.clone()];

        self.prim_layer_and_path =
            LayerAndPath::new(schematics_layer, schematics_prim_path.clone());

        let (_identifier, instance) = SchemaRegistry::get_type_name_and_instance(&api_schema_name);

        // Only single apply API schemas are allowed to provide prim metadata.
        if self.map_schematics_property_paths(properties_to_ignore) && instance.is_empty() {
            self.prop_layer_and_path_map
                .insert(Token::new(""), self.prim_layer_and_path.clone());
        }

        // Store properties according to the propertyOrder metadata
        self.apply_property_order();
        true
    }

    /// Maps property paths from the schematics layer.
    ///
    /// Matches C++ `_MapSchematicsPropertyPaths()`.
    fn map_schematics_property_paths(&mut self, properties_to_ignore: &[Token]) -> bool {
        // Get the names of all the properties defined in the prim spec.
        let spec_property_names = if let Some(val) = self
            .prim_layer_and_path
            .get_field(&Token::new("properties"))
        {
            if let Some(names) = val.get::<Vec<Token>>() {
                names.clone()
            } else {
                Vec::new()
            }
        } else {
            // Check if prim spec exists
            if let Some(ref layer) = self.prim_layer_and_path.layer {
                let data = layer.data();
                if !data.has_spec(&self.prim_layer_and_path.path) {
                    // While it's possible for the spec to have no properties, we expect
                    // the prim spec itself to exist.
                    eprintln!(
                        "Warning: No prim spec exists at path '{}' in schematics layer {}.",
                        self.prim_layer_and_path.path,
                        layer.identifier()
                    );
                    return false;
                }
            }
            return true;
        };

        // Reserve space for properties
        self.properties.reserve(spec_property_names.len());

        // If there are no properties to ignore, just add all properties found in the spec.
        // Otherwise, we need to check to skip any ignored properties.
        if properties_to_ignore.is_empty() {
            for prop_name in spec_property_names {
                if let Some(prop_path) = self
                    .prim_layer_and_path
                    .path
                    .append_property(prop_name.as_str())
                {
                    if let Some(ref layer) = self.prim_layer_and_path.layer {
                        self.add_property(prop_name, layer.clone(), prop_path);
                    }
                }
            }
        } else {
            for prop_name in spec_property_names {
                // Note: propertiesToIgnore list is expected to be extremely small
                // (like a few entries at most) so linear search should be efficient enough.
                if !properties_to_ignore.contains(&prop_name) {
                    if let Some(prop_path) = self
                        .prim_layer_and_path
                        .path
                        .append_property(prop_name.as_str())
                    {
                        if let Some(ref layer) = self.prim_layer_and_path.layer {
                            self.add_property(prop_name, layer.clone(), prop_path);
                        }
                    }
                }
            }
        }

        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: creates a test layer with a simple prim spec
    fn create_test_layer_with_prim(prim_name: &str) -> Arc<Layer> {
        // Create anonymous layer for testing
        let layer = Layer::create_anonymous(Some(&format!("test_{}", prim_name)));

        // Create a simple prim spec at root
        let prim_path = Path::from_string(&format!("/{}", prim_name)).unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        layer
    }

    // Helper: creates a test layer with a prim that has properties
    fn create_test_layer_with_properties(prim_name: &str, prop_names: &[&str]) -> Arc<Layer> {
        let layer = create_test_layer_with_prim(prim_name);
        let prim_path = Path::from_string(&format!("/{}", prim_name)).unwrap();

        // Add properties
        for prop_name in prop_names {
            let prop_path = prim_path.append_property(prop_name).unwrap();
            // Create spec using create_spec instead of create_attribute_spec
            layer.create_spec(&prop_path, SpecType::Attribute);
        }

        layer
    }

    #[test]
    fn test_layer_and_path_is_valid() {
        // Test invalid LayerAndPath
        let invalid = LayerAndPath::invalid();
        assert!(!invalid.is_valid());

        // Test valid LayerAndPath
        let layer = create_test_layer_with_prim("TestPrim");
        let path = Path::from_string("/TestPrim").unwrap();
        let valid = LayerAndPath::new(layer, path);
        assert!(valid.is_valid());
    }

    #[test]
    fn test_layer_and_path_get_field_as() {
        let layer = create_test_layer_with_prim("TestPrim");
        let path = Path::from_string("/TestPrim").unwrap();

        let lap = LayerAndPath::new(layer, path);

        // Note: get_field_as needs T: TryFrom<&Value, Error=()>.
        // String doesn't implement this, so we need to use a different type
        // or implement the trait for String
        // For now, just verify LayerAndPath is valid
        assert!(lap.is_valid());
    }

    #[test]
    fn test_prim_definition_apply_property_order() {
        let layer =
            create_test_layer_with_properties("TestPrim", &["propA", "propB", "propC", "propD"]);
        let prim_path = Path::from_string("/TestPrim").unwrap();

        let mut prim_def = PrimDefinition::new();
        prim_def.set_prim_layer_and_path(layer.clone(), prim_path.clone());

        // Add properties in arbitrary order
        for prop_name in ["propA", "propB", "propC", "propD"] {
            let prop_path = prim_path.append_property(prop_name).unwrap();
            prim_def.add_property(Token::new(prop_name), layer.clone(), prop_path);
        }

        // Set propertyOrder metadata to reorder them
        let property_order = vec![
            Token::new("propD"),
            Token::new("propB"),
            Token::new("propA"),
            Token::new("propC"),
        ];
        layer.set_field(
            &prim_path,
            &Token::new("propertyOrder"),
            Value::from(property_order),
        );

        // Apply property order
        prim_def.apply_property_order();

        // Verify the order matches propertyOrder metadata
        let props = prim_def.property_names();
        assert_eq!(props[0].as_str(), "propD");
        assert_eq!(props[1].as_str(), "propB");
        assert_eq!(props[2].as_str(), "propA");
        assert_eq!(props[3].as_str(), "propC");
    }

    #[test]
    fn test_prim_definition_initialize_for_typed_schema() {
        let layer =
            create_test_layer_with_properties("TestTypedPrim", &["size", "color", "visible"]);
        let prim_path = Path::from_string("/TestTypedPrim").unwrap();

        let mut prim_def = PrimDefinition::new();
        let result = prim_def.initialize_for_typed_schema(
            layer.clone(),
            prim_path.clone(),
            &[], // No properties to ignore
        );

        assert!(result);

        // Verify properties were mapped
        let props = prim_def.property_names();
        assert_eq!(props.len(), 3);
        assert!(props.iter().any(|p| p == "size"));
        assert!(props.iter().any(|p| p == "color"));
        assert!(props.iter().any(|p| p == "visible"));

        // Verify prim metadata is accessible via empty token
        assert!(
            prim_def
                .get_property_layer_and_path(&Token::new(""))
                .is_some()
        );
    }

    #[test]
    fn test_prim_definition_initialize_for_typed_schema_with_ignored_props() {
        let layer = create_test_layer_with_properties(
            "TestTypedPrim",
            &["size", "color", "visible", "internal"],
        );
        let prim_path = Path::from_string("/TestTypedPrim").unwrap();

        let mut prim_def = PrimDefinition::new();
        let result = prim_def.initialize_for_typed_schema(
            layer.clone(),
            prim_path.clone(),
            &[Token::new("internal")], // Ignore "internal" property
        );

        assert!(result);

        // Verify properties were mapped except ignored one
        let props = prim_def.property_names();
        assert_eq!(props.len(), 3);
        assert!(props.iter().any(|p| p == "size"));
        assert!(props.iter().any(|p| p == "color"));
        assert!(props.iter().any(|p| p == "visible"));
        assert!(!props.iter().any(|p| p == "internal"));
    }

    #[test]
    fn test_prim_definition_initialize_for_api_schema() {
        let layer = create_test_layer_with_properties("TestAPI", &["apiAttr1", "apiAttr2"]);
        let prim_path = Path::from_string("/TestAPI").unwrap();

        let api_schema_name = Token::new("TestAPI");
        let mut prim_def = PrimDefinition::new();
        let result = prim_def.initialize_for_api_schema(
            api_schema_name.clone(),
            layer.clone(),
            prim_path.clone(),
            &[], // No properties to ignore
        );

        assert!(result);

        // Verify properties were mapped
        let props = prim_def.property_names();
        assert_eq!(props.len(), 2);
        assert!(props.iter().any(|p| p == "apiAttr1"));
        assert!(props.iter().any(|p| p == "apiAttr2"));

        // Verify applied API schema is recorded
        let applied_schemas = prim_def.applied_api_schemas();
        assert_eq!(applied_schemas.len(), 1);
        assert_eq!(applied_schemas[0], api_schema_name);

        // For single-apply API schema (no instance name), prim metadata should be accessible
        assert!(
            prim_def
                .get_property_layer_and_path(&Token::new(""))
                .is_some()
        );
    }

    #[test]
    fn test_prim_definition_initialize_for_multi_apply_api_schema() {
        let layer = create_test_layer_with_properties(
            "CollectionAPI",
            &[
                "collection:__INSTANCE_NAME__:includes",
                "collection:__INSTANCE_NAME__:excludes",
            ],
        );
        let prim_path = Path::from_string("/CollectionAPI").unwrap();

        // Multiple-apply API schema with instance name
        let api_schema_name = Token::new("CollectionAPI:plasticStuff");
        let mut prim_def = PrimDefinition::new();
        let result = prim_def.initialize_for_api_schema(
            api_schema_name.clone(),
            layer.clone(),
            prim_path.clone(),
            &[],
        );

        assert!(result);

        // Multiple-apply API schemas should NOT have prim metadata accessible
        // because they have an instance name
        assert!(
            prim_def
                .get_property_layer_and_path(&Token::new(""))
                .is_none()
        );

        // Verify applied API schema is recorded
        let applied_schemas = prim_def.applied_api_schemas();
        assert_eq!(applied_schemas.len(), 1);
        assert_eq!(applied_schemas[0], api_schema_name);
    }

    #[test]
    fn test_prim_definition_map_schematics_property_paths() {
        let layer = create_test_layer_with_properties("TestPrim", &["prop1", "prop2", "prop3"]);
        let prim_path = Path::from_string("/TestPrim").unwrap();

        let mut prim_def = PrimDefinition::new();
        prim_def.prim_layer_and_path = LayerAndPath::new(layer.clone(), prim_path.clone());

        // Call map_schematics_property_paths
        let result = prim_def.map_schematics_property_paths(&[]);
        assert!(result);

        // Verify all properties are mapped
        assert_eq!(prim_def.properties.len(), 3);
        assert!(
            prim_def
                .prop_layer_and_path_map
                .contains_key(&Token::new("prop1"))
        );
        assert!(
            prim_def
                .prop_layer_and_path_map
                .contains_key(&Token::new("prop2"))
        );
        assert!(
            prim_def
                .prop_layer_and_path_map
                .contains_key(&Token::new("prop3"))
        );
    }
}
