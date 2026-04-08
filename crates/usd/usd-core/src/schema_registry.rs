//! Schema Registry - singleton registry for schema type information.
//!
//! Port of pxr/usd/usd/schemaRegistry.h/cpp
//!
//! Provides access to schema type information and prim definitions for
//! registered USD "IsA" and applied API schema types.
//!
//! Supports both static (compile-time) and dynamic (runtime) schema registration.
//! Dynamic schemas are stored in a global RwLock-protected registry alongside
//! any statically-defined defaults.

use super::common::{SchemaKind, SchemaVersion, VersionPolicy};
use super::prim_definition::PrimDefinition;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use usd_gf::Vec4f;
use usd_gf::vec2::Vec2f;
use usd_sdf::{Layer, Path};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// SchemaInfo
// ============================================================================

/// Structure that holds the information about a schema that is registered
/// with the schema registry.
///
/// Matches C++ `UsdSchemaRegistry::SchemaInfo`.
#[derive(Clone)]
pub struct SchemaInfo {
    /// The schema's identifier which is how the schema type is referred to
    /// in scene description and is also the key used to look up the
    /// schema's prim definition.
    pub identifier: Token,
    /// The schema's type name (for now, we use Token instead of TfType).
    /// In full implementation, this would be a TfType.
    pub type_name: String,
    /// The name of the family of schema's which the schema is a version
    /// of. This is the same as the schema identifier with the version
    /// suffix removed (or exactly the same as the schema identifier in the
    /// case of version 0 of a schema which will not have a version suffix.)
    pub family: Token,
    /// The version number of the schema within its schema family.
    pub version: SchemaVersion,
    /// The schema's kind: ConcreteTyped, SingleApplyAPI, etc.
    pub kind: SchemaKind,
    /// Base type names that this schema derives from.
    pub base_type_names: Vec<Token>,
    /// List of prim type names this API schema is auto-applied to.
    pub auto_apply_to: Vec<Token>,
    /// List of prim type names this API schema can only be applied to.
    /// An empty list means it can apply to any type.
    pub can_only_apply_to: Vec<Token>,
    /// For multi-apply API schemas, the allowed instance names.
    /// None means any instance name is allowed (subject to property-name checks).
    pub allowed_instance_names: Option<Vec<Token>>,
    /// Optional prim definition associated with this schema.
    pub prim_definition: Option<Arc<PrimDefinition>>,
}

impl std::fmt::Debug for SchemaInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaInfo")
            .field("identifier", &self.identifier)
            .field("type_name", &self.type_name)
            .field("family", &self.family)
            .field("version", &self.version)
            .field("kind", &self.kind)
            .field("base_type_names", &self.base_type_names)
            .field("auto_apply_to", &self.auto_apply_to)
            .field("can_only_apply_to", &self.can_only_apply_to)
            .field("allowed_instance_names", &self.allowed_instance_names)
            .field("prim_definition", &self.prim_definition.is_some())
            .finish()
    }
}

impl Default for SchemaInfo {
    fn default() -> Self {
        Self {
            identifier: Token::new(""),
            type_name: String::new(),
            family: Token::new(""),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: Vec::new(),
            auto_apply_to: Vec::new(),
            can_only_apply_to: Vec::new(),
            allowed_instance_names: None,
            prim_definition: None,
        }
    }
}

// ============================================================================
// TokenToTokenVectorMap
// ============================================================================

/// Map from Token to Vec<Token>.
///
/// Matches C++ `UsdSchemaRegistry::TokenToTokenVectorMap`.
pub type TokenToTokenVectorMap = HashMap<Token, Vec<Token>>;

// ============================================================================
// DynamicRegistry - single global registry for all schema data
// ============================================================================

/// Internal registry holding both static and dynamically-registered schemas.
///
/// All access goes through `global_registry()` which returns a `&RwLock<..>`.
struct DynamicRegistry {
    /// Owned SchemaInfo entries keyed by identifier.
    schemas: HashMap<Token, SchemaInfo>,
    /// Map from type_name -> identifier for reverse lookup.
    type_name_to_id: HashMap<String, Token>,
    /// Map from family -> list of identifiers (sorted by version desc on read).
    family_to_ids: HashMap<Token, Vec<Token>>,
    /// Aggregated auto-apply map: api_schema_id -> Vec<target_type_names>.
    auto_apply: TokenToTokenVectorMap,
    /// Aggregated can-only-apply map: api_schema_name (or name:instance) -> Vec<target_type_names>.
    can_only_apply: TokenToTokenVectorMap,
    /// Allowed instance names: api_schema_name -> set of allowed instance names.
    allowed_instances: HashMap<Token, Vec<Token>>,
}

impl DynamicRegistry {
    fn new() -> Self {
        Self {
            schemas: HashMap::new(),
            type_name_to_id: HashMap::new(),
            family_to_ids: HashMap::new(),
            auto_apply: TokenToTokenVectorMap::new(),
            can_only_apply: TokenToTokenVectorMap::new(),
            allowed_instances: HashMap::new(),
        }
    }
}

/// Returns a reference to the single global dynamic registry.
fn global_registry() -> &'static RwLock<DynamicRegistry> {
    static REG: OnceLock<RwLock<DynamicRegistry>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(DynamicRegistry::new()))
}

// ============================================================================
// SchemaPropertyRegistry - lightweight type->property-names map
// ============================================================================

struct SchemaPropertyRegistry {
    type_to_props: HashMap<Token, Vec<Token>>,
    type_to_fallbacks: HashMap<Token, HashMap<Token, Value>>,
    /// Maps (prim_type, prop_name) -> SDF type name string (e.g. "color3f", "float").
    /// Used by `Attribute::type_name()` when no spec exists but the schema defines the type.
    type_to_prop_types: HashMap<Token, HashMap<Token, String>>,
    /// Composed variability for registered built-in attributes (wins over local `Sdf` specs).
    type_to_prop_variability: HashMap<Token, HashMap<Token, usd_sdf::Variability>>,
}

impl SchemaPropertyRegistry {
    fn new() -> Self {
        Self {
            type_to_props: HashMap::new(),
            type_to_fallbacks: HashMap::new(),
            type_to_prop_types: HashMap::new(),
            type_to_prop_variability: HashMap::new(),
        }
    }
}

fn global_prop_registry() -> &'static RwLock<SchemaPropertyRegistry> {
    static REG: OnceLock<RwLock<SchemaPropertyRegistry>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(SchemaPropertyRegistry::new()))
}

/// Register property names for a schema type.
pub fn register_schema_properties(type_name: &str, props: &[&str]) {
    let reg = global_prop_registry();
    let mut w = reg.write();
    let key = Token::new(type_name);
    let tokens: Vec<Token> = props.iter().map(|p| Token::new(p)).collect();
    w.type_to_props.insert(key, tokens);
}

/// Register fallback values for a schema type's properties.
pub fn register_schema_fallbacks(type_name: &str, fallbacks: &[(&str, Value)]) {
    let reg = global_prop_registry();
    let mut w = reg.write();
    let key = Token::new(type_name);
    let map: HashMap<Token, Value> = fallbacks
        .iter()
        .map(|(name, val)| (Token::new(name), val.clone()))
        .collect();
    w.type_to_fallbacks.insert(key, map);
}

/// Register SDF type names for a schema type's properties.
/// Maps (prim_type, prop_name) -> sdf_type_name (e.g. "color3f").
pub fn register_schema_property_types(type_name: &str, prop_types: &[(&str, &str)]) {
    let reg = global_prop_registry();
    let mut w = reg.write();
    let key = Token::new(type_name);
    let map: HashMap<Token, String> = prop_types
        .iter()
        .map(|(name, sdf_type)| (Token::new(name), sdf_type.to_string()))
        .collect();
    w.type_to_prop_types.insert(key, map);
}

/// Register composed variability for built-in schema attributes (matches `usdGeom` / `usdLux` schematics).
pub fn register_schema_property_variabilities(
    type_name: &str,
    pairs: &[(&str, usd_sdf::Variability)],
) {
    let reg = global_prop_registry();
    let mut w = reg.write();
    let key = Token::new(type_name);
    let map: HashMap<Token, usd_sdf::Variability> = pairs
        .iter()
        .map(|(name, var)| (Token::new(name), *var))
        .collect();
    w.type_to_prop_variability.insert(key, map);
}

/// Composed variability from the lightweight schema registry, if registered.
pub fn schema_get_property_variability(
    type_name: &Token,
    prop_name: &str,
) -> Option<usd_sdf::Variability> {
    let prop_reg = global_prop_registry();
    let r = prop_reg.read();
    let prop_token = Token::new(prop_name);
    if let Some(vars) = r.type_to_prop_variability.get(type_name) {
        if let Some(v) = vars.get(&prop_token) {
            return Some(*v);
        }
    }
    let dyn_reg = global_registry();
    let dr = dyn_reg.read();
    if let Some(info) = dr.schemas.get(type_name) {
        for base in &info.base_type_names {
            if let Some(vars) = r.type_to_prop_variability.get(base) {
                if let Some(v) = vars.get(&prop_token) {
                    return Some(*v);
                }
            }
        }
    }
    None
}

/// Get the SDF type name for a schema type's property.
/// Returns None if not registered.
pub fn schema_get_property_type(type_name: &Token, prop_name: &str) -> Option<String> {
    let prop_reg = global_prop_registry();
    let r = prop_reg.read();
    let prop_token = Token::new(prop_name);
    if let Some(types) = r.type_to_prop_types.get(type_name) {
        if let Some(sdf_type) = types.get(&prop_token) {
            return Some(sdf_type.clone());
        }
    }
    // Check base types
    let dyn_reg = global_registry();
    let dr = dyn_reg.read();
    if let Some(info) = dr.schemas.get(type_name) {
        for base in &info.base_type_names {
            if let Some(types) = r.type_to_prop_types.get(base) {
                if let Some(sdf_type) = types.get(&prop_token) {
                    return Some(sdf_type.clone());
                }
            }
        }
    }
    None
}

/// Check if a schema type has a given property name as a builtin.
pub fn schema_has_property(type_name: &Token, prop_name: &str) -> bool {
    let prop_reg = global_prop_registry();
    let r = prop_reg.read();
    if let Some(props) = r.type_to_props.get(type_name) {
        if props.iter().any(|t| t.as_str() == prop_name) {
            return true;
        }
    }
    let dyn_reg = global_registry();
    let dr = dyn_reg.read();
    if let Some(info) = dr.schemas.get(type_name) {
        for base in &info.base_type_names {
            if let Some(base_props) = r.type_to_props.get(base) {
                if base_props.iter().any(|t| t.as_str() == prop_name) {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns all schema-defined property names for a given type, including
/// properties inherited from base types registered in the SchemaPropertyRegistry.
/// Used by `Prim::get_properties_in_namespace` when `onlyAuthored=false`.
/// Names registered via [`register_schema_properties`] that are relationships, not attributes.
///
/// Used when `get_defining_spec_type` is `Unknown` (no authored spec yet).
#[inline]
pub fn schema_builtin_relationship_property_name(prop_name: &str) -> bool {
    matches!(prop_name, "proxyPrim")
}

pub fn get_schema_property_names(type_name: &Token) -> Vec<Token> {
    let prop_reg = global_prop_registry();
    let r = prop_reg.read();
    let mut names = Vec::new();

    // Direct properties
    if let Some(props) = r.type_to_props.get(type_name) {
        names.extend(props.iter().cloned());
    }

    // Inherited properties from base types
    let dyn_reg = global_registry();
    let dr = dyn_reg.read();
    if let Some(info) = dr.schemas.get(type_name) {
        for base in &info.base_type_names {
            if let Some(base_props) = r.type_to_props.get(base) {
                for p in base_props {
                    if !names.contains(p) {
                        names.push(p.clone());
                    }
                }
            }
        }
    }
    names
}

/// Check if a multiple-apply API schema instance has a given property name as a builtin.
pub fn schema_instance_has_property(
    schema_type_name: &Token,
    instance_name: &Token,
    prop_name: &str,
) -> bool {
    let prop_reg = global_prop_registry();
    let r = prop_reg.read();

    let has_property_in_templates = |type_name: &Token| -> bool {
        r.type_to_props.get(type_name).is_some_and(|props| {
            props.iter().any(|template| {
                let template_str = template.as_str();
                if SchemaRegistry::is_multiple_apply_name_template(template_str) {
                    SchemaRegistry::make_multiple_apply_name_instance(
                        template_str,
                        instance_name.as_str(),
                    )
                    .as_str()
                        == prop_name
                } else {
                    template_str == prop_name
                }
            })
        })
    };

    if has_property_in_templates(schema_type_name) {
        return true;
    }

    let dyn_reg = global_registry();
    let dr = dyn_reg.read();
    if let Some(info) = dr.schemas.get(schema_type_name) {
        for base in &info.base_type_names {
            if has_property_in_templates(base) {
                return true;
            }
        }
    }

    false
}

/// Get fallback value for a schema type's property.
pub fn schema_get_fallback(type_name: &Token, prop_name: &str) -> Option<Value> {
    let prop_reg = global_prop_registry();
    let r = prop_reg.read();
    let prop_token = Token::new(prop_name);
    if let Some(fallbacks) = r.type_to_fallbacks.get(type_name) {
        if let Some(val) = fallbacks.get(&prop_token) {
            return Some(val.clone());
        }
    }
    let dyn_reg = global_registry();
    let dr = dyn_reg.read();
    if let Some(info) = dr.schemas.get(type_name) {
        for base in &info.base_type_names {
            if let Some(fallbacks) = r.type_to_fallbacks.get(base) {
                if let Some(val) = fallbacks.get(&prop_token) {
                    return Some(val.clone());
                }
            }
        }
    }
    None
}

/// Register a schema in the global dynamic registry (module-level convenience).
///
/// Returns true if the schema was inserted, false if a schema with that
/// identifier was already registered.
pub fn register_schema(info: SchemaInfo) -> bool {
    SchemaRegistry::register_schema_info(info)
}

// ============================================================================
// SchemaRegistry
// ============================================================================

/// Singleton registry that provides access to schema type information.
///
/// Matches C++ `UsdSchemaRegistry`.
///
/// Schemas can be registered at runtime via `register_schema` / `register_schema_info`,
/// and unregistered via `unregister_schema`. All query methods consult the
/// global dynamic registry.
pub struct SchemaRegistry {
    /// Schematics layers (generatedSchema.usda files).
    schematics_layers: Vec<Arc<Layer>>,
    /// Map from concrete typed schema identifier to prim definition.
    concrete_typed_prim_definitions: HashMap<Token, Arc<PrimDefinition>>,
    /// Map from applied API schema identifier to API schema definition info.
    applied_api_prim_definitions: HashMap<Token, ApiSchemaDefinitionInfo>,
    /// Empty prim definition.
    empty_prim_definition: Arc<PrimDefinition>,
    /// Fallback prim types dictionary.
    fallback_prim_types: HashMap<String, Value>,
}

/// Internal structure for API schema definition info.
struct ApiSchemaDefinitionInfo {
    /// The prim definition for this API schema.
    prim_def: Arc<PrimDefinition>,
    /// Whether applying this schema expects an instance name.
    apply_expects_instance_name: bool,
}

impl SchemaRegistry {
    fn ensure_builtin_schemas_registered() {
        register_builtin_schemas();
    }

    /// Returns a reference to the singleton instance.
    ///
    /// Matches C++ `GetInstance()`.
    pub fn get_instance() -> &'static Self {
        static INSTANCE: OnceLock<SchemaRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            Self::ensure_builtin_schemas_registered();
            SchemaRegistry::new()
        })
    }

    /// Creates a new schema registry (private, called by get_instance).
    fn new() -> Self {
        let empty_prim_def = Arc::new(PrimDefinition::new());

        Self {
            schematics_layers: Vec::new(),
            concrete_typed_prim_definitions: HashMap::new(),
            applied_api_prim_definitions: HashMap::new(),
            empty_prim_definition: empty_prim_def,
            fallback_prim_types: HashMap::new(),
        }
    }

    // ========================================================================
    // Dynamic registration / unregistration
    // ========================================================================

    /// Register a schema at runtime.
    ///
    /// Inserts the SchemaInfo into the global dynamic registry. Also updates
    /// auto-apply, can-only-apply, and allowed-instance-names indices.
    ///
    /// Returns `true` if newly inserted, `false` if a schema with the same
    /// identifier already existed (in which case the old entry is replaced).
    pub fn register_schema_info(info: SchemaInfo) -> bool {
        let reg = global_registry();
        let mut w = reg.write();
        let id = info.identifier.clone();
        let is_new = !w.schemas.contains_key(&id);

        // Update type_name -> id reverse map (first registration wins — avoids alias
        // schemas overwriting the canonical `MotionAPI` mapping for `UsdGeomMotionAPI`).
        if !info.type_name.is_empty() {
            w.type_name_to_id
                .entry(info.type_name.clone())
                .or_insert_with(|| id.clone());
        }

        // Update family -> ids map
        let ids = w.family_to_ids.entry(info.family.clone()).or_default();
        if !ids.contains(&id) {
            ids.push(id.clone());
        }

        // Auto-apply index
        if !info.auto_apply_to.is_empty() {
            w.auto_apply.insert(id.clone(), info.auto_apply_to.clone());
        }

        // Can-only-apply index
        if !info.can_only_apply_to.is_empty() {
            w.can_only_apply
                .insert(id.clone(), info.can_only_apply_to.clone());
        }

        // Allowed instance names index
        if let Some(ref names) = info.allowed_instance_names {
            w.allowed_instances.insert(id.clone(), names.clone());
        }

        w.schemas.insert(id, info);
        is_new
    }

    /// Unregister a schema by its identifier.
    ///
    /// Returns `true` if a schema was found and removed.
    pub fn unregister_schema(type_name: &str) -> bool {
        let reg = global_registry();
        let mut w = reg.write();
        let id = Token::new(type_name);
        if let Some(info) = w.schemas.remove(&id) {
            // Clean up reverse maps
            if !info.type_name.is_empty() {
                w.type_name_to_id.remove(&info.type_name);
            }
            if let Some(ids) = w.family_to_ids.get_mut(&info.family) {
                ids.retain(|x| *x != id);
                if ids.is_empty() {
                    w.family_to_ids.remove(&info.family);
                }
            }
            w.auto_apply.remove(&id);
            w.can_only_apply.remove(&id);
            w.allowed_instances.remove(&id);
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Identifier / family / version helpers (pure, no registry access)
    // ========================================================================

    /// Creates the schema identifier that would be used to define a schema of
    /// the given schemaFamily with the given schemaVersion.
    ///
    /// If the provided schema version is zero, the returned identifier will
    /// be the schema family itself. For all other versions, the returned
    /// identifier will be the family followed by an underscore and the version
    /// number.
    ///
    /// Matches C++ `MakeSchemaIdentifierForFamilyAndVersion()`.
    pub fn make_schema_identifier_for_family_and_version(
        schema_family: &Token,
        schema_version: SchemaVersion,
    ) -> Token {
        if schema_version == 0 {
            return schema_family.clone();
        }
        let id_str = format!("{}_{}", schema_family.as_str(), schema_version);
        Token::new(&id_str)
    }

    /// Parses and returns the schema family and version values from the given
    /// schemaIdentifier.
    ///
    /// Matches C++ `ParseSchemaFamilyAndVersionFromIdentifier()`.
    pub fn parse_schema_family_and_version_from_identifier(
        schema_identifier: &Token,
    ) -> (Token, SchemaVersion) {
        let id_str = schema_identifier.as_str();

        let find_version_delimiter = |s: &str| -> Option<usize> {
            if s.len() < 2 {
                return None;
            }
            let bytes = s.as_bytes();
            let mut delim = s.len() - 1;
            if !bytes[delim].is_ascii_digit() {
                return None;
            }
            while delim > 0 {
                delim -= 1;
                if bytes[delim] == b'_' {
                    return Some(delim);
                }
                if !bytes[delim].is_ascii_digit() {
                    return None;
                }
            }
            None
        };

        if let Some(delim) = find_version_delimiter(id_str) {
            let family = Token::new(&id_str[..delim]);
            let version_str = &id_str[delim + 1..];
            if let Ok(version) = version_str.parse::<SchemaVersion>() {
                return (family, version);
            }
        }
        (schema_identifier.clone(), 0)
    }

    /// Returns whether the given schemaFamily is an allowed schema family name.
    ///
    /// Matches C++ `IsAllowedSchemaFamily()`.
    pub fn is_allowed_schema_family(schema_family: &Token) -> bool {
        if !Path::is_valid_identifier(schema_family.as_str()) {
            return false;
        }
        let (_, version) = Self::parse_schema_family_and_version_from_identifier(schema_family);
        version == 0
            && schema_family.as_str()
                == Self::make_schema_identifier_for_family_and_version(schema_family, 0).as_str()
    }

    /// Returns whether the given schemaIdentifier is an allowed schema identifier.
    ///
    /// Matches C++ `IsAllowedSchemaIdentifier()`.
    pub fn is_allowed_schema_identifier(schema_identifier: &Token) -> bool {
        let (family, version) =
            Self::parse_schema_family_and_version_from_identifier(schema_identifier);
        Self::is_allowed_schema_family(&family)
            && Self::make_schema_identifier_for_family_and_version(&family, version)
                == *schema_identifier
    }

    // ========================================================================
    // Schema info lookups (read from the global registry)
    // ========================================================================

    /// Finds and returns the schema info for a registered schema with the
    /// given schemaIdentifier.
    ///
    /// Matches C++ `FindSchemaInfo(const TfToken &schemaIdentifier)`.
    pub fn find_schema_info(schema_identifier: &Token) -> Option<SchemaInfo> {
        Self::ensure_builtin_schemas_registered();
        let reg = global_registry();
        let r = reg.read();
        r.schemas.get(schema_identifier).cloned()
    }

    /// Resolve a schema by **identifier** (e.g. `"Mesh"`) or **C++ type name** (e.g. `"UsdGeomMesh"`).
    ///
    /// Matches lookups used by Python `Tf.Type` / `UsdPrim::IsA` parity paths.
    pub fn find_schema_info_for_type_string(name: &str) -> Option<SchemaInfo> {
        Self::ensure_builtin_schemas_registered();
        if let Some(info) = Self::find_schema_info(&Token::new(name)) {
            return Some(info);
        }
        let reg = global_registry();
        let r = reg.read();
        r.type_name_to_id
            .get(name)
            .and_then(|id| r.schemas.get(id).cloned())
    }

    /// True if `prim_identifier`'s schema inherits `query` (identifier or C++ type name).
    ///
    /// Walks the registered schema `base_type_names` graph (breadth-first), comparing
    /// [`SchemaInfo::type_name`] against the resolved target schema's `type_name`.
    pub fn prim_type_is_a_query(prim_identifier: &Token, query: &str) -> bool {
        if prim_identifier.is_empty() {
            return false;
        }
        let Some(target) = Self::find_schema_info_for_type_string(query) else {
            return false;
        };
        let target_cxx = target.type_name;
        let mut stack = vec![prim_identifier.clone()];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Some(info) = Self::find_schema_info(&id) else {
                continue;
            };
            if info.type_name == target_cxx {
                return true;
            }
            for b in &info.base_type_names {
                stack.push(b.clone());
            }
        }
        false
    }

    /// Finds and returns the schema info for a registered schema in the
    /// given schemaFamily with the given schemaVersion.
    ///
    /// Matches C++ `FindSchemaInfo(const TfToken &schemaFamily, UsdSchemaVersion schemaVersion)`.
    pub fn find_schema_info_by_family_and_version(
        schema_family: &Token,
        schema_version: SchemaVersion,
    ) -> Option<SchemaInfo> {
        if !Self::is_allowed_schema_family(schema_family) {
            return None;
        }
        let identifier =
            Self::make_schema_identifier_for_family_and_version(schema_family, schema_version);
        Self::find_schema_info(&identifier)
    }

    /// Finds all schemas in the given schemaFamily and returns their
    /// schema info ordered from highest version to lowest version.
    ///
    /// Matches C++ `FindSchemaInfosInFamily(const TfToken &schemaFamily)`.
    pub fn find_schema_infos_in_family(schema_family: &Token) -> Vec<SchemaInfo> {
        Self::ensure_builtin_schemas_registered();
        let reg = global_registry();
        let r = reg.read();
        if let Some(ids) = r.family_to_ids.get(schema_family) {
            let mut infos: Vec<SchemaInfo> = ids
                .iter()
                .filter_map(|id| r.schemas.get(id).cloned())
                .collect();
            infos.sort_by(|a, b| b.version.cmp(&a.version));
            infos
        } else {
            Vec::new()
        }
    }

    /// Finds all schemas in the given schemaFamily, filtered by version policy.
    ///
    /// Matches C++ `FindSchemaInfosInFamily(const TfToken &, UsdSchemaVersion, VersionPolicy)`.
    pub fn find_schema_infos_in_family_filtered(
        schema_family: &Token,
        schema_version: SchemaVersion,
        version_policy: VersionPolicy,
    ) -> Vec<SchemaInfo> {
        let all_infos = Self::find_schema_infos_in_family(schema_family);
        match version_policy {
            VersionPolicy::All => all_infos,
            VersionPolicy::GreaterThan => all_infos
                .into_iter()
                .filter(|info| info.version > schema_version)
                .collect(),
            VersionPolicy::GreaterThanOrEqual => all_infos
                .into_iter()
                .filter(|info| info.version >= schema_version)
                .collect(),
            VersionPolicy::LessThan => all_infos
                .into_iter()
                .filter(|info| info.version < schema_version)
                .collect(),
            VersionPolicy::LessThanOrEqual => all_infos
                .into_iter()
                .filter(|info| info.version <= schema_version)
                .collect(),
        }
    }

    /// Return the type name in the USD schema for prims or API schemas of the
    /// given registered schemaType.
    ///
    /// Matches C++ `GetSchemaTypeName(const TfType &schemaType)`.
    pub fn get_schema_type_name(schema_type: &str) -> Token {
        Self::ensure_builtin_schemas_registered();
        if let Some(info) = Self::find_schema_info(&Token::new(schema_type)) {
            return info.identifier;
        }
        let reg = global_registry();
        let r = reg.read();
        if let Some(id) = r.type_name_to_id.get(schema_type) {
            return id.clone();
        }
        Token::new("")
    }

    /// Return the type name in the USD schema for concrete prim types only.
    ///
    /// Matches C++ `GetConcreteSchemaTypeName(const TfType &schemaType)`.
    pub fn get_concrete_schema_type_name(schema_type: &str) -> Token {
        if let Some(info) = Self::find_schema_info(&Token::new(schema_type)) {
            if info.kind == SchemaKind::ConcreteTyped {
                return info.identifier.clone();
            }
        }
        Token::new("")
    }

    /// Return the type name in the USD schema for API schema types only.
    ///
    /// Matches C++ `GetAPISchemaTypeName(const TfType &schemaType)`.
    pub fn get_api_schema_type_name(schema_type: &str) -> Token {
        if let Some(info) = Self::find_schema_info(&Token::new(schema_type)) {
            if Self::is_api_schema_kind(info.kind) {
                return info.identifier.clone();
            }
        }
        Token::new("")
    }

    /// Return the schema type name from the given prim or API schema name.
    ///
    /// Matches C++ `GetTypeFromSchemaTypeName(const TfToken &typeName)`.
    pub fn get_type_from_schema_type_name(type_name: &Token) -> String {
        if let Some(info) = Self::find_schema_info(type_name) {
            return info.type_name.clone();
        }
        String::new()
    }

    /// Return the schema type name from the given concrete prim type name.
    ///
    /// Matches C++ `GetConcreteTypeFromSchemaTypeName(const TfToken &typeName)`.
    pub fn get_concrete_type_from_schema_type_name(type_name: &Token) -> String {
        if let Some(info) = Self::find_schema_info(type_name) {
            if info.kind == SchemaKind::ConcreteTyped {
                return info.type_name.clone();
            }
        }
        String::new()
    }

    /// Return the schema type name from the given API schema type name.
    ///
    /// Matches C++ `GetAPITypeFromSchemaTypeName(const TfToken &typeName)`.
    pub fn get_api_type_from_schema_type_name(type_name: &Token) -> String {
        if let Some(info) = Self::find_schema_info(type_name) {
            if Self::is_api_schema_kind(info.kind) {
                return info.type_name.clone();
            }
        }
        String::new()
    }

    /// Returns true if the field cannot have fallback values specified in schemas.
    ///
    /// Matches C++ `IsDisallowedField()`.
    pub fn is_disallowed_field(field_name: &Token) -> bool {
        let field_str = field_name.as_str();

        // Composition arc fields
        if matches!(
            field_str,
            "inheritPaths"
                | "payload"
                | "references"
                | "specializes"
                | "variantSelection"
                | "variantSetNames"
        ) {
            return true;
        }

        // customData
        if field_str == "customData" {
            return true;
        }

        // Fields not used during scenegraph population or value resolution
        if matches!(
            field_str,
            "active"
                | "instanceable"
                | "timeSamples"
                | "spline"
                | "connectionPaths"
                | "targetPaths"
        ) {
            return true;
        }

        // specifier
        if field_str == "specifier" {
            return true;
        }

        // Children fields
        if matches!(
            field_str,
            "properties" | "variantChildren" | "relocates" | "primChildren"
        ) {
            return true;
        }

        // kind
        if field_str == "kind" {
            return true;
        }

        false
    }

    /// Returns true if the prim type inherits from UsdTyped.
    ///
    /// Matches C++ `IsTyped(const TfType& primType)`.
    pub fn is_typed(prim_type: &str) -> bool {
        matches!(
            Self::get_schema_kind(prim_type),
            SchemaKind::ConcreteTyped | SchemaKind::AbstractTyped
        )
    }

    /// Returns the kind of the schema the given schemaType represents.
    ///
    /// Matches C++ `GetSchemaKind(const TfType &schemaType)`.
    pub fn get_schema_kind(schema_type: &str) -> SchemaKind {
        if let Some(info) = Self::find_schema_info_for_type_string(schema_type) {
            return info.kind;
        }
        SchemaKind::Invalid
    }

    /// Returns the kind of the schema the given typeName represents.
    ///
    /// Matches C++ `GetSchemaKind(const TfToken &typeName)`.
    pub fn get_schema_kind_from_name(type_name: &Token) -> SchemaKind {
        if let Some(info) = Self::find_schema_info(type_name) {
            return info.kind;
        }
        SchemaKind::Invalid
    }

    /// Returns true if the prim type is instantiable in scene description.
    pub fn is_concrete(prim_type: &str) -> bool {
        Self::is_concrete_schema_kind(Self::get_schema_kind(prim_type))
    }

    /// Returns true if the prim type is instantiable in scene description (by Token).
    pub fn is_concrete_by_name(prim_type: &Token) -> bool {
        Self::is_concrete_schema_kind(Self::get_schema_kind_from_name(prim_type))
    }

    /// Returns true if the prim type is an abstract schema type.
    pub fn is_abstract(prim_type: &str) -> bool {
        Self::is_abstract_schema_kind(Self::get_schema_kind(prim_type))
    }

    /// Returns true if the prim type is an abstract schema type (by Token).
    pub fn is_abstract_by_name(prim_type: &Token) -> bool {
        Self::is_abstract_schema_kind(Self::get_schema_kind_from_name(prim_type))
    }

    /// Returns true if the API schema type is an applied API schema.
    pub fn is_applied_api_schema(api_schema_type: &str) -> bool {
        Self::is_applied_api_schema_kind(Self::get_schema_kind(api_schema_type))
    }

    /// Returns true if the API schema type is an applied API schema (by Token).
    pub fn is_applied_api_schema_by_name(api_schema_type: &Token) -> bool {
        Self::is_applied_api_schema_kind(Self::get_schema_kind_from_name(api_schema_type))
    }

    /// Returns true if the API schema type is a multiple-apply API schema.
    pub fn is_multiple_apply_api_schema(api_schema_type: &str) -> bool {
        Self::is_multiple_apply_schema_kind(Self::get_schema_kind(api_schema_type))
    }

    /// Returns true if the API schema type is a multiple-apply API schema (by Token).
    pub fn is_multiple_apply_api_schema_by_name(api_schema_type: &Token) -> bool {
        Self::is_multiple_apply_schema_kind(Self::get_schema_kind_from_name(api_schema_type))
    }

    /// Finds the schema type name from the given typeName.
    ///
    /// Matches C++ `GetTypeFromName(const TfToken& typeName)`.
    pub fn get_type_from_name(type_name: &Token) -> String {
        Self::ensure_builtin_schemas_registered();
        // Try by identifier first
        if let Some(info) = Self::find_schema_info(type_name) {
            return info.type_name.clone();
        }
        // Then try by type_name string
        let reg = global_registry();
        let r = reg.read();
        if let Some(id) = r.type_name_to_id.get(type_name.as_str()) {
            if let Some(info) = r.schemas.get(id) {
                return info.type_name.clone();
            }
        }
        String::new()
    }

    /// Returns the schema type name and the instance name parsed from the given
    /// apiSchemaName.
    ///
    /// Matches C++ `GetTypeNameAndInstance()`.
    pub fn get_type_name_and_instance(api_schema_name: &Token) -> (Token, Token) {
        let name_str = api_schema_name.as_str();
        if let Some(delim_pos) = name_str.find(':') {
            let type_name = Token::new(&name_str[..delim_pos]);
            let instance_name = Token::new(&name_str[delim_pos + 1..]);
            (type_name, instance_name)
        } else {
            (api_schema_name.clone(), Token::new(""))
        }
    }

    /// Returns true if the given instanceName is an allowed instance name
    /// for the multiple apply API schema named apiSchemaName.
    ///
    /// Matches C++ `IsAllowedAPISchemaInstanceName()`.
    pub fn is_allowed_api_schema_instance_name(
        api_schema_name: &Token,
        instance_name: &Token,
    ) -> bool {
        Self::ensure_builtin_schemas_registered();
        if instance_name.is_empty() || !Self::is_multiple_apply_api_schema_by_name(api_schema_name)
        {
            return false;
        }

        let reg = global_registry();
        let r = reg.read();

        // If the schema has an explicit allowed-instance-names list,
        // the instance name must appear in it.
        if let Some(allowed) = r.allowed_instances.get(api_schema_name) {
            return allowed.contains(instance_name);
        }

        // No explicit restriction -> allowed (basic validation)
        true
    }

    /// Returns a list of prim type names that the given apiSchemaName can
    /// only be applied to.
    ///
    /// Matches C++ `GetAPISchemaCanOnlyApplyToTypeNames()`.
    pub fn get_api_schema_can_only_apply_to_type_names(
        api_schema_name: &Token,
        instance_name: &Token,
    ) -> Vec<Token> {
        Self::ensure_builtin_schemas_registered();
        let reg = global_registry();
        let r = reg.read();

        // If instanceName is not empty, try full name first
        if !instance_name.is_empty() {
            let full = Token::new(&format!(
                "{}:{}",
                api_schema_name.as_str(),
                instance_name.as_str()
            ));
            if let Some(v) = r.can_only_apply.get(&full) {
                return v.clone();
            }
        }

        // Fall back to the base schema name
        if let Some(v) = r.can_only_apply.get(api_schema_name) {
            return v.clone();
        }

        Vec::new()
    }

    /// Returns a map of the names of all registered auto apply API schemas
    /// to the list of type names each is registered to be auto applied to.
    ///
    /// Matches C++ `GetAutoApplyAPISchemas()`.
    pub fn get_auto_apply_api_schemas() -> HashMap<Token, Vec<Token>> {
        Self::ensure_builtin_schemas_registered();
        let reg = global_registry();
        let r = reg.read();
        r.auto_apply.clone()
    }

    /// Collects additional auto apply schemas from plugins (no-op in Rust).
    ///
    /// Matches C++ `CollectAddtionalAutoApplyAPISchemasFromPlugins()`.
    pub fn collect_additional_auto_apply_api_schemas_from_plugins(
        _auto_apply_api_schemas: &mut TokenToTokenVectorMap,
    ) {
        // No-op: schemas are registered directly, not via plugins
    }

    // ========================================================================
    // Multiple-apply name template helpers (pure, no registry access)
    // ========================================================================

    /// Creates a name template for a multiple apply schema property.
    ///
    /// Matches C++ `MakeMultipleApplyNameTemplate()`.
    pub fn make_multiple_apply_name_template(namespace_prefix: &str, base_name: &str) -> Token {
        const PH: &str = "__INSTANCE_NAME__";
        let mut result = String::new();
        if !namespace_prefix.is_empty() {
            result.push_str(namespace_prefix);
            result.push(':');
        }
        result.push_str(PH);
        if !base_name.is_empty() {
            result.push(':');
            result.push_str(base_name);
        }
        Token::new(&result)
    }

    /// Returns an instance of a multiple apply schema name from the given
    /// nameTemplate for the given instanceName.
    ///
    /// Matches C++ `MakeMultipleApplyNameInstance()`.
    pub fn make_multiple_apply_name_instance(name_template: &str, instance_name: &str) -> Token {
        const PH: &str = "__INSTANCE_NAME__";

        let find_placeholder = |s: &str| -> Option<usize> {
            let ph_len = PH.len();
            let mut start = 0;
            while start < s.len() {
                let end = s[start..].find(':').map(|i| start + i).unwrap_or(s.len());
                if end - start == ph_len && &s[start..end] == PH {
                    return Some(start);
                }
                start = end + 1;
            }
            None
        };

        if let Some(pos) = find_placeholder(name_template) {
            let mut result = name_template.to_string();
            result.replace_range(pos..pos + PH.len(), instance_name);
            Token::new(&result)
        } else {
            Token::new(name_template)
        }
    }

    /// Returns the base name for the multiple apply schema name template.
    ///
    /// Matches C++ `GetMultipleApplyNameTemplateBaseName()`.
    pub fn get_multiple_apply_name_template_base_name(name_template: &str) -> Token {
        const PH: &str = "__INSTANCE_NAME__";

        let find_placeholder = |s: &str| -> Option<usize> {
            let ph_len = PH.len();
            let mut start = 0;
            while start < s.len() {
                let end = s[start..].find(':').map(|i| start + i).unwrap_or(s.len());
                if end - start == ph_len && &s[start..end] == PH {
                    return Some(start);
                }
                start = end + 1;
            }
            None
        };

        if let Some(pos) = find_placeholder(name_template) {
            let base_pos = pos + PH.len() + 1;
            if base_pos >= name_template.len() {
                return Token::new("");
            }
            Token::new(&name_template[base_pos..])
        } else {
            Token::new(name_template)
        }
    }

    /// Returns true if nameTemplate is a multiple apply schema name template.
    ///
    /// Matches C++ `IsMultipleApplyNameTemplate()`.
    pub fn is_multiple_apply_name_template(name_template: &str) -> bool {
        const PH: &str = "__INSTANCE_NAME__";
        let ph_len = PH.len();
        let mut start = 0;
        while start < name_template.len() {
            let end = name_template[start..]
                .find(':')
                .map(|i| start + i)
                .unwrap_or(name_template.len());
            if end - start == ph_len && &name_template[start..end] == PH {
                return true;
            }
            start = end + 1;
        }
        false
    }

    // ========================================================================
    // PrimDefinition access (on the singleton instance)
    // ========================================================================

    /// Finds the prim definition for the given concrete typed schema.
    ///
    /// Matches C++ `FindConcretePrimDefinition(const TfToken &typeName)`.
    pub fn find_concrete_prim_definition(&self, type_name: &Token) -> Option<Arc<PrimDefinition>> {
        self.concrete_typed_prim_definitions.get(type_name).cloned()
    }

    /// Finds the prim definition for the given applied API schema.
    ///
    /// Matches C++ `FindAppliedAPIPrimDefinition(const TfToken &typeName)`.
    pub fn find_applied_api_prim_definition(
        &self,
        type_name: &Token,
    ) -> Option<Arc<PrimDefinition>> {
        self.applied_api_prim_definitions
            .get(type_name)
            .map(|info| info.prim_def.clone())
    }

    /// Returns the empty prim definition.
    ///
    /// Matches C++ `GetEmptyPrimDefinition()`.
    pub fn get_empty_prim_definition(&self) -> Arc<PrimDefinition> {
        self.empty_prim_definition.clone()
    }

    /// Registers a concrete typed schema prim definition.
    pub fn register_typed_prim_definition(
        &mut self,
        type_name: Token,
        schematics_layer: Arc<Layer>,
        schematics_prim_path: Path,
        properties_to_ignore: &[Token],
    ) -> bool {
        let mut prim_def = PrimDefinition::new();
        if prim_def.initialize_for_typed_schema(
            schematics_layer,
            schematics_prim_path,
            properties_to_ignore,
        ) {
            self.concrete_typed_prim_definitions
                .insert(type_name, Arc::new(prim_def));
            true
        } else {
            false
        }
    }

    /// Registers an applied API schema prim definition.
    pub fn register_api_prim_definition(
        &mut self,
        api_schema_name: Token,
        schematics_layer: Arc<Layer>,
        schematics_prim_path: Path,
        properties_to_ignore: &[Token],
    ) -> bool {
        let mut prim_def = PrimDefinition::new();
        if prim_def.initialize_for_api_schema(
            api_schema_name.clone(),
            schematics_layer,
            schematics_prim_path,
            properties_to_ignore,
        ) {
            let info = ApiSchemaDefinitionInfo {
                prim_def: Arc::new(prim_def),
                apply_expects_instance_name: false,
            };
            self.applied_api_prim_definitions
                .insert(api_schema_name, info);
            true
        } else {
            false
        }
    }

    /// Composes and returns a new PrimDefinition from the given primType
    /// and list of appliedSchemas.
    ///
    /// Matches C++ `BuildComposedPrimDefinition()`.
    pub fn build_composed_prim_definition(
        &self,
        prim_type: &Token,
        applied_api_schemas: &[Token],
    ) -> Option<Arc<PrimDefinition>> {
        if applied_api_schemas.is_empty() {
            return None;
        }
        let prim_def = self.find_concrete_prim_definition(prim_type);
        let composed = prim_def.unwrap_or_else(|| self.empty_prim_definition.clone());
        Some(composed)
    }

    /// Returns fallback prim types dictionary.
    ///
    /// Matches C++ `GetFallbackPrimTypes()`.
    pub fn get_fallback_prim_types(&self) -> &HashMap<String, Value> {
        &self.fallback_prim_types
    }

    /// Returns the list of schematics layers.
    pub fn get_schematics_layers(&self) -> &[Arc<Layer>] {
        &self.schematics_layers
    }

    /// Adds a schematics layer to the registry.
    pub fn add_schematics_layer(&mut self, layer: Arc<Layer>) {
        self.schematics_layers.push(layer);
    }

    /// Checks whether applying the given API schema expects an instance name.
    pub fn api_schema_expects_instance_name(&self, api_schema_name: &Token) -> bool {
        self.applied_api_prim_definitions
            .get(api_schema_name)
            .map(|info| info.apply_expects_instance_name)
            .unwrap_or(false)
    }

    // ========================================================================
    // Helper functions
    // ========================================================================

    /// Helper: Returns true if schema kind is concrete.
    fn is_concrete_schema_kind(kind: SchemaKind) -> bool {
        kind == SchemaKind::ConcreteTyped
    }

    /// Helper: Returns true if schema kind is abstract.
    fn is_abstract_schema_kind(kind: SchemaKind) -> bool {
        kind == SchemaKind::AbstractTyped || kind == SchemaKind::AbstractBase
    }

    /// Helper: Returns true if schema kind is applied API schema.
    fn is_applied_api_schema_kind(kind: SchemaKind) -> bool {
        kind == SchemaKind::SingleApplyAPI || kind == SchemaKind::MultipleApplyAPI
    }

    /// Helper: Returns true if schema kind is API schema.
    fn is_api_schema_kind(kind: SchemaKind) -> bool {
        kind == SchemaKind::SingleApplyAPI
            || kind == SchemaKind::MultipleApplyAPI
            || kind == SchemaKind::NonAppliedAPI
    }

    /// Helper: Returns true if schema kind is multiple apply.
    fn is_multiple_apply_schema_kind(kind: SchemaKind) -> bool {
        kind == SchemaKind::MultipleApplyAPI
    }
}

// ============================================================================
// Builtin Schema Registration
// ============================================================================

/// Registers all builtin USD schema types with their full hierarchy.
///
/// This function is called automatically when the SchemaRegistry singleton
/// is first accessed. It uses a Once lock to ensure registration happens
/// exactly once.
pub fn register_builtin_schemas() {
    usd_trace::trace_scope!("register_builtin_schemas");
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let _t0 = std::time::Instant::now();
        // Helper function to create base chain
        fn bases(types: &[&str]) -> Vec<Token> {
            types.iter().map(|s| Token::new(s)).collect()
        }

        // ====================================================================
        // ABSTRACT_TYPED
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("Typed"),
            type_name: "UsdTyped".to_string(),
            family: Token::new("Typed"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: vec![],
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Imageable"),
            type_name: "UsdGeomImageable".to_string(),
            family: Token::new("Imageable"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Xformable"),
            type_name: "UsdGeomXformable".to_string(),
            family: Token::new("Xformable"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Boundable"),
            type_name: "UsdGeomBoundable".to_string(),
            family: Token::new("Boundable"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Gprim"),
            type_name: "UsdGeomGprim".to_string(),
            family: Token::new("Gprim"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("PointBased"),
            type_name: "UsdGeomPointBased".to_string(),
            family: Token::new("PointBased"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("BoundableLightBase"),
            type_name: "UsdLuxBoundableLightBase".to_string(),
            family: Token::new("BoundableLightBase"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("NonboundableLightBase"),
            type_name: "UsdLuxNonboundableLightBase".to_string(),
            family: Token::new("NonboundableLightBase"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RenderSettingsBase"),
            type_name: "UsdRenderRenderSettingsBase".to_string(),
            family: Token::new("RenderSettingsBase"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // CONCRETE_TYPED - UsdGeom
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("Mesh"),
            type_name: "UsdGeomMesh".to_string(),
            family: Token::new("Mesh"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Points"),
            type_name: "UsdGeomPoints".to_string(),
            family: Token::new("Points"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Cube"),
            type_name: "UsdGeomCube".to_string(),
            family: Token::new("Cube"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Sphere"),
            type_name: "UsdGeomSphere".to_string(),
            family: Token::new("Sphere"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Cylinder"),
            type_name: "UsdGeomCylinder".to_string(),
            family: Token::new("Cylinder"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Cylinder_1"),
            type_name: "UsdGeomCylinder_1".to_string(),
            family: Token::new("Cylinder"),
            version: 1,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Cone"),
            type_name: "UsdGeomCone".to_string(),
            family: Token::new("Cone"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Capsule"),
            type_name: "UsdGeomCapsule".to_string(),
            family: Token::new("Capsule"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Capsule_1"),
            type_name: "UsdGeomCapsule_1".to_string(),
            family: Token::new("Capsule"),
            version: 1,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Plane"),
            type_name: "UsdGeomPlane".to_string(),
            family: Token::new("Plane"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("BasisCurves"),
            type_name: "UsdGeomBasisCurves".to_string(),
            family: Token::new("BasisCurves"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Curves"),
            type_name: "UsdGeomCurves".to_string(),
            family: Token::new("Curves"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("HermiteCurves"),
            type_name: "UsdGeomHermiteCurves".to_string(),
            family: Token::new("HermiteCurves"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("NurbsCurves"),
            type_name: "UsdGeomNurbsCurves".to_string(),
            family: Token::new("NurbsCurves"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("NurbsPatch"),
            type_name: "UsdGeomNurbsPatch".to_string(),
            family: Token::new("NurbsPatch"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Gprim", "Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("TetMesh"),
            type_name: "UsdGeomTetMesh".to_string(),
            family: Token::new("TetMesh"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "PointBased",
                "Gprim",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Camera"),
            type_name: "UsdGeomCamera".to_string(),
            family: Token::new("Camera"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Xform"),
            type_name: "UsdGeomXform".to_string(),
            family: Token::new("Xform"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Scope"),
            type_name: "UsdGeomScope".to_string(),
            family: Token::new("Scope"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("PointInstancer"),
            type_name: "UsdGeomPointInstancer".to_string(),
            family: Token::new("PointInstancer"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("GeomSubset"),
            type_name: "UsdGeomSubset".to_string(),
            family: Token::new("GeomSubset"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // CONCRETE_TYPED - UsdLux
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("SphereLight"),
            type_name: "UsdLuxSphereLight".to_string(),
            family: Token::new("SphereLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "BoundableLightBase",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RectLight"),
            type_name: "UsdLuxRectLight".to_string(),
            family: Token::new("RectLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "BoundableLightBase",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("DiskLight"),
            type_name: "UsdLuxDiskLight".to_string(),
            family: Token::new("DiskLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "BoundableLightBase",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("CylinderLight"),
            type_name: "UsdLuxCylinderLight".to_string(),
            family: Token::new("CylinderLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "BoundableLightBase",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("DistantLight"),
            type_name: "UsdLuxDistantLight".to_string(),
            family: Token::new("DistantLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["NonboundableLightBase", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("DomeLight"),
            type_name: "UsdLuxDomeLight".to_string(),
            family: Token::new("DomeLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["NonboundableLightBase", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("DomeLight_1"),
            type_name: "UsdLuxDomeLight_1".to_string(),
            family: Token::new("DomeLight"),
            version: 1,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["NonboundableLightBase", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("GeometryLight"),
            type_name: "UsdLuxGeometryLight".to_string(),
            family: Token::new("GeometryLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "BoundableLightBase",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("LightFilter"),
            type_name: "UsdLuxLightFilter".to_string(),
            family: Token::new("LightFilter"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("PortalLight"),
            type_name: "UsdLuxPortalLight".to_string(),
            family: Token::new("PortalLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["NonboundableLightBase", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("PluginLight"),
            type_name: "UsdLuxPluginLight".to_string(),
            family: Token::new("PluginLight"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&[
                "BoundableLightBase",
                "Boundable",
                "Xformable",
                "Imageable",
                "Typed",
            ]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("PluginLightFilter"),
            type_name: "UsdLuxPluginLightFilter".to_string(),
            family: Token::new("PluginLightFilter"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // CONCRETE_TYPED - UsdSkel
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("Skeleton"),
            type_name: "UsdSkelSkeleton".to_string(),
            family: Token::new("Skeleton"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("SkelRoot"),
            type_name: "UsdSkelRoot".to_string(),
            family: Token::new("SkelRoot"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("SkelAnimation"),
            type_name: "UsdSkelAnimation".to_string(),
            family: Token::new("SkelAnimation"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("BlendShape"),
            type_name: "UsdSkelBlendShape".to_string(),
            family: Token::new("BlendShape"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // CONCRETE_TYPED - UsdShade
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("Shader"),
            type_name: "UsdShadeShader".to_string(),
            family: Token::new("Shader"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("NodeGraph"),
            type_name: "UsdShadeNodeGraph".to_string(),
            family: Token::new("NodeGraph"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Material"),
            type_name: "UsdShadeMaterial".to_string(),
            family: Token::new("Material"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["NodeGraph", "Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // CONCRETE_TYPED - UsdPhysics
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("Joint"),
            type_name: "UsdPhysicsJoint".to_string(),
            family: Token::new("Joint"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("FixedJoint"),
            type_name: "UsdPhysicsFixedJoint".to_string(),
            family: Token::new("FixedJoint"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Joint", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("PrismaticJoint"),
            type_name: "UsdPhysicsPrismaticJoint".to_string(),
            family: Token::new("PrismaticJoint"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Joint", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RevoluteJoint"),
            type_name: "UsdPhysicsRevoluteJoint".to_string(),
            family: Token::new("RevoluteJoint"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Joint", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("SphericalJoint"),
            type_name: "UsdPhysicsSphericalJoint".to_string(),
            family: Token::new("SphericalJoint"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Joint", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("DistanceJoint"),
            type_name: "UsdPhysicsDistanceJoint".to_string(),
            family: Token::new("DistanceJoint"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Joint", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("CollisionGroup"),
            type_name: "UsdPhysicsCollisionGroup".to_string(),
            family: Token::new("CollisionGroup"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Scene"),
            type_name: "UsdPhysicsScene".to_string(),
            family: Token::new("Scene"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // CONCRETE_TYPED - Others
        // ====================================================================

        register_schema(SchemaInfo {
            identifier: Token::new("SpatialAudio"),
            type_name: "UsdMediaSpatialAudio".to_string(),
            family: Token::new("SpatialAudio"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RenderPass"),
            type_name: "UsdRenderPass".to_string(),
            family: Token::new("RenderPass"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RenderProduct"),
            type_name: "UsdRenderProduct".to_string(),
            family: Token::new("RenderProduct"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["RenderSettingsBase", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RenderSettings"),
            type_name: "UsdRenderSettings".to_string(),
            family: Token::new("RenderSettings"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["RenderSettingsBase", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("RenderVar"),
            type_name: "UsdRenderVar".to_string(),
            family: Token::new("RenderVar"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("GenerativeProcedural"),
            type_name: "UsdGeomGenerativeProcedural".to_string(),
            family: Token::new("GenerativeProcedural"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        register_schema(SchemaInfo {
            identifier: Token::new("Volume"),
            type_name: "UsdVolVolume".to_string(),
            family: Token::new("Volume"),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: bases(&["Boundable", "Xformable", "Imageable", "Typed"]),
            ..Default::default()
        });

        // ====================================================================
        // SINGLE_APPLY_API
        // ====================================================================

        let single_apis = [
            ("ModelAPI", "UsdModelAPI"),
            ("GeomModelAPI", "UsdGeomModelAPI"),
            ("MotionAPI", "UsdGeomMotionAPI"),
            ("VisibilityAPI", "UsdGeomVisibilityAPI"),
            ("PrimvarsAPI", "UsdGeomPrimvarsAPI"),
            ("LightAPI", "UsdLuxLightAPI"),
            ("LightListAPI", "UsdLuxLightListAPI"),
            ("MeshLightAPI", "UsdLuxMeshLightAPI"),
            ("ShadowAPI", "UsdLuxShadowAPI"),
            ("ShapingAPI", "UsdLuxShapingAPI"),
            ("VolumeLightAPI", "UsdLuxVolumeLightAPI"),
            ("RigidBodyAPI", "UsdPhysicsRigidBodyAPI"),
            ("CollisionAPI", "UsdPhysicsCollisionAPI"),
            ("MassAPI", "UsdPhysicsMassAPI"),
            ("MaterialAPI", "UsdPhysicsMaterialAPI"),
            ("MeshCollisionAPI", "UsdPhysicsMeshCollisionAPI"),
            ("ArticulationRootAPI", "UsdPhysicsArticulationRootAPI"),
            ("FilteredPairsAPI", "UsdPhysicsFilteredPairsAPI"),
            ("SkelBindingAPI", "UsdSkelBindingAPI"),
            ("ConnectableAPI", "UsdShadeConnectableAPI"),
            ("NodeDefAPI", "UsdShadeNodeDefAPI"),
            ("AssetPreviewsAPI", "UsdUtilsAssetPreviewsAPI"),
            ("MaterialXConfigAPI", "UsdMtlxMaterialXConfigAPI"),
            ("SceneGraphPrimAPI", "UsdRiSceneGraphPrimAPI"),
            ("NodeGraphNodeAPI", "UsdShadeNodeGraphNodeAPI"),
            ("RiMaterialAPI", "UsdRiMaterialAPI"),
            ("RiSplineAPI", "UsdRiSplineAPI"),
            ("StatementsAPI", "UsdRiStatementsAPI"),
        ];

        for (name, type_name) in &single_apis {
            register_schema(SchemaInfo {
                identifier: Token::new(name),
                type_name: type_name.to_string(),
                family: Token::new(name),
                version: 0,
                kind: SchemaKind::SingleApplyAPI,
                base_type_names: vec![],
                ..Default::default()
            });
            // Also register under the apiSchemas token (e.g. "PhysicsRigidBodyAPI")
            // which is what apply_api / has_api use in USD
            let api_token = type_name.trim_start_matches("Usd");
            if api_token != *name {
                register_schema(SchemaInfo {
                    identifier: Token::new(api_token),
                    type_name: type_name.to_string(),
                    family: Token::new(name),
                    version: 0,
                    kind: SchemaKind::SingleApplyAPI,
                    base_type_names: vec![],
                    ..Default::default()
                });
            }
        }

        // ====================================================================
        // MULTIPLE_APPLY_API
        // ====================================================================

        let multi_apis = [
            ("CollectionAPI", "UsdCollectionAPI"),
            ("MaterialBindingAPI", "UsdShadeMaterialBindingAPI"),
            ("CoordSysAPI", "UsdShadeCoordSysAPI"),
            ("DriveAPI", "UsdPhysicsDriveAPI"),
            ("LimitAPI", "UsdPhysicsLimitAPI"),
            ("LabelsAPI", "UsdSemanticsLabelsAPI"),
            ("AccessibilityAPI", "UsdUIAccessibilityAPI"),
        ];

        for (name, type_name) in &multi_apis {
            register_schema(SchemaInfo {
                identifier: Token::new(name),
                type_name: type_name.to_string(),
                family: Token::new(name),
                version: 0,
                kind: SchemaKind::MultipleApplyAPI,
                base_type_names: vec![],
                allowed_instance_names: None, // Any name allowed
                ..Default::default()
            });
            // Also register under the apiSchemas token (e.g. "PhysicsDriveAPI")
            let api_token = type_name.trim_start_matches("Usd");
            if api_token != *name {
                register_schema(SchemaInfo {
                    identifier: Token::new(api_token),
                    type_name: type_name.to_string(),
                    family: Token::new(name),
                    version: 0,
                    kind: SchemaKind::MultipleApplyAPI,
                    base_type_names: vec![],
                    allowed_instance_names: None,
                    ..Default::default()
                });
            }
        }
        // ====================================================================
        // Schema property names (lightweight prim definitions)
        // ====================================================================
        register_schema_properties("Imageable", &["visibility", "purpose", "proxyPrim"]);
        register_schema_fallbacks(
            "Imageable",
            &[
                ("visibility", Value::from("inherited".to_string())),
                ("purpose", Value::from("default".to_string())),
            ],
        );
        register_schema_properties("Xformable", &["xformOpOrder"]);
        register_schema_properties("Boundable", &["extent"]);
        register_schema_properties(
            "Gprim",
            &[
                "doubleSided",
                "orientation",
                "primvars:displayColor",
                "primvars:displayOpacity",
            ],
        );
        register_schema_fallbacks(
            "Gprim",
            &[
                ("doubleSided", Value::from(false)),
                ("orientation", Value::from("rightHanded".to_string())),
            ],
        );
        register_schema_properties(
            "PointBased",
            &["points", "velocities", "accelerations", "normals"],
        );
        register_schema_properties(
            "Mesh",
            &[
                "faceVertexIndices",
                "faceVertexCounts",
                "subdivisionScheme",
                "interpolateBoundary",
                "faceVaryingLinearInterpolation",
                "triangleSubdivisionRule",
                "holeIndices",
                "cornerIndices",
                "cornerSharpnesses",
                "creaseIndices",
                "creaseLengths",
                "creaseSharpnesses",
            ],
        );
        register_schema_properties("Sphere", &["radius", "extent"]);
        register_schema_property_types("Sphere", &[("radius", "double"), ("extent", "float3[]")]);
        register_schema_property_variabilities(
            "Sphere",
            &[
                ("radius", usd_sdf::Variability::Varying),
                ("extent", usd_sdf::Variability::Varying),
            ],
        );
        register_schema_fallbacks("Sphere", &[("radius", Value::from(1.0_f64))]);
        register_schema_properties("Cube", &["size", "extent"]);
        register_schema_fallbacks("Cube", &[("size", Value::from(2.0_f64))]);
        register_schema_properties("Cylinder", &["height", "radius", "axis", "extent"]);
        register_schema_properties(
            "Cylinder_1",
            &["height", "radiusTop", "radiusBottom", "axis", "extent"],
        );
        register_schema_properties("Cone", &["height", "radius", "axis", "extent"]);
        register_schema_properties("Capsule", &["height", "radius", "axis", "extent"]);
        register_schema_properties(
            "Capsule_1",
            &["height", "radiusTop", "radiusBottom", "axis", "extent"],
        );
        register_schema_properties(
            "Plane",
            &["width", "length", "axis", "extent", "doubleSided"],
        );
        register_schema_properties("Curves", &["curveVertexCounts", "widths"]);
        register_schema_properties("BasisCurves", &["type", "basis", "wrap"]);
        register_schema_properties("NurbsCurves", &["order", "knots", "ranges"]);
        register_schema_properties(
            "NurbsPatch",
            &[
                "uVertexCount",
                "vVertexCount",
                "uOrder",
                "vOrder",
                "uKnots",
                "vKnots",
                "uRange",
                "vRange",
                "pointWeights",
            ],
        );
        register_schema_properties("Points", &["widths", "ids"]);
        register_schema_properties("HermiteCurves", &["tangents"]);
        register_schema_properties(
            "Camera",
            &[
                "projection",
                "horizontalAperture",
                "verticalAperture",
                "horizontalApertureOffset",
                "verticalApertureOffset",
                "focalLength",
                "clippingRange",
                "clippingPlanes",
                "fStop",
                "focusDistance",
                "stereoRole",
                "shutterOpen",
                "shutterClose",
                "exposure",
                "exposure:iso",
                "exposure:time",
                "exposure:fStop",
                "exposure:responsivity",
            ],
        );
        register_schema_fallbacks(
            "Camera",
            &[
                ("projection", Value::from("perspective".to_string())),
                ("horizontalAperture", Value::from(20.955_f32)),
                ("verticalAperture", Value::from(15.2908_f32)),
                ("horizontalApertureOffset", Value::from(0.0_f32)),
                ("verticalApertureOffset", Value::from(0.0_f32)),
                ("focalLength", Value::from(50.0_f32)),
                (
                    "clippingRange",
                    Value::from_no_hash(Vec2f::new(1.0, 1_000_000.0)),
                ),
                ("clippingPlanes", Value::from(Vec::<Vec4f>::new())),
                ("fStop", Value::from(0.0_f32)),
                ("focusDistance", Value::from(0.0_f32)),
                ("exposure", Value::from(0.0_f32)),
                ("exposure:iso", Value::from(100.0_f32)),
                ("exposure:time", Value::from(1.0_f32)),
                ("exposure:fStop", Value::from(1.0_f32)),
                ("exposure:responsivity", Value::from(1.0_f32)),
            ],
        );
        register_schema_properties("Xform", &[]);
        register_schema_properties("Scope", &[]);
        register_schema_properties(
            "PointInstancer",
            &[
                "protoIndices",
                "ids",
                "positions",
                "orientations",
                "scales",
                "velocities",
                "accelerations",
                "angularVelocities",
                "invisibleIds",
                "prototypes",
            ],
        );
        register_schema_properties("TetMesh", &["tetVertexIndices", "surfaceFaceVertexIndices"]);

        // ====================================================================
        // UsdLux light types: property names + SDF type names
        // Matches C++ generatedSchema.usda for LightAPI, ShadowAPI, ShapingAPI
        // ====================================================================
        {
            // Common properties shared by all light types
            let common_props: &[&str] = &[
                "light:shaderId",
                "light:materialSyncMode",
                "inputs:intensity",
                "inputs:exposure",
                "inputs:diffuse",
                "inputs:specular",
                "inputs:normalize",
                "inputs:color",
                "inputs:enableColorTemperature",
                "inputs:colorTemperature",
                "inputs:shadow:enable",
                "inputs:shadow:color",
                "inputs:shadow:distance",
                "inputs:shadow:falloff",
                "inputs:shadow:falloffGamma",
                "inputs:shaping:focus",
                "inputs:shaping:focusTint",
                "inputs:shaping:cone:angle",
                "inputs:shaping:cone:softness",
                "inputs:shaping:ies:file",
                "inputs:shaping:ies:angleScale",
                "inputs:shaping:ies:normalize",
            ];
            let common_types: &[(&str, &str)] = &[
                ("light:shaderId", "token"),
                ("light:materialSyncMode", "token"),
                ("inputs:intensity", "float"),
                ("inputs:exposure", "float"),
                ("inputs:diffuse", "float"),
                ("inputs:specular", "float"),
                ("inputs:normalize", "bool"),
                ("inputs:color", "color3f"),
                ("inputs:enableColorTemperature", "bool"),
                ("inputs:colorTemperature", "float"),
                ("inputs:shadow:enable", "bool"),
                ("inputs:shadow:color", "color3f"),
                ("inputs:shadow:distance", "float"),
                ("inputs:shadow:falloff", "float"),
                ("inputs:shadow:falloffGamma", "float"),
                ("inputs:shaping:focus", "float"),
                ("inputs:shaping:focusTint", "color3f"),
                ("inputs:shaping:cone:angle", "float"),
                ("inputs:shaping:cone:softness", "float"),
                ("inputs:shaping:ies:file", "asset"),
                ("inputs:shaping:ies:angleScale", "float"),
                ("inputs:shaping:ies:normalize", "bool"),
            ];
            let light_types: &[(&str, &[&str], &[(&str, &str)])] = &[
                (
                    "RectLight",
                    &["inputs:width", "inputs:height", "inputs:texture:file"],
                    &[
                        ("inputs:width", "float"),
                        ("inputs:height", "float"),
                        ("inputs:texture:file", "asset"),
                    ],
                ),
                (
                    "SphereLight",
                    &["inputs:radius", "treatAsPoint"],
                    &[("inputs:radius", "float"), ("treatAsPoint", "bool")],
                ),
                (
                    "DiskLight",
                    &["inputs:radius"],
                    &[("inputs:radius", "float")],
                ),
                (
                    "CylinderLight",
                    &["inputs:radius", "inputs:length"],
                    &[("inputs:radius", "float"), ("inputs:length", "float")],
                ),
                (
                    "DistantLight",
                    &["inputs:angle"],
                    &[("inputs:angle", "float")],
                ),
                (
                    "DomeLight",
                    &["inputs:texture:file", "inputs:texture:format"],
                    &[
                        ("inputs:texture:file", "asset"),
                        ("inputs:texture:format", "token"),
                    ],
                ),
                (
                    "DomeLight_1",
                    &["inputs:texture:file", "inputs:texture:format"],
                    &[
                        ("inputs:texture:file", "asset"),
                        ("inputs:texture:format", "token"),
                    ],
                ),
                (
                    "PortalLight",
                    &["inputs:width", "inputs:height"],
                    &[("inputs:width", "float"), ("inputs:height", "float")],
                ),
                ("GeometryLight", &[], &[]),
                ("MeshLight", &[], &[]),
                ("VolumeLight", &[], &[]),
                ("PluginLight", &[], &[]),
                (
                    "LightFilter",
                    &["lightFilter:shaderId"],
                    &[("lightFilter:shaderId", "token")],
                ),
            ];
            for &(type_name, extra_props, extra_types) in light_types {
                let mut all_props: Vec<&str> = common_props.to_vec();
                all_props.extend_from_slice(extra_props);
                register_schema_properties(type_name, &all_props);

                let mut all_types: Vec<(&str, &str)> = common_types.to_vec();
                all_types.extend_from_slice(extra_types);
                register_schema_property_types(type_name, &all_types);
            }
        }

        sync_builtin_tf_types();

        eprintln!("[PERF] register_builtin_schemas: {:?}", _t0.elapsed());
    });
}

/// Register C++-style schema type names with `usd_tf::TfType` (plugin-style) so
/// `Tf.Type.FindByName("UsdGeomMesh")` and `TfType::is_a` match OpenUSD behavior.
fn sync_builtin_tf_types() {
    let schemas: Vec<SchemaInfo> = {
        let reg = global_registry();
        let r = reg.read();
        r.schemas.values().cloned().collect()
    };
    let mut schemas_sorted = schemas;
    schemas_sorted.sort_by_key(|info| info.base_type_names.len());
    for info in schemas_sorted {
        if info.type_name.is_empty() {
            continue;
        }
        let parent_cxx: Option<String> = info.base_type_names.first().and_then(|id| {
            let reg = global_registry();
            let r = reg.read();
            r.schemas.get(id).map(|b| b.type_name.clone())
        });
        if let Some(ref p) = parent_cxx {
            usd_tf::declare_by_name_with_bases(&info.type_name, &[p.as_str()]);
        } else {
            usd_tf::declare_by_name(&info.type_name);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a unique identifier for test isolation
    fn unique_id(base: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        format!("{}_{}", base, CTR.fetch_add(1, Ordering::Relaxed))
    }

    fn make_info(name: &str, kind: SchemaKind) -> SchemaInfo {
        let id = unique_id(name);
        let family = Token::new(name);
        SchemaInfo {
            identifier: Token::new(&id),
            type_name: format!("Usd{}", id),
            family,
            version: 0,
            kind,
            ..Default::default()
        }
    }

    #[test]
    fn test_register_and_find() {
        let info = make_info("TestRF", SchemaKind::ConcreteTyped);
        let id = info.identifier.clone();

        assert!(SchemaRegistry::register_schema_info(info));

        let found = SchemaRegistry::find_schema_info(&id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().kind, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_register_duplicate_replaces() {
        let mut info1 = make_info("TestDup", SchemaKind::ConcreteTyped);
        let id = info1.identifier.clone();
        info1.version = 0;
        assert!(SchemaRegistry::register_schema_info(info1));

        // Register again with same id but different kind
        let info2 = SchemaInfo {
            identifier: id.clone(),
            type_name: format!("Usd{}", id.as_str()),
            family: Token::new("TestDup"),
            version: 0,
            kind: SchemaKind::AbstractTyped,
            ..Default::default()
        };
        // Returns false because key already existed
        assert!(!SchemaRegistry::register_schema_info(info2));

        let found = SchemaRegistry::find_schema_info(&id).unwrap();
        assert_eq!(found.kind, SchemaKind::AbstractTyped);
    }

    #[test]
    fn test_unregister() {
        let info = make_info("TestUnreg", SchemaKind::ConcreteTyped);
        let id = info.identifier.clone();
        SchemaRegistry::register_schema_info(info);

        assert!(SchemaRegistry::find_schema_info(&id).is_some());
        assert!(SchemaRegistry::unregister_schema(id.as_str()));
        assert!(SchemaRegistry::find_schema_info(&id).is_none());
        // Second unregister returns false
        assert!(!SchemaRegistry::unregister_schema(id.as_str()));
    }

    #[test]
    fn test_family_queries() {
        let base = unique_id("FamQ");
        let family = Token::new(&base);

        // Register v0 and v1
        let info0 = SchemaInfo {
            identifier: Token::new(&base),
            type_name: format!("Usd{}_v0", base),
            family: family.clone(),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            ..Default::default()
        };
        let info1 = SchemaInfo {
            identifier: Token::new(&format!("{}_1", base)),
            type_name: format!("Usd{}_v1", base),
            family: family.clone(),
            version: 1,
            kind: SchemaKind::ConcreteTyped,
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info0);
        SchemaRegistry::register_schema_info(info1);

        let infos = SchemaRegistry::find_schema_infos_in_family(&family);
        assert_eq!(infos.len(), 2);
        // Sorted highest to lowest
        assert_eq!(infos[0].version, 1);
        assert_eq!(infos[1].version, 0);
    }

    #[test]
    fn test_family_filtered() {
        let base = unique_id("FamFilt");
        let family = Token::new(&base);

        for v in 0..3 {
            let id = if v == 0 {
                base.clone()
            } else {
                format!("{}_{}", base, v)
            };
            let info = SchemaInfo {
                identifier: Token::new(&id),
                type_name: format!("Usd{}_v{}", base, v),
                family: family.clone(),
                version: v,
                kind: SchemaKind::ConcreteTyped,
                ..Default::default()
            };
            SchemaRegistry::register_schema_info(info);
        }

        let gt = SchemaRegistry::find_schema_infos_in_family_filtered(
            &family,
            1,
            VersionPolicy::GreaterThan,
        );
        assert_eq!(gt.len(), 1);
        assert_eq!(gt[0].version, 2);

        let lte = SchemaRegistry::find_schema_infos_in_family_filtered(
            &family,
            1,
            VersionPolicy::LessThanOrEqual,
        );
        assert_eq!(lte.len(), 2);
    }

    #[test]
    fn test_auto_apply_query() {
        let id_str = unique_id("AutoAPI");
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: format!("Usd{}", id_str),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::SingleApplyAPI,
            auto_apply_to: vec![Token::new("Mesh"), Token::new("Xform")],
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        let map = SchemaRegistry::get_auto_apply_api_schemas();
        let targets = map.get(&Token::new(&id_str));
        assert!(targets.is_some());
        let targets = targets.unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&Token::new("Mesh")));
    }

    #[test]
    fn test_can_only_apply_to() {
        let id_str = unique_id("CanOnly");
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: format!("Usd{}", id_str),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::SingleApplyAPI,
            can_only_apply_to: vec![Token::new("Mesh"), Token::new("Points")],
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        let names = SchemaRegistry::get_api_schema_can_only_apply_to_type_names(
            &Token::new(&id_str),
            &Token::new(""),
        );
        assert_eq!(names.len(), 2);
        assert!(names.contains(&Token::new("Mesh")));
        assert!(names.contains(&Token::new("Points")));
    }

    #[test]
    fn test_allowed_instance_names() {
        let id_str = unique_id("MultiInst");
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: format!("Usd{}", id_str),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::MultipleApplyAPI,
            allowed_instance_names: Some(vec![Token::new("foo"), Token::new("bar")]),
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        assert!(SchemaRegistry::is_allowed_api_schema_instance_name(
            &Token::new(&id_str),
            &Token::new("foo"),
        ));
        assert!(SchemaRegistry::is_allowed_api_schema_instance_name(
            &Token::new(&id_str),
            &Token::new("bar"),
        ));
        assert!(!SchemaRegistry::is_allowed_api_schema_instance_name(
            &Token::new(&id_str),
            &Token::new("baz"),
        ));
        // Empty instance name is always disallowed
        assert!(!SchemaRegistry::is_allowed_api_schema_instance_name(
            &Token::new(&id_str),
            &Token::new(""),
        ));
    }

    #[test]
    fn test_allowed_instance_no_restriction() {
        let id_str = unique_id("MultiNoRestr");
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: format!("Usd{}", id_str),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::MultipleApplyAPI,
            allowed_instance_names: None, // no restriction
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        // Any non-empty instance name is allowed
        assert!(SchemaRegistry::is_allowed_api_schema_instance_name(
            &Token::new(&id_str),
            &Token::new("anything"),
        ));
    }

    #[test]
    fn test_schema_kind_queries() {
        let id_str = unique_id("KindQ");
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: format!("Usd{}", id_str),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::MultipleApplyAPI,
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        assert!(SchemaRegistry::is_multiple_apply_api_schema(&id_str));
        assert!(SchemaRegistry::is_applied_api_schema(&id_str));
        assert!(!SchemaRegistry::is_concrete(&id_str));
        assert!(!SchemaRegistry::is_abstract(&id_str));
    }

    #[test]
    fn test_type_name_reverse_lookup() {
        let id_str = unique_id("RevLook");
        let type_name_str = format!("UsdGeom{}", id_str);
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: type_name_str.clone(),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        // get_schema_type_name: type_name -> identifier
        let resolved = SchemaRegistry::get_schema_type_name(&type_name_str);
        assert_eq!(resolved, Token::new(&id_str));

        // get_type_from_schema_type_name: identifier -> type_name
        let type_back = SchemaRegistry::get_type_from_schema_type_name(&Token::new(&id_str));
        assert_eq!(type_back, type_name_str);
    }

    #[test]
    fn test_identifier_parsing() {
        let (fam, ver) = SchemaRegistry::parse_schema_family_and_version_from_identifier(
            &Token::new("FooAPI_1"),
        );
        assert_eq!(fam, Token::new("FooAPI"));
        assert_eq!(ver, 1);

        let (fam2, ver2) =
            SchemaRegistry::parse_schema_family_and_version_from_identifier(&Token::new("FooAPI"));
        assert_eq!(fam2, Token::new("FooAPI"));
        assert_eq!(ver2, 0);
    }

    #[test]
    fn test_make_identifier() {
        let id =
            SchemaRegistry::make_schema_identifier_for_family_and_version(&Token::new("FooAPI"), 0);
        assert_eq!(id, Token::new("FooAPI"));

        let id2 =
            SchemaRegistry::make_schema_identifier_for_family_and_version(&Token::new("FooAPI"), 3);
        assert_eq!(id2, Token::new("FooAPI_3"));
    }

    #[test]
    fn test_multiple_apply_name_template() {
        let t = SchemaRegistry::make_multiple_apply_name_template("collection", "includes");
        assert_eq!(t.as_str(), "collection:__INSTANCE_NAME__:includes");
        assert!(SchemaRegistry::is_multiple_apply_name_template(t.as_str()));

        let inst = SchemaRegistry::make_multiple_apply_name_instance(t.as_str(), "plasticStuff");
        assert_eq!(inst.as_str(), "collection:plasticStuff:includes");

        let base = SchemaRegistry::get_multiple_apply_name_template_base_name(t.as_str());
        assert_eq!(base.as_str(), "includes");
    }

    #[test]
    fn test_type_name_and_instance() {
        let (ty, inst) =
            SchemaRegistry::get_type_name_and_instance(&Token::new("CollectionAPI:plasticStuff"));
        assert_eq!(ty, Token::new("CollectionAPI"));
        assert_eq!(inst, Token::new("plasticStuff"));

        let (ty2, inst2) = SchemaRegistry::get_type_name_and_instance(&Token::new("ShadowAPI"));
        assert_eq!(ty2, Token::new("ShadowAPI"));
        assert_eq!(inst2, Token::new(""));
    }

    #[test]
    fn test_disallowed_fields() {
        assert!(SchemaRegistry::is_disallowed_field(&Token::new(
            "inheritPaths"
        )));
        assert!(SchemaRegistry::is_disallowed_field(&Token::new("kind")));
        assert!(SchemaRegistry::is_disallowed_field(&Token::new(
            "customData"
        )));
        assert!(!SchemaRegistry::is_disallowed_field(&Token::new(
            "documentation"
        )));
    }

    #[test]
    fn test_base_type_names() {
        let id_str = unique_id("BaseTypes");
        let info = SchemaInfo {
            identifier: Token::new(&id_str),
            type_name: format!("Usd{}", id_str),
            family: Token::new(&id_str),
            version: 0,
            kind: SchemaKind::ConcreteTyped,
            base_type_names: vec![Token::new("Typed"), Token::new("Gprim")],
            ..Default::default()
        };
        SchemaRegistry::register_schema_info(info);

        let found = SchemaRegistry::find_schema_info(&Token::new(&id_str)).unwrap();
        assert_eq!(found.base_type_names.len(), 2);
        assert!(found.base_type_names.contains(&Token::new("Typed")));
        assert!(found.base_type_names.contains(&Token::new("Gprim")));
    }

    #[test]
    #[ignore] // Flaky in parallel test runs due to global state
    fn test_builtin_schemas_registered() {
        // Ensure builtin schemas are registered
        register_builtin_schemas();

        // Test abstract types exist
        let typed = SchemaRegistry::find_schema_info(&Token::new("Typed"));
        assert!(typed.is_some(), "Typed schema should be registered");
        assert_eq!(typed.unwrap().kind, SchemaKind::AbstractTyped);

        let gprim = SchemaRegistry::find_schema_info(&Token::new("Gprim"));
        assert!(gprim.is_some(), "Gprim schema should be registered");
        assert_eq!(gprim.unwrap().kind, SchemaKind::AbstractTyped);

        // Test concrete types exist
        let mesh = SchemaRegistry::find_schema_info(&Token::new("Mesh"));
        assert!(mesh.is_some(), "Mesh schema should be registered");
        let mesh = mesh.unwrap();
        assert_eq!(mesh.kind, SchemaKind::ConcreteTyped);
        assert_eq!(mesh.type_name, "UsdGeomMesh");
        // Note: base_type_names may vary if other tests register schemas
        // We just verify the schema exists and has the correct kind/type_name

        let cube = SchemaRegistry::find_schema_info(&Token::new("Cube"));
        assert!(cube.is_some(), "Cube schema should be registered");
        assert_eq!(cube.unwrap().kind, SchemaKind::ConcreteTyped);

        // Test API schemas
        let model_api = SchemaRegistry::find_schema_info(&Token::new("ModelAPI"));
        assert!(model_api.is_some(), "ModelAPI schema should be registered");
        assert_eq!(model_api.unwrap().kind, SchemaKind::SingleApplyAPI);

        let collection_api = SchemaRegistry::find_schema_info(&Token::new("CollectionAPI"));
        assert!(
            collection_api.is_some(),
            "CollectionAPI schema should be registered"
        );
        assert_eq!(collection_api.unwrap().kind, SchemaKind::MultipleApplyAPI);
    }
}
