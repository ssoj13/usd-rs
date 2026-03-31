
//! Storm package/plugin registration (ported from package.h).
//!
//! Returns resource paths (tokens) for built-in shader files used by Storm.
//! In C++, these resolve to GLSLFX files via the plugin system.
//! In Rust, they serve as identifiers for embedded or bundled shader resources.

use once_cell::sync::Lazy;
use usd_tf::Token;

/// Compute shader resource identifier.
pub static COMPUTE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("hdSt/computeShader"));

/// Dome light shader resource identifier.
pub static DOME_LIGHT_SHADER: Lazy<Token> = Lazy::new(|| Token::new("hdSt/domeLightShader"));

/// Fallback dome light texture resource identifier.
pub static FALLBACK_DOME_LIGHT_TEXTURE: Lazy<Token> =
    Lazy::new(|| Token::new("hdSt/fallbackDomeLightTexture"));

/// Ptex texture shader resource identifier.
pub static PTEX_TEXTURE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("hdSt/ptexTextureShader"));

/// Render pass shader resource identifier.
pub static RENDER_PASS_SHADER: Lazy<Token> = Lazy::new(|| Token::new("hdSt/renderPassShader"));

/// Fallback lighting shader resource identifier.
pub static FALLBACK_LIGHTING_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("hdSt/fallbackLightingShader"));

/// Fallback material network shader resource identifier.
pub static FALLBACK_MATERIAL_NETWORK_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("hdSt/fallbackMaterialNetworkShader"));

/// Invalid material network shader resource identifier.
pub static INVALID_MATERIAL_NETWORK_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("hdSt/invalidMaterialNetworkShader"));

/// Fallback volume shader resource identifier.
pub static FALLBACK_VOLUME_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("hdSt/fallbackVolumeShader"));

/// Image shader resource identifier (for fullscreen passes).
pub static IMAGE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("hdSt/imageShader"));

/// Simple lighting shader resource identifier.
pub static SIMPLE_LIGHTING_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("hdSt/simpleLightingShader"));

/// Overlay shader resource identifier.
pub static OVERLAY_SHADER: Lazy<Token> = Lazy::new(|| Token::new("hdSt/overlayShader"));

/// Get the compute shader package token.
pub fn compute_shader() -> &'static Token {
    &COMPUTE_SHADER
}

/// Get the dome light shader package token.
pub fn dome_light_shader() -> &'static Token {
    &DOME_LIGHT_SHADER
}

/// Get the fallback dome light texture package token.
pub fn fallback_dome_light_texture() -> &'static Token {
    &FALLBACK_DOME_LIGHT_TEXTURE
}

/// Get the ptex texture shader package token.
pub fn ptex_texture_shader() -> &'static Token {
    &PTEX_TEXTURE_SHADER
}

/// Get the render pass shader package token.
pub fn render_pass_shader() -> &'static Token {
    &RENDER_PASS_SHADER
}

/// Get the fallback lighting shader package token.
pub fn fallback_lighting_shader() -> &'static Token {
    &FALLBACK_LIGHTING_SHADER
}

/// Get the fallback material network shader package token.
pub fn fallback_material_network_shader() -> &'static Token {
    &FALLBACK_MATERIAL_NETWORK_SHADER
}

/// Get the invalid material network shader package token.
pub fn invalid_material_network_shader() -> &'static Token {
    &INVALID_MATERIAL_NETWORK_SHADER
}

/// Get the fallback volume shader package token.
pub fn fallback_volume_shader() -> &'static Token {
    &FALLBACK_VOLUME_SHADER
}

/// Get the image shader package token.
pub fn image_shader() -> &'static Token {
    &IMAGE_SHADER
}

/// Get the simple lighting shader package token.
pub fn simple_lighting_shader() -> &'static Token {
    &SIMPLE_LIGHTING_SHADER
}

/// Get the overlay shader package token.
pub fn overlay_shader() -> &'static Token {
    &OVERLAY_SHADER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_tokens() {
        assert_eq!(compute_shader().as_str(), "hdSt/computeShader");
        assert_eq!(render_pass_shader().as_str(), "hdSt/renderPassShader");
        assert_eq!(
            fallback_lighting_shader().as_str(),
            "hdSt/fallbackLightingShader"
        );
    }
}
