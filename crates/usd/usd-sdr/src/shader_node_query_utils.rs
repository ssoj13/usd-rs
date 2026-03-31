//! Shader Node Query Utilities - Helper functions for query results.
//!
//! Port of pxr/usd/sdr/shaderNodeQueryUtils.h
//!
//! This module provides utility functions for working with shader node query results,
//! including the ability to group results into a nested tree form.

use super::shader_node_query::{SdrShaderNodeArcVec, SdrShaderNodeQueryResult};
use std::collections::HashMap;

/// A nested dictionary type for grouped query results.
///
/// The structure can be arbitrarily deep depending on the number of keys
/// in the query.
#[derive(Debug, Clone)]
pub enum GroupedQueryResult {
    /// Leaf node containing shader node pointers.
    Nodes(SdrShaderNodeArcVec),
    /// Intermediate node containing nested results.
    Nested(HashMap<String, GroupedQueryResult>),
}

impl GroupedQueryResult {
    /// Creates a new nested result.
    pub fn new_nested() -> Self {
        GroupedQueryResult::Nested(HashMap::new())
    }

    /// Creates a new leaf with nodes.
    pub fn new_nodes(nodes: SdrShaderNodeArcVec) -> Self {
        GroupedQueryResult::Nodes(nodes)
    }

    /// Returns true if this is a leaf node containing shader nodes.
    pub fn is_nodes(&self) -> bool {
        matches!(self, GroupedQueryResult::Nodes(_))
    }

    /// Returns true if this is a nested result.
    pub fn is_nested(&self) -> bool {
        matches!(self, GroupedQueryResult::Nested(_))
    }

    /// Returns the nodes if this is a leaf node, None otherwise.
    pub fn as_nodes(&self) -> Option<&SdrShaderNodeArcVec> {
        match self {
            GroupedQueryResult::Nodes(nodes) => Some(nodes),
            _ => None,
        }
    }

    /// Returns the nested map if this is a nested result, None otherwise.
    pub fn as_nested(&self) -> Option<&HashMap<String, GroupedQueryResult>> {
        match self {
            GroupedQueryResult::Nested(map) => Some(map),
            _ => None,
        }
    }

    /// Returns a mutable reference to the nested map if this is a nested result.
    pub fn as_nested_mut(&mut self) -> Option<&mut HashMap<String, GroupedQueryResult>> {
        match self {
            GroupedQueryResult::Nested(map) => Some(map),
            _ => None,
        }
    }

    /// Inserts a value at the given path.
    ///
    /// Creates intermediate nested nodes as needed.
    pub fn set_value_at_path(&mut self, path: &[String], nodes: SdrShaderNodeArcVec) {
        if path.is_empty() {
            *self = GroupedQueryResult::Nodes(nodes);
            return;
        }

        let key = &path[0];
        let remaining = &path[1..];

        // Ensure we have a nested structure
        if !self.is_nested() {
            *self = GroupedQueryResult::new_nested();
        }

        let map = self.as_nested_mut().expect("just set to nested");

        if remaining.is_empty() {
            // This is the final key, insert the nodes
            map.insert(key.clone(), GroupedQueryResult::Nodes(nodes));
        } else {
            // Need to go deeper
            let entry = map
                .entry(key.clone())
                .or_insert_with(GroupedQueryResult::new_nested);
            entry.set_value_at_path(remaining, nodes);
        }
    }

    /// Gets a value at the given path.
    pub fn get_value_at_path(&self, path: &[String]) -> Option<&GroupedQueryResult> {
        if path.is_empty() {
            return Some(self);
        }

        match self {
            GroupedQueryResult::Nested(map) => {
                let key = &path[0];
                let remaining = &path[1..];
                map.get(key).and_then(|v| v.get_value_at_path(remaining))
            }
            _ => None,
        }
    }
}

impl Default for GroupedQueryResult {
    fn default() -> Self {
        GroupedQueryResult::new_nested()
    }
}

/// Return shader node query results in a nested tree form.
///
/// For example, if a query result contains:
/// - values: `[["context1", "id1"], ["context1", "id2"]]`
/// - one shader node corresponding to the first value row
/// - two shader nodes corresponding to the second value row
///
/// The grouped result will be:
/// ```text
/// {
///   "context1": {
///     "id1": [<SdrShaderNodeArc>],
///     "id2": [<SdrShaderNodeArc>, <SdrShaderNodeArc>]
///   }
/// }
/// ```
///
/// Values are stringified. Empty values are preserved as keys (empty strings).
///
/// Returns an empty result if the given query result has no keys or no nodes.
pub fn group_query_results(result: &SdrShaderNodeQueryResult) -> GroupedQueryResult {
    // Return empty if no keys
    if result.get_keys().is_empty() {
        return GroupedQueryResult::new_nested();
    }

    let stringified_values = result.get_stringified_values();
    let nodes_by_values = result.get_shader_nodes_by_values();

    // Return empty if no values/nodes
    if stringified_values.is_empty() || nodes_by_values.is_empty() {
        return GroupedQueryResult::new_nested();
    }

    let mut grouped = GroupedQueryResult::new_nested();

    for (i, key_path) in stringified_values.iter().enumerate() {
        if i < nodes_by_values.len() {
            let nodes = nodes_by_values[i].clone();
            grouped.set_value_at_path(key_path, nodes);
        }
    }

    grouped
}

/// Flattens a grouped query result back into a list of (path, nodes) pairs.
pub fn flatten_grouped_results(
    grouped: &GroupedQueryResult,
) -> Vec<(Vec<String>, SdrShaderNodeArcVec)> {
    let mut results = Vec::new();
    flatten_recursive(grouped, &mut Vec::new(), &mut results);
    results
}

fn flatten_recursive(
    grouped: &GroupedQueryResult,
    current_path: &mut Vec<String>,
    results: &mut Vec<(Vec<String>, SdrShaderNodeArcVec)>,
) {
    match grouped {
        GroupedQueryResult::Nodes(nodes) => {
            results.push((current_path.clone(), nodes.clone()));
        }
        GroupedQueryResult::Nested(map) => {
            for (key, value) in map {
                current_path.push(key.clone());
                flatten_recursive(value, current_path, results);
                current_path.pop();
            }
        }
    }
}

/// Counts the total number of shader nodes in a grouped result.
pub fn count_nodes(grouped: &GroupedQueryResult) -> usize {
    match grouped {
        GroupedQueryResult::Nodes(nodes) => nodes.len(),
        GroupedQueryResult::Nested(map) => map.values().map(count_nodes).sum(),
    }
}

/// Returns all unique shader nodes from a grouped result.
pub fn collect_all_nodes(grouped: &GroupedQueryResult) -> SdrShaderNodeArcVec {
    let mut nodes = Vec::new();
    collect_nodes_recursive(grouped, &mut nodes);
    nodes
}

fn collect_nodes_recursive(grouped: &GroupedQueryResult, nodes: &mut SdrShaderNodeArcVec) {
    match grouped {
        GroupedQueryResult::Nodes(n) => {
            nodes.extend(n.iter().cloned());
        }
        GroupedQueryResult::Nested(map) => {
            for value in map.values() {
                collect_nodes_recursive(value, nodes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grouped_result_creation() {
        let grouped = GroupedQueryResult::new_nested();
        assert!(grouped.is_nested());
        assert!(!grouped.is_nodes());
    }

    #[test]
    fn test_set_value_at_path() {
        let mut grouped = GroupedQueryResult::new_nested();
        let path = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        grouped.set_value_at_path(&path, vec![]);

        // Should create nested structure
        let result = grouped.get_value_at_path(&path);
        assert!(result.is_some());
        assert!(result.unwrap().is_nodes());
    }

    #[test]
    fn test_get_value_at_path() {
        let mut grouped = GroupedQueryResult::new_nested();
        let path = vec!["x".to_string(), "y".to_string()];
        grouped.set_value_at_path(&path, vec![]);

        // Existing path
        assert!(grouped.get_value_at_path(&path).is_some());

        // Non-existing path
        let bad_path = vec!["z".to_string()];
        assert!(grouped.get_value_at_path(&bad_path).is_none());
    }

    #[test]
    fn test_flatten_grouped_results() {
        let mut grouped = GroupedQueryResult::new_nested();
        grouped.set_value_at_path(&["a".to_string(), "b".to_string()], vec![]);
        grouped.set_value_at_path(&["a".to_string(), "c".to_string()], vec![]);
        grouped.set_value_at_path(&["d".to_string()], vec![]);

        let flattened = flatten_grouped_results(&grouped);
        assert_eq!(flattened.len(), 3);
    }

    #[test]
    fn test_count_nodes_empty() {
        let grouped = GroupedQueryResult::new_nested();
        assert_eq!(count_nodes(&grouped), 0);
    }

    #[test]
    fn test_group_empty_result() {
        let result = SdrShaderNodeQueryResult::new();
        let grouped = group_query_results(&result);
        assert!(grouped.is_nested());
        assert_eq!(count_nodes(&grouped), 0);
    }
}
