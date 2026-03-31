//! USD Shade CoordSysAPI - API schema for coordinate systems.
//!
//! Port of pxr/usd/usdShade/coordSysAPI.h and coordSysAPI.cpp
//!
//! UsdShadeCoordSysAPI provides a way to designate, name, and discover coordinate systems.

use super::tokens::tokens;
use std::sync::Arc;
use usd_core::prim::Prim;
use usd_core::relationship::Relationship;
use usd_core::schema_base::APISchemaBase;
use usd_core::stage::Stage;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// Binding
// ============================================================================

/// A coordinate system binding.
/// Binds a name to a coordSysPrim for the bindingPrim (and its descendants,
/// unless overriden).
#[derive(Debug, Clone)]
pub struct Binding {
    /// The name of the coordinate system.
    pub name: Token,
    /// The path to the binding relationship.
    pub binding_rel_path: Path,
    /// The path to the coordinate system prim.
    pub coord_sys_prim_path: Path,
}

// ============================================================================
// CoordSysAPI
// ============================================================================

/// UsdShadeCoordSysAPI provides a way to designate, name, and discover coordinate systems.
///
/// This is a MultipleApplyAPI schema.
#[derive(Debug, Clone)]
pub struct CoordSysAPI {
    /// Base API schema with instance name.
    base: APISchemaBase,
}

impl CoordSysAPI {
    /// Constructs a CoordSysAPI on the given prim with the given instance name.
    ///
    /// Matches C++ `UsdShadeCoordSysAPI(const UsdPrim& prim, const TfToken &name)`.
    pub fn new(prim: Prim, name: Token) -> Self {
        Self {
            base: APISchemaBase::new_with_instance(prim, name),
        }
    }

    /// Constructs a CoordSysAPI from an APISchemaBase.
    ///
    /// Matches C++ `UsdShadeCoordSysAPI(const UsdSchemaBase& schemaObj, const TfToken &name)`.
    pub fn from_schema_base(schema: APISchemaBase, name: Token) -> Self {
        Self {
            base: APISchemaBase::new_with_instance(schema.get_prim().clone(), name),
        }
    }

    /// Creates an invalid CoordSysAPI.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Returns the name of this multiple-apply schema instance.
    ///
    /// Matches C++ `GetName()`.
    pub fn name(&self) -> Token {
        self.base
            .instance_name()
            .cloned()
            .unwrap_or_else(|| Token::new(""))
    }

    /// Return a CoordSysAPI holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdShadeCoordSysAPI::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    /// Path must be of format <path>.coordSys:name
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        if !path.is_property_path() {
            return Self::invalid();
        }

        let mut name = Token::new("");
        if !Self::is_coord_sys_api_path(path, &mut name) {
            return Self::invalid();
        }

        if let Some(prim) = stage.get_prim_at_path(&path.get_prim_path()) {
            Self::new(prim, name)
        } else {
            Self::invalid()
        }
    }

    /// Return a CoordSysAPI with name `name` holding the prim `prim`.
    ///
    /// Matches C++ `UsdShadeCoordSysAPI::Get(const UsdPrim &prim, const TfToken &name)`.
    pub fn get_from_prim(prim: &Prim, name: &Token) -> Self {
        Self::new(prim.clone(), name.clone())
    }

    /// Return a vector of all named instances of CoordSysAPI on the given prim.
    ///
    /// Matches C++ `GetAll(const UsdPrim &prim)`.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let applied_schemas = prim.get_applied_schemas();
        let mut coord_sys_apis = Vec::new();

        let coord_sys_api_prefix = format!("{}:", Self::schema_type_name().get_text());

        for schema_name in applied_schemas {
            let schema_str = schema_name.get_text();
            if schema_str.starts_with(&coord_sys_api_prefix) {
                let instance_name = schema_str[coord_sys_api_prefix.len()..].to_string();
                coord_sys_apis.push(Self::new(prim.clone(), Token::new(&instance_name)));
            }
        }

        coord_sys_apis
    }

    /// Checks if the given baseName is the base name of a property of CoordSysAPI.
    ///
    /// Matches C++ `IsSchemaPropertyBaseName(const TfToken &baseName)`.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        // The binding relationship base name is "binding"
        base_name == "binding"
    }

    /// Checks if the given path is of an API schema of type CoordSysAPI.
    ///
    /// Matches C++ `IsCoordSysAPIPath(const SdfPath &path, TfToken *name)`.
    pub fn is_coord_sys_api_path(path: &Path, name: &mut Token) -> bool {
        if !path.is_property_path() {
            return false;
        }

        let property_name = path.get_name();
        let parts: Vec<&str> = property_name.split(':').collect();

        // The baseName of the path can't be one of the schema properties
        if let Some(base_name) = parts.last() {
            if Self::is_schema_property_base_name(&Token::new(base_name)) {
                return false;
            }
        }

        // Check if it starts with "coordSys:"
        if parts.len() >= 2 && parts[0] == tokens().coord_sys.as_str() {
            // Extract the instance name (everything after "coordSys:")
            let instance_name = property_name[tokens().coord_sys.as_str().len() + 1..].to_string();
            *name = Token::new(&instance_name);
            return true;
        }

        false
    }

    /// Returns true if this multiple-apply API schema can be applied to the given prim.
    ///
    /// Matches C++ `CanApply(const UsdPrim &prim, const TfToken &name, std::string *whyNot)`.
    pub fn can_apply(prim: &Prim, _name: &Token, why_not: &mut Option<String>) -> bool {
        if !prim.is_valid() {
            if let Some(reason) = why_not {
                *reason = "Invalid prim".to_string();
            }
            return false;
        }

        let _schema_type_name = tokens().coord_sys_api.clone();
        // For multiple-apply APIs, we need to check if the specific instance can be applied
        // For now, just check if the prim is valid
        true
    }

    /// Applies this multiple-apply API schema to the given prim with the given instance name.
    ///
    /// Matches C++ `UsdShadeCoordSysAPI::Apply(const UsdPrim &prim, const TfToken &name)`.
    pub fn apply(prim: &Prim, name: &Token) -> Self {
        if !prim.is_valid() {
            return Self::invalid();
        }

        // Multi-apply API: use apply_api_instance with schema type + instance name
        if prim.apply_api_instance(&tokens().coord_sys_api, name) {
            Self::new(prim.clone(), name.clone())
        } else {
            Self::invalid()
        }
    }

    /// Returns true if this CoordSysAPI is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid() && self.base.instance_name().is_some()
    }

    /// Returns the wrapped prim.
    ///
    /// Matches C++ `GetPrim()`.
    pub fn get_prim(&self) -> &Prim {
        self.base.get_prim()
    }

    /// Returns the path to this prim.
    ///
    /// Matches C++ `GetPath()`.
    pub fn path(&self) -> &Path {
        self.base.path()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        tokens().coord_sys_api.clone()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // CoordSysAPI doesn't define any schema attributes
        Vec::new()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class for a given instance name.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited, const TfToken &instanceName)`.
    pub fn get_schema_attribute_names_for_instance(
        _include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        let attr_names = Self::get_schema_attribute_names(_include_inherited);
        if instance_name.as_str().is_empty() {
            return attr_names;
        }

        let mut result = Vec::new();
        result.reserve(attr_names.len());
        for attr_name in attr_names {
            // Namespace the attribute name with the instance name
            let namespaced_name =
                format!("coordSys:{}:{}", instance_name.as_str(), attr_name.as_str());
            result.push(Token::new(&namespaced_name));
        }
        result
    }

    // ========================================================================
    // Helper function for constructing namespaced property names
    // ========================================================================

    /// Returns the property name with `__INSTANCE_NAME__` substituted.
    ///
    /// C++ coordSysAPI.cpp:161-164:
    ///   `UsdSchemaRegistry::MakeMultipleApplyNameInstance(propName, instanceName)`
    /// The `prop_name` is a template token like `"coordSys:__INSTANCE_NAME__:binding"`.
    /// This function replaces `__INSTANCE_NAME__` with the actual instance name,
    /// producing e.g. `"coordSys:modelSpace:binding"`.
    fn _get_namespaced_property_name(instance_name: &Token, prop_name: &Token) -> Token {
        Token::new(
            &prop_name
                .as_str()
                .replace("__INSTANCE_NAME__", instance_name.as_str()),
        )
    }

    // ========================================================================
    // Binding
    // ========================================================================

    /// Prim binding expressing the appropriate coordinate systems.
    ///
    /// Matches C++ `GetBindingRel()`.
    pub fn get_binding_rel(&self) -> Relationship {
        let rel_name = Self::_get_namespaced_property_name(
            &self.name(),
            &tokens().coord_sys_multiple_apply_template_binding,
        );
        self.get_prim()
            .get_relationship(rel_name.as_str())
            .unwrap_or_else(Relationship::invalid)
    }

    /// See GetBindingRel(), and also CreateBindingRel for when to use Get vs Create.
    ///
    /// Matches C++ `CreateBindingRel()`.
    pub fn create_binding_rel(&self) -> Option<Relationship> {
        let rel_name = Self::_get_namespaced_property_name(
            &self.name(),
            &tokens().coord_sys_multiple_apply_template_binding,
        );
        self.get_prim()
            .create_relationship(rel_name.as_str(), false)
    }

    /// Get the coordinate system bindings local to this prim corresponding to
    /// this instance name.
    ///
    /// Matches C++ `GetLocalBinding()`.
    pub fn get_local_binding(&self) -> Binding {
        let mut result = Binding {
            name: Token::new(""),
            binding_rel_path: Path::empty(),
            coord_sys_prim_path: Path::empty(),
        };

        let rel = self.get_binding_rel();
        if rel.is_valid() {
            let target_paths = rel.get_forwarded_targets();
            if !target_paths.is_empty() {
                result.name = Self::get_binding_base_name(&rel.name());
                result.binding_rel_path = rel.path().clone();
                result.coord_sys_prim_path = target_paths[0].clone();
            }
        }

        result
    }

    /// Find the coordinate system bindings that apply to this prim, including
    /// inherited bindings.
    ///
    /// Matches C++ `FindBindingWithInheritance()`.
    pub fn find_binding_with_inheritance(&self) -> Binding {
        let mut result = Binding {
            name: Token::new(""),
            binding_rel_path: Path::empty(),
            coord_sys_prim_path: Path::empty(),
        };

        let instance_name = self.name();
        let rel_name = Self::_get_namespaced_property_name(
            &instance_name,
            &tokens().coord_sys_multiple_apply_template_binding,
        );

        let mut p = self.get_prim().clone();
        while p.is_valid() {
            // Check if this prim has CoordSysAPI with the same instance name applied
            let applied_schemas = p.get_applied_schemas();
            let schema_instance_name = format!(
                "{}:{}",
                tokens().coord_sys_api.as_str(),
                instance_name.as_str()
            );
            let has_api = applied_schemas
                .iter()
                .any(|s| s.as_str() == schema_instance_name);

            if has_api {
                if let Some(rel) = p.get_relationship(rel_name.as_str()) {
                    let target_paths = rel.get_forwarded_targets();
                    if !target_paths.is_empty() {
                        result.name = Self::get_binding_base_name(&rel.name());
                        result.binding_rel_path = rel.path().clone();
                        result.coord_sys_prim_path = target_paths[0].clone();
                        break;
                    }
                }
            }

            // Move to parent
            let parent = p.parent();
            if !parent.is_valid() {
                break;
            }
            p = parent;
        }

        result
    }

    /// Bind the name to the given path.
    ///
    /// Matches C++ `Bind(const SdfPath &path)`.
    pub fn bind(&self, path: &Path) -> bool {
        if let Some(rel) = self.create_binding_rel() {
            return rel.set_targets(&[path.clone()]);
        }
        false
    }

    /// Clear the coordinate system binding on the prim corresponding to the
    /// instanceName of this CoordSysAPI, from the current edit target.
    ///
    /// Matches C++ `ClearBinding(bool removeSpec)`.
    pub fn clear_binding(&self, remove_spec: bool) -> bool {
        let rel = self.get_binding_rel();
        if rel.is_valid() {
            if remove_spec {
                return rel.clear_targets();
            } else {
                return rel.set_targets(&[]);
            }
        }
        false
    }

    /// Block the indicated coordinate system binding on this prim by blocking
    /// targets on the underlying relationship.
    ///
    /// Matches C++ `BlockBinding()`.
    pub fn block_binding(&self) -> bool {
        if let Some(rel) = self.create_binding_rel() {
            return rel.block_targets();
        }
        false
    }

    // ========================================================================
    // Static methods
    // ========================================================================

    /// Get the list of coordinate system bindings local to this prim, across
    /// all multi-apply instanceNames.
    ///
    /// Matches C++ `GetLocalBindingsForPrim(const UsdPrim &prim)`.
    pub fn get_local_bindings_for_prim(prim: &Prim) -> Vec<Binding> {
        let mut result = Vec::new();
        Self::_get_bindings_for_prim(prim, &mut result, false);
        result
    }

    /// Find the list of coordinate system bindings that apply to this prim,
    /// including inherited bindings.
    ///
    /// Matches C++ `FindBindingsWithInheritanceForPrim(const UsdPrim &prim)`.
    pub fn find_bindings_with_inheritance_for_prim(prim: &Prim) -> Vec<Binding> {
        let mut result = Vec::new();

        let mut p = prim.clone();
        while p.is_valid() {
            Self::_get_bindings_for_prim(&p, &mut result, true);

            // Move to parent
            let parent = p.parent();
            if !parent.is_valid() {
                break;
            }
            p = parent;
        }

        result
    }

    /// Returns true if the prim has UsdShadeCoordSysAPI applied.
    ///
    /// Matches C++ `HasLocalBindingsForPrim(const UsdPrim &prim)`.
    pub fn has_local_bindings_for_prim(prim: &Prim) -> bool {
        let applied_schemas = prim.get_applied_schemas();
        let coord_sys_api_prefix = format!("{}:", tokens().coord_sys_api.as_str());
        applied_schemas
            .iter()
            .any(|s| s.as_str().starts_with(&coord_sys_api_prefix))
    }

    /// Returns true if this prim has local coord sys bindings.
    ///
    /// Matches C++ `HasLocalBindings()` (instance method).
    pub fn has_local_bindings(&self) -> bool {
        Self::has_local_bindings_for_prim(self.get_prim())
    }

    /// Get the list of coordinate system bindings local to this prim,
    /// across all multi-apply instanceNames.
    ///
    /// Matches C++ `GetLocalBindings()` (instance method, deprecated).
    pub fn get_local_bindings(&self) -> Vec<Binding> {
        Self::get_local_bindings_for_prim(self.get_prim())
    }

    /// Find the list of coordinate system bindings that apply to this prim,
    /// including inherited bindings.
    ///
    /// Matches C++ `FindBindingsWithInheritance()` (instance method, deprecated).
    pub fn find_bindings_with_inheritance(&self) -> Vec<Binding> {
        Self::find_bindings_with_inheritance_for_prim(self.get_prim())
    }

    /// Bind the name to the given path (deprecated 2-arg overload).
    ///
    /// Matches C++ `Bind(const TfToken &name, const SdfPath &path)`.
    pub fn bind_named(&self, name: &Token, path: &Path) -> bool {
        let coord_sys = Self::new(self.get_prim().clone(), name.clone());
        coord_sys.bind(path)
    }

    /// Convenience API: Apply schema instance + bind in one step (deprecated).
    ///
    /// Matches C++ `ApplyAndBind(const TfToken &name, const SdfPath &path)`.
    pub fn apply_and_bind(&self, name: &Token, path: &Path) -> bool {
        let applied = Self::apply(self.get_prim(), name);
        if !applied.is_valid() {
            return false;
        }
        applied.bind(path)
    }

    /// Clear the indicated coordinate system binding by name (deprecated 2-arg overload).
    ///
    /// Matches C++ `ClearBinding(const TfToken &name, bool removeSpec)`.
    pub fn clear_binding_named(&self, name: &Token, remove_spec: bool) -> bool {
        let coord_sys = Self::new(self.get_prim().clone(), name.clone());
        coord_sys.clear_binding(remove_spec)
    }

    /// Block the indicated coordinate system binding by name (deprecated 1-arg overload).
    ///
    /// Matches C++ `BlockBinding(const TfToken &name)`.
    pub fn block_binding_named(&self, name: &Token) -> bool {
        let coord_sys = Self::new(self.get_prim().clone(), name.clone());
        coord_sys.block_binding()
    }

    /// Returns the fully namespaced coordinate system relationship name
    /// given the coordinate system name (deprecated).
    ///
    /// Matches C++ `GetCoordSysRelationshipName(const std::string &coordSysName)`.
    pub fn get_coord_sys_rel_name(coord_sys_name: &str) -> Token {
        Token::new(&format!("coordSys:{}", coord_sys_name))
    }

    /// Test whether a given name contains the "coordSys:" prefix.
    ///
    /// Matches C++ `CanContainPropertyName(const TfToken &name)`.
    pub fn can_contain_property_name(name: &Token) -> bool {
        name.as_str().starts_with(tokens().coord_sys.as_str())
    }

    /// Strips "coordSys:" from the relationship name and returns
    /// "<instanceName>:binding".
    ///
    /// Matches C++ `GetBindingBaseName(const TfToken &name)`.
    pub fn get_binding_base_name(binding_name: &Token) -> Token {
        let name_str = binding_name.as_str();
        if let Some(stripped) = name_str.strip_prefix(tokens().coord_sys.as_str()) {
            if let Some(rest) = stripped.strip_prefix(':') {
                // Extract instance name (everything before ":binding")
                if let Some(colon_idx) = rest.find(':') {
                    return Token::new(&rest[..colon_idx]);
                }
                return Token::new(rest);
            }
        }
        Token::new("")
    }

    /// Strips "coordSys:" from the relationship name and returns
    /// "<instanceName>:binding".
    ///
    /// Matches C++ `GetBindingBaseName()`.
    pub fn get_binding_base_name_instance(&self) -> Token {
        let rel_name = Self::_get_namespaced_property_name(
            &self.name(),
            &tokens().coord_sys_multiple_apply_template_binding,
        );
        Self::get_binding_base_name(&rel_name)
    }

    /// Helper method for getting bindings for a prim.
    fn _get_bindings_for_prim(
        prim: &Prim,
        result: &mut Vec<Binding>,
        check_existing_bindings: bool,
    ) {
        if !Self::has_local_bindings_for_prim(prim) {
            return;
        }

        // Get all CoordSysAPI instances on this prim
        let coord_sys_apis = Self::get_all(prim);

        for coord_sys_api in coord_sys_apis {
            let rel_name = Self::_get_namespaced_property_name(
                &coord_sys_api.name(),
                &tokens().coord_sys_multiple_apply_template_binding,
            );

            if let Some(rel) = prim.get_relationship(rel_name.as_str()) {
                let mut name_is_already_bound = false;
                if check_existing_bindings {
                    let binding_base_name = Self::get_binding_base_name(&rel.name());
                    for existing in result.iter() {
                        if existing.name == binding_base_name {
                            name_is_already_bound = true;
                            break;
                        }
                    }
                }

                if !check_existing_bindings || !name_is_already_bound {
                    let target_paths = rel.get_forwarded_targets();
                    if !target_paths.is_empty() {
                        result.push(Binding {
                            name: Self::get_binding_base_name(&rel.name()),
                            binding_rel_path: rel.path().clone(),
                            coord_sys_prim_path: target_paths[0].clone(),
                        });
                    }
                }
            }
        }
    }
}

impl PartialEq for CoordSysAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for CoordSysAPI {}

impl std::hash::Hash for CoordSysAPI {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base.hash(state);
    }
}
