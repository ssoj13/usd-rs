//! UsdProc tokens for procedural schemas.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdProc/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdProc schemas.
pub struct UsdProcTokensType {
    /// "proceduralSystem" - Procedural system identifier
    pub procedural_system: Token,
    /// "GenerativeProcedural" - Schema identifier
    pub generative_procedural: Token,
}

impl UsdProcTokensType {
    /// Returns all tokens as a vector.
    /// Matches C++ `UsdProcTokensType::allTokens`.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.procedural_system.clone(),
            self.generative_procedural.clone(),
        ]
    }
}

impl UsdProcTokensType {
    fn new() -> Self {
        Self {
            procedural_system: Token::new("proceduralSystem"),
            generative_procedural: Token::new("GenerativeProcedural"),
        }
    }
}

/// Global tokens instance for UsdProc schemas.
pub static USD_PROC_TOKENS: LazyLock<UsdProcTokensType> = LazyLock::new(UsdProcTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(
            USD_PROC_TOKENS.procedural_system.as_str(),
            "proceduralSystem"
        );
        assert_eq!(
            USD_PROC_TOKENS.generative_procedural.as_str(),
            "GenerativeProcedural"
        );
    }
}
