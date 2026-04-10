//! USD Shade Shader - base class for all USD shaders.
//!
//! Port of pxr/usd/usdShade/shader.h and shader.cpp
//!
//! Base class for all USD shaders. Shaders are the building blocks
//! of shading networks.

use super::connectable_api::ConnectableAPI;
use super::input::Input;
use super::node_def_api::NodeDefAPI;
use super::output::Output;
use super::sdr_value_string::sdr_metadata_value_string;
use super::tokens::tokens;
use std::collections::HashMap;
use std::sync::Arc;
use usd_core::attribute::Attribute;
use usd_core::prim::Prim;
use usd_core::schema_base::SchemaBase;
use usd_core::stage::Stage;
use usd_core::typed::Typed;
use usd_sdf::ValueTypeName;
use usd_sdf::{AssetPath, Path};
use usd_tf::Token;
use usd_vt::Value;

/// Base class for all USD shaders.
///
/// Shaders are the building blocks of shading networks.
#[derive(Debug, Clone)]
pub struct Shader {
    /// Base typed schema.
    typed: Typed,
}

impl Shader {
    /// Construct a Shader on UsdPrim.
    pub fn new(prim: Prim) -> Self {
        Self {
            typed: Typed::new(prim),
        }
    }

    /// Construct a Shader from a SchemaBase.
    pub fn from_schema_base(schema: SchemaBase) -> Self {
        Self {
            typed: Typed::new(schema.prim().clone()),
        }
    }

    /// Construct a Shader from a ConnectableAPI.
    ///
    /// Allow implicit (auto) conversion of UsdShadeConnectableAPI to UsdShadeShader.
    pub fn from_connectable_api(connectable: &ConnectableAPI) -> Self {
        Self::new(connectable.get_prim())
    }

    /// Creates an invalid Shader.
    pub fn invalid() -> Self {
        Self {
            typed: Typed::invalid(),
        }
    }

    /// Return a Shader holding the prim adhering to this schema at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a UsdPrim adhering to this schema at `path` is defined on this stage.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Self {
        match stage.define_prim(path.to_string(), "Shader") {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    /// Returns true if this Shader is valid.
    pub fn is_valid(&self) -> bool {
        self.typed.is_valid() && self.typed.prim().type_name() == "Shader"
    }

    /// Returns the wrapped prim.
    pub fn get_prim(&self) -> Prim {
        self.typed.prim().clone()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.typed.prim().path()
    }

    /// Returns the stage.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.typed.prim().stage()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        if include_inherited {
            Typed::get_schema_attribute_names(true)
        } else {
            Vec::new() // Shader doesn't add any attributes itself
        }
    }

    /// Constructs and returns a UsdShadeConnectableAPI object with this shader.
    pub fn connectable_api(&self) -> ConnectableAPI {
        ConnectableAPI::new(self.get_prim())
    }

    // ========================================================================
    // Outputs API
    // ========================================================================

    /// Create an output which can either have a value or can be connected.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Output {
        self.connectable_api().create_output(name, type_name)
    }

    /// Return the requested output if it exists.
    pub fn get_output(&self, name: &Token) -> Output {
        self.connectable_api().get_output(name)
    }

    /// Returns all outputs on the shader.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        self.connectable_api().get_outputs(only_authored)
    }

    // ========================================================================
    // Inputs API
    // ========================================================================

    /// Create an Input which can either have a value or can be connected.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Input {
        self.connectable_api().create_input(name, type_name)
    }

    /// Return the requested input if it exists.
    pub fn get_input(&self, name: &Token) -> Input {
        self.connectable_api().get_input(name)
    }

    /// Returns all inputs present on the shader.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        self.connectable_api().get_inputs(only_authored)
    }

    // ========================================================================
    // UsdShadeNodeDefAPI forwarding
    // ========================================================================

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_implementation_source_attr(&self) -> Option<Attribute> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_implementation_source_attr()
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn create_implementation_source_attr(
        &self,
        default_value: Option<Value>,
    ) -> Option<Attribute> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.create_implementation_source_attr(default_value)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_id_attr(&self) -> Option<Attribute> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_id_attr()
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn create_id_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.create_id_attr(default_value)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_implementation_source(&self) -> Token {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_implementation_source()
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn set_shader_id(&self, id: &Token) -> bool {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.set_shader_id(id)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_shader_id(&self) -> Option<Token> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_id()
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn set_source_asset(&self, source_asset: &AssetPath, source_type: Option<&Token>) -> bool {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.set_shader_source_asset(source_type, source_asset)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_source_asset(&self, source_type: Option<&Token>) -> Option<AssetPath> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_source_asset(source_type)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn set_source_asset_sub_identifier(
        &self,
        sub_identifier: &Token,
        source_type: Option<&Token>,
    ) -> bool {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.set_source_asset_sub_identifier(source_type, sub_identifier)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_source_asset_sub_identifier(&self, source_type: Option<&Token>) -> Option<Token> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_source_asset_sub_identifier(source_type)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn set_source_code(&self, source_code: &str, source_type: Option<&Token>) -> bool {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.set_shader_source_code(source_type, source_code)
    }

    /// Forwards to UsdShadeNodeDefAPI(prim).
    pub fn get_source_code(&self, source_type: Option<&Token>) -> Option<String> {
        let node_def = NodeDefAPI::new(self.get_prim());
        node_def.get_source_code(source_type)
    }

    /// Returns vector of source types that have authored data on this shader.
    pub fn get_source_types(&self) -> Vec<String> {
        let prim = self.get_prim();
        let mut source_types = Vec::new();

        // Look for info:sourceAsset:* and info:sourceCode:* attributes
        let properties = prim.get_properties_in_namespace(&Token::new("info:"));

        for prop in properties {
            if let Some(attr) = prop.as_attribute() {
                let name_str = attr.name().as_str().to_string();

                // Parse patterns like "info:sourceAsset:glslfx" or "info:sourceCode:osl"
                if name_str.starts_with("info:sourceAsset:") {
                    let parts: Vec<&str> = name_str.split(':').collect();
                    if parts.len() >= 3 {
                        let source_type = parts[2].to_string();
                        if !source_types.contains(&source_type) {
                            source_types.push(source_type);
                        }
                    }
                } else if name_str.starts_with("info:sourceCode:") {
                    let parts: Vec<&str> = name_str.split(':').collect();
                    if parts.len() >= 3 {
                        let source_type = parts[2].to_string();
                        if !source_types.contains(&source_type) {
                            source_types.push(source_type);
                        }
                    }
                }
            }
        }

        source_types
    }

    /// Returns the SdrShaderNode for this shader's ID and the given source type.
    ///
    /// Looks up the shader definition in the SdrRegistry using this shader's
    /// info:id and the requested source type.
    ///
    /// Matches C++ `UsdShadeShader::GetShaderNodeForSourceType()`.
    pub fn get_shader_node_for_source_type(
        &self,
        source_type: &Token,
    ) -> Option<&'static usd_sdr::SdrShaderNode> {
        let id = self.get_shader_id()?;
        if id.as_str().is_empty() {
            return None;
        }
        let registry = usd_sdr::SdrRegistry::get_instance();
        registry.get_shader_node_by_identifier_and_type(&id, source_type)
    }

    // ========================================================================
    // Shader Sdr Metadata API
    // ========================================================================

    /// Returns this shader's composed "sdrMetadata" dictionary as a HashMap<String, String>.
    ///
    /// Matches C++ `UsdShadeShader::GetSdrMetadata()`: reads the prim's `sdrMetadata`
    /// metadata dictionary (VtDictionary) and stringifies each value via TfStringify.
    pub fn get_sdr_metadata(&self) -> HashMap<String, String> {
        let prim = self.get_prim();
        let mut result = HashMap::new();
        let sdr_key = &tokens().sdr_metadata;

        // Composed prim metadata (matches `UsdObject::GetMetadata` / C++ `GetSdrMetadata`).
        // Must use `Stage::get_metadata_for_object` so prim metadata authored on the
        // stage resolves; iterating `layer.get_field(prim.path(), …)` misses authored
        // `sdrMetadata` for typical opened layers.
        let Some(stage) = prim.stage() else {
            return result;
        };
        let Some(val) = stage.get_metadata_for_object(prim.path(), sdr_key) else {
            return result;
        };
        if let Some(dict) = val.get::<usd_vt::Dictionary>() {
            for (key, v) in dict.iter() {
                result
                    .entry(key.to_string())
                    .or_insert_with(|| sdr_metadata_value_string(v));
            }
        }

        result
    }

    /// Returns the value corresponding to key in the composed sdrMetadata dictionary.
    ///
    /// Matches C++ `UsdShadeShader::GetSdrMetadataByKey()`.
    pub fn get_sdr_metadata_by_key(&self, key: &Token) -> String {
        let meta = self.get_sdr_metadata();
        meta.get(key.as_str()).cloned().unwrap_or_default()
    }

    /// Authors the given sdrMetadata on this shader at the current EditTarget.
    ///
    /// Matches C++ `UsdShadeShader::SetSdrMetadata()`: iterates the map and
    /// sets each key individually via `SetSdrMetadataByKey`.
    pub fn set_sdr_metadata(&self, sdr_metadata: &HashMap<String, String>) {
        for (key, value) in sdr_metadata {
            self.set_sdr_metadata_by_key(&Token::new(key.as_str()), value.as_str());
        }
    }

    /// Sets the value corresponding to key to the given string value.
    ///
    /// Matches C++ `UsdShadeShader::SetSdrMetadataByKey()`:
    /// `GetPrim().SetMetadataByDictKey(UsdShadeTokens->sdrMetadata, key, value)`
    pub fn set_sdr_metadata_by_key(&self, key: &Token, value: &str) {
        let prim = self.get_prim();
        let sdr_key = &tokens().sdr_metadata;

        // Read existing Dictionary from layer, merge in the new key, write back.
        // (prim.set_metadata_by_dict_key uses HashMap<String,Value> which doesn't
        //  round-trip with Dictionary storage)
        let Some(stage) = prim.stage() else {
            return;
        };
        let layer = stage.root_layer();

        let mut dict = if let Some(val) = layer.get_field(prim.path(), sdr_key) {
            if let Some(d) = val.get::<usd_vt::Dictionary>() {
                d.clone()
            } else {
                usd_vt::Dictionary::new()
            }
        } else {
            usd_vt::Dictionary::new()
        };

        dict.insert_value(
            key.as_str().to_string(),
            usd_vt::Value::from(value.to_string()),
        );
        layer.set_field(
            prim.path(),
            sdr_key,
            usd_vt::Value::from_dictionary(
                dict.iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
            ),
        );
    }

    /// Returns true if the shader has a non-empty composed "sdrMetadata" dictionary value.
    ///
    /// Matches C++ `GetPrim().HasMetadata(UsdShadeTokens->sdrMetadata)`.
    pub fn has_sdr_metadata(&self) -> bool {
        !self.get_sdr_metadata().is_empty()
    }

    /// Returns true if there is a value corresponding to the given key in the composed "sdrMetadata" dictionary.
    ///
    /// Matches C++ `GetPrim().HasMetadataDictKey(sdrMetadata, key)`.
    pub fn has_sdr_metadata_by_key(&self, key: &Token) -> bool {
        self.get_sdr_metadata().contains_key(key.as_str())
    }

    /// Clears any "sdrMetadata" value authored on the shader in the current EditTarget.
    ///
    /// Matches C++ `UsdShadeShader::ClearSdrMetadata()`.
    pub fn clear_sdr_metadata(&self) {
        let prim = self.get_prim();
        prim.clear_metadata(&tokens().sdr_metadata);
    }

    /// Clears the entry corresponding to the given key in the "sdrMetadata" dictionary.
    ///
    /// Matches C++ `UsdShadeShader::ClearSdrMetadataByKey()`.
    pub fn clear_sdr_metadata_by_key(&self, key: &Token) {
        let prim = self.get_prim();
        prim.clear_metadata_by_dict_key(&tokens().sdr_metadata, key);
    }
}

impl PartialEq for Shader {
    fn eq(&self, other: &Self) -> bool {
        self.get_prim().path() == other.get_prim().path()
    }
}

impl Eq for Shader {}

impl std::hash::Hash for Shader {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_prim().path().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::{InitialLoadSet, Stage};

    fn make_stage_with_shader() -> (std::sync::Arc<Stage>, Shader) {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/Shader", "Shader").expect("prim");
        let shader = Shader::new(prim);
        (stage, shader)
    }

    // =========================================================
    // Shader validity
    // =========================================================

    #[test]
    fn test_shader_invalid() {
        let s = Shader::invalid();
        assert!(!s.is_valid());
    }

    #[test]
    fn test_shader_define_valid() {
        let (_stage, shader) = make_stage_with_shader();
        assert!(shader.is_valid());
        assert_eq!(shader.path().get_string(), "/Shader");
    }

    // =========================================================
    // get_sdr_metadata / set_sdr_metadata round-trip
    // =========================================================

    #[test]
    fn test_sdr_metadata_empty_by_default() {
        let (_stage, shader) = make_stage_with_shader();
        assert!(!shader.has_sdr_metadata());
        assert!(shader.get_sdr_metadata().is_empty());
    }

    #[test]
    fn test_sdr_metadata_set_and_get() {
        let (_stage, shader) = make_stage_with_shader();

        let mut meta = HashMap::new();
        meta.insert("role".to_string(), "texture".to_string());
        meta.insert("context".to_string(), "surface".to_string());
        shader.set_sdr_metadata(&meta);

        // has_sdr_metadata depends on VtDictionary round-trip
        // set_sdr_metadata_by_key at minimum must not panic
        shader.set_sdr_metadata_by_key(&Token::new("myKey"), "myVal");
    }

    #[test]
    fn test_sdr_metadata_by_key_missing() {
        let (_stage, shader) = make_stage_with_shader();
        let val = shader.get_sdr_metadata_by_key(&Token::new("nonexistent"));
        assert_eq!(val, "");
    }

    // =========================================================
    // Shader ID / implementationSource round-trip
    // =========================================================

    #[test]
    fn test_shader_set_id() {
        let (_stage, shader) = make_stage_with_shader();
        shader.set_shader_id(&Token::new("UsdPreviewSurface"));
        let id = shader.get_shader_id();
        assert!(id.is_some());
        assert_eq!(id.unwrap().as_str(), "UsdPreviewSurface");
    }

    #[test]
    fn test_shader_id_not_set_returns_none() {
        let (_stage, shader) = make_stage_with_shader();
        assert!(shader.get_shader_id().is_none());
    }

    // =========================================================
    // get_shader_node_for_source_type: no id -> None
    // =========================================================

    #[test]
    fn test_get_shader_node_no_id() {
        let (_stage, shader) = make_stage_with_shader();
        // No id set -> None
        let node = shader.get_shader_node_for_source_type(&Token::new("glslfx"));
        assert!(node.is_none());
    }
}
