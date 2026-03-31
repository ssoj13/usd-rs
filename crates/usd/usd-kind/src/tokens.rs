//! Kind tokens.
//!
//! Provides static, efficient tokens for built-in kinds.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use usd_tf::Token;

/// A kind token wraps a TfToken for kind identification.
///
/// Kind tokens provide static, efficient representation of kind names
/// used throughout the kind registry system.
#[derive(Clone)]
pub struct KindToken {
    token: Token,
}

impl KindToken {
    /// Creates a new kind token with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - The kind name
    pub fn new(name: &'static str) -> Self {
        Self {
            token: Token::new(name),
        }
    }

    /// Returns the token's string value.
    pub fn as_str(&self) -> &str {
        self.token.as_str()
    }

    /// Returns a reference to the underlying TfToken.
    pub fn as_token(&self) -> &Token {
        &self.token
    }

    /// Returns true if this is an empty token.
    pub fn is_empty(&self) -> bool {
        self.token.is_empty()
    }
}

impl fmt::Debug for KindToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("KindToken").field(&self.as_str()).finish()
    }
}

impl fmt::Display for KindToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl PartialEq for KindToken {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl Eq for KindToken {}

impl Hash for KindToken {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq<Token> for KindToken {
    fn eq(&self, other: &Token) -> bool {
        &self.token == other
    }
}

impl PartialEq<str> for KindToken {
    fn eq(&self, other: &str) -> bool {
        self.token.as_str() == other
    }
}

impl PartialEq<&str> for KindToken {
    fn eq(&self, other: &&str) -> bool {
        self.token.as_str() == *other
    }
}

impl From<Token> for KindToken {
    fn from(token: Token) -> Self {
        Self { token }
    }
}

impl From<&str> for KindToken {
    fn from(s: &str) -> Self {
        Self {
            token: Token::new(s),
        }
    }
}

impl AsRef<Token> for KindToken {
    fn as_ref(&self) -> &Token {
        &self.token
    }
}

impl AsRef<str> for KindToken {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Collection of built-in kind tokens.
///
/// This struct provides access to all standard kind tokens used in USD.
#[derive(Debug)]
pub struct KindTokens {
    /// The "model" kind token.
    pub model: KindToken,
    /// The "component" kind token.
    pub component: KindToken,
    /// The "group" kind token.
    pub group: KindToken,
    /// The "assembly" kind token.
    pub assembly: KindToken,
    /// The "subcomponent" kind token.
    pub subcomponent: KindToken,
}

impl KindTokens {
    /// Creates a new set of kind tokens.
    fn new() -> Self {
        Self {
            model: KindToken::new("model"),
            component: KindToken::new("component"),
            group: KindToken::new("group"),
            assembly: KindToken::new("assembly"),
            subcomponent: KindToken::new("subcomponent"),
        }
    }

    /// Returns the singleton instance of kind tokens.
    pub fn get_instance() -> &'static KindTokens {
        static INSTANCE: OnceLock<KindTokens> = OnceLock::new();
        INSTANCE.get_or_init(KindTokens::new)
    }
}

// Static token accessor functions

/// Gets the MODEL kind token (initialized on first access).
///
/// Models are the primary building blocks of USD scenes.
pub fn model() -> &'static KindToken {
    &KindTokens::get_instance().model
}

/// Gets the COMPONENT kind token (initialized on first access).
pub fn component() -> &'static KindToken {
    &KindTokens::get_instance().component
}

/// Gets the GROUP kind token (initialized on first access).
pub fn group() -> &'static KindToken {
    &KindTokens::get_instance().group
}

/// Gets the ASSEMBLY kind token (initialized on first access).
pub fn assembly() -> &'static KindToken {
    &KindTokens::get_instance().assembly
}

/// Gets the SUBCOMPONENT kind token (initialized on first access).
pub fn subcomponent() -> &'static KindToken {
    &KindTokens::get_instance().subcomponent
}

// Re-export with SCREAMING_CASE names for C++ API compatibility

/// The "model" kind token - base kind for all model-like elements.
pub const MODEL: fn() -> &'static KindToken = model;
/// The "component" kind token - a leaf model.
pub const COMPONENT: fn() -> &'static KindToken = component;
/// The "group" kind token - a model containing other models.
pub const GROUP: fn() -> &'static KindToken = group;
/// The "assembly" kind token - a complete asset.
pub const ASSEMBLY: fn() -> &'static KindToken = assembly;
/// The "subcomponent" kind token - a part of a component.
pub const SUBCOMPONENT: fn() -> &'static KindToken = subcomponent;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_token_new() {
        let token = KindToken::new("test");
        assert_eq!(token.as_str(), "test");
        assert!(!token.is_empty());
    }

    #[test]
    fn test_kind_token_empty() {
        let token = KindToken::from(Token::empty());
        assert!(token.is_empty());
    }

    #[test]
    fn test_kind_token_equality() {
        let t1 = KindToken::new("model");
        let t2 = KindToken::new("model");
        let t3 = KindToken::new("component");

        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_kind_token_str_equality() {
        let token = KindToken::new("model");
        assert!(token == "model");
        assert!(token != "component");
    }

    #[test]
    fn test_kind_token_hash() {
        use std::collections::HashSet;

        let t1 = KindToken::new("model");
        let t2 = KindToken::new("model");

        let mut set = HashSet::new();
        set.insert(t1.as_str().to_string());
        assert!(set.contains(t2.as_str()));
    }

    #[test]
    fn test_kind_token_display() {
        let token = KindToken::new("assembly");
        assert_eq!(format!("{}", token), "assembly");
    }

    #[test]
    fn test_kind_token_debug() {
        let token = KindToken::new("component");
        let debug = format!("{:?}", token);
        assert!(debug.contains("KindToken"));
        assert!(debug.contains("component"));
    }

    #[test]
    fn test_kind_tokens_singleton() {
        let tokens1 = KindTokens::get_instance();
        let tokens2 = KindTokens::get_instance();
        assert!(std::ptr::eq(tokens1, tokens2));
    }

    #[test]
    fn test_builtin_tokens() {
        let tokens = KindTokens::get_instance();

        assert_eq!(tokens.model.as_str(), "model");
        assert_eq!(tokens.component.as_str(), "component");
        assert_eq!(tokens.group.as_str(), "group");
        assert_eq!(tokens.assembly.as_str(), "assembly");
        assert_eq!(tokens.subcomponent.as_str(), "subcomponent");
    }

    #[test]
    fn test_static_token_functions() {
        assert_eq!(model().as_str(), "model");
        assert_eq!(component().as_str(), "component");
        assert_eq!(group().as_str(), "group");
        assert_eq!(assembly().as_str(), "assembly");
        assert_eq!(subcomponent().as_str(), "subcomponent");
    }

    #[test]
    fn test_kind_token_from_token() {
        let tf_token = Token::new("custom");
        let kind_token = KindToken::from(tf_token.clone());
        assert_eq!(kind_token.as_token(), &tf_token);
    }

    #[test]
    fn test_kind_token_from_str() {
        let kind_token: KindToken = "custom".into();
        assert_eq!(kind_token.as_str(), "custom");
    }

    #[test]
    fn test_kind_token_clone() {
        let t1 = KindToken::new("test");
        let t2 = t1.clone();
        assert_eq!(t1, t2);
    }
}
