//! Boolean expression evaluation utilities.
//!
//! Port of pxr/usd/sdf/booleanExpressionEval.h
//!
//! Convenience functions for evaluating a BooleanExpression with a
//! variable callback. The core evaluation logic lives in
//! `BooleanExpression::evaluate()`, but this module provides the
//! standalone function matching the C++ API.

use crate::boolean_expression::BooleanExpression;
use usd_tf::Token;
use usd_vt::Value;

/// Evaluates the provided expression using the provided callback to resolve
/// any variables encountered along the way.
///
/// This is a convenience wrapper around `BooleanExpression::evaluate()`.
///
/// # Arguments
/// * `expression` - The boolean expression to evaluate.
/// * `variable_callback` - Called with each variable name; should return
///   the variable's value.
pub fn eval_boolean_expression<F>(expression: &BooleanExpression, variable_callback: F) -> bool
where
    F: Fn(&Token) -> Value,
{
    expression.evaluate(variable_callback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_simple() {
        let expr = BooleanExpression::make_constant(Value::new(true));
        assert!(eval_boolean_expression(&expr, |_| Value::new(false)));
    }

    #[test]
    fn test_eval_variable() {
        let expr = BooleanExpression::make_variable(Token::from("x"));
        assert!(eval_boolean_expression(&expr, |name| {
            if name == "x" {
                Value::new(true)
            } else {
                Value::new(false)
            }
        }));
    }
}
