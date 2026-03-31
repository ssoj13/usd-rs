//! USD tokens - commonly used string tokens.

use std::sync::OnceLock;
use usd_tf::Token;

/// USD module tokens - commonly used metadata and composition keywords.
pub struct UsdTokens {
    /// "apiSchemas" - applied API schemas list.
    pub api_schemas: Token,
    /// "clips" - value clips root.
    pub clips: Token,
    /// "clipAssetPaths" - clip asset paths.
    pub clip_asset_paths: Token,
    /// "clipManifestAssetPath" - clip manifest asset path.
    pub clip_manifest_asset_path: Token,
    /// "clipPrimPath" - clip prim path.
    pub clip_prim_path: Token,
    /// "clipSet" - clip set name.
    pub clip_set: Token,
    /// "clipSets" - clip sets dictionary.
    pub clip_sets: Token,
    /// "clipTemplateActiveOffset" - clip template active offset.
    pub clip_template_active_offset: Token,
    /// "clipTemplateAssetPath" - clip template asset path pattern.
    pub clip_template_asset_path: Token,
    /// "clipTemplateEndTime" - clip template end time.
    pub clip_template_end_time: Token,
    /// "clipTemplateStartTime" - clip template start time.
    pub clip_template_start_time: Token,
    /// "clipTemplateStride" - clip template stride.
    pub clip_template_stride: Token,
    /// "clipTimes" - clip times mapping.
    pub clip_times: Token,
    /// "clipsActive" - active clips list.
    pub clips_active: Token,
    /// "collection" - collection prefix.
    pub collection: Token,
    /// "exclude" - collection exclude mode.
    pub exclude: Token,
    /// "excludes" - collection excludes paths.
    pub excludes: Token,
    /// "expansionRule" - collection expansion rule.
    pub expansion_rule: Token,
    /// "explicitOnly" - collection explicit only mode.
    pub explicit_only: Token,
    /// "fallbackPrimTypes" - fallback prim types.
    pub fallback_prim_types: Token,
    /// "include" - collection include mode.
    pub include: Token,
    /// "includeRoot" - collection include root.
    pub include_root: Token,
    /// "inherits" - inherits composition arc.
    pub inherits: Token,
    /// "kind" - prim kind metadata.
    pub kind: Token,
    /// "model" - model kind.
    pub model: Token,
    /// "payload" - payload composition arc.
    pub payload: Token,
    /// "prefixSubstitutions" - prefix substitutions.
    pub prefix_substitutions: Token,
    /// "references" - references composition arc.
    pub references: Token,
    /// "specializes" - specializes composition arc.
    pub specializes: Token,
    /// "suffixSubstitutions" - suffix substitutions.
    pub suffix_substitutions: Token,
    /// "typeName" - prim type name.
    pub type_name: Token,
    /// "variantSet" - variant set name.
    pub variant_set: Token,
    /// "variantSetNames" - variant set names list.
    pub variant_set_names: Token,
    /// "expandPrims" - default expansion rule for collections.
    pub expand_prims: Token,
    /// "expandPrimsAndProperties" - expand collections to include properties.
    pub expand_prims_and_properties: Token,
    /// "custom" - custom token for ColorSpaceDefinitionAPI.
    pub custom: Token,
    /// "APISchemaBase" - schema identifier for UsdAPISchemaBase.
    pub api_schema_base: Token,
    /// "ClipsAPI" - schema identifier for UsdClipsAPI.
    pub clips_api: Token,
    /// "CollectionAPI" - schema identifier for UsdCollectionAPI.
    pub collection_api: Token,
    /// "ColorSpaceAPI" - schema identifier for UsdColorSpaceAPI.
    pub color_space_api: Token,
    /// "ModelAPI" - schema identifier for UsdModelAPI.
    pub model_api: Token,
    /// "Typed" - schema identifier for UsdTyped.
    pub typed: Token,
    /// "membershipExpression" - collection membership expression.
    pub membership_expression: Token,
}

impl UsdTokens {
    /// Creates a new UsdTokens instance.
    ///
    /// Matches C++ static initialization.
    pub fn new() -> Self {
        Self {
            api_schemas: Token::new("apiSchemas"),
            clips: Token::new("clips"),
            clip_asset_paths: Token::new("clipAssetPaths"),
            clip_manifest_asset_path: Token::new("clipManifestAssetPath"),
            clip_prim_path: Token::new("clipPrimPath"),
            clip_set: Token::new("clipSet"),
            clip_sets: Token::new("clipSets"),
            clip_template_active_offset: Token::new("clipTemplateActiveOffset"),
            clip_template_asset_path: Token::new("clipTemplateAssetPath"),
            clip_template_end_time: Token::new("clipTemplateEndTime"),
            clip_template_start_time: Token::new("clipTemplateStartTime"),
            clip_template_stride: Token::new("clipTemplateStride"),
            clip_times: Token::new("clipTimes"),
            clips_active: Token::new("clipsActive"),
            collection: Token::new("collection"),
            exclude: Token::new("exclude"),
            excludes: Token::new("excludes"),
            expansion_rule: Token::new("expansionRule"),
            explicit_only: Token::new("explicitOnly"),
            fallback_prim_types: Token::new("fallbackPrimTypes"),
            include: Token::new("include"),
            include_root: Token::new("includeRoot"),
            inherits: Token::new("inherits"),
            kind: Token::new("kind"),
            model: Token::new("model"),
            payload: Token::new("payload"),
            prefix_substitutions: Token::new("prefixSubstitutions"),
            references: Token::new("references"),
            specializes: Token::new("specializes"),
            suffix_substitutions: Token::new("suffixSubstitutions"),
            type_name: Token::new("typeName"),
            variant_set: Token::new("variantSet"),
            variant_set_names: Token::new("variantSetNames"),
            expand_prims: Token::new("expandPrims"),
            expand_prims_and_properties: Token::new("expandPrimsAndProperties"),
            custom: Token::new("custom"),
            api_schema_base: Token::new("APISchemaBase"),
            clips_api: Token::new("ClipsAPI"),
            collection_api: Token::new("CollectionAPI"),
            color_space_api: Token::new("ColorSpaceAPI"),
            model_api: Token::new("ModelAPI"),
            typed: Token::new("Typed"),
            membership_expression: Token::new("membershipExpression"),
        }
    }
}

/// Returns the static USD tokens instance.
pub fn usd_tokens() -> &'static UsdTokens {
    static TOKENS: OnceLock<UsdTokens> = OnceLock::new();
    TOKENS.get_or_init(UsdTokens::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        let tokens = usd_tokens();
        assert_eq!(tokens.kind.get_text(), "kind");
        assert_eq!(tokens.references.get_text(), "references");
        assert_eq!(tokens.inherits.get_text(), "inherits");
    }
}
