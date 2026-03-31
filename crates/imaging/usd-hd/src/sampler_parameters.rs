
//! Sampler parameter resolution from material node parameters.
//!
//! Port of the `HdGetSamplerParameters` functions from `pxr/imaging/hd/material.cpp`.
//!
//! Resolves texture sampler wrap/filter modes from material node parameters,
//! with fallback to SdrShaderNode property defaults.

use std::collections::BTreeMap;

use crate::enums::{HdBorderColor, HdCompareFunction, HdMagFilter, HdMinFilter, HdWrap};
use crate::material_network::HdMaterialNode2;
use crate::types::HdSamplerParameters;
use usd_sdf::Path as SdfPath;
use usd_sdr::SdrShaderNode;
use usd_tf::Token;
use usd_vt::Value;

// Private token constants for wrap/filter mode string matching.
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static WRAP_S: LazyLock<Token> = LazyLock::new(|| Token::new("wrapS"));
    pub static WRAP_T: LazyLock<Token> = LazyLock::new(|| Token::new("wrapT"));
    pub static WRAP_R: LazyLock<Token> = LazyLock::new(|| Token::new("wrapR"));

    pub static REPEAT: LazyLock<Token> = LazyLock::new(|| Token::new("repeat"));
    pub static MIRROR: LazyLock<Token> = LazyLock::new(|| Token::new("mirror"));
    pub static CLAMP: LazyLock<Token> = LazyLock::new(|| Token::new("clamp"));
    pub static BLACK: LazyLock<Token> = LazyLock::new(|| Token::new("black"));
    pub static USE_METADATA: LazyLock<Token> = LazyLock::new(|| Token::new("useMetadata"));

    pub static HW_UV_TEXTURE_1: LazyLock<Token> = LazyLock::new(|| Token::new("HwUvTexture_1"));

    pub static MIN_FILTER: LazyLock<Token> = LazyLock::new(|| Token::new("minFilter"));
    pub static MAG_FILTER: LazyLock<Token> = LazyLock::new(|| Token::new("magFilter"));

    pub static NEAREST: LazyLock<Token> = LazyLock::new(|| Token::new("nearest"));
    pub static LINEAR: LazyLock<Token> = LazyLock::new(|| Token::new("linear"));
    pub static NEAREST_MIPMAP_NEAREST: LazyLock<Token> =
        LazyLock::new(|| Token::new("nearestMipmapNearest"));
    pub static NEAREST_MIPMAP_LINEAR: LazyLock<Token> =
        LazyLock::new(|| Token::new("nearestMipmapLinear"));
    pub static LINEAR_MIPMAP_NEAREST: LazyLock<Token> =
        LazyLock::new(|| Token::new("linearMipmapNearest"));
    pub static LINEAR_MIPMAP_LINEAR: LazyLock<Token> =
        LazyLock::new(|| Token::new("linearMipmapLinear"));
}

/// Look up a Token value from parameters map, falling back to SdrShaderNode
/// property default, then to provided default.
///
/// Mirrors C++ `_ResolveParameter<TfToken>`.
fn resolve_token_param(
    parameters: &BTreeMap<Token, Value>,
    sdr_node: Option<&SdrShaderNode>,
    name: &Token,
    default_value: &Token,
) -> Token {
    // First consult parameters
    if let Some(value) = parameters.get(name) {
        if let Some(tok) = value.downcast::<Token>() {
            return tok.clone();
        }
    }

    // Then fallback to SdrNode
    if let Some(node) = sdr_node {
        if let Some(input) = node.get_shader_input(name) {
            let value = input.get_default_value_as_sdf_type();
            if let Some(tok) = value.downcast::<Token>() {
                return tok.clone();
            }
        }
    }

    default_value.clone()
}

/// Resolve a wrap mode parameter (wrapS, wrapT, wrapR) to an HdWrap enum value.
///
/// Mirrors C++ `_ResolveWrapSamplerParameter`.
fn resolve_wrap(
    node_type_id: &Token,
    parameters: &BTreeMap<Token, Value>,
    sdr_node: Option<&SdrShaderNode>,
    node_path: &SdfPath,
    name: &Token,
) -> HdWrap {
    let value = resolve_token_param(parameters, sdr_node, name, &tokens::USE_METADATA);

    if value == *tokens::REPEAT {
        return HdWrap::Repeat;
    }
    if value == *tokens::MIRROR {
        return HdWrap::Mirror;
    }
    if value == *tokens::CLAMP {
        return HdWrap::Clamp;
    }
    if value == *tokens::BLACK {
        return HdWrap::Black;
    }
    if value == *tokens::USE_METADATA {
        // Legacy HwUvTexture_1 nodes use LegacyNoOpinionFallbackRepeat
        if *node_type_id == *tokens::HW_UV_TEXTURE_1 {
            return HdWrap::LegacyNoOpinionFallbackRepeat;
        }
        return HdWrap::NoOpinion;
    }

    // Unknown wrap mode - warn
    if !node_path.is_empty() {
        log::warn!(
            "Unknown wrap mode on prim {}: {}",
            node_path.as_str(),
            value.as_str()
        );
    } else {
        log::warn!("Unknown wrap mode: {}", value.as_str());
    }

    HdWrap::NoOpinion
}

/// Resolve minFilter parameter to HdMinFilter.
///
/// Default fallback is linearMipmapLinear.
/// Mirrors C++ `_ResolveMinSamplerParameter`.
fn resolve_min_filter(
    _node_type_id: &Token,
    parameters: &BTreeMap<Token, Value>,
    sdr_node: Option<&SdrShaderNode>,
    _node_path: &SdfPath,
) -> HdMinFilter {
    let value = resolve_token_param(
        parameters,
        sdr_node,
        &tokens::MIN_FILTER,
        &tokens::LINEAR_MIPMAP_LINEAR,
    );

    if value == *tokens::NEAREST {
        return HdMinFilter::Nearest;
    }
    if value == *tokens::LINEAR {
        return HdMinFilter::Linear;
    }
    if value == *tokens::NEAREST_MIPMAP_NEAREST {
        return HdMinFilter::NearestMipmapNearest;
    }
    if value == *tokens::NEAREST_MIPMAP_LINEAR {
        return HdMinFilter::NearestMipmapLinear;
    }
    if value == *tokens::LINEAR_MIPMAP_NEAREST {
        return HdMinFilter::LinearMipmapNearest;
    }
    if value == *tokens::LINEAR_MIPMAP_LINEAR {
        return HdMinFilter::LinearMipmapLinear;
    }

    HdMinFilter::LinearMipmapLinear
}

/// Resolve magFilter parameter to HdMagFilter.
///
/// Default fallback is linear.
/// Mirrors C++ `_ResolveMagSamplerParameter`.
fn resolve_mag_filter(
    _node_type_id: &Token,
    parameters: &BTreeMap<Token, Value>,
    sdr_node: Option<&SdrShaderNode>,
    _node_path: &SdfPath,
) -> HdMagFilter {
    let value = resolve_token_param(parameters, sdr_node, &tokens::MAG_FILTER, &tokens::LINEAR);

    if value == *tokens::NEAREST {
        return HdMagFilter::Nearest;
    }

    HdMagFilter::Linear
}

/// Internal: assemble all sampler parameters from resolved components.
fn get_sampler_params(
    node_type_id: &Token,
    parameters: &BTreeMap<Token, Value>,
    sdr_node: Option<&SdrShaderNode>,
    node_path: &SdfPath,
) -> HdSamplerParameters {
    HdSamplerParameters::new(
        resolve_wrap(
            node_type_id,
            parameters,
            sdr_node,
            node_path,
            &tokens::WRAP_S,
        ),
        resolve_wrap(
            node_type_id,
            parameters,
            sdr_node,
            node_path,
            &tokens::WRAP_T,
        ),
        resolve_wrap(
            node_type_id,
            parameters,
            sdr_node,
            node_path,
            &tokens::WRAP_R,
        ),
        resolve_min_filter(node_type_id, parameters, sdr_node, node_path),
        resolve_mag_filter(node_type_id, parameters, sdr_node, node_path),
        HdBorderColor::TransparentBlack,
        false,
        HdCompareFunction::Never,
        16,
    )
}

/// Resolve sampler parameters from a material node and its Sdr definition.
///
/// Looks up wrap modes (wrapS, wrapT, wrapR) and filter modes (minFilter,
/// magFilter) from node parameters, falling back to SdrShaderNode property
/// defaults when not explicitly authored.
///
/// Mirrors C++ `HdGetSamplerParameters(HdMaterialNode2, SdrShaderNodeConstPtr, SdfPath)`.
pub fn hd_get_sampler_params(
    node: &HdMaterialNode2,
    sdr_node: Option<&SdrShaderNode>,
    node_path: &SdfPath,
) -> HdSamplerParameters {
    get_sampler_params(&node.node_type_id, &node.parameters, sdr_node, node_path)
}

/// Resolve sampler parameters from node type id and parameter map (no Sdr node).
///
/// This overload is for when no SdrShaderNode is available. Only the parameters
/// map is consulted (no fallback to Sdr defaults).
///
/// Mirrors C++ `HdGetSamplerParameters(TfToken, map<TfToken,VtValue>, SdfPath)`.
pub fn hd_get_sampler_params_from_type(
    node_type_id: &Token,
    parameters: &BTreeMap<Token, Value>,
    node_path: &SdfPath,
) -> HdSamplerParameters {
    get_sampler_params(node_type_id, parameters, None, node_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_sampler_params() {
        // With no parameters and no sdr node, should get default values
        let params = BTreeMap::new();
        let node_type_id = Token::new("UsdUVTexture");
        let node_path = SdfPath::empty();

        let result = hd_get_sampler_params_from_type(&node_type_id, &params, &node_path);

        // Default wrap is useMetadata -> NoOpinion
        assert_eq!(result.wrap_s, HdWrap::NoOpinion);
        assert_eq!(result.wrap_t, HdWrap::NoOpinion);
        assert_eq!(result.wrap_r, HdWrap::NoOpinion);
        // Default min filter is linearMipmapLinear
        assert_eq!(result.min_filter, HdMinFilter::LinearMipmapLinear);
        // Default mag filter is linear
        assert_eq!(result.mag_filter, HdMagFilter::Linear);
        assert_eq!(result.border_color, HdBorderColor::TransparentBlack);
        assert!(!result.enable_compare);
        assert_eq!(result.compare_function, HdCompareFunction::Never);
        assert_eq!(result.max_anisotropy, 16);
    }

    #[test]
    fn test_explicit_wrap_modes() {
        let mut params = BTreeMap::new();
        params.insert(Token::new("wrapS"), Value::new(Token::new("repeat")));
        params.insert(Token::new("wrapT"), Value::new(Token::new("clamp")));
        params.insert(Token::new("wrapR"), Value::new(Token::new("mirror")));

        let node_type_id = Token::new("UsdUVTexture");
        let node_path = SdfPath::empty();

        let result = hd_get_sampler_params_from_type(&node_type_id, &params, &node_path);

        assert_eq!(result.wrap_s, HdWrap::Repeat);
        assert_eq!(result.wrap_t, HdWrap::Clamp);
        assert_eq!(result.wrap_r, HdWrap::Mirror);
    }

    #[test]
    fn test_black_wrap_mode() {
        let mut params = BTreeMap::new();
        params.insert(Token::new("wrapS"), Value::new(Token::new("black")));

        let node_type_id = Token::new("UsdUVTexture");
        let node_path = SdfPath::empty();

        let result = hd_get_sampler_params_from_type(&node_type_id, &params, &node_path);

        assert_eq!(result.wrap_s, HdWrap::Black);
    }

    #[test]
    fn test_legacy_hw_uv_texture() {
        // HwUvTexture_1 with useMetadata should return LegacyNoOpinionFallbackRepeat
        let params = BTreeMap::new();
        let node_type_id = Token::new("HwUvTexture_1");
        let node_path = SdfPath::empty();

        let result = hd_get_sampler_params_from_type(&node_type_id, &params, &node_path);

        assert_eq!(result.wrap_s, HdWrap::LegacyNoOpinionFallbackRepeat);
        assert_eq!(result.wrap_t, HdWrap::LegacyNoOpinionFallbackRepeat);
        assert_eq!(result.wrap_r, HdWrap::LegacyNoOpinionFallbackRepeat);
    }

    #[test]
    fn test_filter_modes() {
        let mut params = BTreeMap::new();
        params.insert(Token::new("minFilter"), Value::new(Token::new("nearest")));
        params.insert(Token::new("magFilter"), Value::new(Token::new("nearest")));

        let node_type_id = Token::new("UsdUVTexture");
        let node_path = SdfPath::empty();

        let result = hd_get_sampler_params_from_type(&node_type_id, &params, &node_path);

        assert_eq!(result.min_filter, HdMinFilter::Nearest);
        assert_eq!(result.mag_filter, HdMagFilter::Nearest);
    }

    #[test]
    fn test_all_min_filter_modes() {
        let node_type_id = Token::new("UsdUVTexture");
        let node_path = SdfPath::empty();

        let cases = [
            ("nearest", HdMinFilter::Nearest),
            ("linear", HdMinFilter::Linear),
            ("nearestMipmapNearest", HdMinFilter::NearestMipmapNearest),
            ("nearestMipmapLinear", HdMinFilter::NearestMipmapLinear),
            ("linearMipmapNearest", HdMinFilter::LinearMipmapNearest),
            ("linearMipmapLinear", HdMinFilter::LinearMipmapLinear),
        ];

        for (token_str, expected) in &cases {
            let mut params = BTreeMap::new();
            params.insert(Token::new("minFilter"), Value::new(Token::new(token_str)));

            let result = hd_get_sampler_params_from_type(&node_type_id, &params, &node_path);
            assert_eq!(
                result.min_filter, *expected,
                "Failed for minFilter={token_str}"
            );
        }
    }

    #[test]
    fn test_from_material_node() {
        let mut node = HdMaterialNode2::default();
        node.node_type_id = Token::new("UsdUVTexture");
        node.parameters
            .insert(Token::new("wrapS"), Value::new(Token::new("repeat")));
        node.parameters
            .insert(Token::new("minFilter"), Value::new(Token::new("linear")));

        let node_path = SdfPath::empty();

        let result = hd_get_sampler_params(&node, None, &node_path);

        assert_eq!(result.wrap_s, HdWrap::Repeat);
        assert_eq!(result.min_filter, HdMinFilter::Linear);
        assert_eq!(result.mag_filter, HdMagFilter::Linear);
    }
}
