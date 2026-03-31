//! Primvar utilities for USD to Hydra conversion.
//!
//! Port of pxr/usdImaging/usdImaging/primvarUtils.h
//!
//! Utility functions for converting USD primvar data to Hydra representation,
//! including role and interpolation conversions.

use usd_hd::HdInterpolation;
use usd_tf::Token;

// ============================================================================
// USD Primvar Interpolation Tokens
// ============================================================================

mod usd_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // USD primvar interpolation tokens (from UsdGeomTokens)
    pub static CONSTANT: LazyLock<Token> = LazyLock::new(|| Token::new("constant"));
    pub static UNIFORM: LazyLock<Token> = LazyLock::new(|| Token::new("uniform"));
    pub static VARYING: LazyLock<Token> = LazyLock::new(|| Token::new("varying"));
    pub static VERTEX: LazyLock<Token> = LazyLock::new(|| Token::new("vertex"));
    pub static FACE_VARYING: LazyLock<Token> = LazyLock::new(|| Token::new("faceVarying"));
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Converts USD primvar role token to corresponding Hydra role token.
///
/// USD and Hydra use similar but not identical role naming conventions.
/// This function maps common USD roles to their Hydra equivalents.
///
/// # Arguments
///
/// * `usd_role` - USD role token (e.g., "Point", "Normal", "Color")
///
/// # Returns
///
/// Hydra-compatible role token. Returns empty token for unknown roles.
///
/// # Examples
///
/// ```
/// use usd_tf::Token;
/// use usd_imaging::primvar_utils::usd_to_hd_role;
///
/// let usd_role = Token::new("Point");
/// let hd_role = usd_to_hd_role(&usd_role);
/// assert_eq!(hd_role.as_str(), "point");
/// ```
pub fn usd_to_hd_role(usd_role: &Token) -> Token {
    // USD uses capitalized roles like "Point", "Normal", "Color"
    // Hydra uses lowercase like "point", "normal", "color"
    let role_str = usd_role.as_str();

    // Common role mappings
    match role_str {
        "Point" => Token::new("point"),
        "Normal" => Token::new("normal"),
        "Vector" => Token::new("vector"),
        "Color" => Token::new("color"),
        "TextureCoordinate" => Token::new("textureCoordinate"),
        // Pass through if already lowercase or unknown
        _ => {
            if role_str.is_empty() {
                Token::new("")
            } else {
                // Convert to lowercase for consistency
                Token::new(&role_str.to_lowercase())
            }
        }
    }
}

/// Converts USD primvar interpolation token to Hydra interpolation enum.
///
/// Maps USD's string-based interpolation modes to Hydra's enum representation.
///
/// # Arguments
///
/// * `usd_interp` - USD interpolation token (constant, uniform, varying, vertex, faceVarying)
///
/// # Returns
///
/// Corresponding `HdInterpolation` enum value. Defaults to `Constant` for unknown values.
///
/// # Examples
///
/// ```
/// use usd_tf::Token;
/// use usd_hd::HdInterpolation;
/// use usd_imaging::primvar_utils::usd_to_hd_interpolation;
///
/// let interp = usd_to_hd_interpolation(&Token::new("vertex"));
/// assert_eq!(interp, HdInterpolation::Vertex);
/// ```
pub fn usd_to_hd_interpolation(usd_interp: &Token) -> HdInterpolation {
    if usd_interp == &*usd_tokens::CONSTANT {
        HdInterpolation::Constant
    } else if usd_interp == &*usd_tokens::UNIFORM {
        HdInterpolation::Uniform
    } else if usd_interp == &*usd_tokens::VARYING {
        HdInterpolation::Varying
    } else if usd_interp == &*usd_tokens::VERTEX {
        HdInterpolation::Vertex
    } else if usd_interp == &*usd_tokens::FACE_VARYING {
        HdInterpolation::FaceVarying
    } else {
        // Default to constant for unknown interpolation modes
        HdInterpolation::Constant
    }
}

/// Converts USD primvar interpolation token to Hydra interpolation token.
///
/// Similar to `usd_to_hd_interpolation` but returns a Token instead of enum.
/// Useful when token representation is needed instead of enum.
///
/// # Arguments
///
/// * `usd_interp` - USD interpolation token
///
/// # Returns
///
/// Hydra interpolation token. Returns "constant" for unknown values.
///
/// # Examples
///
/// ```
/// use usd_tf::Token;
/// use usd_imaging::primvar_utils::usd_to_hd_interpolation_token;
///
/// let token = usd_to_hd_interpolation_token(&Token::new("faceVarying"));
/// assert_eq!(token.as_str(), "faceVarying");
/// ```
pub fn usd_to_hd_interpolation_token(usd_interp: &Token) -> Token {
    let hd_interp = usd_to_hd_interpolation(usd_interp);
    Token::new(hd_interp.as_str())
}

/// Validates that a primvar interpolation mode is supported.
///
/// # Arguments
///
/// * `interp` - Interpolation token to validate
///
/// # Returns
///
/// `true` if the interpolation mode is recognized, `false` otherwise.
pub fn is_valid_interpolation(interp: &Token) -> bool {
    interp == &*usd_tokens::CONSTANT
        || interp == &*usd_tokens::UNIFORM
        || interp == &*usd_tokens::VARYING
        || interp == &*usd_tokens::VERTEX
        || interp == &*usd_tokens::FACE_VARYING
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::HdInterpolation;

    #[test]
    fn test_role_conversion() {
        assert_eq!(usd_to_hd_role(&Token::new("Point")).as_str(), "point");
        assert_eq!(usd_to_hd_role(&Token::new("Normal")).as_str(), "normal");
        assert_eq!(usd_to_hd_role(&Token::new("Vector")).as_str(), "vector");
        assert_eq!(usd_to_hd_role(&Token::new("Color")).as_str(), "color");
        assert_eq!(
            usd_to_hd_role(&Token::new("TextureCoordinate")).as_str(),
            "textureCoordinate"
        );
    }

    #[test]
    fn test_role_passthrough() {
        // Empty token
        assert_eq!(usd_to_hd_role(&Token::new("")).as_str(), "");

        // Already lowercase
        assert_eq!(usd_to_hd_role(&Token::new("point")).as_str(), "point");

        // Unknown role - should lowercase
        assert_eq!(usd_to_hd_role(&Token::new("Custom")).as_str(), "custom");
    }

    #[test]
    fn test_interpolation_conversion() {
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("constant")),
            HdInterpolation::Constant
        );
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("uniform")),
            HdInterpolation::Uniform
        );
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("varying")),
            HdInterpolation::Varying
        );
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("vertex")),
            HdInterpolation::Vertex
        );
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("faceVarying")),
            HdInterpolation::FaceVarying
        );
    }

    #[test]
    fn test_interpolation_unknown() {
        // Unknown interpolation should default to Constant
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("unknown")),
            HdInterpolation::Constant
        );
        assert_eq!(
            usd_to_hd_interpolation(&Token::new("")),
            HdInterpolation::Constant
        );
    }

    #[test]
    fn test_interpolation_token_conversion() {
        assert_eq!(
            usd_to_hd_interpolation_token(&Token::new("constant")).as_str(),
            "constant"
        );
        assert_eq!(
            usd_to_hd_interpolation_token(&Token::new("uniform")).as_str(),
            "uniform"
        );
        assert_eq!(
            usd_to_hd_interpolation_token(&Token::new("vertex")).as_str(),
            "vertex"
        );
        assert_eq!(
            usd_to_hd_interpolation_token(&Token::new("faceVarying")).as_str(),
            "faceVarying"
        );
    }

    #[test]
    fn test_is_valid_interpolation() {
        assert!(is_valid_interpolation(&Token::new("constant")));
        assert!(is_valid_interpolation(&Token::new("uniform")));
        assert!(is_valid_interpolation(&Token::new("varying")));
        assert!(is_valid_interpolation(&Token::new("vertex")));
        assert!(is_valid_interpolation(&Token::new("faceVarying")));

        assert!(!is_valid_interpolation(&Token::new("invalid")));
        assert!(!is_valid_interpolation(&Token::new("")));
    }

    #[test]
    fn test_all_interpolation_modes() {
        // Ensure all HdInterpolation modes have corresponding USD tokens
        let modes = [
            ("constant", HdInterpolation::Constant),
            ("uniform", HdInterpolation::Uniform),
            ("varying", HdInterpolation::Varying),
            ("vertex", HdInterpolation::Vertex),
            ("faceVarying", HdInterpolation::FaceVarying),
        ];

        for (token_str, expected_interp) in modes.iter() {
            let token = Token::new(token_str);
            assert_eq!(usd_to_hd_interpolation(&token), *expected_interp);
            assert!(is_valid_interpolation(&token));
        }
    }
}
