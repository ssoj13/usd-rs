//! Token definitions for UsdProcImaging.
//!
//! Private tokens used internally by the usdProcImaging module.

use std::sync::LazyLock;
use usd_tf::Token as TfToken;

/// Private tokens for UsdProcImaging internal use.
#[derive(Debug, Clone, Copy)]
pub struct UsdProcImagingTokens;

impl UsdProcImagingTokens {
    /// Token for inert generative procedural type.
    /// Used when no procedural system is specified.
    pub fn inert_generative_procedural() -> &'static TfToken {
        &INERT_GENERATIVE_PROCEDURAL
    }
}

// Private token constants
static INERT_GENERATIVE_PROCEDURAL: LazyLock<TfToken> =
    LazyLock::new(|| TfToken::new("inertGenerativeProcedural"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        let token = UsdProcImagingTokens::inert_generative_procedural();
        assert_eq!(token.as_str(), "inertGenerativeProcedural");
    }
}
