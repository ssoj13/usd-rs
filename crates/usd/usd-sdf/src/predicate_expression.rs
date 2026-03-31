//! Predicate expressions for filtering objects.
//!
//! Port of pxr/usd/sdf/predicateExpression.h
//!
//! Predicate expressions represent logical expressions consisting of predicate
//! function calls joined by logical operators:
//! - `and` - logical AND
//! - `or` - logical OR
//! - `not` - logical NOT
//! - whitespace - implied AND
//!
//! # Syntax
//!
//! Three syntaxes for function calls:
//! - Bare call: `isDefined`
//! - Colon call: `isa:mammal,bird`
//! - Paren call: `isClose(1.23, tolerance=0.01)`
//!
//! # Examples
//!
//! - `foo` - call "foo" with no arguments
//! - `foo bar` - implicit AND of "foo" and "bar"
//! - `color:red (shiny or matte)`
//! - `(mammal or bird) and (tame or small)`

use std::fmt;
use usd_vt::Value;

/// Function call style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FnCallKind {
    /// No-arg call like `active`.
    BareCall,
    /// Colon-separated positional args like `isa:Imageable`.
    ColonCall,
    /// Paren/comma with pos/kw args like `foo(23, bar=baz)`.
    ParenCall,
}

/// A function argument (positional or keyword).
#[derive(Debug, Clone, PartialEq)]
pub struct FnArg {
    /// Argument name (empty for positional).
    pub name: String,
    /// Argument value.
    pub value: Value,
}

impl FnArg {
    /// Creates a positional argument.
    pub fn positional(value: Value) -> Self {
        Self {
            name: String::new(),
            value,
        }
    }

    /// Creates a keyword argument.
    pub fn keyword(name: impl Into<String>, value: Value) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    /// Returns true if this is a positional argument.
    pub fn is_positional(&self) -> bool {
        self.name.is_empty()
    }

    /// Returns true if this is a keyword argument.
    pub fn is_keyword(&self) -> bool {
        !self.name.is_empty()
    }
}

/// A function call in a predicate expression.
#[derive(Debug, Clone, PartialEq)]
pub struct FnCall {
    /// Calling style.
    pub kind: FnCallKind,
    /// Function name.
    pub func_name: String,
    /// Function arguments.
    pub args: Vec<FnArg>,
}

impl FnCall {
    /// Creates a bare call (no arguments).
    pub fn bare(name: impl Into<String>) -> Self {
        Self {
            kind: FnCallKind::BareCall,
            func_name: name.into(),
            args: Vec::new(),
        }
    }

    /// Creates a colon call with positional arguments.
    pub fn colon(name: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            kind: FnCallKind::ColonCall,
            func_name: name.into(),
            args: args.into_iter().map(FnArg::positional).collect(),
        }
    }

    /// Creates a paren call with mixed arguments.
    pub fn paren(name: impl Into<String>, args: Vec<FnArg>) -> Self {
        Self {
            kind: FnCallKind::ParenCall,
            func_name: name.into(),
            args,
        }
    }

    /// Returns the function name.
    pub fn name(&self) -> &str {
        &self.func_name
    }

    /// Returns the function arguments.
    pub fn args(&self) -> &[FnArg] {
        &self.args
    }

    /// Returns the calling style.
    pub fn kind(&self) -> FnCallKind {
        self.kind
    }
}

/// Logical operation in predicate expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredicateOp {
    /// Logical NOT.
    Not,
    /// Implied AND (whitespace).
    ImpliedAnd,
    /// Explicit AND.
    And,
    /// Logical OR.
    Or,
    /// Function call leaf.
    Call,
}

/// A node in the predicate expression tree.
#[derive(Debug, Clone, PartialEq)]
enum PredicateNode {
    /// An operation with children indices.
    Op {
        op: PredicateOp,
        left: usize,
        right: Option<usize>,
    },
    /// A function call leaf.
    Call(usize),
}

/// A predicate expression for filtering objects.
///
/// Expressions can be constructed from strings or built programmatically.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PredicateExpression {
    /// Expression tree nodes.
    nodes: Vec<PredicateNode>,
    /// Function calls in the expression.
    calls: Vec<FnCall>,
    /// Parse error, if any.
    parse_error: Option<String>,
}

impl PredicateExpression {
    /// Creates an empty expression (evaluates to false).
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses an expression from a string.
    pub fn parse(expr: &str) -> Self {
        Self::parse_with_context(expr, None)
    }

    /// Parses an expression with optional context for error messages.
    pub fn parse_with_context(expr: &str, context: Option<&str>) -> Self {
        let expr = expr.trim();

        if expr.is_empty() {
            return Self::new();
        }

        // Parse full grammar
        match parse_predicate_expression(expr) {
            Ok(result) => result,
            Err(err_msg) => {
                let full_err = if let Some(ctx) = context {
                    format!("{}: {}", ctx, err_msg)
                } else {
                    err_msg
                };
                Self {
                    nodes: Vec::new(),
                    calls: Vec::new(),
                    parse_error: Some(full_err),
                }
            }
        }
    }

    /// Creates an expression from a single function call.
    pub fn make_call(call: FnCall) -> Self {
        Self {
            nodes: vec![PredicateNode::Call(0)],
            calls: vec![call],
            parse_error: None,
        }
    }

    /// Creates the logical NOT of an expression.
    pub fn make_not(expr: Self) -> Self {
        if expr.is_empty() {
            return expr;
        }

        let mut result = expr;
        let root = result.nodes.len().saturating_sub(1);
        result.nodes.push(PredicateNode::Op {
            op: PredicateOp::Not,
            left: root,
            right: None,
        });
        result
    }

    /// Creates a binary logical operation.
    pub fn make_op(op: PredicateOp, left: Self, right: Self) -> Self {
        match op {
            PredicateOp::Not => Self::make_not(left),
            PredicateOp::Call => left,
            PredicateOp::ImpliedAnd | PredicateOp::And | PredicateOp::Or => {
                if left.is_empty() {
                    return right;
                }
                if right.is_empty() {
                    return left;
                }

                let mut result = Self::new();

                // Merge calls
                let left_call_offset = 0;
                let right_call_offset = left.calls.len();
                let left_nodes_len = left.nodes.len();
                let right_nodes_len = right.nodes.len();

                result.calls.extend(left.calls);
                result.calls.extend(right.calls);

                // Merge nodes with adjusted indices
                let left_node_offset = 0;
                let right_node_offset = left_nodes_len;

                for node in left.nodes {
                    result.nodes.push(adjust_predicate_node(
                        node,
                        left_call_offset,
                        left_node_offset,
                    ));
                }
                for node in right.nodes {
                    result.nodes.push(adjust_predicate_node(
                        node,
                        right_call_offset,
                        right_node_offset,
                    ));
                }

                // Add the operation node
                // Left root is the last node of the left expression
                let left_root = left_node_offset + left_nodes_len - 1;
                // Right root is the last node of the right expression
                let right_root = right_node_offset + right_nodes_len - 1;
                result.nodes.push(PredicateNode::Op {
                    op,
                    left: left_root,
                    right: Some(right_root),
                });

                result
            }
        }
    }

    /// Returns true if this is an empty expression.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.parse_error.is_none()
    }

    /// Returns the parse error, if any.
    pub fn parse_error(&self) -> Option<&str> {
        self.parse_error.as_deref()
    }

    /// Returns the function calls in this expression.
    pub fn calls(&self) -> &[FnCall] {
        &self.calls
    }

    /// Returns true if this expression has logical operations (NOT, AND, OR).
    ///
    /// A bare call like "isDefined" has no operations, while "not isDefined"
    /// or "foo and bar" have operations.
    pub fn has_operations(&self) -> bool {
        self.nodes
            .iter()
            .any(|n| matches!(n, PredicateNode::Op { .. }))
    }

    /// Walks the expression tree.
    ///
    /// Calls `logic` for each operation and `call_fn` for each function call.
    pub fn walk<F, G>(&self, mut logic: F, mut call_fn: G)
    where
        F: FnMut(PredicateOp, i32),
        G: FnMut(&FnCall),
    {
        if self.nodes.is_empty() {
            return;
        }

        self.walk_node(self.nodes.len() - 1, &mut logic, &mut call_fn);
    }

    fn walk_node<F, G>(&self, idx: usize, logic: &mut F, call_fn: &mut G)
    where
        F: FnMut(PredicateOp, i32),
        G: FnMut(&FnCall),
    {
        match &self.nodes[idx] {
            PredicateNode::Call(call_idx) => {
                call_fn(&self.calls[*call_idx]);
            }
            PredicateNode::Op { op, left, right } => {
                logic(*op, 0);
                self.walk_node(*left, logic, call_fn);
                if let Some(right_idx) = right {
                    logic(*op, 1);
                    self.walk_node(*right_idx, logic, call_fn);
                }
                logic(*op, if right.is_some() { 2 } else { 1 });
            }
        }
    }

    /// Walks the expression tree, providing the full op stack context.
    ///
    /// Similar to `walk()` but the `logic` callback receives the entire
    /// op stack (as a slice of `(PredicateOp, i32)`) instead of just the
    /// current op and arg_index. The top of the stack is the last element.
    ///
    /// Matches C++ `SdfPredicateExpression::WalkWithOpStack()`.
    pub fn walk_with_op_stack<F, G>(&self, mut logic: F, mut call_fn: G)
    where
        F: FnMut(&[(PredicateOp, i32)]),
        G: FnMut(&FnCall),
    {
        if self.nodes.is_empty() {
            return;
        }

        let mut op_stack: Vec<(PredicateOp, i32)> = Vec::new();
        self.walk_node_with_stack(
            self.nodes.len() - 1,
            &mut op_stack,
            &mut logic,
            &mut call_fn,
        );
    }

    fn walk_node_with_stack<F, G>(
        &self,
        idx: usize,
        op_stack: &mut Vec<(PredicateOp, i32)>,
        logic: &mut F,
        call_fn: &mut G,
    ) where
        F: FnMut(&[(PredicateOp, i32)]),
        G: FnMut(&FnCall),
    {
        match &self.nodes[idx] {
            PredicateNode::Call(call_idx) => {
                call_fn(&self.calls[*call_idx]);
            }
            PredicateNode::Op { op, left, right } => {
                op_stack.push((*op, 0));
                logic(op_stack);

                self.walk_node_with_stack(*left, op_stack, logic, call_fn);

                if let Some(right_idx) = right {
                    if let Some(last) = op_stack.last_mut() {
                        last.1 = 1;
                    }
                    logic(op_stack);
                    self.walk_node_with_stack(*right_idx, op_stack, logic, call_fn);
                }

                if let Some(last) = op_stack.last_mut() {
                    last.1 = if right.is_some() { 2 } else { 1 };
                }
                logic(op_stack);

                op_stack.pop();
            }
        }
    }

    /// Returns a text representation of this expression that parses
    /// to the same expression.
    ///
    /// Matches C++ `SdfPredicateExpression::GetText()`.
    pub fn get_text(&self) -> String {
        if self.is_empty() || self.nodes.is_empty() {
            return String::new();
        }
        self.node_to_text(self.nodes.len() - 1, None)
    }

    fn node_to_text(&self, idx: usize, parent_op: Option<PredicateOp>) -> String {
        match &self.nodes[idx] {
            PredicateNode::Call(call_idx) => Self::format_fn_call(&self.calls[*call_idx]),
            PredicateNode::Op { op, left, right } => match op {
                PredicateOp::Not => {
                    let inner = self.node_to_text(*left, Some(PredicateOp::Not));
                    format!("not {}", inner)
                }
                PredicateOp::Call => String::new(),
                _ => {
                    let right_idx = right.expect("binary op must have right child");
                    let left_str = self.node_to_text(*left, Some(*op));
                    let right_str = self.node_to_text(right_idx, Some(*op));

                    let op_str = match op {
                        PredicateOp::And => " and ",
                        PredicateOp::ImpliedAnd => " ",
                        PredicateOp::Or => " or ",
                        _ => unreachable!(),
                    };

                    let inner = format!("{}{}{}", left_str, op_str, right_str);

                    let needs_parens = if let Some(parent) = parent_op {
                        Self::op_precedence(*op) < Self::op_precedence(parent)
                    } else {
                        false
                    };

                    if needs_parens {
                        format!("({})", inner)
                    } else {
                        inner
                    }
                }
            },
        }
    }

    fn op_precedence(op: PredicateOp) -> i32 {
        match op {
            PredicateOp::Or => 1,
            PredicateOp::And => 2,
            PredicateOp::ImpliedAnd => 3,
            PredicateOp::Not => 4,
            PredicateOp::Call => 5,
        }
    }

    fn format_fn_call(call: &FnCall) -> String {
        match call.kind {
            FnCallKind::BareCall => call.func_name.clone(),
            FnCallKind::ColonCall => {
                let args_str: Vec<String> = call
                    .args
                    .iter()
                    .map(|a| Self::value_to_text(&a.value))
                    .collect();
                format!("{}:{}", call.func_name, args_str.join(","))
            }
            FnCallKind::ParenCall => {
                let args_str: Vec<String> = call
                    .args
                    .iter()
                    .map(|a| {
                        if a.is_keyword() {
                            format!("{}={}", a.name, Self::value_to_text(&a.value))
                        } else {
                            Self::value_to_text(&a.value)
                        }
                    })
                    .collect();
                format!("{}({})", call.func_name, args_str.join(", "))
            }
        }
    }

    fn value_to_text(value: &Value) -> String {
        if let Some(s) = value.get::<String>() {
            if s.contains(' ') || s.contains(',') || s.contains('(') || s.contains(')') {
                format!("\"{}\"", s)
            } else {
                s.clone()
            }
        } else if let Some(&b) = value.get::<bool>() {
            if b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        } else if let Some(&i) = value.get::<i64>() {
            i.to_string()
        } else if let Some(&i) = value.get::<i32>() {
            i.to_string()
        } else if let Some(&f) = value.get::<f64>() {
            if f == f.floor() && f.abs() < 1e15 {
                format!("{:.1}", f)
            } else {
                f.to_string()
            }
        } else if let Some(&f) = value.get::<f32>() {
            f.to_string()
        } else {
            format!("{:?}", value)
        }
    }
}

/// Adjusts indices in a predicate node when merging expressions.
fn adjust_predicate_node(
    node: PredicateNode,
    call_offset: usize,
    node_offset: usize,
) -> PredicateNode {
    match node {
        PredicateNode::Call(idx) => PredicateNode::Call(idx + call_offset),
        PredicateNode::Op { op, left, right } => PredicateNode::Op {
            op,
            left: left + node_offset,
            right: right.map(|r| r + node_offset),
        },
    }
}

/// Parses an argument value from a string.
fn parse_arg_value(s: &str) -> Value {
    // Try parsing as various types
    if s == "true" {
        return Value::new(true);
    }
    if s == "false" {
        return Value::new(false);
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::new(i);
    }
    // For floats, we need to use the special float handling in Value
    if let Ok(f) = s.parse::<f64>() {
        return Value::from_f64(f);
    }
    // Default to string
    Value::new(s.to_string())
}

impl fmt::Display for PredicateExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_text())
    }
}

impl From<&str> for PredicateExpression {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

impl From<String> for PredicateExpression {
    fn from(s: String) -> Self {
        Self::parse(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_expression() {
        let expr = PredicateExpression::new();
        assert!(expr.is_empty());
    }

    #[test]
    fn test_bare_call() {
        let expr = PredicateExpression::parse("isDefined");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 1);
        assert_eq!(expr.calls()[0].func_name, "isDefined");
        assert_eq!(expr.calls()[0].kind, FnCallKind::BareCall);
    }

    #[test]
    fn test_colon_call() {
        let expr = PredicateExpression::parse("isa:Mesh");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 1);
        assert_eq!(expr.calls()[0].func_name, "isa");
        assert_eq!(expr.calls()[0].kind, FnCallKind::ColonCall);
        assert_eq!(expr.calls()[0].args.len(), 1);
    }

    #[test]
    fn test_fn_arg() {
        let pos = FnArg::positional(Value::new(42));
        assert!(pos.is_positional());
        assert!(!pos.is_keyword());

        let kw = FnArg::keyword("tolerance", Value::from_f64(0.01));
        assert!(!kw.is_positional());
        assert!(kw.is_keyword());
    }

    #[test]
    fn test_fn_call() {
        let bare = FnCall::bare("active");
        assert_eq!(bare.kind, FnCallKind::BareCall);
        assert!(bare.args.is_empty());

        let colon = FnCall::colon("isa", vec![Value::new("Mesh")]);
        assert_eq!(colon.kind, FnCallKind::ColonCall);
        assert_eq!(colon.args.len(), 1);
    }

    #[test]
    fn test_display() {
        let expr = PredicateExpression::parse("isDefined");
        assert_eq!(format!("{}", expr), "isDefined");
    }

    #[test]
    fn test_parse_and() {
        let expr = PredicateExpression::parse("foo and bar");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 2);
    }

    #[test]
    fn test_parse_or() {
        let expr = PredicateExpression::parse("foo or bar");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 2);
    }

    #[test]
    fn test_parse_not() {
        let expr = PredicateExpression::parse("not foo");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 1);
    }

    #[test]
    fn test_parse_parentheses() {
        let expr = PredicateExpression::parse("(foo or bar) and baz");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 3);
    }

    #[test]
    fn test_parse_implied_and() {
        let expr = PredicateExpression::parse("foo bar");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 2);
    }

    #[test]
    fn test_parse_paren_call() {
        let expr = PredicateExpression::parse("foo(23, bar=baz)");
        assert!(!expr.is_empty());
        assert_eq!(expr.calls().len(), 1);
        assert_eq!(expr.calls()[0].kind, FnCallKind::ParenCall);
    }

    #[test]
    fn test_get_text_bare() {
        let expr = PredicateExpression::parse("isDefined");
        assert_eq!(expr.get_text(), "isDefined");
    }

    #[test]
    fn test_get_text_and() {
        let expr = PredicateExpression::parse("foo and bar");
        assert_eq!(expr.get_text(), "foo and bar");
    }

    #[test]
    fn test_get_text_or() {
        let expr = PredicateExpression::parse("foo or bar");
        assert_eq!(expr.get_text(), "foo or bar");
    }

    #[test]
    fn test_get_text_not() {
        let expr = PredicateExpression::parse("not foo");
        assert_eq!(expr.get_text(), "not foo");
    }

    #[test]
    fn test_get_text_complex() {
        // (foo or bar) and not baz
        let expr = PredicateExpression::parse("(foo or bar) and not baz");
        let text = expr.get_text();
        assert_eq!(text, "(foo or bar) and not baz");
    }

    #[test]
    fn test_walk_with_op_stack() {
        let expr = PredicateExpression::parse("foo and bar");
        let mut logic_calls: Vec<(Vec<(PredicateOp, i32)>,)> = Vec::new();
        let mut call_names: Vec<String> = Vec::new();

        expr.walk_with_op_stack(
            |stack| {
                logic_calls.push((stack.to_vec(),));
            },
            |call| {
                call_names.push(call.func_name.clone());
            },
        );

        assert_eq!(call_names, vec!["foo", "bar"]);
        // Should have logic calls with stack context
        assert!(!logic_calls.is_empty());
    }
}

// ============================================================================
// Parser Implementation
// ============================================================================

/// Parser state for predicate expressions.
struct PredicateParser {
    /// Input string being parsed.
    input: Vec<char>,
    /// Current position in input.
    pos: usize,
    /// Current character.
    current: Option<char>,
}

impl PredicateParser {
    fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let current = chars.first().copied();
        Self {
            input: chars,
            pos: 0,
            current,
        }
    }

    /// Advances to the next character.
    fn advance(&mut self) {
        self.pos += 1;
        self.current = self.input.get(self.pos).copied();
    }

    /// Skips whitespace.
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Checks if we're at the end of input.
    fn is_eof(&self) -> bool {
        self.current.is_none()
    }

    /// Parses a complete expression.
    fn parse_expression(&mut self) -> Result<PredicateExpression, String> {
        self.skip_whitespace();
        if self.is_eof() {
            return Ok(PredicateExpression::new());
        }

        // Parse with operator precedence
        // Precedence (lowest to highest):
        // 1. Implied AND (whitespace)
        // 2. OR
        // 3. AND
        // 4. NOT
        // 5. Atoms (function calls, parentheses)

        self.parse_or_expression()
    }

    /// Parses OR expressions (lowest precedence).
    fn parse_or_expression(&mut self) -> Result<PredicateExpression, String> {
        let mut left = self.parse_and_expression()?;

        self.skip_whitespace();

        while self.match_keyword("or") {
            self.skip_whitespace();
            let right = self.parse_and_expression()?;
            left = PredicateExpression::make_op(PredicateOp::Or, left, right);
            self.skip_whitespace();
        }

        Ok(left)
    }

    /// Parses AND expressions.
    fn parse_and_expression(&mut self) -> Result<PredicateExpression, String> {
        let mut left = self.parse_not_expression()?;

        self.skip_whitespace();

        // Check for explicit AND or implied AND (whitespace)
        loop {
            if self.match_keyword("and") {
                self.skip_whitespace();
                let right = self.parse_not_expression()?;
                left = PredicateExpression::make_op(PredicateOp::And, left, right);
                self.skip_whitespace();
            } else if !self.is_eof() && self.peek_identifier() && !self.is_keyword_ahead() {
                // Implied AND - whitespace separates expressions
                // Next token is an identifier (not a keyword), so parse it as implied AND
                let right = self.parse_not_expression()?;
                left = PredicateExpression::make_op(PredicateOp::ImpliedAnd, left, right);
                self.skip_whitespace();
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parses NOT expressions.
    fn parse_not_expression(&mut self) -> Result<PredicateExpression, String> {
        self.skip_whitespace();

        let mut count = 0;
        while self.match_keyword("not") {
            count += 1;
            self.skip_whitespace();
        }

        let mut expr = self.parse_atom()?;

        // Apply NOT operators (right-associative)
        for _ in 0..count {
            expr = PredicateExpression::make_not(expr);
        }

        Ok(expr)
    }

    /// Parses an atom (function call or parenthesized expression).
    fn parse_atom(&mut self) -> Result<PredicateExpression, String> {
        self.skip_whitespace();

        if self.is_eof() {
            return Err("Unexpected end of expression".to_string());
        }

        // Check for parentheses
        if let Some('(') = self.current {
            self.advance();
            self.skip_whitespace();
            let expr = self.parse_expression()?;
            self.skip_whitespace();
            if let Some(')') = self.current {
                self.advance();
                Ok(expr)
            } else {
                Err("Expected ')' after expression".to_string())
            }
        }
        // Otherwise parse as a function call
        else {
            self.parse_function_call()
        }
    }

    /// Parses a function call (bare, colon, or paren style).
    fn parse_function_call(&mut self) -> Result<PredicateExpression, String> {
        // Parse function name
        let name = self.parse_identifier()?;

        self.skip_whitespace();

        // Check for colon call: name:arg1,arg2
        if let Some(':') = self.current {
            self.advance();
            let args = self.parse_colon_args()?;
            Ok(PredicateExpression::make_call(FnCall::colon(name, args)))
        }
        // Check for paren call: name(arg1, arg2, kw=val)
        else if let Some('(') = self.current {
            self.advance();
            let args = self.parse_paren_args()?;
            self.skip_whitespace();
            if let Some(')') = self.current {
                self.advance();
                Ok(PredicateExpression::make_call(FnCall::paren(name, args)))
            } else {
                Err("Expected ')' after function arguments".to_string())
            }
        }
        // Bare call: name
        else {
            Ok(PredicateExpression::make_call(FnCall::bare(name)))
        }
    }

    /// Parses colon-separated arguments (name:arg1,arg2).
    fn parse_colon_args(&mut self) -> Result<Vec<Value>, String> {
        let mut args = Vec::new();

        loop {
            self.skip_whitespace();
            let arg_str = self.parse_arg_string()?;
            args.push(parse_arg_value(&arg_str));

            if let Some(',') = self.current {
                self.advance();
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// Parses parenthesized arguments (name(arg1, arg2, kw=val)).
    fn parse_paren_args(&mut self) -> Result<Vec<FnArg>, String> {
        let mut args = Vec::new();

        self.skip_whitespace();

        // Empty args
        if let Some(')') = self.current {
            return Ok(args);
        }

        loop {
            self.skip_whitespace();

            // Check for keyword argument: name=value
            let arg = if self.peek_identifier() {
                // Save position to check for '='
                let saved_pos = self.pos;
                let saved_current = self.current;
                let name = self.parse_identifier()?;
                self.skip_whitespace();

                if let Some('=') = self.current {
                    // It's a keyword argument
                    self.advance(); // skip '='
                    self.skip_whitespace();
                    let value_str = self.parse_arg_string()?;
                    FnArg::keyword(name, parse_arg_value(&value_str))
                } else {
                    // Not a keyword arg, restore and parse as positional
                    self.pos = saved_pos;
                    self.current = saved_current;
                    let value_str = self.parse_arg_string()?;
                    FnArg::positional(parse_arg_value(&value_str))
                }
            } else {
                // Positional argument
                let value_str = self.parse_arg_string()?;
                FnArg::positional(parse_arg_value(&value_str))
            };

            args.push(arg);

            self.skip_whitespace();

            if let Some(',') = self.current {
                self.advance();
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// Parses an argument value string (handles quoted strings, numbers, etc.).
    fn parse_arg_string(&mut self) -> Result<String, String> {
        self.skip_whitespace();

        if self.is_eof() {
            return Err("Expected argument value".to_string());
        }

        // Check for quoted string
        if let Some('"') | Some('\'') = self.current {
            let quote = self.current.expect("matched above");
            self.advance();
            let mut result = String::new();

            while let Some(ch) = self.current {
                if ch == quote {
                    self.advance();
                    return Ok(result);
                }
                if ch == '\\' {
                    self.advance();
                    if let Some(escaped) = self.current {
                        result.push(match escaped {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '\\' => '\\',
                            _ => escaped,
                        });
                        self.advance();
                    }
                } else {
                    result.push(ch);
                    self.advance();
                }
            }

            Err("Unterminated string literal".to_string())
        }
        // Otherwise parse identifier or number
        else {
            let mut result = String::new();

            while let Some(ch) = self.current {
                if ch.is_whitespace() || ch == ',' || ch == ')' || ch == '=' {
                    break;
                }
                result.push(ch);
                self.advance();
            }

            if result.is_empty() {
                Err("Expected argument value".to_string())
            } else {
                Ok(result)
            }
        }
    }

    /// Parses an identifier.
    fn parse_identifier(&mut self) -> Result<String, String> {
        let mut result = String::new();

        if let Some(ch) = self.current {
            if ch.is_alphabetic() || ch == '_' {
                result.push(ch);
                self.advance();
            } else {
                return Err(format!("Expected identifier, found: {:?}", ch));
            }
        } else {
            return Err("Expected identifier".to_string());
        }

        while let Some(ch) = self.current {
            if ch.is_alphanumeric() || ch == '_' {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        Ok(result)
    }

    /// Checks if the next token matches a keyword (case-insensitive).
    fn match_keyword(&mut self, keyword: &str) -> bool {
        let saved_pos = self.pos;
        let saved_current = self.current;

        // Try to match keyword
        for expected_ch in keyword.chars() {
            if let Some(ch) = self.current {
                if !ch.eq_ignore_ascii_case(&expected_ch) {
                    // Restore state
                    self.pos = saved_pos;
                    self.current = saved_current;
                    return false;
                }
                self.advance();
            } else {
                // Restore state
                self.pos = saved_pos;
                self.current = saved_current;
                return false;
            }
        }

        // Check that next char is not alphanumeric (to avoid matching "and" in "android")
        if let Some(ch) = self.current {
            if ch.is_alphanumeric() || ch == '_' {
                // Restore state
                self.pos = saved_pos;
                self.current = saved_current;
                return false;
            }
        }

        true
    }

    /// Peeks at the next character without advancing.
    #[allow(dead_code)] // Parser helper
    fn peek(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
    }

    /// Checks if next token is an identifier.
    fn peek_identifier(&self) -> bool {
        if let Some(ch) = self.current {
            ch.is_alphabetic() || ch == '_'
        } else {
            false
        }
    }

    /// Checks if next token is a keyword (and, or, not).
    fn is_keyword_ahead(&self) -> bool {
        if !self.peek_identifier() {
            return false;
        }
        // Collect the identifier
        let mut word = String::new();
        let mut pos = self.pos;
        while pos < self.input.len() {
            let ch = self.input[pos];
            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch);
                pos += 1;
            } else {
                break;
            }
        }
        let word_lower = word.to_lowercase();
        word_lower == "and" || word_lower == "or" || word_lower == "not"
    }
}

/// Parses a predicate expression from a string.
fn parse_predicate_expression(expr: &str) -> Result<PredicateExpression, String> {
    let mut parser = PredicateParser::new(expr);
    let result = parser.parse_expression()?;

    // Check for trailing characters
    parser.skip_whitespace();
    if !parser.is_eof() {
        return Err(format!(
            "Unexpected character at position {}: {:?}",
            parser.pos, parser.current
        ));
    }

    Ok(result)
}
