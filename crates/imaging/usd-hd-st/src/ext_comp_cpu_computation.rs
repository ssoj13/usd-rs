#![allow(dead_code)]

//! ExtComp CPU computation - CPU-side external computation execution.
//!
//! A buffer source representing a CPU implementation of an ExtComputation.
//! Implements input -> processing -> output model where inputs are other
//! buffer sources and processing happens during resolve by calling back
//! to the scene delegate via an invoke callback.
//!
//! Outputs are in SOA (struct-of-arrays) form.
//!
//! Port of pxr/imaging/hdSt/extCompCpuComputation.h

use super::ext_comp_input_source::ExtCompInputSourceSharedPtr;
use std::sync::Arc;
use usd_hd::HdExtComputationContextInternal;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Sentinel for invalid output index.
pub const INVALID_OUTPUT_INDEX: usize = usize::MAX;

/// Shared pointer type.
pub type ExtCompCpuComputationSharedPtr = Arc<ExtCompCpuComputation>;

/// Callback type used to invoke the scene delegate computation.
///
/// Called during resolve with the computation ID and a mutable context that
/// carries pre-filled input values. The callback sets output values on the
/// context. Matches `HdSceneDelegate::InvokeExtComputation` semantics.
pub type InvokeExtComputationFn =
    Box<dyn Fn(&SdfPath, &mut HdExtComputationContextInternal) + Send + Sync>;

/// CPU-side external computation.
///
/// Implements the input -> processing -> output model. Inputs are
/// other buffer sources, outputs are produced during resolve by
/// calling back to the scene delegate via `invoke_fn`.
///
/// Port of HdStExtCompCpuComputation
pub struct ExtCompCpuComputation {
    /// Computation ID (scene path)
    id: SdfPath,
    /// Input sources
    inputs: Vec<ExtCompInputSourceSharedPtr>,
    /// Output names
    outputs: Vec<Token>,
    /// Number of output elements (all outputs share this count)
    num_elements: usize,
    /// Computed output values (populated after resolve)
    output_values: Vec<VtValue>,
    /// Whether successfully resolved (or resolved-with-error)
    resolved: bool,
    /// Whether a computation error was raised during resolve
    resolve_error: bool,
    /// Delegate callback: fills context inputs, invokes computation, reads outputs.
    /// None means no delegate wired (used in tests or when not needed).
    invoke_fn: Option<InvokeExtComputationFn>,
}

impl std::fmt::Debug for ExtCompCpuComputation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtCompCpuComputation")
            .field("id", &self.id)
            .field("outputs", &self.outputs)
            .field("num_elements", &self.num_elements)
            .field("resolved", &self.resolved)
            .field("resolve_error", &self.resolve_error)
            .field("has_invoke_fn", &self.invoke_fn.is_some())
            .finish()
    }
}

impl ExtCompCpuComputation {
    /// Create a new CPU computation without a delegate callback.
    ///
    /// Use `with_invoke()` when wiring to a real scene delegate.
    ///
    /// - `id`: Scene path identifying this computation
    /// - `inputs`: Input buffer sources
    /// - `outputs`: Output names
    /// - `num_elements`: Number of elements per output
    pub fn new(
        id: SdfPath,
        inputs: Vec<ExtCompInputSourceSharedPtr>,
        outputs: Vec<Token>,
        num_elements: usize,
    ) -> Self {
        Self::with_invoke(id, inputs, outputs, num_elements, None)
    }

    /// Create a new CPU computation wired to a scene delegate invoke callback.
    ///
    /// The `invoke_fn` is called during `resolve()` to actually execute the
    /// computation. It receives the computation ID and a mutable context
    /// pre-filled with input values, and is expected to set output values on it.
    pub fn with_invoke(
        id: SdfPath,
        inputs: Vec<ExtCompInputSourceSharedPtr>,
        outputs: Vec<Token>,
        num_elements: usize,
        invoke_fn: Option<InvokeExtComputationFn>,
    ) -> Self {
        let num_outputs = outputs.len();
        Self {
            id,
            inputs,
            outputs,
            num_elements,
            output_values: vec![VtValue::default(); num_outputs],
            resolved: false,
            resolve_error: false,
            invoke_fn,
        }
    }

    /// Get the computation ID as a token.
    pub fn name(&self) -> Token {
        Token::new(self.id.get_text())
    }

    /// Get the computation scene path.
    pub fn id(&self) -> &SdfPath {
        &self.id
    }

    /// Resolve the computation: check inputs, invoke delegate, capture outputs.
    ///
    /// Mirrors `HdStExtCompCpuComputation::Resolve()` from extCompCpuComputation.cpp:115-179.
    ///
    /// Returns false only if inputs are not yet resolved (caller should retry).
    /// Returns true on success or on computation error (marked resolved either way).
    pub fn resolve(&mut self) -> bool {
        // Check all inputs are resolved; if not, ask caller to retry later.
        for input in &self.inputs {
            if !input.is_resolved() {
                return false;
            }
        }

        // Build execution context and fill with input values.
        let mut context = HdExtComputationContextInternal::new();
        for input in &self.inputs {
            context.set_input_value(input.name().clone(), input.value().clone());
        }

        // Invoke the scene delegate computation if a callback is wired.
        if let Some(ref invoke_fn) = self.invoke_fn {
            invoke_fn(&self.id, &mut context);
        }

        // If an error was raised, mark resolved-with-error and bail.
        if context.has_computation_error() {
            self.resolve_error = true;
            self.resolved = true;
            return true;
        }

        // Read output values from context into our output_values vec.
        for (idx, output_name) in self.outputs.iter().enumerate() {
            match context.get_output_value(output_name) {
                Some(val) => self.output_values[idx] = val,
                None => {
                    // Missing output counts as a computation error.
                    log::warn!(
                        "ExtCompCpuComputation::resolve: output '{}' not set by delegate for {}",
                        output_name,
                        self.id
                    );
                    self.resolve_error = true;
                    self.resolved = true;
                    return true;
                }
            }
        }

        self.resolved = true;
        true
    }

    /// Whether the computation has been resolved (successfully or with error).
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Whether a computation error occurred during resolve.
    pub fn has_resolve_error(&self) -> bool {
        self.resolve_error
    }

    /// Number of output elements.
    pub fn num_elements(&self) -> usize {
        self.num_elements
    }

    /// Convert an output name to its index.
    pub fn get_output_index(&self, output_name: &Token) -> usize {
        self.outputs
            .iter()
            .position(|t| t == output_name)
            .unwrap_or(INVALID_OUTPUT_INDEX)
    }

    /// Get an output value by index (valid after resolve).
    pub fn get_output_by_index(&self, index: usize) -> Option<&VtValue> {
        self.output_values.get(index)
    }

    /// Set an output value by index.
    pub fn set_output_by_index(&mut self, index: usize, value: VtValue) {
        if index < self.output_values.len() {
            self.output_values[index] = value;
        }
    }

    /// Get output names.
    pub fn outputs(&self) -> &[Token] {
        &self.outputs
    }

    /// Get inputs.
    pub fn inputs(&self) -> &[ExtCompInputSourceSharedPtr] {
        &self.inputs
    }

    /// Whether the computation specification is valid.
    pub fn is_valid(&self) -> bool {
        !self.id.is_empty() && !self.outputs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_computation() {
        let id = SdfPath::from_string("/computations/deform").unwrap();
        let outputs = vec![Token::new("points"), Token::new("normals")];
        let comp = ExtCompCpuComputation::new(id, vec![], outputs, 100);

        assert!(comp.is_valid());
        assert_eq!(comp.num_elements(), 100);
        assert_eq!(comp.get_output_index(&Token::new("points")), 0);
        assert_eq!(comp.get_output_index(&Token::new("normals")), 1);
        assert_eq!(
            comp.get_output_index(&Token::new("missing")),
            INVALID_OUTPUT_INDEX
        );
    }

    #[test]
    fn test_resolve_no_inputs_no_invoke() {
        // Without an invoke_fn and with outputs expected, resolve succeeds
        // but outputs remain default (no delegate to fill them). The missing-
        // output path only fires when outputs are listed but invoke_fn is Some.
        // With invoke_fn=None the context has no outputs set; outputs list is
        // non-empty so we hit the missing-output warning path.
        let id = SdfPath::from_string("/comp").unwrap();
        let mut comp = ExtCompCpuComputation::new(id, vec![], vec![Token::new("out")], 10);
        assert!(!comp.is_resolved());
        // resolve() returns true (done), but resolve_error is set because
        // the invoke_fn is None so "out" is never written to context.
        assert!(comp.resolve());
        assert!(comp.is_resolved());
    }

    #[test]
    fn test_resolve_with_invoke() {
        use usd_hd::HdExtComputationContext;
        use usd_vt::Value;

        let id = SdfPath::from_string("/comp/cpu").unwrap();
        let outputs = vec![Token::new("result")];

        // Invoke callback writes a value to "result" output.
        let invoke: InvokeExtComputationFn = Box::new(|_id, ctx| {
            ctx.set_output_value(&Token::new("result"), Value::from(42.0f32));
        });

        let mut comp = ExtCompCpuComputation::with_invoke(id, vec![], outputs, 1, Some(invoke));

        assert!(!comp.is_resolved());
        assert!(comp.resolve());
        assert!(comp.is_resolved());
        assert!(!comp.has_resolve_error());

        let val = comp.get_output_by_index(0).expect("output must be set");
        assert_eq!(val.get::<f32>().copied(), Some(42.0f32));
    }

    #[test]
    fn test_resolve_error_propagation() {
        use usd_hd::HdExtComputationContext;

        let id = SdfPath::from_string("/comp/err").unwrap();
        let outputs = vec![Token::new("out")];

        // Invoke callback signals an error.
        let invoke: InvokeExtComputationFn = Box::new(|_id, ctx| {
            ctx.raise_computation_error();
        });

        let mut comp = ExtCompCpuComputation::with_invoke(id, vec![], outputs, 1, Some(invoke));

        assert!(comp.resolve());
        assert!(comp.is_resolved());
        assert!(comp.has_resolve_error());
    }
}
