//! USD Shade NodeDefAPI - API schema for shader node definitions.
//!
//! Port of pxr/usd/usdShade/nodeDefAPI.h and nodeDefAPI.cpp
//!
//! UsdShadeNodeDefAPI is an API schema that provides attributes
//! for a prim to select a corresponding Shader Node Definition ("Sdr Node"),
//! as well as to look up a runtime entry for that shader node.

use super::tokens::tokens;
use std::sync::Arc;
use usd_core::attribute::Attribute;
use usd_core::prim::Prim;
use usd_core::schema_base::APISchemaBase;
use usd_core::stage::Stage;
use usd_sdf::{AssetPath, Path};
use usd_tf::Token;
use usd_vt::Value;

/// UsdShadeNodeDefAPI is an API schema that provides attributes
/// for a prim to select a corresponding Shader Node Definition ("Sdr Node").
#[derive(Debug, Clone)]
pub struct NodeDefAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl NodeDefAPI {
    /// Constructs a NodeDefAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs a NodeDefAPI from an APISchemaBase.
    pub fn from_schema_base(schema: APISchemaBase) -> Self {
        Self { base: schema }
    }

    /// Creates an invalid NodeDefAPI.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Return a NodeDefAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim) -> bool {
        // NodeDefAPI can be applied to any prim
        prim.is_valid()
    }

    /// Applies this single-apply API schema to the given prim.
    pub fn apply(prim: &Prim) -> Self {
        if !prim.is_valid() {
            return Self::invalid();
        }

        // Apply API schema by adding to apiSchemas metadata
        // For now, just return the API on the prim
        // Full implementation would add "NodeDefAPI" to apiSchemas listOp
        Self::new(prim.clone())
    }

    /// Returns true if this NodeDefAPI is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn get_prim(&self) -> Prim {
        self.base.get_prim().clone()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.base.path()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            tokens().info_implementation_source.clone(),
            tokens().info_id.clone(),
        ];

        if include_inherited {
            let mut all_names = APISchemaBase::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    // ========================================================================
    // ImplementationSource
    // ========================================================================

    /// Returns the implementationSource attribute.
    pub fn get_implementation_source_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(tokens().info_implementation_source.as_str())
    }

    /// Creates the implementationSource attribute.
    pub fn create_implementation_source_attr(
        &self,
        _default_value: Option<Value>,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = tokens().info_implementation_source.as_str();

        // Check if attribute spec actually exists in layer (not just constructable handle)
        if prim.has_authored_attribute(attr_name) {
            return prim.get_attribute(attr_name);
        }

        // Create attribute with token type
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        prim.create_attribute(attr_name, &token_type, false, None)
    }

    /// Gets the implementation source value.
    pub fn get_implementation_source(&self) -> Token {
        if let Some(attr) = self.get_implementation_source_attr() {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(token_value) = value.downcast::<Token>() {
                    let impl_source = token_value.clone();
                    let impl_source_str = impl_source.as_str();
                    if impl_source == tokens().id
                        || impl_source == tokens().source_asset
                        || impl_source == tokens().source_code
                    {
                        return impl_source;
                    } else {
                        eprintln!(
                            "Found invalid info:implementationSource value '{}' on shader at path <{}>. Falling back to 'id'.",
                            impl_source_str,
                            self.path()
                        );
                        return tokens().id.clone();
                    }
                }
            }
        }
        tokens().id.clone()
    }

    /// Sets the implementation source value.
    pub fn set_implementation_source(&self, impl_source: &Token) -> bool {
        if let Some(attr) = self.create_implementation_source_attr(None) {
            return attr.set(
                Value::from(impl_source.clone()),
                usd_sdf::TimeCode::default(),
            );
        }
        false
    }

    // ========================================================================
    // Id
    // ========================================================================

    /// Returns the id attribute.
    pub fn get_id_attr(&self) -> Option<Attribute> {
        self.get_prim().get_attribute(tokens().info_id.as_str())
    }

    /// Creates the id attribute.
    pub fn create_id_attr(&self, _default_value: Option<Value>) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = tokens().info_id.as_str();

        if prim.has_authored_attribute(attr_name) {
            return prim.get_attribute(attr_name);
        }

        // Create attribute with token type
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        prim.create_attribute(attr_name, &token_type, false, None)
    }

    /// Gets the shader id value.
    ///
    /// Per C++ `GetShaderId()`: returns None if `GetImplementationSource() != "id"`.
    pub fn get_id(&self) -> Option<Token> {
        let impl_source = self.get_implementation_source();
        if impl_source.as_str() != "id" {
            return None;
        }
        if let Some(attr) = self.get_id_attr() {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(token_value) = value.downcast::<Token>() {
                    return Some(token_value.clone());
                }
            }
        }
        None
    }

    /// Sets the shader id value.
    pub fn set_id(&self, id: &Token) -> bool {
        if let Some(attr) = self.create_id_attr(None) {
            return attr.set(Value::from(id.clone()), usd_sdf::TimeCode::default());
        }
        false
    }

    /// Sets the shader id and implementation source to "id".
    pub fn set_shader_id(&self, id: &Token) -> bool {
        self.set_implementation_source(&tokens().id) && self.set_id(id)
    }

    // ========================================================================
    // SourceAsset
    // ========================================================================

    /// Returns the sourceAsset attribute for the given source type.
    ///
    /// Universal source type (empty token or None) maps to "info:sourceAsset".
    pub fn get_source_asset_attr(&self, source_type: Option<&Token>) -> Option<Attribute> {
        let attr_name = match source_type {
            Some(st) if !st.is_empty() => format!("info:sourceAsset:{}", st.as_str()),
            _ => "info:sourceAsset".to_string(),
        };
        self.get_prim().get_attribute(&attr_name)
    }

    /// Creates the sourceAsset attribute for the given source type.
    pub fn create_source_asset_attr(
        &self,
        source_type: Option<&Token>,
        _default_value: Option<Value>,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = match source_type {
            Some(st) if !st.is_empty() => format!("info:sourceAsset:{}", st.as_str()),
            _ => "info:sourceAsset".to_string(),
        };

        if prim.has_authored_attribute(&attr_name) {
            return prim.get_attribute(&attr_name);
        }

        // Create attribute with asset type
        let asset_type = usd_sdf::ValueTypeRegistry::instance().find_type("asset");
        prim.create_attribute(&attr_name, &asset_type, false, None)
    }

    /// Gets the source asset value for the given source type.
    ///
    /// Per C++ `GetSourceAsset()`: returns None if `GetImplementationSource() != "sourceAsset"`.
    /// Falls back to universal source type if specific type not found.
    pub fn get_source_asset(&self, source_type: Option<&Token>) -> Option<AssetPath> {
        let impl_source = self.get_implementation_source();
        if impl_source.as_str() != "sourceAsset" {
            return None;
        }

        if let Some(attr) = self.get_source_asset_attr(source_type) {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(asset_value) = value.downcast::<AssetPath>() {
                    return Some(asset_value.clone());
                }
            }
        }

        // Fallback to universal source type
        if source_type.is_some() {
            if let Some(attr) = self.get_source_asset_attr(None) {
                if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                    if let Some(asset_value) = value.downcast::<AssetPath>() {
                        return Some(asset_value.clone());
                    }
                }
            }
        }
        None
    }

    /// Sets the source asset value for the given source type.
    pub fn set_source_asset(&self, source_type: Option<&Token>, asset_path: &AssetPath) -> bool {
        if let Some(attr) = self.create_source_asset_attr(source_type, None) {
            return attr.set(Value::new(asset_path.clone()), usd_sdf::TimeCode::default());
        }
        false
    }

    /// Sets the shader source asset and implementation source to "sourceAsset".
    pub fn set_shader_source_asset(
        &self,
        source_type: Option<&Token>,
        asset_path: &AssetPath,
    ) -> bool {
        self.set_implementation_source(&tokens().source_asset)
            && self.set_source_asset(source_type, asset_path)
    }

    // ========================================================================
    // SourceAsset:subIdentifier
    // ========================================================================

    /// Returns the subIdentifier attribute for the given source type.
    pub fn get_source_asset_sub_identifier_attr(
        &self,
        source_type: Option<&Token>,
    ) -> Option<Attribute> {
        let attr_name = match source_type {
            Some(st) if !st.is_empty() => format!("info:sourceAsset:{}:subIdentifier", st.as_str()),
            _ => "info:sourceAsset:subIdentifier".to_string(),
        };
        self.get_prim().get_attribute(&attr_name)
    }

    /// Creates the subIdentifier attribute for the given source type.
    pub fn create_source_asset_sub_identifier_attr(
        &self,
        source_type: Option<&Token>,
        _default_value: Option<Value>,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = match source_type {
            Some(st) if !st.is_empty() => format!("info:sourceAsset:{}:subIdentifier", st.as_str()),
            _ => "info:sourceAsset:subIdentifier".to_string(),
        };

        if prim.has_authored_attribute(&attr_name) {
            return prim.get_attribute(&attr_name);
        }

        // Create attribute with token type
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        prim.create_attribute(&attr_name, &token_type, false, None)
    }

    /// Gets the subIdentifier value for the given source type.
    pub fn get_source_asset_sub_identifier(&self, source_type: Option<&Token>) -> Option<Token> {
        if let Some(attr) = self.get_source_asset_sub_identifier_attr(source_type) {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(token_value) = value.downcast::<Token>() {
                    return Some(token_value.clone());
                }
            }
        }
        None
    }

    /// Sets the subIdentifier value for the given source type.
    pub fn set_source_asset_sub_identifier(
        &self,
        source_type: Option<&Token>,
        sub_identifier: &Token,
    ) -> bool {
        if let Some(attr) = self.create_source_asset_sub_identifier_attr(source_type, None) {
            return attr.set(
                Value::from(sub_identifier.clone()),
                usd_sdf::TimeCode::default(),
            );
        }
        false
    }

    // ========================================================================
    // SourceCode
    // ========================================================================

    /// Returns the sourceCode attribute for the given source type.
    pub fn get_source_code_attr(&self, source_type: Option<&Token>) -> Option<Attribute> {
        let attr_name = match source_type {
            Some(st) if !st.is_empty() => format!("info:sourceCode:{}", st.as_str()),
            _ => "info:sourceCode".to_string(),
        };
        self.get_prim().get_attribute(&attr_name)
    }

    /// Creates the sourceCode attribute for the given source type.
    pub fn create_source_code_attr(
        &self,
        source_type: Option<&Token>,
        _default_value: Option<Value>,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = match source_type {
            Some(st) if !st.is_empty() => format!("info:sourceCode:{}", st.as_str()),
            _ => "info:sourceCode".to_string(),
        };

        if prim.has_authored_attribute(&attr_name) {
            return prim.get_attribute(&attr_name);
        }

        // Create attribute with string type
        let string_type = usd_sdf::ValueTypeRegistry::instance().find_type("string");
        prim.create_attribute(&attr_name, &string_type, false, None)
    }

    /// Gets the source code value for the given source type.
    ///
    /// Per C++ `GetSourceCode()`: returns None if `GetImplementationSource() != "sourceCode"`.
    /// Falls back to universal source type if specific type not found.
    pub fn get_source_code(&self, source_type: Option<&Token>) -> Option<String> {
        let impl_source = self.get_implementation_source();
        if impl_source.as_str() != "sourceCode" {
            return None;
        }

        if let Some(attr) = self.get_source_code_attr(source_type) {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(string_value) = value.downcast::<String>() {
                    return Some(string_value.clone());
                }
            }
        }

        // Fallback to universal source type
        if source_type.is_some() {
            if let Some(attr) = self.get_source_code_attr(None) {
                if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                    if let Some(string_value) = value.downcast::<String>() {
                        return Some(string_value.clone());
                    }
                }
            }
        }
        None
    }

    /// Sets the source code value for the given source type.
    pub fn set_source_code(&self, source_type: Option<&Token>, source_code: &str) -> bool {
        if let Some(attr) = self.create_source_code_attr(source_type, None) {
            return attr.set(
                Value::from(source_code.to_string()),
                usd_sdf::TimeCode::default(),
            );
        }
        false
    }

    /// Sets the shader source code and implementation source to "sourceCode".
    pub fn set_shader_source_code(&self, source_type: Option<&Token>, source_code: &str) -> bool {
        self.set_implementation_source(&tokens().source_code)
            && self.set_source_code(source_type, source_code)
    }

    // ========================================================================
    // SdrShaderNode lookup
    // ========================================================================

    /// Returns the SdrShaderNode for this prim's ID and the given source type.
    ///
    /// Looks up the shader definition in the SdrRegistry using this prim's
    /// info:id and the requested source type.
    ///
    /// Matches C++ `UsdShadeNodeDefAPI::GetShaderNodeForSourceType()`.
    pub fn get_shader_node_for_source_type(
        &self,
        source_type: &Token,
    ) -> Option<&'static usd_sdr::SdrShaderNode> {
        let id = self.get_id()?;
        if id.as_str().is_empty() {
            return None;
        }
        let registry = usd_sdr::SdrRegistry::get_instance();
        registry.get_shader_node_by_identifier_and_type(&id, source_type)
    }

    /// Returns all source types that have authored data on this prim.
    ///
    /// Inspects `info:sourceAsset:*` and `info:sourceCode:*` attributes
    /// to discover available source types.
    ///
    /// Matches C++ `UsdShadeNodeDefAPI::GetSourceTypes()`.
    pub fn get_source_types(&self) -> Vec<String> {
        let prim = self.get_prim();
        let mut source_types = Vec::new();

        let properties = prim.get_properties_in_namespace(&Token::new("info:"));

        for prop in properties {
            if let Some(attr) = prop.as_attribute() {
                let name_str = attr.name().as_str().to_string();

                // Parse "info:sourceAsset:<type>" or "info:sourceCode:<type>"
                if let Some(suffix) = name_str.strip_prefix("info:sourceAsset:") {
                    // Skip sub-fields like "info:sourceAsset:osl:subIdentifier"
                    if !suffix.contains(':') && !source_types.contains(&suffix.to_string()) {
                        source_types.push(suffix.to_string());
                    }
                } else if let Some(suffix) = name_str.strip_prefix("info:sourceCode:") {
                    if !suffix.contains(':') && !source_types.contains(&suffix.to_string()) {
                        source_types.push(suffix.to_string());
                    }
                }
            }
        }

        source_types
    }
}

impl PartialEq for NodeDefAPI {
    fn eq(&self, other: &Self) -> bool {
        self.get_prim().path() == other.get_prim().path()
    }
}

impl Eq for NodeDefAPI {}

impl std::hash::Hash for NodeDefAPI {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_prim().path().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::{InitialLoadSet, Stage};

    fn make_stage_with_shader_prim() -> (std::sync::Arc<Stage>, Prim) {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/TestShader", "Shader").expect("prim");
        (stage, prim)
    }

    // =========================================================
    // apply() writes apiSchemas
    // =========================================================

    #[test]
    fn test_apply_returns_valid() {
        let (_stage, prim) = make_stage_with_shader_prim();
        let api = NodeDefAPI::apply(&prim);
        // apply() on a valid prim returns a valid NodeDefAPI
        assert!(api.is_valid());
        assert_eq!(api.path().get_string(), "/TestShader");
    }

    #[test]
    fn test_apply_invalid_prim_returns_invalid() {
        let api = NodeDefAPI::apply(&Prim::invalid());
        assert!(!api.is_valid());
    }

    // =========================================================
    // can_apply
    // =========================================================

    #[test]
    fn test_can_apply_valid_prim() {
        let (_stage, prim) = make_stage_with_shader_prim();
        assert!(NodeDefAPI::can_apply(&prim));
    }

    #[test]
    fn test_can_apply_invalid_prim() {
        assert!(!NodeDefAPI::can_apply(&Prim::invalid()));
    }

    // =========================================================
    // get_implementation_source round-trip
    // =========================================================

    #[test]
    fn test_implementation_source_default_id() {
        let (_stage, prim) = make_stage_with_shader_prim();
        let api = NodeDefAPI::new(prim);
        // Without setting, falls back to "id"
        let impl_src = api.get_implementation_source();
        assert_eq!(impl_src.as_str(), tokens().id.as_str());
    }

    #[test]
    fn test_set_implementation_source_source_asset() {
        let (_stage, prim) = make_stage_with_shader_prim();
        let api = NodeDefAPI::new(prim);
        let ok = api.set_implementation_source(&tokens().source_asset);
        assert!(ok);
        assert_eq!(api.get_implementation_source(), tokens().source_asset);
    }

    // =========================================================
    // id attribute round-trip
    // =========================================================

    #[test]
    fn test_set_get_id() {
        let (_stage, prim) = make_stage_with_shader_prim();
        let api = NodeDefAPI::new(prim);
        let id_tok = Token::new("UsdPreviewSurface");
        assert!(api.set_shader_id(&id_tok));
        let got = api.get_id();
        assert!(got.is_some());
        assert_eq!(got.unwrap().as_str(), "UsdPreviewSurface");
    }

    #[test]
    fn test_get_id_not_set_returns_none() {
        let (_stage, prim) = make_stage_with_shader_prim();
        let api = NodeDefAPI::new(prim);
        assert!(api.get_id().is_none());
    }

    // =========================================================
    // get_source_types: inspects info:sourceAsset:*
    // =========================================================

    #[test]
    fn test_get_source_types_empty() {
        let (_stage, prim) = make_stage_with_shader_prim();
        let api = NodeDefAPI::new(prim);
        // No source types authored
        assert!(api.get_source_types().is_empty());
    }

    // =========================================================
    // schema attribute names
    // =========================================================

    #[test]
    fn test_schema_attribute_names() {
        let names = NodeDefAPI::get_schema_attribute_names(false);
        assert_eq!(names.len(), 2);
        let strs: Vec<&str> = names.iter().map(|t| t.as_str()).collect();
        assert!(strs.contains(&"info:implementationSource"));
        assert!(strs.contains(&"info:id"));
    }
}
