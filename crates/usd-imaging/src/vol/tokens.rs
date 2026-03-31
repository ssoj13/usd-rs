//! Token definitions for UsdVolImaging.
//!
//! These tokens identify volume field asset types in the imaging system.
//!
//! # C++ Reference
//!
//! Port of `pxr/usdImaging/usdVolImaging/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdVolImaging module.
pub struct UsdVolImagingTokensType {
    /// "field3dAsset" - Field3D asset type identifier
    pub field3d_asset: Token,
    /// "openvdbAsset" - OpenVDB asset type identifier
    pub openvdb_asset: Token,
}

impl UsdVolImagingTokensType {
    fn new() -> Self {
        Self {
            field3d_asset: Token::new("field3dAsset"),
            openvdb_asset: Token::new("openvdbAsset"),
        }
    }
}

/// Global tokens instance for UsdVolImaging.
pub static USD_VOL_IMAGING_TOKENS: LazyLock<UsdVolImagingTokensType> =
    LazyLock::new(UsdVolImagingTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(
            USD_VOL_IMAGING_TOKENS.field3d_asset.as_str(),
            "field3dAsset"
        );
        assert_eq!(
            USD_VOL_IMAGING_TOKENS.openvdb_asset.as_str(),
            "openvdbAsset"
        );
    }
}
