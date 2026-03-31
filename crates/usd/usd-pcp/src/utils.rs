//! PCP utility functions.
//!
//! Internal helper functions for PCP composition, including variable expression
//! evaluation, file format argument handling, path translation, and class hierarchy
//! navigation.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/utils.h` and `utils.cpp`.

use std::collections::{HashMap, HashSet};

use crate::{ArcType, ExpressionVariables, NodeRef};
use usd_sdf::{LayerHandle, Path};

/// File format arguments type (maps argument name to value).
pub type FileFormatArguments = HashMap<String, String>;

/// Standard file format token for target argument.
pub const TARGET_ARG: &str = "target";

// ============================================================================
// Variable Expression Functions
// ============================================================================

/// Evaluates a variable expression using the given expression variables.
///
/// Variables that are used during evaluation will be inserted into `used_variables`.
/// Any errors that occur during evaluation will be appended to `errors`.
///
/// # Arguments
///
/// * `expression` - The expression to evaluate
/// * `expression_vars` - The variables available for substitution
/// * `context` - A string like "sublayer" or "reference" for error messages
/// * `source_layer` - The layer where the expression is defined
/// * `source_path` - The path where the expression is defined
/// * `used_variables` - Optional set to receive variable names used in evaluation
/// * `errors` - Optional vector to receive evaluation errors
///
/// # Returns
///
/// The result of evaluation if successful, or empty string on error.
pub fn evaluate_variable_expression(
    expression: &str,
    expression_vars: &ExpressionVariables,
    context: &str,
    source_layer: Option<&LayerHandle>,
    source_path: &Path,
    used_variables: Option<&mut HashSet<String>>,
    errors: Option<&mut Vec<VariableExpressionError>>,
) -> String {
    // Check if this is actually a variable expression
    if !is_variable_expression(expression) {
        return expression.to_string();
    }

    // Evaluate the expression by substituting variables
    let result = evaluate_expression_internal(expression, expression_vars, used_variables);

    match result {
        Ok(value) => value,
        Err(err_msg) => {
            if let Some(errs) = errors {
                errs.push(VariableExpressionError {
                    expression: expression.to_string(),
                    expression_error: err_msg,
                    context: context.to_string(),
                    source_layer: source_layer.cloned(),
                    source_path: source_path.clone(),
                });
            }
            String::new()
        }
    }
}

/// Simplified overload that does not populate used_variables or errors.
pub fn evaluate_variable_expression_simple(
    expression: &str,
    expression_vars: &ExpressionVariables,
) -> String {
    evaluate_variable_expression(
        expression,
        expression_vars,
        "",
        None,
        &Path::empty(),
        None,
        None,
    )
}

/// Checks if a string is a variable expression.
///
/// A variable expression starts with `${` and ends with `}`, or contains
/// `${...}` substitution patterns.
pub fn is_variable_expression(s: &str) -> bool {
    // Check for `${...}` pattern
    s.contains("${")
}

/// Error from evaluating a variable expression.
#[derive(Clone, Debug)]
pub struct VariableExpressionError {
    /// The expression that failed.
    pub expression: String,
    /// The error message from evaluation.
    pub expression_error: String,
    /// Context string (e.g., "sublayer", "reference").
    pub context: String,
    /// Source layer where the expression was defined.
    pub source_layer: Option<LayerHandle>,
    /// Source path where the expression was defined.
    pub source_path: Path,
}

/// Internal expression evaluation.
fn evaluate_expression_internal(
    expression: &str,
    expression_vars: &ExpressionVariables,
    mut used_variables: Option<&mut HashSet<String>>,
) -> Result<String, String> {
    let mut result = String::new();
    let chars: Vec<char> = expression.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '{' {
            // Start of variable substitution
            i += 2; // Skip "${"
            let start = i;

            // Find closing brace
            while i < chars.len() && chars[i] != '}' {
                i += 1;
            }

            if i >= chars.len() {
                return Err(format!("Unclosed variable expression in '{}'", expression));
            }

            let var_name: String = chars[start..i].iter().collect();
            i += 1; // Skip "}"

            // Record used variable
            if let Some(ref mut used) = used_variables {
                used.insert(var_name.clone());
            }

            // Look up the variable value
            if let Some(value) = expression_vars.variables().get(&var_name) {
                // Try to get string value from the dictionary value
                if let Some(s) = value.get::<String>() {
                    result.push_str(s);
                } else if let Some(s) = value.get::<&str>() {
                    result.push_str(s);
                } else {
                    result.push_str(&format!("{:?}", value));
                }
            } else {
                return Err(format!(
                    "Unknown variable '{}' in expression '{}'",
                    var_name, expression
                ));
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    Ok(result)
}

// ============================================================================
// File Format Target Functions
// ============================================================================

/// Returns file format arguments with the "target" argument set if not empty.
pub fn get_arguments_for_file_format_target(target: &str) -> FileFormatArguments {
    let mut args = FileFormatArguments::new();
    if !target.is_empty() {
        args.insert(TARGET_ARG.to_string(), target.to_string());
    }
    args
}

/// Returns file format arguments with target set if not already in identifier.
///
/// If the identifier already contains a target argument, returns empty args.
pub fn get_arguments_for_file_format_target_with_identifier(
    identifier: &str,
    target: &str,
) -> FileFormatArguments {
    let mut args = FileFormatArguments::new();
    get_arguments_for_file_format_target_into(identifier, target, &mut args);
    args
}

/// Modifies args to add target argument if not in identifier.
pub fn get_arguments_for_file_format_target_into(
    identifier: &str,
    target: &str,
    args: &mut FileFormatArguments,
) {
    if !target.is_empty() && !target_is_specified_in_identifier(identifier) {
        args.insert(TARGET_ARG.to_string(), target.to_string());
    }
}

/// Returns appropriate file format arguments, removing target if already in identifier.
///
/// If a target argument is embedded in the identifier, returns a copy of default_args
/// with the target argument removed. Otherwise, returns default_args unchanged.
pub fn get_arguments_for_file_format_target_stripped<'a>(
    identifier: &str,
    default_args: &'a FileFormatArguments,
    local_args: &'a mut FileFormatArguments,
) -> &'a FileFormatArguments {
    if !target_is_specified_in_identifier(identifier) {
        return default_args;
    }

    *local_args = default_args.clone();
    local_args.remove(TARGET_ARG);
    local_args
}

/// Removes the "target" argument from args if it matches the given target.
pub fn strip_file_format_target(target: &str, args: &mut FileFormatArguments) {
    if let Some(value) = args.get(TARGET_ARG) {
        if value == target {
            args.remove(TARGET_ARG);
        }
    }
}

/// Checks if the identifier contains a target argument.
fn target_is_specified_in_identifier(identifier: &str) -> bool {
    // Check for target argument in identifier
    // Identifiers can have arguments in the form: "layer.usda:target=xxx"
    if let Some(colon_pos) = identifier.rfind(':') {
        let args_part = &identifier[colon_pos + 1..];
        // Parse simple key=value arguments
        for arg in args_part.split(',') {
            let parts: Vec<&str> = arg.splitn(2, '=').collect();
            if parts.len() == 2 && parts[0] == TARGET_ARG {
                return true;
            }
        }
    }
    false
}

// ============================================================================
// Class Hierarchy Functions
// ============================================================================

/// Find the starting node of the class hierarchy of which node n is a part.
///
/// Returns (instance_node, class_node) where:
/// - `instance_node` is the prim that starts the class chain
/// - `class_node` is the first class in the chain that the instance inherits from
///
/// For example, with an inherits chain I --> C1 --> C2 --> C3:
/// When given C1, C2, or C3, this returns (I, C1).
///
/// # Panics
///
/// Panics if the node is not part of a class-based arc.
pub fn find_starting_node_of_class_hierarchy(n: &NodeRef) -> (NodeRef, NodeRef) {
    assert!(
        n.arc_type().is_class_based(),
        "Node must be part of a class-based arc"
    );

    let mut instance_node = n.clone();
    let mut class_node = NodeRef::invalid();

    // Handle propagated specializes
    if is_propagated_specializes_node(&instance_node) {
        instance_node = instance_node.origin_node();
    }

    let depth = instance_node.depth_below_introduction();

    // Walk up while we're still in a class-based arc at the same depth
    while instance_node.arc_type().is_class_based()
        && instance_node.depth_below_introduction() == depth
    {
        let parent = instance_node.parent_node();
        if !parent.is_valid() {
            break;
        }

        class_node = instance_node.clone();
        instance_node = parent;

        // Handle propagated specializes
        if is_propagated_specializes_node(&instance_node) {
            instance_node = instance_node.origin_node();
        }
    }

    (instance_node, class_node)
}

// ============================================================================
// Path Translation Functions
// ============================================================================

/// Translates a path from a node to the root or closest possible node.
///
/// The path (which must be a prim or prim variant selection path) is translated
/// from the namespace of the given node toward the root node. If translation
/// to the root fails at any point, returns the path translated to the closest
/// ancestor node where mapping is successful.
///
/// # Returns
///
/// A pair of (translated_path, closest_node) where closest_node is either the
/// root node (if translation succeeded fully) or the ancestor where translation
/// stopped.
pub fn translate_path_from_node_to_root_or_closest(node: &NodeRef, path: &Path) -> (Path, NodeRef) {
    if node.is_root_node() {
        // Already at root, nothing to do
        return (path.clone(), node.clone());
    }

    let mut cur_node = node.clone();
    let mut cur_path = path.strip_all_variant_selections();

    // First try direct translation to root
    let map_to_root = node.map_to_root();
    if let Some(path_in_root) = map_to_root.map_source_to_target(&cur_path) {
        if !path_in_root.is_empty() {
            cur_node = node.root_node();
            cur_path = path_in_root;
        }
    } else {
        // Walk up step by step until translation fails
        while !cur_node.is_root_node() {
            let map_to_parent = cur_node.map_to_parent();
            if let Some(path_in_parent) = map_to_parent.map_source_to_target(&cur_path) {
                if !path_in_parent.is_empty() {
                    cur_node = cur_node.parent_node();
                    cur_path = path_in_parent;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    // If cur_node's path contains a variant selection, apply it to the translated path
    let path_at_intro = cur_node.path_at_introduction();
    if path_at_intro.contains_prim_variant_selection() {
        let stripped = path_at_intro.strip_all_variant_selections();
        if let Some(replaced) = cur_path.replace_prefix(&stripped, &path_at_intro) {
            cur_path = replaced;
        }
    }

    (cur_path, cur_node)
}

// ============================================================================
// Propagated Specializes Node Check
// ============================================================================

/// Returns true if the given node is a specializes node that has been
/// propagated to the root of the graph for strength ordering purposes.
///
/// A propagated specializes node has:
/// - Specialize arc type
/// - Parent is the root node
/// - Same site as its origin node
pub fn is_propagated_specializes_node(node: &NodeRef) -> bool {
    if !node.arc_type().is_specialize() {
        return false;
    }

    let parent = node.parent_node();
    let root = node.root_node();

    if parent != root {
        return false;
    }

    let origin = node.origin_node();
    let site = node.site();
    let origin_site = origin.site();
    if site.is_valid() && origin_site.is_valid() {
        site == origin_site
    } else {
        false
    }
}

/// Checks if the given arc type is a specialize arc.
#[inline]
pub fn is_specialize_arc(arc_type: ArcType) -> bool {
    arc_type.is_specialize()
}

/// Checks if the given arc type is a class-based arc (inherit or specialize).
#[inline]
pub fn is_class_based_arc(arc_type: ArcType) -> bool {
    arc_type.is_class_based()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExpressionVariablesSource;
    use usd_vt::Dictionary;

    #[test]
    fn test_is_variable_expression() {
        assert!(is_variable_expression("${VAR}"));
        assert!(is_variable_expression("prefix/${VAR}/suffix"));
        assert!(is_variable_expression("${VAR1}/${VAR2}"));
        assert!(!is_variable_expression("plain_string"));
        assert!(!is_variable_expression("$VAR")); // Not enclosed in braces
        assert!(!is_variable_expression(""));
    }

    #[test]
    fn test_evaluate_simple_expression() {
        let source = ExpressionVariablesSource::new();
        let mut vars_dict = Dictionary::new();
        vars_dict.insert("ROOT", "/assets");
        let expr_vars = ExpressionVariables::new(source, vars_dict);

        let result = evaluate_variable_expression_simple("${ROOT}/model.usda", &expr_vars);
        assert_eq!(result, "/assets/model.usda");
    }

    #[test]
    fn test_evaluate_no_variables() {
        let source = ExpressionVariablesSource::new();
        let expr_vars = ExpressionVariables::new(source, Dictionary::new());

        let result = evaluate_variable_expression_simple("plain.usda", &expr_vars);
        assert_eq!(result, "plain.usda");
    }

    #[test]
    fn test_evaluate_with_used_variables() {
        let source = ExpressionVariablesSource::new();
        let mut vars_dict = Dictionary::new();
        vars_dict.insert("A", "aaa");
        vars_dict.insert("B", "bbb");
        let expr_vars = ExpressionVariables::new(source, vars_dict);

        let mut used = HashSet::new();
        let result = evaluate_variable_expression(
            "${A}/${B}",
            &expr_vars,
            "test",
            None,
            &Path::empty(),
            Some(&mut used),
            None,
        );

        assert_eq!(result, "aaa/bbb");
        assert!(used.contains("A"));
        assert!(used.contains("B"));
    }

    #[test]
    fn test_evaluate_unknown_variable() {
        let source = ExpressionVariablesSource::new();
        let expr_vars = ExpressionVariables::new(source, Dictionary::new());

        let mut errors = Vec::new();
        let result = evaluate_variable_expression(
            "${UNKNOWN}",
            &expr_vars,
            "test",
            None,
            &Path::empty(),
            None,
            Some(&mut errors),
        );

        assert!(result.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].expression_error.contains("Unknown variable"));
    }

    #[test]
    fn test_file_format_target_basic() {
        let args = get_arguments_for_file_format_target("usd");
        assert_eq!(args.get(TARGET_ARG), Some(&"usd".to_string()));

        let empty_args = get_arguments_for_file_format_target("");
        assert!(empty_args.is_empty());
    }

    #[test]
    fn test_file_format_target_with_identifier() {
        // No target in identifier - should add it
        let args = get_arguments_for_file_format_target_with_identifier("layer.usda", "usd");
        assert_eq!(args.get(TARGET_ARG), Some(&"usd".to_string()));

        // Target already in identifier - should not add
        let args =
            get_arguments_for_file_format_target_with_identifier("layer.usda:target=abc", "usd");
        assert!(args.is_empty());
    }

    #[test]
    fn test_strip_file_format_target() {
        let mut args = FileFormatArguments::new();
        args.insert(TARGET_ARG.to_string(), "usd".to_string());
        args.insert("other".to_string(), "value".to_string());

        // Matching target - should remove
        strip_file_format_target("usd", &mut args);
        assert!(!args.contains_key(TARGET_ARG));
        assert!(args.contains_key("other"));

        // Non-matching target - should keep
        let mut args2 = FileFormatArguments::new();
        args2.insert(TARGET_ARG.to_string(), "usd".to_string());
        strip_file_format_target("other", &mut args2);
        assert!(args2.contains_key(TARGET_ARG));
    }

    #[test]
    fn test_target_is_specified_in_identifier() {
        assert!(target_is_specified_in_identifier("layer.usda:target=usd"));
        assert!(target_is_specified_in_identifier(
            "layer.usda:other=val,target=usd"
        ));
        assert!(!target_is_specified_in_identifier("layer.usda"));
        assert!(!target_is_specified_in_identifier("layer.usda:other=val"));
    }

    #[test]
    fn test_evaluate_multiple_variables_underscore() {
        let source = ExpressionVariablesSource::new();
        let mut vars_dict = Dictionary::new();
        vars_dict.insert("A", "alpha");
        vars_dict.insert("B", "beta");
        let expr_vars = ExpressionVariables::new(source, vars_dict);

        let result = evaluate_variable_expression_simple("${A}_${B}", &expr_vars);
        assert_eq!(result, "alpha_beta");
    }

    #[test]
    fn test_evaluate_unclosed_variable_expression() {
        let source = ExpressionVariablesSource::new();
        let mut vars_dict = Dictionary::new();
        vars_dict.insert("X", "val");
        let expr_vars = ExpressionVariables::new(source, vars_dict);

        let mut errors = Vec::new();
        let result = evaluate_variable_expression(
            "${X",
            &expr_vars,
            "test",
            None,
            &Path::empty(),
            None,
            Some(&mut errors),
        );
        assert!(result.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].expression_error.contains("Unclosed"));
    }

    #[test]
    fn test_evaluate_dollar_without_brace() {
        // A lone $ without { should be passed through
        let source = ExpressionVariablesSource::new();
        let expr_vars = ExpressionVariables::new(source, Dictionary::new());

        let result = evaluate_variable_expression_simple("price$5", &expr_vars);
        assert_eq!(result, "price$5");
    }

    #[test]
    fn test_evaluate_adjacent_variables() {
        let source = ExpressionVariablesSource::new();
        let mut vars_dict = Dictionary::new();
        vars_dict.insert("X", "hello");
        vars_dict.insert("Y", "world");
        let expr_vars = ExpressionVariables::new(source, vars_dict);

        let result = evaluate_variable_expression_simple("${X}${Y}", &expr_vars);
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_evaluate_empty_expression() {
        let source = ExpressionVariablesSource::new();
        let expr_vars = ExpressionVariables::new(source, Dictionary::new());

        let result = evaluate_variable_expression_simple("", &expr_vars);
        assert_eq!(result, "");
    }

    #[test]
    fn test_is_propagated_specializes_node_invalid() {
        let invalid = NodeRef::invalid();
        assert!(!is_propagated_specializes_node(&invalid));
    }

    #[test]
    fn test_arc_type_checks() {
        assert!(is_specialize_arc(ArcType::Specialize));
        assert!(!is_specialize_arc(ArcType::Reference));

        assert!(is_class_based_arc(ArcType::Inherit));
        assert!(is_class_based_arc(ArcType::Specialize));
        assert!(!is_class_based_arc(ArcType::Reference));
    }
}
