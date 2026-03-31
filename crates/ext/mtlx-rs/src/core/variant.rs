//! VariantSet, Variant, VariantAssign -- material variants.

use crate::core::element::{
    ElementPtr, VARIANT_ATTRIBUTE, VARIANT_SET_ATTRIBUTE, add_child_of_category, category,
};

// --- VariantSet CRUD ---

/// Add a Variant to a VariantSet
pub fn add_variant(variant_set: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(variant_set, category::VARIANT, name)
}

/// Get a Variant by name from a VariantSet
pub fn get_variant(variant_set: &ElementPtr, name: &str) -> Option<ElementPtr> {
    variant_set
        .borrow()
        .get_child_of_category(name, category::VARIANT)
}

/// Get all Variant children of a VariantSet
pub fn get_variants(variant_set: &ElementPtr) -> Vec<ElementPtr> {
    variant_set
        .borrow()
        .get_children_of_category(category::VARIANT)
}

/// Get variant names from a VariantSet element
pub fn get_variant_names(variant_set: &ElementPtr) -> Vec<String> {
    variant_set
        .borrow()
        .get_children()
        .iter()
        .filter(|c| c.borrow().get_category() == category::VARIANT)
        .map(|c| c.borrow().get_name().to_string())
        .collect()
}

/// Remove a Variant by name from a VariantSet
pub fn remove_variant(variant_set: &ElementPtr, name: &str) {
    variant_set
        .borrow_mut()
        .remove_child_of_category(name, category::VARIANT);
}

// --- VariantAssign helpers ---

/// Set variant set string on a VariantAssign element
pub fn set_variant_set_string(assign: &ElementPtr, variant_set: impl Into<String>) {
    assign
        .borrow_mut()
        .set_attribute(VARIANT_SET_ATTRIBUTE, variant_set.into());
}

/// Has variant set string
pub fn has_variant_set_string(assign: &ElementPtr) -> bool {
    assign.borrow().has_attribute(VARIANT_SET_ATTRIBUTE)
}

/// Get variant set string
pub fn get_variant_set_string(assign: &ElementPtr) -> String {
    assign
        .borrow()
        .get_attribute_or_empty(VARIANT_SET_ATTRIBUTE)
}

/// Set variant string on a VariantAssign element
pub fn set_variant_string(assign: &ElementPtr, variant: impl Into<String>) {
    assign
        .borrow_mut()
        .set_attribute(VARIANT_ATTRIBUTE, variant.into());
}

/// Has variant string
pub fn has_variant_string(assign: &ElementPtr) -> bool {
    assign.borrow().has_attribute(VARIANT_ATTRIBUTE)
}

/// Get variant string
pub fn get_variant_string(assign: &ElementPtr) -> String {
    assign.borrow().get_attribute_or_empty(VARIANT_ATTRIBUTE)
}

// --- VariantAssign CRUD on Look ---

/// Add a VariantAssign child to a Look
pub fn add_variant_assign(look: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(look, category::VARIANT_ASSIGN, name)
}

/// Get VariantAssign by name from a Look
pub fn get_variant_assign(look: &ElementPtr, name: &str) -> Option<ElementPtr> {
    look.borrow()
        .get_child_of_category(name, category::VARIANT_ASSIGN)
}

/// Get all VariantAssign children of a Look
pub fn get_variant_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    look.borrow()
        .get_children_of_category(category::VARIANT_ASSIGN)
}

/// Remove a VariantAssign by name from a Look
pub fn remove_variant_assign(look: &ElementPtr, name: &str) {
    look.borrow_mut()
        .remove_child_of_category(name, category::VARIANT_ASSIGN);
}
