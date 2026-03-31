//! USD Shade tokens - commonly used string tokens for usdShade module.
//!
//! Port of pxr/usd/usdShade/tokens.h

use std::sync::OnceLock;
use usd_tf::Token;

/// USD Shade module tokens.
pub struct UsdShadeTokens {
    /// "" - allPurpose token (empty string)
    pub all_purpose: Token,
    /// "bindMaterialAs" - bindMaterialAs metadata key
    pub bind_material_as: Token,
    /// "coordSys" - coordSys property namespace prefix
    pub coord_sys: Token,
    /// "coordSys:__INSTANCE_NAME__:binding" - coordSys multiple-apply template binding
    pub coord_sys_multiple_apply_template_binding: Token,
    /// "displacement" - displacement output terminal name
    pub displacement: Token,
    /// "fallbackStrength" - fallback strength sentinel value
    pub fallback_strength: Token,
    /// "full" - full material purpose and full connectability
    pub full: Token,
    /// "id" - fallback value for implementation source
    pub id: Token,
    /// "info:id" - info:id attribute name
    pub info_id: Token,
    /// "info:implementationSource" - info:implementationSource attribute name
    pub info_implementation_source: Token,
    /// "inputs:" - inputs namespace prefix
    pub inputs: Token,
    /// "interfaceOnly" - interfaceOnly connectability value
    pub interface_only: Token,
    /// "materialBind" - materialBind GeomSubset family name
    pub material_bind: Token,
    /// "material:binding" - material:binding relationship name
    pub material_binding: Token,
    /// "material:binding:collection" - material:binding:collection relationship name
    pub material_binding_collection: Token,
    /// "materialVariant" - materialVariant variant name
    pub material_variant: Token,
    /// "outputs:" - outputs namespace prefix
    pub outputs: Token,
    /// "outputs:displacement" - outputs:displacement attribute name
    pub outputs_displacement: Token,
    /// "outputs:surface" - outputs:surface attribute name
    pub outputs_surface: Token,
    /// "outputs:volume" - outputs:volume attribute name
    pub outputs_volume: Token,
    /// "preview" - preview material purpose
    pub preview: Token,
    /// "sdrMetadata" - sdrMetadata dictionary metadata key
    pub sdr_metadata: Token,
    /// "sourceAsset" - sourceAsset implementation source value
    pub source_asset: Token,
    /// "sourceCode" - sourceCode implementation source value
    pub source_code: Token,
    /// "strongerThanDescendants" - strongerThanDescendants bindMaterialAs value
    pub stronger_than_descendants: Token,
    /// "subIdentifier" - subIdentifier metadata key
    pub sub_identifier: Token,
    /// "surface" - surface output terminal name
    pub surface: Token,
    /// "" - universalRenderContext (empty string)
    pub universal_render_context: Token,
    /// "" - universalSourceType (empty string)
    pub universal_source_type: Token,
    /// "volume" - volume output terminal name
    pub volume: Token,
    /// "weakerThanDescendants" - weakerThanDescendants bindMaterialAs value
    pub weaker_than_descendants: Token,
    /// "ConnectableAPI" - ConnectableAPI schema identifier
    pub connectable_api: Token,
    /// "CoordSysAPI" - CoordSysAPI schema identifier
    pub coord_sys_api: Token,
    /// "Material" - Material schema identifier
    pub material: Token,
    /// "MaterialBindingAPI" - MaterialBindingAPI schema identifier
    pub material_binding_api: Token,
    /// "NodeDefAPI" - NodeDefAPI schema identifier
    pub node_def_api: Token,
    /// "NodeGraph" - NodeGraph schema identifier
    pub node_graph: Token,
    /// "Shader" - Shader schema identifier
    pub shader: Token,
}

static TOKENS: OnceLock<UsdShadeTokens> = OnceLock::new();

impl UsdShadeTokens {
    /// Get the global tokens instance.
    pub fn get() -> &'static UsdShadeTokens {
        TOKENS.get_or_init(|| UsdShadeTokens {
            all_purpose: Token::new(""),
            bind_material_as: Token::new("bindMaterialAs"),
            coord_sys: Token::new("coordSys"),
            coord_sys_multiple_apply_template_binding: Token::new(
                "coordSys:__INSTANCE_NAME__:binding",
            ),
            displacement: Token::new("displacement"),
            fallback_strength: Token::new("fallbackStrength"),
            full: Token::new("full"),
            id: Token::new("id"),
            info_id: Token::new("info:id"),
            info_implementation_source: Token::new("info:implementationSource"),
            inputs: Token::new("inputs:"),
            interface_only: Token::new("interfaceOnly"),
            material_bind: Token::new("materialBind"),
            material_binding: Token::new("material:binding"),
            material_binding_collection: Token::new("material:binding:collection"),
            material_variant: Token::new("materialVariant"),
            outputs: Token::new("outputs:"),
            outputs_displacement: Token::new("outputs:displacement"),
            outputs_surface: Token::new("outputs:surface"),
            outputs_volume: Token::new("outputs:volume"),
            preview: Token::new("preview"),
            sdr_metadata: Token::new("sdrMetadata"),
            source_asset: Token::new("sourceAsset"),
            source_code: Token::new("sourceCode"),
            stronger_than_descendants: Token::new("strongerThanDescendants"),
            sub_identifier: Token::new("subIdentifier"),
            surface: Token::new("surface"),
            universal_render_context: Token::new(""),
            universal_source_type: Token::new(""),
            volume: Token::new("volume"),
            weaker_than_descendants: Token::new("weakerThanDescendants"),
            connectable_api: Token::new("ConnectableAPI"),
            coord_sys_api: Token::new("CoordSysAPI"),
            material: Token::new("Material"),
            material_binding_api: Token::new("MaterialBindingAPI"),
            node_def_api: Token::new("NodeDefAPI"),
            node_graph: Token::new("NodeGraph"),
            shader: Token::new("Shader"),
        })
    }
}

/// Global tokens instance for convenience.
pub fn tokens() -> &'static UsdShadeTokens {
    UsdShadeTokens::get()
}
