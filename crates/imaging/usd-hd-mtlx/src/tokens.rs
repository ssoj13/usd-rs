//! HdMtlx tokens.
//!
//! Static tokens for MaterialX shader terminal names.

use std::sync::LazyLock;
use usd_tf::Token;

/// Surface shader terminal name in MaterialX documents.
pub static SURFACE_SHADER_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("Surface"));

/// Displacement shader terminal name in MaterialX documents.
pub static DISPLACEMENT_SHADER_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("Displacement"));

/// Volume shader terminal name in MaterialX documents.
pub static VOLUME_SHADER_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("Volume"));

/// Light shader terminal name in MaterialX documents.
pub static LIGHT_SHADER_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("Light"));

/// Default MaterialX surface shader type.
pub static SURFACE_SHADER_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("surfaceshader"));

/// Default MaterialX displacement shader type.
pub static DISPLACEMENT_SHADER_TYPE: LazyLock<Token> =
    LazyLock::new(|| Token::new("displacementshader"));

/// Default MaterialX volume shader type.
pub static VOLUME_SHADER_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("volumeshader"));

/// MaterialX standard surface node definition name.
pub static STANDARD_SURFACE: LazyLock<Token> =
    LazyLock::new(|| Token::new("ND_standard_surface_surfaceshader"));

/// MaterialX USD preview surface node definition name.
pub static USD_PREVIEW_SURFACE: LazyLock<Token> =
    LazyLock::new(|| Token::new("ND_UsdPreviewSurface_surfaceshader"));

/// MaterialX gltf PBR node definition name.
pub static GLTF_PBR: LazyLock<Token> = LazyLock::new(|| Token::new("ND_gltf_pbr_surfaceshader"));

/// MaterialX "file" input name (for texture nodes).
pub static FILE_INPUT: LazyLock<Token> = LazyLock::new(|| Token::new("file"));

/// MaterialX "texcoord" input name.
pub static TEXCOORD_INPUT: LazyLock<Token> = LazyLock::new(|| Token::new("texcoord"));

/// MaterialX default geomprop ("UV0") for texture coordinate lookups.
pub static DEFAULT_GEOMPROP: LazyLock<Token> = LazyLock::new(|| Token::new("UV0"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(SURFACE_SHADER_NAME.as_str(), "Surface");
        assert_eq!(DISPLACEMENT_SHADER_NAME.as_str(), "Displacement");
    }
}
