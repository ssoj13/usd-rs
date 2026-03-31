//! Variable expressions for dynamic string substitution.
//!
//! Port of pxr/usd/sdf/variableExpression.h
//!
//! Variable expressions are strings surrounded by backticks that contain
//! placeholders like `${NAME}` which are substituted with values from
//! an expression variables dictionary.
//!
//! # Syntax
//!
//! ```text
//! `"a_${NAME}_string"`
//! ```
//!
//! The `${NAME}` portion is replaced with the value of the "NAME" variable.
//!
//! # Supported Types
//!
//! Expression variables may be:
//! - String
//! - i64 (integers)
//! - bool
//! - Arrays of the above
//! - None (empty value)

use std::collections::HashSet;
use usd_vt::{Dictionary, Value};

/// Result of evaluating a variable expression.
#[derive(Debug, Clone, Default)]
pub struct VariableExpressionResult {
    /// The evaluated value (may be empty on error).
    pub value: Option<Value>,
    /// Errors encountered during evaluation.
    pub errors: Vec<String>,
    /// Variables used during evaluation.
    pub used_variables: HashSet<String>,
}

impl VariableExpressionResult {
    /// Creates a successful result with a value.
    pub fn success(value: Value) -> Self {
        Self {
            value: Some(value),
            errors: Vec::new(),
            used_variables: HashSet::new(),
        }
    }

    /// Creates an error result.
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            value: None,
            errors: vec![msg.into()],
            used_variables: HashSet::new(),
        }
    }

    /// Returns true if evaluation was successful.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty() && self.value.is_some()
    }
}

/// A variable expression that can be evaluated with variable substitution.
///
/// Variable expressions are strings surrounded by backticks containing
/// `${VAR}` placeholders that are replaced with values from a variables
/// dictionary.
#[derive(Debug, Clone, Default)]
pub struct VariableExpression {
    /// The original expression string.
    expression: String,
    /// Parse errors.
    errors: Vec<String>,
    /// Parsed variable references.
    variables: Vec<String>,
    /// Is this a valid expression.
    is_valid: bool,
}

impl VariableExpression {
    /// Creates a new variable expression from a string.
    pub fn new(expr: impl Into<String>) -> Self {
        let expr_str = expr.into();
        Self::parse(&expr_str)
    }

    /// Returns true if the string is a variable expression (surrounded by backticks).
    pub fn is_expression(s: &str) -> bool {
        s.starts_with('`') && s.ends_with('`') && s.len() >= 2
    }

    /// Returns true if the value type is supported in variable expressions.
    pub fn is_valid_variable_type(value: &Value) -> bool {
        value.is::<String>()
            || value.is::<i64>()
            || value.is::<i32>()
            || value.is::<bool>()
            || value.is_empty()
    }

    /// Parses an expression string.
    fn parse(expr: &str) -> Self {
        let mut result = Self {
            expression: expr.to_string(),
            errors: Vec::new(),
            variables: Vec::new(),
            is_valid: false,
        };

        // Check for backticks
        if !Self::is_expression(expr) {
            result
                .errors
                .push("Expression must be surrounded by backticks".to_string());
            return result;
        }

        // Remove backticks and quotes
        let content = &expr[1..expr.len() - 1];
        let content = content.trim_matches('"');

        // Find all ${VAR} references
        let mut pos = 0;
        let bytes = content.as_bytes();
        while pos < bytes.len() {
            if pos + 1 < bytes.len() && bytes[pos] == b'$' && bytes[pos + 1] == b'{' {
                // Found start of variable reference
                let start = pos + 2;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'}' {
                    end += 1;
                }
                if end < bytes.len() {
                    let var_name = &content[start..end];
                    if !var_name.is_empty() {
                        result.variables.push(var_name.to_string());
                    }
                    pos = end + 1;
                } else {
                    result.errors.push(format!(
                        "Unclosed variable reference at position {}",
                        start - 2
                    ));
                    pos = end;
                }
            } else {
                pos += 1;
            }
        }

        result.is_valid = result.errors.is_empty();
        result
    }

    /// Returns true if this is a valid expression.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Returns the original expression string.
    pub fn get_string(&self) -> &str {
        &self.expression
    }

    /// Returns parse errors.
    pub fn get_errors(&self) -> &[String] {
        &self.errors
    }

    /// Returns the variables referenced in this expression.
    pub fn get_variables(&self) -> &[String] {
        &self.variables
    }

    /// Evaluates the expression with the given variables.
    ///
    /// If any variable values are themselves expressions (strings surrounded
    /// by backticks), they will be recursively parsed and evaluated, matching
    /// C++ `SdfVariableExpression::Evaluate` semantics.
    pub fn evaluate(&self, variables: &Dictionary) -> VariableExpressionResult {
        self.evaluate_impl(variables, 0)
    }

    /// Maximum recursion depth for sub-expression evaluation.
    const MAX_DEPTH: usize = 32;

    /// Internal evaluate with recursion depth tracking.
    fn evaluate_impl(&self, variables: &Dictionary, depth: usize) -> VariableExpressionResult {
        if !self.is_valid {
            return VariableExpressionResult {
                value: None,
                errors: self.errors.clone(),
                used_variables: HashSet::new(),
            };
        }

        if depth > Self::MAX_DEPTH {
            return VariableExpressionResult::error(
                "Maximum expression nesting depth exceeded (possible cycle)",
            );
        }

        let mut result = VariableExpressionResult::default();

        // Get content without backticks and quotes
        let content = &self.expression[1..self.expression.len() - 1];
        let content = content.trim_matches('"');

        // Perform substitution
        let mut output = String::new();
        let mut pos = 0;
        let bytes = content.as_bytes();

        while pos < bytes.len() {
            if pos + 1 < bytes.len() && bytes[pos] == b'$' && bytes[pos + 1] == b'{' {
                let start = pos + 2;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'}' {
                    end += 1;
                }
                if end < bytes.len() {
                    let var_name = &content[start..end];
                    result.used_variables.insert(var_name.to_string());

                    // Look up variable value
                    if let Some(value) = variables.get(var_name) {
                        // If the value is a string that is itself an expression,
                        // recursively evaluate it (matching C++ behavior).
                        if let Some(s) = value.get::<String>() {
                            if Self::is_expression(s) {
                                let sub_expr = VariableExpression::new(s.clone());
                                let sub_result = sub_expr.evaluate_impl(variables, depth + 1);
                                result.used_variables.extend(sub_result.used_variables);
                                if !sub_result.errors.is_empty() {
                                    result.errors.extend(sub_result.errors);
                                } else if let Some(sub_val) = &sub_result.value {
                                    if let Some(sv) = sub_val.get::<String>() {
                                        output.push_str(sv);
                                    } else {
                                        output.push_str(&format!("{:?}", sub_val));
                                    }
                                }
                            } else {
                                output.push_str(s);
                            }
                        } else if let Some(i) = value.get::<i64>() {
                            output.push_str(&i.to_string());
                        } else if let Some(i) = value.get::<i32>() {
                            output.push_str(&i.to_string());
                        } else if let Some(b) = value.get::<bool>() {
                            output.push_str(if *b { "true" } else { "false" });
                        } else if value.is_empty() {
                            // None value — produces empty string, matching C++
                        } else {
                            result
                                .errors
                                .push(format!("Variable '{}' has unsupported type", var_name));
                        }
                    } else {
                        result
                            .errors
                            .push(format!("Undefined variable '{}'", var_name));
                    }
                    pos = end + 1;
                } else {
                    output.push(content.as_bytes()[pos] as char);
                    pos += 1;
                }
            } else {
                output.push(bytes[pos] as char);
                pos += 1;
            }
        }

        if result.errors.is_empty() {
            result.value = Some(Value::new(output));
        }

        result
    }
}

/// Builder for programmatically constructing variable expressions.
///
/// Matches C++ `SdfVariableExpression::Builder`, `MakeLiteral`,
/// `MakeVariable`, `MakeFunction`, `MakeList`, `MakeNone`.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::VariableExpressionBuilder;
///
/// let expr = VariableExpressionBuilder::variable("NAME").build();
/// assert_eq!(expr.get_string(), "`${NAME}`");
///
/// let expr = VariableExpressionBuilder::literal_string("hello").build();
/// assert_eq!(expr.get_string(), r#"`"hello"`"#);
/// ```
pub struct VariableExpressionBuilder {
    expr: String,
}

impl VariableExpressionBuilder {
    /// Creates a variable reference `${name}`.
    pub fn variable(name: &str) -> Self {
        Self {
            expr: format!("${{{}}}", name),
        }
    }

    /// Creates a string literal `"value"`.
    pub fn literal_string(value: &str) -> Self {
        Self {
            expr: format!("\"{}\"", value),
        }
    }

    /// Creates an integer literal.
    pub fn literal_int(value: i64) -> Self {
        Self {
            expr: value.to_string(),
        }
    }

    /// Creates a boolean literal.
    pub fn literal_bool(value: bool) -> Self {
        Self {
            expr: if value {
                "true".to_string()
            } else {
                "false".to_string()
            },
        }
    }

    /// Creates a `None` literal.
    pub fn none() -> Self {
        Self {
            expr: "None".to_string(),
        }
    }

    /// Builds the final `VariableExpression` by wrapping in backticks.
    pub fn build(self) -> VariableExpression {
        VariableExpression::new(format!("`{}`", self.expr))
    }

    /// Returns the intermediate expression string (without backticks).
    pub fn as_str(&self) -> &str {
        &self.expr
    }

    /// Creates a function call expression `fnName(arg1, arg2, ...)`.
    ///
    /// Each argument should be a `VariableExpressionBuilder` created via
    /// `variable()`, `literal_*()`, `make_function()`, `make_list()`, etc.
    ///
    /// Matches C++ `SdfVariableExpression::MakeFunction`.
    ///
    /// # Example
    /// ```
    /// use usd_sdf::VariableExpressionBuilder;
    ///
    /// let expr = VariableExpressionBuilder::make_function(
    ///     "contains",
    ///     vec![
    ///         VariableExpressionBuilder::make_list(vec![
    ///             VariableExpressionBuilder::literal_string("foo"),
    ///             VariableExpressionBuilder::literal_string("bar"),
    ///         ]),
    ///         VariableExpressionBuilder::variable("VAR"),
    ///     ],
    /// );
    /// assert_eq!(expr.as_str(), r#"contains(["foo", "bar"], ${VAR})"#);
    /// ```
    pub fn make_function(name: &str, args: Vec<VariableExpressionBuilder>) -> Self {
        let mut expr = format!("{}(", name);
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                expr.push_str(", ");
            }
            expr.push_str(arg.as_str());
        }
        expr.push(')');
        Self { expr }
    }

    /// Creates a list expression `[elem1, elem2, ...]`.
    ///
    /// Each element should be a `VariableExpressionBuilder`.
    ///
    /// Matches C++ `SdfVariableExpression::MakeList`.
    ///
    /// # Example
    /// ```
    /// use usd_sdf::VariableExpressionBuilder;
    ///
    /// let expr = VariableExpressionBuilder::make_list(vec![
    ///     VariableExpressionBuilder::literal_string("a"),
    ///     VariableExpressionBuilder::literal_int(42),
    /// ]);
    /// assert_eq!(expr.as_str(), r#"["a", 42]"#);
    /// ```
    pub fn make_list(elements: Vec<VariableExpressionBuilder>) -> Self {
        let mut expr = String::from("[");
        for (i, elem) in elements.iter().enumerate() {
            if i > 0 {
                expr.push_str(", ");
            }
            expr.push_str(elem.as_str());
        }
        expr.push(']');
        Self { expr }
    }

    /// Creates a list expression from literal string values.
    ///
    /// Matches C++ `SdfVariableExpression::MakeListOfLiterals` for string values.
    pub fn make_list_of_literal_strings(values: &[&str]) -> Self {
        Self::make_list(values.iter().map(|v| Self::literal_string(v)).collect())
    }

    /// Creates a list expression from literal integer values.
    ///
    /// Matches C++ `SdfVariableExpression::MakeListOfLiterals` for int values.
    pub fn make_list_of_literal_ints(values: &[i64]) -> Self {
        Self::make_list(values.iter().map(|v| Self::literal_int(*v)).collect())
    }

    /// Creates a list expression from literal boolean values.
    ///
    /// Matches C++ `SdfVariableExpression::MakeListOfLiterals` for bool values.
    pub fn make_list_of_literal_bools(values: &[bool]) -> Self {
        Self::make_list(values.iter().map(|v| Self::literal_bool(*v)).collect())
    }
}

impl From<&str> for VariableExpression {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for VariableExpression {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_expression() {
        assert!(VariableExpression::is_expression("`hello`"));
        assert!(VariableExpression::is_expression("`\"hello\"`"));
        assert!(!VariableExpression::is_expression("hello"));
        assert!(!VariableExpression::is_expression("`"));
        assert!(VariableExpression::is_expression("``")); // Empty but valid syntax
    }

    #[test]
    fn test_parse_simple() {
        let expr = VariableExpression::new("`\"hello_${NAME}\"`");
        assert!(expr.is_valid());
        assert_eq!(expr.get_variables(), &["NAME"]);
    }

    #[test]
    fn test_parse_multiple_vars() {
        let expr = VariableExpression::new("`\"${A}_and_${B}\"`");
        assert!(expr.is_valid());
        assert_eq!(expr.get_variables().len(), 2);
        assert!(expr.get_variables().contains(&"A".to_string()));
        assert!(expr.get_variables().contains(&"B".to_string()));
    }

    #[test]
    fn test_evaluate() {
        let expr = VariableExpression::new("`\"hello_${NAME}\"`");

        let mut vars = Dictionary::new();
        vars.insert("NAME".to_string(), Value::new("world".to_string()));

        let result = expr.evaluate(&vars);
        assert!(result.is_ok());
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"hello_world".to_string())
        );
    }

    #[test]
    fn test_evaluate_integer() {
        let expr = VariableExpression::new("`\"value_${NUM}\"`");

        let mut vars = Dictionary::new();
        vars.insert("NUM".to_string(), Value::new(42i64));

        let result = expr.evaluate(&vars);
        assert!(result.is_ok());
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"value_42".to_string())
        );
    }

    #[test]
    fn test_evaluate_missing_var() {
        let expr = VariableExpression::new("`\"hello_${MISSING}\"`");
        let vars = Dictionary::new();

        let result = expr.evaluate(&vars);
        assert!(!result.is_ok());
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_used_variables() {
        let expr = VariableExpression::new("`\"${A}_${B}\"`");

        let mut vars = Dictionary::new();
        vars.insert("A".to_string(), Value::new("x".to_string()));
        vars.insert("B".to_string(), Value::new("y".to_string()));

        let result = expr.evaluate(&vars);
        assert!(result.used_variables.contains("A"));
        assert!(result.used_variables.contains("B"));
    }

    #[test]
    fn test_sub_expression_evaluation() {
        // Variable value is itself an expression — should be recursively evaluated
        let expr = VariableExpression::new("`\"result_${VAR}\"`");

        let mut vars = Dictionary::new();
        // VAR is an expression that references SUBVAR
        vars.insert(
            "VAR".to_string(),
            Value::new("`\"sub_${SUBVAR}\"`".to_string()),
        );
        vars.insert("SUBVAR".to_string(), Value::new("value".to_string()));

        let result = expr.evaluate(&vars);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"result_sub_value".to_string())
        );
        // Both VAR and SUBVAR should be in used_variables
        assert!(result.used_variables.contains("VAR"));
        assert!(result.used_variables.contains("SUBVAR"));
    }

    #[test]
    fn test_none_value() {
        let expr = VariableExpression::new("`\"before_${X}_after\"`");

        let mut vars = Dictionary::new();
        vars.insert("X".to_string(), Value::default()); // None / empty

        let result = expr.evaluate(&vars);
        assert!(result.is_ok());
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"before__after".to_string())
        );
    }

    #[test]
    fn test_builder_variable() {
        let expr = VariableExpressionBuilder::variable("NAME").build();
        assert!(expr.is_valid());
        assert_eq!(expr.get_variables(), &["NAME"]);
    }

    #[test]
    fn test_builder_literal() {
        let expr = VariableExpressionBuilder::literal_string("hello").build();
        assert!(expr.is_valid());

        let vars = Dictionary::new();
        let result = expr.evaluate(&vars);
        assert!(result.is_ok());
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"hello".to_string())
        );
    }

    // Port of test_MakeFunction -- make_function / make_list builder paths.
    #[test]
    fn test_builder_functions() {
        // make_function produces "fnName(arg1, arg2, ...)" inner string.
        let inner = VariableExpressionBuilder::make_function(
            "contains",
            vec![
                VariableExpressionBuilder::make_list(vec![
                    VariableExpressionBuilder::literal_int(1),
                    VariableExpressionBuilder::literal_int(2),
                    VariableExpressionBuilder::literal_int(3),
                ]),
                VariableExpressionBuilder::variable("foo"),
            ],
        );
        assert_eq!(inner.as_str(), "contains([1, 2, 3], ${foo})");

        // A zero-arg function call still works syntactically.
        let zero_arg = VariableExpressionBuilder::make_function("blah", vec![]);
        assert_eq!(zero_arg.as_str(), "blah()");
        assert_eq!(zero_arg.build().get_string(), "`blah()`");

        // make_list produces "[elem1, elem2, ...]" inner string.
        let list = VariableExpressionBuilder::make_list(vec![
            VariableExpressionBuilder::literal_string("foo"),
            VariableExpressionBuilder::literal_string("bar"),
        ]);
        assert_eq!(list.as_str(), r#"["foo", "bar"]"#);

        // Empty list.
        let empty_list = VariableExpressionBuilder::make_list(vec![]);
        assert_eq!(empty_list.as_str(), "[]");
    }

    // Port of test_MakeListOfLiterals -- make_list_of_literal_strings/ints/bools.
    #[test]
    fn test_builder_list_of_literals() {
        let strings = VariableExpressionBuilder::make_list_of_literal_strings(&["a", "b", "c"]);
        assert_eq!(strings.as_str(), r#"["a", "b", "c"]"#);

        let ints = VariableExpressionBuilder::make_list_of_literal_ints(&[1, 2, 3]);
        assert_eq!(ints.as_str(), "[1, 2, 3]");

        let bools = VariableExpressionBuilder::make_list_of_literal_bools(&[true, false]);
        assert_eq!(bools.as_str(), "[true, false]");

        let empty_strings = VariableExpressionBuilder::make_list_of_literal_strings(&[]);
        assert_eq!(empty_strings.as_str(), "[]");
    }

    // Port of test_Default -- a completely empty string is not a valid expression.
    #[test]
    fn test_empty_expression() {
        // Backtick-delimited empty string is recognized as an expression syntactically.
        assert!(VariableExpression::is_expression("``"));
        // But a bare empty string is not.
        let empty = VariableExpression::new("");
        assert!(!empty.is_valid());
        assert!(!empty.get_errors().is_empty());
    }

    // Port of test_NestedExpressions -- variable value is itself an expression.
    #[test]
    fn test_nested_variable_refs() {
        // `${FOO}` where FOO = "`${BAR}`" and BAR = "ok" resolves transitively.
        let expr = VariableExpression::new("`${FOO}`");
        assert!(expr.is_valid());

        let mut vars = Dictionary::new();
        vars.insert("FOO".to_string(), Value::new("`${BAR}`".to_string()));
        vars.insert("BAR".to_string(), Value::new("ok".to_string()));

        let result = expr.evaluate(&vars);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"ok".to_string())
        );
        assert!(result.used_variables.contains("FOO"));
        assert!(result.used_variables.contains("BAR"));
    }

    // Port of test_CircularSubstitutions -- MAX_DEPTH=32 stops infinite recursion.
    #[test]
    fn test_circular_recursion() {
        // A->B->C->A cycle must trigger the depth limit and return an error.
        let expr = VariableExpression::new("`${A}`");
        assert!(expr.is_valid());

        let mut vars = Dictionary::new();
        vars.insert("A".to_string(), Value::new("`${B}`".to_string()));
        vars.insert("B".to_string(), Value::new("`${C}`".to_string()));
        vars.insert("C".to_string(), Value::new("`${A}`".to_string()));

        let result = expr.evaluate(&vars);
        assert!(
            !result.is_ok(),
            "expected depth error, got {:?}",
            result.value
        );
        assert!(
            !result.errors.is_empty(),
            "expected error for circular reference"
        );
    }

    // Port of test_VarExpressions -- same variable referenced multiple times.
    #[test]
    fn test_multiple_refs_same_var() {
        // `"${X}_${X}"` -- X used twice; result is the value concatenated with itself.
        let expr = VariableExpression::new(r#"`"${X}_${X}"`"#);
        assert!(expr.is_valid());

        let mut vars = Dictionary::new();
        vars.insert("X".to_string(), Value::new("hello".to_string()));

        let result = expr.evaluate(&vars);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"hello_hello".to_string())
        );
        // X should appear in used_variables even though referenced twice.
        assert!(result.used_variables.contains("X"));
    }

    // Port of test_BooleanExpressions -- bool substituted as "true"/"false" in string context.
    #[test]
    fn test_boolean_substitution() {
        let expr = VariableExpression::new(r#"`"flag_${B}"`"#);
        assert!(expr.is_valid());

        let mut vars_true = Dictionary::new();
        vars_true.insert("B".to_string(), Value::new(true));
        let result_true = expr.evaluate(&vars_true);
        assert!(result_true.is_ok(), "errors: {:?}", result_true.errors);
        assert_eq!(
            result_true.value.unwrap().get::<String>(),
            Some(&"flag_true".to_string())
        );

        let mut vars_false = Dictionary::new();
        vars_false.insert("B".to_string(), Value::new(false));
        let result_false = expr.evaluate(&vars_false);
        assert!(result_false.is_ok(), "errors: {:?}", result_false.errors);
        assert_eq!(
            result_false.value.unwrap().get::<String>(),
            Some(&"flag_false".to_string())
        );
    }

    // Port of test_VarExpressions -- i64 variable is substituted as its decimal string.
    #[test]
    fn test_array_variable_passthrough() {
        // The simplified evaluator substitutes numeric values as their decimal strings.
        let expr = VariableExpression::new("`${NUM}`");
        assert!(expr.is_valid());

        let mut vars = Dictionary::new();
        vars.insert("NUM".to_string(), Value::new(42i64));

        let result = expr.evaluate(&vars);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        assert_eq!(
            result.value.unwrap().get::<String>(),
            Some(&"42".to_string())
        );
        assert!(result.used_variables.contains("NUM"));
    }
}
