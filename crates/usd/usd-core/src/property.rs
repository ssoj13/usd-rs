//! Base class for attributes and relationships.

use super::object::{ObjType, Object, Stage};
use std::sync::Weak;
use usd_sdf::Path;
use usd_pcp::{Site, TargetSpecType, build_prim_property_index, build_target_index};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Property
// ============================================================================

/// Base class for attributes and relationships.
///
/// A property represents a named piece of data on a prim. Properties can be
/// either attributes (typed values) or relationships (connections to other prims).
#[derive(Debug, Clone)]
pub struct Property {
    /// Base object data.
    inner: Object,
}

impl Property {
    /// Creates a new property.
    pub(crate) fn new(stage: Weak<Stage>, path: Path) -> Self {
        Self {
            inner: Object::new(stage, path),
        }
    }

    /// Creates a new property with an explicit object type.
    pub(crate) fn new_with_type(stage: Weak<Stage>, path: Path, obj_type: ObjType) -> Self {
        Self {
            inner: Object::new_with_type(stage, path, obj_type),
        }
    }

    /// Creates an invalid property.
    pub fn invalid() -> Self {
        Self {
            inner: Object::invalid(),
        }
    }

    /// Returns true if this property is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid() && self.inner.path.is_property_path()
    }

    /// Returns the stage that owns this property.
    pub fn stage(&self) -> Option<std::sync::Arc<Stage>> {
        self.inner.stage()
    }

    /// Returns the path to this property.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Returns the name of this property.
    pub fn name(&self) -> Token {
        Token::new(self.inner.path.get_name())
    }

    /// Returns the base name (without namespace prefix).
    pub fn base_name(&self) -> Token {
        let name = self.inner.path.get_name();
        if let Some(idx) = name.rfind(':') {
            Token::new(&name[idx + 1..])
        } else {
            Token::new(name)
        }
    }

    /// Returns the namespace prefix (if any).
    pub fn namespace(&self) -> Token {
        let name = self.inner.path.get_name();
        if let Some(idx) = name.rfind(':') {
            Token::new(&name[..idx])
        } else {
            Token::new("")
        }
    }

    /// Returns the namespaces as a list.
    pub fn split_name(&self) -> Vec<Token> {
        let name = self.inner.path.get_name();
        name.split(':').map(Token::new).collect()
    }

    /// Returns the prim path that owns this property.
    pub fn prim_path(&self) -> Path {
        self.inner.prim_path()
    }

    /// Composes target-style list ops for relationships and attribute connections
    /// via PCP, then remaps prototype-space paths exactly like
    /// `UsdProperty::_GetTargets`.
    pub(crate) fn get_composed_targets(&self, spec_type: TargetSpecType) -> Vec<Path> {
        let Some(stage) = self.stage() else {
            return Vec::new();
        };
        let Some(pcp_cache) = stage.pcp_cache() else {
            return Vec::new();
        };

        let prim_path = self.prim_path();
        let owning_prim = stage.get_prim_at_path(&prim_path);
        let indexing_prim = owning_prim.as_ref().map(|prim| {
            if prim.is_instance_proxy() {
                prim.get_prim_in_prototype()
            } else {
                prim.clone()
            }
        });
        let uses_prototype_mapping = indexing_prim
            .as_ref()
            .is_some_and(|prim| prim.is_in_prototype());
        let source_anchor = indexing_prim
            .as_ref()
            .and_then(|prim| prim.source_prim_index())
            .map(|index| index.path().clone())
            .unwrap_or_else(|| prim_path.clone());

        let property_path = if uses_prototype_mapping {
            source_anchor
                .append_property(self.name().get_text())
                .unwrap_or_else(|| self.path().clone())
        } else {
            self.path().clone()
        };

        let Some(prim_index) = indexing_prim
            .as_ref()
            .and_then(|prim| prim.prim_index())
            .map(std::sync::Arc::new)
        else {
            return Vec::new();
        };

        let (prop_index, _prop_errors) = build_prim_property_index(&property_path, &prim_index);
        let prop_site = Site::new(
            pcp_cache.layer_stack_identifier().clone(),
            property_path.clone(),
        );
        let (target_index, _target_errors) = build_target_index(&prop_site, &prop_index, spec_type);

        self.resolve_composed_target_paths(
            target_index.paths,
            if uses_prototype_mapping {
                &source_anchor
            } else {
                &prim_path
            },
            owning_prim
                .as_ref()
                .filter(|prim| prim.is_instance_proxy() || uses_prototype_mapping),
        )
    }

    fn resolve_composed_target_paths(
        &self,
        paths: Vec<Path>,
        anchor: &Path,
        prototype_prim: Option<&super::prim::Prim>,
    ) -> Vec<Path> {
        let resolved: Vec<Path> = paths
            .into_iter()
            .filter_map(|path| {
                let absolute = if path.is_absolute_path() {
                    path
                } else {
                    path.make_absolute(anchor)?
                };
                Self::normalize_dotdot_segments(&absolute)
            })
            .collect();

        if let Some(prim) = prototype_prim {
            resolved
                .into_iter()
                .map(|path| prim.map_prototype_source_path_to_current(&path))
                .collect()
        } else {
            resolved
        }
    }

    fn normalize_dotdot_segments(path: &Path) -> Option<Path> {
        let text = path.as_str();
        if !text.contains("..") {
            return Some(path.clone());
        }

        let (prim_str, prop_str) = if let Some(dot) = text.rfind('.') {
            let before = &text[..dot];
            if before.contains('/') {
                (before, Some(&text[dot + 1..]))
            } else {
                (text, None)
            }
        } else {
            (text, None)
        };

        let mut components: Vec<&str> = Vec::new();
        for component in prim_str.split('/').filter(|component| !component.is_empty()) {
            if component == ".." {
                components.pop();
            } else {
                components.push(component);
            }
        }

        let mut result = format!("/{}", components.join("/"));
        if let Some(prop) = prop_str {
            result.push('.');
            result.push_str(prop);
        }

        Path::from_string(&result)
    }

    /// Returns true if this is an attribute.
    ///
    /// Queries the spec type from the layer stack to determine property type.
    /// Matches C++ `UsdProperty::Is<UsdAttribute>()`.
    pub fn is_attribute(&self) -> bool {
        if let Some(stage) = self.inner.stage() {
            for layer in stage.layer_stack() {
                if layer.get_attribute_at_path(&self.inner.path).is_some() {
                    return true;
                }
                if layer.get_relationship_at_path(&self.inner.path).is_some() {
                    return false;
                }
            }
        }
        // No spec found - default to attribute for backward compat
        true
    }

    /// Returns true if this is a relationship.
    ///
    /// Queries the spec type from the layer stack.
    /// Matches C++ `UsdProperty::Is<UsdRelationship>()`.
    pub fn is_relationship(&self) -> bool {
        if let Some(stage) = self.inner.stage() {
            for layer in stage.layer_stack() {
                if layer.get_relationship_at_path(&self.inner.path).is_some() {
                    return true;
                }
                if layer.get_attribute_at_path(&self.inner.path).is_some() {
                    return false;
                }
            }
        }
        false
    }

    /// Converts this property to an attribute if it is one.
    ///
    /// Matches C++ `As<UsdAttribute>()`.
    pub fn as_attribute(&self) -> Option<super::attribute::Attribute> {
        if self.is_attribute() {
            Some(super::attribute::Attribute::new(
                self.inner.stage.clone(),
                self.inner.path.clone(),
            ))
        } else {
            None
        }
    }

    /// Converts this property to a relationship if it is one.
    ///
    /// Matches C++ `As<UsdRelationship>()`.
    pub fn as_relationship(&self) -> Option<super::relationship::Relationship> {
        if self.is_relationship() {
            Some(super::relationship::Relationship::new(
                self.inner.stage.clone(),
                self.inner.path.clone(),
            ))
        } else {
            None
        }
    }

    /// Returns true if there are any authored opinions for this property in any
    /// layer that contributes to this stage.
    ///
    /// Matches C++ `UsdProperty::IsAuthored()`.
    pub fn is_authored(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        // Check entire layer stack, not just edit target (C++ parity fix)
        for layer in stage.layer_stack() {
            if layer.get_attribute_at_path(self.path()).is_some()
                || layer.get_relationship_at_path(self.path()).is_some()
            {
                return true;
            }
        }
        false
    }

    /// Returns true if there is an SdfPropertySpec authored for this property
    /// at the given edit target, otherwise returns false.
    ///
    /// Does not do partial composition - checks only exactly at the given target.
    ///
    /// Matches C++ `UsdProperty::IsAuthoredAt(const UsdEditTarget&) const`.
    pub fn is_authored_at(&self, edit_target: &super::edit_target::EditTarget) -> bool {
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        let spec_path = edit_target.map_to_spec_path(self.path());
        layer.get_attribute_at_path(&spec_path).is_some()
            || layer.get_relationship_at_path(&spec_path).is_some()
    }

    /// Returns true if this property is custom (not defined in a schema).
    ///
    /// Checks the strongest layer opinion, across the full layer stack.
    /// Schema-defined properties have `custom` = false by default.
    ///
    /// Matches C++ `UsdProperty::IsCustom`: built-in schema properties are never
    /// reported as custom even if a local `SdfAttributeSpec` authors `custom = true`.
    pub fn is_custom(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let prim_path = self.prim_path();
        let prop_name = self.name();
        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let type_name = prim.type_name();
            if !type_name.is_empty() {
                let registry = super::schema_registry::SchemaRegistry::get_instance();
                if let Some(def) = registry.find_concrete_prim_definition(&type_name) {
                    if def.property_names().contains(&prop_name) {
                        return false;
                    }
                }
                if super::schema_registry::schema_has_property(&type_name, prop_name.as_str()) {
                    return false;
                }
            }
        }

        let custom_key = usd_tf::Token::new("custom");
        // Strongest-wins: iterate layer stack, return first opinion found
        for layer in stage.layer_stack() {
            if let Some(attr_spec) = layer.get_attribute_at_path(self.path()) {
                return attr_spec
                    .as_spec()
                    .get_field(&custom_key)
                    .get::<bool>()
                    .copied()
                    .unwrap_or(false);
            }
            if let Some(rel_spec) = layer.get_relationship_at_path(self.path()) {
                return rel_spec
                    .spec()
                    .get_field(&custom_key)
                    .get::<bool>()
                    .copied()
                    .unwrap_or(false);
            }
        }
        false
    }

    /// Returns true if this property name uses namespaces.
    pub fn is_namespaced(&self) -> bool {
        self.inner.path.get_name().contains(':')
    }

    /// Returns the display name (from displayName metadata).
    ///
    /// Falls back to the property name if no displayName metadata is authored.
    /// Matches C++ `UsdProperty::GetDisplayName()`.
    pub fn display_name(&self) -> String {
        let key = Token::new("displayName");
        self.inner
            .get_metadata(&key)
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_else(|| self.inner.path.get_name().to_string())
    }

    /// Returns the display group for this property.
    ///
    /// Display groups are used by UI to organize properties.
    /// Matches C++ `UsdProperty::GetDisplayGroup()`.
    pub fn get_display_group(&self) -> String {
        let key = Token::new("displayGroup");
        self.inner
            .get_metadata(&key)
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_default()
    }

    /// Sets the display group for this property.
    ///
    /// Matches C++ `UsdProperty::SetDisplayGroup()`.
    pub fn set_display_group(&self, group: &str) -> bool {
        let key = Token::new("displayGroup");
        self.inner
            .set_metadata(&key, Value::from(group.to_string()))
    }

    /// Clears the display group for this property.
    ///
    /// Matches C++ `UsdProperty::ClearDisplayGroup()`.
    pub fn clear_display_group(&self) -> bool {
        let key = Token::new("displayGroup");
        self.inner.clear_metadata(&key)
    }

    /// Returns true if a display group has been authored.
    ///
    /// Matches C++ `UsdProperty::HasAuthoredDisplayGroup()`.
    pub fn has_authored_display_group(&self) -> bool {
        let key = Token::new("displayGroup");
        self.inner.has_authored_metadata(&key)
    }

    /// Returns nested display groups by splitting on ':'.
    ///
    /// Matches C++ `UsdProperty::GetNestedDisplayGroups()`.
    pub fn get_nested_display_groups(&self) -> Vec<String> {
        let group = self.get_display_group();
        if group.is_empty() {
            return Vec::new();
        }
        group.split(':').map(|s| s.to_string()).collect()
    }

    /// Sets nested display groups by joining with ':'.
    ///
    /// Matches C++ `UsdProperty::SetNestedDisplayGroups()`.
    pub fn set_nested_display_groups(&self, groups: &[String]) -> bool {
        let group = groups.join(":");
        self.set_display_group(&group)
    }

    /// Returns a description of this property.
    pub fn description(&self) -> String {
        if self.is_valid() {
            format!("Property at {}", self.path().get_string())
        } else {
            "Invalid property".to_string()
        }
    }

    // =========================================================================
    // Metadata Access (delegated to Object)
    // =========================================================================

    /// Returns the value of a metadata field by dictionary key path.
    pub fn get_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> Option<Value> {
        self.inner.get_metadata_by_dict_key(key, key_path)
    }

    /// Sets a metadata value by dictionary key path.
    pub fn set_metadata_by_dict_key(&self, key: &Token, key_path: &Token, value: Value) -> bool {
        self.inner.set_metadata_by_dict_key(key, key_path, value)
    }

    /// Returns true if a metadata dictionary key path exists.
    pub fn has_metadata_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        self.inner.has_metadata_dict_key(key, key_path)
    }

    /// Clears a metadata value by dictionary key path.
    pub fn clear_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        self.inner.clear_metadata_by_dict_key(key, key_path)
    }

    /// Returns metadata value for a given key.
    pub fn get_metadata(&self, key: &Token) -> Option<Value> {
        self.inner.get_metadata(key)
    }

    /// Composed metadata map (matches C++ `UsdProperty::GetAllMetadata`).
    pub fn get_all_metadata(&self) -> super::object::MetadataValueMap {
        self.inner.get_all_metadata()
    }

    /// Sets metadata value for a given key.
    pub fn set_metadata(&self, key: &Token, value: Value) -> bool {
        self.inner.set_metadata(key, value)
    }

    /// Clears metadata value for a given key.
    pub fn clear_metadata(&self, key: &Token) -> bool {
        self.inner.clear_metadata(key)
    }

    /// Returns true if metadata is authored for the given key.
    pub fn has_authored_metadata(&self, key: &Token) -> bool {
        self.inner.has_authored_metadata(key)
    }

    /// Returns the property spec stack from strongest to weakest layer.
    ///
    /// Uses PrimIndex-based Resolver walk to find specs across all composed
    /// layers (including payloads, references). Matches C++ `_PropertyStackResolver`.
    pub fn get_property_stack(&self) -> Vec<usd_sdf::PropertySpec> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };
        let mut specs = Vec::new();
        let prop_name = self.name();
        let prim_path = self.prim_path();

        // Use PrimIndex-based Resolver (C++ _PropertyStackResolver)
        let prim_index = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index())
            .map(std::sync::Arc::new);

        if let Some(ref prim_index) = prim_index {
            let mut resolver = super::resolver::Resolver::new(prim_index, true);
            let mut is_new_node = true;
            let mut spec_path: Option<usd_sdf::Path> = None;

            while resolver.is_valid() {
                if is_new_node {
                    spec_path = resolver.get_local_path_for_property(&prop_name);
                }
                if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                    if let Some(attr_spec) = layer.get_attribute_at_path(sp) {
                        specs.push(usd_sdf::PropertySpec::new(attr_spec.as_spec().clone()));
                    } else if let Some(rel_spec) = layer.get_relationship_at_path(sp) {
                        specs.push(usd_sdf::PropertySpec::new(rel_spec.spec().clone()));
                    }
                }
                is_new_node = resolver.next_layer();
            }
        } else {
            // Fallback: root layer stack
            for layer in stage.layer_stack() {
                if let Some(attr_spec) = layer.get_attribute_at_path(self.path()) {
                    specs.push(usd_sdf::PropertySpec::new(attr_spec.as_spec().clone()));
                } else if let Some(rel_spec) = layer.get_relationship_at_path(self.path()) {
                    specs.push(usd_sdf::PropertySpec::new(rel_spec.spec().clone()));
                }
            }
        }
        specs
    }

    /// Sets the `custom` flag on this property in the current edit target.
    ///
    /// Matches C++ `UsdProperty::SetCustom(bool isCustom)`.
    pub fn set_custom(&self, is_custom: bool) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        let custom_token = usd_tf::Token::new("custom");
        let val = Value::from(is_custom);

        if let Some(mut attr_spec) = layer.get_attribute_at_path(self.path()) {
            attr_spec.as_spec_mut().set_field(&custom_token, val);
            return true;
        }
        if let Some(mut rel_spec) = layer.get_relationship_at_path(self.path()) {
            rel_spec.spec_mut().set_field(&custom_token, val);
            return true;
        }
        false
    }

    /// Returns true if this property is defined in any layer or in a schema.
    ///
    /// Uses PrimIndex-based resolution to check ALL composed layers
    /// (including payloads, references). Matches C++ `UsdProperty::IsDefined()`.
    pub fn is_defined(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };

        let prim_path = self.prim_path();
        let prop_name_str = self.name();
        let spec = stage.get_defining_spec_type(&prim_path, prop_name_str.get_text());
        if spec != usd_sdf::SpecType::Unknown {
            return true;
        }

        // Check schema registry (full PrimDefinition then lightweight)
        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let type_name = prim.type_name();
            if !type_name.is_empty() {
                let registry = super::schema_registry::SchemaRegistry::get_instance();
                if let Some(def) = registry.find_concrete_prim_definition(&type_name) {
                    let prop_name = self.name();
                    if def.property_names().contains(&prop_name) {
                        return true;
                    }
                }
                return super::schema_registry::schema_has_property(
                    &type_name,
                    prop_name_str.get_text(),
                );
            }
        }

        false
    }

    // =========================================================================
    // Property Stack with Layer Offsets
    // =========================================================================

    /// Returns the property spec stack paired with cumulative layer offsets.
    ///
    /// Each entry is (PropertySpec, LayerOffset) from strongest to weakest.
    /// Uses PrimIndex-based Resolver walk. Matches C++ `GetPropertyStackWithLayerOffsets()`.
    pub fn get_property_stack_with_layer_offsets(
        &self,
    ) -> Vec<(usd_sdf::PropertySpec, usd_sdf::LayerOffset)> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let prop_name = self.name();
        let prim_path = self.prim_path();

        let prim_index = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index())
            .map(std::sync::Arc::new);

        if let Some(ref prim_index) = prim_index {
            let mut resolver = super::resolver::Resolver::new(prim_index, true);
            let mut is_new_node = true;
            let mut spec_path: Option<usd_sdf::Path> = None;

            while resolver.is_valid() {
                if is_new_node {
                    spec_path = resolver.get_local_path_for_property(&prop_name);
                }
                if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                    let offset = resolver.get_layer_to_stage_offset();
                    if let Some(attr_spec) = layer.get_attribute_at_path(sp) {
                        result.push((
                            usd_sdf::PropertySpec::new(attr_spec.as_spec().clone()),
                            offset,
                        ));
                    } else if let Some(rel_spec) = layer.get_relationship_at_path(sp) {
                        result.push((usd_sdf::PropertySpec::new(rel_spec.spec().clone()), offset));
                    }
                }
                is_new_node = resolver.next_layer();
            }
        } else {
            // Fallback: root layer stack
            for layer in stage.layer_stack() {
                let offset = usd_sdf::LayerOffset::identity();
                if let Some(attr_spec) = layer.get_attribute_at_path(self.path()) {
                    result.push((
                        usd_sdf::PropertySpec::new(attr_spec.as_spec().clone()),
                        offset,
                    ));
                } else if let Some(rel_spec) = layer.get_relationship_at_path(self.path()) {
                    result.push((usd_sdf::PropertySpec::new(rel_spec.spec().clone()), offset));
                }
            }
        }
        result
    }

    // =========================================================================
    // Flattening
    // =========================================================================

    /// Flattens this property to a property spec with the same name beneath
    /// the given parent prim in the edit target of its owning stage.
    ///
    /// Authors all resolved values and metadata into the destination spec.
    /// Matches C++ `UsdProperty::FlattenTo(const UsdPrim&) const`.
    pub fn flatten_to(&self, parent: &super::prim::Prim) -> Property {
        let prop_name = self.name();
        self.flatten_to_named(parent, &prop_name)
    }

    /// Flattens this property to a property spec with the given name beneath
    /// the given parent prim.
    ///
    /// Matches C++ `UsdProperty::FlattenTo(const UsdPrim&, const TfToken&) const`.
    pub fn flatten_to_named(&self, parent: &super::prim::Prim, prop_name: &Token) -> Property {
        let Some(stage) = parent.stage() else {
            return Property::invalid();
        };
        let dest_path = parent.path().append_property(prop_name.get_text());
        let Some(dest_path) = dest_path else {
            return Property::invalid();
        };

        self.flatten_to_path(&stage, &dest_path)
    }

    /// Flattens this property to an existing property's location.
    ///
    /// Matches C++ `UsdProperty::FlattenTo(const UsdProperty&) const`.
    pub fn flatten_to_property(&self, property: &Property) -> Property {
        let Some(stage) = property.stage() else {
            return Property::invalid();
        };
        self.flatten_to_path(&stage, property.path())
    }

    /// Internal: flatten to a destination path on a stage.
    fn flatten_to_path(
        &self,
        dest_stage: &std::sync::Arc<super::object::Stage>,
        dest_path: &Path,
    ) -> Property {
        let edit_target = dest_stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return Property::invalid();
        };
        let spec_path = edit_target.map_to_spec_path(dest_path);

        // Copy property specs from source property stack to destination
        let specs = self.get_property_stack();
        if specs.is_empty() {
            return Property::invalid();
        }

        // Get the strongest spec to determine property type and copy metadata
        let strongest = &specs[0];
        let spec_data = strongest.spec();

        // Author the destination spec: copy all fields from strongest opinion
        for field_name in spec_data.list_fields() {
            let val = spec_data.get_field(&field_name);
            if !val.is_empty() {
                layer.set_field(&spec_path, &field_name, val);
            }
        }

        Property::new(std::sync::Arc::downgrade(dest_stage), dest_path.clone())
    }
}

impl PartialEq for Property {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Property {}

impl std::hash::Hash for Property {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl From<super::relationship::Relationship> for Property {
    fn from(rel: super::relationship::Relationship) -> Self {
        rel.into_property()
    }
}

impl From<super::attribute::Attribute> for Property {
    fn from(attr: super::attribute::Attribute) -> Self {
        attr.into_property()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_property() {
        let prop = Property::invalid();
        assert!(!prop.is_valid());
    }

    #[test]
    fn test_property_path() {
        let path = Path::from_string("/World.visibility").unwrap();
        let prop = Property::new(Weak::new(), path.clone());
        assert_eq!(prop.path(), &path);
    }

    #[test]
    fn test_property_name() {
        let path = Path::from_string("/World.visibility").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert_eq!(prop.name().get_text(), "visibility");
    }

    #[test]
    fn test_namespaced_property() {
        let path = Path::from_string("/World.primvars:displayColor").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert!(prop.is_namespaced());
        assert_eq!(prop.namespace().get_text(), "primvars");
        assert_eq!(prop.base_name().get_text(), "displayColor");
    }

    #[test]
    fn test_split_name() {
        let path = Path::from_string("/World.foo:bar:baz").unwrap();
        let prop = Property::new(Weak::new(), path);
        let parts = prop.split_name();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].get_text(), "foo");
        assert_eq!(parts[1].get_text(), "bar");
        assert_eq!(parts[2].get_text(), "baz");
    }

    #[test]
    fn test_prim_path() {
        let path = Path::from_string("/World/Cube.size").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert_eq!(prop.prim_path().get_string(), "/World/Cube");
    }

    #[test]
    fn test_is_attribute_and_is_relationship() {
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        // Default: is_attribute returns true, is_relationship returns false
        assert!(prop.is_attribute());
        assert!(!prop.is_relationship());
    }

    #[test]
    fn test_as_attribute_as_relationship() {
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        // Default is_attribute() == true => as_attribute returns Some
        assert!(prop.as_attribute().is_some());
        assert!(prop.as_relationship().is_none());
    }

    #[test]
    fn test_get_property_stack() {
        // Without a valid stage, property stack should be empty
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert!(prop.get_property_stack().is_empty());
    }

    #[test]
    fn test_set_custom() {
        // Without a valid stage, set_custom should return false
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert!(!prop.set_custom(true));
    }

    #[test]
    fn test_is_defined() {
        // Without a valid stage, is_defined should return false
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert!(!prop.is_defined());
    }

    // M4: Display group API
    #[test]
    fn test_display_group_default_empty() {
        // Without a stage, display group should be empty
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert!(prop.get_display_group().is_empty());
        assert!(!prop.has_authored_display_group());
    }

    #[test]
    fn test_nested_display_groups_empty() {
        let path = Path::from_string("/World.vis").unwrap();
        let prop = Property::new(Weak::new(), path);
        assert!(prop.get_nested_display_groups().is_empty());
    }

    // M4: is_attribute/is_relationship with actual stage
    #[test]
    fn test_is_attribute_with_stage() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;
        use usd_sdf::value_type_registry::ValueTypeRegistry;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let type_name = ValueTypeRegistry::instance().find_type("float");
        let attr = prim
            .create_attribute("size", &type_name, false, None)
            .unwrap();

        // Get as property and check type detection
        let prop = Property::new(std::sync::Arc::downgrade(&stage), attr.path().clone());
        assert!(prop.is_attribute());
        assert!(!prop.is_relationship());
    }

    #[test]
    fn test_is_relationship_with_stage() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Mesh").unwrap();

        let rel = prim.create_relationship("material:binding", false).unwrap();

        // Get as property and check type detection
        let prop = Property::new(std::sync::Arc::downgrade(&stage), rel.path().clone());
        assert!(prop.is_relationship());
        assert!(!prop.is_attribute());
    }
}
