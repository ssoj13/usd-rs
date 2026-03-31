//! Common types for UsdImaging.

use usd_tf::Token;

/// Property invalidation type for change tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyInvalidationType {
    /// Property was resynced (added/removed)
    Resync,
    /// Property value changed
    PropertyChanged,
}

/// Population mode determines how an adapter handles prims and their descendants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PopulationMode {
    /// Adapter is responsible only for prims of its registered type.
    /// Descendant prims are managed independently.
    RepresentsSelf,

    /// Adapter is responsible for prims of its registered type AND
    /// all descendants. No population occurs for descendant prims.
    /// Changes to descendant prims are sent to this adapter.
    RepresentsSelfAndDescendents,

    /// Changes to prims of this type are sent to the first ancestor
    /// whose adapter has RepresentsSelfAndDescendents mode.
    RepresentedByAncestor,
}

impl Default for PopulationMode {
    fn default() -> Self {
        Self::RepresentsSelf
    }
}

/// Draw mode for prims (for debugging/performance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawMode {
    /// Normal rendering
    Default,
    /// Draw bounding box only
    Bounds,
    /// Draw as cards (textured quads)
    Cards,
    /// Draw origin axes
    Origin,
}

impl DrawMode {
    /// Convert from token name
    pub fn from_token(token: &Token) -> Option<Self> {
        match token.as_str() {
            "default" => Some(Self::Default),
            "bounds" => Some(Self::Bounds),
            "cards" => Some(Self::Cards),
            "origin" => Some(Self::Origin),
            _ => None,
        }
    }

    /// Convert to token name
    pub fn to_token(&self) -> Token {
        Token::new(match self {
            Self::Default => "default",
            Self::Bounds => "bounds",
            Self::Cards => "cards",
            Self::Origin => "origin",
        })
    }
}

impl Default for DrawMode {
    fn default() -> Self {
        Self::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_population_mode_default() {
        assert_eq!(PopulationMode::default(), PopulationMode::RepresentsSelf);
    }

    #[test]
    fn test_draw_mode_conversions() {
        let bounds = DrawMode::Bounds;
        let token = bounds.to_token();
        assert_eq!(token.as_str(), "bounds");
        assert_eq!(DrawMode::from_token(&token), Some(DrawMode::Bounds));
    }

    #[test]
    fn test_draw_mode_invalid_token() {
        let token = Token::new("invalid");
        assert_eq!(DrawMode::from_token(&token), None);
    }

    #[test]
    fn test_property_invalidation_types() {
        let resync = PropertyInvalidationType::Resync;
        let changed = PropertyInvalidationType::PropertyChanged;
        assert_ne!(resync, changed);
    }
}
