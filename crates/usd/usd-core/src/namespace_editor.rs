//! UsdNamespaceEditor - Namespace editing operations for USD stages.
//!
//! Port of pxr/usd/usd/namespaceEditor.h
//!
//! @warning
//! This code is a work in progress and should not be used in production
//! scenarios. It is currently not feature-complete and subject to change.

use super::prim::Prim;
use super::property::Property;
use super::stage::Stage;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// EditOptions
// ============================================================================

/// Options for how the namespace editor will behave when performing edits.
#[derive(Debug, Clone)]
pub struct EditOptions {
    /// Whether the namespace editor will allow authoring of relocates
    /// in order to perform edits that would otherwise not be possible
    /// because of opinions across composition arcs.
    ///
    /// By default this is set to true. If set to false the namespace editor
    /// will consider edits that require relocates as errors and will not
    /// apply the edit.
    pub allow_relocates_authoring: bool,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            allow_relocates_authoring: true,
        }
    }
}

// ============================================================================
// EditOperation
// ============================================================================

/// Types of namespace edit operations.
#[derive(Debug, Clone)]
pub enum EditOperation {
    /// Delete a prim at the given path.
    DeletePrim(Path),
    /// Move a prim from one path to another.
    MovePrim {
        /// Source path.
        from: Path,
        /// Target path.
        to: Path,
    },
    /// Rename a prim.
    RenamePrim {
        /// Path to the prim.
        path: Path,
        /// New name for the prim.
        new_name: Token,
    },
    /// Reparent a prim under a new parent.
    ReparentPrim {
        /// Path to the prim.
        path: Path,
        /// New parent path.
        new_parent: Path,
    },
    /// Reparent and rename a prim.
    ReparentAndRenamePrim {
        /// Path to the prim.
        path: Path,
        /// New parent path.
        new_parent: Path,
        /// New name for the prim.
        new_name: Token,
    },
    /// Delete a property at the given path.
    DeleteProperty(Path),
    /// Move a property from one path to another.
    MoveProperty {
        /// Source path.
        from: Path,
        /// Target path.
        to: Path,
    },
    /// Rename a property.
    RenameProperty {
        /// Path to the property.
        path: Path,
        /// New name for the property.
        new_name: Token,
    },
    /// Reparent a property under a new prim.
    ReparentProperty {
        /// Path to the property.
        path: Path,
        /// New parent prim path.
        new_parent: Path,
    },
}

// ============================================================================
// CanApplyResult
// ============================================================================

/// Result of checking if edits can be applied.
#[derive(Debug, Clone)]
pub struct CanApplyResult {
    /// Whether the edits can be applied.
    pub success: bool,
    /// Error message if the edits cannot be applied.
    pub error_message: Option<String>,
}

impl CanApplyResult {
    /// Creates a successful result.
    pub fn success() -> Self {
        Self {
            success: true,
            error_message: None,
        }
    }

    /// Creates a failure result with an error message.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error_message: Some(message.into()),
        }
    }
}

// ============================================================================
// UsdNamespaceEditor
// ============================================================================

/// Provides namespace editing operations for USD stages.
///
/// The namespace editor allows batch editing of namespace operations
/// (delete, move, rename, reparent) on prims and properties in a stage.
///
/// @warning
/// This code is a work in progress and should not be used in production
/// scenarios.
pub struct NamespaceEditor {
    /// The primary stage being edited.
    stage: Arc<Stage>,
    /// Edit options.
    options: EditOptions,
    /// Dependent stages that may have composition dependencies.
    dependent_stages: Vec<Arc<Stage>>,
    /// Pending edit operations.
    pending_edits: Vec<EditOperation>,
}

impl NamespaceEditor {
    /// Creates a new namespace editor for the given stage.
    pub fn new(stage: Arc<Stage>) -> Self {
        Self {
            stage,
            options: EditOptions::default(),
            dependent_stages: Vec::new(),
            pending_edits: Vec::new(),
        }
    }

    /// Creates a new namespace editor with the given options.
    pub fn with_options(stage: Arc<Stage>, options: EditOptions) -> Self {
        Self {
            stage,
            options,
            dependent_stages: Vec::new(),
            pending_edits: Vec::new(),
        }
    }

    /// Returns the primary stage.
    pub fn stage(&self) -> &Arc<Stage> {
        &self.stage
    }

    /// Returns the edit options.
    pub fn options(&self) -> &EditOptions {
        &self.options
    }

    // =========================================================================
    // Dependent Stages
    // =========================================================================

    /// Adds the given stage as a dependent stage of this namespace editor.
    pub fn add_dependent_stage(&mut self, stage: Arc<Stage>) {
        if !self.dependent_stages.iter().any(|s| Arc::ptr_eq(s, &stage)) {
            self.dependent_stages.push(stage);
        }
    }

    /// Removes the given stage as a dependent stage of this namespace editor.
    pub fn remove_dependent_stage(&mut self, stage: &Arc<Stage>) {
        self.dependent_stages.retain(|s| !Arc::ptr_eq(s, stage));
    }

    /// Sets the list of dependent stages for this namespace editor.
    pub fn set_dependent_stages(&mut self, stages: Vec<Arc<Stage>>) {
        self.dependent_stages = stages;
    }

    /// Returns the dependent stages.
    pub fn dependent_stages(&self) -> &[Arc<Stage>] {
        &self.dependent_stages
    }

    // =========================================================================
    // Prim Operations
    // =========================================================================

    /// Adds an edit operation to delete the composed prim at the given path.
    ///
    /// Returns true if the path is a valid composed prim path.
    pub fn delete_prim_at_path(&mut self, path: &Path) -> bool {
        if !path.is_prim_path() {
            return false;
        }
        self.pending_edits
            .push(EditOperation::DeletePrim(path.clone()));
        true
    }

    /// Adds an edit operation to move the composed prim at the given path
    /// to a new path.
    ///
    /// Returns true if both paths are valid composed prim paths.
    pub fn move_prim_at_path(&mut self, path: &Path, new_path: &Path) -> bool {
        if !path.is_prim_path() || !new_path.is_prim_path() {
            return false;
        }
        self.pending_edits.push(EditOperation::MovePrim {
            from: path.clone(),
            to: new_path.clone(),
        });
        true
    }

    /// Adds an edit operation to delete the composed prim.
    pub fn delete_prim(&mut self, prim: &Prim) -> bool {
        if !prim.is_valid() {
            return false;
        }
        self.delete_prim_at_path(prim.path())
    }

    /// Adds an edit operation to rename the composed prim.
    pub fn rename_prim(&mut self, prim: &Prim, new_name: &Token) -> bool {
        if !prim.is_valid() || new_name.is_empty() {
            return false;
        }
        self.pending_edits.push(EditOperation::RenamePrim {
            path: prim.path().clone(),
            new_name: new_name.clone(),
        });
        true
    }

    /// Adds an edit operation to reparent the composed prim under a new parent.
    pub fn reparent_prim(&mut self, prim: &Prim, new_parent: &Prim) -> bool {
        if !prim.is_valid() || !new_parent.is_valid() {
            return false;
        }
        self.pending_edits.push(EditOperation::ReparentPrim {
            path: prim.path().clone(),
            new_parent: new_parent.path().clone(),
        });
        true
    }

    /// Adds an edit operation to reparent and rename the composed prim.
    pub fn reparent_prim_with_name(
        &mut self,
        prim: &Prim,
        new_parent: &Prim,
        new_name: &Token,
    ) -> bool {
        if !prim.is_valid() || !new_parent.is_valid() || new_name.is_empty() {
            return false;
        }
        self.pending_edits
            .push(EditOperation::ReparentAndRenamePrim {
                path: prim.path().clone(),
                new_parent: new_parent.path().clone(),
                new_name: new_name.clone(),
            });
        true
    }

    // =========================================================================
    // Property Operations
    // =========================================================================

    /// Adds an edit operation to delete the property at the given path.
    pub fn delete_property_at_path(&mut self, path: &Path) -> bool {
        if !path.is_property_path() {
            return false;
        }
        self.pending_edits
            .push(EditOperation::DeleteProperty(path.clone()));
        true
    }

    /// Adds an edit operation to move the property at the given path to a new path.
    pub fn move_property_at_path(&mut self, path: &Path, new_path: &Path) -> bool {
        if !path.is_property_path() || !new_path.is_property_path() {
            return false;
        }
        self.pending_edits.push(EditOperation::MoveProperty {
            from: path.clone(),
            to: new_path.clone(),
        });
        true
    }

    /// Adds an edit operation to rename the property.
    pub fn rename_property_at_path(&mut self, path: &Path, new_name: &Token) -> bool {
        if !path.is_property_path() || new_name.is_empty() {
            return false;
        }
        self.pending_edits.push(EditOperation::RenameProperty {
            path: path.clone(),
            new_name: new_name.clone(),
        });
        true
    }

    /// Adds an edit operation to delete the composed property.
    ///
    /// Equivalent to `delete_property_at_path(property.path())`.
    /// Matches C++ `DeleteProperty(const UsdProperty &property)`.
    pub fn delete_property(&mut self, property: &Property) -> bool {
        if !property.is_valid() {
            return false;
        }
        self.delete_property_at_path(property.path())
    }

    /// Adds an edit operation to rename the composed property.
    ///
    /// Matches C++ `RenameProperty(const UsdProperty &property, const TfToken &newName)`.
    pub fn rename_property(&mut self, property: &Property, new_name: &Token) -> bool {
        if !property.is_valid() || new_name.is_empty() {
            return false;
        }
        self.rename_property_at_path(property.path(), new_name)
    }

    /// Adds an edit operation to reparent the composed property under a new prim.
    ///
    /// Matches C++ `ReparentProperty(const UsdProperty &property, const UsdPrim &newParent)`.
    pub fn reparent_property(&mut self, property: &Property, new_parent: &Prim) -> bool {
        if !property.is_valid() || !new_parent.is_valid() {
            return false;
        }
        self.pending_edits.push(EditOperation::ReparentProperty {
            path: property.path().clone(),
            new_parent: new_parent.path().clone(),
        });
        true
    }

    /// Adds an edit operation to reparent and rename the composed property.
    ///
    /// Matches C++ `ReparentProperty(const UsdProperty &, const UsdPrim &, const TfToken &)`.
    pub fn reparent_property_with_name(
        &mut self,
        property: &Property,
        new_parent: &Prim,
        new_name: &Token,
    ) -> bool {
        if !property.is_valid() || !new_parent.is_valid() || new_name.is_empty() {
            return false;
        }
        // Build new property path = new_parent/new_name
        let new_parent_path = new_parent.path().clone();
        if let Some(new_path) = new_parent_path.append_property(new_name.get_text()) {
            self.pending_edits.push(EditOperation::MoveProperty {
                from: property.path().clone(),
                to: new_path,
            });
            true
        } else {
            false
        }
    }

    // =========================================================================
    // Applying Edits
    // =========================================================================

    /// Returns the pending edit operations.
    pub fn pending_edits(&self) -> &[EditOperation] {
        &self.pending_edits
    }

    /// Clears all pending edit operations.
    pub fn clear_pending_edits(&mut self) {
        self.pending_edits.clear();
    }

    /// Checks if the pending edits can be applied.
    pub fn can_apply_edits(&self) -> CanApplyResult {
        // Validate all pending edits
        for edit in &self.pending_edits {
            match edit {
                EditOperation::DeletePrim(path) => {
                    if self.stage.get_prim_at_path(path).is_none() {
                        return CanApplyResult::failure(format!(
                            "Prim at path '{}' does not exist",
                            path.get_string()
                        ));
                    }
                }
                EditOperation::MovePrim { from, to } => {
                    if self.stage.get_prim_at_path(from).is_none() {
                        return CanApplyResult::failure(format!(
                            "Source prim at path '{}' does not exist",
                            from.get_string()
                        ));
                    }
                    if self.stage.get_prim_at_path(to).is_some() {
                        return CanApplyResult::failure(format!(
                            "Target path '{}' already exists",
                            to.get_string()
                        ));
                    }
                }
                EditOperation::RenamePrim { path, new_name } => {
                    if self.stage.get_prim_at_path(path).is_none() {
                        return CanApplyResult::failure(format!(
                            "Prim at path '{}' does not exist",
                            path.get_string()
                        ));
                    }
                    // Check if new name would cause a conflict
                    let parent_path = path.get_parent_path();
                    if !parent_path.is_empty() {
                        if let Some(new_path) = parent_path.append_child(new_name.get_text()) {
                            if self.stage.get_prim_at_path(&new_path).is_some() {
                                return CanApplyResult::failure(format!(
                                    "A prim named '{}' already exists under '{}'",
                                    new_name.get_text(),
                                    parent_path.get_string()
                                ));
                            }
                        }
                    }
                }
                EditOperation::ReparentPrim { path, new_parent } => {
                    if self.stage.get_prim_at_path(path).is_none() {
                        return CanApplyResult::failure(format!(
                            "Prim at path '{}' does not exist",
                            path.get_string()
                        ));
                    }
                    // Verify target doesn't already exist
                    let name = path.get_name();
                    if let Some(new_path) = new_parent.append_child(name) {
                        if self.stage.get_prim_at_path(&new_path).is_some() {
                            return CanApplyResult::failure(format!(
                                "Target path '{}' already exists",
                                new_path.get_string()
                            ));
                        }
                    }
                }
                EditOperation::ReparentAndRenamePrim {
                    path,
                    new_parent,
                    new_name,
                } => {
                    if self.stage.get_prim_at_path(path).is_none() {
                        return CanApplyResult::failure(format!(
                            "Prim at path '{}' does not exist",
                            path.get_string()
                        ));
                    }
                    if let Some(new_path) = new_parent.append_child(new_name.get_text()) {
                        if self.stage.get_prim_at_path(&new_path).is_some() {
                            return CanApplyResult::failure(format!(
                                "Target path '{}' already exists",
                                new_path.get_string()
                            ));
                        }
                    }
                }
                _ => {
                    // Property operations: basic validation passes
                    // (properties may not exist yet for delete, which is a no-op)
                }
            }
        }

        CanApplyResult::success()
    }

    /// Copies a prim spec from `from` to `to` across all layers, then deletes `from`.
    fn copy_and_delete_prim(&self, from: &Path, to: &Path) -> bool {
        // Copy spec in each layer that has it, then delete old
        for layer in self.stage.layer_stack() {
            if layer.get_prim_at_path(from).is_some() {
                if !usd_sdf::copy_spec(&layer, from, &layer, to) {
                    return false;
                }
            }
        }
        self.stage.remove_prim(from)
    }

    /// Copies a property spec from `from` to `to` across all layers, then deletes `from`.
    fn copy_and_delete_property(&self, from: &Path, to: &Path) -> bool {
        for layer in self.stage.layer_stack() {
            let has_spec = layer.get_attribute_at_path(from).is_some()
                || layer.get_relationship_at_path(from).is_some();
            if has_spec {
                if !usd_sdf::copy_spec(&layer, from, &layer, to) {
                    return false;
                }
            }
        }
        // Remove old property spec from all layers
        for layer in self.stage.layer_stack() {
            layer.delete_spec(from);
        }
        true
    }

    /// Applies all pending edits.
    ///
    /// Returns true if all edits were successfully applied.
    /// Copies specs to their new paths before deleting the originals.
    pub fn apply_edits(&mut self) -> bool {
        let result = self.can_apply_edits();
        if !result.success {
            return false;
        }

        let edits = std::mem::take(&mut self.pending_edits);

        for edit in edits {
            match edit {
                EditOperation::DeletePrim(path) => {
                    if !self.stage.remove_prim(&path) {
                        return false;
                    }
                }
                EditOperation::MovePrim { from, to } => {
                    // Copy to new location across all layers, then delete old
                    if !self.copy_and_delete_prim(&from, &to) {
                        return false;
                    }
                }
                EditOperation::RenamePrim { path, new_name } => {
                    let parent_path = path.get_parent_path();
                    if let Some(new_path) = parent_path.append_child(new_name.get_text()) {
                        if !self.copy_and_delete_prim(&path, &new_path) {
                            return false;
                        }
                    }
                }
                EditOperation::ReparentPrim { path, new_parent } => {
                    let name = path.get_name();
                    if let Some(new_path) = new_parent.append_child(name) {
                        if !self.copy_and_delete_prim(&path, &new_path) {
                            return false;
                        }
                    }
                }
                EditOperation::ReparentAndRenamePrim {
                    path,
                    new_parent,
                    new_name,
                } => {
                    if let Some(new_path) = new_parent.append_child(new_name.get_text()) {
                        if !self.copy_and_delete_prim(&path, &new_path) {
                            return false;
                        }
                    }
                }
                EditOperation::DeleteProperty(path) => {
                    // Remove property spec from all layers
                    for layer in self.stage.layer_stack() {
                        layer.delete_spec(&path);
                    }
                }
                EditOperation::MoveProperty { from, to } => {
                    if !self.copy_and_delete_property(&from, &to) {
                        return false;
                    }
                }
                EditOperation::RenameProperty { path, new_name } => {
                    let prim_path = path.get_parent_path();
                    if let Some(new_path) = prim_path.append_property(new_name.get_text()) {
                        if !self.copy_and_delete_property(&path, &new_path) {
                            return false;
                        }
                    }
                }
                EditOperation::ReparentProperty { path, new_parent } => {
                    let prop_name = path.get_name();
                    if let Some(new_path) = new_parent.append_property(prop_name) {
                        if !self.copy_and_delete_property(&path, &new_path) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::InitialLoadSet;

    #[test]
    fn test_edit_options_default() {
        let options = EditOptions::default();
        assert!(options.allow_relocates_authoring);
    }

    #[test]
    fn test_can_apply_result() {
        let success = CanApplyResult::success();
        assert!(success.success);
        assert!(success.error_message.is_none());

        let failure = CanApplyResult::failure("error");
        assert!(!failure.success);
        assert_eq!(failure.error_message, Some("error".to_string()));
    }

    #[test]
    fn test_delete_prim_at_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/World", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        let path = Path::from_string("/World").unwrap();
        assert!(editor.delete_prim_at_path(&path));
        assert_eq!(editor.pending_edits().len(), 1);

        let result = editor.can_apply_edits();
        assert!(result.success);

        assert!(editor.apply_edits());
        assert!(stage.get_prim_at_path(&path).is_none());
    }

    #[test]
    fn test_delete_nonexistent_prim_fails_validation() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mut editor = NamespaceEditor::new(stage.clone());

        let path = Path::from_string("/DoesNotExist").unwrap();
        // Adding the op succeeds (path is valid)
        assert!(editor.delete_prim_at_path(&path));
        // But validation fails
        let result = editor.can_apply_edits();
        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("does not exist"));
    }

    #[test]
    fn test_move_prim_at_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Source", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        let src = Path::from_string("/Source").unwrap();
        let dst = Path::from_string("/Dest").unwrap();
        assert!(editor.move_prim_at_path(&src, &dst));

        let result = editor.can_apply_edits();
        assert!(result.success);
    }

    #[test]
    fn test_move_prim_target_exists_fails() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Source", "Xform").unwrap();
        stage.define_prim("/Dest", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        let src = Path::from_string("/Source").unwrap();
        let dst = Path::from_string("/Dest").unwrap();
        assert!(editor.move_prim_at_path(&src, &dst));

        let result = editor.can_apply_edits();
        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("already exists"));
    }

    #[test]
    fn test_rename_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        let new_name = Token::new("Scene");
        assert!(editor.rename_prim(&prim, &new_name));

        let result = editor.can_apply_edits();
        assert!(result.success);
    }

    #[test]
    fn test_rename_prim_empty_name_rejected() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        let empty = Token::new("");
        assert!(!editor.rename_prim(&prim, &empty));
        assert!(editor.pending_edits().is_empty());
    }

    #[test]
    fn test_reparent_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let child = stage.define_prim("/Child", "Xform").unwrap();
        let parent = stage.define_prim("/NewParent", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        assert!(editor.reparent_prim(&child, &parent));
        assert_eq!(editor.pending_edits().len(), 1);
    }

    #[test]
    fn test_clear_pending_edits() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/A", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        let path = Path::from_string("/A").unwrap();
        editor.delete_prim_at_path(&path);
        assert!(!editor.pending_edits().is_empty());

        editor.clear_pending_edits();
        assert!(editor.pending_edits().is_empty());
    }

    #[test]
    fn test_invalid_path_rejected() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mut editor = NamespaceEditor::new(stage);

        // Property paths should be rejected by prim ops
        let prop_path = Path::from_string("/World.attr").unwrap();
        assert!(!editor.delete_prim_at_path(&prop_path));
        assert!(!editor.move_prim_at_path(&prop_path, &prop_path));
        assert!(editor.pending_edits().is_empty());
    }

    #[test]
    fn test_dependent_stages() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let dep = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        assert!(editor.dependent_stages().is_empty());

        editor.add_dependent_stage(dep.clone());
        assert_eq!(editor.dependent_stages().len(), 1);

        // Adding same stage again should not duplicate
        editor.add_dependent_stage(dep.clone());
        assert_eq!(editor.dependent_stages().len(), 1);

        editor.remove_dependent_stage(&dep);
        assert!(editor.dependent_stages().is_empty());
    }

    #[test]
    fn test_delete_property_at_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mut editor = NamespaceEditor::new(stage);

        let prop_path = Path::from_string("/World.myAttr").unwrap();
        assert!(editor.delete_property_at_path(&prop_path));
        assert_eq!(editor.pending_edits().len(), 1);

        // Prim path should be rejected
        let prim_path = Path::from_string("/World").unwrap();
        assert!(!editor.delete_property_at_path(&prim_path));
    }

    #[test]
    fn test_with_options() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let options = EditOptions {
            allow_relocates_authoring: false,
        };
        let editor = NamespaceEditor::with_options(stage.clone(), options);
        assert!(!editor.options().allow_relocates_authoring);
    }

    #[test]
    fn test_empty_edits_can_apply() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let editor = NamespaceEditor::new(stage);
        let result = editor.can_apply_edits();
        assert!(result.success);
    }

    #[test]
    fn test_empty_edits_apply() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mut editor = NamespaceEditor::new(stage);
        assert!(editor.apply_edits());
    }

    // ---- New tests for MEDIUM parity fixes ----

    #[test]
    fn test_reparent_prim_validates_source_exists() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Parent", "Xform").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        // Reparent a non-existent prim
        editor.pending_edits.push(EditOperation::ReparentPrim {
            path: Path::from_string("/DoesNotExist").unwrap(),
            new_parent: Path::from_string("/Parent").unwrap(),
        });

        let result = editor.can_apply_edits();
        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("does not exist"));
    }

    #[test]
    fn test_reparent_prim_validates_target_no_conflict() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Source", "Xform").unwrap();
        stage.define_prim("/Parent", "Xform").unwrap();
        stage.define_prim("/Parent/Source", "Mesh").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        // Reparent /Source under /Parent, but /Parent/Source already exists
        editor.pending_edits.push(EditOperation::ReparentPrim {
            path: Path::from_string("/Source").unwrap(),
            new_parent: Path::from_string("/Parent").unwrap(),
        });

        let result = editor.can_apply_edits();
        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("already exists"));
    }

    #[test]
    fn test_reparent_and_rename_prim_validates() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/A", "Xform").unwrap();
        stage.define_prim("/B", "Xform").unwrap();
        stage.define_prim("/B/NewName", "Mesh").unwrap();

        let mut editor = NamespaceEditor::new(stage.clone());
        editor
            .pending_edits
            .push(EditOperation::ReparentAndRenamePrim {
                path: Path::from_string("/A").unwrap(),
                new_parent: Path::from_string("/B").unwrap(),
                new_name: Token::new("NewName"),
            });

        let result = editor.can_apply_edits();
        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("already exists"));
    }

    /// Helper: create a property on a prim for tests.
    fn make_test_prop(stage: &Arc<Stage>, prim: &Prim, name: &str) -> Property {
        use usd_sdf::value_type_registry::ValueTypeRegistry;
        let type_name = ValueTypeRegistry::instance().find_type("float");
        prim.create_attribute(name, &type_name, false, None);
        let attr = prim.get_attribute(name).unwrap();
        Property::new(std::sync::Arc::downgrade(stage), attr.path().clone())
    }

    #[test]
    fn test_delete_property_via_object() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();
        let prop = make_test_prop(&stage, &prim, "myAttr");

        let mut editor = NamespaceEditor::new(stage.clone());
        assert!(editor.delete_property(&prop));
        assert_eq!(editor.pending_edits().len(), 1);
    }

    #[test]
    fn test_rename_property_via_object() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();
        let prop = make_test_prop(&stage, &prim, "myAttr");

        let mut editor = NamespaceEditor::new(stage.clone());
        let new_name = Token::new("renamedAttr");
        assert!(editor.rename_property(&prop, &new_name));
        assert_eq!(editor.pending_edits().len(), 1);
    }

    #[test]
    fn test_rename_property_empty_name_rejected() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();
        let prop = make_test_prop(&stage, &prim, "myAttr");

        let mut editor = NamespaceEditor::new(stage.clone());
        let empty = Token::new("");
        assert!(!editor.rename_property(&prop, &empty));
        assert!(editor.pending_edits().is_empty());
    }

    #[test]
    fn test_reparent_property_via_objects() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim_a = stage.define_prim("/A", "Xform").unwrap();
        let prim_b = stage.define_prim("/B", "Xform").unwrap();
        let prop = make_test_prop(&stage, &prim_a, "myAttr");

        let mut editor = NamespaceEditor::new(stage.clone());
        assert!(editor.reparent_property(&prop, &prim_b));
        assert_eq!(editor.pending_edits().len(), 1);
    }

    #[test]
    fn test_reparent_property_with_name() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim_a = stage.define_prim("/A", "Xform").unwrap();
        let prim_b = stage.define_prim("/B", "Xform").unwrap();
        let prop = make_test_prop(&stage, &prim_a, "myAttr");

        let mut editor = NamespaceEditor::new(stage.clone());
        let new_name = Token::new("newAttrName");
        assert!(editor.reparent_property_with_name(&prop, &prim_b, &new_name));
        assert_eq!(editor.pending_edits().len(), 1);
    }

    #[test]
    fn test_invalid_property_rejected() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mut editor = NamespaceEditor::new(stage.clone());

        let invalid_prop = Property::invalid();
        assert!(!editor.delete_property(&invalid_prop));
        assert!(!editor.rename_property(&invalid_prop, &Token::new("x")));
        assert!(editor.pending_edits().is_empty());
    }
}
