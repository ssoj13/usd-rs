//! HdExtComputationContext - Execution environment for ext computations.
//!
//! Port of pxr/imaging/hd/extComputationContext.h
//!
//! Interface that defines the execution environment for running a computation:
//! get input values, set output values, report errors.

use usd_tf::Token;
use usd_vt::Value;

/// Interface for the execution environment of an ext computation.
///
/// The computation receives inputs via `get_input_value` / `get_optional_input_value`,
/// writes outputs via `set_output_value`, and signals failures via `raise_computation_error`.
///
/// Matches C++ `HdExtComputationContext`.
pub trait HdExtComputationContext: Send {
    /// Obtains the value of a named input to the computation.
    ///
    /// Issues a coding error and returns a default value if the input is missing.
    fn get_input_value(&self, name: &Token) -> Value;

    /// Obtains the value of a named input if present.
    ///
    /// Returns None if the input isn't present.
    fn get_optional_input_value(&self, name: &Token) -> Option<&Value>;

    /// Sets the value of the specified output.
    fn set_output_value(&mut self, name: &Token, output: Value);

    /// Signals that an error occurred and output values are invalid.
    fn raise_computation_error(&mut self);
}
