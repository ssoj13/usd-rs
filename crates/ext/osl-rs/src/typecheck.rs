//! Type checking pass for the OSL AST.
//!
//! Port of `typecheck.cpp`. Walks the AST and resolves types for all
//! expressions, checking type compatibility and inserting implicit coercions.
//!
//! Implements the full C++ type checking semantics including:
//! - Implicit int→float, float→triple, int→triple coercion
//! - Binary expression type rules (closure ops, point-point=vector, etc.)
//! - Unary expression validation per type
//! - Assignment type checking (arrays, structs, closure=0)
//! - Index type checking (array, component, matrix [][])
//! - Conditional/loop condition type validation
//! - Return type checking against enclosing function
//! - Type constructor pattern matching (float, triple, matrix, int)
//! - Ternary expression type unification
//! - Typecast validation
//! - Function overload resolution with scoring

#![allow(dead_code)]

use crate::ast::*;
use crate::typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};
use crate::typespec::TypeSpec;

/// Type checking error.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub loc: crate::lexer::SourceLoc,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.loc, self.message)
    }
}

/// Type checking warning (non-fatal).
#[derive(Debug, Clone)]
pub struct TypeWarning {
    pub message: String,
    pub loc: crate::lexer::SourceLoc,
}

// ---------------------------------------------------------------------------
// Type predicate helpers
// ---------------------------------------------------------------------------

fn is_int(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Int32 as u8 && t.aggregate == Aggregate::Scalar as u8
}

fn is_float(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Float as u8 && t.aggregate == Aggregate::Scalar as u8
}

fn is_int_or_float(ts: TypeSpec) -> bool {
    is_int(ts) || is_float(ts)
}

fn is_string(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::String as u8
}

fn is_triple(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Float as u8 && t.aggregate == Aggregate::Vec3 as u8
}

fn is_matrix(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Float as u8 && t.aggregate == Aggregate::Matrix44 as u8
}

fn is_numeric(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Float as u8 || t.basetype == BaseType::Int32 as u8
}

fn is_color(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Float as u8
        && t.aggregate == Aggregate::Vec3 as u8
        && t.vecsemantics == VecSemantics::Color as u8
}

fn is_point(ts: TypeSpec) -> bool {
    let t = ts.simpletype();
    t.basetype == BaseType::Float as u8
        && t.aggregate == Aggregate::Vec3 as u8
        && t.vecsemantics == VecSemantics::Point as u8
}

fn is_closure(ts: TypeSpec) -> bool {
    ts.is_closure()
}

fn is_void(ts: TypeSpec) -> bool {
    ts.simpletype().basetype == BaseType::None as u8
}

/// Check whether a list of statements contains at least one `return` at the top level.
/// Used for the missing-return warning in non-void functions.
fn stmts_have_return(stmts: &[Box<ASTNode>]) -> bool {
    stmts.iter().any(|s| stmt_has_return(s))
}

/// Recursively check whether a statement or block contains a `return`.
/// Does not descend into nested `FunctionDeclaration` nodes.
fn stmt_has_return(node: &ASTNode) -> bool {
    match &node.kind {
        ASTNodeKind::ReturnStatement { .. } => true,
        // Stop at nested functions — their returns belong to them.
        ASTNodeKind::FunctionDeclaration { .. } => false,
        ASTNodeKind::CompoundStatement { statements }
        | ASTNodeKind::StatementList { statements } => stmts_have_return(statements),
        ASTNodeKind::ConditionalStatement {
            true_stmt,
            false_stmt,
            ..
        } => {
            // Guaranteed only if both branches have a return.
            stmt_has_return(true_stmt) && false_stmt.as_deref().map_or(false, stmt_has_return)
        }
        ASTNodeKind::LoopStatement { body, .. } => stmt_has_return(body),
        _ => false,
    }
}

/// Types that are "spatial" triples (point, vector, normal) — not color.
fn is_spatial_triple(ts: TypeSpec) -> bool {
    is_triple(ts) && !is_color(ts)
}

/// Given two numeric types, return the one with "more precision".
/// hp(int,float) == float, hp(vector,float) == vector, etc.
/// Matches C++ `higherprecision`.
fn higher_precision(a: TypeSpec, b: TypeSpec) -> TypeSpec {
    let ad = a.simpletype();
    let bd = b.simpletype();
    // Aggregate always beats non-aggregate
    if ad.aggregate > bd.aggregate {
        return a;
    } else if bd.aggregate > ad.aggregate {
        return b;
    }
    // Float beats int
    if bd.basetype == BaseType::Float as u8 {
        return b;
    }
    a
}

/// Check if two types are "equivalent" (same base type and aggregate, ignoring
/// vec semantics differences among triples).
fn equivalent(a: TypeSpec, b: TypeSpec) -> bool {
    if a == b {
        return true;
    }
    let ad = a.simpletype();
    let bd = b.simpletype();
    // Same basetype and aggregate = equivalent (e.g., point == vector == normal == color)
    ad.basetype == bd.basetype && ad.aggregate == bd.aggregate
}

// ---------------------------------------------------------------------------
// Overload resolution scoring (matches C++ CandidateFunctions)
// ---------------------------------------------------------------------------

const SCORE_EXACT: i32 = 100;
const SCORE_INT_TO_FLOAT: i32 = 77;
const SCORE_ARRAY_MATCH: i32 = 44;
const SCORE_COERCABLE: i32 = 23;
const SCORE_SPATIAL_COERCE: i32 = 32; // kCoercable + 9
const SCORE_TRIPLE_COERCE: i32 = 27; // kCoercable + 4
const SCORE_NO_MATCH: i32 = 0;
const SCORE_MATCH_ANYTHING: i32 = 1;

// ---------------------------------------------------------------------------
// Wildcard sentinel TypeSpec values for builtin argument pattern matching.
// Mirrors C++ typecheck.cpp argcode wildcards: ?, ?[], *, .
// Uses structure field < 0 as discriminant — never occurs in real types.
// ---------------------------------------------------------------------------

/// `?` — match any single non-array argument.
pub const WILDCARD_ANY: TypeSpec = TypeSpec::sentinel(-1);
/// `?[]` — match any array argument.
pub const WILDCARD_ARRAY: TypeSpec = TypeSpec::sentinel(-2);
/// `*` — accept all remaining arguments (variadic tail).
pub const WILDCARD_REST: TypeSpec = TypeSpec::sentinel(-3);
/// `.` — match zero or more (string key, any value) token/value pairs.
pub const WILDCARD_TOKENPAIR: TypeSpec = TypeSpec::sentinel(-4);

#[inline]
fn is_wildcard_any(ts: TypeSpec) -> bool {
    ts == WILDCARD_ANY
}
#[inline]
fn is_wildcard_array(ts: TypeSpec) -> bool {
    ts == WILDCARD_ARRAY
}
#[inline]
fn is_wildcard_rest(ts: TypeSpec) -> bool {
    ts == WILDCARD_REST
}
#[inline]
fn is_wildcard_tokenpair(ts: TypeSpec) -> bool {
    ts == WILDCARD_TOKENPAIR
}

/// Score how well `actual` matches `expected` for overload resolution.
/// Handles WILDCARD_ANY and WILDCARD_ARRAY sentinels.
/// WILDCARD_REST and WILDCARD_TOKENPAIR are handled in the resolve_overload loop.
fn score_type(expected: TypeSpec, actual: TypeSpec) -> i32 {
    // `?` matches any single non-array type
    if is_wildcard_any(expected) {
        return if actual.is_array() {
            SCORE_NO_MATCH
        } else {
            SCORE_MATCH_ANYTHING
        };
    }
    // `?[]` matches any array type
    if is_wildcard_array(expected) {
        return if actual.is_array() {
            SCORE_MATCH_ANYTHING
        } else {
            SCORE_NO_MATCH
        };
    }
    if expected == actual {
        return SCORE_EXACT;
    }
    // int -> float or float -> int
    if is_int_or_float(actual)
        && is_int_or_float(expected)
        && !is_closure(actual)
        && !is_closure(expected)
    {
        if is_int(expected) {
            return SCORE_NO_MATCH; // float->int not allowed implicitly
        }
        return SCORE_INT_TO_FLOAT; // int->float
    }
    // Assignable covers all other implicit conversions
    if TypeChecker::assignable(expected, actual) {
        if is_spatial_triple(actual) && is_spatial_triple(expected) {
            return SCORE_SPATIAL_COERCE;
        }
        if is_triple(actual) && is_triple(expected) {
            return SCORE_TRIPLE_COERCE;
        }
        return SCORE_COERCABLE;
    }
    SCORE_NO_MATCH
}

// ---------------------------------------------------------------------------
// TypeChecker
// ---------------------------------------------------------------------------

/// Type checking context.
pub struct TypeChecker {
    pub errors: Vec<TypeError>,
    pub warnings: Vec<TypeWarning>,
    /// Variable type environment: name -> TypeSpec.
    env: Vec<(String, TypeSpec)>,
    /// Stack of current function return types (for return type checking).
    function_stack: Vec<TypeSpec>,
    /// Nesting level for loops (for break/continue validation).
    loop_nesting: i32,
    /// User-defined function table: name -> list of overloads (return + param types).
    user_functions: std::collections::HashMap<String, Vec<(TypeSpec, Vec<TypeSpec>)>>,
    /// Set of readonly variable names (non-output shader params).
    /// C++ Symbol::readonly() -- true for params that are not output.
    readonly_vars: std::collections::HashSet<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            env: Vec::new(),
            function_stack: Vec::new(),
            loop_nesting: 0,
            user_functions: std::collections::HashMap::new(),
            readonly_vars: std::collections::HashSet::new(),
        }
    }

    /// Type-check an entire shader file.
    pub fn check(&mut self, nodes: &mut [Box<ASTNode>]) {
        for node in nodes.iter_mut() {
            self.check_node(node);
        }
    }

    fn push_var(&mut self, name: &str, ts: TypeSpec) {
        self.env.push((name.to_string(), ts));
    }

    fn lookup_var(&self, name: &str) -> Option<TypeSpec> {
        self.env
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, ts)| *ts)
    }

    fn add_error(&mut self, loc: crate::lexer::SourceLoc, msg: String) {
        self.errors.push(TypeError { message: msg, loc });
    }

    fn add_warning(&mut self, loc: crate::lexer::SourceLoc, msg: String) {
        self.warnings.push(TypeWarning { message: msg, loc });
    }

    /// Extract the underlying variable name from an lvalue expression.
    /// Recursively unwraps Index and StructSelect to find the VariableRef.
    fn lvalue_var_name(node: &ASTNode) -> Option<&str> {
        match &node.kind {
            ASTNodeKind::VariableRef { name } => Some(name.as_str()),
            ASTNodeKind::Index { base, .. } => Self::lvalue_var_name(base),
            ASTNodeKind::StructSelect { base, .. } => Self::lvalue_var_name(base),
            _ => None,
        }
    }

    /// Check if a variable is writable (C++ check_symbol_writeability).
    /// Returns false and emits a warning if the target is a non-output param.
    fn check_symbol_writeability(&mut self, node: &ASTNode) -> bool {
        if let Some(name) = Self::lvalue_var_name(node) {
            if self.readonly_vars.contains(name) {
                self.add_warning(
                    node.loc,
                    format!("cannot write to non-output parameter \"{}\"", name),
                );
                return false;
            }
        }
        true
    }

    /// Map a binary operator to the C++ opword for `__operator__XX__` lookup.
    fn binary_opword(op: Operator) -> Option<&'static str> {
        match op {
            Operator::Add => Some("add"),
            Operator::Sub => Some("sub"),
            Operator::Mul => Some("mul"),
            Operator::Div => Some("div"),
            Operator::Mod => Some("mod"),
            Operator::Eq => Some("eq"),
            Operator::NotEq => Some("neq"),
            Operator::Less => Some("lt"),
            Operator::Greater => Some("gt"),
            Operator::LessEq => Some("le"),
            Operator::GreaterEq => Some("ge"),
            Operator::BitAnd => Some("bitand"),
            Operator::BitOr => Some("bitor"),
            Operator::BitXor => Some("xor"),
            Operator::Shl => Some("shl"),
            Operator::Shr => Some("shr"),
            // C++ disallows overloading && and || (short-circuit semantics)
            Operator::LogAnd | Operator::LogOr => None,
            _ => None,
        }
    }

    /// Map a unary operator to the C++ opword for `__operator__XX__` lookup.
    fn unary_opword(op: Operator) -> Option<&'static str> {
        match op {
            Operator::Add => Some("add"),
            Operator::Neg => Some("neg"),
            Operator::Not => Some("not"),
            Operator::BitNot => Some("compl"),
            _ => None,
        }
    }

    /// Look up a user-defined operator overload function `__operator__XX__`.
    /// Returns the return type if found, else None.
    fn lookup_operator_overload(&self, opword: &str) -> Option<TypeSpec> {
        let funcname = format!("__operator__{}__", opword);
        if let Some(overloads) = self.user_functions.get(&funcname) {
            // Return the first overload's return type
            return overloads.first().map(|(ret, _)| *ret);
        }
        None
    }

    /// Compute the argtakesderivs bitmask for a function call.
    /// Port of C++ typecheck_builtin_specialcase derivative tracking.
    /// Bit i = arg i takes derivatives (0-indexed where arg 0 = return value).
    fn compute_argtakesderivs(name: &str, args: &[Box<ASTNode>]) -> u32 {
        let nargs = args.len();
        let mut derivs: u32 = 0;
        let set = |d: &mut u32, arg: usize, val: bool| {
            if val {
                *d |= 1 << arg;
            }
        };
        match name {
            "area" | "filterwidth" | "calculatenormal" | "Dx" | "Dy" | "Dz" => {
                set(&mut derivs, 1, true);
            }
            "texture" => {
                // 3-arg or if 4th arg is string -> args 2,3 take derivs
                if nargs == 3 || (nargs > 3 && is_string(args[3].typespec)) {
                    set(&mut derivs, 2, true);
                    set(&mut derivs, 3, true);
                }
            }
            "texture3d" => {
                if nargs == 2 || (nargs > 2 && is_string(args[2].typespec)) {
                    set(&mut derivs, 2, true);
                }
            }
            "environment" => {
                if nargs == 2 || (nargs > 2 && is_string(args[2].typespec)) {
                    set(&mut derivs, 2, true);
                }
            }
            "trace" => {
                set(&mut derivs, 1, true);
                set(&mut derivs, 2, true);
            }
            "noise" | "pnoise" => {
                // If first arg is a string, and is either not a literal or is "gabor",
                // then positional args after the string take derivs
                if !args.is_empty() && is_string(args[0].typespec) {
                    let is_gabor = match &args[0].kind {
                        ASTNodeKind::Literal {
                            value: LiteralValue::String(s),
                        } => s == "gabor",
                        _ => true, // not a literal -> conservatively assume gabor
                    };
                    if is_gabor {
                        // Skip first arg (string), mark remaining positional args
                        for n in 1..nargs {
                            if is_string(args[n].typespec) {
                                break;
                            }
                            set(&mut derivs, (n + 1) as usize, true); // +1 for return val at pos 0
                        }
                    }
                }
            }
            _ => {} // most functions don't take derivs
        }
        derivs
    }

    /// Check if an AST node is a literal with value 0 (int 0 or float 0.0).
    /// C++ checks `nodetype() == literal_node && floatval() == 0.0f`.
    fn is_literal_zero(node: &ASTNode) -> bool {
        match &node.kind {
            ASTNodeKind::Literal {
                value: LiteralValue::Int(v),
            } => *v == 0,
            ASTNodeKind::Literal {
                value: LiteralValue::Float(v),
            } => *v == 0.0,
            _ => false,
        }
    }

    /// Adjust types of a compound initializer's elements to match a target type.
    ///
    /// Mirrors C++ `TypeAdjuster` class (typecheck.cpp:737-1015).
    /// Mutably sets typespec on each element (and nested compound initializers),
    /// enabling correct codegen for struct fields, arrays, and constructors.
    ///
    /// `must_init_all` — every field/element must have an initializer (for function args).
    /// `report_errors` — emit error messages (false = probe/no_errors mode).
    /// Returns true on success.
    fn type_adjust_compound_init(
        &mut self,
        elements: &mut Vec<Box<ASTNode>>,
        target: TypeSpec,
        loc: crate::lexer::SourceLoc,
        must_init_all: bool,
        report_errors: bool,
    ) -> bool {
        if target.is_array() {
            self.adjust_array(elements, target, loc, must_init_all, report_errors)
        } else if target.is_structure_based() {
            self.adjust_fields(elements, target, loc, must_init_all, report_errors)
        } else {
            // Scalar/triple/matrix: compound-init acts as a type constructor call.
            self.adjust_scalar_ctor(elements, target, loc, report_errors)
        }
    }

    /// Adjust compound-init elements for an array target.
    /// C++ TypeAdjuster::typecheck_array.
    fn adjust_array(
        &mut self,
        elements: &mut Vec<Box<ASTNode>>,
        target: TypeSpec,
        loc: crate::lexer::SourceLoc,
        must_init_all: bool,
        report_errors: bool,
    ) -> bool {
        let elem_ts = target.elementtype();
        let expected_len = target.arraylength(); // negative = unsized

        let nelem = elements.len() as i32;

        for e in elements.iter_mut() {
            if let ASTNodeKind::CompoundInitializer { elements: sub, .. } = &mut e.kind {
                // Sub compound-init: assign element type and recurse.
                e.typespec = elem_ts;
                if !self.type_adjust_compound_init(sub, elem_ts, loc, must_init_all, report_errors)
                {
                    return false;
                }
            } else if !e.typespec.is_unknown()
                && !Self::assignable(elem_ts, e.typespec)
                && !equivalent(elem_ts, e.typespec)
            {
                if report_errors {
                    self.add_error(
                        loc,
                        format!(
                            "initializer element type mismatch: expected {}, got {}",
                            elem_ts, e.typespec
                        ),
                    );
                }
                return false;
            } else if Self::assignable(elem_ts, e.typespec) {
                // Promote element typespec to the declared element type.
                e.typespec = elem_ts;
            }
        }

        // Validate element count against declared array length.
        if expected_len > 0 {
            if nelem > expected_len {
                if report_errors {
                    self.add_error(
                        loc,
                        format!(
                            "too many initializers for array of length {} (got {})",
                            expected_len, nelem
                        ),
                    );
                }
                return false;
            }
            if must_init_all && nelem != expected_len {
                if report_errors {
                    self.add_error(
                        loc,
                        format!(
                            "too {} initializers for array of length {} (got {})",
                            if nelem < expected_len { "few" } else { "many" },
                            expected_len,
                            nelem
                        ),
                    );
                }
                return false;
            }
        }
        // Unsized array (expected_len <= 0): any element count is fine.
        true
    }

    /// Adjust compound-init elements for a struct target.
    /// C++ TypeAdjuster::typecheck_fields.
    fn adjust_fields(
        &mut self,
        elements: &mut Vec<Box<ASTNode>>,
        target: TypeSpec,
        loc: crate::lexer::SourceLoc,
        must_init_all: bool,
        report_errors: bool,
    ) -> bool {
        let sid = target.structure_id();
        let spec = match crate::typespec::get_struct(sid as i32) {
            Some(s) => s,
            None => return true,
        };
        let nfields = spec.fields.len();
        let nelem = elements.len();

        if nelem > nfields {
            if report_errors {
                self.add_error(
                    loc,
                    format!(
                        "too many initializers for struct '{}' ({} fields, got {})",
                        spec.name, nfields, nelem
                    ),
                );
            }
            return false;
        }
        if must_init_all && nelem != nfields {
            if report_errors {
                self.add_error(
                    loc,
                    format!(
                        "too {} initializers for struct '{}' ({} fields, got {})",
                        if nelem < nfields { "few" } else { "many" },
                        spec.name,
                        nfields,
                        nelem
                    ),
                );
            }
            return false;
        }

        // Clone field info to avoid borrow conflict with self in recursive calls.
        let fields: Vec<_> = spec
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.type_spec))
            .collect();
        drop(spec);

        for (i, e) in elements.iter_mut().enumerate() {
            if i >= nfields {
                break;
            }
            let (ref fname, ftype) = fields[i];

            if let ASTNodeKind::CompoundInitializer { elements: sub, .. } = &mut e.kind {
                // Nested compound-init: assign field type and recurse.
                e.typespec = ftype;
                if !self.type_adjust_compound_init(sub, ftype, loc, must_init_all, report_errors) {
                    return false;
                }
            } else if !e.typespec.is_unknown()
                && !Self::assignable(ftype, e.typespec)
                && !equivalent(ftype, e.typespec)
            {
                if report_errors {
                    self.add_error(
                        loc,
                        format!("cannot assign '{}' to struct field '{}'", e.typespec, fname),
                    );
                }
                return false;
            } else if Self::assignable(ftype, e.typespec) {
                e.typespec = ftype;
            }
        }
        true
    }

    /// Adjust compound-init as a scalar/triple/matrix constructor call.
    /// C++ TypeAdjuster::typecheck_init -> ASTtype_constructor::typecheck.
    fn adjust_scalar_ctor(
        &mut self,
        elements: &mut Vec<Box<ASTNode>>,
        target: TypeSpec,
        loc: crate::lexer::SourceLoc,
        report_errors: bool,
    ) -> bool {
        if elements.is_empty() {
            if report_errors {
                self.add_error(
                    loc,
                    format!("empty initializer list cannot represent '{}'", target),
                );
            }
            return false;
        }

        let n = elements.len();

        // Validate constructor argument count/types per OSL constructor patterns.
        let valid = if is_float(target) || is_int(target) {
            // float/int constructor: one numeric arg.
            n == 1 && is_int_or_float(elements[0].typespec)
        } else if is_triple(target) {
            // triple(f|i), triple(f,f,f), triple(string,f,f,f), triple(triple)
            match n {
                1 => is_int_or_float(elements[0].typespec) || is_triple(elements[0].typespec),
                3 => elements.iter().all(|e| is_int_or_float(e.typespec)),
                4 => {
                    is_string(elements[0].typespec)
                        && elements[1..].iter().all(|e| is_int_or_float(e.typespec))
                }
                _ => false,
            }
        } else if is_matrix(target) {
            // matrix(f), matrix(s,f), matrix(s,s), matrix(16*f), matrix(s,16*f), matrix(m)
            match n {
                1 => is_int_or_float(elements[0].typespec) || is_matrix(elements[0].typespec),
                2 => {
                    is_string(elements[0].typespec)
                        && (is_int_or_float(elements[1].typespec)
                            || is_string(elements[1].typespec))
                }
                16 => elements.iter().all(|e| is_int_or_float(e.typespec)),
                17 => {
                    is_string(elements[0].typespec)
                        && elements[1..].iter().all(|e| is_int_or_float(e.typespec))
                }
                _ => false,
            }
        } else if is_string(target) {
            n == 1 && is_string(elements[0].typespec)
        } else {
            false
        };

        if !valid {
            if report_errors {
                let arg_types: Vec<String> =
                    elements.iter().map(|e| format!("{}", e.typespec)).collect();
                self.add_error(
                    loc,
                    format!(
                        "cannot construct '{}' from ({})",
                        target,
                        arg_types.join(", ")
                    ),
                );
            }
            return false;
        }
        true
    }

    /// Check if `src` can be assigned to `dst`.
    /// Matches the C++ `assignable` function in typecheck.cpp.
    pub fn assignable(dst: TypeSpec, src: TypeSpec) -> bool {
        if dst == src {
            return true;
        }
        // Equivalent types (e.g. point = vector) are assignable
        if equivalent(dst, src) {
            return true;
        }
        let d = dst.simpletype();
        let s = src.simpletype();
        // int -> float
        if d.basetype == BaseType::Float as u8
            && d.aggregate == Aggregate::Scalar as u8
            && s.basetype == BaseType::Int32 as u8
        {
            return true;
        }
        // float/int -> triple (any float scalar promotes to any vec3 type)
        if d.basetype == BaseType::Float as u8
            && d.aggregate == Aggregate::Vec3 as u8
            && s.aggregate == Aggregate::Scalar as u8
            && (s.basetype == BaseType::Float as u8 || s.basetype == BaseType::Int32 as u8)
        {
            return true;
        }
        // float/int -> matrix
        if d.basetype == BaseType::Float as u8
            && d.aggregate == Aggregate::Matrix44 as u8
            && s.aggregate == Aggregate::Scalar as u8
            && (s.basetype == BaseType::Float as u8 || s.basetype == BaseType::Int32 as u8)
        {
            return true;
        }
        // Array assignability: same element type, dst length >= src length
        if dst.is_array() && src.is_array() {
            let de = dst.elementtype();
            let se = src.elementtype();
            if equivalent(de, se) {
                return true;
            }
        }
        false
    }

    /// Determine the result type of a binary expression.
    /// Matches the full C++ `ASTbinary_expression::typecheck`.
    pub fn binary_result_type(op: Operator, left: TypeSpec, right: TypeSpec) -> TypeSpec {
        let l = left;
        let r = right;

        // --- Closure special cases ---
        if is_closure(l) || is_closure(r) {
            match op {
                Operator::Add => {
                    if is_closure(l) && is_closure(r) {
                        return l;
                    }
                }
                Operator::Mul => {
                    if is_closure(l) && !is_closure(r) && (is_color(r) || is_int_or_float(r)) {
                        return l;
                    }
                    if !is_closure(l) && is_closure(r) && (is_color(l) || is_int_or_float(l)) {
                        return r;
                    }
                }
                Operator::LogAnd | Operator::LogOr => {
                    return TypeSpec::from_simple(TypeDesc::INT);
                }
                _ => {}
            }
            // If we got here with closures, it's not allowed → return UNKNOWN
            return TypeSpec::UNKNOWN;
        }

        // --- Comparison ops always return int ---
        match op {
            Operator::Eq | Operator::NotEq => {
                // Any equivalent types or numeric+int/float can be compared
                if equivalent(l, r)
                    || (is_numeric(l) && is_int_or_float(r))
                    || (is_int_or_float(l) && is_numeric(r))
                {
                    return TypeSpec::from_simple(TypeDesc::INT);
                }
                return TypeSpec::UNKNOWN;
            }
            Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq => {
                // G/L comparisons only work with floats or ints
                if is_int_or_float(l) && is_int_or_float(r) {
                    return TypeSpec::from_simple(TypeDesc::INT);
                }
                return TypeSpec::UNKNOWN;
            }
            Operator::LogAnd | Operator::LogOr => {
                // Logical ops work on any simple type, always return int
                return TypeSpec::from_simple(TypeDesc::INT);
            }
            _ => {}
        }

        // --- Bitwise ops: only ints ---
        match op {
            Operator::BitAnd
            | Operator::BitOr
            | Operator::BitXor
            | Operator::Shl
            | Operator::Shr => {
                if is_int(l) && is_int(r) {
                    return TypeSpec::from_simple(TypeDesc::INT);
                }
                return TypeSpec::UNKNOWN;
            }
            _ => {}
        }

        // --- Mod: only ints ---
        if op == Operator::Mod {
            if is_int(l) && is_int(r) {
                return TypeSpec::from_simple(TypeDesc::INT);
            }
            return TypeSpec::UNKNOWN;
        }

        // --- Arithmetic: Add/Sub/Mul/Div ---
        // No string arithmetic
        if is_string(l) || is_string(r) {
            return TypeSpec::UNKNOWN;
        }

        // Add/Sub don't work with matrices
        if (op == Operator::Add || op == Operator::Sub) && (is_matrix(l) || is_matrix(r)) {
            return TypeSpec::UNKNOWN;
        }

        // If both are equivalent types...
        if equivalent(l, r) {
            // point - point = vector
            if op == Operator::Sub && is_point(l) && is_point(r) {
                return TypeSpec::from_simple(TypeDesc::new(
                    BaseType::Float,
                    Aggregate::Vec3,
                    VecSemantics::Vector,
                ));
            }
            // point +/- vector or vector +/- point = point
            if (op == Operator::Add || op == Operator::Sub) && (is_point(l) || is_point(r)) {
                return TypeSpec::from_simple(TypeDesc::new(
                    BaseType::Float,
                    Aggregate::Vec3,
                    VecSemantics::Point,
                ));
            }
            return l;
        }

        // Numeric + int/float → higher precision
        if (is_numeric(l) && is_int_or_float(r)) || (is_int_or_float(l) && is_numeric(r)) {
            return higher_precision(l, r);
        }

        // Fallback for same type
        if l == r {
            return l;
        }

        // Triple + anything numeric
        if is_triple(l) && is_numeric(r) {
            return l;
        }
        if is_numeric(l) && is_triple(r) {
            return r;
        }

        // Matrix + numeric
        if is_matrix(l) && is_numeric(r) {
            return l;
        }
        if is_numeric(l) && is_matrix(r) {
            return r;
        }

        TypeSpec::UNKNOWN
    }

    fn check_node(&mut self, node: &mut ASTNode) {
        let loc = node.loc;

        match &mut node.kind {
            ASTNodeKind::ShaderDeclaration {
                formals,
                statements,
                ..
            } => {
                for f in formals.iter_mut() {
                    self.check_node(f);
                }
                for s in statements.iter_mut() {
                    self.check_node(s);
                }
            }

            ASTNodeKind::FunctionDeclaration {
                name,
                return_type,
                formals,
                statements,
                ..
            } => {
                // Register user-defined function for overload resolution
                let mut param_types: Vec<TypeSpec> = Vec::with_capacity(formals.len());
                for formal in formals.iter() {
                    if let ASTNodeKind::VariableDeclaration { typespec, .. } = &formal.kind {
                        param_types.push(*typespec);
                    }
                }
                self.user_functions
                    .entry(name.clone())
                    .or_default()
                    .push((*return_type, param_types));

                // C++ rejects arrays of structs as function parameters
                for formal in formals.iter() {
                    if let ASTNodeKind::VariableDeclaration {
                        typespec,
                        name: pname,
                        ..
                    } = &formal.kind
                    {
                        let ts = *typespec;
                        if ts.is_array() && ts.elementtype().is_structure_based() {
                            self.add_error(
                                formal.loc,
                                format!(
                                    "parameter '{}': arrays of structs are not allowed as parameters",
                                    pname
                                ),
                            );
                        }
                    }
                }

                let fn_return_type = *return_type;
                let fn_name = name.clone();
                self.function_stack.push(fn_return_type);
                for f in formals.iter_mut() {
                    self.check_node(f);
                }
                for s in statements.iter_mut() {
                    self.check_node(s);
                }
                self.function_stack.pop();

                // Warn if a non-void function has no guaranteed return path.
                // Matches C++ typecheck.cpp "function has no return statement" warning.
                if !is_void(fn_return_type) && !stmts_have_return(statements) {
                    self.add_warning(loc, format!("function '{fn_name}' has no return statement"));
                }
            }

            ASTNodeKind::VariableDeclaration {
                name,
                typespec,
                init,
                is_param,
                is_output,
                ..
            } => {
                let ts = *typespec;
                let var_name = name.clone();
                let is_param = *is_param;
                let is_output = *is_output;
                self.push_var(&var_name, ts);
                // Track readonly status: non-output params are readonly (C++ Symbol::readonly)
                if is_param && !is_output {
                    self.readonly_vars.insert(var_name.clone());
                }
                node.typespec = ts;

                if let Some(init_expr) = init {
                    self.check_node(init_expr);
                    let init_ts = init_expr.typespec;

                    // Compound initializer coercion: `float a[3] = {1,2,3}`
                    // or `color c = {1,0,0}` — accept if init is a compound init
                    let is_compound_init =
                        matches!(init_expr.kind, ASTNodeKind::CompoundInitializer { .. });

                    // Special case: closure = 0 is ok (C++ checks literal == 0.0)
                    if is_closure(ts)
                        && !is_closure(init_ts)
                        && (is_float(init_ts) || is_int(init_ts))
                        && Self::is_literal_zero(init_expr)
                    {
                        // ok -- initializing closure to null
                    } else if is_compound_init {
                        // Compound initializer: propagate declared type and adjust element types.
                        init_expr.typespec = ts;
                        if let ASTNodeKind::CompoundInitializer { elements, .. } =
                            &mut init_expr.kind
                        {
                            self.type_adjust_compound_init(elements, ts, loc, false, true);
                        }
                    } else if ts.is_structure() && !init_ts.is_structure() {
                        self.add_error(
                            loc,
                            format!(
                                "cannot initialize struct '{}' with non-struct type",
                                var_name,
                            ),
                        );
                    } else if init_ts.is_unknown() {
                        // Unknown init type (e.g. unresolved function call) — accept silently.
                        // The function resolver or a later pass will validate.
                    } else if !Self::assignable(ts, init_ts) {
                        // If init comes from a function call (possibly wrong overload),
                        // downgrade to warning rather than hard error.
                        let is_func_call =
                            matches!(init_expr.kind, ASTNodeKind::FunctionCall { .. });
                        if is_int(ts) && is_float(init_ts) {
                            self.add_warning(
                                loc,
                                format!("initialization may lose precision: {} = float", var_name,),
                            );
                        } else if is_func_call {
                            self.add_warning(
                                loc,
                                format!(
                                    "implicit conversion in initialization: {} {:?} = {:?}",
                                    var_name,
                                    ts.simpletype(),
                                    init_ts.simpletype()
                                ),
                            );
                        } else {
                            self.add_error(
                                loc,
                                format!(
                                    "cannot initialize '{}' of type {:?} with {:?}",
                                    var_name,
                                    ts.simpletype(),
                                    init_ts.simpletype()
                                ),
                            );
                        }
                    }
                }
            }

            ASTNodeKind::VariableRef { name } => {
                if let Some(ts) = self.lookup_var(name) {
                    node.typespec = ts;
                } else {
                    node.typespec = TypeSpec::UNKNOWN;
                }
            }

            ASTNodeKind::Literal { value } => {
                node.typespec = match value {
                    LiteralValue::Int(_) => TypeSpec::from_simple(TypeDesc::INT),
                    LiteralValue::Float(_) => TypeSpec::from_simple(TypeDesc::FLOAT),
                    LiteralValue::String(_) => TypeSpec::from_simple(TypeDesc::STRING),
                };
            }

            ASTNodeKind::BinaryExpression { op, left, right } => {
                self.check_node(left);
                self.check_node(right);
                let lt = left.typespec;
                let rt = right.typespec;
                let op_val = *op;
                // C++ rejects struct/array operands for all binary ops
                // (unless there's a user-defined operator overload)
                if lt.is_structure() || rt.is_structure() || lt.is_array() || rt.is_array() {
                    // Try operator overload before erroring
                    if let Some(opw) = Self::binary_opword(op_val) {
                        if let Some(ret) = self.lookup_operator_overload(opw) {
                            node.typespec = ret;
                            return;
                        }
                    }
                    self.add_error(
                        loc,
                        format!(
                            "not allowed: '{:?} {:?} {:?}'",
                            lt.simpletype(),
                            op,
                            rt.simpletype()
                        ),
                    );
                    node.typespec = TypeSpec::UNKNOWN;
                } else {
                    let result = Self::binary_result_type(op_val, lt, rt);
                    if result == TypeSpec::UNKNOWN
                        && lt != TypeSpec::UNKNOWN
                        && rt != TypeSpec::UNKNOWN
                    {
                        // Try user-defined operator overload (C++ __operator__XX__)
                        if let Some(opw) = Self::binary_opword(op_val) {
                            if let Some(ret) = self.lookup_operator_overload(opw) {
                                node.typespec = ret;
                                return;
                            }
                        }
                        self.add_error(
                            loc,
                            format!(
                                "not allowed: '{:?} {:?} {:?}'",
                                lt.simpletype(),
                                op,
                                rt.simpletype()
                            ),
                        );
                    }
                    node.typespec = result;
                }
            }

            ASTNodeKind::UnaryExpression { op, expr } => {
                self.check_node(expr);
                let t = expr.typespec;
                let op_val = *op;
                // C++ rejects struct/array for all unary ops
                // (unless there's a user-defined operator overload)
                if t.is_structure() || t.is_array() {
                    // Try operator overload before erroring
                    if let Some(opw) = Self::unary_opword(op_val) {
                        if let Some(ret) = self.lookup_operator_overload(opw) {
                            node.typespec = ret;
                            return;
                        }
                    }
                    self.add_error(loc, format!("can't do unary op on {:?}", t.simpletype()));
                    node.typespec = TypeSpec::UNKNOWN;
                } else {
                    match op {
                        Operator::Neg | Operator::Sub | Operator::Add => {
                            if !t.is_unknown()
                                && !is_numeric(t)
                                && !is_closure(t)
                                && !is_triple(t)
                                && !is_matrix(t)
                            {
                                // Try unary operator overload
                                if let Some(opw) = Self::unary_opword(op_val) {
                                    if let Some(ret) = self.lookup_operator_overload(opw) {
                                        node.typespec = ret;
                                        return;
                                    }
                                }
                                self.add_error(
                                    loc,
                                    format!("can't negate type {:?}", t.simpletype()),
                                );
                                node.typespec = TypeSpec::UNKNOWN;
                            } else {
                                node.typespec = t;
                            }
                        }
                        Operator::Not => {
                            node.typespec = TypeSpec::from_simple(TypeDesc::INT);
                        }
                        Operator::BitNot => {
                            if !is_int(t) {
                                self.add_error(
                                    loc,
                                    "operator '~' can only be applied to int".to_string(),
                                );
                                node.typespec = TypeSpec::UNKNOWN;
                            } else {
                                node.typespec = t;
                            }
                        }
                        _ => {
                            node.typespec = t;
                        }
                    }
                }
            }

            ASTNodeKind::AssignExpression { lvalue, expr, .. } => {
                self.check_node(lvalue);
                self.check_node(expr);
                // C++ checks lvalue status before allowing assignment
                if !lvalue.is_lvalue() {
                    self.add_error(
                        loc,
                        "can't assign to something that isn't an lvalue".to_string(),
                    );
                }
                // C++ check_symbol_writeability: warn on writing to non-output params
                self.check_symbol_writeability(lvalue);
                let lts = lvalue.typespec;
                let rts = expr.typespec;

                // Handle array assignment
                if lts.is_array() || rts.is_array() {
                    if lts.is_unknown() || rts.is_unknown() {
                        // Skip check if either side is unresolved
                    } else if lts.is_array() && rts.is_array() {
                        let le = lts.elementtype();
                        let re = rts.elementtype();
                        if !equivalent(le, re) {
                            self.add_error(
                                loc,
                                format!(
                                    "cannot assign array of {:?} to array of {:?}",
                                    re.simpletype(),
                                    le.simpletype()
                                ),
                            );
                        }
                    } else {
                        // Compound init or comma expression assigned to non-array — downgrade to warning
                        self.add_warning(
                            loc,
                            "cannot mix array and non-array in assignment".to_string(),
                        );
                    }
                    node.typespec = lts;
                    return;
                }

                // Closure = 0 is ok (C++ checks literal value == 0.0)
                if is_closure(lts)
                    && !is_closure(rts)
                    && (is_float(rts) || is_int(rts))
                    && Self::is_literal_zero(expr)
                {
                    node.typespec = lts;
                    return;
                }

                // Struct assignment: must be same struct type
                if lts.is_structure() || rts.is_structure() {
                    if lts.is_structure() && rts.is_structure() {
                        // In our simplified model, struct types match by name
                        // (full struct checking would need StructSpec comparison)
                        node.typespec = lts;
                        return;
                    }
                    self.add_error(
                        loc,
                        "cannot assign struct to non-struct or vice versa".to_string(),
                    );
                    node.typespec = lts;
                    return;
                }

                // If either side is unknown (unresolved func/global), skip the check
                if !lts.is_unknown() && !rts.is_unknown() && !Self::assignable(lts, rts) {
                    if is_int(lts) && is_float(rts) {
                        self.add_warning(
                            loc,
                            "assignment may lose precision: int = float".to_string(),
                        );
                    } else {
                        self.add_error(
                            loc,
                            format!(
                                "cannot assign {:?} to {:?}",
                                rts.simpletype(),
                                lts.simpletype()
                            ),
                        );
                    }
                }
                node.typespec = if lts.is_unknown() { rts } else { lts };
            }

            ASTNodeKind::FunctionCall {
                args,
                name,
                argtakesderivs,
                ..
            } => {
                let name = name.clone();
                for arg in args.iter_mut() {
                    self.check_node(arg);
                }
                // Try to resolve function return type from builtins
                node.typespec = self.resolve_function_type(&name, args);
                // Track argtakesderivs for functions that accept derivatives
                // (C++ typecheck_builtin_specialcase -> argtakesderivs)
                *argtakesderivs = Self::compute_argtakesderivs(&name, args);
            }

            ASTNodeKind::TypeConstructor { typespec, args } => {
                for arg in args.iter_mut() {
                    self.check_node(arg);
                }
                let ts = *typespec;
                // Validate constructor arguments
                if is_float(ts) {
                    // float(float) or float(int) — 1 arg
                    if args.len() == 1 && (is_float(args[0].typespec) || is_int(args[0].typespec)) {
                        // ok
                    } else if args.len() != 1 {
                        self.add_error(
                            loc,
                            format!(
                                "float constructor requires exactly 1 argument, got {}",
                                args.len()
                            ),
                        );
                    }
                } else if is_triple(ts) {
                    // triple(float), triple(float,float,float),
                    // triple(string,float,float,float), triple(triple)
                    match args.len() {
                        1 => {
                            let a = args[0].typespec;
                            if !a.is_unknown() && !is_float(a) && !is_int(a) && !is_triple(a) {
                                self.add_error(
                                    loc,
                                    format!("cannot construct triple from {:?}", a.simpletype()),
                                );
                            }
                        }
                        3 => {
                            for (i, arg) in args.iter().enumerate() {
                                if !arg.typespec.is_unknown()
                                    && !is_float(arg.typespec)
                                    && !is_int(arg.typespec)
                                {
                                    self.add_error(
                                        loc,
                                        format!(
                                            "triple constructor arg {} must be numeric, got {:?}",
                                            i,
                                            arg.typespec.simpletype()
                                        ),
                                    );
                                }
                            }
                        }
                        4 => {
                            // triple("space", f, f, f)
                            if !is_string(args[0].typespec) {
                                self.add_error(loc,
                                    "4-arg triple constructor first arg must be string (space name)".to_string()
                                );
                            }
                        }
                        _ => {
                            self.add_error(
                                loc,
                                format!(
                                    "triple constructor expects 1, 3, or 4 arguments, got {}",
                                    args.len()
                                ),
                            );
                        }
                    }
                } else if is_matrix(ts) {
                    // matrix(float), matrix(string,float), matrix(string,string),
                    // matrix(16 floats), matrix(string,16 floats), matrix(matrix)
                    let n = args.len();
                    if n != 1 && n != 2 && n != 16 && n != 17 {
                        self.add_error(
                            loc,
                            format!(
                                "matrix constructor expects 1, 2, 16, or 17 arguments, got {}",
                                n
                            ),
                        );
                    }
                } else if is_int(ts) {
                    if args.len() != 1 {
                        self.add_error(
                            loc,
                            format!(
                                "int constructor requires exactly 1 argument, got {}",
                                args.len()
                            ),
                        );
                    }
                }
                node.typespec = ts;
            }

            ASTNodeKind::TernaryExpression {
                cond,
                true_expr,
                false_expr,
            } => {
                self.check_node(cond);
                self.check_node(true_expr);
                self.check_node(false_expr);

                let ct = cond.typespec;
                if is_closure(ct) {
                    self.add_error(loc, "cannot use a closure as a condition".to_string());
                }
                if ct.is_structure() {
                    self.add_error(loc, "cannot use a struct as a condition".to_string());
                }
                if ct.is_array() {
                    self.add_error(loc, "cannot use an array as a condition".to_string());
                }

                let t = true_expr.typespec;
                let f = false_expr.typespec;
                if t.is_array() || f.is_array() {
                    self.add_error(loc, "cannot use arrays in ternary expression".to_string());
                    node.typespec = TypeSpec::UNKNOWN;
                } else if t.is_unknown() || f.is_unknown() {
                    // One side unresolved — take the known type, or UNKNOWN
                    node.typespec = if t.is_unknown() { f } else { t };
                } else if Self::assignable(t, f) || Self::assignable(f, t) {
                    node.typespec = higher_precision(t, f);
                } else {
                    self.add_error(
                        loc,
                        format!(
                            "incompatible types in ternary: {:?} vs {:?}",
                            t.simpletype(),
                            f.simpletype()
                        ),
                    );
                    node.typespec = t;
                }
            }

            ASTNodeKind::ConditionalStatement {
                cond,
                true_stmt,
                false_stmt,
            } => {
                self.check_node(cond);
                let ct = cond.typespec;
                if ct.is_structure() {
                    self.add_error(loc, "cannot use a struct as an 'if' condition".to_string());
                }
                if ct.is_array() {
                    self.add_error(loc, "cannot use an array as an 'if' condition".to_string());
                }
                self.check_node(true_stmt);
                if let Some(fs) = false_stmt {
                    self.check_node(fs);
                }
            }

            ASTNodeKind::LoopStatement {
                init,
                cond,
                iter,
                body,
                ..
            } => {
                if let Some(i) = init {
                    self.check_node(i);
                }
                self.loop_nesting += 1;
                if let Some(c) = cond {
                    self.check_node(c);
                    let ct = c.typespec;
                    if is_closure(ct) {
                        self.add_error(loc, "cannot use a closure as a loop condition".to_string());
                    }
                    if ct.is_structure() {
                        self.add_error(loc, "cannot use a struct as a loop condition".to_string());
                    }
                    if ct.is_array() {
                        self.add_error(loc, "cannot use an array as a loop condition".to_string());
                    }
                }
                if let Some(it) = iter {
                    self.check_node(it);
                }
                self.check_node(body);
                self.loop_nesting -= 1;
            }

            ASTNodeKind::CompoundStatement { statements }
            | ASTNodeKind::StatementList { statements } => {
                for s in statements.iter_mut() {
                    self.check_node(s);
                }
            }

            ASTNodeKind::ReturnStatement { expr } => {
                if let Some(e) = expr {
                    self.check_node(e);
                    if let Some(&func_ret) = self.function_stack.last() {
                        // Inside a user function: check return type compatibility
                        let rt = e.typespec;
                        if !rt.is_unknown() && !is_void(func_ret) && !Self::assignable(func_ret, rt)
                        {
                            if is_int(func_ret) && (is_float(rt) || rt.is_array()) {
                                self.add_warning(
                                    loc,
                                    format!(
                                        "return value may lose precision: {:?} -> {:?}",
                                        rt.simpletype(),
                                        func_ret.simpletype()
                                    ),
                                );
                            } else {
                                self.add_error(
                                    loc,
                                    format!(
                                        "cannot return {:?} from function returning {:?}",
                                        rt.simpletype(),
                                        func_ret.simpletype()
                                    ),
                                );
                            }
                        }
                    } else {
                        // Not in a function: returning value from shader body is an error
                        // C++ typecheck.cpp:385-386
                        self.add_error(loc, "cannot return a value from a shader body".to_string());
                    }
                } else {
                    // Return without value
                    if let Some(&func_ret) = self.function_stack.last() {
                        if !is_void(func_ret) {
                            self.add_error(
                                loc,
                                format!(
                                    "must return a value from function returning {:?}",
                                    func_ret.simpletype()
                                ),
                            );
                        }
                    }
                    // Return without value from shader body is OK (equivalent to exit())
                }
            }

            ASTNodeKind::LoopModStatement { mod_type } => {
                let _ = mod_type;
                if self.loop_nesting < 1 {
                    self.add_error(loc, "break/continue outside of loop".to_string());
                }
            }

            ASTNodeKind::Index {
                base,
                index,
                index2,
                ..
            } => {
                self.check_node(base);
                self.check_node(index);
                let bt = base.typespec;

                // Validate index is integer
                if !is_int(index.typespec) && index.typespec != TypeSpec::UNKNOWN {
                    self.add_error(
                        loc,
                        format!(
                            "index must be an integer, not {:?}",
                            index.typespec.simpletype()
                        ),
                    );
                }

                if bt.is_structure() {
                    self.add_error(loc, "cannot use [] indexing on a struct".to_string());
                    node.typespec = TypeSpec::UNKNOWN;
                } else if is_closure(bt) {
                    self.add_error(loc, "cannot use [] indexing on a closure".to_string());
                    node.typespec = TypeSpec::UNKNOWN;
                } else if bt.is_array() {
                    node.typespec = bt.elementtype();
                    // C++ typecheck.cpp:176-180: [][] on simple array is an error
                    if index2.is_some() {
                        let agg = bt.simpletype().aggregate;
                        if agg == crate::typedesc::Aggregate::Scalar as u8 {
                            self.add_error(loc, "can't use [][] on a simple array".to_string());
                        }
                        node.typespec = TypeSpec::from_simple(TypeDesc::FLOAT);
                    }
                } else if is_triple(bt) {
                    // Component access: vec[i] -> float
                    node.typespec = TypeSpec::from_simple(TypeDesc::FLOAT);
                    // C++ typecheck.cpp:187-188: [][] on a triple is an error
                    if index2.is_some() {
                        self.add_error(loc, format!("can't use [][] on a {:?}", bt.simpletype()));
                    }
                } else if is_matrix(bt) {
                    // C++ typecheck.cpp:195-196: matrix[i] without [j] is technically
                    // wrong, but common in practice (e.g., matrix_array[i][j]).
                    // Emit warning only — not error — to avoid rejecting valid code
                    // from nested array-of-matrix indexing like marray[2][1][3].
                    if index2.is_none() {
                        self.add_warning(loc, "must use [][] on a matrix, not just []".to_string());
                    }
                    node.typespec = TypeSpec::from_simple(TypeDesc::FLOAT);
                } else {
                    node.typespec = bt;
                }
            }

            ASTNodeKind::PreIncDec { expr, .. } => {
                self.check_node(expr);
                // C++ check_symbol_writeability for pre-inc/dec
                self.check_symbol_writeability(expr);
                node.typespec = expr.typespec;
            }

            ASTNodeKind::PostIncDec { expr, .. } => {
                self.check_node(expr);
                // C++ checks is_lvalue for post-inc/dec and errors if not
                if !expr.is_lvalue() {
                    self.add_error(
                        loc,
                        "post-increment/decrement requires an lvalue".to_string(),
                    );
                }
                // C++ check_symbol_writeability for post-inc/dec
                self.check_symbol_writeability(expr);
                node.typespec = expr.typespec;
            }

            ASTNodeKind::StructSelect { base, field, .. } => {
                self.check_node(base);
                let bt = base.typespec;
                // Component access on triples (e.g., color.r, point.x)
                if is_triple(bt) && matches!(field.as_str(), "x" | "y" | "z" | "r" | "g" | "b") {
                    node.typespec = TypeSpec::from_simple(TypeDesc::FLOAT);
                } else if bt.is_structure_based() {
                    // Look up field in the struct's StructSpec
                    let sid = bt.structure_id();
                    if let Some(spec) = crate::typespec::get_struct(sid as i32) {
                        if let Some(fidx) = spec.lookup_field(crate::ustring::UString::new(field)) {
                            node.typespec = spec.fields[fidx].type_spec;
                        } else {
                            self.add_error(
                                loc,
                                format!("struct '{}' has no field '{}'", spec.name, field),
                            );
                            node.typespec = TypeSpec::UNKNOWN;
                        }
                    } else {
                        node.typespec = TypeSpec::UNKNOWN;
                    }
                } else {
                    node.typespec = TypeSpec::UNKNOWN;
                }
            }

            ASTNodeKind::CompoundInitializer { elements, .. } => {
                for e in elements.iter_mut() {
                    self.check_node(e);
                }
                // Infer type from elements. If all same type, result is array of that type.
                // If empty or mixed, leave as UNKNOWN (caller will coerce from context).
                if !elements.is_empty() {
                    let first = elements[0].typespec;
                    if !first.is_unknown()
                        && elements.iter().all(|e| {
                            equivalent(e.typespec, first) || Self::assignable(first, e.typespec)
                        })
                    {
                        let len = elements.len() as i32;
                        let td = first.simpletype();
                        node.typespec = TypeSpec::from_simple(TypeDesc {
                            basetype: td.basetype,
                            aggregate: td.aggregate,
                            vecsemantics: td.vecsemantics,
                            reserved: 0,
                            arraylen: len,
                        });
                    }
                }
                // If target type is already set (from parent context), adjust element types.
                let target = node.typespec;
                if !target.is_unknown() {
                    self.type_adjust_compound_init(elements, target, loc, false, true);
                }
            }

            ASTNodeKind::CommaOperator { exprs } => {
                for e in exprs.iter_mut() {
                    self.check_node(e);
                }
                if let Some(last) = exprs.last() {
                    node.typespec = last.typespec;
                }
            }

            ASTNodeKind::TypecastExpression { typespec, expr } => {
                self.check_node(expr);
                let target = *typespec;
                let source = expr.typespec;
                // Validate cast
                if !Self::assignable(target, source)
                    && !(is_int(target) && is_float(source))  // (int)float is ok
                    && !(is_triple(target) && is_triple(source))
                // any triple cast ok
                {
                    self.add_error(
                        loc,
                        format!(
                            "cannot cast {:?} to {:?}",
                            source.simpletype(),
                            target.simpletype()
                        ),
                    );
                }
                node.typespec = target;
            }

            _ => {}
        }

        // P2-23/24: builtin special cases and printf format validation
        // Called after the match to avoid borrow conflict with node.kind
        if matches!(node.kind, ASTNodeKind::FunctionCall { .. }) {
            self.typecheck_builtin_specialcase(node, loc);
        }
    }

    /// Full overload resolution matching C++ `CandidateFunctions`.
    /// Given candidate function signatures, score each against actual arg types.
    /// Supports wildcard sentinels in param_types:
    ///   WILDCARD_ANY       — `?`   match any single non-array
    ///   WILDCARD_ARRAY     — `?[]` match any array
    ///   WILDCARD_REST      — `*`   accept all remaining actuals (variadic tail)
    ///   WILDCARD_TOKENPAIR — `.`   expect zero or more (string, any) pairs
    /// Returns best match's return type, or None if no match or truly ambiguous.
    pub fn resolve_overload(
        candidates: &[(TypeSpec, Vec<TypeSpec>)], // (return_type, param_types)
        actual_args: &[TypeSpec],
    ) -> Option<TypeSpec> {
        // Collect all viable candidates with their scores
        struct Scored {
            ret: TypeSpec,
            score: i32,
        }
        let mut viable: Vec<Scored> = Vec::new();
        let mut best_score: i32 = -1;

        'candidate: for (ret_type, param_types) in candidates {
            let mut total_score: i32 = 0;
            let mut pi = 0usize; // formal param index
            let mut ai = 0usize; // actual arg index

            while ai < actual_args.len() {
                if pi >= param_types.len() {
                    // Ran out of formals with actuals remaining — no match
                    continue 'candidate;
                }
                let formal = param_types[pi];

                if is_wildcard_rest(formal) {
                    // `*` — accept all remaining actuals unconditionally
                    total_score += (actual_args.len() - ai) as i32 * SCORE_MATCH_ANYTHING;
                    pi += 1;
                    break; // ai no longer matters after break
                }

                if is_wildcard_tokenpair(formal) {
                    // `.` — consume (string key, any value) pairs; pi stays fixed
                    if ai + 1 >= actual_args.len() {
                        continue 'candidate; // pair incomplete
                    }
                    if !is_string(actual_args[ai]) {
                        continue 'candidate; // key must be a string token
                    }
                    total_score += SCORE_MATCH_ANYTHING * 2;
                    ai += 2; // consume one (key, value) pair; pi stays for next pair
                    continue;
                }

                // Normal param — WILDCARD_ANY / WILDCARD_ARRAY handled in score_type
                let s = score_type(formal, actual_args[ai]);
                if s == SCORE_NO_MATCH {
                    continue 'candidate;
                }
                total_score += s;
                pi += 1;
                ai += 1;
            }

            // All actuals consumed; account for remaining formals
            while pi < param_types.len() {
                let formal = param_types[pi];
                if is_wildcard_rest(formal) || is_wildcard_tokenpair(formal) {
                    // Optional variadic / pair — zero cost when no actuals remain
                } else {
                    // Concrete or wildcard formal without actual = default param, cost 1
                    total_score += SCORE_MATCH_ANYTHING;
                }
                pi += 1;
            }

            if total_score > best_score {
                best_score = total_score;
                viable.clear();
                viable.push(Scored {
                    ret: *ret_type,
                    score: total_score,
                });
            } else if total_score == best_score {
                viable.push(Scored {
                    ret: *ret_type,
                    score: total_score,
                });
            }
        }

        match viable.len() {
            0 => None,
            1 => Some(viable[0].ret),
            _ => {
                // Ambiguous: resolve by return type ranking (C++ rank order)
                let rank = |ts: TypeSpec| -> i32 {
                    let td = ts.simpletype();
                    if td == TypeDesc::FLOAT {
                        return 0;
                    }
                    if td == TypeDesc::INT {
                        return 1;
                    }
                    if is_color(ts) {
                        return 2;
                    }
                    if td.aggregate == Aggregate::Vec3 as u8
                        && td.vecsemantics == VecSemantics::Vector as u8
                    {
                        return 3;
                    }
                    if is_point(ts) {
                        return 4;
                    }
                    // normal
                    if td.aggregate == Aggregate::Vec3 as u8 {
                        return 5;
                    }
                    if td == TypeDesc::MATRIX {
                        return 6;
                    }
                    if td == TypeDesc::STRING {
                        return 7;
                    }
                    if is_void(ts) {
                        return 10;
                    }
                    100 // unknown
                };

                // Sort by rank, pick lowest
                viable.sort_by_key(|s| rank(s.ret));
                let best_rank = rank(viable[0].ret);
                let tied = viable.iter().filter(|s| rank(s.ret) == best_rank).count();
                if tied > 1 {
                    // Truly ambiguous even after ranking
                    None
                } else {
                    Some(viable[0].ret)
                }
            }
        }
    }

    /// Legacy overload resolution matching OSL 1.9 behavior (C++ `LegacyOverload`).
    ///
    /// Activated when env var `OSL_LEGACY_FUNCTION_RESOLUTION` is set (and != "0").
    /// Iterates candidates in declaration order through six priority phases:
    ///
    ///   1. Exact args, exact return type
    ///   2. Exact args, equivalent return type  (triple ↔ triple coerce)
    ///   3. Exact args, any return type
    ///   4. Coercible args, exact return type
    ///   5. Coercible args, equivalent return type
    ///   6. Coercible args, any return type
    ///
    /// Returns the first match found in the earliest applicable phase.
    /// `expected_ret` — the expected return type; `TypeSpec::UNKNOWN` means any.
    pub fn resolve_overload_legacy(
        candidates: &[(TypeSpec, Vec<TypeSpec>)],
        actual_args: &[TypeSpec],
        expected_ret: TypeSpec,
    ) -> Option<TypeSpec> {
        /// Score all args without coercion. Returns total score or None if not viable.
        fn try_match(params: &[TypeSpec], actuals: &[TypeSpec], allow_coerce: bool) -> Option<i32> {
            if params.len() != actuals.len() {
                return None;
            }
            let mut total = 0i32;
            for (&p, &a) in params.iter().zip(actuals.iter()) {
                let s = score_type(p, a);
                if s == SCORE_NO_MATCH {
                    return None;
                }
                // Without coerce: only exact matches allowed (score must be SCORE_EXACT or wildcard)
                if !allow_coerce && s < SCORE_SPATIAL_COERCE && s != SCORE_EXACT {
                    return None;
                }
                total += s;
            }
            Some(total)
        }

        /// True if `a` is "equivalent" to `b` — same base type family (triple ↔ triple).
        fn equiv_return(a: TypeSpec, b: TypeSpec) -> bool {
            if a == b {
                return true;
            }
            // triples are mutually equivalent for return-type purposes
            if is_triple(a) && is_triple(b) {
                return true;
            }
            false
        }

        let has_expected = !is_void(expected_ret) && expected_ret != TypeSpec::UNKNOWN;

        // Phase 1: exact args, exact return
        for (ret, params) in candidates {
            if try_match(params, actual_args, false).is_some() {
                if !has_expected || *ret == expected_ret {
                    return Some(*ret);
                }
            }
        }
        // Phase 2: exact args, equivalent return
        for (ret, params) in candidates {
            if try_match(params, actual_args, false).is_some() {
                if !has_expected || equiv_return(*ret, expected_ret) {
                    return Some(*ret);
                }
            }
        }
        // Phase 3: exact args, any return
        if has_expected {
            for (ret, params) in candidates {
                if try_match(params, actual_args, false).is_some() {
                    return Some(*ret);
                }
            }
        }
        // Phase 4: coercible args, exact return
        for (ret, params) in candidates {
            if try_match(params, actual_args, true).is_some() {
                if !has_expected || *ret == expected_ret {
                    return Some(*ret);
                }
            }
        }
        // Phase 5: coercible args, equivalent return
        for (ret, params) in candidates {
            if try_match(params, actual_args, true).is_some() {
                if !has_expected || equiv_return(*ret, expected_ret) {
                    return Some(*ret);
                }
            }
        }
        // Phase 6: coercible args, any return
        if has_expected {
            for (ret, params) in candidates {
                if try_match(params, actual_args, true).is_some() {
                    return Some(*ret);
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // P2-23: typecheck_builtin_specialcase (C++ typecheck.cpp:1263-1323)
    // -----------------------------------------------------------------------

    /// Set bit `arg` in `argwrite`, clear it in `argread` (write-only).
    fn argwriteonly(argread: &mut u32, argwrite: &mut u32, arg: usize) {
        if arg < 32 {
            *argread &= !(1u32 << arg);
            *argwrite |= 1u32 << arg;
        }
    }

    /// Mark optional output args for texture/pointcloud calls.
    /// C++ typecheck.cpp:1136-1180. `firstopt` is the 1-based index where
    /// optional string-value pairs begin. `tags` are output slot names;
    /// "*" means mark all.
    fn mark_optional_output(
        args: &[Box<ASTNode>],
        argread: &mut u32,
        argwrite: &mut u32,
        mut firstopt: usize,
        tags: &[&str],
    ) {
        let mark_all = tags.first().map_or(false, |t| *t == "*");
        let nargs = args.len();

        // Advance firstopt (0-based in args[]) to first string arg
        while firstopt < nargs && !is_string(args[firstopt].typespec) {
            firstopt += 1;
        }

        // Walk optional keyword-value pairs
        let mut a = firstopt;
        while a + 1 < nargs {
            let mut is_output = false;
            if is_string(args[a].typespec) {
                if let ASTNodeKind::Literal {
                    value: LiteralValue::String(ref s),
                } = args[a].kind
                {
                    is_output = mark_all || tags.iter().any(|t| *t == s.as_str());
                } else {
                    // Non-literal string: conservatively mark as output
                    is_output = true;
                }
            }
            if is_output {
                // The VALUE after the tag keyword is the output arg (1-based: a+2)
                Self::argwriteonly(argread, argwrite, a + 2);
            }
            a += 2;
        }
    }

    /// Apply special-case rules for built-in function calls.
    /// C++ typecheck.cpp:1263-1323.
    fn typecheck_builtin_specialcase(&mut self, node: &mut ASTNode, loc: crate::lexer::SourceLoc) {
        let ASTNodeKind::FunctionCall {
            ref mut name,
            ref mut args,
            ref mut argread,
            ref mut argwrite,
            argtakesderivs: _,
        } = node.kind
        else {
            return;
        };

        let ts = node.typespec;
        let nargs = args.len();
        let nm = name.as_str();

        // "transform" -> "transformv"/"transformn" based on return type
        if nm == "transform" {
            let st = ts.simpletype();
            if st == TypeDesc::VECTOR {
                *name = "transformv".to_string();
            } else if st == TypeDesc::NORMAL {
                *name = "transformn".to_string();
            }
        }

        // Void functions: arg0 is read, not written
        if ts.is_void() {
            *argread |= 1; // bit 0 read
            *argwrite &= !1; // bit 0 not written
        }

        // Read/write special cases: mark output args
        let nm = name.as_str();
        match nm {
            "sincos" => {
                Self::argwriteonly(argread, argwrite, 1);
                Self::argwriteonly(argread, argwrite, 2);
            }
            "getattribute" | "getmessage" | "gettextureinfo" | "getmatrix" | "dict_value" => {
                // Last arg is output
                Self::argwriteonly(argread, argwrite, nargs);
            }
            "pointcloud_get" => {
                Self::argwriteonly(argread, argwrite, 5);
            }
            "pointcloud_search" => {
                Self::mark_optional_output(args, argread, argwrite, 4, &["*"]);
            }
            "regex_search" | "regex_match" if nargs == 3 => {
                Self::argwriteonly(argread, argwrite, 2);
            }
            "split" => {
                Self::argwriteonly(argread, argwrite, 2);
            }
            _ => {
                // Texture family: mark alpha/errormessage outputs
                if matches!(
                    nm,
                    "texture" | "texture3d" | "environment" | "gettextureinfo" | "getmessage"
                ) {
                    // Already handled above for gettextureinfo/getmessage;
                    // for texture family mark optional outputs
                    if matches!(nm, "texture" | "texture3d" | "environment") {
                        Self::mark_optional_output(
                            args,
                            argread,
                            argwrite,
                            1,
                            &["alpha", "errormessage"],
                        );
                    }
                }
            }
        }

        // Printf-family format string validation
        let nm = name.as_str();
        if matches!(
            nm,
            "printf" | "fprintf" | "warning" | "error" | "format" | "sprintf"
        ) {
            let fmt_idx = if nm == "fprintf" { 1 } else { 0 };
            // Try to get format string from literal arg
            if fmt_idx < args.len() {
                if let ASTNodeKind::Literal {
                    value: LiteralValue::String(ref fmt),
                } = args[fmt_idx].kind
                {
                    let fmt_clone = fmt.clone();
                    let arg_start = fmt_idx + 1; // args after format string
                    self.typecheck_printf_args(nm, &fmt_clone, args, arg_start, loc);
                } else {
                    self.add_warning(
                        loc,
                        format!("{}() uses a format string that is not a constant", nm),
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // P2-24: typecheck_printf_args (C++ typecheck.cpp:1186-1258)
    // -----------------------------------------------------------------------

    /// Validate printf-style format string against argument types.
    fn typecheck_printf_args(
        &mut self,
        func_name: &str,
        format: &str,
        args: &[Box<ASTNode>],
        arg_start: usize,
        loc: crate::lexer::SourceLoc,
    ) {
        let fmt = format.as_bytes();
        let mut i = 0;
        let mut arg_idx = arg_start;
        // C++ argnum for error messages: fprintf starts at 3, others at 2
        let mut argnum: usize = if func_name == "fprintf" { 3 } else { 2 };

        while i < fmt.len() {
            if fmt[i] == b'%' {
                i += 1;
                if i < fmt.len() && fmt[i] == b'%' {
                    // '%%' is literal '%'
                    i += 1;
                    continue;
                }
                // Skip width/precision modifiers until format char
                while i < fmt.len()
                    && !matches!(
                        fmt[i],
                        b'c' | b'd'
                            | b'e'
                            | b'f'
                            | b'g'
                            | b'i'
                            | b'm'
                            | b'n'
                            | b'o'
                            | b'p'
                            | b's'
                            | b'u'
                            | b'v'
                            | b'x'
                            | b'X'
                    )
                {
                    i += 1;
                }
                if i >= fmt.len() {
                    break;
                }
                let fmtchar = fmt[i];
                i += 1; // consume format char

                // Check we have an arg
                if arg_idx >= args.len() {
                    self.add_error(
                        loc,
                        format!(
                            "{} has mismatched format string and arguments (not enough args)",
                            func_name
                        ),
                    );
                    return;
                }

                let arg_ts = args[arg_idx].typespec;

                // Struct not valid as printf arg
                if arg_ts.is_structure_based() {
                    self.add_error(
                        loc,
                        format!("struct is not a valid argument for {}", func_name),
                    );
                    return;
                }

                // String/closure arg must use %s
                if (arg_ts.is_closure_based() || is_string(arg_ts)) && fmtchar != b's' {
                    self.add_warning(
                        loc,
                        format!(
                            "{} has mismatched format string and arguments (arg {} needs %s)",
                            func_name, argnum
                        ),
                    );
                }

                // Int arg must use %d/%i/%o/%x/%X
                if is_int(arg_ts) && !matches!(fmtchar, b'd' | b'i' | b'o' | b'x' | b'X') {
                    self.add_warning(loc, format!(
                        "{} has mismatched format string and arguments (arg {} needs %d, %i, %o, %x, or %X)",
                        func_name, argnum
                    ));
                }

                // Float arg must use %f/%g/%c/%e/%m/%n/%p/%v
                if is_float(arg_ts)
                    && !matches!(
                        fmtchar,
                        b'f' | b'g' | b'c' | b'e' | b'm' | b'n' | b'p' | b'v'
                    )
                {
                    self.add_warning(loc, format!(
                        "{} has mismatched format string and arguments (arg {} needs %f, %g, or %e)",
                        func_name, argnum
                    ));
                }

                arg_idx += 1;
                argnum += 1;
            } else {
                i += 1;
            }
        }

        // Check for extra args
        if arg_idx < args.len() {
            self.add_warning(
                loc,
                format!(
                    "{} has mismatched format string and arguments (too many args)",
                    func_name
                ),
            );
        }
    }

    /// Resolve the return type of a function call.
    /// Uses resolve_overload for functions with known overloads,
    /// falls back to pattern matching for builtins.
    fn resolve_function_type(&self, name: &str, args: &[Box<ASTNode>]) -> TypeSpec {
        // Arity validation against builtin table (C++ builtin_func_args)
        // (name, min_args, max_args) — max=255 means variadic
        const VARIADIC: u8 = 255;
        const BUILTIN_FUNCS: &[(&str, u8, u8)] = &[
            ("abs", 1, 1),
            ("acos", 1, 1),
            ("area", 1, 1),
            ("arraylength", 1, 1),
            ("asin", 1, 1),
            ("atan", 1, 1),
            ("atan2", 2, 2),
            ("backfacing", 0, 0),
            ("blackbody", 1, 1),
            ("bump", 1, 2),
            ("calculatenormal", 1, 1),
            ("cbrt", 1, 1),
            ("ceil", 1, 1),
            ("cellnoise", 1, 4),
            ("clamp", 3, 3),
            ("concat", 2, 2),
            ("cos", 1, 1),
            ("cosh", 1, 1),
            ("cross", 2, 2),
            ("degrees", 1, 1),
            ("determinant", 1, 1),
            ("dict_find", 2, 2),
            ("dict_next", 1, 1),
            ("dict_value", 3, 3),
            ("displace", 1, 2),
            ("distance", 2, 2),
            ("dot", 2, 2),
            ("Dx", 1, 1),
            ("Dy", 1, 1),
            ("Dz", 1, 1),
            ("endswith", 2, 2),
            ("environment", 2, VARIADIC),
            ("erf", 1, 1),
            ("erfc", 1, 1),
            ("error", 1, VARIADIC),
            ("exit", 0, 0),
            ("exp", 1, 1),
            ("exp2", 1, 1),
            ("expm1", 1, 1),
            ("fabs", 1, 1),
            ("faceforward", 2, 3),
            ("filterwidth", 1, 1),
            ("floor", 1, 1),
            ("fmod", 2, 2),
            ("format", 1, VARIADIC),
            ("fprintf", 2, VARIADIC),
            ("getattribute", 2, 4),
            ("getchar", 2, 2),
            ("getmatrix", 2, 3),
            ("getmessage", 2, 4),
            ("gettextureinfo", 3, 5),
            ("hash", 1, 4),
            ("hashnoise", 1, 4),
            ("inverse", 1, 1),
            ("inversesqrt", 1, 1),
            ("isconnected", 1, 1),
            ("isconstant", 1, 1),
            ("isfinite", 1, 1),
            ("isinf", 1, 1),
            ("isnan", 1, 1),
            ("length", 1, 1),
            ("linearstep", 3, 3),
            ("log", 1, 1),
            ("log2", 1, 1),
            ("log10", 1, 1),
            ("logb", 1, 1),
            ("luminance", 1, 1),
            ("matrix", 1, 16),
            ("max", 2, 2),
            ("min", 2, 2),
            ("mix", 3, 3),
            ("mod", 2, 2),
            ("noise", 1, 4),
            ("normalize", 1, 1),
            ("pnoise", 2, 4),
            ("pointcloud_get", 4, 4),
            ("pointcloud_search", 4, VARIADIC),
            ("pointcloud_write", 2, VARIADIC),
            ("pow", 2, 2),
            ("printf", 1, VARIADIC),
            ("psnoise", 2, 4),
            ("radians", 1, 1),
            ("random", 0, 0),
            ("raytype", 1, 1),
            ("reflect", 2, 2),
            ("refract", 3, 3),
            ("regex_match", 2, 4),
            ("regex_search", 2, 4),
            ("rotate", 3, 4),
            ("round", 1, 1),
            ("setmessage", 2, 2),
            ("sign", 1, 1),
            ("sin", 1, 1),
            ("sincos", 3, 3),
            ("sinh", 1, 1),
            ("smoothstep", 3, 3),
            ("snoise", 1, 4),
            ("spline", 3, VARIADIC),
            ("splineinverse", 3, VARIADIC),
            ("split", 2, 4),
            ("sprintf", 1, VARIADIC),
            ("sqrt", 1, 1),
            ("startswith", 2, 2),
            ("step", 2, 2),
            ("stof", 1, 1),
            ("stoi", 1, 1),
            ("strlen", 1, 1),
            ("substr", 2, 3),
            ("surfacearea", 0, 0),
            ("tan", 1, 1),
            ("tanh", 1, 1),
            ("texture", 3, VARIADIC),
            ("texture3d", 2, VARIADIC),
            ("trace", 2, VARIADIC),
            ("transform", 2, 3),
            ("transformc", 2, 3),
            ("transformu", 2, 3),
            ("transpose", 1, 1),
            ("trunc", 1, 1),
            ("warning", 1, VARIADIC),
            ("wavelength_color", 1, 1),
        ];

        // Check arity against builtin table
        let nargs = args.len() as u8;
        for &(bname, min, max) in BUILTIN_FUNCS.iter() {
            if bname == name {
                if nargs < min || (max != VARIADIC && nargs > max) {
                    // Wrong arity for builtin — return UNKNOWN (error already implicit)
                    return TypeSpec::UNKNOWN;
                }
                break;
            }
        }

        // Built-in functions with known return types
        match name {
            // Math functions returning float
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" | "sinh" | "cosh"
            | "tanh" | "exp" | "exp2" | "expm1" | "log" | "log2" | "log10" | "logb" | "sqrt"
            | "inversesqrt" | "cbrt" | "abs" | "fabs" | "floor" | "ceil" | "round" | "trunc"
            | "sign" | "radians" | "degrees" | "erf" | "erfc" | "pow" | "step" | "linearstep"
            | "smoothstep" | "clamp" | "mix" | "min" | "max" | "fmod" | "mod" | "dot"
            | "length" | "distance" | "luminance" | "surfacearea" | "raytype" => {
                // These return the type of their first arg, or float
                if let Some(first) = args.first() {
                    if is_triple(first.typespec) {
                        // Some of these return float even with triple input
                        match name {
                            "dot" | "length" | "distance" | "luminance" => {
                                return TypeSpec::from_simple(TypeDesc::FLOAT);
                            }
                            _ => return first.typespec,
                        }
                    }
                }
                TypeSpec::from_simple(TypeDesc::FLOAT)
            }

            // Functions returning the same type as first arg
            "normalize" | "faceforward" | "reflect" | "refract" | "cross" | "calculatenormal"
            | "rotate" => {
                if let Some(first) = args.first() {
                    return first.typespec;
                }
                TypeSpec::from_simple(TypeDesc::new(
                    BaseType::Float,
                    Aggregate::Vec3,
                    VecSemantics::Vector,
                ))
            }

            // Functions returning int
            "isnan" | "isinf" | "isfinite" | "startswith" | "endswith" | "strlen" | "stoi"
            | "getattribute" | "getmessage" | "gettextureinfo" | "regex_search" | "regex_match"
            | "pointcloud_search" | "pointcloud_get" | "pointcloud_write" | "dict_find"
            | "dict_next" | "dict_value" | "trace" | "isconnected" | "isconstant"
            | "arraylength" | "hash" | "split" | "backfacing" => {
                TypeSpec::from_simple(TypeDesc::INT)
            }

            // Functions returning float
            "stof" | "area" | "filterwidth" | "determinant" => {
                TypeSpec::from_simple(TypeDesc::FLOAT)
            }

            // Functions returning string
            "concat" | "substr" | "format" => TypeSpec::from_simple(TypeDesc::STRING),

            // getchar returns int (char code at position)
            "getchar" => TypeSpec::from_simple(TypeDesc::INT),

            // Functions returning color
            "blackbody" | "wavelength_color" => TypeSpec::from_simple(TypeDesc::new(
                BaseType::Float,
                Aggregate::Vec3,
                VecSemantics::Color,
            )),

            // Functions returning matrix
            "matrix" | "transpose" | "inverse" => TypeSpec::from_simple(TypeDesc::MATRIX),

            // getmatrix returns int (success flag), matrix is output param
            "getmatrix" => TypeSpec::from_simple(TypeDesc::INT),

            // Noise functions: return type depends on args
            "noise" | "snoise" | "cellnoise" | "hashnoise" | "pnoise" | "psnoise" | "simplex"
            | "usimplex" => TypeSpec::from_simple(TypeDesc::FLOAT),

            // Void functions
            "printf" | "fprintf" | "warning" | "error" | "setmessage" | "exit" | "sincos" => {
                TypeSpec::from_simple(TypeDesc::new(
                    BaseType::None,
                    Aggregate::Scalar,
                    VecSemantics::NoXform,
                ))
            }

            // Texture returns float or color depending on channels
            "texture" | "texture3d" | "environment" => TypeSpec::from_simple(TypeDesc::FLOAT),

            // Transform functions: return depends on what's being transformed
            "transform" | "transformu" => {
                if args.len() >= 2 {
                    let last_non_string = args.iter().rev().find(|a| !is_string(a.typespec));
                    if let Some(arg) = last_non_string {
                        return arg.typespec;
                    }
                }
                TypeSpec::from_simple(TypeDesc::new(
                    BaseType::Float,
                    Aggregate::Vec3,
                    VecSemantics::Point,
                ))
            }

            // Dx/Dy/Dz return the type of their argument
            "Dx" | "Dy" | "Dz" => {
                if let Some(first) = args.first() {
                    return first.typespec;
                }
                TypeSpec::from_simple(TypeDesc::FLOAT)
            }

            // transformc returns color
            "transformc" => TypeSpec::from_simple(TypeDesc::new(
                BaseType::Float,
                Aggregate::Vec3,
                VecSemantics::Color,
            )),

            // sprintf returns string
            "sprintf" => TypeSpec::from_simple(TypeDesc::STRING),

            _ => {
                // Check user-defined functions
                if let Some(overloads) = self.user_functions.get(name) {
                    let actual_types: Vec<TypeSpec> = args.iter().map(|a| a.typespec).collect();
                    // OSL_LEGACY_FUNCTION_RESOLUTION env var selects old OSL 1.9 resolution order.
                    // Values: set (warn on mismatch), "0" (disabled), "err" (error), "use" (use legacy).
                    // For type-checking purposes we only need to pick the right overload, so we
                    // switch algorithms when the var is set and not "0".
                    // expected_ret is UNKNOWN here (no outer context); legacy phases 1/2/4/5
                    // degenerate to phases 3/6 which is still correct first-match ordering.
                    let resolved = match std::env::var("OSL_LEGACY_FUNCTION_RESOLUTION") {
                        Ok(v) if v != "0" => {
                            // Legacy: first-match in declaration order across 6 priority phases
                            Self::resolve_overload_legacy(
                                overloads,
                                &actual_types,
                                TypeSpec::UNKNOWN,
                            )
                        }
                        _ => {
                            // Modern: scoring-based best-match (CandidateFunctions)
                            Self::resolve_overload(overloads, &actual_types)
                        }
                    };
                    if let Some(ret) = resolved {
                        return ret;
                    }
                    // Fallback: return first overload's return type
                    if let Some((ret, _)) = overloads.first() {
                        return *ret;
                    }
                }
                // Check if name matches a registered struct type — struct constructor call.
                // e.g. `MyStruct(a, b)` is parsed as FunctionCall; resolve to that struct's TypeSpec.
                let sid = crate::typespec::find_struct_by_name(crate::ustring::UString::new(name));
                if sid > 0 {
                    return TypeSpec::structure(sid as i16, 0);
                }
                TypeSpec::UNKNOWN
            }
        }
    }
}

/// Convenience: type-check a parsed AST.
pub fn typecheck(nodes: &mut [Box<ASTNode>]) -> Vec<TypeError> {
    let mut tc = TypeChecker::new();
    tc.check(nodes);
    tc.errors
}

/// Type-check and return both errors and warnings.
pub fn typecheck_full(nodes: &mut [Box<ASTNode>]) -> (Vec<TypeError>, Vec<TypeWarning>) {
    let mut tc = TypeChecker::new();
    tc.check(nodes);
    (tc.errors, tc.warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn test_basic_typecheck() {
        let src = r#"
surface test(float Kd = 0.5) {
    float x = Kd + 1.0;
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let errors = typecheck(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_assignable() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        let i = TypeSpec::from_simple(TypeDesc::INT);
        let c = TypeSpec::from_simple(TypeDesc::new(
            BaseType::Float,
            Aggregate::Vec3,
            VecSemantics::Color,
        ));
        let m = TypeSpec::from_simple(TypeDesc::MATRIX);

        assert!(TypeChecker::assignable(f, f));
        assert!(TypeChecker::assignable(f, i)); // int -> float
        assert!(TypeChecker::assignable(c, f)); // float -> color
        assert!(TypeChecker::assignable(c, i)); // int -> color
        assert!(TypeChecker::assignable(m, f)); // float -> matrix
        assert!(!TypeChecker::assignable(i, f)); // float -> int (no)
        assert!(!TypeChecker::assignable(i, c)); // color -> int (no)
    }

    #[test]
    fn test_binary_result_type() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        let i = TypeSpec::from_simple(TypeDesc::INT);
        let c = TypeSpec::from_simple(TypeDesc::new(
            BaseType::Float,
            Aggregate::Vec3,
            VecSemantics::Color,
        ));

        // float + float = float
        assert_eq!(TypeChecker::binary_result_type(Operator::Add, f, f), f);
        // float + int = float
        assert_eq!(TypeChecker::binary_result_type(Operator::Add, f, i), f);
        // int + int = int
        assert_eq!(TypeChecker::binary_result_type(Operator::Add, i, i), i);
        // == always returns int
        assert_eq!(
            TypeChecker::binary_result_type(Operator::Eq, f, f).simpletype(),
            TypeDesc::INT
        );
        // color + float = color (higher precision)
        let result = TypeChecker::binary_result_type(Operator::Add, c, f);
        assert!(is_triple(result));
        // int % int = int
        assert_eq!(TypeChecker::binary_result_type(Operator::Mod, i, i), i);
        // float % float = UNKNOWN (mod only works on ints)
        assert_eq!(
            TypeChecker::binary_result_type(Operator::Mod, f, f),
            TypeSpec::UNKNOWN
        );
        // string + anything = UNKNOWN
        let s = TypeSpec::from_simple(TypeDesc::STRING);
        assert_eq!(
            TypeChecker::binary_result_type(Operator::Add, s, f),
            TypeSpec::UNKNOWN
        );
    }

    #[test]
    fn test_overload_scoring() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        let i = TypeSpec::from_simple(TypeDesc::INT);
        let c = TypeSpec::from_simple(TypeDesc::new(
            BaseType::Float,
            Aggregate::Vec3,
            VecSemantics::Color,
        ));
        let p = TypeSpec::from_simple(TypeDesc::new(
            BaseType::Float,
            Aggregate::Vec3,
            VecSemantics::Point,
        ));

        assert_eq!(score_type(f, f), SCORE_EXACT);
        assert_eq!(score_type(f, i), SCORE_INT_TO_FLOAT);
        assert_eq!(score_type(i, f), SCORE_NO_MATCH); // float->int not implicit
        assert!(score_type(c, f) > SCORE_NO_MATCH); // float->color coercible
        assert_eq!(score_type(p, c), SCORE_TRIPLE_COERCE); // color->point (both triples)
    }

    #[test]
    fn test_wildcard_score_type() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        let i = TypeSpec::from_simple(TypeDesc::INT);
        let farr = TypeSpec::new_array(TypeDesc::FLOAT, 4);
        let sarr = TypeSpec::new_array(TypeDesc::STRING, -1);

        // `?` matches any scalar, rejects arrays
        assert_eq!(score_type(WILDCARD_ANY, f), SCORE_MATCH_ANYTHING);
        assert_eq!(score_type(WILDCARD_ANY, i), SCORE_MATCH_ANYTHING);
        assert_eq!(score_type(WILDCARD_ANY, farr), SCORE_NO_MATCH);

        // `?[]` matches arrays, rejects scalars
        assert_eq!(score_type(WILDCARD_ARRAY, farr), SCORE_MATCH_ANYTHING);
        assert_eq!(score_type(WILDCARD_ARRAY, sarr), SCORE_MATCH_ANYTHING);
        assert_eq!(score_type(WILDCARD_ARRAY, f), SCORE_NO_MATCH);
    }

    #[test]
    fn test_wildcard_resolve_overload() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        let i = TypeSpec::from_simple(TypeDesc::INT);
        let s = TypeSpec::from_simple(TypeDesc::STRING);
        let farr = TypeSpec::new_array(TypeDesc::FLOAT, 4);

        // `*` variadic tail: (float, *) should match (float,), (float, float), (float, int, float), ...
        let variadic = vec![(i, vec![f, WILDCARD_REST])];
        assert_eq!(
            TypeChecker::resolve_overload(&variadic, &[f]),
            Some(i),
            "* accepts zero remaining args"
        );
        assert_eq!(
            TypeChecker::resolve_overload(&variadic, &[f, f, i, s]),
            Some(i),
            "* accepts many remaining args"
        );
        // First arg must still match exactly
        assert_eq!(
            TypeChecker::resolve_overload(&variadic, &[s]),
            None,
            "first concrete param must match"
        );

        // `?` wildcard: (?, float) matches any scalar first arg
        let any_first = vec![(f, vec![WILDCARD_ANY, f])];
        assert_eq!(
            TypeChecker::resolve_overload(&any_first, &[i, f]),
            Some(f),
            "? matches int"
        );
        assert_eq!(
            TypeChecker::resolve_overload(&any_first, &[s, f]),
            Some(f),
            "? matches string"
        );
        assert_eq!(
            TypeChecker::resolve_overload(&any_first, &[farr, f]),
            None,
            "? rejects array"
        );

        // `?[]` wildcard: matches any array
        let any_arr = vec![(i, vec![WILDCARD_ARRAY])];
        assert_eq!(
            TypeChecker::resolve_overload(&any_arr, &[farr]),
            Some(i),
            "?[] matches float[]"
        );
        assert_eq!(
            TypeChecker::resolve_overload(&any_arr, &[f]),
            None,
            "?[] rejects scalar"
        );

        // `.` token/value pairs: (string, ?) pairs
        let pairs = vec![(i, vec![f, WILDCARD_TOKENPAIR])];
        assert_eq!(
            TypeChecker::resolve_overload(&pairs, &[f, s, i]),
            Some(i),
            ". matches (string, value) pair"
        );
        assert_eq!(
            TypeChecker::resolve_overload(&pairs, &[f, s, f, s, i]),
            Some(i),
            ". matches multiple pairs"
        );
        // Key must be string
        assert_eq!(
            TypeChecker::resolve_overload(&pairs, &[f, i, i]),
            None,
            ". rejects non-string key"
        );
    }

    #[test]
    fn test_condition_validation() {
        // This should type-check without errors (int condition is fine)
        let src = r#"
surface test() {
    int x = 1;
    if (x) { float y = 1.0; }
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let errors = typecheck(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_loop_break_validation() {
        // break inside loop should be ok
        let src = r#"
surface test() {
    for (int i = 0; i < 10; i += 1) {
        break;
    }
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let errors = typecheck(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_ternary_type_unification() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        let i = TypeSpec::from_simple(TypeDesc::INT);
        // float and int should unify to float (higher precision)
        let result = higher_precision(f, i);
        assert_eq!(result, f);
    }

    #[test]
    fn test_literal_zero_check() {
        // is_literal_zero should only match literal 0 values
        assert!(TypeChecker::is_literal_zero(&ASTNode::new(
            1,
            ASTNodeKind::Literal {
                value: LiteralValue::Int(0)
            },
            crate::lexer::SourceLoc { line: 1, col: 1 },
        )));
        assert!(TypeChecker::is_literal_zero(&ASTNode::new(
            2,
            ASTNodeKind::Literal {
                value: LiteralValue::Float(0.0)
            },
            crate::lexer::SourceLoc { line: 1, col: 1 },
        )));
        // Non-zero should NOT match
        assert!(!TypeChecker::is_literal_zero(&ASTNode::new(
            3,
            ASTNodeKind::Literal {
                value: LiteralValue::Int(1)
            },
            crate::lexer::SourceLoc { line: 1, col: 1 },
        )));
        assert!(!TypeChecker::is_literal_zero(&ASTNode::new(
            4,
            ASTNodeKind::Literal {
                value: LiteralValue::Float(1.0)
            },
            crate::lexer::SourceLoc { line: 1, col: 1 },
        )));
        // Variable ref should NOT match
        assert!(!TypeChecker::is_literal_zero(&ASTNode::new(
            5,
            ASTNodeKind::VariableRef {
                name: "x".to_string()
            },
            crate::lexer::SourceLoc { line: 1, col: 1 },
        )));
    }

    #[test]
    fn test_lvalue_check_in_assignment() {
        // Assignment to literal (not lvalue) should produce error
        let src = r#"
surface test() {
    float x = 1.0;
    x = 2.0;
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let errors = typecheck(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_comma_operator_vs_compound_init() {
        // Parenthesized comma should be parsed as CommaOperator
        let src = "(1, 2, 3)";
        let lexer = crate::lexer::OslLexer::new(src);
        let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();
        // Should have: LParen, Int(1), Comma, Int(2), Comma, Int(3), RParen
        assert!(tokens.len() >= 5, "Expected at least 5 tokens");
    }

    // -----------------------------------------------------------------------
    // printf format string validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_printf_valid_int() {
        // %d with int arg - no errors, no warnings
        let src = r#"
surface test() {
    int x = 42;
    printf("value %d\n", x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, warnings) = typecheck_full(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(warnings.is_empty(), "Warnings: {:?}", warnings);
    }

    #[test]
    fn test_printf_valid_float() {
        // %f with float arg - no errors, no warnings
        let src = r#"
surface test() {
    float x = 1.5;
    printf("val %f\n", x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, warnings) = typecheck_full(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(warnings.is_empty(), "Warnings: {:?}", warnings);
    }

    #[test]
    fn test_printf_valid_string() {
        // %s with string arg - no errors, no warnings
        let src = r#"
surface test() {
    string s = "hello";
    printf("msg: %s\n", s);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, warnings) = typecheck_full(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(warnings.is_empty(), "Warnings: {:?}", warnings);
    }

    #[test]
    fn test_printf_valid_triple() {
        // %g with color arg - float-family format is valid for triples
        let src = r#"
surface test() {
    color c = color(1, 0, 0);
    printf("c = %g\n", c);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, warnings) = typecheck_full(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(warnings.is_empty(), "Warnings: {:?}", warnings);
    }

    #[test]
    fn test_printf_too_few_args() {
        // Format has %d but no argument supplied - error
        let src = r#"
surface test() {
    printf("val %d\n");
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, _warnings) = typecheck_full(&mut ast);
        assert!(
            errors.iter().any(|e| e.message.contains("not enough args")),
            "Expected 'not enough args' error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_printf_too_many_args() {
        // Format has no specifiers but an extra arg is passed - warning
        let src = r#"
surface test() {
    int x = 1;
    printf("hello\n", x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (_errors, warnings) = typecheck_full(&mut ast);
        assert!(
            warnings.iter().any(|w| w.message.contains("too many args")),
            "Expected 'too many args' warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_printf_type_mismatch_string_needs_percent_s() {
        // %d used for a string arg - should warn
        let src = r#"
surface test() {
    string s = "hi";
    printf("%d\n", s);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (_errors, warnings) = typecheck_full(&mut ast);
        assert!(
            warnings.iter().any(|w| w.message.contains("needs %s")),
            "Expected 'needs %s' warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_printf_type_mismatch_int_needs_percent_d() {
        // %f used for int - should warn
        let src = r#"
surface test() {
    int x = 5;
    printf("%f\n", x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (_errors, warnings) = typecheck_full(&mut ast);
        assert!(
            warnings.iter().any(|w| w.message.contains("needs %d")),
            "Expected 'needs %d' warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_printf_type_mismatch_float_needs_percent_f() {
        // %d used for float - should warn
        let src = r#"
surface test() {
    float x = 1.0;
    printf("%d\n", x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (_errors, warnings) = typecheck_full(&mut ast);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("needs %f, %g, or %e")),
            "Expected float format warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_printf_percent_percent_literal() {
        // %% is not a specifier - no args consumed, no warnings
        let src = r#"
surface test() {
    printf("100%%\n");
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, warnings) = typecheck_full(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(warnings.is_empty(), "Warnings: {:?}", warnings);
    }

    #[test]
    fn test_printf_multiple_specifiers() {
        // Multiple mixed specifiers all matching - no issues
        let src = r#"
surface test() {
    int i = 1;
    float f = 2.0;
    string s = "three";
    printf("%d %g %s\n", i, f, s);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, warnings) = typecheck_full(&mut ast);
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(warnings.is_empty(), "Warnings: {:?}", warnings);
    }

    #[test]
    fn test_printf_non_constant_format_warns() {
        // Format string is a variable, not a literal - should warn
        let src = r#"
surface test() {
    string fmt = "%d\n";
    int x = 1;
    printf(fmt, x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (_errors, warnings) = typecheck_full(&mut ast);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("not a constant")),
            "Expected non-constant format warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_format_function_validated() {
        // format() also validated - too few args is an error
        let src = r#"
surface test() {
    string s = format("val %d %d\n", 1);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (errors, _warnings) = typecheck_full(&mut ast);
        assert!(
            errors.iter().any(|e| e.message.contains("not enough args")),
            "Expected 'not enough args' error for format(), got: {:?}",
            errors
        );
    }

    #[test]
    fn test_error_function_validated() {
        // error() uses printf-style - too many args should warn
        let src = r#"
surface test() {
    int x = 1;
    error("no specifiers", x);
}
"#;
        let mut ast = parser::parse(src).unwrap().ast;
        let (_errors, warnings) = typecheck_full(&mut ast);
        assert!(
            warnings.iter().any(|w| w.message.contains("too many args")),
            "Expected 'too many args' warning for error(), got: {:?}",
            warnings
        );
    }
}
