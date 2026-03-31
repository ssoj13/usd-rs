#![allow(dead_code)]

//! Shader key for image shaders in Storm.
//!
//! Identifies the GLSLFX file and entry points for the fullscreen
//! image shader vertex + fragment stages.
//!
//! Matches C++ `HdSt_ImageShaderShaderKey`.

use std::sync::LazyLock;
use usd_tf::Token;

use crate::shader_key::PrimitiveType;

// Tokens for image shader stages
static GLSLFX_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("imageShader.glslfx"));
static VS_MAIN: LazyLock<Token> = LazyLock::new(|| Token::new("ImageShader.Vertex"));
static FS_MAIN: LazyLock<Token> = LazyLock::new(|| Token::new("ImageShader.Fragment"));

/// Shader key for image (fullscreen post-process) shaders.
///
/// Provides the GLSLFX filename and vertex/fragment entry points for the
/// fullscreen triangle used by `ImageShaderRenderPass`.
///
/// The vertex shader generates clip-space positions from vertex index:
/// ```glsl
/// // Fullscreen triangle: vertex 0 = (-1,-1), 1 = (3,-1), 2 = (-1,3)
/// vec2 pos = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2) * 2.0 - 1.0;
/// gl_Position = vec4(pos, 0.0, 1.0);
/// ```
///
/// The fragment shader is provided by the RenderPassShader set on the
/// RenderPassState (the actual post-processing effect).
#[derive(Debug, Clone)]
pub struct ImageShaderShaderKey {
    /// GLSLFX filename
    pub glslfx: Token,
    /// Vertex shader entry points (terminated by empty token)
    pub vs: [Token; 2],
    /// Fragment shader entry points (terminated by empty token)
    pub fs: [Token; 2],
}

impl ImageShaderShaderKey {
    /// Create a new image shader key with default entry points.
    pub fn new() -> Self {
        Self {
            glslfx: GLSLFX_FILE.clone(),
            vs: [VS_MAIN.clone(), Token::default()],
            fs: [FS_MAIN.clone(), Token::default()],
        }
    }

    /// Get the GLSLFX filename.
    pub fn get_glslfx_filename(&self) -> &Token {
        &self.glslfx
    }

    /// Get vertex shader entry points.
    pub fn get_vs(&self) -> &[Token; 2] {
        &self.vs
    }

    /// Get fragment shader entry points.
    pub fn get_fs(&self) -> &[Token; 2] {
        &self.fs
    }

    /// Get the primitive type for this shader.
    ///
    /// Image shaders use coarse triangles (single fullscreen tri).
    pub fn get_primitive_type(&self) -> PrimitiveType {
        PrimitiveType::Triangles
    }
}

impl Default for ImageShaderShaderKey {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let key = ImageShaderShaderKey::new();
        assert_eq!(key.get_glslfx_filename(), &Token::new("imageShader.glslfx"));
    }

    #[test]
    fn test_vs_entry() {
        let key = ImageShaderShaderKey::new();
        let vs = key.get_vs();
        assert_eq!(vs[0], Token::new("ImageShader.Vertex"));
        assert_eq!(vs[1], Token::default());
    }

    #[test]
    fn test_fs_entry() {
        let key = ImageShaderShaderKey::new();
        let fs = key.get_fs();
        assert_eq!(fs[0], Token::new("ImageShader.Fragment"));
        assert_eq!(fs[1], Token::default());
    }

    #[test]
    fn test_primitive_type() {
        let key = ImageShaderShaderKey::new();
        assert_eq!(key.get_primitive_type(), PrimitiveType::Triangles);
    }

    #[test]
    fn test_default() {
        let key = ImageShaderShaderKey::default();
        assert_eq!(key.glslfx, Token::new("imageShader.glslfx"));
    }
}
