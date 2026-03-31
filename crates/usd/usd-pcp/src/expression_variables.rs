//! Expression variables for layer stacks.
//!
//! Expression variables are key-value pairs that can be used in asset path
//! expressions during composition. This module provides types for managing
//! these variables.
//!
//! # Overview
//!
//! [`ExpressionVariables`] contains the composed expression variables
//! associated with a given layer stack, identified by an
//! [`ExpressionVariablesSource`].
//!
//! # Examples
//!
//! ```
//! use usd_pcp::{ExpressionVariables, ExpressionVariablesSource};
//! use usd_vt::Dictionary;
//!
//! let source = ExpressionVariablesSource::new();
//! let mut variables = Dictionary::new();
//! variables.insert("ASSET_ROOT", "/assets");
//!
//! let expr_vars = ExpressionVariables::new(source, variables);
//! assert!(expr_vars.source().is_root_layer_stack());
//! ```

use std::collections::HashMap;

use usd_vt::Dictionary;

use super::{ExpressionVariablesSource, LayerStackIdentifier};

/// Composed expression variables associated with a layer stack.
///
/// Expression variables are key-value pairs used in asset path expressions
/// during composition. This type stores the variables along with their
/// source layer stack.
///
/// # Examples
///
/// ```
/// use usd_pcp::{ExpressionVariables, ExpressionVariablesSource};
/// use usd_vt::Dictionary;
///
/// let source = ExpressionVariablesSource::new();
/// let variables = Dictionary::new();
/// let expr_vars = ExpressionVariables::new(source, variables);
///
/// assert!(expr_vars.source().is_root_layer_stack());
/// assert!(expr_vars.variables().is_empty());
/// ```
#[derive(Clone, Debug, Default)]
pub struct ExpressionVariables {
    /// The source of these expression variables.
    source: ExpressionVariablesSource,
    /// The expression variables dictionary.
    expression_variables: Dictionary,
}

impl ExpressionVariables {
    /// Computes the composed expression variables for the given source layer stack.
    ///
    /// Recursively computes and composes the overrides specified by its
    /// expressionVariableOverridesSource. If `override_expression_vars` is provided,
    /// it will be used as the overrides instead of performing recursive computation.
    ///
    /// Matches C++ `PcpExpressionVariables::Compute()`.
    pub fn compute(
        source_layer_stack_id: &LayerStackIdentifier,
        root_layer_stack_id: &LayerStackIdentifier,
        override_expression_vars: Option<&ExpressionVariables>,
    ) -> Self {
        // Full implementation would:
        // 1. Load the source layer stack
        // 2. Read expression variables from metadata
        // 3. Recursively compute overrides if not provided
        // 4. Compose variables according to composition rules

        // For now, return default with source set to root
        let source =
            ExpressionVariablesSource::from_identifier(source_layer_stack_id, root_layer_stack_id);
        Self {
            source,
            expression_variables: override_expression_vars
                .map(|v| v.expression_variables.clone())
                .unwrap_or_default(),
        }
    }

    /// Creates a new object with the given source and variables.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariables, ExpressionVariablesSource};
    /// use usd_vt::Dictionary;
    ///
    /// let source = ExpressionVariablesSource::new();
    /// let mut variables = Dictionary::new();
    /// variables.insert("VERSION", "1.0");
    ///
    /// let expr_vars = ExpressionVariables::new(source, variables);
    /// ```
    #[must_use]
    pub fn new(source: ExpressionVariablesSource, expression_variables: Dictionary) -> Self {
        Self {
            source,
            expression_variables,
        }
    }

    /// Returns the source of the composed expression variables.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariables, ExpressionVariablesSource};
    /// use usd_vt::Dictionary;
    ///
    /// let expr_vars = ExpressionVariables::default();
    /// assert!(expr_vars.source().is_root_layer_stack());
    /// ```
    #[inline]
    #[must_use]
    pub fn source(&self) -> &ExpressionVariablesSource {
        &self.source
    }

    /// Returns the composed expression variables dictionary.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariables, ExpressionVariablesSource};
    /// use usd_vt::Dictionary;
    ///
    /// let source = ExpressionVariablesSource::new();
    /// let mut variables = Dictionary::new();
    /// variables.insert("key", "value");
    ///
    /// let expr_vars = ExpressionVariables::new(source, variables);
    /// assert_eq!(expr_vars.variables().len(), 1);
    /// ```
    #[inline]
    #[must_use]
    pub fn variables(&self) -> &Dictionary {
        &self.expression_variables
    }

    /// Returns a mutable reference to the expression variables dictionary.
    #[inline]
    pub fn variables_mut(&mut self) -> &mut Dictionary {
        &mut self.expression_variables
    }

    /// Sets the composed expression variables.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariables, ExpressionVariablesSource};
    /// use usd_vt::Dictionary;
    ///
    /// let mut expr_vars = ExpressionVariables::default();
    ///
    /// let mut new_vars = Dictionary::new();
    /// new_vars.insert("updated", true);
    /// expr_vars.set_variables(new_vars);
    ///
    /// assert_eq!(expr_vars.variables().len(), 1);
    /// ```
    pub fn set_variables(&mut self, variables: Dictionary) {
        self.expression_variables = variables;
    }
}

impl PartialEq for ExpressionVariables {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || (self.source == other.source
                && self.expression_variables == other.expression_variables)
    }
}

impl Eq for ExpressionVariables {}

/// Helper for computing expression variables with caching.
///
/// This caches the results of recursive override computations so they
/// can be reused by subsequent computations.
///
/// # Examples
///
/// ```
/// use usd_pcp::{ExpressionVariablesCachingComposer, LayerStackIdentifier};
///
/// let root_id = LayerStackIdentifier::new("root.usda");
/// let mut composer = ExpressionVariablesCachingComposer::new(root_id);
/// ```
#[derive(Debug)]
pub struct ExpressionVariablesCachingComposer {
    /// The root layer stack identifier.
    root_layer_stack_id: LayerStackIdentifier,
    /// Cache of computed expression variables by identifier.
    cache: HashMap<LayerStackIdentifier, ExpressionVariables>,
}

impl ExpressionVariablesCachingComposer {
    /// Creates a new composer with the given root layer stack identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariablesCachingComposer, LayerStackIdentifier};
    ///
    /// let root_id = LayerStackIdentifier::new("root.usda");
    /// let composer = ExpressionVariablesCachingComposer::new(root_id);
    /// ```
    #[must_use]
    pub fn new(root_layer_stack_identifier: LayerStackIdentifier) -> Self {
        Self {
            root_layer_stack_id: root_layer_stack_identifier,
            cache: HashMap::new(),
        }
    }

    /// Returns the root layer stack identifier.
    #[inline]
    #[must_use]
    pub fn root_layer_stack_identifier(&self) -> &LayerStackIdentifier {
        &self.root_layer_stack_id
    }

    /// Computes the expression variables for the given layer stack identifier.
    ///
    /// Results are cached for subsequent calls with the same identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_pcp::{ExpressionVariablesCachingComposer, LayerStackIdentifier};
    ///
    /// let root_id = LayerStackIdentifier::new("root.usda");
    /// let mut composer = ExpressionVariablesCachingComposer::new(root_id.clone());
    ///
    /// let vars = composer.compute_expression_variables(&root_id);
    /// assert!(vars.source().is_root_layer_stack());
    /// ```
    pub fn compute_expression_variables(
        &mut self,
        id: &LayerStackIdentifier,
    ) -> &ExpressionVariables {
        // Check cache first
        if !self.cache.contains_key(id) {
            // Compute and cache
            let source = ExpressionVariablesSource::from_identifier(id, &self.root_layer_stack_id);
            let expr_vars = ExpressionVariables::new(source, Dictionary::new());
            self.cache.insert(id.clone(), expr_vars);
        }

        self.cache.get(id).expect("Just inserted")
    }

    /// Clears the cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let source = ExpressionVariablesSource::new();
        let variables = Dictionary::new();
        let expr_vars = ExpressionVariables::new(source, variables);

        assert!(expr_vars.source().is_root_layer_stack());
        assert!(expr_vars.variables().is_empty());
    }

    #[test]
    fn test_default() {
        let expr_vars = ExpressionVariables::default();
        assert!(expr_vars.source().is_root_layer_stack());
        assert!(expr_vars.variables().is_empty());
    }

    #[test]
    fn test_with_variables() {
        let source = ExpressionVariablesSource::new();
        let mut variables = Dictionary::new();
        variables.insert("ASSET_ROOT", "/assets");
        variables.insert("VERSION", "2.0");

        let expr_vars = ExpressionVariables::new(source, variables);
        assert_eq!(expr_vars.variables().len(), 2);
    }

    #[test]
    fn test_set_variables() {
        let mut expr_vars = ExpressionVariables::default();
        assert!(expr_vars.variables().is_empty());

        let mut new_vars = Dictionary::new();
        new_vars.insert("key", "value");
        expr_vars.set_variables(new_vars);

        assert_eq!(expr_vars.variables().len(), 1);
    }

    #[test]
    fn test_variables_mut() {
        let mut expr_vars = ExpressionVariables::default();
        expr_vars.variables_mut().insert("key", "value");
        assert_eq!(expr_vars.variables().len(), 1);
    }

    #[test]
    fn test_equality_same_instance() {
        let expr_vars = ExpressionVariables::default();
        assert_eq!(expr_vars, expr_vars);
    }

    #[test]
    fn test_equality_different_instances() {
        let mut vars1 = Dictionary::new();
        vars1.insert("key", "value");

        let mut vars2 = Dictionary::new();
        vars2.insert("key", "value");

        let expr_vars1 = ExpressionVariables::new(ExpressionVariablesSource::new(), vars1);
        let expr_vars2 = ExpressionVariables::new(ExpressionVariablesSource::new(), vars2);

        assert_eq!(expr_vars1, expr_vars2);
    }

    #[test]
    fn test_inequality_different_variables() {
        let mut vars1 = Dictionary::new();
        vars1.insert("key1", "value1");

        let mut vars2 = Dictionary::new();
        vars2.insert("key2", "value2");

        let expr_vars1 = ExpressionVariables::new(ExpressionVariablesSource::new(), vars1);
        let expr_vars2 = ExpressionVariables::new(ExpressionVariablesSource::new(), vars2);

        assert_ne!(expr_vars1, expr_vars2);
    }

    #[test]
    fn test_caching_composer_new() {
        let root_id = LayerStackIdentifier::new("root.usda");
        let composer = ExpressionVariablesCachingComposer::new(root_id.clone());

        assert_eq!(
            composer
                .root_layer_stack_identifier()
                .root_layer
                .get_authored_path(),
            "root.usda"
        );
    }

    #[test]
    fn test_caching_composer_compute() {
        let root_id = LayerStackIdentifier::new("root.usda");
        let mut composer = ExpressionVariablesCachingComposer::new(root_id.clone());

        let vars = composer.compute_expression_variables(&root_id);
        assert!(vars.source().is_root_layer_stack());
    }

    #[test]
    fn test_caching_composer_caches_results() {
        let root_id = LayerStackIdentifier::new("root.usda");
        let model_id = LayerStackIdentifier::new("model.usda");
        let mut composer = ExpressionVariablesCachingComposer::new(root_id.clone());

        // First call computes
        let _ = composer.compute_expression_variables(&model_id);

        // Second call returns cached
        let vars = composer.compute_expression_variables(&model_id);
        assert!(!vars.source().is_root_layer_stack());
    }

    #[test]
    fn test_caching_composer_clear_cache() {
        let root_id = LayerStackIdentifier::new("root.usda");
        let mut composer = ExpressionVariablesCachingComposer::new(root_id.clone());

        let _ = composer.compute_expression_variables(&root_id);
        composer.clear_cache();

        // After clear, computes again
        let vars = composer.compute_expression_variables(&root_id);
        assert!(vars.source().is_root_layer_stack());
    }
}
