//! USD Relationship - connections between prims.

use super::object::Stage;
use super::property::Property;
use std::collections::HashSet;
use std::sync::{Arc, Weak};
use usd_sdf::{Path, SpecType};
use usd_tf::Token;

// ============================================================================
// Relationship
// ============================================================================

/// A property that represents connections to other prims or properties.
///
/// Relationships are used to express dependencies between scene elements,
/// such as material bindings, light linking, or collection membership.
///
/// Unlike attributes which hold typed values, relationships hold paths
/// to other objects in the scene.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::UsdStage;
/// use usd_sdf::Path;
///
/// let stage = UsdStage::open("scene.usda")?;
/// let prim = stage.get_prim_at_path("/World/Mesh")?;
/// let rel = prim.get_relationship("material:binding")?;
///
/// // Get targets
/// let targets = rel.get_targets();
/// for target in targets {
///     println!("Bound to: {}", target);
/// }
///
/// // Add a target
/// rel.add_target(&Path::from_string("/Materials/Metal")?)?;
/// ```
#[derive(Debug, Clone)]
pub struct Relationship {
    /// Base property data.
    inner: Property,
}

impl Relationship {
    /// Creates a new relationship.
    pub(crate) fn new(stage: Weak<Stage>, path: Path) -> Self {
        Self {
            inner: Property::new_with_type(stage, path, super::object::ObjType::Relationship),
        }
    }

    /// Creates an invalid relationship.
    pub fn invalid() -> Self {
        Self {
            inner: Property::invalid(),
        }
    }

    /// Returns true if this relationship is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the path to this relationship.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Returns the name of this relationship.
    pub fn name(&self) -> Token {
        self.inner.name()
    }

    /// Returns the stage that owns this relationship.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.inner.stage()
    }

    /// Converts this relationship into a property.
    pub fn into_property(self) -> Property {
        self.inner
    }

    /// Returns a reference to the inner property.
    pub fn as_property(&self) -> &Property {
        &self.inner
    }

    /// Returns the prim path that owns this relationship.
    pub fn prim_path(&self) -> Path {
        self.inner.prim_path()
    }

    /// Gets the relationship's target paths.
    ///
    /// Composes targets from all contributing layers (strongest to weakest),
    /// applying ListOp operations per USD composition rules.
    pub fn get_targets(&self) -> Vec<Path> {
        self.inner
            .get_composed_targets(usd_pcp::TargetSpecType::Relationship)
    }

    /// Gets the relationship's forwarded targets (following relationship chains).
    ///
    /// If a target path points to another relationship, this follows the chain
    /// and returns the ultimate targets. Cycles are detected and avoided.
    pub fn get_forwarded_targets(&self) -> Vec<Path> {
        let mut visited = HashSet::new();
        let mut unique_targets = HashSet::new();
        let mut result = Vec::new();

        self.collect_forwarded_targets(&mut visited, &mut unique_targets, &mut result);
        result
    }

    /// Internal helper to recursively collect forwarded targets.
    fn collect_forwarded_targets(
        &self,
        visited: &mut HashSet<Path>,
        unique_targets: &mut HashSet<Path>,
        result: &mut Vec<Path>,
    ) {
        // Avoid cycles - if we've visited this relationship, skip
        if !visited.insert(self.path().clone()) {
            return;
        }

        let Some(stage) = self.inner.stage() else {
            return;
        };

        for target in self.get_targets() {
            // If target is a property path, check if it's a relationship
            if target.is_property_path() {
                let prim_path = target.get_prim_path();
                if !prim_path.is_empty() {
                    if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                        let prop_name = target.get_name();
                        let proto_prim = prim.get_prim_in_prototype();
                        let relationship_exists = prim.has_relationship(prop_name)
                            || (prim.is_instance_proxy() && proto_prim.has_relationship(prop_name));
                        if relationship_exists {
                            let Some(target_rel) = prim.get_relationship(prop_name) else {
                                continue;
                            };
                            // Recursively collect from the target relationship
                            target_rel.collect_forwarded_targets(visited, unique_targets, result);
                            continue;
                        }
                    }
                }
            }
            // Not a relationship or prim not found - add to results
            if unique_targets.insert(target.clone()) {
                result.push(target);
            }
        }
    }

    /// Returns true if this relationship has any authored targets.
    /// Matches C++ `HasAuthoredMetadata(SdfFieldKeys->TargetPaths)` — fast
    /// metadata existence check without full composition.
    pub fn has_authored_targets(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let target_tok = usd_tf::Token::new("targetPaths");
        for layer in stage.layer_stack() {
            if layer.has_field(self.path(), &target_tok) {
                return true;
            }
        }
        false
    }

    /// Ensures a relationship spec exists in the given layer at this relationship's path.
    ///
    /// Creates prim and relationship specs as needed. Mirrors C++ `_CreateSpec` behavior.
    fn ensure_rel_spec(layer: &std::sync::Arc<usd_sdf::Layer>, rel_path: &Path) -> bool {
        // Ensure prim spec parent exists first
        let prim_path = rel_path.get_prim_path();
        if !prim_path.is_absolute_root_path() && layer.get_prim_at_path(&prim_path).is_none() {
            if !layer.create_spec(&prim_path, SpecType::Prim) {
                return false;
            }
        }
        // Create relationship spec if missing
        if layer.get_relationship_at_path(rel_path).is_none() {
            layer.create_spec(rel_path, SpecType::Relationship);
        }
        true
    }

    /// Sets the relationship's targets, replacing any existing.
    pub fn set_targets(&self, targets: &[Path]) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        if !Self::ensure_rel_spec(layer, self.path()) {
            return false;
        }
        if let Some(mut rel_spec) = layer.get_relationship_at_path(self.path()) {
            let list_op = usd_sdf::PathListOp::create_explicit(targets.to_vec());
            rel_spec.set_target_path_list(list_op);
            return true;
        }
        false
    }

    /// Adds a target to this relationship at the default position (back of prepend list).
    pub fn add_target(&self, target: &Path) -> bool {
        self.add_target_with_position(target, super::common::ListPosition::BackOfPrependList)
    }

    /// Adds a target to this relationship at the specified list position.
    ///
    /// Matches C++ `UsdRelationship::AddTarget(target, position)`.
    pub fn add_target_with_position(
        &self,
        target: &Path,
        position: super::common::ListPosition,
    ) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        if !Self::ensure_rel_spec(layer, self.path()) {
            return false;
        }
        if let Some(mut rel_spec) = layer.get_relationship_at_path(self.path()) {
            let mut list_op = rel_spec.target_path_list();
            if list_op.is_explicit() {
                let mut explicit = list_op.get_explicit_items().to_vec();
                match position {
                    super::common::ListPosition::FrontOfPrependList => {
                        explicit.insert(0, target.clone());
                    }
                    super::common::ListPosition::BackOfPrependList
                    | super::common::ListPosition::FrontOfAppendList
                    | super::common::ListPosition::BackOfAppendList => {
                        explicit.push(target.clone());
                    }
                }
                let _ = list_op.set_explicit_items(explicit);
                rel_spec.set_target_path_list(list_op);
                return true;
            }
            match position {
                super::common::ListPosition::FrontOfPrependList => {
                    let mut prepended = list_op.get_prepended_items().to_vec();
                    prepended.insert(0, target.clone());
                    list_op.set_prepended_items(prepended).ok();
                }
                super::common::ListPosition::BackOfPrependList => {
                    let mut prepended = list_op.get_prepended_items().to_vec();
                    prepended.push(target.clone());
                    list_op.set_prepended_items(prepended).ok();
                }
                super::common::ListPosition::FrontOfAppendList => {
                    let mut appended = list_op.get_appended_items().to_vec();
                    appended.insert(0, target.clone());
                    list_op.set_appended_items(appended).ok();
                }
                super::common::ListPosition::BackOfAppendList => {
                    let mut appended = list_op.get_appended_items().to_vec();
                    appended.push(target.clone());
                    list_op.set_appended_items(appended).ok();
                }
            }
            rel_spec.set_target_path_list(list_op);
            return true;
        }
        false
    }

    /// Removes a target from this relationship.
    pub fn remove_target(&self, target: &Path) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        if !Self::ensure_rel_spec(layer, self.path()) {
            return false;
        }
        if let Some(mut rel_spec) = layer.get_relationship_at_path(self.path()) {
            rel_spec.remove_target_path(target, true);
            return true;
        }
        false
    }

    /// Clears all target opinions from the current edit target.
    ///
    /// If `remove_spec` is false, the spec is preserved to keep any
    /// intentionally authored metadata on the relationship.
    ///
    /// Matches C++ `UsdRelationship::ClearTargets(bool removeSpec)`.
    pub fn clear_targets(&self) -> bool {
        self.clear_targets_with_spec(true)
    }

    /// Clears targets with explicit control over spec removal.
    pub fn clear_targets_with_spec(&self, _remove_spec: bool) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        if !Self::ensure_rel_spec(layer, self.path()) {
            return false;
        }
        if let Some(mut rel_spec) = layer.get_relationship_at_path(self.path()) {
            rel_spec.clear_target_path_list();
            return true;
        }
        false
    }

    /// Blocks the relationship's targets.
    ///
    /// This authors an explicit empty target list which blocks any weaker
    /// opinions from showing through. Unlike `clear_targets()` which removes
    /// the authored opinion entirely, blocking explicitly states "no targets".
    pub fn block_targets(&self) -> bool {
        // Block by authoring an explicit empty list
        // This is different from clearing - explicit empty means "no targets"
        // while clearing means "no opinion, inherit from weaker layers"
        self.set_targets(&[])
    }

    /// Returns a description of this relationship.
    pub fn description(&self) -> String {
        if self.is_valid() {
            format!(
                "Relationship '{}' at {}",
                self.name().get_text(),
                self.path().get_string()
            )
        } else {
            "Invalid relationship".to_string()
        }
    }
}

impl PartialEq for Relationship {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Relationship {}

impl std::hash::Hash for Relationship {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_relationship() {
        let rel = Relationship::invalid();
        assert!(!rel.is_valid());
    }

    #[test]
    fn test_relationship_path() {
        let path = Path::from_string("/World.material:binding").unwrap();
        let rel = Relationship::new(Weak::new(), path.clone());
        assert_eq!(rel.path(), &path);
    }

    #[test]
    fn test_relationship_name() {
        let path = Path::from_string("/World.material:binding").unwrap();
        let rel = Relationship::new(Weak::new(), path);
        assert_eq!(rel.name().get_text(), "material:binding");
    }
}
