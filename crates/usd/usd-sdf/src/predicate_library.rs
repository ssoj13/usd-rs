//! SdfPredicateLibrary - library of predicate functions.
//!
//! Port of pxr/usd/sdf/predicateLibrary.h and predicateProgram.h
//!
//! Represents a library of predicate functions for use with
//! SdfPredicateExpression. Call link_predicate_expression() with an
//! expression and a library to produce a callable PredicateProgram.
//!
//! PredicateProgram uses a flat RPN-style operation list with
//! short-circuiting, matching the C++ SdfPredicateProgram architecture.

use crate::{FnArg, PredicateExpression};
use std::collections::HashMap;
use std::sync::Arc;
use usd_vt::Value;

// ============================================================================
// FromValue — typed extraction from Value (mirrors C++ VtValue::Cast<T>)
// ============================================================================

/// Trait for extracting typed values from `Value`.
///
/// Mirrors the implicit `VtValue::Cast<ParamType>` used in C++
/// `_TryToBindCall` for argument binding.
pub trait FromValue: Sized + 'static {
    /// Attempt to extract a value of this type from a `Value`.
    fn from_value(value: &Value) -> Option<Self>;
}

impl FromValue for bool {
    fn from_value(value: &Value) -> Option<Self> {
        value.get::<bool>().copied()
    }
}

impl FromValue for i32 {
    fn from_value(value: &Value) -> Option<Self> {
        if let Some(&v) = value.get::<i32>() {
            return Some(v);
        }
        if let Some(&v) = value.get::<i64>() {
            return Some(v as i32);
        }
        None
    }
}

impl FromValue for i64 {
    fn from_value(value: &Value) -> Option<Self> {
        if let Some(&v) = value.get::<i64>() {
            return Some(v);
        }
        if let Some(&v) = value.get::<i32>() {
            return Some(v as i64);
        }
        None
    }
}

impl FromValue for f32 {
    fn from_value(value: &Value) -> Option<Self> {
        if let Some(&v) = value.get::<f32>() {
            return Some(v);
        }
        if let Some(&v) = value.get::<f64>() {
            return Some(v as f32);
        }
        None
    }
}

impl FromValue for f64 {
    fn from_value(value: &Value) -> Option<Self> {
        if let Some(&v) = value.get::<f64>() {
            return Some(v);
        }
        if let Some(&v) = value.get::<f32>() {
            return Some(v as f64);
        }
        if let Some(&v) = value.get::<i64>() {
            return Some(v as f64);
        }
        if let Some(&v) = value.get::<i32>() {
            return Some(v as f64);
        }
        None
    }
}

impl FromValue for String {
    fn from_value(value: &Value) -> Option<Self> {
        value.get::<String>().cloned()
    }
}

// ============================================================================
// PredicateParam, PredicateParamNamesAndDefaults
// ============================================================================

/// Represents a single named parameter with an optional default value.
#[derive(Debug, Clone)]
pub struct PredicateParam {
    /// Parameter name.
    pub name: String,
    /// Default value (if any).
    pub default_value: Option<Value>,
}

impl PredicateParam {
    /// Creates a parameter with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_value: None,
        }
    }

    /// Creates a parameter with name and default.
    pub fn with_default(name: impl Into<String>, default: Value) -> Self {
        Self {
            name: name.into(),
            default_value: Some(default),
        }
    }
}

/// Represents named function parameters with optional default values.
#[derive(Debug, Clone, Default)]
pub struct PredicateParamNamesAndDefaults {
    /// The parameters.
    params: Vec<PredicateParam>,
    /// Number of parameters with defaults.
    num_defaults: usize,
}

impl PredicateParamNamesAndDefaults {
    /// Creates empty params.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates from a list of params.
    pub fn from_params(params: Vec<PredicateParam>) -> Self {
        let num_defaults = params.iter().filter(|p| p.default_value.is_some()).count();
        Self {
            params,
            num_defaults,
        }
    }

    /// Returns the parameters.
    pub fn params(&self) -> &[PredicateParam] {
        &self.params
    }

    /// Returns the number of defaults.
    pub fn num_defaults(&self) -> usize {
        self.num_defaults
    }

    /// Checks validity.
    ///
    /// All parameters must have non-empty names, and all parameters following
    /// the first one with a default value must also have default values.
    pub fn check_validity(&self) -> bool {
        let mut seen_default = false;
        for param in &self.params {
            if param.default_value.is_some() {
                seen_default = true;
            } else if seen_default {
                return false; // Non-default after default
            }
            if param.name.is_empty() {
                return false; // Empty name
            }
        }
        true
    }
}

// ============================================================================
// try_bind_args — runtime argument matching (mirrors C++ _TryBindOne)
// ============================================================================

/// Resolves expression arguments against parameter names and defaults.
///
/// Given a set of `FnArg`s (from the expression) and a
/// `PredicateParamNamesAndDefaults` specification, tries to bind each
/// parameter by:
/// 1. Positional matching (unnamed args by index)
/// 2. Keyword matching (named args by parameter name)
/// 3. Default values
///
/// Returns `Some(Vec<Value>)` with one value per parameter if all can be
/// bound, `None` if binding fails.
///
/// Mirrors the C++ `_TryBindArgs` / `_TryBindOne` logic.
pub fn try_bind_args(
    args: &[FnArg],
    params: &PredicateParamNamesAndDefaults,
) -> Option<Vec<Value>> {
    let param_list = params.params();

    if param_list.is_empty() {
        return if args.is_empty() {
            Some(Vec::new())
        } else {
            None
        };
    }

    let num_params = param_list.len();
    let mut result: Vec<Option<Value>> = vec![None; num_params];
    let mut bound_args = vec![false; args.len()];

    // Bind each parameter
    for (i, param) in param_list.iter().enumerate() {
        // 1. Check for positional arg at this index
        if i < args.len() && args[i].is_positional() && !bound_args[i] {
            result[i] = Some(args[i].value.clone());
            bound_args[i] = true;
            continue;
        }

        // 2. Check for keyword arg matching this param name
        if !param.name.is_empty() {
            let mut found = false;
            for (j, arg) in args.iter().enumerate() {
                if !bound_args[j] && arg.is_keyword() && arg.name == param.name {
                    result[i] = Some(arg.value.clone());
                    bound_args[j] = true;
                    found = true;
                    break;
                }
            }
            if found {
                continue;
            }
        }

        // 3. Fill from default
        if let Some(ref default) = param.default_value {
            result[i] = Some(default.clone());
        }
    }

    // Check all required params were bound
    for res in &result {
        if res.is_none() {
            return None;
        }
    }

    // Extra unbound args are allowed only if there are no params declared
    // (the caller should use define_binder for variadic functions)
    for (i, &bound) in bound_args.iter().enumerate() {
        if !bound {
            // If this is a positional arg beyond the param count, that's OK
            // only if we already bound all params (extra args ignored for
            // SimpleBinder, handled by CustomBinder)
            if i >= num_params {
                // Extra args beyond declared params
                break;
            }
            return None;
        }
    }

    Some(result.into_iter().map(|v| v.unwrap()).collect())
}

// ============================================================================
// Constancy, PredicateFunctionResult
// ============================================================================

/// Constancy of predicate result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constancy {
    /// Result is constant over descendants.
    ConstantOverDescendants,
    /// Result may vary over descendants.
    MayVaryOverDescendants,
}

/// Result of a predicate function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredicateFunctionResult {
    /// The boolean result.
    pub value: bool,
    /// Whether the result is constant over descendants.
    pub constancy: Constancy,
}

impl Default for PredicateFunctionResult {
    fn default() -> Self {
        Self {
            value: false,
            constancy: Constancy::MayVaryOverDescendants,
        }
    }
}

impl PredicateFunctionResult {
    /// Creates a constant result.
    pub fn make_constant(value: bool) -> Self {
        Self {
            value,
            constancy: Constancy::ConstantOverDescendants,
        }
    }

    /// Creates a varying result.
    pub fn make_varying(value: bool) -> Self {
        Self {
            value,
            constancy: Constancy::MayVaryOverDescendants,
        }
    }

    /// Returns the value.
    pub fn get_value(&self) -> bool {
        self.value
    }

    /// Returns the constancy.
    pub fn get_constancy(&self) -> Constancy {
        self.constancy
    }

    /// Returns true if constant.
    pub fn is_constant(&self) -> bool {
        self.constancy == Constancy::ConstantOverDescendants
    }

    /// Negates the result (preserves constancy).
    pub fn not(&self) -> Self {
        Self {
            value: !self.value,
            constancy: self.constancy,
        }
    }

    /// Sets value and propagates constancy.
    ///
    /// The constancy degrades to MayVaryOverDescendants if either operand
    /// is MayVaryOverDescendants. Once degraded it never upgrades.
    pub fn set_and_propagate_constancy(&mut self, other: PredicateFunctionResult) {
        self.value = other.value;
        if self.constancy == Constancy::ConstantOverDescendants
            && other.constancy == Constancy::MayVaryOverDescendants
        {
            self.constancy = Constancy::MayVaryOverDescendants;
        }
    }
}

impl From<bool> for PredicateFunctionResult {
    fn from(value: bool) -> Self {
        Self::make_varying(value)
    }
}

// ============================================================================
// PredicateFunction, PredicateBinder, SimpleBinder, CustomBinder
// ============================================================================

/// Type alias for predicate function.
pub type PredicateFunction<D> = Arc<dyn Fn(&D) -> PredicateFunctionResult + Send + Sync>;

/// A binder that can bind arguments to create a predicate function.
pub trait PredicateBinder<D>: Send + Sync {
    /// Attempts to bind arguments, returning a predicate function if successful.
    fn bind(&self, args: &[FnArg]) -> Option<PredicateFunction<D>>;

    /// Clones the binder.
    fn clone_box(&self) -> Box<dyn PredicateBinder<D>>;
}

/// Simple function binder for `Fn(&D) -> PredicateFunctionResult` (no extra args).
pub struct SimpleBinder<D, F>
where
    F: Fn(&D) -> PredicateFunctionResult + Send + Sync + Clone + 'static,
{
    func: F,
    names_and_defaults: PredicateParamNamesAndDefaults,
    _phantom: std::marker::PhantomData<D>,
}

impl<D, F> SimpleBinder<D, F>
where
    F: Fn(&D) -> PredicateFunctionResult + Send + Sync + Clone + 'static,
{
    /// Creates a new binder.
    pub fn new(func: F, names_and_defaults: PredicateParamNamesAndDefaults) -> Self {
        Self {
            func,
            names_and_defaults,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<D: Send + Sync + 'static, F> PredicateBinder<D> for SimpleBinder<D, F>
where
    F: Fn(&D) -> PredicateFunctionResult + Send + Sync + Clone + 'static,
{
    fn bind(&self, args: &[FnArg]) -> Option<PredicateFunction<D>> {
        // For functions with no declared params, args must be empty
        if self.names_and_defaults.params().is_empty() {
            if !args.is_empty() {
                return None;
            }
            let f = self.func.clone();
            return Some(Arc::new(move |d| f(d)));
        }

        // Validate args can bind against declared params
        let _bound = try_bind_args(args, &self.names_and_defaults)?;

        // SimpleBinder wraps Fn(&D), so bound args are validated but not
        // passed through. For typed arg binding, use define_binder.
        let f = self.func.clone();
        Some(Arc::new(move |d| f(d)))
    }

    fn clone_box(&self) -> Box<dyn PredicateBinder<D>> {
        Box::new(Self {
            func: self.func.clone(),
            names_and_defaults: self.names_and_defaults.clone(),
            _phantom: std::marker::PhantomData,
        })
    }
}

/// Custom binder that takes a user-supplied binding function.
///
/// Matches C++ `SdfPredicateLibrary::DefineBinder()`.
/// The binding function receives the expression arguments and should
/// return a bound `PredicateFunction` if binding succeeds.
struct CustomBinder<D: 'static> {
    bind_fn: Arc<dyn Fn(&[FnArg]) -> Option<PredicateFunction<D>> + Send + Sync>,
}

impl<D: 'static> PredicateBinder<D> for CustomBinder<D> {
    fn bind(&self, args: &[FnArg]) -> Option<PredicateFunction<D>> {
        (self.bind_fn)(args)
    }

    fn clone_box(&self) -> Box<dyn PredicateBinder<D>> {
        Box::new(CustomBinder {
            bind_fn: self.bind_fn.clone(),
        })
    }
}

// ============================================================================
// PredicateLibrary
// ============================================================================

/// Library of predicate functions for a domain type.
pub struct PredicateLibrary<D: 'static> {
    /// Binders by name.
    binders: HashMap<String, Vec<Box<dyn PredicateBinder<D>>>>,
}

impl<D: Send + Sync + 'static> Default for PredicateLibrary<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: Send + Sync + 'static> Clone for PredicateLibrary<D> {
    fn clone(&self) -> Self {
        let mut binders = HashMap::new();
        for (name, bs) in &self.binders {
            binders.insert(name.clone(), bs.iter().map(|b| b.clone_box()).collect());
        }
        Self { binders }
    }
}

impl<D: Send + Sync + 'static> PredicateLibrary<D> {
    /// Creates an empty library.
    pub fn new() -> Self {
        Self {
            binders: HashMap::new(),
        }
    }

    /// Defines a predicate function (no extra arguments).
    ///
    /// Matches C++ `Define(name, fn)` for functions taking only `DomainType`.
    pub fn define<F>(self, name: &str, func: F) -> Self
    where
        F: Fn(&D) -> PredicateFunctionResult + Send + Sync + Clone + 'static,
    {
        self.define_with_params(name, func, PredicateParamNamesAndDefaults::new())
    }

    /// Defines a predicate function with parameter names and defaults.
    pub fn define_with_params<F>(
        mut self,
        name: &str,
        func: F,
        params: PredicateParamNamesAndDefaults,
    ) -> Self
    where
        F: Fn(&D) -> PredicateFunctionResult + Send + Sync + Clone + 'static,
    {
        let binder = SimpleBinder::new(func, params);
        self.binders
            .entry(name.to_string())
            .or_default()
            .push(Box::new(binder));
        self
    }

    /// Defines a custom binding function.
    ///
    /// Matches C++ `DefineBinder(name, fn)`. The binder function receives
    /// the expression arguments and should return a bound `PredicateFunction`
    /// if binding succeeds, or `None` if the arguments are invalid.
    ///
    /// # Example
    ///
    /// ```ignore
    /// lib.define_binder("isClose", |args: &[FnArg]| {
    ///     let target = f64::from_value(&args.get(0)?.value)?;
    ///     let tolerance = args.get(1)
    ///         .and_then(|a| f64::from_value(&a.value))
    ///         .unwrap_or(0.01);
    ///     Some(Arc::new(move |obj: &f64| {
    ///         PredicateFunctionResult::make_varying((obj - target).abs() <= tolerance)
    ///     }))
    /// });
    /// ```
    pub fn define_binder<F>(mut self, name: &str, binder: F) -> Self
    where
        F: Fn(&[FnArg]) -> Option<PredicateFunction<D>> + Send + Sync + 'static,
    {
        let custom = CustomBinder {
            bind_fn: Arc::new(binder),
        };
        self.binders
            .entry(name.to_string())
            .or_default()
            .push(Box::new(custom));
        self
    }

    /// Binds a function call with arguments.
    ///
    /// Tries binders in reverse order (last defined wins), matching C++
    /// `_BindCall` behavior.
    pub fn bind_call(&self, name: &str, args: &[FnArg]) -> Option<PredicateFunction<D>> {
        let binders = self.binders.get(name)?;
        for binder in binders.iter().rev() {
            if let Some(func) = binder.bind(args) {
                return Some(func);
            }
        }
        None
    }

    /// Returns true if a function is defined.
    pub fn has_function(&self, name: &str) -> bool {
        self.binders.contains_key(name)
    }

    /// Returns all function names.
    pub fn function_names(&self) -> Vec<&str> {
        self.binders.keys().map(|s| s.as_str()).collect()
    }
}

// ============================================================================
// PredicateProgram — flat RPN with short-circuiting
// (matches C++ SdfPredicateProgram)
// ============================================================================

/// Operations in a predicate program (RPN-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgramOp {
    /// Invoke a function call.
    Call,
    /// Logical NOT (postfix).
    Not,
    /// Open bracket for short-circuiting scope.
    Open,
    /// Close bracket for short-circuiting scope.
    Close,
    /// Logical AND (infix, with Open/Close for short-circuit).
    And,
    /// Logical OR (infix, with Open/Close for short-circuit).
    Or,
}

/// Compiled predicate program.
///
/// Uses a flat RPN-style operation list with short-circuiting,
/// matching the C++ `SdfPredicateProgram` architecture.
pub struct PredicateProgram<D: 'static> {
    /// Operations in RPN order.
    ops: Vec<ProgramOp>,
    /// Bound predicate functions, corresponding to Call ops.
    funcs: Vec<PredicateFunction<D>>,
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
    /// Creates an empty program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a program from a single function.
    ///
    /// Convenience for wrapping a single bound function as a program.
    pub fn from_function(func: PredicateFunction<D>) -> Self {
        Self {
            ops: vec![ProgramOp::Call],
            funcs: vec![func],
        }
    }

    /// Returns true if the program is empty (has no ops).
    ///
    /// Matches C++ `operator bool()`.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Evaluates the program on an object with short-circuiting.
    ///
    /// Matches the C++ `SdfPredicateProgram::operator()` logic:
    /// - For AND: if the left operand is false, skip the right operand.
    /// - For OR: if the left operand is true, skip the right operand.
    /// - Constancy is propagated via `SetAndPropagateConstancy`.
    pub fn evaluate(&self, obj: &D) -> PredicateFunctionResult {
        let mut result = PredicateFunctionResult::make_constant(false);
        let mut nest: i32 = 0;
        let mut func_idx: usize = 0;
        let mut op_idx: usize = 0;
        let ops_len = self.ops.len();

        while op_idx < ops_len {
            match self.ops[op_idx] {
                ProgramOp::Call => {
                    if func_idx < self.funcs.len() {
                        result.set_and_propagate_constancy(self.funcs[func_idx](obj));
                        func_idx += 1;
                    }
                }
                ProgramOp::Not => {
                    result = result.not();
                }
                ProgramOp::And | ProgramOp::Or => {
                    // For And: deciding_value = false (short-circuit if false)
                    // For Or: deciding_value = true (short-circuit if true)
                    let deciding_value = self.ops[op_idx] != ProgramOp::And;
                    if result.value == deciding_value {
                        // Short-circuit: skip to matching Close
                        let orig_nest = nest;
                        op_idx += 1;
                        while op_idx < ops_len {
                            match self.ops[op_idx] {
                                ProgramOp::Call => {
                                    func_idx += 1;
                                }
                                ProgramOp::Open => {
                                    nest += 1;
                                }
                                ProgramOp::Close => {
                                    nest -= 1;
                                    if nest == orig_nest {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            op_idx += 1;
                        }
                    }
                }
                ProgramOp::Open => {
                    nest += 1;
                }
                ProgramOp::Close => {
                    nest -= 1;
                }
            }
            op_idx += 1;
        }

        result
    }
}

impl<D: 'static> Clone for PredicateProgram<D> {
    fn clone(&self) -> Self {
        Self {
            ops: self.ops.clone(),
            funcs: self.funcs.clone(),
        }
    }
}

// ============================================================================
// SdfLinkPredicateExpression
// ============================================================================

/// Links a predicate expression with a library to produce a program.
///
/// Walks the predicate expression tree and emits flat RPN ops with
/// Open/Close brackets for short-circuiting, matching the C++
/// `SdfLinkPredicateExpression` implementation.
pub fn link_predicate_expression<D: Send + Sync + 'static>(
    expr: &PredicateExpression,
    lib: &PredicateLibrary<D>,
) -> PredicateProgram<D> {
    use crate::predicate_expression::PredicateOp;

    if expr.is_empty() {
        return PredicateProgram::new();
    }

    // Use RefCell for shared mutable state between the two walk closures.
    use std::cell::RefCell;

    struct ProgramBuilder<D: 'static> {
        ops: Vec<ProgramOp>,
        funcs: Vec<PredicateFunction<D>>,
        errors: Vec<String>,
    }

    let builder = RefCell::new(ProgramBuilder {
        ops: Vec::new(),
        funcs: Vec::new(),
        errors: Vec::new(),
    });

    expr.walk(
        |op, arg_index| {
            let mut b = builder.borrow_mut();
            match op {
                PredicateOp::Not => {
                    // Not is postfix, RPN-style: push after operand is walked
                    if arg_index == 1 {
                        b.ops.push(ProgramOp::Not);
                    }
                }
                PredicateOp::ImpliedAnd | PredicateOp::And | PredicateOp::Or => {
                    // Binary logic ops are infix with Open/Close for
                    // short-circuiting. After the left operand (arg_index==1),
                    // push the op and Open. After the right operand
                    // (arg_index==2), push Close.
                    if arg_index == 1 {
                        let prog_op = match op {
                            PredicateOp::ImpliedAnd | PredicateOp::And => ProgramOp::And,
                            PredicateOp::Or => ProgramOp::Or,
                            _ => unreachable!(),
                        };
                        b.ops.push(prog_op);
                        b.ops.push(ProgramOp::Open);
                    } else if arg_index == 2 {
                        b.ops.push(ProgramOp::Close);
                    }
                }
                PredicateOp::Call => {
                    // Handled by the call callback
                }
            }
        },
        |call| {
            let mut b = builder.borrow_mut();
            if let Some(func) = lib.bind_call(call.name(), call.args()) {
                b.funcs.push(func);
                b.ops.push(ProgramOp::Call);
            } else {
                b.errors
                    .push(format!("Failed to bind call of '{}'", call.name()));
            }
        },
    );

    let b = builder.into_inner();

    if !b.errors.is_empty() {
        eprintln!("SdfLinkPredicateExpression errors: {}", b.errors.join(", "));
        return PredicateProgram::new();
    }

    PredicateProgram {
        ops: b.ops,
        funcs: b.funcs,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predicate_result() {
        let r = PredicateFunctionResult::make_constant(true);
        assert!(r.get_value());
        assert!(r.is_constant());

        let r = PredicateFunctionResult::make_varying(false);
        assert!(!r.get_value());
        assert!(!r.is_constant());
    }

    #[test]
    fn test_library_define() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("isPositive", |x| {
                PredicateFunctionResult::make_varying(*x > 0)
            })
            .define("isEven", |x| {
                PredicateFunctionResult::make_varying(*x % 2 == 0)
            });

        assert!(lib.has_function("isPositive"));
        assert!(lib.has_function("isEven"));
        assert!(!lib.has_function("unknown"));
    }

    #[test]
    fn test_bind_and_evaluate() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new().define("isPositive", |x| {
            PredicateFunctionResult::make_varying(*x > 0)
        });

        if let Some(func) = lib.bind_call("isPositive", &[]) {
            assert!(func(&5).get_value());
            assert!(!func(&-3).get_value());
        }
    }

    #[test]
    fn test_link_predicate_expression_single() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new().define("isPositive", |x| {
            PredicateFunctionResult::make_varying(*x > 0)
        });

        let expr = PredicateExpression::parse("isPositive");
        let program = link_predicate_expression(&expr, &lib);
        assert!(!program.is_empty());
        assert!(program.evaluate(&5).get_value());
        assert!(!program.evaluate(&-3).get_value());
    }

    #[test]
    fn test_link_predicate_expression_and() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("isPositive", |x| {
                PredicateFunctionResult::make_varying(*x > 0)
            })
            .define("isEven", |x| {
                PredicateFunctionResult::make_varying(*x % 2 == 0)
            });

        let expr = PredicateExpression::parse("isPositive and isEven");
        let program = link_predicate_expression(&expr, &lib);
        assert!(!program.is_empty());
        assert!(program.evaluate(&4).get_value()); // positive and even
        assert!(!program.evaluate(&3).get_value()); // positive but odd
        assert!(!program.evaluate(&-2).get_value()); // even but negative
    }

    #[test]
    fn test_link_predicate_expression_or() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("isPositive", |x| {
                PredicateFunctionResult::make_varying(*x > 0)
            })
            .define("isEven", |x| {
                PredicateFunctionResult::make_varying(*x % 2 == 0)
            });

        let expr = PredicateExpression::parse("isPositive or isEven");
        let program = link_predicate_expression(&expr, &lib);
        assert!(!program.is_empty());
        assert!(program.evaluate(&4).get_value()); // positive and even
        assert!(program.evaluate(&3).get_value()); // positive
        assert!(program.evaluate(&-2).get_value()); // even
        assert!(!program.evaluate(&-3).get_value()); // neither
    }

    #[test]
    fn test_link_predicate_expression_not() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new().define("isPositive", |x| {
            PredicateFunctionResult::make_varying(*x > 0)
        });

        let expr = PredicateExpression::parse("not isPositive");
        let program = link_predicate_expression(&expr, &lib);
        assert!(!program.is_empty());
        assert!(!program.evaluate(&5).get_value());
        assert!(program.evaluate(&-3).get_value());
    }

    #[test]
    fn test_link_empty_expression() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new().define("isPositive", |x| {
            PredicateFunctionResult::make_varying(*x > 0)
        });

        let expr = PredicateExpression::new();
        let program = link_predicate_expression(&expr, &lib);
        assert!(program.is_empty());
    }

    #[test]
    fn test_param_validity() {
        let valid = PredicateParamNamesAndDefaults::from_params(vec![
            PredicateParam::new("a"),
            PredicateParam::with_default("b", Value::new(10)),
        ]);
        assert!(valid.check_validity());

        let invalid = PredicateParamNamesAndDefaults::from_params(vec![
            PredicateParam::with_default("a", Value::new(10)),
            PredicateParam::new("b"), // Non-default after default
        ]);
        assert!(!invalid.check_validity());
    }

    #[test]
    fn test_short_circuit_and() {
        // "false_fn and side_effect" should short-circuit: side_effect not called
        use std::sync::atomic::{AtomicBool, Ordering};
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("alwaysFalse", |_x| {
                PredicateFunctionResult::make_varying(false)
            })
            .define_binder("sideEffect", move |_args| {
                let c = called_clone.clone();
                Some(Arc::new(move |_x: &i32| {
                    c.store(true, Ordering::SeqCst);
                    PredicateFunctionResult::make_varying(true)
                }))
            });

        let expr = PredicateExpression::parse("alwaysFalse and sideEffect");
        let program = link_predicate_expression(&expr, &lib);
        let result = program.evaluate(&42);
        assert!(!result.get_value());
        assert!(!called.load(Ordering::SeqCst), "AND should short-circuit");
    }

    #[test]
    fn test_short_circuit_or() {
        // "true_fn or side_effect" should short-circuit: side_effect not called
        use std::sync::atomic::{AtomicBool, Ordering};
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("alwaysTrue", |_x| {
                PredicateFunctionResult::make_varying(true)
            })
            .define_binder("sideEffect", move |_args| {
                let c = called_clone.clone();
                Some(Arc::new(move |_x: &i32| {
                    c.store(true, Ordering::SeqCst);
                    PredicateFunctionResult::make_varying(true)
                }))
            });

        let expr = PredicateExpression::parse("alwaysTrue or sideEffect");
        let program = link_predicate_expression(&expr, &lib);
        let result = program.evaluate(&42);
        assert!(result.get_value());
        assert!(!called.load(Ordering::SeqCst), "OR should short-circuit");
    }

    #[test]
    fn test_define_binder() {
        // Define a function that takes a typed argument via DefineBinder
        let lib: PredicateLibrary<i32> =
            PredicateLibrary::new().define_binder("greaterThan", |args: &[FnArg]| {
                let threshold = i64::from_value(&args.first()?.value)?;
                Some(Arc::new(move |x: &i32| {
                    PredicateFunctionResult::make_varying(*x as i64 > threshold)
                }))
            });

        let expr = PredicateExpression::parse("greaterThan(10)");
        let program = link_predicate_expression(&expr, &lib);
        assert!(!program.is_empty());
        assert!(program.evaluate(&15).get_value());
        assert!(!program.evaluate(&5).get_value());
    }

    #[test]
    fn test_try_bind_args_positional() {
        let params = PredicateParamNamesAndDefaults::from_params(vec![
            PredicateParam::new("a"),
            PredicateParam::new("b"),
        ]);
        let args = vec![
            FnArg::positional(Value::new(1)),
            FnArg::positional(Value::new(2)),
        ];
        let bound = try_bind_args(&args, &params);
        assert!(bound.is_some());
        let bound = bound.unwrap();
        assert_eq!(bound.len(), 2);
        assert_eq!(bound[0].get::<i32>(), Some(&1));
        assert_eq!(bound[1].get::<i32>(), Some(&2));
    }

    #[test]
    fn test_try_bind_args_keyword() {
        let params = PredicateParamNamesAndDefaults::from_params(vec![
            PredicateParam::new("a"),
            PredicateParam::with_default("b", Value::new(99)),
        ]);
        let args = vec![FnArg::keyword("b", Value::new(42))];
        let bound = try_bind_args(&args, &params);
        // Should fail: param "a" has no positional arg and no default
        assert!(bound.is_none());

        // With positional for "a" and keyword for "b"
        let args2 = vec![
            FnArg::positional(Value::new(1)),
            FnArg::keyword("b", Value::new(42)),
        ];
        let bound2 = try_bind_args(&args2, &params);
        assert!(bound2.is_some());
        let bound2 = bound2.unwrap();
        assert_eq!(bound2[0].get::<i32>(), Some(&1));
        assert_eq!(bound2[1].get::<i32>(), Some(&42));
    }

    #[test]
    fn test_try_bind_args_defaults() {
        let params = PredicateParamNamesAndDefaults::from_params(vec![
            PredicateParam::new("a"),
            PredicateParam::with_default("b", Value::new(99)),
        ]);
        let args = vec![FnArg::positional(Value::new(1))];
        let bound = try_bind_args(&args, &params);
        assert!(bound.is_some());
        let bound = bound.unwrap();
        assert_eq!(bound[0].get::<i32>(), Some(&1));
        assert_eq!(bound[1].get::<i32>(), Some(&99));
    }

    #[test]
    fn test_from_value_trait() {
        assert_eq!(bool::from_value(&Value::new(true)), Some(true));
        assert_eq!(i64::from_value(&Value::new(42i64)), Some(42i64));
        assert_eq!(i64::from_value(&Value::new(42i32)), Some(42i64));
        assert_eq!(f64::from_value(&Value::from_f64(3.14)), Some(3.14f64));
        assert_eq!(f64::from_value(&Value::new(42i64)), Some(42.0f64));
        assert_eq!(
            String::from_value(&Value::new("hello".to_string())),
            Some("hello".to_string())
        );
        assert_eq!(String::from_value(&Value::new(42i32)), None);
    }

    #[test]
    fn test_complex_expression() {
        // (foo or bar) and not baz
        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("isPositive", |x: &i32| {
                PredicateFunctionResult::make_varying(*x > 0)
            })
            .define("isEven", |x: &i32| {
                PredicateFunctionResult::make_varying(*x % 2 == 0)
            })
            .define("isSmall", |x: &i32| {
                PredicateFunctionResult::make_varying(x.abs() < 10)
            });

        let expr = PredicateExpression::parse("(isPositive or isEven) and not isSmall");
        let program = link_predicate_expression(&expr, &lib);
        assert!(!program.is_empty());

        // 20: positive=true, even=true, small=false → (T or T) and not F = T and T = T
        assert!(program.evaluate(&20).get_value());
        // 3: positive=true, even=false, small=true → (T or F) and not T = T and F = F
        assert!(!program.evaluate(&3).get_value());
        // -20: positive=false, even=true, small=false → (F or T) and not F = T and T = T
        assert!(program.evaluate(&-20).get_value());
        // -3: positive=false, even=false, small=true → (F or F) and not T = F and F = F
        assert!(!program.evaluate(&-3).get_value());
    }

    #[test]
    fn test_constancy_propagation() {
        let lib: PredicateLibrary<i32> = PredicateLibrary::new()
            .define("constant_true", |_x| {
                PredicateFunctionResult::make_constant(true)
            })
            .define("varying_true", |_x| {
                PredicateFunctionResult::make_varying(true)
            });

        // Both constant → result should be constant
        let expr1 = PredicateExpression::parse("constant_true");
        let prog1 = link_predicate_expression(&expr1, &lib);
        let r1 = prog1.evaluate(&0);
        assert!(r1.is_constant());

        // Mix of constant and varying → result should be varying
        let expr2 = PredicateExpression::parse("constant_true and varying_true");
        let prog2 = link_predicate_expression(&expr2, &lib);
        let r2 = prog2.evaluate(&0);
        assert!(!r2.is_constant());
    }
}
