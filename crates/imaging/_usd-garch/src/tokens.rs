//! Garch tokens for GL-related identifiers.

use once_cell::sync::Lazy;
use usd_tf::Token as TfToken;

/// Garch tokens collection.
pub struct GarchTokens;

impl GarchTokens {
    /// OpenGL API
    pub fn opengl() -> &'static TfToken {
        &OPENGL
    }

    /// GLSL shading language
    pub fn glsl() -> &'static TfToken {
        &GLSL
    }

    /// Core profile
    pub fn core_profile() -> &'static TfToken {
        &CORE_PROFILE
    }

    /// Compatibility profile
    pub fn compatibility_profile() -> &'static TfToken {
        &COMPATIBILITY_PROFILE
    }

    /// Debug context
    pub fn debug_context() -> &'static TfToken {
        &DEBUG_CONTEXT
    }

    /// Forward compatible context
    pub fn forward_compatible() -> &'static TfToken {
        &FORWARD_COMPATIBLE
    }
}

// Token constants
static OPENGL: Lazy<TfToken> = Lazy::new(|| TfToken::new("opengl"));
static GLSL: Lazy<TfToken> = Lazy::new(|| TfToken::new("glsl"));
static CORE_PROFILE: Lazy<TfToken> = Lazy::new(|| TfToken::new("coreProfile"));
static COMPATIBILITY_PROFILE: Lazy<TfToken> = Lazy::new(|| TfToken::new("compatibilityProfile"));
static DEBUG_CONTEXT: Lazy<TfToken> = Lazy::new(|| TfToken::new("debugContext"));
static FORWARD_COMPATIBLE: Lazy<TfToken> = Lazy::new(|| TfToken::new("forwardCompatible"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(GarchTokens::opengl().as_str(), "opengl");
        assert_eq!(GarchTokens::glsl().as_str(), "glsl");
        assert_eq!(GarchTokens::core_profile().as_str(), "coreProfile");
    }
}
