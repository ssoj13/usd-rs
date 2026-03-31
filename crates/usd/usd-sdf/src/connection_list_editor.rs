//! Connection and relationship target list editors.
//!
//! Port of pxr/usd/sdf/connectionListEditor.h
//!
//! List editor implementations that ensure appropriate target specs are
//! created or destroyed when connection/relationship targets are added
//! to the underlying list operation.

use crate::{Layer, ListOpType, Path, SpecType};
use usd_tf::Token;
use std::sync::Arc;

/// Trait for connection child policies.
///
/// Determines whether we're dealing with attribute connections or
/// relationship targets.
pub trait ConnectionChildPolicy: Clone + Send + Sync + 'static {
    /// Returns the spec type for children managed by this policy.
    fn spec_type() -> SpecType;

    /// Returns the token for the connection list field.
    fn list_field() -> Token;
}

/// Connection list editor base.
///
/// List editor implementation that ensures appropriate target specs
/// are created or destroyed when connection/relationship targets are
/// added to the underlying list operation.
pub struct ConnectionListEditor<P: ConnectionChildPolicy> {
    /// The layer containing this editor.
    layer: Option<Arc<Layer>>,
    /// Path to the owning spec (attribute or relationship).
    owner_path: Path,
    /// The connection list field token.
    connection_list_field: Token,
    /// Marker for the child policy type.
    _policy: std::marker::PhantomData<P>,
}

impl<P: ConnectionChildPolicy> ConnectionListEditor<P> {
    /// Creates a new connection list editor.
    pub fn new(layer: Arc<Layer>, owner_path: Path) -> Self {
        Self {
            layer: Some(layer),
            owner_path,
            connection_list_field: P::list_field(),
            _policy: std::marker::PhantomData,
        }
    }

    /// Returns the layer containing this editor.
    pub fn layer(&self) -> Option<&Arc<Layer>> {
        self.layer.as_ref()
    }

    /// Returns the owning spec path.
    pub fn owner_path(&self) -> &Path {
        &self.owner_path
    }

    /// Returns the connection list field token.
    pub fn list_field(&self) -> &Token {
        &self.connection_list_field
    }

    /// Called when an edit is performed on the connection list.
    ///
    /// Creates or removes target specs as needed to keep the spec tree
    /// consistent with the list op edits.
    pub fn on_edit_shared(
        &self,
        _op: ListOpType,
        spec_type: SpecType,
        old_items: &[Path],
        new_items: &[Path],
    ) {
        let Some(layer) = &self.layer else { return };

        // Remove specs for items that were removed from the list.
        for old in old_items {
            if !new_items.contains(old) {
                if let Some(target_path) = self.owner_path.append_target(old) {
                    layer.delete_spec(&target_path);
                }
            }
        }

        // Create specs for newly added items.
        for new_item in new_items {
            if !old_items.contains(new_item) {
                if let Some(target_path) = self.owner_path.append_target(new_item) {
                    layer.create_spec(&target_path, spec_type);
                }
            }
        }
    }
}

/// Attribute connection child policy.
#[derive(Clone)]
pub struct AttributeConnectionChildPolicy;

impl ConnectionChildPolicy for AttributeConnectionChildPolicy {
    fn spec_type() -> SpecType {
        SpecType::Connection
    }

    fn list_field() -> Token {
        Token::from("connectionPaths")
    }
}

/// Relationship target child policy.
#[derive(Clone)]
pub struct RelationshipTargetChildPolicy;

impl ConnectionChildPolicy for RelationshipTargetChildPolicy {
    fn spec_type() -> SpecType {
        SpecType::RelationshipTarget
    }

    fn list_field() -> Token {
        Token::from("targetPaths")
    }
}

/// List editor for attribute connections.
pub type AttributeConnectionListEditor =
    ConnectionListEditor<AttributeConnectionChildPolicy>;

/// List editor for relationship targets.
pub type RelationshipTargetListEditor =
    ConnectionListEditor<RelationshipTargetChildPolicy>;

impl AttributeConnectionListEditor {
    /// Handle edits specific to attribute connections.
    pub fn on_edit(&self, op: ListOpType, old_items: &[Path], new_items: &[Path]) {
        self.on_edit_shared(op, SpecType::Connection, old_items, new_items);
    }
}

impl RelationshipTargetListEditor {
    /// Handle edits specific to relationship targets.
    pub fn on_edit(&self, op: ListOpType, old_items: &[Path], new_items: &[Path]) {
        self.on_edit_shared(op, SpecType::RelationshipTarget, old_items, new_items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attr_connection_policy() {
        assert_eq!(
            AttributeConnectionChildPolicy::spec_type(),
            SpecType::Connection
        );
    }

    #[test]
    fn test_rel_target_policy() {
        assert_eq!(
            RelationshipTargetChildPolicy::spec_type(),
            SpecType::RelationshipTarget
        );
    }
}
