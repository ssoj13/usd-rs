//! Predicate program - compiled predicate expressions.
//!
//! Port of pxr/usd/sdf/predicateProgram.h
//!
//! Represents a callable "program", the result of linking an
//! SdfPredicateExpression with an SdfPredicateLibrary via
//! link_predicate_expression(). The main public interface is the
//! `evaluate()` method accepting a single argument of the domain type.
//!
//! Note: The primary PredicateProgram implementation lives in
//! predicate_library.rs. This module provides an alternative standalone
//! implementation that can be used independently.

use crate::predicate_expression::{FnCall, PredicateExpression, PredicateOp};
use crate::predicate_library::{PredicateFunction, PredicateFunctionResult, PredicateLibrary};

/// Operations in a compiled predicate program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// Call a bound predicate function.
    Call,
    /// Logical NOT (postfix, RPN-style).
    Not,
    /// Open a group (for short-circuit evaluation).
    Open,
    /// Close a group.
    Close,
    /// Logical AND (infix).
    And,
    /// Logical OR (infix).
    Or,
}

/// A compiled predicate program.
///
/// Created by linking a `PredicateExpression` with a `PredicateLibrary`.
/// Call `evaluate()` to run the program against a domain object.
pub struct PredicateProgram<D: 'static> {
    /// The sequence of operations to execute.
    ops: Vec<Op>,
    /// Bound predicate functions, called in order by `Call` ops.
    funcs: Vec<PredicateFunction<D>>,
}

impl<D: 'static> Clone for PredicateProgram<D> {
    fn clone(&self) -> Self {
        Self {
            ops: self.ops.clone(),
            funcs: self.funcs.clone(),
        }
    }
}

impl<D: 'static> Default for PredicateProgram<D> {
    fn default() -> Self {
        Self {
            ops: Vec::new(),
            funcs: Vec::new(),
        }
    }
}

impl<D: 'static> PredicateProgram<D> {
    /// Returns true if this program has any operations (is non-empty).
    pub fn is_valid(&self) -> bool {
        !self.ops.is_empty()
    }

    /// Evaluates the predicate program on the given object.
    ///
    /// Uses short-circuit evaluation for And/Or operators.
    pub fn evaluate(&self, obj: &D) -> PredicateFunctionResult {
        let mut result = PredicateFunctionResult::make_constant(false);
        let mut nest = 0i32;
        let mut func_idx = 0usize;
        let ops = &self.ops;
        let mut i = 0usize;

        while i < ops.len() {
            match ops[i] {
                Op::Call => {
                    if func_idx < self.funcs.len() {
                        let call_result = (self.funcs[func_idx])(obj);
                        result.set_and_propagate_constancy(call_result);
                        func_idx += 1;
                    }
                }
                Op::Not => {
                    result = result.not();
                }
                Op::And | Op::Or => {
                    let deciding_value = ops[i] != Op::And;
                    if result.value == deciding_value {
                        // Short-circuit: skip to matching Close.
                        let orig_nest = nest;
                        i += 1;
                        while i < ops.len() {
                            match ops[i] {
                                Op::Call => {
                                    func_idx += 1;
                                }
                                Op::Open => nest += 1,
                                Op::Close => {
                                    nest -= 1;
                                    if nest == orig_nest {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            i += 1;
                        }
                    }
                }
                Op::Open => nest += 1,
                Op::Close => nest -= 1,
            }
            i += 1;
        }

        result
    }
}

/// Links a predicate expression with a predicate library, producing
/// a callable program.
///
/// Returns an empty program on failure (with errors logged to stderr).
pub fn link<D: Send + Sync + 'static>(
    expr: &PredicateExpression,
    lib: &PredicateLibrary<D>,
) -> PredicateProgram<D> {
    let mut prog = PredicateProgram::default();

    // Use the expression's walk method to traverse and compile.
    // The walk callback signature: FnMut(PredicateOp, i32) and FnMut(&FnCall).
    let _calls = expr.calls().to_vec();

    // We need interior mutability since walk takes FnMut closures.
    let ops = std::cell::RefCell::new(Vec::new());
    let funcs = std::cell::RefCell::new(Vec::<PredicateFunction<D>>::new());
    let errs = std::cell::RefCell::new(Vec::<String>::new());

    expr.walk(
        |op: PredicateOp, arg_index: i32| {
            match op {
                PredicateOp::Not => {
                    if arg_index == 1 {
                        ops.borrow_mut().push(Op::Not);
                    }
                }
                PredicateOp::And | PredicateOp::ImpliedAnd | PredicateOp::Or => {
                    if arg_index == 1 {
                        let prog_op = if op == PredicateOp::Or {
                            Op::Or
                        } else {
                            Op::And
                        };
                        ops.borrow_mut().push(prog_op);
                        ops.borrow_mut().push(Op::Open);
                    } else if arg_index == 2 {
                        ops.borrow_mut().push(Op::Close);
                    }
                }
                PredicateOp::Call => {
                    // Handled in call callback.
                }
            }
        },
        |call: &FnCall| {
            if let Some(func) = lib.bind_call(&call.func_name, &call.args) {
                funcs.borrow_mut().push(func);
                ops.borrow_mut().push(Op::Call);
            } else {
                errs.borrow_mut()
                    .push(format!("Failed to bind call of {}", call.func_name));
            }
        },
    );

    prog.ops = ops.into_inner();
    prog.funcs = funcs.into_inner();
    let link_errors = errs.into_inner();

    if !link_errors.is_empty() {
        eprintln!("Predicate linking errors: {}", link_errors.join(", "));
        return PredicateProgram::default();
    }

    prog
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_empty_program() {
        let prog: PredicateProgram<i32> = PredicateProgram::default();
        assert!(!prog.is_valid());
    }

    #[test]
    fn test_single_call_program() {
        let mut prog: PredicateProgram<i32> = PredicateProgram::default();
        let func: PredicateFunction<i32> =
            Arc::new(|val: &i32| PredicateFunctionResult::make_constant(*val > 0));
        prog.funcs.push(func);
        prog.ops.push(Op::Call);

        assert!(prog.is_valid());
        assert!(prog.evaluate(&5).value);
        assert!(!prog.evaluate(&-1).value);
    }
}
