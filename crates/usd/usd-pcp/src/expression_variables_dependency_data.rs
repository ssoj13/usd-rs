//! Expression Variables Dependency Data.
//!
//! Port of pxr/usd/pcp/expressionVariablesDependencyData.h
//!
//! Captures the expression variables used by an associated prim index
//! during composition.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::LayerStackRefPtr;

/// Captures the expression variables used by an associated prim index
/// during composition.
///
/// Matches C++ `PcpExpressionVariablesDependencyData`.
#[derive(Debug)]
pub struct ExpressionVariablesDependencyData {
    /// Internal data (None if empty).
    data: Option<Box<ExpressionVariablesData>>,
}

#[derive(Debug)]
struct ExpressionVariablesData {
    /// Map from layer stack to expression variable names.
    layer_stack_to_expression_vars: HashMap<LayerStackRefPtr, HashSet<String>>,
}

impl ExpressionVariablesDependencyData {
    /// Creates a new empty dependency data.
    ///
    /// Matches C++ default constructor.
    pub fn new() -> Self {
        Self { data: None }
    }

    /// Returns true if any dependencies have been recorded, false otherwise.
    ///
    /// Matches C++ `IsEmpty()` method.
    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    /// Moves dependencies in `other` and appends it to the dependencies
    /// in this object.
    ///
    /// Matches C++ `AppendDependencyData()` method.
    pub fn append_dependency_data(&mut self, mut other: Self) {
        if other.data.is_none() {
            return;
        }

        if let Some(other_data) = other.data.take() {
            for (layer_stack, expr_vars) in other_data.layer_stack_to_expression_vars {
                self.add_dependencies(layer_stack, expr_vars);
            }
        }
    }

    /// Adds dependencies on the expression variables in `expr_var_dependencies`
    /// from `layer_stack`.
    ///
    /// Matches C++ `AddDependencies()` method.
    pub fn add_dependencies(
        &mut self,
        layer_stack: LayerStackRefPtr,
        expr_var_dependencies: HashSet<String>,
    ) {
        if expr_var_dependencies.is_empty() {
            return;
        }

        // Create the data now if it was empty before this call.
        if self.data.is_none() {
            self.data = Some(Box::new(ExpressionVariablesData {
                layer_stack_to_expression_vars: HashMap::new(),
            }));
        }

        if let Some(ref mut data) = self.data {
            let stored_deps = data
                .layer_stack_to_expression_vars
                .entry(layer_stack)
                .or_insert_with(HashSet::new);

            if stored_deps.is_empty() {
                *stored_deps = expr_var_dependencies;
            } else {
                stored_deps.extend(expr_var_dependencies);
            }
        }
    }

    /// Runs the given callback on all of the dependencies in this object.
    ///
    /// Matches C++ `ForEachDependency()` template method.
    pub fn for_each_dependency<F>(&self, callback: F)
    where
        F: Fn(&LayerStackRefPtr, &HashSet<String>),
    {
        if let Some(ref data) = self.data {
            for (layer_stack, expr_vars) in &data.layer_stack_to_expression_vars {
                callback(layer_stack, expr_vars);
            }
        }
    }

    /// Returns the expression variable dependencies associated with
    /// `layer_stack`. If no such dependencies have been added, returns `None`.
    ///
    /// Matches C++ `GetDependenciesForLayerStack()` method.
    pub fn get_dependencies_for_layer_stack(
        &self,
        layer_stack: &LayerStackRefPtr,
    ) -> Option<&HashSet<String>> {
        self.data
            .as_ref()
            .and_then(|d| d.layer_stack_to_expression_vars.get(layer_stack))
    }

    // Compatibility methods for existing code
    /// Returns true if there are any dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.is_empty()
    }
}

impl Default for ExpressionVariablesDependencyData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let data = ExpressionVariablesDependencyData::new();
        assert!(data.is_empty());
    }

    #[test]
    fn test_add_dependencies() {
        let mut data = ExpressionVariablesDependencyData::new();
        let layer_stack = LayerStackRefPtr::default();
        let mut expr_vars = HashSet::new();
        expr_vars.insert("VAR1".to_string());
        expr_vars.insert("VAR2".to_string());

        data.add_dependencies(layer_stack.clone(), expr_vars);

        assert!(!data.is_empty());
        let deps = data.get_dependencies_for_layer_stack(&layer_stack);
        assert!(deps.is_some());
        assert_eq!(deps.unwrap().len(), 2);
    }

    #[test]
    fn test_append_dependency_data() {
        let mut data1 = ExpressionVariablesDependencyData::new();
        let layer_stack1 = LayerStackRefPtr::default();
        let mut expr_vars1 = HashSet::new();
        expr_vars1.insert("VAR1".to_string());
        data1.add_dependencies(layer_stack1, expr_vars1);

        let mut data2 = ExpressionVariablesDependencyData::new();
        let layer_stack2 = LayerStackRefPtr::default();
        let mut expr_vars2 = HashSet::new();
        expr_vars2.insert("VAR2".to_string());
        data2.add_dependencies(layer_stack2, expr_vars2);

        data1.append_dependency_data(data2);

        // After appending, data1 should have dependencies from both
        assert!(!data1.is_empty());
    }
}
