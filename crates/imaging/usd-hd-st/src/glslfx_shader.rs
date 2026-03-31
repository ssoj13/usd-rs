
//! GlslfxShader - GLSLFX shader wrapper for Storm.
//!
//! Port of C++ `HdStGLSLFXShader`.  Wraps a parsed `HioGlslfx` object and
//! implements `HdStShaderCode` for Storm's material-network shader pipeline.
//!
//! The C++ constructor extracts `surfaceShader` and `displacementShader`
//! source sections from the GLSLFX and stores them via `_SetSource`.
//! `Reload()` re-parses the file and refreshes those sources.

use crate::shader_code::{HdStShaderCode, NamedTextureHandle, ShaderParameter, ShaderStage};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use usd_hio::HioGlslfx;
use usd_tf::Token;

/// Shared pointer type.
pub type HdStGlslfxShaderSharedPtr = Arc<HdStGlslfxShader>;

/// Global counter for unique shader IDs.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1_000_000);

/// GLSLFX shader wrapper.
///
/// Wraps a `HioGlslfx` parser result and implements `HdStShaderCode`.
/// Extracts `surfaceShader` -> `Fragment` and `displacementShader` -> `TessEval`.
/// Supports hot-reload via `reload()`.
///
/// Matches C++ `HdStGLSLFXShader`.
pub struct HdStGlslfxShader {
    id: u64,
    glslfx: Arc<HioGlslfx>,
    /// Extracted per-stage sources
    sources: HashMap<ShaderStage, String>,
    named_texture_handles: Vec<NamedTextureHandle>,
    enabled: bool,
}

// HioGlslfx doesn't derive Debug, so we implement it manually.
impl fmt::Debug for HdStGlslfxShader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdStGlslfxShader")
            .field("id", &self.id)
            .field("valid", &self.glslfx.is_valid())
            .field("path", &self.glslfx.get_file_path())
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl HdStGlslfxShader {
    /// Create from an already-parsed `HioGlslfx`.
    ///
    /// Extracts `surfaceShader` -> `Fragment` and `displacementShader` -> `TessEval`.
    pub fn new(glslfx: Arc<HioGlslfx>) -> Self {
        let mut sources = HashMap::new();
        let surface = glslfx.get_surface_source();
        if !surface.is_empty() {
            sources.insert(ShaderStage::Fragment, surface);
        }
        let displ = glslfx.get_displacement_source();
        if !displ.is_empty() {
            sources.insert(ShaderStage::TessEval, displ);
        }
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            glslfx,
            sources,
            named_texture_handles: Vec::new(),
            enabled: true,
        }
    }

    /// Parse from file path; returns `None` if invalid.
    pub fn from_file(path: &str) -> Option<Self> {
        let glslfx = Arc::new(HioGlslfx::from_file(path, None));
        if !glslfx.is_valid() {
            log::warn!("HdStGlslfxShader::from_file: invalid '{}'", path);
            return None;
        }
        Some(Self::new(glslfx))
    }

    /// Parse from in-memory source string.
    pub fn from_string(source: &str) -> Self {
        Self::new(Arc::new(HioGlslfx::from_string(source, None)))
    }

    pub fn get_glslfx(&self) -> &HioGlslfx {
        &self.glslfx
    }
    pub fn get_named_texture_handles(&self) -> &[NamedTextureHandle] {
        &self.named_texture_handles
    }
    pub fn set_named_texture_handles(&mut self, h: Vec<NamedTextureHandle>) {
        self.named_texture_handles = h;
    }
    pub fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn is_glslfx_valid(&self) -> bool {
        self.glslfx.is_valid()
    }

    /// Hot-reload: re-parse from disk and refresh sources.  No-op for in-memory.
    pub fn reload(&mut self) {
        let path = self.glslfx.get_file_path().to_string();
        if path.is_empty() || path == "string" {
            return;
        }
        let new = Arc::new(HioGlslfx::from_file(&path, None));
        if !new.is_valid() {
            log::warn!("HdStGlslfxShader::reload: re-parse failed '{}'", path);
            return;
        }
        let mut sources = HashMap::new();
        let surface = new.get_surface_source();
        if !surface.is_empty() {
            sources.insert(ShaderStage::Fragment, surface);
        }
        let displ = new.get_displacement_source();
        if !displ.is_empty() {
            sources.insert(ShaderStage::TessEval, displ);
        }
        self.glslfx = new;
        self.sources = sources;
        log::debug!("HdStGlslfxShader::reload: ok '{}'", path);
    }
}

impl HdStShaderCode for HdStGlslfxShader {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn get_source(&self, stage: ShaderStage) -> String {
        self.sources.get(&stage).cloned().unwrap_or_default()
    }

    fn get_params(&self) -> Vec<ShaderParameter> {
        self.glslfx
            .get_parameters()
            .iter()
            .map(|p| ShaderParameter::new(Token::new(&p.name), usd_vt::Value::default()))
            .collect()
    }

    fn get_textures(&self) -> Vec<NamedTextureHandle> {
        self.named_texture_handles.clone()
    }
    fn is_valid(&self) -> bool {
        self.glslfx.is_valid()
    }
    fn get_hash(&self) -> u64 {
        self.glslfx.get_hash()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"-- glslfx version 0.1

-- configuration
{
    "techniques": {
        "default": {
            "surfaceShader": { "source": ["Surf"] }
        }
    }
}

-- glsl Surf
void surfaceShader() {}
"#;

    #[test]
    fn test_from_string_valid() {
        let s = HdStGlslfxShader::from_string(MINIMAL);
        assert!(s.is_glslfx_valid());
        assert!(s.is_enabled());
        assert!(!s.get_source(ShaderStage::Fragment).is_empty());
    }

    #[test]
    fn test_from_string_invalid() {
        let s = HdStGlslfxShader::from_string("// not glslfx");
        assert!(!s.is_glslfx_valid());
    }

    #[test]
    fn test_id_unique() {
        let a = HdStGlslfxShader::from_string("");
        let b = HdStGlslfxShader::from_string("");
        assert_ne!(a.get_id(), b.get_id());
    }

    #[test]
    fn test_enable_disable() {
        let mut s = HdStGlslfxShader::from_string("");
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn test_texture_handles() {
        let mut s = HdStGlslfxShader::from_string("");
        s.set_named_texture_handles(vec![NamedTextureHandle::new(
            Token::new("diffuse"),
            Token::new("2D"),
        )]);
        assert_eq!(s.get_textures().len(), 1);
    }

    #[test]
    fn test_from_file_missing() {
        assert!(HdStGlslfxShader::from_file("/nonexistent.glslfx").is_none());
    }

    #[test]
    fn test_reload_noop_string() {
        let mut s = HdStGlslfxShader::from_string(MINIMAL);
        let id = s.get_id();
        s.reload(); // no-op for string source
        assert_eq!(s.get_id(), id);
    }
}
