//! Kind Registry module.
//!
//! The `kind` module provides a registry for categorizing scene elements
//! into hierarchical "kinds". Kinds are used to classify prims in USD stages
//! for purposes like traversal, selection, and rendering.
//!
//! # Core Kinds
//!
//! The built-in kind hierarchy is:
//!
//! ```text
//! model
//! ├── component
//! └── group
//!     └── assembly
//! subcomponent
//! ```
//!
//! - `model` - Base kind for all model-like elements
//! - `component` - A leaf model that cannot contain other models
//! - `group` - A model that contains other models
//! - `assembly` - A special group representing a complete asset
//! - `subcomponent` - A part of a component
//!
//! # Examples
//!
//! ```ignore
//! use usd_kind::{Registry, tokens};
//!
//! // Check if a kind is registered
//! assert!(Registry::has_kind(tokens::model()));
//!
//! // Check kind inheritance
//! assert!(Registry::is_a(tokens::component(), tokens::model()));
//! assert!(Registry::is_a(tokens::assembly(), tokens::group()));
//! ```

pub mod registry;
pub mod tokens;

pub use registry::{
    get_all_kinds, get_base_kind, has_kind, is_a, is_assembly_kind, is_component_kind,
    is_group_kind, is_model_kind, is_subcomponent_kind, kind_tokens,
};
pub use tokens::{KindToken, KindTokens};

/// Re-export built-in kind tokens.
pub mod kinds {
    pub use super::tokens::{ASSEMBLY, COMPONENT, GROUP, MODEL, SUBCOMPONENT};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all exports are accessible
        let _ = tokens::model();
        assert!(has_kind(tokens::model().as_token()));
    }
}
