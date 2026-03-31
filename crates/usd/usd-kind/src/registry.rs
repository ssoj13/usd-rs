//! Kind Registry - singleton registry for model kind information.
//!
//! Port of pxr/usd/kind/registry.h/cpp
//!
//! Provides access to kind hierarchy information for model kinds.

use std::collections::HashMap;
use std::sync::OnceLock;
use usd_plug::PlugRegistry;
use usd_tf::Token;

use crate::tokens;

/// Returns the static kind tokens instance.
/// Re-exports `tokens::KindTokens` for backward compatibility.
pub fn kind_tokens() -> &'static tokens::KindTokens {
    tokens::KindTokens::get_instance()
}

// ============================================================================
// KindRegistry
// ============================================================================

/// Singleton registry that holds known kinds and information about them.
///
/// Matches C++ `KindRegistry`.
///
/// Currently implements a simplified version with built-in kinds only.
/// Full implementation would support plugin-based kind extensions.
struct KindRegistry {
    /// Map from kind to base kind.
    kind_map: HashMap<Token, Token>,
}

impl KindRegistry {
    /// Returns the singleton instance.
    fn get_instance() -> &'static KindRegistry {
        static INSTANCE: OnceLock<KindRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let mut registry = KindRegistry {
                kind_map: HashMap::new(),
            };
            registry.register_defaults();
            registry
        })
    }

    /// Registers default built-in kinds.
    fn register_defaults(&mut self) {
        let tokens = kind_tokens();

        // Register built-in kind hierarchy:
        // subcomponent (no base)
        // model (no base)
        // component -> model
        // group -> model
        // assembly -> group -> model

        self.register(tokens.subcomponent.as_token().clone(), Token::new(""));
        self.register(tokens.model.as_token().clone(), Token::new(""));
        self.register(
            tokens.component.as_token().clone(),
            tokens.model.as_token().clone(),
        );
        self.register(
            tokens.group.as_token().clone(),
            tokens.model.as_token().clone(),
        );
        self.register(
            tokens.assembly.as_token().clone(),
            tokens.group.as_token().clone(),
        );

        // Load custom kinds from registered plugins (C++ kind/registry.cpp:199-241)
        let plug_registry = PlugRegistry::get_instance();
        for plugin in plug_registry.get_all_plugins() {
            let metadata = plugin.get_metadata();
            let kinds_obj = match metadata.get("Kinds").and_then(|v| v.as_object()) {
                Some(obj) => obj,
                None => continue,
            };
            for (kind_name, kind_value) in kinds_obj {
                let base_kind = kind_value
                    .as_object()
                    .and_then(|d| d.get("baseKind"))
                    .and_then(|v| v.as_string())
                    .unwrap_or("");
                self.register(Token::new(kind_name), Token::new(base_kind));
            }
        }
    }

    /// Register a kind with its base kind.
    /// Validates the identifier and rejects duplicates (matches C++ _Register).
    fn register(&mut self, kind: Token, base_kind: Token) {
        // Validate: kind must be a valid identifier (alphanumeric + underscore)
        let kind_str = kind.get_text();
        if kind_str.is_empty()
            || !kind_str.chars().next().unwrap_or('0').is_alphabetic()
            || !kind_str.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            log::error!("Invalid kind: '{}'", kind_str);
            return;
        }

        // Reject duplicate registrations
        if self.kind_map.contains_key(&kind) {
            log::error!("Kind '{}' has already been registered", kind_str);
            return;
        }

        self.kind_map.insert(kind, base_kind);
    }

    /// Checks if a kind is known to the registry.
    fn has_kind(&self, kind: &Token) -> bool {
        self.kind_map.contains_key(kind)
    }

    /// Returns the base kind of the given kind.
    /// Logs a warning for unknown kinds (matches C++ TF_CODING_ERROR).
    fn get_base_kind(&self, kind: &Token) -> Token {
        match self.kind_map.get(kind) {
            Some(base) => base.clone(),
            None => {
                log::warn!("Unknown kind: '{}'", kind.get_text());
                Token::new("")
            }
        }
    }

    /// Tests whether derivedKind is the same as baseKind or has it as a base kind.
    fn is_a(&self, derived_kind: &Token, base_kind: &Token) -> bool {
        if *derived_kind == *base_kind {
            return true;
        }

        if let Some(base) = self.kind_map.get(derived_kind) {
            if base.is_empty() {
                return false;
            }
            return self.is_a(base, base_kind);
        }

        false
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Test whether kind is known to the registry.
///
/// Matches C++ `KindRegistry::HasKind(const TfToken& kind)`.
pub fn has_kind(kind: &Token) -> bool {
    KindRegistry::get_instance().has_kind(kind)
}

/// Return the base kind of the given kind.
///
/// Matches C++ `KindRegistry::GetBaseKind(const TfToken &kind)`.
pub fn get_base_kind(kind: &Token) -> Token {
    KindRegistry::get_instance().get_base_kind(kind)
}

/// Test whether derivedKind is the same as baseKind or has it as a base kind.
///
/// Matches C++ `KindRegistry::IsA(const TfToken& derivedKind, const TfToken &baseKind)`.
pub fn is_a(derived_kind: &Token, base_kind: &Token) -> bool {
    KindRegistry::get_instance().is_a(derived_kind, base_kind)
}

/// Returns true if kind IsA model kind.
///
/// Matches C++ `KindRegistry::IsModel(const TfToken& kind)`.
pub fn is_model_kind(kind: &Token) -> bool {
    is_a(kind, kind_tokens().model.as_token())
}

/// Returns true if kind IsA group kind.
///
/// Matches C++ `KindRegistry::IsGroup(const TfToken& kind)`.
pub fn is_group_kind(kind: &Token) -> bool {
    is_a(kind, kind_tokens().group.as_token())
}

/// Returns true if kind IsA assembly kind.
///
/// Matches C++ `KindRegistry::IsAssembly(const TfToken& kind)`.
pub fn is_assembly_kind(kind: &Token) -> bool {
    is_a(kind, kind_tokens().assembly.as_token())
}

/// Returns true if kind IsA component kind.
///
/// Matches C++ `KindRegistry::IsComponent(const TfToken& kind)`.
pub fn is_component_kind(kind: &Token) -> bool {
    is_a(kind, kind_tokens().component.as_token())
}

/// Returns true if kind IsA subcomponent kind.
///
/// Matches C++ `KindRegistry::IsSubComponent(const TfToken& kind)`.
pub fn is_subcomponent_kind(kind: &Token) -> bool {
    is_a(kind, kind_tokens().subcomponent.as_token())
}

/// Return an unordered vector of all kinds known to the registry.
///
/// Matches C++ `KindRegistry::GetAllKinds()`.
pub fn get_all_kinds() -> Vec<Token> {
    KindRegistry::get_instance()
        .kind_map
        .keys()
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Token {
        kind_tokens().model.as_token().clone()
    }
    fn group() -> Token {
        kind_tokens().group.as_token().clone()
    }
    fn assembly() -> Token {
        kind_tokens().assembly.as_token().clone()
    }
    fn component() -> Token {
        kind_tokens().component.as_token().clone()
    }
    fn subcomponent() -> Token {
        kind_tokens().subcomponent.as_token().clone()
    }

    // -------------------------------------------------------------------------
    // has_kind
    // -------------------------------------------------------------------------

    #[test]
    fn test_has_kind_builtins() {
        assert!(has_kind(&model()));
        assert!(has_kind(&group()));
        assert!(has_kind(&assembly()));
        assert!(has_kind(&component()));
        assert!(has_kind(&subcomponent()));
    }

    #[test]
    fn test_has_kind_unknown() {
        assert!(!has_kind(&Token::new("unknown_xyz")));
        assert!(!has_kind(&Token::new("")));
    }

    // -------------------------------------------------------------------------
    // get_base_kind
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_base_kind_model() {
        // model has no base (empty token)
        assert!(get_base_kind(&model()).is_empty());
    }

    #[test]
    fn test_get_base_kind_component() {
        assert_eq!(get_base_kind(&component()), model());
    }

    #[test]
    fn test_get_base_kind_group() {
        assert_eq!(get_base_kind(&group()), model());
    }

    #[test]
    fn test_get_base_kind_assembly() {
        // assembly base is group
        assert_eq!(get_base_kind(&assembly()), group());
    }

    #[test]
    fn test_get_base_kind_subcomponent() {
        assert!(get_base_kind(&subcomponent()).is_empty());
    }

    // -------------------------------------------------------------------------
    // is_a — transitivity
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_a_identity() {
        // Every kind IsA itself
        assert!(is_a(&model(), &model()));
        assert!(is_a(&group(), &group()));
        assert!(is_a(&assembly(), &assembly()));
        assert!(is_a(&component(), &component()));
        assert!(is_a(&subcomponent(), &subcomponent()));
    }

    #[test]
    fn test_is_a_direct_base() {
        assert!(is_a(&component(), &model())); // component -> model
        assert!(is_a(&group(), &model())); // group -> model
        assert!(is_a(&assembly(), &group())); // assembly -> group
    }

    #[test]
    fn test_is_a_transitive() {
        // assembly -> group -> model: transitivity must hold
        assert!(is_a(&assembly(), &model()));
    }

    #[test]
    fn test_is_a_not_related() {
        // component and group are siblings — not related
        assert!(!is_a(&component(), &group()));
        assert!(!is_a(&group(), &component()));
        // subcomponent is independent
        assert!(!is_a(&subcomponent(), &model()));
        assert!(!is_a(&model(), &subcomponent()));
    }

    fn unknown() -> Token {
        Token::new("totally_unknown_kind")
    }

    #[test]
    fn test_is_a_unknown_kind() {
        // Unknown kind returns false (not a coding error per C++ comment)
        assert!(!is_a(&unknown(), &model()));
        assert!(!is_a(&model(), &unknown()));
    }

    #[test]
    fn test_is_a_unknown_vs_unknown() {
        let a = Token::new("aaa");
        let b = Token::new("bbb");
        // Different unknown kinds are not related
        assert!(!is_a(&a, &b));
        // C++ returns true for is_a(x, x) even for unknown kinds:
        // the identity check fires before the map lookup.
        assert!(is_a(&a, &a));
    }

    // -------------------------------------------------------------------------
    // Convenience predicates
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_model_kind() {
        assert!(is_model_kind(&model()));
        assert!(is_model_kind(&component()));
        assert!(is_model_kind(&group()));
        assert!(is_model_kind(&assembly())); // assembly->group->model
        assert!(!is_model_kind(&subcomponent()));
        assert!(!is_model_kind(&unknown()));
    }

    #[test]
    fn test_is_group_kind() {
        assert!(is_group_kind(&group()));
        assert!(is_group_kind(&assembly())); // assembly->group
        assert!(!is_group_kind(&component()));
        assert!(!is_group_kind(&model()));
    }

    #[test]
    fn test_is_assembly_kind() {
        assert!(is_assembly_kind(&assembly()));
        assert!(!is_assembly_kind(&group()));
        assert!(!is_assembly_kind(&component()));
    }

    #[test]
    fn test_is_component_kind() {
        assert!(is_component_kind(&component()));
        assert!(!is_component_kind(&assembly()));
        assert!(!is_component_kind(&group()));
    }

    #[test]
    fn test_is_subcomponent_kind() {
        assert!(is_subcomponent_kind(&subcomponent()));
        assert!(!is_subcomponent_kind(&model()));
        assert!(!is_subcomponent_kind(&component()));
    }

    // -------------------------------------------------------------------------
    // get_all_kinds
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_all_kinds_contains_builtins() {
        let all = get_all_kinds();
        assert!(all.iter().any(|k| k == &model()));
        assert!(all.iter().any(|k| k == &group()));
        assert!(all.iter().any(|k| k == &assembly()));
        assert!(all.iter().any(|k| k == &component()));
        assert!(all.iter().any(|k| k == &subcomponent()));
        assert_eq!(all.len(), 5, "Expected exactly 5 built-in kinds");
    }
}
