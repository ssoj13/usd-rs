//! UsdMtlx tokens for MaterialX schemas.
//!
//! These tokens are used for attribute names and schema identifiers
//! in the UsdMtlx schema module.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMtlx/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdMtlx schemas.
pub struct UsdMtlxTokensType {
    /// "config:mtlx:version" - MaterialX version attribute
    pub config_mtlx_version: Token,
    /// "out" - Default output name
    pub default_output_name: Token,
    /// "MaterialXConfigAPI" - Schema identifier
    pub material_x_config_api: Token,
}

impl UsdMtlxTokensType {
    fn new() -> Self {
        Self {
            config_mtlx_version: Token::new_immortal("config:mtlx:version"),
            default_output_name: Token::new_immortal("out"),
            material_x_config_api: Token::new_immortal("MaterialXConfigAPI"),
        }
    }
}

/// Global tokens instance for UsdMtlx schemas.
pub static USD_MTLX_TOKENS: LazyLock<UsdMtlxTokensType> = LazyLock::new(UsdMtlxTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(
            USD_MTLX_TOKENS.config_mtlx_version.as_str(),
            "config:mtlx:version"
        );
        assert_eq!(USD_MTLX_TOKENS.default_output_name.as_str(), "out");
        assert_eq!(
            USD_MTLX_TOKENS.material_x_config_api.as_str(),
            "MaterialXConfigAPI"
        );
    }

    #[test]
    fn test_tokens_are_immortal() {
        // All tokens should be immortal to prevent GC
        assert!(USD_MTLX_TOKENS.config_mtlx_version.is_immortal());
        assert!(USD_MTLX_TOKENS.default_output_name.is_immortal());
        assert!(USD_MTLX_TOKENS.material_x_config_api.is_immortal());
    }
}
