//! Debug codes for GLF diagnostics.
//!
//! Port of pxr/imaging/glf/debugCodes.h

/// Debug codes for GLF subsystem.
///
/// Used with the debug logging system to enable/disable diagnostic output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlfDebugCode {
    /// Debug output for context capabilities detection
    ContextCaps,
    /// Error stacktrace output
    ErrorStacktrace,
    /// Debug output for shadow texture operations
    ShadowTextures,
    /// Dump shadow textures to disk for debugging
    DumpShadowTextures,
    /// Debug output for post-surface lighting operations
    PostSurfaceLighting,
}

impl GlfDebugCode {
    /// Get the string representation of this debug code.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContextCaps => "GLF_DEBUG_CONTEXT_CAPS",
            Self::ErrorStacktrace => "GLF_DEBUG_ERROR_STACKTRACE",
            Self::ShadowTextures => "GLF_DEBUG_SHADOW_TEXTURES",
            Self::DumpShadowTextures => "GLF_DEBUG_DUMP_SHADOW_TEXTURES",
            Self::PostSurfaceLighting => "GLF_DEBUG_POST_SURFACE_LIGHTING",
        }
    }

    /// Get all debug codes.
    pub fn all() -> &'static [GlfDebugCode] {
        &[
            Self::ContextCaps,
            Self::ErrorStacktrace,
            Self::ShadowTextures,
            Self::DumpShadowTextures,
            Self::PostSurfaceLighting,
        ]
    }
}

impl std::fmt::Display for GlfDebugCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_codes() {
        assert_eq!(GlfDebugCode::ContextCaps.as_str(), "GLF_DEBUG_CONTEXT_CAPS");
        assert_eq!(GlfDebugCode::all().len(), 5);
    }

    #[test]
    fn test_display() {
        let code = GlfDebugCode::ShadowTextures;
        assert_eq!(format!("{code}"), "GLF_DEBUG_SHADOW_TEXTURES");
    }
}
