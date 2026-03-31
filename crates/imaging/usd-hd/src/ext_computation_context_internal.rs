//! HdExtComputationContextInternal - Concrete execution context for ext computations.
//!
//! Port of pxr/imaging/hd/extComputationContextInternal.h
//!
//! Provides HashMap-backed storage for inputs/outputs during computation execution.

use super::ext_computation_context::HdExtComputationContext;
use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::Value;

/// Concrete implementation of HdExtComputationContext.
///
/// Stores input and output values in HashMaps. Used when executing
/// CPU ext computations (e.g., in HdsiExtComputationPrimvarPruningSceneIndex).
#[derive(Default)]
pub struct HdExtComputationContextInternal {
    inputs: HashMap<Token, Value>,
    outputs: HashMap<Token, Value>,
    computation_error: bool,
}

impl HdExtComputationContextInternal {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets an input value. Does not replace if already present (matches C++).
    pub fn set_input_value(&mut self, name: Token, value: Value) {
        self.inputs.entry(name).or_insert(value);
    }

    /// Gets an output value. Returns false if not present.
    pub fn get_output_value(&self, name: &Token) -> Option<Value> {
        self.outputs.get(name).cloned()
    }

    /// Returns true if a computation error occurred.
    pub fn has_computation_error(&self) -> bool {
        self.computation_error
    }
}

impl HdExtComputationContext for HdExtComputationContextInternal {
    fn get_input_value(&self, name: &Token) -> Value {
        self.inputs
            .get(name)
            .cloned()
            .unwrap_or_else(Value::default)
    }

    fn get_optional_input_value(&self, name: &Token) -> Option<&Value> {
        self.inputs.get(name)
    }

    fn set_output_value(&mut self, name: &Token, output: Value) {
        self.outputs.insert(name.clone(), output);
    }

    fn raise_computation_error(&mut self) {
        self.computation_error = true;
    }
}
