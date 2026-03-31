//! PCP Dynamic File Format Support.
//!
//! Context and interface for dynamic file format argument generation.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/dynamicFileFormatContext.h`,
//! `dynamicFileFormatInterface.h`, and their implementations.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::{NodeRef, compare_sibling_payload_node_strength};
use usd_sdf::{FileFormatArguments, Path};
use usd_tf::Token;
use usd_vt::Value;

use super::prim_index_stack_frame::{PrimIndexStackFrame, PrimIndexStackFrameIterator};
use super::utils::translate_path_from_node_to_root_or_closest;

/// Vector of values for compose operations.
pub type VtValueVector = Vec<Value>;

/// Context for composing field values when generating dynamic file format arguments.
///
/// The context allows implementations to iterate over all nodes that have already
/// been composed, looking for the strongest opinion for a relevant field.
///
/// Matches C++ `PcpDynamicFileFormatContext`.
#[derive(Debug)]
pub struct DynamicFileFormatContext {
    /// Parent node for traversal.
    parent_node: NodeRef,
    /// Path in the node's namespace.
    path_in_node: Path,
    /// Arc number.
    arc_num: i32,
    /// Previous stack frame (for recursive prim indexing).
    previous_stack_frame: Option<Box<PrimIndexStackFrame>>,
    /// Composed field names (for dependency tracking).
    composed_field_names: Option<Box<HashSet<Token>>>,
    /// Composed attribute names (for dependency tracking).
    composed_attribute_names: Option<Box<HashSet<Token>>>,
}

impl DynamicFileFormatContext {
    /// Creates a new dynamic file format context.
    ///
    /// Should only be called by prim indexing.
    ///
    /// Matches C++ private constructor and `Pcp_CreateDynamicFileFormatContext`.
    pub(crate) fn new(
        parent_node: NodeRef,
        path_in_node: Path,
        arc_num: i32,
        previous_stack_frame: Option<Box<PrimIndexStackFrame>>,
        composed_field_names: Option<Box<HashSet<Token>>>,
        composed_attribute_names: Option<Box<HashSet<Token>>>,
    ) -> Self {
        Self {
            parent_node,
            path_in_node,
            arc_num,
            previous_stack_frame,
            composed_field_names,
            composed_attribute_names,
        }
    }

    /// Composes the value of the given field and returns its strongest opinion.
    ///
    /// For dictionary valued fields, this returns a dictionary containing the
    /// strongest value for each individual key.
    ///
    /// Returns `Some(value)` if a value was found, `None` otherwise.
    ///
    /// # Implementation Notes
    ///
    /// This traverses the prim index graph and composes field values from
    /// all contributing nodes. Currently returns None because the composition
    /// requires access to the full prim index and layer infrastructure which
    /// is passed through the indexing stack frame in C++.
    /// Composes the value of the given field and returns its strongest opinion.
    ///
    /// For dictionary valued fields, this returns a dictionary containing the
    /// strongest value for each individual key.
    ///
    /// Returns `true` if a value was found, `false` otherwise.
    ///
    /// Matches C++ `ComposeValue(const TfToken &field, VtValue *value)`.
    pub fn compose_value(&mut self, field: &Token, value: &mut Value) -> bool {
        // Update the cached field names for dependency tracking.
        if let Some(ref mut field_names) = self.composed_field_names {
            field_names.insert(field.clone());
        }

        // Check if field is allowed for argument generation
        let mut field_is_dict_valued = false;
        if !self.is_allowed_field_for_arguments(field, Some(&mut field_is_dict_valued)) {
            return false;
        }

        // If the field is a dictionary, compose the dictionary's key values from
        // strongest to weakest opinions.
        if field_is_dict_valued {
            let mut composed_dict = HashMap::<String, Value>::new();
            let found = compose_field_value_helper(
                self,
                field,
                false, // findStrongestOnly = false for dictionaries
                |val| {
                    if let Some(dict) = val.as_dictionary() {
                        // Merge dictionaries recursively (stronger overrides weaker)
                        for (key, val) in dict {
                            composed_dict.insert(key, val);
                        }
                    }
                },
            );
            if found {
                *value = Value::from_dictionary(composed_dict);
                return true;
            }
            false
        } else {
            // For all other value types, compose by just grabbing the strongest opinion.
            let mut found_value = None;
            let found = compose_field_value_helper(
                self,
                field,
                true, // findStrongestOnly = true
                |val| {
                    found_value = Some(val);
                },
            );
            if found {
                if let Some(found_val) = found_value {
                    *value = found_val;
                    return true;
                }
            }
            false
        }
    }

    /// Composes all values of the given field, ordered from strongest to weakest.
    ///
    /// For dictionary valued fields, the dictionaries from each opinion are not
    /// composed together and are returned as-is in the list.
    ///
    /// Returns `Some(values)` if any values were found, `None` otherwise.
    ///
    /// Note: This is slower than `compose_value`, especially for non-dictionary
    /// valued fields.
    /// Composes all values of the given field, ordered from strongest to weakest.
    ///
    /// For dictionary valued fields, the dictionaries from each opinion are not
    /// composed together and are returned as-is in the list.
    ///
    /// Returns `true` if any values were found, `false` otherwise.
    ///
    /// Matches C++ `ComposeValueStack(const TfToken &field, VtValueVector *values)`.
    pub fn compose_value_stack(&mut self, field: &Token, values: &mut VtValueVector) -> bool {
        // Update the cached field names for dependency tracking.
        if let Some(ref mut field_names) = self.composed_field_names {
            field_names.insert(field.clone());
        }

        // Check if field is allowed
        if !self.is_allowed_field_for_arguments(field, None) {
            return false;
        }

        // For the value stack, just add all opinions we can find for the field
        // in strength order.
        compose_field_value_helper(
            self,
            field,
            false, // findStrongestOnly = false
            |val| {
                values.push(val);
            },
        )
    }

    /// Composes the default value of the attribute with the given name.
    ///
    /// Returns `true` if a value was found, `false` otherwise.
    ///
    /// Matches C++ `ComposeAttributeDefaultValue(const TfToken &attributeName, VtValue *value)`.
    pub fn compose_attribute_default_value(
        &mut self,
        attribute_name: &Token,
        value: &mut Value,
    ) -> bool {
        // Update the cached attribute names for dependency tracking.
        if let Some(ref mut attr_names) = self.composed_attribute_names {
            attr_names.insert(attribute_name.clone());
        }

        // Unlike metadata fields, attributes cannot have dictionary values which
        // simplifies this function compared to compose_value. We compose by just
        // grabbing the strongest default value for the attribute if one exists.
        let mut found_value = None;
        let found = compose_attribute_default_value_helper(self, attribute_name, |val| {
            found_value = Some(val);
        });
        if found {
            if let Some(found_val) = found_value {
                *value = found_val;
                return true;
            }
        }
        false
    }

    /// Returns the set of field names that were composed.
    pub fn composed_field_names(&self) -> Option<&HashSet<Token>> {
        self.composed_field_names.as_deref()
    }

    /// Returns the set of attribute names that were composed.
    pub fn composed_attribute_names(&self) -> Option<&HashSet<Token>> {
        self.composed_attribute_names.as_deref()
    }

    /// Checks if the given field is allowed for generating file format arguments.
    ///
    /// Matches C++ `_IsAllowedFieldForArguments`.
    fn is_allowed_field_for_arguments(
        &self,
        field: &Token,
        field_value_is_dictionary: Option<&mut bool>,
    ) -> bool {
        // We're starting off by restricting the allowed fields to be only fields
        // defined by plugins. We may ease this in the future to allow certain
        // builtin fields as well but there will need to be some updates to
        // change management to handle these correctly.
        //
        // In C++, this checks the schema for plugin fields. For now, we allow
        // common plugin fields like customData and assetInfo.
        let allowed_fields = ["customData", "assetInfo"];

        let is_allowed = allowed_fields.iter().any(|f| field == *f);

        if !is_allowed {
            // In C++, this would call TF_CODING_ERROR. For now, we just return false.
            return false;
        }

        // Check if field value is a dictionary
        if let Some(is_dict) = field_value_is_dictionary {
            *is_dict = field == "customData" || field == "assetInfo";
        }

        true
    }
}

// ============================================================================
// Helper functions for composing values
// ============================================================================

/// Helper function to compose field values from ancestors.
///
/// Matches C++ `_ComposeValueHelper::ComposeFieldValue`.
fn compose_field_value_helper<F>(
    context: &DynamicFileFormatContext,
    field_name: &Token,
    strongest_opinion_only: bool,
    mut compose_func: F,
) -> bool
where
    F: FnMut(Value),
{
    let mut iterator = PrimIndexStackFrameIterator::new(
        context.parent_node.clone(),
        context
            .previous_stack_frame
            .as_ref()
            .map(|f| Box::new((**f).clone())),
    );
    let parent = context.parent_node.clone();
    let path_in_node = if context.path_in_node.is_empty() {
        context.parent_node.path()
    } else {
        context.path_in_node.clone()
    };
    let arc_num = context.arc_num;

    compose_opinion_from_ancestors(
        &mut iterator,
        &parent,
        &path_in_node,
        arc_num,
        &Token::empty(),
        field_name,
        strongest_opinion_only,
        &mut compose_func,
    )
}

/// Helper function to compose attribute default values from ancestors.
///
/// Matches C++ `_ComposeValueHelper::ComposeAttributeDefaultValue`.
fn compose_attribute_default_value_helper<F>(
    context: &DynamicFileFormatContext,
    attribute_name: &Token,
    mut compose_func: F,
) -> bool
where
    F: FnMut(Value),
{
    // Use "default" token for attribute default value field
    let default_field = Token::new("default");
    let mut iterator = PrimIndexStackFrameIterator::new(
        context.parent_node.clone(),
        context
            .previous_stack_frame
            .as_ref()
            .map(|f| Box::new((**f).clone())),
    );
    let parent = context.parent_node.clone();
    let path_in_node = if context.path_in_node.is_empty() {
        context.parent_node.path()
    } else {
        context.path_in_node.clone()
    };
    let arc_num = context.arc_num;

    compose_opinion_from_ancestors(
        &mut iterator,
        &parent,
        &path_in_node,
        arc_num,
        attribute_name,
        &default_field,
        true, // strongest only for attributes
        &mut compose_func,
    )
}

/// Composes opinions from ancestors of the parent node and their subtrees in strength order.
///
/// Matches C++ `_ComposeValueHelper::_ComposeOpinionFromAncestors`.
fn compose_opinion_from_ancestors<F>(
    iterator: &mut PrimIndexStackFrameIterator,
    parent: &NodeRef,
    path_in_node: &Path,
    arc_num: i32,
    prop_name: &Token,
    field_name: &Token,
    strongest_opinion_only: bool,
    compose_func: &mut F,
) -> bool
where
    F: FnMut(Value),
{
    // Translate the path from the given node's namespace to the root of the node's prim index.
    let (rootmost_path, rootmost_node) =
        translate_path_from_node_to_root_or_closest(&iterator.node, path_in_node);

    // If we were able to translate the path all the way to the root node, and we're in the
    // middle of a recursive prim indexing call, map across the previous frame and recurse.
    if rootmost_node.is_root_node() {
        if let Some(ref frame) = iterator.previous_frame {
            let parent_node = frame.parent_node.clone();
            let parent_node_path = frame
                .arc_to_parent
                .map_to_parent()
                .map_source_to_target(&rootmost_path.strip_all_variant_selections())
                .unwrap_or_else(Path::empty);

            iterator.next_frame();

            if compose_opinion_from_ancestors(
                iterator,
                &parent_node,
                &parent_node_path,
                arc_num,
                prop_name,
                field_name,
                strongest_opinion_only,
                compose_func,
            ) {
                return true;
            }
        }
    }

    // Compose opinions in the subtree.
    compose_opinion_in_subtree(
        &rootmost_node,
        &rootmost_path,
        prop_name,
        field_name,
        parent,
        arc_num,
        strongest_opinion_only,
        compose_func,
    )
}

/// Composes the values from the node and its subtree.
///
/// Matches C++ `_ComposeValueHelper::_ComposeOpinionInSubtree`.
fn compose_opinion_in_subtree<F>(
    node: &NodeRef,
    path_in_node: &Path,
    prop_name: &Token,
    field_name: &Token,
    parent: &NodeRef,
    arc_num: i32,
    strongest_opinion_only: bool,
    compose_func: &mut F,
) -> bool
where
    F: FnMut(Value),
{
    // Get the prim or property path within the node's spec.
    let path = if prop_name.is_empty() {
        path_in_node.clone()
    } else {
        path_in_node
            .append_property(prop_name.as_str())
            .unwrap_or_else(|| path_in_node.clone())
    };

    // Search the node's layer stack in strength order for the field on the spec.
    if let Some(layer_stack) = node.layer_stack() {
        let layers = layer_stack.get_layers();
        for layer in &layers {
            if let Some(field_value) = layer.get_field(&path, field_name) {
                // Note: Asset path resolution via expression variables deferred.
                // For now, we skip asset path resolution as it requires more infrastructure.

                // Process the value and mark found.
                compose_func(field_value);
                if strongest_opinion_only {
                    return true;
                }
            }
        }
    }

    // Recursively check child nodes.
    for child_node in node.children_range() {
        // If this is the parent, check if each of its children is weaker than the future node.
        if *node == *parent
            && compare_sibling_payload_node_strength(parent, arc_num, &child_node) == -1
        {
            return true;
        }

        // Map the path in this node to the next child node, also applying any variant
        // selections represented by the child node.
        let path_in_child_node = child_node
            .map_to_parent()
            .map_target_to_source(&path_in_node.strip_all_variant_selections())
            .unwrap_or_else(Path::empty);

        if path_in_child_node.is_empty() {
            continue;
        }

        let mut final_path_in_child = path_in_child_node;
        let child_node_path_at_intro = child_node.path_at_introduction();
        if child_node_path_at_intro.contains_prim_variant_selection() {
            let stripped = child_node_path_at_intro.strip_all_variant_selections();
            if let Some(replaced) =
                final_path_in_child.replace_prefix(&stripped, &child_node_path_at_intro)
            {
                final_path_in_child = replaced;
            }
        }

        if compose_opinion_in_subtree(
            &child_node,
            &final_path_in_child,
            prop_name,
            field_name,
            parent,
            arc_num,
            strongest_opinion_only,
            compose_func,
        ) {
            return true;
        }
    }

    false
}

/// Interface for file formats that support dynamic argument generation.
///
/// When prim index composition encounters a payload to an asset of a file format
/// that implements this interface, it calls `compose_fields_for_file_format_arguments`
/// to generate arguments from the current composition context.
pub trait DynamicFileFormatInterface: Send + Sync {
    /// Composes prim metadata fields and/or attribute default values using the
    /// given context and uses them to generate file format arguments for the
    /// layer at `asset_path`.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - The asset path being processed
    /// * `context` - The composition context for composing values
    /// * `args` - The arguments to populate (in/out)
    /// * `dependency_context_data` - Optional output for dependency tracking data
    fn compose_fields_for_file_format_arguments(
        &self,
        asset_path: &str,
        context: &mut DynamicFileFormatContext,
        args: &mut FileFormatArguments,
        dependency_context_data: &mut Option<Value>,
    );

    /// Returns whether a change to the given field can affect file format arguments.
    ///
    /// The default implementation returns `true` for all fields.
    fn can_field_change_affect_file_format_arguments(
        &self,
        _field: &Token,
        _old_value: &Value,
        _new_value: &Value,
        _dependency_context_data: &Value,
    ) -> bool {
        true
    }

    /// Returns whether a change to an attribute's default value can affect
    /// file format arguments.
    ///
    /// The default implementation returns `true` for all attributes.
    fn can_attribute_default_value_change_affect_file_format_arguments(
        &self,
        _attribute_name: &Token,
        _old_value: &Value,
        _new_value: &Value,
        _dependency_context_data: &Value,
    ) -> bool {
        true
    }
}

/// Creates a dynamic file format context for use during prim indexing.
///
/// Matches C++ `Pcp_CreateDynamicFileFormatContext`.
pub fn create_dynamic_file_format_context(
    parent_node: &NodeRef,
    ancestral_path: &Path,
    arc_num: i32,
    previous_frame: Option<Box<PrimIndexStackFrame>>,
    composed_field_names: Option<Box<HashSet<Token>>>,
    composed_attribute_names: Option<Box<HashSet<Token>>>,
) -> DynamicFileFormatContext {
    DynamicFileFormatContext::new(
        parent_node.clone(),
        ancestral_path.clone(),
        arc_num,
        previous_frame,
        composed_field_names,
        composed_attribute_names,
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrimIndex;

    #[test]
    fn test_context_creation() {
        let prim_index = PrimIndex::new();
        let root = prim_index.root_node();
        let path = Path::from_string("/World").unwrap();

        let context = DynamicFileFormatContext::new(root, path, 0, None, None, None);
        assert!(
            context
                .composed_field_names()
                .map(|s| s.is_empty())
                .unwrap_or(true)
        );
        assert!(
            context
                .composed_attribute_names()
                .map(|s| s.is_empty())
                .unwrap_or(true)
        );
    }

    #[test]
    fn test_allowed_fields() {
        let prim_index = PrimIndex::new();
        let root = prim_index.root_node();
        let path = Path::from_string("/World").unwrap();

        let context = DynamicFileFormatContext::new(root, path, 0, None, None, None);

        // customData should be allowed
        let field = Token::from("customData");
        assert!(context.is_allowed_field_for_arguments(&field, None));

        // assetInfo should be allowed
        let field = Token::from("assetInfo");
        assert!(context.is_allowed_field_for_arguments(&field, None));
    }

    #[test]
    fn test_compose_tracks_fields() {
        let prim_index = PrimIndex::new();
        let root = prim_index.root_node();
        let path = Path::from_string("/World").unwrap();

        let mut context = DynamicFileFormatContext::new(
            root,
            path,
            0,
            None,
            Some(Box::new(HashSet::new())),
            None,
        );

        let field = Token::from("customData");
        let mut value = Value::default();
        let _ = context.compose_value(&field, &mut value);

        assert!(
            context
                .composed_field_names()
                .map(|s| s.contains(&field))
                .unwrap_or(false)
        );
    }
}
