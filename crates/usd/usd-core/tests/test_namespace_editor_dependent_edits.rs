//! Port of testUsdNamespaceEditorDependentEdits*.py from OpenUSD
//! 7 massive test files covering dependent edit scenarios:
//! - Base (11KB) — shared utilities
//! - BasicClassArcs (151KB)
//! - BasicReferencesAndPayloads (92KB)
//! - BasicRelocates (91KB)
//! - BasicSublayers (40KB)
//! - BasicVariants (40KB)
//! - Properties (77KB)
//! - SiblingNodeSpecConflicts (85KB)

mod common;

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (151KB)"]
fn namespace_editor_dependent_edits_class_arcs() {
    common::setup();
}

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (92KB)"]
fn namespace_editor_dependent_edits_references_payloads() {
    common::setup();
}

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (91KB)"]
fn namespace_editor_dependent_edits_relocates() {
    common::setup();
}

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (40KB)"]
fn namespace_editor_dependent_edits_sublayers() {
    common::setup();
}

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (40KB)"]
fn namespace_editor_dependent_edits_variants() {
    common::setup();
}

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (77KB)"]
fn namespace_editor_dependent_edits_properties() {
    common::setup();
}

#[test]
#[ignore = "Needs full NamespaceEditor dependent edits infrastructure (85KB)"]
fn namespace_editor_dependent_edits_sibling_conflicts() {
    common::setup();
}
