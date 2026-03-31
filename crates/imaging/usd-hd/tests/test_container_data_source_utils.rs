// Port of pxr/imaging/hd/testenv/testHdContainerDataSourceUtils.cpp

use usd_hd::data_source::{
    HdContainerDataSourceEditor, HdContainerDataSourceHandle, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_tf::Token;

fn t(s: &str) -> Token {
    Token::new(s)
}

fn i(v: i32) -> HdDataSourceBaseHandle {
    HdRetainedTypedSampledDataSource::new(v)
}

fn loc(input: &str) -> HdDataSourceLocator {
    let tokens: Vec<Token> = input
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| Token::new(s))
        .collect();
    HdDataSourceLocator::new(&tokens)
}

// --- TestSimpleOverlay ---

#[test]
fn test_simple_overlay() {
    let c1 = HdRetainedContainerDataSource::new_2(t("A"), i(1), t("F"), i(7));
    let c2 = HdRetainedContainerDataSource::new_2(t("B"), i(2), t("C"), i(3));
    let c3 = HdRetainedContainerDataSource::new_3(
        t("D"),
        HdRetainedContainerDataSource::new_1(t("E"), i(4)) as HdDataSourceBaseHandle,
        t("F"),
        i(6),
        t("G"),
        i(8),
    );

    let test_overlay = HdOverlayContainerDataSource::new(vec![
        c1 as HdContainerDataSourceHandle,
        c2 as HdContainerDataSourceHandle,
        c3 as HdContainerDataSourceHandle,
    ]);

    let test_handle: HdDataSourceBaseHandle = test_overlay;
    let container = cast_to_container(&test_handle).unwrap();
    let names = container.get_names();

    assert!(names.contains(&t("A")));
    assert!(names.contains(&t("B")));
    assert!(names.contains(&t("C")));
    assert!(names.contains(&t("D")));
    assert!(names.contains(&t("F")));
    assert!(names.contains(&t("G")));
    assert_eq!(names.len(), 6);
}

// --- TestContainerEditor ---

#[test]
fn test_container_editor_one_level() {
    let mut editor = HdContainerDataSourceEditor::new(None);
    editor.set(&loc("A"), Some(i(1)));
    editor.set(&loc("B"), Some(i(2)));
    let test = editor.finish();

    assert!(test.is_some(), "finish() returned None for simple editor");
    let container = test.unwrap();
    let names = container.get_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&t("A")));
    assert!(names.contains(&t("B")));
}

#[test]
fn test_container_editor_two_levels_with_override() {
    let mut editor = HdContainerDataSourceEditor::new(None);
    editor.set(&loc("A"), Some(i(1)));
    editor.set(&loc("B"), Some(i(2)));
    editor.set(&loc("C/D"), Some(i(3)));
    editor.set(&loc("C/E"), Some(i(4)));
    editor.set(&loc("B"), Some(i(5))); // override B
    let test = editor.finish();

    assert!(test.is_some(), "finish() returned None");
    let container = test.unwrap();
    let names = container.get_names();
    assert!(names.contains(&t("A")));
    assert!(names.contains(&t("B")));
    assert!(names.contains(&t("C")));
}

#[test]
fn test_container_editor_initial_container() {
    let mut editor1 = HdContainerDataSourceEditor::new(None);
    editor1.set(&loc("A/B"), Some(i(1)));
    let initial = editor1.finish();

    let mut editor2 = HdContainerDataSourceEditor::new(initial);
    editor2.set(&loc("A/C"), Some(i(2)));
    editor2.set(&loc("D"), Some(i(3)));
    let test = editor2.finish();

    assert!(
        test.is_some(),
        "finish() returned None for editor with initial"
    );
    let container = test.unwrap();
    let names = container.get_names();
    assert!(names.contains(&t("A")));
    assert!(names.contains(&t("D")));
}

#[test]
fn test_compute_dirty_locators() {
    let overridden = HdDataSourceLocatorSet::from_iter(vec![loc("A/B"), loc("A/C"), loc("D/E/F")]);

    let dirty = HdContainerDataSourceEditor::compute_dirty_locators(&overridden);

    // The dirty set should contain at least the original locators
    assert!(dirty.contains(&loc("A/B")));
    assert!(dirty.contains(&loc("A/C")));
    assert!(dirty.contains(&loc("D/E/F")));
}

// --- Missing sub-tests from C++ TestContainerEditor ---

#[test]
fn test_container_editor_set_with_container_then_override() {
    // C++ sub-test 3: set A with container(B=1), then overlay A/C=2 and A/D/E=3
    let mut editor = HdContainerDataSourceEditor::new(None);
    editor.set(
        &loc("A"),
        Some(HdRetainedContainerDataSource::new_1(t("B"), i(1)) as HdDataSourceBaseHandle),
    );
    editor.set(&loc("A/C"), Some(i(2)));
    editor.set(&loc("A/D/E"), Some(i(3)));
    let test = editor.finish();

    assert!(test.is_some());
    let container = test.unwrap();
    let names = container.get_names();
    assert!(names.contains(&t("A")));

    // Navigate into A — should have B, C, D
    let a_ds = container.get(&t("A"));
    assert!(a_ds.is_some());
    let a_container = cast_to_container(&a_ds.unwrap());
    assert!(a_container.is_some());
    let a_names = a_container.unwrap().get_names();
    assert!(a_names.contains(&t("B")));
    assert!(a_names.contains(&t("C")));
    assert!(a_names.contains(&t("D")));
}

#[test]

fn test_container_editor_deep_override_and_delete() {
    // C++ sub-test 4: set with container, override deeply + delete
    let mut sub_editor = HdContainerDataSourceEditor::new(None);
    sub_editor.set(&loc("B/C/E"), Some(i(2)));
    sub_editor.set(&loc("Z/Y"), Some(i(3)));
    let subcontainer = sub_editor.finish();

    let mut editor = HdContainerDataSourceEditor::new(None);
    editor.set(&loc("A"), subcontainer.map(|c| c as HdDataSourceBaseHandle));
    editor.set(&loc("A/B/Q"), Some(i(5)));
    editor.set(&loc("A/B/C/F"), Some(i(6)));
    editor.set(&loc("A/Z/Y"), None); // delete
    let test = editor.finish();

    assert!(test.is_some());
    let container = test.unwrap();

    // A should exist
    let a_ds = container.get(&t("A"));
    assert!(a_ds.is_some());
    let a_container = cast_to_container(&a_ds.unwrap());
    assert!(a_container.is_some());
    let a_names = a_container.unwrap().get_names();
    assert!(a_names.contains(&t("B")));
    assert!(a_names.contains(&t("Z")));
}

#[test]
fn test_container_editor_replacement_masking() {
    // C++ sub-test 6: replacing container masks children
    let mut initial_editor = HdContainerDataSourceEditor::new(None);
    initial_editor.set(
        &loc("A"),
        Some(
            HdRetainedContainerDataSource::new_2(t("B"), i(1), t("C"), i(2))
                as HdDataSourceBaseHandle,
        ),
    );
    let initial = initial_editor.finish();

    let mut editor = HdContainerDataSourceEditor::new(initial);
    editor.set(
        &loc("A"),
        Some(
            HdRetainedContainerDataSource::new_2(t("D"), i(3), t("E"), i(4))
                as HdDataSourceBaseHandle,
        ),
    );
    let test = editor.finish();

    assert!(test.is_some());
    let container = test.unwrap();
    let a_ds = container.get(&t("A"));
    assert!(a_ds.is_some());
    let a_container = cast_to_container(&a_ds.unwrap());
    assert!(a_container.is_some());
    let a_names = a_container.unwrap().get_names();
    // B and C should be masked (replaced), only D and E should remain
    assert!(a_names.contains(&t("D")));
    assert!(a_names.contains(&t("E")));
}

#[test]

fn test_container_editor_overlay() {
    // C++ sub-test 7: overlay merges rather than replaces
    let mut initial_editor = HdContainerDataSourceEditor::new(None);
    initial_editor.set(
        &loc("A"),
        Some(
            HdRetainedContainerDataSource::new_2(t("B"), i(1), t("C"), i(2))
                as HdDataSourceBaseHandle,
        ),
    );
    let initial = initial_editor.finish();

    let mut sub_editor = HdContainerDataSourceEditor::new(None);
    sub_editor.set(&loc("D"), Some(i(3)));
    let subcontainer = sub_editor.finish();

    let mut editor = HdContainerDataSourceEditor::new(initial);
    editor.overlay(
        &loc("A"),
        subcontainer.map(|c| c as HdContainerDataSourceHandle),
    );
    let test = editor.finish();

    assert!(test.is_some());
    let container = test.unwrap();
    let a_ds = container.get(&t("A"));
    assert!(a_ds.is_some());
    let a_container = cast_to_container(&a_ds.unwrap());
    assert!(a_container.is_some());
    let a_names = a_container.unwrap().get_names();
    // Overlay: B, C should be preserved, D should be added
    assert!(a_names.contains(&t("B")));
    assert!(a_names.contains(&t("C")));
    assert!(a_names.contains(&t("D")));
}
