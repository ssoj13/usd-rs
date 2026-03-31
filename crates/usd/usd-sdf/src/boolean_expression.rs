//! SdfBooleanExpression - expressions that evaluate to boolean values.
//!
//! Port of pxr/usd/sdf/booleanExpression.h
//!
//! Objects of this class represent expressions that can be evaluated to produce
//! a boolean value. Used for conditional evaluation in USD.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value;

/// Binary operators for combining two subexpressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    /// The `==` operator.
    EqualTo,
    /// The `!=` operator.
    NotEqualTo,
    /// The `<` operator.
    LessThan,
    /// The `<=` operator.
    LessThanOrEqualTo,
    /// The `>` operator.
    GreaterThan,
    /// The `>=` operator.
    GreaterThanOrEqualTo,
    /// The `&&` operator.
    And,
    /// The `||` operator.
    Or,
}

impl fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::EqualTo => "==",
            Self::NotEqualTo => "!=",
            Self::LessThan => "<",
            Self::LessThanOrEqualTo => "<=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqualTo => ">=",
            Self::And => "&&",
            Self::Or => "||",
        };
        write!(f, "{}", s)
    }
}

/// Unary operators applied to a single subexpression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperator {
    /// The `!` operator.
    Not,
}

impl fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Not => write!(f, "!"),
        }
    }
}

/// Internal node representing an expression element.
#[derive(Debug, Clone)]
enum Node {
    /// A variable reference.
    Variable(Token),
    /// A constant value.
    Constant(Value),
    /// A binary operation.
    Binary {
        lhs: Arc<Node>,
        op: BinaryOperator,
        rhs: Arc<Node>,
    },
    /// A unary operation.
    Unary { op: UnaryOperator, expr: Arc<Node> },
}

/// Represents expressions that can be evaluated to produce a boolean value.
///
/// These expressions support:
/// - Variable references (`variableName`)
/// - Constant values (`true`, `10.0`, `"string"`)
/// - Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`)
/// - Logical operators (`&&`, `||`, `!`)
#[derive(Debug, Clone, Default)]
pub struct BooleanExpression {
    /// Original text if parsed from string.
    text: String,
    /// Parse error if any.
    parse_error: String,
    /// Root node of expression tree.
    root: Option<Arc<Node>>,
}

impl BooleanExpression {
    /// Constructs an empty expression.
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs an expression by parsing a string representation.
    ///
    /// If an error occurs while parsing, the result will be empty
    /// and `get_parse_error()` will contain the error message.
    pub fn from_text(text: &str) -> Self {
        let mut expr = Self {
            text: text.to_string(),
            parse_error: String::new(),
            root: None,
        };

        // Parse the expression
        match Self::parse(text) {
            Ok(node) => expr.root = Some(node),
            Err(e) => expr.parse_error = e,
        }

        expr
    }

    /// Returns true if the expression is empty.
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Returns the text representation of the expression.
    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// Returns parsing errors as a string if any occurred.
    pub fn get_parse_error(&self) -> &str {
        &self.parse_error
    }

    /// Returns the collection of variable names referenced by the expression.
    pub fn get_variable_names(&self) -> HashSet<Token> {
        let mut names = HashSet::new();
        if let Some(ref root) = self.root {
            Self::collect_variables(root, &mut names);
        }
        names
    }

    /// Recursively collects variable names from the expression tree.
    fn collect_variables(node: &Node, names: &mut HashSet<Token>) {
        match node {
            Node::Variable(name) => {
                names.insert(name.clone());
            }
            Node::Constant(_) => {}
            Node::Binary { lhs, rhs, .. } => {
                Self::collect_variables(lhs, names);
                Self::collect_variables(rhs, names);
            }
            Node::Unary { expr, .. } => {
                Self::collect_variables(expr, names);
            }
        }
    }

    /// Constructs an expression representing a variable.
    pub fn make_variable(name: Token) -> Self {
        Self {
            text: name.as_str().to_string(),
            parse_error: String::new(),
            root: Some(Arc::new(Node::Variable(name))),
        }
    }

    /// Constructs an expression wrapping a constant value.
    pub fn make_constant(value: Value) -> Self {
        let text = format!("{:?}", value);
        Self {
            text,
            parse_error: String::new(),
            root: Some(Arc::new(Node::Constant(value))),
        }
    }

    /// Constructs an expression that applies a binary operator.
    pub fn make_binary_op(
        lhs: BooleanExpression,
        op: BinaryOperator,
        rhs: BooleanExpression,
    ) -> Self {
        let text = format!("({} {} {})", lhs.text, op, rhs.text);

        match (lhs.root, rhs.root) {
            (Some(l), Some(r)) => Self {
                text,
                parse_error: String::new(),
                root: Some(Arc::new(Node::Binary { lhs: l, op, rhs: r })),
            },
            _ => Self {
                text,
                parse_error: "Invalid operands".to_string(),
                root: None,
            },
        }
    }

    /// Constructs an expression that applies a unary operator.
    pub fn make_unary_op(expr: BooleanExpression, op: UnaryOperator) -> Self {
        let text = format!("{}({})", op, expr.text);

        match expr.root {
            Some(e) => Self {
                text,
                parse_error: String::new(),
                root: Some(Arc::new(Node::Unary { op, expr: e })),
            },
            None => Self {
                text,
                parse_error: "Invalid operand".to_string(),
                root: None,
            },
        }
    }

    /// Evaluates the expression using the provided variable callback.
    pub fn evaluate<F>(&self, variable_callback: F) -> bool
    where
        F: Fn(&Token) -> Value,
    {
        match &self.root {
            Some(node) => Self::eval_node(node, &variable_callback),
            None => false,
        }
    }

    /// Recursively evaluates a node.
    fn eval_node<F>(node: &Node, var_cb: &F) -> bool
    where
        F: Fn(&Token) -> Value,
    {
        match node {
            Node::Variable(name) => {
                let val = var_cb(name);
                Self::value_to_bool(&val)
            }
            Node::Constant(val) => Self::value_to_bool(val),
            Node::Binary { lhs, op, rhs } => {
                match op {
                    BinaryOperator::And => {
                        Self::eval_node(lhs, var_cb) && Self::eval_node(rhs, var_cb)
                    }
                    BinaryOperator::Or => {
                        Self::eval_node(lhs, var_cb) || Self::eval_node(rhs, var_cb)
                    }
                    _ => {
                        // Comparison operators need the actual values
                        let lval = Self::eval_to_value(lhs, var_cb);
                        let rval = Self::eval_to_value(rhs, var_cb);
                        Self::compare_values(&lval, *op, &rval)
                    }
                }
            }
            Node::Unary { op, expr } => match op {
                UnaryOperator::Not => !Self::eval_node(expr, var_cb),
            },
        }
    }

    /// Evaluates a node to get its value.
    fn eval_to_value<F>(node: &Node, var_cb: &F) -> Value
    where
        F: Fn(&Token) -> Value,
    {
        match node {
            Node::Variable(name) => var_cb(name),
            Node::Constant(val) => val.clone(),
            Node::Binary { lhs, op, rhs } => {
                let result = match op {
                    BinaryOperator::And => {
                        Self::eval_node(lhs, var_cb) && Self::eval_node(rhs, var_cb)
                    }
                    BinaryOperator::Or => {
                        Self::eval_node(lhs, var_cb) || Self::eval_node(rhs, var_cb)
                    }
                    _ => {
                        let lval = Self::eval_to_value(lhs, var_cb);
                        let rval = Self::eval_to_value(rhs, var_cb);
                        Self::compare_values(&lval, *op, &rval)
                    }
                };
                Value::new(result)
            }
            Node::Unary { op, expr } => {
                let result = match op {
                    UnaryOperator::Not => !Self::eval_node(expr, var_cb),
                };
                Value::new(result)
            }
        }
    }

    /// Converts a Value to bool.
    fn value_to_bool(val: &Value) -> bool {
        if let Some(&b) = val.get::<bool>() {
            return b;
        }
        if let Some(&i) = val.get::<i32>() {
            return i != 0;
        }
        if let Some(&i) = val.get::<i64>() {
            return i != 0;
        }
        if let Some(&f) = val.get::<f64>() {
            return f != 0.0;
        }
        if let Some(s) = val.get::<String>() {
            return !s.is_empty();
        }
        !val.is_empty()
    }

    /// Compares two values with the given operator.
    fn compare_values(lhs: &Value, op: BinaryOperator, rhs: &Value) -> bool {
        // Try numeric comparison first
        if let (Some(&l), Some(&r)) = (lhs.get::<f64>(), rhs.get::<f64>()) {
            return match op {
                BinaryOperator::EqualTo => (l - r).abs() < f64::EPSILON,
                BinaryOperator::NotEqualTo => (l - r).abs() >= f64::EPSILON,
                BinaryOperator::LessThan => l < r,
                BinaryOperator::LessThanOrEqualTo => l <= r,
                BinaryOperator::GreaterThan => l > r,
                BinaryOperator::GreaterThanOrEqualTo => l >= r,
                BinaryOperator::And | BinaryOperator::Or => false,
            };
        }

        if let (Some(&l), Some(&r)) = (lhs.get::<i64>(), rhs.get::<i64>()) {
            return match op {
                BinaryOperator::EqualTo => l == r,
                BinaryOperator::NotEqualTo => l != r,
                BinaryOperator::LessThan => l < r,
                BinaryOperator::LessThanOrEqualTo => l <= r,
                BinaryOperator::GreaterThan => l > r,
                BinaryOperator::GreaterThanOrEqualTo => l >= r,
                BinaryOperator::And | BinaryOperator::Or => false,
            };
        }

        if let (Some(&l), Some(&r)) = (lhs.get::<i32>(), rhs.get::<i32>()) {
            return match op {
                BinaryOperator::EqualTo => l == r,
                BinaryOperator::NotEqualTo => l != r,
                BinaryOperator::LessThan => l < r,
                BinaryOperator::LessThanOrEqualTo => l <= r,
                BinaryOperator::GreaterThan => l > r,
                BinaryOperator::GreaterThanOrEqualTo => l >= r,
                BinaryOperator::And | BinaryOperator::Or => false,
            };
        }

        // String comparison
        if let (Some(l), Some(r)) = (lhs.get::<String>(), rhs.get::<String>()) {
            return match op {
                BinaryOperator::EqualTo => l == r,
                BinaryOperator::NotEqualTo => l != r,
                BinaryOperator::LessThan => l < r,
                BinaryOperator::LessThanOrEqualTo => l <= r,
                BinaryOperator::GreaterThan => l > r,
                BinaryOperator::GreaterThanOrEqualTo => l >= r,
                BinaryOperator::And | BinaryOperator::Or => false,
            };
        }

        // Bool comparison
        if let (Some(&l), Some(&r)) = (lhs.get::<bool>(), rhs.get::<bool>()) {
            return match op {
                BinaryOperator::EqualTo => l == r,
                BinaryOperator::NotEqualTo => l != r,
                _ => false,
            };
        }

        // Default: equality check
        match op {
            BinaryOperator::EqualTo => lhs.is_empty() && rhs.is_empty(),
            BinaryOperator::NotEqualTo => !(lhs.is_empty() && rhs.is_empty()),
            _ => false,
        }
    }

    /// Applies a transform to each variable name and returns the resulting expression.
    pub fn rename_variables<F>(&self, transform: F) -> Self
    where
        F: Fn(&Token) -> Token,
    {
        match &self.root {
            Some(node) => {
                let new_root = Self::rename_node(node, &transform);
                Self {
                    text: Self::node_to_text(&new_root),
                    parse_error: String::new(),
                    root: Some(new_root),
                }
            }
            None => self.clone(),
        }
    }

    /// Recursively renames variables in a node.
    fn rename_node<F>(node: &Node, transform: &F) -> Arc<Node>
    where
        F: Fn(&Token) -> Token,
    {
        match node {
            Node::Variable(name) => Arc::new(Node::Variable(transform(name))),
            Node::Constant(val) => Arc::new(Node::Constant(val.clone())),
            Node::Binary { lhs, op, rhs } => Arc::new(Node::Binary {
                lhs: Self::rename_node(lhs, transform),
                op: *op,
                rhs: Self::rename_node(rhs, transform),
            }),
            Node::Unary { op, expr } => Arc::new(Node::Unary {
                op: *op,
                expr: Self::rename_node(expr, transform),
            }),
        }
    }

    /// Converts a node to text representation.
    fn node_to_text(node: &Arc<Node>) -> String {
        match node.as_ref() {
            Node::Variable(name) => name.as_str().to_string(),
            Node::Constant(val) => format!("{:?}", val),
            Node::Binary { lhs, op, rhs } => {
                format!(
                    "({} {} {})",
                    Self::node_to_text(lhs),
                    op,
                    Self::node_to_text(rhs)
                )
            }
            Node::Unary { op, expr } => {
                format!("{}({})", op, Self::node_to_text(expr))
            }
        }
    }

    /// Validates a string as an expression.
    pub fn validate(expression: &str) -> Result<(), String> {
        Self::parse(expression).map(|_| ())
    }

    /// Simple expression parser.
    fn parse(text: &str) -> Result<Arc<Node>, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Empty expression".to_string());
        }

        // Try to parse as simple expressions for now
        // Full parser would need lexer + recursive descent

        // Boolean literals
        if text == "true" {
            return Ok(Arc::new(Node::Constant(Value::new(true))));
        }
        if text == "false" {
            return Ok(Arc::new(Node::Constant(Value::new(false))));
        }

        // Numeric literals
        if let Ok(i) = text.parse::<i64>() {
            return Ok(Arc::new(Node::Constant(Value::new(i))));
        }
        if let Ok(f) = text.parse::<f64>() {
            return Ok(Arc::new(Node::Constant(Value::from_no_hash(f))));
        }

        // Quoted string
        if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
            let s = &text[1..text.len() - 1];
            return Ok(Arc::new(Node::Constant(Value::new(s.to_string()))));
        }

        // Unary not
        if text.starts_with('!') {
            let inner = Self::parse(&text[1..])?;
            return Ok(Arc::new(Node::Unary {
                op: UnaryOperator::Not,
                expr: inner,
            }));
        }

        // Simple variable (identifier)
        if text.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Ok(Arc::new(Node::Variable(Token::new(text))));
        }

        // Try to find binary operators (simplified - doesn't handle precedence)
        for (op_str, op) in [
            ("&&", BinaryOperator::And),
            ("||", BinaryOperator::Or),
            ("==", BinaryOperator::EqualTo),
            ("!=", BinaryOperator::NotEqualTo),
            ("<=", BinaryOperator::LessThanOrEqualTo),
            (">=", BinaryOperator::GreaterThanOrEqualTo),
            ("<", BinaryOperator::LessThan),
            (">", BinaryOperator::GreaterThan),
        ] {
            if let Some(pos) = text.find(op_str) {
                let lhs = Self::parse(&text[..pos])?;
                let rhs = Self::parse(&text[pos + op_str.len()..])?;
                return Ok(Arc::new(Node::Binary { lhs, op, rhs }));
            }
        }

        Err(format!("Cannot parse expression: {}", text))
    }
}

impl fmt::Display for BooleanExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_variable() {
        let expr = BooleanExpression::make_variable(Token::new("width"));
        assert!(!expr.is_empty());
        assert!(expr.get_variable_names().contains(&Token::new("width")));
    }

    #[test]
    fn test_make_constant() {
        let expr = BooleanExpression::make_constant(Value::new(true));
        assert!(!expr.is_empty());
        assert!(expr.get_variable_names().is_empty());
    }

    #[test]
    fn test_evaluate_constant() {
        let expr = BooleanExpression::make_constant(Value::new(true));
        assert!(expr.evaluate(|_| Value::default()));

        let expr = BooleanExpression::make_constant(Value::new(false));
        assert!(!expr.evaluate(|_| Value::default()));
    }

    #[test]
    fn test_evaluate_variable() {
        let expr = BooleanExpression::make_variable(Token::new("enabled"));

        assert!(expr.evaluate(|name| {
            if name == "enabled" {
                Value::new(true)
            } else {
                Value::default()
            }
        }));

        assert!(!expr.evaluate(|_| Value::new(false)));
    }

    #[test]
    fn test_binary_and() {
        let lhs = BooleanExpression::make_constant(Value::new(true));
        let rhs = BooleanExpression::make_constant(Value::new(true));
        let expr = BooleanExpression::make_binary_op(lhs, BinaryOperator::And, rhs);
        assert!(expr.evaluate(|_| Value::default()));

        let lhs = BooleanExpression::make_constant(Value::new(true));
        let rhs = BooleanExpression::make_constant(Value::new(false));
        let expr = BooleanExpression::make_binary_op(lhs, BinaryOperator::And, rhs);
        assert!(!expr.evaluate(|_| Value::default()));
    }

    #[test]
    fn test_binary_or() {
        let lhs = BooleanExpression::make_constant(Value::new(false));
        let rhs = BooleanExpression::make_constant(Value::new(true));
        let expr = BooleanExpression::make_binary_op(lhs, BinaryOperator::Or, rhs);
        assert!(expr.evaluate(|_| Value::default()));
    }

    #[test]
    fn test_comparison() {
        let lhs = BooleanExpression::make_constant(Value::new(10i32));
        let rhs = BooleanExpression::make_constant(Value::new(5i32));

        let expr = BooleanExpression::make_binary_op(
            lhs.clone(),
            BinaryOperator::GreaterThan,
            rhs.clone(),
        );
        assert!(expr.evaluate(|_| Value::default()));

        let expr = BooleanExpression::make_binary_op(lhs, BinaryOperator::LessThan, rhs);
        assert!(!expr.evaluate(|_| Value::default()));
    }

    #[test]
    fn test_unary_not() {
        let inner = BooleanExpression::make_constant(Value::new(true));
        let expr = BooleanExpression::make_unary_op(inner, UnaryOperator::Not);
        assert!(!expr.evaluate(|_| Value::default()));
    }

    #[test]
    fn test_parse_simple() {
        let expr = BooleanExpression::from_text("true");
        assert!(!expr.is_empty());
        assert!(expr.evaluate(|_| Value::default()));

        let expr = BooleanExpression::from_text("false");
        assert!(!expr.is_empty());
        assert!(!expr.evaluate(|_| Value::default()));
    }

    #[test]
    fn test_rename_variables() {
        let expr = BooleanExpression::make_variable(Token::new("old_name"));
        let renamed = expr.rename_variables(|name| {
            if name == "old_name" {
                Token::new("new_name")
            } else {
                name.clone()
            }
        });

        assert!(
            renamed
                .get_variable_names()
                .contains(&Token::new("new_name"))
        );
        assert!(
            !renamed
                .get_variable_names()
                .contains(&Token::new("old_name"))
        );
    }
}
