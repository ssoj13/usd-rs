//! SdfChildrenUtils - utilities for working with spec children.
//!
//! Port of pxr/usd/sdf/childrenUtils.h
//!
//! Provides utility functions for managing children specs.

use crate::{Layer, Path, PrimSpec, PropertySpec, SpecType};
use usd_tf::Token;
use std::sync::Arc;

/// Checks if a path has the given child.
pub fn has_child(layer: &Arc<Layer>, parent_path: &Path, child_name: &str) -> bool {
    if let Some(child_path) = parent_path.append_child(child_name) {
        layer.has_spec(&child_path)
    } else {
        false
    }
}

/// Gets a child by name.
pub fn get_child_prim(layer: &Arc<Layer>, parent_path: &Path, child_name: &str) -> Option<PrimSpec> {
    let child_path = parent_path.append_child(child_name)?;
    layer.get_prim_at_path(&child_path)
}

/// Gets a property child by name.
pub fn get_child_property(
    layer: &Arc<Layer>,
    parent_path: &Path,
    property_name: &str,
) -> Option<PropertySpec> {
    let prop_path = parent_path.append_property(property_name)?;
    layer.get_property_at_path(&prop_path)
}

/// Gets all prim children names from the "primChildren" field.
pub fn get_prim_children_names(layer: &Arc<Layer>, parent_path: &Path) -> Vec<Token> {
    let field_name = Token::new("primChildren");
    layer
        .get_field_as_token_vector(parent_path, &field_name)
        .unwrap_or_default()
}

/// Gets all property children names from the "properties" field.
pub fn get_property_children_names(layer: &Arc<Layer>, parent_path: &Path) -> Vec<Token> {
    let field_name = Token::new("properties");
    layer
        .get_field_as_token_vector(parent_path, &field_name)
        .unwrap_or_default()
}

/// Inserts a child at a specific position.
///
/// C++ childrenUtils.cpp:299-356: reads current childNames from parent,
/// inserts the new child key at the specified index, creates the child spec,
/// and writes back the updated childNames to the parent field.
pub fn insert_child(
    layer: &Arc<Layer>,
    parent_path: &Path,
    child_name: &str,
    index: Option<usize>,
    spec_type: SpecType,
) -> Option<Path> {
    let child_path = parent_path.append_child(child_name)?;

    if layer.has_spec(&child_path) {
        return None; // Already exists
    }

    // Create the spec
    match spec_type {
        SpecType::Prim => {
            layer.create_prim_spec(&child_path, crate::Specifier::Over, "");
        }
        _ => {
            return None;
        }
    }

    // C++ childrenUtils.cpp:354-356: update parent's primChildren field.
    // Read current children list, insert at the specified position, write back.
    let children_key = usd_tf::Token::new("primChildren");
    let mut child_names: Vec<usd_tf::Token> = layer
        .get_field(parent_path, &children_key)
        .and_then(|v| v.get::<Vec<usd_tf::Token>>().cloned())
        .unwrap_or_default();

    let child_token = usd_tf::Token::new(child_name);
    let insert_pos = match index {
        Some(idx) if idx <= child_names.len() => idx,
        _ => child_names.len(), // C++: index == -1 means append
    };
    child_names.insert(insert_pos, child_token);

    layer.set_field(
        parent_path,
        &children_key,
        usd_vt::Value::new(child_names),
    );

    Some(child_path)
}

/// Removes a child by name.
///
/// Matches C++ `Sdf_ChildrenUtils<ChildPolicy>::RemoveChild()`.
pub fn remove_child(layer: &Arc<Layer>, parent_path: &Path, child_name: &str) -> bool {
    if let Some(child_path) = parent_path.append_child(child_name) {
        if layer.has_spec(&child_path) {
            return layer.delete_spec(&child_path);
        }
    }
    false
}

/// Moves a child to a new parent.
pub fn move_child(
    layer: &Arc<Layer>,
    old_parent: &Path,
    new_parent: &Path,
    child_name: &str,
) -> bool {
    if let (Some(old_path), Some(new_path)) = (
        old_parent.append_child(child_name),
        new_parent.append_child(child_name),
    ) {
        layer.move_spec(&old_path, &new_path)
    } else {
        false
    }
}

/// Renames a child.
pub fn rename_child(
    layer: &Arc<Layer>,
    parent_path: &Path,
    old_name: &str,
    new_name: &str,
) -> bool {
    if let (Some(old_path), Some(new_path)) = (
        parent_path.append_child(old_name),
        parent_path.append_child(new_name),
    ) {
        layer.move_spec(&old_path, &new_path)
    } else {
        false
    }
}

/// Reorders children according to the given ordering.
///
/// C++ uses `SdfLayer::SetField(path, childrenKey, reorderedNames)` to
/// write back the reordered children list. Children not mentioned in
/// `new_order` are appended at the end in their original relative order.
pub fn reorder_children(
    layer: &Arc<Layer>,
    parent_path: &Path,
    new_order: &[&str],
) -> bool {
    let children_key = usd_tf::Token::new("primChildren");
    let current: Vec<usd_tf::Token> = layer
        .get_field(parent_path, &children_key)
        .and_then(|v| v.get::<Vec<usd_tf::Token>>().cloned())
        .unwrap_or_default();

    if current.is_empty() {
        return true;
    }

    // Build reordered list: first items from new_order that exist in current,
    // then remaining items from current not in new_order (preserving order).
    let mut reordered: Vec<usd_tf::Token> = Vec::with_capacity(current.len());
    let mut used = vec![false; current.len()];

    for &name in new_order {
        if let Some(pos) = current.iter().position(|t| t.as_str() == name) {
            if !used[pos] {
                reordered.push(current[pos].clone());
                used[pos] = true;
            }
        }
    }
    for (i, token) in current.iter().enumerate() {
        if !used[i] {
            reordered.push(token.clone());
        }
    }

    layer.set_field(parent_path, &children_key, usd_vt::Value::new(reordered));
    true
}

/// Gets the index of a child in its parent's children list.
pub fn get_child_index(
    layer: &Arc<Layer>,
    parent_path: &Path,
    child_name: &str,
) -> Option<usize> {
    let children = get_prim_children_names(layer, parent_path);
    children.iter().position(|n| n == child_name)
}

/// Counts the number of prim children.
pub fn count_prim_children(layer: &Arc<Layer>, parent_path: &Path) -> usize {
    get_prim_children_names(layer, parent_path).len()
}

/// Counts the number of property children.
pub fn count_property_children(layer: &Arc<Layer>, parent_path: &Path) -> usize {
    get_property_children_names(layer, parent_path).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_child() {
        let layer = Layer::create_anonymous(Some("test"));
        let root = Path::absolute_root();
        
        assert!(!has_child(&layer, &root, "Child"));
    }

    #[test]
    fn test_move_child() {
        let layer = Layer::create_anonymous(Some("test"));
        let parent1 = Path::from_string("/Parent1").unwrap();
        let parent2 = Path::from_string("/Parent2").unwrap();
        
        // Would need actual specs to test
        let _ = move_child(&layer, &parent1, &parent2, "Child");
    }
}
