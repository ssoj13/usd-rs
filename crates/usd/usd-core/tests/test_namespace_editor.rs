// Port of testUsdNamespaceEditor.py — core prim operations subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdNamespaceEditor.py

mod common;

use usd_core::namespace_editor::{CanApplyResult, EditOptions, NamespaceEditor};
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;
use usd_tf::Token;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/A", "Xform").expect("define /A");
    stage.define_prim("/A/B", "Xform").expect("define /A/B");
    stage.define_prim("/A/B/C", "Scope").expect("define /A/B/C");
    stage.define_prim("/D", "Mesh").expect("define /D");
    stage
}

// ============================================================================
// EditOptions
// ============================================================================

#[test]
fn edit_options_default() {
    let opts = EditOptions::default();
    assert!(opts.allow_relocates_authoring);
}

#[test]
fn edit_options_custom() {
    let opts = EditOptions {
        allow_relocates_authoring: false,
    };
    assert!(!opts.allow_relocates_authoring);
}

// ============================================================================
// CanApplyResult
// ============================================================================

#[test]
fn can_apply_result_success() {
    let result = CanApplyResult::success();
    assert!(result.success);
    assert!(result.error_message.is_none());
}

#[test]
fn can_apply_result_failure() {
    let result = CanApplyResult::failure("some error");
    assert!(!result.success);
    assert_eq!(result.error_message.as_deref(), Some("some error"));
}

// ============================================================================
// NamespaceEditor construction
// ============================================================================

#[test]
fn editor_new() {
    let stage = setup_stage();
    let editor = NamespaceEditor::new(stage.clone());
    assert!(std::sync::Arc::ptr_eq(editor.stage(), &stage));
    assert!(editor.options().allow_relocates_authoring);
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn editor_with_options() {
    let stage = setup_stage();
    let opts = EditOptions {
        allow_relocates_authoring: false,
    };
    let editor = NamespaceEditor::with_options(stage.clone(), opts);
    assert!(!editor.options().allow_relocates_authoring);
}

// ============================================================================
// Dependent stages
// ============================================================================

#[test]
fn dependent_stages_add_remove() {
    let stage = setup_stage();
    let dep1 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("dep1");
    let dep2 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("dep2");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.dependent_stages().is_empty());

    editor.add_dependent_stage(dep1.clone());
    assert_eq!(editor.dependent_stages().len(), 1);

    // Adding same stage again should not duplicate
    editor.add_dependent_stage(dep1.clone());
    assert_eq!(editor.dependent_stages().len(), 1);

    editor.add_dependent_stage(dep2.clone());
    assert_eq!(editor.dependent_stages().len(), 2);

    editor.remove_dependent_stage(&dep1);
    assert_eq!(editor.dependent_stages().len(), 1);

    editor.remove_dependent_stage(&dep2);
    assert!(editor.dependent_stages().is_empty());
}

#[test]
fn dependent_stages_set() {
    let stage = setup_stage();
    let dep1 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("dep1");
    let dep2 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("dep2");

    let mut editor = NamespaceEditor::new(stage.clone());
    editor.set_dependent_stages(vec![dep1.clone(), dep2.clone()]);
    assert_eq!(editor.dependent_stages().len(), 2);

    editor.set_dependent_stages(vec![]);
    assert!(editor.dependent_stages().is_empty());
}

// ============================================================================
// Delete prim
// ============================================================================

#[test]
fn delete_prim_at_path() {
    // C++ ref: test_BasicDeletePrim — DeletePrimAtPath variant
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path = Path::from_string("/D").expect("path");
    assert!(editor.delete_prim_at_path(&path));
    assert_eq!(editor.pending_edits().len(), 1);

    let result = editor.can_apply_edits();
    assert!(result.success);

    assert!(editor.apply_edits());
    assert!(stage.get_prim_at_path(&path).is_none());
}

#[test]
fn delete_prim() {
    // C++ ref: test_BasicDeletePrim — DeletePrim variant
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/D").expect("path"))
        .expect("prim /D");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.delete_prim(&prim));
    assert!(editor.apply_edits());
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/D").expect("path"))
            .is_none()
    );
}

#[test]
fn delete_prim_with_children() {
    // C++ ref: deleting /A should remove /A/B and /A/B/C too
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path_a = Path::from_string("/A").expect("path");
    assert!(editor.delete_prim_at_path(&path_a));
    assert!(editor.apply_edits());

    assert!(stage.get_prim_at_path(&path_a).is_none());
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/A/B").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/A/B/C").expect("p"))
            .is_none()
    );
    // /D should still exist
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/D").expect("p"))
            .is_some()
    );
}

#[test]
fn delete_nonexistent_prim_cannot_apply() {
    // C++ ref: test_BasicCanEditPrim — delete nonexistent
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path = Path::from_string("/DoesNotExist").expect("path");
    // Adding the operation succeeds (path is a valid prim path)
    assert!(editor.delete_prim_at_path(&path));
    // But validation fails
    let result = editor.can_apply_edits();
    assert!(!result.success);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("does not exist")
    );
}

// ============================================================================
// Rename prim
// ============================================================================

#[test]
fn rename_prim() {
    // C++ ref: test_BasicRenamePrim
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/D").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.rename_prim(&prim, &Token::new("NewD")));

    let result = editor.can_apply_edits();
    assert!(result.success);

    assert!(editor.apply_edits());
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/D").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/NewD").expect("p"))
            .is_some()
    );
}

#[test]
fn rename_prim_empty_name_rejected() {
    // C++ ref: test_BasicCanEditPrim — invalid name
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/D").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    // Empty name should be rejected at add time
    assert!(!editor.rename_prim(&prim, &Token::new("")));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn rename_prim_conflict_cannot_apply() {
    // C++ ref: test_BasicCanEditPrim — renaming to name that already exists
    let stage = setup_stage();
    let prim_d = stage
        .get_prim_at_path(&Path::from_string("/D").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    // Try to rename /D to "A" — /A already exists
    assert!(editor.rename_prim(&prim_d, &Token::new("A")));
    let result = editor.can_apply_edits();
    assert!(!result.success);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("already exists")
    );
}

// ============================================================================
// Move prim at path
// ============================================================================

#[test]
fn move_prim_at_path() {
    // C++ ref: test_BasicRenamePrim — MovePrimAtPath variant
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let src = Path::from_string("/D").expect("p");
    let dst = Path::from_string("/E").expect("p");
    assert!(editor.move_prim_at_path(&src, &dst));
    assert!(editor.apply_edits());

    assert!(stage.get_prim_at_path(&src).is_none());
    assert!(stage.get_prim_at_path(&dst).is_some());
}

#[test]
fn move_prim_target_exists_cannot_apply() {
    // C++ ref: test_BasicCanEditPrim — target exists
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let src = Path::from_string("/D").expect("p");
    let dst = Path::from_string("/A").expect("p");
    assert!(editor.move_prim_at_path(&src, &dst));

    let result = editor.can_apply_edits();
    assert!(!result.success);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("already exists")
    );
}

#[test]
fn move_prim_source_nonexistent() {
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let src = Path::from_string("/Nope").expect("p");
    let dst = Path::from_string("/Where").expect("p");
    assert!(editor.move_prim_at_path(&src, &dst));

    let result = editor.can_apply_edits();
    assert!(!result.success);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("does not exist")
    );
}

// ============================================================================
// Reparent prim
// ============================================================================

#[test]
fn reparent_prim() {
    // C++ ref: test_BasicReparentPrim
    let stage = setup_stage();

    let prim_d = stage
        .get_prim_at_path(&Path::from_string("/D").expect("p"))
        .expect("prim");
    let prim_a = stage
        .get_prim_at_path(&Path::from_string("/A").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.reparent_prim(&prim_d, &prim_a));

    let result = editor.can_apply_edits();
    assert!(result.success);

    assert!(editor.apply_edits());
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/D").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/A/D").expect("p"))
            .is_some()
    );
}

#[test]
fn reparent_prim_with_name() {
    // C++ ref: test_BasicReparentAndRenamePrim
    let stage = setup_stage();

    let prim_d = stage
        .get_prim_at_path(&Path::from_string("/D").expect("p"))
        .expect("prim");
    let prim_a = stage
        .get_prim_at_path(&Path::from_string("/A").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.reparent_prim_with_name(&prim_d, &prim_a, &Token::new("NewD")));

    let result = editor.can_apply_edits();
    assert!(result.success);

    assert!(editor.apply_edits());
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/D").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/A/NewD").expect("p"))
            .is_some()
    );
}

// ============================================================================
// Invalid path handling
// ============================================================================

#[test]
fn delete_prim_property_path_rejected() {
    // C++ ref: test_BasicCanEditPrim — invalid prim paths
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    // Property path should be rejected for prim operations
    let prop_path = Path::from_string("/A.myAttr").expect("p");
    assert!(!editor.delete_prim_at_path(&prop_path));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn move_prim_property_path_rejected() {
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let prop_path = Path::from_string("/A.myAttr").expect("p");
    let dst = Path::from_string("/B").expect("p");
    assert!(!editor.move_prim_at_path(&prop_path, &dst));

    let src = Path::from_string("/A").expect("p");
    assert!(!editor.move_prim_at_path(&src, &prop_path));

    assert!(editor.pending_edits().is_empty());
}

// ============================================================================
// Pending edits management
// ============================================================================

#[test]
fn clear_pending_edits() {
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path = Path::from_string("/A").expect("p");
    editor.delete_prim_at_path(&path);
    assert!(!editor.pending_edits().is_empty());

    editor.clear_pending_edits();
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn multiple_edits_accumulate() {
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path_a = Path::from_string("/A").expect("p");
    let path_d = Path::from_string("/D").expect("p");
    editor.delete_prim_at_path(&path_a);
    editor.delete_prim_at_path(&path_d);
    assert_eq!(editor.pending_edits().len(), 2);
}

// ============================================================================
// Apply edits end-to-end
// ============================================================================

#[test]
fn apply_edits_delete_and_rename() {
    // Multiple operations in sequence: delete /D, then rename /A to /Root
    let stage = setup_stage();
    let prim_a = stage
        .get_prim_at_path(&Path::from_string("/A").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    let path_d = Path::from_string("/D").expect("p");
    editor.delete_prim_at_path(&path_d);
    assert!(editor.apply_edits());

    // Now rename /A
    editor.rename_prim(&prim_a, &Token::new("Root"));
    assert!(editor.apply_edits());

    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/D").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/A").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Root").expect("p"))
            .is_some()
    );
    // Children should follow the rename
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Root/B").expect("p"))
            .is_some()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Root/B/C").expect("p"))
            .is_some()
    );
}

#[test]
fn apply_move_preserves_children() {
    // C++ ref: test_BasicReparentPrim — children move with parent
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let src = Path::from_string("/A").expect("p");
    let dst = Path::from_string("/Moved").expect("p");
    assert!(editor.move_prim_at_path(&src, &dst));
    assert!(editor.apply_edits());

    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/A").expect("p"))
            .is_none()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Moved").expect("p"))
            .is_some()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Moved/B").expect("p"))
            .is_some()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Moved/B/C").expect("p"))
            .is_some()
    );
}

// ============================================================================
// Invalid prim object operations
// ============================================================================

#[test]
fn delete_invalid_prim_rejected() {
    // C++ ref: test_BasicCanEditPrim — invalid Usd.Prim()
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    // Get a prim at nonexistent path => None, so construct a situation
    // where the prim is invalid. We can get this by looking up a bad path.
    let bad_prim = stage.get_prim_at_path(&Path::from_string("/Nonexistent").expect("p"));
    assert!(bad_prim.is_none());
    // We can't call delete_prim without a Prim object, but delete_prim_at_path works
    // and then can_apply_edits catches it.
    let path = Path::from_string("/Nonexistent").expect("p");
    assert!(editor.delete_prim_at_path(&path));
    assert!(!editor.can_apply_edits().success);
}

#[test]
fn rename_invalid_prim_rejected() {
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    // Create a default invalid prim
    let invalid_prim = usd_core::Prim::invalid();
    assert!(!editor.rename_prim(&invalid_prim, &Token::new("Foo")));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn reparent_invalid_prim_rejected() {
    let stage = setup_stage();
    let mut editor = NamespaceEditor::new(stage.clone());

    let valid = stage
        .get_prim_at_path(&Path::from_string("/A").expect("p"))
        .expect("prim");
    let invalid_prim = usd_core::Prim::invalid();

    // Invalid prim as source
    assert!(!editor.reparent_prim(&invalid_prim, &valid));
    // Invalid prim as target parent
    assert!(!editor.reparent_prim(&valid, &invalid_prim));
    assert!(editor.pending_edits().is_empty());
}
