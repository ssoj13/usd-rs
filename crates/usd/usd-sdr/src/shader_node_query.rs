//! Shader Node Query - Constraint-based query builder for shader nodes.
//!
//! Port of pxr/usd/sdr/shaderNodeQuery.h
//!
//! This module provides `SdrShaderNodeQuery`, a constraint-based query builder
//! that operates on all SdrShaderNodes contained in the SdrRegistry.
//!
//! # Example
//!
//! ```ignore
//! use usd_sdr::{SdrShaderNodeQuery, SdrRegistry};
//!
//! // Query for all nodes with a specific metadata value
//! let query = SdrShaderNodeQuery::new()
//!     .node_value_is(&Token::new("customMetadata"), "stage2")
//!     .node_value_is_not(&Token::new("identifier"), "notthisone");
//!
//! let result = query.run();
//! for node in result.get_all_shader_nodes() {
//!     println!("Found: {}", node.get_name());
//! }
//! ```
//!
//! Queries may additionally specify `select_distinct` to get aggregated data
//! from the nodes satisfying given constraints.

use std::sync::Arc;

use super::declare::SdrTokenVec;
use super::shader_node::SdrShaderNode;
use usd_tf::Token;
use usd_vt::Value;

/// A shared pointer to a shader node for query results.
pub type SdrShaderNodeArc = Arc<SdrShaderNode>;

/// A vector of shared shader node pointers.
pub type SdrShaderNodeArcVec = Vec<SdrShaderNodeArc>;

/// A filter function that operates on a shader node and returns true to keep it.
pub type SdrShaderNodeFilterFn = Box<dyn Fn(&SdrShaderNode) -> bool + Send + Sync>;

/// SdrShaderNodeQuery is a constraint-based query builder object that
/// operates on all SdrShaderNodes contained in the SdrRegistry.
///
/// Queries can be used to get nodes associated with given constraints, or to
/// examine specific data from the nodes.
///
/// # Filtering Semantics
///
/// - **Inclusion constraints** (NodeValueIs, NodeValueIsIn, NodeHasValueFor):
///   Joined with boolean "and" - only nodes satisfying ALL these constraints are kept.
///
/// - **Exclusion constraints** (NodeValueIsNot, NodeValueIsNotIn, NodeHasNoValueFor):
///   Joined with boolean "or" - only nodes satisfying NONE of these constraints are kept.
///
/// # Value Semantics
///
/// Nonexistence and empty VtValue are considered equivalent states.
/// A VtValue containing an empty item (e.g., empty string) is NOT equivalent
/// to the former states.
#[derive(Default)]
pub struct SdrShaderNodeQuery {
    /// Inclusion constraints: (key, value) pairs that must match
    has_values: Vec<(Token, Value)>,

    /// Inclusion constraints: (key, values) where any value must match
    has_one_of_values: Vec<(Token, Vec<Value>)>,

    /// Exclusion constraints: (key, value) pairs that must NOT match
    lacks_values: Vec<(Token, Value)>,

    /// Exclusion constraints: (key, values) where NONE should match
    lacks_all_of_values: Vec<(Token, Vec<Value>)>,

    /// Keys to select distinct values for
    select_keys: SdrTokenVec,

    /// Custom filter functions
    custom_filters: Vec<SdrShaderNodeFilterFn>,
}

impl SdrShaderNodeQuery {
    /// Creates a new empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies inclusion/exclusion/`select_distinct` state without custom Rust filters.
    ///
    /// Used when Python supplies [`Vec<Py<PyAny>>`] custom filters separately
    /// and the final [`SdrShaderNodeQuery`] must merge them at run time.
    pub fn clone_without_custom_filters(&self) -> Self {
        Self {
            has_values: self.has_values.clone(),
            has_one_of_values: self.has_one_of_values.clone(),
            lacks_values: self.lacks_values.clone(),
            lacks_all_of_values: self.lacks_all_of_values.clone(),
            select_keys: self.select_keys.clone(),
            custom_filters: Vec::new(),
        }
    }

    /// SelectDistinct asks for distinct information from SdrShaderNodes via
    /// the SdrShaderNode::get_data_for_key method.
    ///
    /// Any number of keys can be requested. If SelectDistinct is not called
    /// and therefore no keys are requested, the result of running this query
    /// will contain no data but will contain nodes that satisfy the query
    /// filter constraints.
    ///
    /// This echoes SQL's "SELECT DISTINCT" behavior - duplicate value combinations
    /// are coalesced.
    pub fn select_distinct(mut self, key: &Token) -> Self {
        self.select_keys.push(key.clone());
        self
    }

    /// Specify multiple keys to query data for.
    pub fn select_distinct_multi(mut self, keys: &[Token]) -> Self {
        self.select_keys.extend(keys.iter().cloned());
        self
    }

    // ========================================================================
    // Inclusion constraints (AND semantics)
    // ========================================================================

    /// Only keep SdrShaderNodes whose value returned from
    /// `get_data_for_key(key)` matches the given `value`.
    pub fn node_value_is(mut self, key: &Token, value: impl Into<Value>) -> Self {
        self.has_values.push((key.clone(), value.into()));
        self
    }

    /// Only keep SdrShaderNodes whose value returned from
    /// `get_data_for_key(key)` matches any of the given `values`.
    pub fn node_value_is_in(mut self, key: &Token, values: Vec<Value>) -> Self {
        self.has_one_of_values.push((key.clone(), values));
        self
    }

    /// Only keep SdrShaderNodes that have an existing (non-empty) value
    /// for the given key.
    pub fn node_has_value_for(mut self, key: &Token) -> Self {
        // Empty VtValue is semantically equivalent to nonexistence
        self.lacks_values.push((key.clone(), Value::default()));
        self
    }

    // ========================================================================
    // Exclusion constraints (OR semantics)
    // ========================================================================

    /// Only keep SdrShaderNodes whose value returned from
    /// `get_data_for_key(key)` doesn't match the given 'value'.
    pub fn node_value_is_not(mut self, key: &Token, value: impl Into<Value>) -> Self {
        self.lacks_values.push((key.clone(), value.into()));
        self
    }

    /// Only keep SdrShaderNodes whose value returned from
    /// `get_data_for_key(key)` doesn't match any of the given `values`.
    pub fn node_value_is_not_in(mut self, key: &Token, values: Vec<Value>) -> Self {
        self.lacks_all_of_values.push((key.clone(), values));
        self
    }

    /// Only keep SdrShaderNodes that don't have an existing value for
    /// for the given key. Empty values are considered "existing".
    pub fn node_has_no_value_for(mut self, key: &Token) -> Self {
        // Empty VtValue is semantically equivalent to nonexistence
        self.has_values.push((key.clone(), Value::default()));
        self
    }

    // ========================================================================
    // Custom filters
    // ========================================================================

    /// Supply a custom filter to this query.
    ///
    /// This custom filter function will run on every considered SdrShaderNode.
    /// When this function evaluates to true, the node will be kept for further
    /// consideration. When the function evaluates to false, the node will be
    /// discarded from further consideration.
    pub fn custom_filter(mut self, filter: SdrShaderNodeFilterFn) -> Self {
        self.custom_filters.push(filter);
        self
    }

    // ========================================================================
    // Query execution
    // ========================================================================

    /// Convenience to run this query on the SdrRegistry.
    ///
    /// Equivalent to `SdrRegistry::run_query(query)`.
    pub fn run(self) -> SdrShaderNodeQueryResult {
        use super::registry::SdrRegistry;
        SdrRegistry::get_instance().run_query(&self)
    }

    /// Evaluates whether a node matches the inclusion constraints.
    pub fn matches_inclusion(&self, node: &SdrShaderNode) -> bool {
        // Check all has_values constraints (AND)
        for (key, expected) in &self.has_values {
            let actual = node.get_data_for_key(key);
            if !values_match(&actual, expected) {
                return false;
            }
        }

        // Check all has_one_of_values constraints (AND with OR for values)
        for (key, expected_values) in &self.has_one_of_values {
            let actual = node.get_data_for_key(key);
            let matches_any = expected_values
                .iter()
                .any(|expected| values_match(&actual, expected));
            if !matches_any {
                return false;
            }
        }

        true
    }

    /// Evaluates whether a node matches the exclusion constraints.
    pub fn matches_exclusion(&self, node: &SdrShaderNode) -> bool {
        // Check lacks_values constraints (OR - must match NONE)
        for (key, excluded) in &self.lacks_values {
            let actual = node.get_data_for_key(key);
            if values_match(&actual, excluded) {
                return false;
            }
        }

        // Check lacks_all_of_values constraints (OR with AND for values)
        for (key, excluded_values) in &self.lacks_all_of_values {
            let actual = node.get_data_for_key(key);
            let matches_any = excluded_values
                .iter()
                .any(|excluded| values_match(&actual, excluded));
            if matches_any {
                return false;
            }
        }

        true
    }

    /// Evaluates custom filters.
    pub(crate) fn matches_custom_filters(&self, node: &SdrShaderNode) -> bool {
        self.custom_filters.iter().all(|filter| filter(node))
    }

    /// Returns whether a node satisfies all constraints.
    pub fn matches(&self, node: &SdrShaderNode) -> bool {
        self.matches_inclusion(node)
            && self.matches_exclusion(node)
            && self.matches_custom_filters(node)
    }

    /// Returns the select keys.
    pub fn get_select_keys(&self) -> &SdrTokenVec {
        &self.select_keys
    }
}

/// Compares two Values for equality.
fn values_match(a: &Value, b: &Value) -> bool {
    // Empty values are equivalent
    if a.is_empty() && b.is_empty() {
        return true;
    }
    if a == b {
        return true;
    }
    // `SdrShaderNode::get_data_for_key` returns `TfToken` for Identifier / Family /
    // SourceType; Python `NodeValueIs` passes `str`. OpenUSD treats these as matching
    // when the string content is equal.
    if let Some(ta) = a.get::<Token>() {
        if let Some(sb) = b.get::<String>() {
            return ta.as_str() == sb.as_str();
        }
    }
    if let Some(sa) = a.get::<String>() {
        if let Some(tb) = b.get::<Token>() {
            return sa.as_str() == tb.as_str();
        }
    }
    false
}

/// String form of a query result cell for [`SdrShaderNodeQueryResult::get_stringified_values`].
///
/// Empty values stringify to `""` (matches OpenUSD `GetStringifiedValues` tests).
fn stringify_query_result_value(v: &Value) -> String {
    if v.is_empty() {
        return String::new();
    }
    if let Some(t) = v.get::<Token>() {
        return t.as_str().to_string();
    }
    if let Some(s) = v.get::<String>() {
        return s.clone();
    }
    if let Some(b) = v.get::<bool>() {
        return b.to_string();
    }
    if let Some(i) = v.get::<i32>() {
        return i.to_string();
    }
    if let Some(i) = v.get::<i64>() {
        return i.to_string();
    }
    if let Some(x) = v.get::<f32>() {
        return x.to_string();
    }
    if let Some(x) = v.get::<f64>() {
        return x.to_string();
    }
    format!("{v:?}")
}

// ============================================================================
// SdrShaderNodeQueryResult
// ============================================================================

/// Stores the results of an SdrShaderNodeQuery.
#[derive(Default)]
pub struct SdrShaderNodeQueryResult {
    /// Keys requested by SelectDistinct calls.
    keys: SdrTokenVec,

    /// Distinct value rows extracted from nodes.
    values: Vec<Vec<Value>>,

    /// Nodes grouped by value rows.
    nodes: Vec<SdrShaderNodeArcVec>,

    /// Matches when the query had **no** `SelectDistinct`: flat list for `get_all_shader_nodes`.
    /// Left empty when `SelectDistinct` was used so `get_shader_nodes_by_values` stays empty
    /// (matches OpenUSD).
    no_select_nodes: SdrShaderNodeArcVec,
}

impl SdrShaderNodeQueryResult {
    /// Creates an empty query result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a query result with the given data.
    pub fn with_data(
        keys: SdrTokenVec,
        values: Vec<Vec<Value>>,
        nodes: Vec<SdrShaderNodeArcVec>,
    ) -> Self {
        Self {
            keys,
            values,
            nodes,
            no_select_nodes: SdrShaderNodeArcVec::new(),
        }
    }

    /// Result for a query with no `SelectDistinct`: no value rows, nodes only via
    /// [`Self::get_all_shader_nodes`].
    pub fn with_no_select_data(mut matching_nodes: SdrShaderNodeArcVec) -> Self {
        matching_nodes.sort_by(|a, b| {
            let id_cmp = a.get_identifier().as_str().cmp(b.get_identifier().as_str());
            if id_cmp == std::cmp::Ordering::Equal {
                a.get_source_type()
                    .as_str()
                    .cmp(b.get_source_type().as_str())
            } else {
                id_cmp
            }
        });
        Self {
            keys: SdrTokenVec::new(),
            values: Vec::new(),
            nodes: Vec::new(),
            no_select_nodes: matching_nodes,
        }
    }

    /// Returns keys requested by SelectDistinct calls on SdrShaderNodeQuery
    /// in the order they were added to the query.
    ///
    /// If the query had no calls to SelectDistinct, returns an empty vector.
    pub fn get_keys(&self) -> &SdrTokenVec {
        &self.keys
    }

    /// Moves out the keys (consuming self).
    pub fn take_keys(self) -> SdrTokenVec {
        self.keys
    }

    /// Returns distinct "list of values" extracted from SdrShaderNodes
    /// corresponding to keys requested by SelectDistinct calls.
    ///
    /// The result is an (N x M) container of Values, where M is the number
    /// of keys and N is the number of distinct "list of values" that
    /// correspond to the keys.
    ///
    /// Non-existent values are represented by empty Values.
    pub fn get_values(&self) -> &Vec<Vec<Value>> {
        &self.values
    }

    /// Moves out the values (consuming self).
    pub fn take_values(self) -> Vec<Vec<Value>> {
        self.values
    }

    /// Converts all values to the requested type `T`.
    ///
    /// Matches C++ `GetValuesAs<T>()`. If any value fails conversion,
    /// returns `None`. Empty values always fail conversion.
    pub fn get_values_as<T: Clone + 'static>(&self) -> Option<Vec<Vec<T>>> {
        let mut result = Vec::with_capacity(self.values.len());
        for row in &self.values {
            let mut converted_row = Vec::with_capacity(row.len());
            for value in row {
                match value.downcast_clone::<T>() {
                    Some(v) => converted_row.push(v),
                    None => {
                        log::error!("Failed to convert value in query result to requested type");
                        return None;
                    }
                }
            }
            result.push(converted_row);
        }
        Some(result)
    }

    /// Get string representations of all values.
    pub fn get_stringified_values(&self) -> Vec<Vec<String>> {
        self.values
            .iter()
            .map(|row| row.iter().map(stringify_query_result_value).collect())
            .collect()
    }

    /// Gets shader nodes, grouped by value rows.
    ///
    /// The result is an (N x S) container of shader nodes, where S is the
    /// number of shader nodes that have the key-value characteristics
    /// represented by the "nth" row of the returned structure of get_values.
    /// S is not constant, and may vary from row to row.
    ///
    /// Each SdrShaderNodeArcVec is sorted alphabetically by identifier,
    /// then sourceType.
    pub fn get_shader_nodes_by_values(&self) -> &Vec<SdrShaderNodeArcVec> {
        &self.nodes
    }

    /// Returns all shader nodes that match the constraints of the query.
    ///
    /// The resulting SdrShaderNodeArcVec is sorted alphabetically by identifier,
    /// then sourceType.
    pub fn get_all_shader_nodes(&self) -> SdrShaderNodeArcVec {
        if !self.no_select_nodes.is_empty() {
            return self.no_select_nodes.clone();
        }

        let mut result = SdrShaderNodeArcVec::new();
        for inner in &self.nodes {
            result.extend(inner.iter().cloned());
        }

        // Sort by identifier, then source type
        result.sort_by(|a, b| {
            let id_cmp = a.get_identifier().as_str().cmp(b.get_identifier().as_str());
            if id_cmp == std::cmp::Ordering::Equal {
                a.get_source_type()
                    .as_str()
                    .cmp(b.get_source_type().as_str())
            } else {
                id_cmp
            }
        });

        result
    }

    /// Returns true if the contents of this result are well-formed.
    pub fn is_valid(&self) -> bool {
        if !self.no_select_nodes.is_empty() {
            return self.keys.is_empty() && self.values.is_empty() && self.nodes.is_empty();
        }

        let num_keys = self.keys.len();

        // Check that all value rows have the correct number of columns
        for value_row in &self.values {
            if num_keys != value_row.len() {
                return false;
            }
        }

        // Check that nodes and values have the same number of rows
        self.nodes.len() == self.values.len()
    }

    /// Returns true if the result is empty.
    pub fn is_empty(&self) -> bool {
        self.no_select_nodes.is_empty() && self.nodes.is_empty() && self.values.is_empty()
    }

    /// Returns the number of distinct value combinations.
    pub fn len(&self) -> usize {
        self.values.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder() {
        let query = SdrShaderNodeQuery::new()
            .select_distinct(&Token::new("family"))
            .node_value_is(&Token::new("context"), Value::from("pattern"))
            .node_value_is_not(&Token::new("deprecated"), Value::from(true));

        assert_eq!(query.select_keys.len(), 1);
        assert_eq!(query.has_values.len(), 1);
        assert_eq!(query.lacks_values.len(), 1);
    }

    #[test]
    fn test_query_result_empty() {
        let result = SdrShaderNodeQueryResult::new();
        assert!(result.is_empty());
        assert!(result.get_keys().is_empty());
        assert!(result.get_values().is_empty());
    }

    #[test]
    fn test_query_result_validity() {
        let result = SdrShaderNodeQueryResult::with_data(
            vec![Token::new("a"), Token::new("b")],
            vec![
                vec![Value::from("val1"), Value::from("val2")],
                vec![Value::from("val3"), Value::from("val4")],
            ],
            vec![vec![], vec![]], // Empty node lists for testing
        );

        assert!(result.is_valid());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_query_result_invalid() {
        let result = SdrShaderNodeQueryResult::with_data(
            vec![Token::new("a"), Token::new("b")],
            vec![
                vec![Value::from("val1")], // Wrong number of columns
            ],
            vec![vec![]],
        );

        assert!(!result.is_valid());
    }

    #[test]
    fn test_query_result_no_select_distinct_openusd_shape() {
        // OpenUSD: no SelectDistinct => GetShaderNodesByValues is empty, GetAllShaderNodes has matches.
        use crate::declare::SdrVersion;
        use crate::shader_node::SdrShaderNode;
        use crate::shader_node_metadata::SdrShaderNodeMetadata;

        let node = std::sync::Arc::new(SdrShaderNode::new(
            Token::new("TestNode"),
            SdrVersion::new(1, 0),
            String::new(),
            Token::default(),
            Token::default(),
            Token::new("OSL"),
            String::new(),
            String::new(),
            Vec::new(),
            SdrShaderNodeMetadata::new(),
            String::new(),
        ));
        let result = SdrShaderNodeQueryResult::with_no_select_data(vec![node.clone()]);

        assert!(result.is_valid());
        assert!(result.get_keys().is_empty());
        assert!(result.get_values().is_empty());
        assert!(result.get_shader_nodes_by_values().is_empty());
        assert_eq!(result.get_all_shader_nodes().len(), 1);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 0);
    }
}
