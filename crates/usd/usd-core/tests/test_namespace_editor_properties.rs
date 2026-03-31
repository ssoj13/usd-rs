// Port of testUsdNamespaceEditorProperties.py — property operations subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdNamespaceEditorProperties.py

mod common;

use usd_core::namespace_editor::NamespaceEditor;
use usd_core::{InitialLoadSet, Property, Stage};
use usd_sdf::Path;
use usd_tf::Token;

fn setup_stage_with_properties() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim_a = stage.define_prim("/A", "Xform").expect("define /A");
    let prim_b = stage.define_prim("/B", "Xform").expect("define /B");

    // Create some attributes on /A
    let float_type = common::vtn("float");
    let int_type = common::vtn("int");
    prim_a.create_attribute("myFloat", &float_type, false, None);
    prim_a.create_attribute("myInt", &int_type, false, None);

    // Create an attribute on /B
    prim_b.create_attribute("bAttr", &float_type, false, None);

    stage
}

fn get_property(stage: &Stage, path_str: &str) -> Property {
    let path = Path::from_string(path_str).expect("valid path");
    let prim_path = path.get_parent_path();
    let prop_name = path.get_name();
    let prim = stage.get_prim_at_path(&prim_path).expect("prim exists");
    let props = prim.get_properties_in_namespace(&Token::new(""));
    props
        .into_iter()
        .find(|p| p.name().as_str() == prop_name)
        .expect(&format!("property {} exists", path_str))
}

// ============================================================================
// Delete property
// ============================================================================

#[test]
fn delete_property_at_path() {
    // C++ ref: testUsdNamespaceEditorProperties — basic delete
    let stage = setup_stage_with_properties();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path = Path::from_string("/A.myFloat").expect("path");
    assert!(editor.delete_property_at_path(&path));
    assert_eq!(editor.pending_edits().len(), 1);

    let result = editor.can_apply_edits();
    assert!(result.success, "can_apply: {:?}", result.error_message);
}

#[test]
fn delete_property() {
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");
    assert!(prop.is_valid());

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.delete_property(&prop));
    assert_eq!(editor.pending_edits().len(), 1);
}

#[test]
fn delete_property_prim_path_rejected() {
    // Prim paths should be rejected for property operations
    let stage = setup_stage_with_properties();
    let mut editor = NamespaceEditor::new(stage.clone());

    let prim_path = Path::from_string("/A").expect("path");
    assert!(!editor.delete_property_at_path(&prim_path));
    assert!(editor.pending_edits().is_empty());
}

// ============================================================================
// Move property
// ============================================================================

#[test]
fn move_property_at_path() {
    let stage = setup_stage_with_properties();
    let mut editor = NamespaceEditor::new(stage.clone());

    let src = Path::from_string("/A.myFloat").expect("p");
    let dst = Path::from_string("/A.renamedFloat").expect("p");
    assert!(editor.move_property_at_path(&src, &dst));
    assert_eq!(editor.pending_edits().len(), 1);
}

#[test]
fn move_property_prim_path_rejected() {
    let stage = setup_stage_with_properties();
    let mut editor = NamespaceEditor::new(stage.clone());

    let prop = Path::from_string("/A.myFloat").expect("p");
    let prim = Path::from_string("/B").expect("p");
    // Prim path as destination
    assert!(!editor.move_property_at_path(&prop, &prim));
    // Prim path as source
    assert!(!editor.move_property_at_path(&prim, &prop));
    assert!(editor.pending_edits().is_empty());
}

// ============================================================================
// Rename property
// ============================================================================

#[test]
fn rename_property_at_path() {
    let stage = setup_stage_with_properties();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path = Path::from_string("/A.myFloat").expect("p");
    assert!(editor.rename_property_at_path(&path, &Token::new("newFloat")));
    assert_eq!(editor.pending_edits().len(), 1);
}

#[test]
fn rename_property_at_path_empty_name_rejected() {
    let stage = setup_stage_with_properties();
    let mut editor = NamespaceEditor::new(stage.clone());

    let path = Path::from_string("/A.myFloat").expect("p");
    assert!(!editor.rename_property_at_path(&path, &Token::new("")));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn rename_property() {
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.rename_property(&prop, &Token::new("newFloat")));
    assert_eq!(editor.pending_edits().len(), 1);
}

#[test]
fn rename_property_empty_name_rejected() {
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(!editor.rename_property(&prop, &Token::new("")));
    assert!(editor.pending_edits().is_empty());
}

// ============================================================================
// Reparent property
// ============================================================================

#[test]
fn reparent_property() {
    // Move property from /A to /B
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");
    let prim_b = stage
        .get_prim_at_path(&Path::from_string("/B").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.reparent_property(&prop, &prim_b));
    assert_eq!(editor.pending_edits().len(), 1);
}

#[test]
fn reparent_property_with_name() {
    // Move property from /A.myFloat to /B.movedFloat
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");
    let prim_b = stage
        .get_prim_at_path(&Path::from_string("/B").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(editor.reparent_property_with_name(&prop, &prim_b, &Token::new("movedFloat")));
    assert_eq!(editor.pending_edits().len(), 1);
}

#[test]
fn reparent_property_with_empty_name_rejected() {
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");
    let prim_b = stage
        .get_prim_at_path(&Path::from_string("/B").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(!editor.reparent_property_with_name(&prop, &prim_b, &Token::new("")));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn reparent_property_invalid_property_rejected() {
    let stage = setup_stage_with_properties();
    let invalid_prop = Property::invalid();
    let prim_b = stage
        .get_prim_at_path(&Path::from_string("/B").expect("p"))
        .expect("prim");

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(!editor.reparent_property(&invalid_prop, &prim_b));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn reparent_property_invalid_parent_rejected() {
    let stage = setup_stage_with_properties();
    let prop = get_property(&stage, "/A.myFloat");
    let invalid_prim = usd_core::Prim::invalid();

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(!editor.reparent_property(&prop, &invalid_prim));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn delete_invalid_property_rejected() {
    let stage = setup_stage_with_properties();
    let invalid_prop = Property::invalid();

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(!editor.delete_property(&invalid_prop));
    assert!(editor.pending_edits().is_empty());
}

#[test]
fn rename_invalid_property_rejected() {
    let stage = setup_stage_with_properties();
    let invalid_prop = Property::invalid();

    let mut editor = NamespaceEditor::new(stage.clone());
    assert!(!editor.rename_property(&invalid_prop, &Token::new("foo")));
    assert!(editor.pending_edits().is_empty());
}
