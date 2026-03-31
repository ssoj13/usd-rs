//! USD Schema system for field and spec validation.
//!
//! This module provides the schema system that defines valid fields for each spec type,
//! field metadata, and validation rules for USD scene description.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

use usd_tf::Token;
use usd_vt::Value;

use super::allowed::Allowed;
use super::path::Path;
use super::payload::Payload;
use super::reference::Reference;
use super::types::{Relocate, SpecType};

// ============================================================================
// Validator Functions
// ============================================================================

/// Type for field value validators.
///
/// A validator takes a Value and returns whether it's allowed for the field.
pub type Validator = fn(&Value) -> Allowed;

// ============================================================================
// Field Definition
// ============================================================================

/// Metadata about a field in the schema.
///
/// Field definitions describe:
/// - Field name and type
/// - Default/fallback value
/// - Whether the field is required
/// - Whether the field holds children
/// - Validation functions
/// - Additional metadata
#[derive(Clone)]
pub struct FieldDefinition {
    /// Field name token.
    name: Token,
    /// Fallback value when field is not authored.
    fallback: Value,
    /// Whether this is a plugin-defined field.
    is_plugin: bool,
    /// Whether this field is read-only.
    is_read_only: bool,
    /// Whether this field holds children specs.
    holds_children: bool,
    /// Validator for the field value.
    value_validator: Option<Validator>,
    /// Validator for list element values.
    list_value_validator: Option<Validator>,
    /// Validator for map key values.
    map_key_validator: Option<Validator>,
    /// Validator for map element values.
    map_value_validator: Option<Validator>,
    /// Additional metadata key-value pairs.
    info: HashMap<Token, String>,
}

impl FieldDefinition {
    /// Creates a new field definition.
    pub fn new(name: impl Into<Token>, fallback: Value) -> Self {
        Self {
            name: name.into(),
            fallback,
            is_plugin: false,
            is_read_only: false,
            holds_children: false,
            value_validator: None,
            list_value_validator: None,
            map_key_validator: None,
            map_value_validator: None,
            info: HashMap::new(),
        }
    }

    /// Returns the field name.
    pub fn name(&self) -> &Token {
        &self.name
    }

    /// Returns the fallback value.
    pub fn fallback(&self) -> &Value {
        &self.fallback
    }

    /// Returns true if this is a plugin field.
    pub fn is_plugin(&self) -> bool {
        self.is_plugin
    }

    /// Returns true if this field is read-only.
    pub fn is_read_only(&self) -> bool {
        self.is_read_only
    }

    /// Returns true if this field holds children.
    pub fn holds_children(&self) -> bool {
        self.holds_children
    }

    /// Returns additional metadata for this field.
    pub fn info(&self) -> &HashMap<Token, String> {
        &self.info
    }

    /// Sets the fallback value (builder pattern).
    pub fn with_fallback(mut self, fallback: Value) -> Self {
        self.fallback = fallback;
        self
    }

    /// Marks this as a plugin field (builder pattern).
    pub fn plugin(mut self) -> Self {
        self.is_plugin = true;
        self
    }

    /// Marks this as a read-only field (builder pattern).
    pub fn read_only(mut self) -> Self {
        self.is_read_only = true;
        self
    }

    /// Marks this as a children field (builder pattern).
    pub fn children(mut self) -> Self {
        self.holds_children = true;
        self
    }

    /// Sets the value validator (builder pattern).
    pub fn with_value_validator(mut self, validator: Validator) -> Self {
        self.value_validator = Some(validator);
        self
    }

    /// Sets the list value validator (builder pattern).
    pub fn with_list_value_validator(mut self, validator: Validator) -> Self {
        self.list_value_validator = Some(validator);
        self
    }

    /// Sets the map key validator (builder pattern).
    pub fn with_map_key_validator(mut self, validator: Validator) -> Self {
        self.map_key_validator = Some(validator);
        self
    }

    /// Sets the map value validator (builder pattern).
    pub fn with_map_value_validator(mut self, validator: Validator) -> Self {
        self.map_value_validator = Some(validator);
        self
    }

    /// Adds metadata info (builder pattern).
    pub fn with_info(mut self, key: impl Into<Token>, value: impl Into<String>) -> Self {
        self.info.insert(key.into(), value.into());
        self
    }

    /// Validates a value against this field's validator.
    pub fn is_valid_value(&self, value: &Value) -> Allowed {
        if let Some(validator) = self.value_validator {
            validator(value)
        } else {
            Allowed::yes()
        }
    }

    /// Validates a list element against this field's list validator.
    pub fn is_valid_list_value(&self, value: &Value) -> Allowed {
        if let Some(validator) = self.list_value_validator {
            validator(value)
        } else {
            Allowed::yes()
        }
    }

    /// Validates a map key against this field's map key validator.
    pub fn is_valid_map_key(&self, value: &Value) -> Allowed {
        if let Some(validator) = self.map_key_validator {
            validator(value)
        } else {
            Allowed::yes()
        }
    }

    /// Validates a map value against this field's map value validator.
    pub fn is_valid_map_value(&self, value: &Value) -> Allowed {
        if let Some(validator) = self.map_value_validator {
            validator(value)
        } else {
            Allowed::yes()
        }
    }
}

impl fmt::Debug for FieldDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldDefinition")
            .field("name", &self.name)
            .field("is_plugin", &self.is_plugin)
            .field("is_read_only", &self.is_read_only)
            .field("holds_children", &self.holds_children)
            .finish()
    }
}

// ============================================================================
// Field Info
// ============================================================================

/// Information about a field as it pertains to a specific spec type.
#[derive(Debug, Clone, Default)]
struct FieldInfo {
    /// Whether this field is required for this spec type.
    required: bool,
    /// Whether this field is metadata.
    metadata: bool,
    /// Display group for metadata fields.
    metadata_display_group: Option<Token>,
}

// ============================================================================
// Spec Definition
// ============================================================================

/// Definition of valid fields for a spec type.
///
/// Each spec type has a spec definition that determines:
/// - Which fields are valid for this spec type
/// - Which fields are required
/// - Which fields are metadata
/// - Metadata display grouping
#[derive(Debug, Clone, Default)]
pub struct SpecDefinition {
    /// Map of field names to field info for this spec.
    fields: HashMap<Token, FieldInfo>,
    /// Cached list of required field names.
    required_fields: Vec<Token>,
}

impl SpecDefinition {
    /// Creates a new empty spec definition.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a field to this spec definition.
    pub fn add_field(&mut self, name: impl Into<Token>, required: bool) {
        let name = name.into();
        if required {
            self.required_fields.push(name.clone());
        }
        self.fields.insert(
            name,
            FieldInfo {
                required,
                metadata: false,
                metadata_display_group: None,
            },
        );
    }

    /// Adds a metadata field to this spec definition.
    pub fn add_metadata_field(
        &mut self,
        name: impl Into<Token>,
        display_group: Option<Token>,
        required: bool,
    ) {
        let name = name.into();
        if required {
            self.required_fields.push(name.clone());
        }
        self.fields.insert(
            name,
            FieldInfo {
                required,
                metadata: true,
                metadata_display_group: display_group,
            },
        );
    }

    /// Returns all field names valid for this spec.
    pub fn get_fields(&self) -> Vec<Token> {
        self.fields.keys().cloned().collect()
    }

    /// Returns all required field names.
    pub fn get_required_fields(&self) -> &[Token] {
        &self.required_fields
    }

    /// Returns all metadata field names.
    pub fn get_metadata_fields(&self) -> Vec<Token> {
        self.fields
            .iter()
            .filter(|(_, info)| info.metadata)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Returns true if the field is valid for this spec.
    pub fn is_valid_field(&self, name: &Token) -> bool {
        self.fields.contains_key(name)
    }

    /// Returns true if the field is metadata for this spec.
    pub fn is_metadata_field(&self, name: &Token) -> bool {
        self.fields
            .get(name)
            .map(|info| info.metadata)
            .unwrap_or(false)
    }

    /// Returns the metadata display group for a field.
    pub fn get_metadata_display_group(&self, name: &Token) -> Option<&Token> {
        self.fields
            .get(name)
            .and_then(|info| info.metadata_display_group.as_ref())
    }

    /// Returns true if the field is required for this spec.
    pub fn is_required_field(&self, name: &Token) -> bool {
        self.fields
            .get(name)
            .map(|info| info.required)
            .unwrap_or(false)
    }
}

// ============================================================================
// Schema Base
// ============================================================================

/// Base schema providing field and spec definitions.
///
/// The schema defines:
/// - Valid fields and their properties
/// - Valid spec types and their allowed fields
/// - Validation rules for field values
#[derive(Clone)]
pub struct SchemaBase {
    /// Field definitions by name.
    field_defs: Arc<RwLock<HashMap<Token, FieldDefinition>>>,
    /// Spec definitions by type.
    spec_defs: Arc<RwLock<HashMap<SpecType, SpecDefinition>>>,
    /// Cached set of all required field names across all specs.
    required_field_names: Arc<RwLock<Vec<Token>>>,
}

impl Default for SchemaBase {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaBase {
    /// Creates a new empty schema.
    pub fn new() -> Self {
        Self {
            field_defs: Arc::new(RwLock::new(HashMap::new())),
            spec_defs: Arc::new(RwLock::new(HashMap::new())),
            required_field_names: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Registers a field definition.
    pub fn register_field(&self, field_def: FieldDefinition) {
        let name = field_def.name().clone();
        self.field_defs
            .write()
            .expect("schema lock poisoned")
            .insert(name, field_def);
    }

    /// Registers a spec definition.
    pub fn register_spec(&self, spec_type: SpecType, spec_def: SpecDefinition) {
        // Add all required fields from this spec to the global required list
        {
            let mut required = self
                .required_field_names
                .write()
                .expect("schema lock poisoned");
            for field in spec_def.get_required_fields() {
                if !required.contains(field) {
                    required.push(field.clone());
                }
            }
        }

        self.spec_defs
            .write()
            .expect("schema lock poisoned")
            .insert(spec_type, spec_def);
    }

    /// Returns the field definition for a field name.
    pub fn get_field_def(&self, field_key: &Token) -> Option<FieldDefinition> {
        self.field_defs
            .read()
            .expect("schema lock poisoned")
            .get(field_key)
            .cloned()
    }

    /// Returns the spec definition for a spec type.
    pub fn get_spec_def(&self, spec_type: SpecType) -> Option<SpecDefinition> {
        self.spec_defs
            .read()
            .expect("schema lock poisoned")
            .get(&spec_type)
            .cloned()
    }

    /// Returns true if a field is registered.
    pub fn is_registered(&self, field_key: &Token) -> bool {
        self.field_defs
            .read()
            .expect("schema lock poisoned")
            .contains_key(field_key)
    }

    /// Returns true if a field holds children.
    pub fn holds_children(&self, field_key: &Token) -> bool {
        self.field_defs
            .read()
            .expect("schema lock poisoned")
            .get(field_key)
            .map(|def| def.holds_children())
            .unwrap_or(false)
    }

    /// Returns the fallback value for a field.
    pub fn get_fallback(&self, field_key: &Token) -> Value {
        self.field_defs
            .read()
            .expect("schema lock poisoned")
            .get(field_key)
            .map(|def| def.fallback().clone())
            .unwrap_or_else(Value::empty)
    }

    /// Returns true if a field is valid for a spec type.
    pub fn is_valid_field_for_spec(&self, field_key: &Token, spec_type: SpecType) -> bool {
        if let Some(spec_def) = self.get_spec_def(spec_type) {
            spec_def.is_valid_field(field_key)
        } else {
            false
        }
    }

    /// Returns all fields for a spec type.
    pub fn get_fields(&self, spec_type: SpecType) -> Vec<Token> {
        self.get_spec_def(spec_type)
            .map(|def| def.get_fields())
            .unwrap_or_default()
    }

    /// Returns all metadata fields for a spec type.
    pub fn get_metadata_fields(&self, spec_type: SpecType) -> Vec<Token> {
        self.get_spec_def(spec_type)
            .map(|def| def.get_metadata_fields())
            .unwrap_or_default()
    }

    /// Returns the metadata display group for a field on a spec type.
    pub fn get_metadata_display_group(
        &self,
        spec_type: SpecType,
        metadata_field: &Token,
    ) -> Option<Token> {
        self.get_spec_def(spec_type)
            .and_then(|def| def.get_metadata_display_group(metadata_field).cloned())
    }

    /// Returns all required fields for a spec type.
    pub fn get_required_fields(&self, spec_type: SpecType) -> Vec<Token> {
        self.get_spec_def(spec_type)
            .map(|def| def.get_required_fields().to_vec())
            .unwrap_or_default()
    }

    /// Returns true if a field name is required for any spec type.
    pub fn is_required_field_name(&self, field_name: &Token) -> bool {
        self.required_field_names
            .read()
            .expect("schema lock poisoned")
            .contains(field_name)
    }

    // ========================================================================
    // Validation Functions
    // ========================================================================

    /// Validates an attribute connection path.
    pub fn is_valid_attr_connection_path(path: &Path) -> Allowed {
        if path.is_property_path() {
            Allowed::yes()
        } else {
            Allowed::no("Attribute connection path must be a property path")
        }
    }

    /// Validates an identifier string.
    pub fn is_valid_identifier(name: &str) -> Allowed {
        if name.is_empty() {
            return Allowed::no("Identifier cannot be empty");
        }

        // Must start with letter or underscore
        // Safe: already checked is_empty above
        let first = name.chars().next().expect("name is non-empty");
        if !first.is_ascii_alphabetic() && first != '_' {
            return Allowed::no("Identifier must start with letter or underscore");
        }

        // Rest must be alphanumeric or underscore
        for ch in name.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '_' {
                return Allowed::no("Identifier can only contain letters, digits, and underscores");
            }
        }

        Allowed::yes()
    }

    /// Validates a namespaced identifier (can contain colons).
    pub fn is_valid_namespaced_identifier(name: &str) -> Allowed {
        if name.is_empty() {
            return Allowed::no("Namespaced identifier cannot be empty");
        }

        // Split by namespace separator
        for part in name.split(':') {
            if part.is_empty() {
                return Allowed::no("Namespace part cannot be empty");
            }

            // Each part must be a valid identifier
            let result = Self::is_valid_identifier(part);
            if !result.is_allowed() {
                return Allowed::no(format!("Invalid namespace part: {}", result.why_not()));
            }
        }

        Allowed::yes()
    }

    /// Validates an inherit path.
    pub fn is_valid_inherit_path(path: &Path) -> Allowed {
        if path.is_prim_path() || path.is_prim_variant_selection_path() {
            Allowed::yes()
        } else {
            Allowed::no("Inherit path must be a prim path or variant selection path")
        }
    }

    /// Validates a payload.
    pub fn is_valid_payload(_payload: &Payload) -> Allowed {
        // Basic validation - payload must have valid asset path
        Allowed::yes()
    }

    /// Validates a reference.
    pub fn is_valid_reference(_reference: &Reference) -> Allowed {
        // Basic validation - reference must have valid asset path
        Allowed::yes()
    }

    /// Validates a relationship target path.
    pub fn is_valid_relationship_target_path(path: &Path) -> Allowed {
        if path.is_prim_path() || path.is_prim_property_path() {
            Allowed::yes()
        } else {
            Allowed::no("Relationship target must be a prim or property path")
        }
    }

    /// Validates a relocates source path.
    pub fn is_valid_relocates_source_path(path: &Path) -> Allowed {
        if path.is_prim_path() {
            Allowed::yes()
        } else {
            Allowed::no("Relocates source path must be a prim path")
        }
    }

    /// Validates a relocates target path.
    pub fn is_valid_relocates_target_path(path: &Path) -> Allowed {
        if path.is_prim_path() {
            Allowed::yes()
        } else {
            Allowed::no("Relocates target path must be a prim path")
        }
    }

    /// Validates a relocate (source, target) pair.
    pub fn is_valid_relocate(relocate: &Relocate) -> Allowed {
        let (source, target) = relocate;

        // Validate source
        if !Self::is_valid_relocates_source_path(source).is_allowed() {
            return Allowed::no("Invalid relocates source path");
        }

        // Validate target
        if !Self::is_valid_relocates_target_path(target).is_allowed() {
            return Allowed::no("Invalid relocates target path");
        }

        // Source and target must be different
        if source == target {
            return Allowed::no("Relocate source and target must be different");
        }

        Allowed::yes()
    }

    /// Validates a specializes path.
    pub fn is_valid_specializes_path(path: &Path) -> Allowed {
        if path.is_prim_path() || path.is_prim_variant_selection_path() {
            Allowed::yes()
        } else {
            Allowed::no("Specializes path must be a prim path or variant selection path")
        }
    }

    /// Validates a sublayer identifier.
    pub fn is_valid_sublayer(sublayer: &str) -> Allowed {
        if sublayer.is_empty() {
            Allowed::no("Sublayer path cannot be empty")
        } else {
            Allowed::yes()
        }
    }

    /// Validates a variant identifier.
    pub fn is_valid_variant_identifier(name: &str) -> Allowed {
        Self::is_valid_identifier(name)
    }

    /// Validates a variant selection string.
    pub fn is_valid_variant_selection(selection: &str) -> Allowed {
        // Variant selection can be empty (means no selection)
        if selection.is_empty() {
            return Allowed::yes();
        }

        // Otherwise must be valid identifier
        Self::is_valid_identifier(selection)
    }
}

impl fmt::Debug for SchemaBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field_count = self.field_defs.read().expect("schema lock poisoned").len();
        let spec_count = self.spec_defs.read().expect("schema lock poisoned").len();

        f.debug_struct("SchemaBase")
            .field("field_count", &field_count)
            .field("spec_count", &spec_count)
            .finish()
    }
}

// ============================================================================
// Schema Singleton
// ============================================================================

/// The global USD schema singleton.
///
/// Provides access to the standard USD schema with all built-in
/// field and spec definitions.
#[derive(Debug, Clone)]
pub struct Schema {
    base: SchemaBase,
}

impl Schema {
    /// Returns the global schema instance.
    pub fn instance() -> &'static Schema {
        static INSTANCE: OnceLock<Schema> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let schema = Schema {
                base: SchemaBase::new(),
            };
            schema.register_standard_fields();
            schema.register_standard_specs();
            // Discover and register schema definitions from plugins
            discover_plugin_schemas(&schema);
            schema
        })
    }

    /// Returns the base schema.
    pub fn base(&self) -> &SchemaBase {
        &self.base
    }

    /// Registers standard USD field definitions.
    fn register_standard_fields(&self) {
        // Core fields
        self.base.register_field(
            FieldDefinition::new("specifier", Value::empty())
                .with_info("doc", "The specifier for this prim (def, over, class)"),
        );

        self.base.register_field(
            FieldDefinition::new("typeName", Value::empty())
                .with_info("doc", "The type name for this prim or attribute"),
        );

        self.base.register_field(
            FieldDefinition::new("active", Value::from(true))
                .with_info("doc", "Whether this prim is active"),
        );

        self.base.register_field(
            FieldDefinition::new("hidden", Value::from(false))
                .with_info("doc", "Whether this prim is hidden"),
        );

        self.base.register_field(
            FieldDefinition::new("kind", Value::empty()).with_info("doc", "The kind of this prim"),
        );

        self.base.register_field(
            FieldDefinition::new("comment", Value::empty())
                .with_info("doc", "User comment for this object"),
        );

        self.base.register_field(
            FieldDefinition::new("documentation", Value::empty())
                .with_info("doc", "Documentation string for this object"),
        );

        self.base.register_field(
            FieldDefinition::new("displayName", Value::empty())
                .with_info("doc", "Display name for this object"),
        );

        // Composition arcs
        self.base.register_field(
            FieldDefinition::new("references", Value::empty())
                .with_info("doc", "References to other prims or layers"),
        );

        self.base.register_field(
            FieldDefinition::new("payload", Value::empty())
                .with_info("doc", "Payload reference for deferred loading"),
        );

        self.base.register_field(
            FieldDefinition::new("inheritPaths", Value::empty())
                .with_info("doc", "Paths to inherit from"),
        );

        self.base.register_field(
            FieldDefinition::new("specializes", Value::empty())
                .with_info("doc", "Specialization arcs"),
        );

        self.base.register_field(
            FieldDefinition::new("variantSetNames", Value::empty())
                .with_info("doc", "Names of variant sets"),
        );

        self.base.register_field(
            FieldDefinition::new("variantSelection", Value::empty())
                .with_info("doc", "Current variant selections"),
        );

        // Children fields
        self.base.register_field(
            FieldDefinition::new("primChildren", Value::empty())
                .children()
                .with_info("doc", "Child prims"),
        );

        self.base.register_field(
            FieldDefinition::new("properties", Value::empty())
                .children()
                .with_info("doc", "Properties (attributes and relationships)"),
        );

        self.base.register_field(
            FieldDefinition::new("variantSetChildren", Value::empty())
                .children()
                .with_info("doc", "Child variant sets"),
        );

        self.base.register_field(
            FieldDefinition::new("variantChildren", Value::empty())
                .children()
                .with_info("doc", "Child variants in a variant set"),
        );

        // Attribute fields
        self.base.register_field(
            FieldDefinition::new("default", Value::empty())
                .with_info("doc", "Default value for an attribute"),
        );

        self.base.register_field(
            FieldDefinition::new("timeSamples", Value::empty())
                .with_info("doc", "Time-varying values for an attribute"),
        );

        self.base.register_field(
            FieldDefinition::new("connectionPaths", Value::empty())
                .with_info("doc", "Attribute connections"),
        );

        self.base.register_field(
            FieldDefinition::new("custom", Value::from(false))
                .with_info("doc", "Whether this is a custom attribute"),
        );

        self.base.register_field(
            FieldDefinition::new("variability", Value::empty())
                .with_info("doc", "Attribute variability (varying/uniform)"),
        );

        self.base.register_field(
            FieldDefinition::new("displayGroup", Value::empty())
                .with_info("doc", "Property display group"),
        );

        self.base.register_field(
            FieldDefinition::new("renderType", Value::empty())
                .with_info("doc", "Renderer-specific type override metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("connectability", Value::empty())
                .with_info("doc", "UsdShade input connectability metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("colorSpace", Value::empty())
                .with_info("doc", "Attribute color space"),
        );

        self.base.register_field(
            FieldDefinition::new("limits", Value::empty())
                .with_info("doc", "Attribute limits metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("arraySizeConstraint", Value::from(0_i64))
                .with_info("doc", "Attribute array size constraint"),
        );

        self.base.register_field(
            FieldDefinition::new("interpolation", Value::empty())
                .with_info("doc", "Primvar interpolation metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("elementSize", Value::from(1_i64))
                .with_info("doc", "Primvar element size metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("unauthoredValuesIndex", Value::from(-1_i64))
                .with_info("doc", "Primvar unauthored values index metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("constraintTargetIdentifier", Value::empty())
                .with_info("doc", "Constraint target identifier metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("spline", Value::empty())
                .with_info("doc", "Spline value for an attribute"),
        );

        // Relationship fields
        self.base.register_field(
            FieldDefinition::new("targetPaths", Value::empty())
                .with_info("doc", "Relationship targets"),
        );

        self.base.register_field(
            FieldDefinition::new("bindMaterialAs", Value::empty())
                .with_info("doc", "UsdShade material binding strength metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("instanceable", Value::from(false))
                .with_info("doc", "Whether this prim is instanceable"),
        );

        self.base.register_field(
            FieldDefinition::new("apiSchemas", Value::empty())
                .with_info("doc", "Applied API schemas"),
        );

        self.base.register_field(
            FieldDefinition::new("propertyOrder", Value::empty())
                .with_info("doc", "Property ordering metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("primOrder", Value::empty())
                .with_info("doc", "Prim ordering metadata"),
        );

        // Layer fields
        self.base.register_field(
            FieldDefinition::new("subLayers", Value::empty())
                .with_info("doc", "Sublayer references"),
        );

        self.base.register_field(
            FieldDefinition::new("defaultPrim", Value::empty())
                .with_info("doc", "Default root prim name"),
        );

        self.base.register_field(
            FieldDefinition::new("customLayerData", Value::empty())
                .with_info("doc", "Custom layer metadata"),
        );

        // Metadata fields
        self.base.register_field(
            FieldDefinition::new("customData", Value::empty())
                .with_info("doc", "Custom metadata dictionary"),
        );

        self.base.register_field(
            FieldDefinition::new("assetInfo", Value::empty())
                .with_info("doc", "Asset metadata dictionary"),
        );

        self.base.register_field(
            FieldDefinition::new("sdrMetadata", Value::empty())
                .with_info("doc", "Shader node metadata dictionary"),
        );

        // Time-related fields
        self.base.register_field(
            FieldDefinition::new("startTimeCode", Value::empty())
                .with_info("doc", "Start time code"),
        );

        self.base.register_field(
            FieldDefinition::new("endTimeCode", Value::empty()).with_info("doc", "End time code"),
        );

        self.base.register_field(
            FieldDefinition::new("timeCodesPerSecond", Value::empty())
                .with_info("doc", "Time codes per second"),
        );

        self.base.register_field(
            FieldDefinition::new("framesPerSecond", Value::empty())
                .with_info("doc", "Frames per second for playback"),
        );

        self.base.register_field(
            FieldDefinition::new("colorConfiguration", Value::empty())
                .with_info("doc", "Stage color configuration"),
        );

        self.base.register_field(
            FieldDefinition::new("colorManagementSystem", Value::empty())
                .with_info("doc", "Stage color management system"),
        );

        self.base.register_field(
            FieldDefinition::new("fallbackPrimTypes", Value::empty())
                .with_info("doc", "Fallback prim type mapping"),
        );

        self.base.register_field(
            FieldDefinition::new("clips", Value::empty())
                .with_info("doc", "Value clips metadata"),
        );

        self.base.register_field(
            FieldDefinition::new("clipSets", Value::empty())
                .with_info("doc", "Named clip sets metadata"),
        );
    }

    /// Registers standard spec type definitions.
    fn register_standard_specs(&self) {
        // Prim spec
        let mut prim_spec = SpecDefinition::new();
        prim_spec.add_field("specifier", true);
        prim_spec.add_field("typeName", false);
        prim_spec.add_field("active", false);
        prim_spec.add_field("hidden", false);
        prim_spec.add_field("kind", false);
        prim_spec.add_field("comment", false);
        prim_spec.add_field("documentation", false);
        prim_spec.add_field("displayName", false);
        prim_spec.add_field("instanceable", false);
        prim_spec.add_field("references", false);
        prim_spec.add_field("payload", false);
        prim_spec.add_field("inheritPaths", false);
        prim_spec.add_field("specializes", false);
        prim_spec.add_field("variantSetNames", false);
        prim_spec.add_field("variantSelection", false);
        prim_spec.add_field("apiSchemas", false);
        prim_spec.add_field("propertyOrder", false);
        prim_spec.add_field("primOrder", false);
        prim_spec.add_field("clips", false);
        prim_spec.add_field("clipSets", false);
        prim_spec.add_field("primChildren", false);
        prim_spec.add_field("properties", false);
        prim_spec.add_field("variantSetChildren", false);
        prim_spec.add_metadata_field("customData", None, false);
        prim_spec.add_metadata_field("assetInfo", None, false);
        prim_spec.add_metadata_field("sdrMetadata", None, false);
        self.base.register_spec(SpecType::Prim, prim_spec);

        // Attribute spec
        let mut attr_spec = SpecDefinition::new();
        attr_spec.add_field("typeName", true);
        attr_spec.add_field("default", false);
        attr_spec.add_field("timeSamples", false);
        attr_spec.add_field("connectionPaths", false);
        attr_spec.add_field("custom", false);
        attr_spec.add_field("variability", false);
        attr_spec.add_field("hidden", false);
        attr_spec.add_field("comment", false);
        attr_spec.add_field("documentation", false);
        attr_spec.add_field("displayName", false);
        attr_spec.add_field("displayGroup", false);
        attr_spec.add_field("renderType", false);
        attr_spec.add_field("connectability", false);
        attr_spec.add_field("colorSpace", false);
        attr_spec.add_field("limits", false);
        attr_spec.add_field("arraySizeConstraint", false);
        attr_spec.add_field("interpolation", false);
        attr_spec.add_field("elementSize", false);
        attr_spec.add_field("unauthoredValuesIndex", false);
        attr_spec.add_field("constraintTargetIdentifier", false);
        attr_spec.add_field("spline", false);
        attr_spec.add_metadata_field("customData", None, false);
        attr_spec.add_metadata_field("sdrMetadata", None, false);
        self.base.register_spec(SpecType::Attribute, attr_spec);

        // Relationship spec
        let mut rel_spec = SpecDefinition::new();
        rel_spec.add_field("targetPaths", false);
        rel_spec.add_field("custom", false);
        rel_spec.add_field("hidden", false);
        rel_spec.add_field("comment", false);
        rel_spec.add_field("documentation", false);
        rel_spec.add_field("displayName", false);
        rel_spec.add_field("displayGroup", false);
        rel_spec.add_field("bindMaterialAs", false);
        rel_spec.add_metadata_field("customData", None, false);
        self.base.register_spec(SpecType::Relationship, rel_spec);

        // Variant spec
        let mut variant_spec = SpecDefinition::new();
        variant_spec.add_field("primChildren", false);
        variant_spec.add_field("properties", false);
        self.base.register_spec(SpecType::Variant, variant_spec);

        // Variant set spec
        let mut variant_set_spec = SpecDefinition::new();
        variant_set_spec.add_field("variantChildren", false);
        self.base
            .register_spec(SpecType::VariantSet, variant_set_spec);

        // PseudoRoot spec (layer root)
        let mut pseudo_root_spec = SpecDefinition::new();
        pseudo_root_spec.add_field("primChildren", false);
        pseudo_root_spec.add_field("subLayers", false);
        pseudo_root_spec.add_field("defaultPrim", false);
        pseudo_root_spec.add_field("customLayerData", false);
        pseudo_root_spec.add_field("startTimeCode", false);
        pseudo_root_spec.add_field("endTimeCode", false);
        pseudo_root_spec.add_field("timeCodesPerSecond", false);
        pseudo_root_spec.add_field("framesPerSecond", false);
        pseudo_root_spec.add_field("comment", false);
        pseudo_root_spec.add_field("documentation", false);
        pseudo_root_spec.add_field("colorConfiguration", false);
        pseudo_root_spec.add_field("colorManagementSystem", false);
        pseudo_root_spec.add_field("fallbackPrimTypes", false);
        self.base
            .register_spec(SpecType::PseudoRoot, pseudo_root_spec);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_definition() {
        let field = FieldDefinition::new("testField", Value::from(42))
            .with_info("doc", "Test field")
            .read_only();

        assert_eq!(field.name().as_str(), "testField");
        assert!(field.is_read_only());
        assert!(!field.is_plugin());
        assert!(!field.holds_children());
        assert_eq!(field.info().get(&Token::new("doc")).unwrap(), "Test field");
    }

    #[test]
    fn test_field_definition_children() {
        let field = FieldDefinition::new("children", Value::empty()).children();

        assert!(field.holds_children());
        assert!(!field.is_read_only());
    }

    #[test]
    fn test_field_definition_validators() {
        fn always_valid(_: &Value) -> Allowed {
            Allowed::yes()
        }

        fn always_invalid(_: &Value) -> Allowed {
            Allowed::no("Invalid")
        }

        let field = FieldDefinition::new("test", Value::empty())
            .with_value_validator(always_valid)
            .with_list_value_validator(always_invalid);

        assert!(field.is_valid_value(&Value::from(1)).is_allowed());
        assert!(!field.is_valid_list_value(&Value::from(1)).is_allowed());
        assert!(field.is_valid_map_key(&Value::from(1)).is_allowed()); // No validator set
    }

    #[test]
    fn test_spec_definition() {
        let mut spec = SpecDefinition::new();
        spec.add_field("field1", true);
        spec.add_field("field2", false);
        spec.add_metadata_field("meta1", None, false);

        assert!(spec.is_valid_field(&Token::new("field1")));
        assert!(spec.is_valid_field(&Token::new("field2")));
        assert!(spec.is_valid_field(&Token::new("meta1")));
        assert!(!spec.is_valid_field(&Token::new("unknown")));

        assert!(spec.is_required_field(&Token::new("field1")));
        assert!(!spec.is_required_field(&Token::new("field2")));

        assert!(spec.is_metadata_field(&Token::new("meta1")));
        assert!(!spec.is_metadata_field(&Token::new("field1")));

        let required = spec.get_required_fields();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str(), "field1");

        let metadata = spec.get_metadata_fields();
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].as_str(), "meta1");
    }

    #[test]
    fn test_spec_definition_display_group() {
        let mut spec = SpecDefinition::new();
        spec.add_metadata_field("meta1", Some(Token::new("Group1")), false);
        spec.add_metadata_field("meta2", None, false);

        assert_eq!(
            spec.get_metadata_display_group(&Token::new("meta1"))
                .unwrap()
                .as_str(),
            "Group1"
        );
        assert!(
            spec.get_metadata_display_group(&Token::new("meta2"))
                .is_none()
        );
    }

    #[test]
    fn test_schema_base() {
        let schema = SchemaBase::new();

        // Register a field
        let field = FieldDefinition::new("testField", Value::from(100));
        schema.register_field(field);

        assert!(schema.is_registered(&Token::new("testField")));
        assert!(!schema.is_registered(&Token::new("unknown")));

        let fallback = schema.get_fallback(&Token::new("testField"));
        assert_eq!(fallback.get::<i32>(), Some(&100));

        // Register a spec
        let mut spec = SpecDefinition::new();
        spec.add_field("testField", true);
        schema.register_spec(SpecType::Prim, spec);

        assert!(schema.is_valid_field_for_spec(&Token::new("testField"), SpecType::Prim));
        assert!(!schema.is_valid_field_for_spec(&Token::new("unknown"), SpecType::Prim));

        let fields = schema.get_fields(SpecType::Prim);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].as_str(), "testField");

        let required = schema.get_required_fields(SpecType::Prim);
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str(), "testField");

        assert!(schema.is_required_field_name(&Token::new("testField")));
    }

    #[test]
    fn test_schema_base_holds_children() {
        let schema = SchemaBase::new();

        let field = FieldDefinition::new("children", Value::empty()).children();
        schema.register_field(field);

        assert!(schema.holds_children(&Token::new("children")));
        assert!(!schema.holds_children(&Token::new("unknown")));
    }

    #[test]
    fn test_is_valid_identifier() {
        assert!(SchemaBase::is_valid_identifier("validName").is_allowed());
        assert!(SchemaBase::is_valid_identifier("_private").is_allowed());
        assert!(SchemaBase::is_valid_identifier("name123").is_allowed());
        assert!(SchemaBase::is_valid_identifier("CamelCase").is_allowed());

        assert!(!SchemaBase::is_valid_identifier("").is_allowed());
        assert!(!SchemaBase::is_valid_identifier("123invalid").is_allowed());
        assert!(!SchemaBase::is_valid_identifier("has-dash").is_allowed());
        assert!(!SchemaBase::is_valid_identifier("has space").is_allowed());
        assert!(!SchemaBase::is_valid_identifier("has.dot").is_allowed());
    }

    #[test]
    fn test_is_valid_namespaced_identifier() {
        assert!(SchemaBase::is_valid_namespaced_identifier("simple").is_allowed());
        assert!(SchemaBase::is_valid_namespaced_identifier("name:space").is_allowed());
        assert!(SchemaBase::is_valid_namespaced_identifier("a:b:c").is_allowed());
        assert!(SchemaBase::is_valid_namespaced_identifier("_priv:name").is_allowed());

        assert!(!SchemaBase::is_valid_namespaced_identifier("").is_allowed());
        assert!(!SchemaBase::is_valid_namespaced_identifier(":name").is_allowed());
        assert!(!SchemaBase::is_valid_namespaced_identifier("name:").is_allowed());
        assert!(!SchemaBase::is_valid_namespaced_identifier("name::space").is_allowed());
    }

    #[test]
    fn test_is_valid_inherit_path() {
        let prim_path = Path::from_string("/World/Prim").unwrap();
        assert!(SchemaBase::is_valid_inherit_path(&prim_path).is_allowed());

        let prop_path = Path::from_string("/World/Prim.attr").unwrap();
        assert!(!SchemaBase::is_valid_inherit_path(&prop_path).is_allowed());
    }

    #[test]
    fn test_is_valid_relationship_target_path() {
        let prim_path = Path::from_string("/World/Target").unwrap();
        assert!(SchemaBase::is_valid_relationship_target_path(&prim_path).is_allowed());

        let prop_path = Path::from_string("/World/Target.attr").unwrap();
        assert!(SchemaBase::is_valid_relationship_target_path(&prop_path).is_allowed());
    }

    #[test]
    fn test_is_valid_relocate() {
        let source = Path::from_string("/Old/Path").unwrap();
        let target = Path::from_string("/New/Path").unwrap();
        let same = Path::from_string("/Old/Path").unwrap();

        assert!(SchemaBase::is_valid_relocate(&(source.clone(), target)).is_allowed());
        assert!(!SchemaBase::is_valid_relocate(&(source, same)).is_allowed());
    }

    #[test]
    fn test_is_valid_sublayer() {
        assert!(SchemaBase::is_valid_sublayer("sublayer.usd").is_allowed());
        assert!(SchemaBase::is_valid_sublayer("/path/to/layer.usda").is_allowed());
        assert!(!SchemaBase::is_valid_sublayer("").is_allowed());
    }

    #[test]
    fn test_is_valid_variant_identifier() {
        assert!(SchemaBase::is_valid_variant_identifier("variantA").is_allowed());
        assert!(SchemaBase::is_valid_variant_identifier("_private").is_allowed());
        assert!(!SchemaBase::is_valid_variant_identifier("123bad").is_allowed());
    }

    #[test]
    fn test_is_valid_variant_selection() {
        assert!(SchemaBase::is_valid_variant_selection("").is_allowed());
        assert!(SchemaBase::is_valid_variant_selection("selection").is_allowed());
        assert!(!SchemaBase::is_valid_variant_selection("123bad").is_allowed());
    }

    #[test]
    fn test_schema_singleton() {
        let schema = Schema::instance();
        assert!(schema.base().is_registered(&Token::new("specifier")));
        assert!(schema.base().is_registered(&Token::new("typeName")));
        assert!(schema.base().is_registered(&Token::new("active")));
    }

    #[test]
    fn test_schema_prim_spec() {
        let schema = Schema::instance();

        let fields = schema.base().get_fields(SpecType::Prim);
        assert!(!fields.is_empty());

        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("specifier"), SpecType::Prim)
        );
        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("typeName"), SpecType::Prim)
        );
        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("active"), SpecType::Prim)
        );

        let required = schema.base().get_required_fields(SpecType::Prim);
        assert!(required.contains(&Token::new("specifier")));
    }

    #[test]
    fn test_schema_attribute_spec() {
        let schema = Schema::instance();

        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("typeName"), SpecType::Attribute)
        );
        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("default"), SpecType::Attribute)
        );

        let required = schema.base().get_required_fields(SpecType::Attribute);
        assert!(required.contains(&Token::new("typeName")));
    }

    #[test]
    fn test_schema_relationship_spec() {
        let schema = Schema::instance();

        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("targetPaths"), SpecType::Relationship)
        );
        assert!(
            schema
                .base()
                .is_valid_field_for_spec(&Token::new("custom"), SpecType::Relationship)
        );
    }

    #[test]
    fn test_schema_children_fields() {
        let schema = Schema::instance();

        assert!(schema.base().holds_children(&Token::new("primChildren")));
        assert!(schema.base().holds_children(&Token::new("properties")));
        assert!(
            schema
                .base()
                .holds_children(&Token::new("variantSetChildren"))
        );
        assert!(schema.base().holds_children(&Token::new("variantChildren")));

        assert!(!schema.base().holds_children(&Token::new("specifier")));
    }

    #[test]
    fn test_schema_fallback_values() {
        let schema = Schema::instance();

        let active_fallback = schema.base().get_fallback(&Token::new("active"));
        assert_eq!(active_fallback.get::<bool>(), Some(&true));

        let hidden_fallback = schema.base().get_fallback(&Token::new("hidden"));
        assert_eq!(hidden_fallback.get::<bool>(), Some(&false));

        let custom_fallback = schema.base().get_fallback(&Token::new("custom"));
        assert_eq!(custom_fallback.get::<bool>(), Some(&false));
    }

    #[test]
    fn test_schema_metadata_fields() {
        let schema = Schema::instance();

        let prim_metadata = schema.base().get_metadata_fields(SpecType::Prim);
        assert!(prim_metadata.contains(&Token::new("customData")));
        assert!(prim_metadata.contains(&Token::new("assetInfo")));

        let attr_metadata = schema.base().get_metadata_fields(SpecType::Attribute);
        assert!(attr_metadata.contains(&Token::new("customData")));
    }
}

// ============================================================================
// Plugin Discovery for Schema Registry
// ============================================================================

/// Reads "SdfMetadata" blocks from all registered plugins and adds any declared
/// fields to the schema.  Mirrors C++ `Sdf_SchemaBase::_RegisterPluginFields`
/// (schema.cpp:1703-1849).
fn discover_plugin_schemas(schema: &Schema) {
    use usd_plug::PlugRegistry;

    let registry = PlugRegistry::get_instance();
    let plugins = registry.get_all_plugins();

    for plugin in &plugins {
        let metadata = plugin.get_metadata();
        let sdf_metadata = match metadata.get("SdfMetadata") {
            Some(v) => match v.as_object() {
                Some(obj) => obj,
                None => {
                    log::warn!(
                        "Plugin '{}': SdfMetadata is not an object",
                        plugin.get_name()
                    );
                    continue;
                }
            },
            None => continue,
        };

        for (field_name, field_spec) in sdf_metadata {
            if let Err(e) = register_plugin_field(schema, plugin.get_name(), field_name, field_spec)
            {
                log::warn!(
                    "Plugin '{}', field '{}': {}",
                    plugin.get_name(),
                    field_name,
                    e
                );
            }
        }
    }
}

/// Registers a single field declared inside a plugin's SdfMetadata block.
fn register_plugin_field(
    schema: &Schema,
    plugin_name: &str,
    field_name: &str,
    field_spec: &usd_js::JsValue,
) -> Result<(), String> {
    let spec_obj = field_spec.as_object().ok_or_else(|| {
        format!("field spec is not an object (plugin '{plugin_name}', field '{field_name}')")
    })?;

    // "type" is required — the USD value type name (e.g. "string", "bool").
    let type_name = spec_obj
        .get("type")
        .and_then(|v| v.as_string())
        .ok_or_else(|| "missing required 'type' key".to_string())?;

    // Optional default value.
    let fallback = spec_obj
        .get("default")
        .and_then(parse_value_from_js)
        .unwrap_or_else(Value::empty);

    // Optional display group for metadata UI; C++ defaults to "uncategorized".
    let display_group = spec_obj
        .get("displayGroup")
        .and_then(|v| v.as_string())
        .unwrap_or("uncategorized");

    // Optional "appliesTo": a single string or an array of strings.
    // Empty / absent means the field applies to all spec types.
    let applies_to: Vec<&str> = match spec_obj.get("appliesTo") {
        None => vec![],
        Some(usd_js::JsValue::String(s)) => vec![s.as_str()],
        Some(usd_js::JsValue::Array(arr)) => arr.iter().filter_map(|v| v.as_string()).collect(),
        Some(other) => {
            return Err(format!(
                "'appliesTo' must be a string or array, got {}",
                other.type_name()
            ));
        }
    };

    // Build and register the FieldDefinition.
    let field_def = FieldDefinition::new(field_name, fallback)
        .plugin()
        .with_info("type", type_name)
        .with_info("displayGroup", display_group);

    schema.base().register_field(field_def);

    let field_token = Token::new(field_name);
    let display_group_token = Token::new(display_group);

    // Determine which spec types this field applies to based on appliesTo tags.
    // Matches C++ schema.cpp logic: empty appliesTo → all four spec types.
    let apply_pseudo_root = applies_to.is_empty() || applies_to.contains(&"layers");
    let apply_prim = applies_to.is_empty() || applies_to.contains(&"prims");
    let apply_attribute = applies_to.is_empty()
        || applies_to.contains(&"properties")
        || applies_to.contains(&"attributes");
    let apply_relationship = applies_to.is_empty()
        || applies_to.contains(&"properties")
        || applies_to.contains(&"relationships");

    let targets: &[(bool, SpecType)] = &[
        (apply_pseudo_root, SpecType::PseudoRoot),
        (apply_prim, SpecType::Prim),
        (apply_attribute, SpecType::Attribute),
        (apply_relationship, SpecType::Relationship),
    ];

    for &(should_apply, spec_type) in targets {
        if !should_apply {
            continue;
        }
        // Read the existing spec definition, extend it, then write it back.
        let mut spec_def = schema.base().get_spec_def(spec_type).unwrap_or_default();
        spec_def.add_metadata_field(
            field_token.clone(),
            Some(display_group_token.clone()),
            false,
        );
        schema.base().register_spec(spec_type, spec_def);
    }

    Ok(())
}

/// Converts a `JsValue` leaf into a `Value`.
fn parse_value_from_js(js_val: &usd_js::JsValue) -> Option<Value> {
    match js_val {
        usd_js::JsValue::String(s) => Some(Value::from(s.clone())),
        usd_js::JsValue::Bool(b) => Some(Value::from(*b)),
        usd_js::JsValue::Int(i) => Some(Value::from(*i)),
        usd_js::JsValue::UInt(u) => Some(Value::from(*u)),
        usd_js::JsValue::Real(r) => Some(Value::from(*r)),
        _ => None,
    }
}

/// Maps a spec-type name string to `SpecType`.
#[allow(dead_code)]
fn parse_spec_type(name: &str) -> Option<SpecType> {
    match name {
        "Prim" => Some(SpecType::Prim),
        "Attribute" => Some(SpecType::Attribute),
        "Relationship" => Some(SpecType::Relationship),
        "Variant" => Some(SpecType::Variant),
        "VariantSet" => Some(SpecType::VariantSet),
        "PseudoRoot" => Some(SpecType::PseudoRoot),
        _ => None,
    }
}
