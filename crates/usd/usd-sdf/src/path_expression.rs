//! Path expressions for matching paths with patterns.
//!
//! Port of pxr/usd/sdf/pathExpression.h
//!
//! Path expressions represent a logical expression syntax tree consisting
//! of path patterns joined by set-algebraic operators:
//! - `+` (union)
//! - `&` (intersection)
//! - `-` (difference)
//! - `~` (complement)
//! - whitespace (implied union)

use crate::Path;
use std::fmt;

/// Enumerant describing a subexpression operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathExpressionOp {
    /// Complement (set negation).
    Complement,
    /// Implied union (whitespace separated).
    ImpliedUnion,
    /// Explicit union (+).
    Union,
    /// Intersection (&).
    Intersection,
    /// Difference (-).
    Difference,
    /// Expression reference.
    ExpressionRef,
    /// Pattern atom.
    Pattern,
}

/// A reference to another path expression.
///
/// Expression references start with `%` followed by a prim path, a `:`, and
/// a name. The special reference `%_` means "the weaker expression" when
/// composing expressions together.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ExpressionReference {
    /// Optional path reference.
    pub path: Path,
    /// Name is either a property name, or "_" for weaker reference.
    pub name: String,
}

impl ExpressionReference {
    /// Creates a new expression reference.
    pub fn new(path: Path, name: impl Into<String>) -> Self {
        Self {
            path,
            name: name.into(),
        }
    }

    /// Returns the special "weaker" reference, whose syntax is "%_".
    pub fn weaker() -> Self {
        Self {
            path: Path::empty(),
            name: "_".to_string(),
        }
    }

    /// Returns true if this is a weaker reference.
    pub fn is_weaker(&self) -> bool {
        self.name == "_" && self.path.is_empty()
    }
}

/// A path pattern that can match multiple paths.
///
/// Path patterns are similar to paths but may contain:
/// - Glob-style wildcards (`*`, `**`)
/// - `//` elements indicating arbitrary hierarchy depth
/// - Brace-enclosed predicate expressions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathPattern {
    /// The pattern prefix (exact path part).
    prefix: Path,
    /// Pattern components after the prefix.
    components: Vec<PatternComponent>,
}

/// A component of a path pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PatternComponent {
    /// Literal name.
    Literal(String),
    /// Wildcard matching any single component (*).
    Wildcard,
    /// Recursive wildcard matching any depth (//).
    RecursiveWildcard,
    /// Glob pattern (e.g., "foo*").
    Glob(String),
    /// Predicate expression in braces.
    Predicate(String),
}

impl PathPattern {
    /// Creates a pattern from an exact path.
    pub fn from_path(path: Path) -> Self {
        Self {
            prefix: path,
            components: Vec::new(),
        }
    }

    /// Creates a pattern matching all paths ("//").
    pub fn everything() -> Self {
        Self {
            prefix: Path::absolute_root(),
            components: vec![PatternComponent::RecursiveWildcard],
        }
    }

    /// Creates a pattern matching all descendants (".//").
    pub fn every_descendant() -> Self {
        Self {
            prefix: Path::reflexive_relative(),
            components: vec![PatternComponent::RecursiveWildcard],
        }
    }

    /// Returns the prefix path.
    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// Returns a new pattern with a different prefix, preserving components.
    pub fn with_prefix(&self, new_prefix: Path) -> Self {
        Self {
            prefix: new_prefix,
            components: self.components.clone(),
        }
    }

    /// Returns the pattern components.
    pub fn components(&self) -> &[PatternComponent] {
        &self.components
    }

    /// Returns true if this pattern has no wildcards (exact match).
    pub fn is_exact(&self) -> bool {
        self.components.is_empty()
    }

    /// Returns true if the pattern prefix is absolute.
    pub fn is_absolute(&self) -> bool {
        self.prefix.is_absolute_path()
    }

    /// Returns true if this pattern has a leading stretch (starts with //).
    pub fn has_leading_stretch(&self) -> bool {
        self.components
            .first()
            .map(|c| matches!(c, PatternComponent::RecursiveWildcard))
            .unwrap_or(false)
    }

    /// Returns true if this pattern has a trailing stretch (ends with //).
    pub fn has_trailing_stretch(&self) -> bool {
        self.components
            .last()
            .map(|c| matches!(c, PatternComponent::RecursiveWildcard))
            .unwrap_or(false)
    }

    /// Checks if a path matches this pattern.
    pub fn matches(&self, path: &Path) -> bool {
        if self.is_exact() {
            return path == &self.prefix;
        }

        // Must have the prefix
        if !path.has_prefix(&self.prefix) {
            return false;
        }

        // Get the suffix of the path after the prefix
        let path_str = path.get_string();
        let prefix_str = self.prefix.get_string();
        let suffix = if prefix_str == "/" {
            &path_str[1..] // Root prefix: everything after "/"
        } else if path_str.len() > prefix_str.len() {
            &path_str[prefix_str.len() + 1..] // Skip prefix + "/"
        } else {
            "" // Path equals prefix
        };

        let path_parts: Vec<&str> = if suffix.is_empty() {
            Vec::new()
        } else {
            suffix.split('/').collect()
        };

        // Match path parts against components using recursive matching
        Self::match_components(&self.components, &path_parts)
    }

    /// Recursively matches path parts against pattern components.
    ///
    /// Handles Wildcard (*), RecursiveWildcard (//), Glob, Literal,
    /// and Predicate components.
    fn match_components(components: &[PatternComponent], parts: &[&str]) -> bool {
        if components.is_empty() {
            return parts.is_empty();
        }

        let comp = &components[0];
        let rest_components = &components[1..];

        match comp {
            PatternComponent::RecursiveWildcard => {
                // Matches zero or more path components.
                // Try matching rest_components at every possible offset.
                for i in 0..=parts.len() {
                    if Self::match_components(rest_components, &parts[i..]) {
                        return true;
                    }
                }
                false
            }
            PatternComponent::Wildcard => {
                // Matches exactly one component (any name)
                if parts.is_empty() {
                    return false;
                }
                Self::match_components(rest_components, &parts[1..])
            }
            PatternComponent::Literal(name) => {
                // Matches exactly one component with the given name
                if parts.is_empty() {
                    return false;
                }
                if parts[0] != name {
                    return false;
                }
                Self::match_components(rest_components, &parts[1..])
            }
            PatternComponent::Glob(pattern) => {
                // Matches one component against a glob pattern
                if parts.is_empty() {
                    return false;
                }
                if !glob_match(pattern, parts[0]) {
                    return false;
                }
                Self::match_components(rest_components, &parts[1..])
            }
            PatternComponent::Predicate(_) => {
                // Predicate components match structurally (predicate is
                // evaluated externally via PredicateLibrary).
                // A predicate acts like a wildcard for structural matching.
                if parts.is_empty() {
                    return false;
                }
                Self::match_components(rest_components, &parts[1..])
            }
        }
    }
}

/// Matches a string against a glob pattern.
///
/// Supports `*` (match any sequence) and `?` (match single char).
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_impl(&p, &t)
}

fn glob_match_impl(pattern: &[char], text: &[char]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }

    match pattern[0] {
        '*' => {
            // '*' matches zero or more characters
            // Try matching rest of pattern at every position
            for i in 0..=text.len() {
                if glob_match_impl(&pattern[1..], &text[i..]) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // '?' matches exactly one character
            if text.is_empty() {
                return false;
            }
            glob_match_impl(&pattern[1..], &text[1..])
        }
        ch => {
            // Literal character
            if text.is_empty() || text[0] != ch {
                return false;
            }
            glob_match_impl(&pattern[1..], &text[1..])
        }
    }
}

impl From<Path> for PathPattern {
    fn from(path: Path) -> Self {
        Self::from_path(path)
    }
}

/// A path expression that can match paths using patterns and set operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathExpression {
    /// The expression tree nodes.
    nodes: Vec<ExpressionNode>,
    /// Expression references.
    refs: Vec<ExpressionReference>,
    /// Patterns used in this expression.
    patterns: Vec<PathPattern>,
    /// Parse error, if any.
    parse_error: Option<String>,
}

/// A node in the expression tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExpressionNode {
    /// An operation with indices to operands.
    Op {
        op: PathExpressionOp,
        left: usize,
        right: Option<usize>,
    },
    /// Reference to a pattern.
    Pattern(usize),
    /// Reference to an expression reference.
    Ref(usize),
}

impl PathExpression {
    /// Creates an empty expression that matches nothing.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            refs: Vec::new(),
            patterns: Vec::new(),
            parse_error: None,
        }
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

        // Simple parsing for common cases
        if expr == "//" {
            return Self::everything();
        }

        if expr == ".//" {
            return Self::every_descendant();
        }

        if expr == "%_" {
            return Self::weaker_ref();
        }

        // Parse full grammar
        match parse_expression(expr) {
            Ok(result) => result,
            Err(err_msg) => {
                let full_err = if let Some(ctx) = context {
                    format!("{}: {}", ctx, err_msg)
                } else {
                    err_msg
                };
                Self {
                    nodes: Vec::new(),
                    refs: Vec::new(),
                    patterns: Vec::new(),
                    parse_error: Some(full_err),
                }
            }
        }
    }

    /// Returns the expression "//" which matches all paths.
    pub fn everything() -> Self {
        Self::make_atom_pattern(PathPattern::everything())
    }

    /// Returns the expression ".//" which matches all descendants.
    pub fn every_descendant() -> Self {
        Self::make_atom_pattern(PathPattern::every_descendant())
    }

    /// Returns true if the expression is trivial for light linking: it matches
    /// all prims (everything) or all prim paths but not properties.
    /// Port of HdsiLightLinkingSceneIndex_Impl::_Cache::IsTrivial.
    pub fn is_trivial(&self) -> bool {
        *self == Self::everything() || *self == Self::parse("~//*.*")
    }

    /// Returns the empty expression which matches nothing.
    pub fn nothing() -> Self {
        Self::new()
    }

    /// Returns the expression "%_" (weaker reference).
    pub fn weaker_ref() -> Self {
        Self::make_atom_ref(ExpressionReference::weaker())
    }

    /// Creates an expression containing only a pattern.
    pub fn make_atom_pattern(pattern: PathPattern) -> Self {
        Self {
            nodes: vec![ExpressionNode::Pattern(0)],
            refs: Vec::new(),
            patterns: vec![pattern],
            parse_error: None,
        }
    }

    /// Creates an expression containing only a path.
    pub fn make_atom_path(path: Path) -> Self {
        Self::make_atom_pattern(PathPattern::from_path(path))
    }

    /// Creates an expression containing only a reference.
    pub fn make_atom_ref(reference: ExpressionReference) -> Self {
        Self {
            nodes: vec![ExpressionNode::Ref(0)],
            refs: vec![reference],
            patterns: Vec::new(),
            parse_error: None,
        }
    }

    /// Creates the complement of an expression.
    pub fn make_complement(expr: Self) -> Self {
        if expr.is_empty() {
            return Self::everything();
        }

        let mut result = expr;
        let root = result.nodes.len().saturating_sub(1);
        result.nodes.push(ExpressionNode::Op {
            op: PathExpressionOp::Complement,
            left: root,
            right: None,
        });
        result
    }

    /// Creates a binary operation expression.
    pub fn make_op(op: PathExpressionOp, left: Self, right: Self) -> Self {
        match op {
            PathExpressionOp::Complement => {
                // Complement is unary
                Self::make_complement(left)
            }
            PathExpressionOp::ImpliedUnion
            | PathExpressionOp::Union
            | PathExpressionOp::Intersection
            | PathExpressionOp::Difference => {
                if left.is_empty() {
                    return right;
                }
                if right.is_empty() {
                    return left;
                }

                let mut result = Self::new();

                // Merge patterns
                let left_pattern_offset = 0;
                let right_pattern_offset = left.patterns.len();
                result.patterns.extend(left.patterns);
                result.patterns.extend(right.patterns);

                // Merge refs
                let left_ref_offset = 0;
                let right_ref_offset = left.refs.len();
                result.refs.extend(left.refs);
                result.refs.extend(right.refs);

                // Merge nodes with adjusted indices
                let left_node_offset = 0usize;
                let left_node_count = left.nodes.len();
                let right_node_offset = left_node_count;
                let right_node_count = right.nodes.len();

                for node in left.nodes {
                    result.nodes.push(adjust_node(
                        node,
                        left_pattern_offset,
                        left_ref_offset,
                        left_node_offset,
                    ));
                }
                for node in right.nodes {
                    result.nodes.push(adjust_node(
                        node,
                        right_pattern_offset,
                        right_ref_offset,
                        right_node_offset,
                    ));
                }

                // Each subtree's root is its last node in the merged array.
                let left_root = left_node_offset + left_node_count - 1;
                let right_root = right_node_offset + right_node_count - 1;
                result.nodes.push(ExpressionNode::Op {
                    op,
                    left: left_root,
                    right: Some(right_root),
                });

                result
            }
            _ => left,
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

    /// Returns true if all patterns are absolute.
    pub fn is_absolute(&self) -> bool {
        self.patterns.iter().all(|p| p.is_absolute())
    }

    /// Returns true if this expression contains references.
    pub fn contains_expression_references(&self) -> bool {
        !self.refs.is_empty()
    }

    /// Returns true if this expression contains a weaker reference.
    pub fn contains_weaker_expression_reference(&self) -> bool {
        self.refs.iter().any(|r| r.is_weaker())
    }

    /// Returns true if this expression is complete (absolute with no references).
    pub fn is_complete(&self) -> bool {
        !self.contains_expression_references() && self.is_absolute()
    }

    /// Returns the patterns used in this expression.
    ///
    /// Matches C++ `GetPatterns()`.
    pub fn get_patterns(&self) -> &[PathPattern] {
        &self.patterns
    }

    /// Returns the top-level operation, if any.
    ///
    /// This is a simplified version that returns the operation from the first node
    /// if it's an operation node. Only returns operations that are relevant for
    /// expression evaluation (Union, Intersection, Difference, Complement).
    pub fn op(&self) -> Option<PathExpressionOp> {
        if let Some(node) = self.nodes.first() {
            if let ExpressionNode::Op { op, .. } = node {
                match op {
                    PathExpressionOp::Union
                    | PathExpressionOp::Intersection
                    | PathExpressionOp::Difference
                    | PathExpressionOp::Complement => Some(*op),
                    // Don't return ImpliedUnion, ExpressionRef, or Pattern as they're not top-level operations
                    PathExpressionOp::ImpliedUnion
                    | PathExpressionOp::ExpressionRef
                    | PathExpressionOp::Pattern => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Replaces path prefixes in the expression.
    pub fn replace_prefix(&self, old_prefix: &Path, new_prefix: &Path) -> Self {
        let mut result = self.clone();
        for pattern in &mut result.patterns {
            if pattern.prefix.has_prefix(old_prefix) {
                if let Some(new_path) = pattern.prefix.replace_prefix(old_prefix, new_prefix) {
                    pattern.prefix = new_path;
                }
            }
        }
        result
    }

    /// Walks this expression's syntax tree in depth-first order.
    ///
    /// Matches C++ `Walk()` method.
    /// Calls `pattern` with the current PathPattern when one is encountered,
    /// `ref_callback` with the current ExpressionReference when one is encountered,
    /// and `logic` multiple times for each logical operation encountered.
    ///
    /// When calling `logic`, the logical operation is passed as the `Op` parameter,
    /// and an integer indicating "where" we are in the set of operands is passed.
    /// For a Complement, call `logic(Op=Complement, int=0)` to start, then after
    /// the subexpression that the Complement applies to is walked, call
    /// `logic(Op=Complement, int=1)`. For binary operators like Union and
    /// Intersection, call `logic(Op, 0)` before the first argument, then
    /// `logic(Op, 1)` after the first subexpression, then `logic(Op, 2)` after
    /// the second subexpression.
    pub fn walk<FLogic, FRef, FPattern>(
        &self,
        mut logic: FLogic,
        mut ref_callback: FRef,
        mut pattern: FPattern,
    ) where
        FLogic: FnMut(PathExpressionOp, i32),
        FRef: FnMut(&ExpressionReference),
        FPattern: FnMut(&PathPattern),
    {
        if self.is_empty() {
            return;
        }

        // Helper to walk a node recursively
        fn walk_node<FLogic, FRef, FPattern>(
            nodes: &[ExpressionNode],
            patterns: &[PathPattern],
            refs: &[ExpressionReference],
            node_idx: usize,
            logic: &mut FLogic,
            ref_callback: &mut FRef,
            pattern: &mut FPattern,
        ) where
            FLogic: FnMut(PathExpressionOp, i32),
            FRef: FnMut(&ExpressionReference),
            FPattern: FnMut(&PathPattern),
        {
            if node_idx >= nodes.len() {
                return;
            }

            match &nodes[node_idx] {
                ExpressionNode::Pattern(pattern_idx) => {
                    if *pattern_idx < patterns.len() {
                        pattern(&patterns[*pattern_idx]);
                    }
                }
                ExpressionNode::Ref(ref_idx) => {
                    if *ref_idx < refs.len() {
                        ref_callback(&refs[*ref_idx]);
                    }
                }
                ExpressionNode::Op { op, left, right } => {
                    // Call logic before processing operands
                    logic(*op, 0);

                    // Process left operand
                    if *op == PathExpressionOp::Complement {
                        // Complement is unary - process left as the operand
                        walk_node(nodes, patterns, refs, *left, logic, ref_callback, pattern);
                        // Call logic after operand
                        logic(*op, 1);
                    } else {
                        // Binary operation
                        // Process left operand
                        walk_node(nodes, patterns, refs, *left, logic, ref_callback, pattern);
                        // Call logic after first operand
                        logic(*op, 1);

                        // Process right operand if present
                        if let Some(right_idx) = right {
                            walk_node(
                                nodes,
                                patterns,
                                refs,
                                *right_idx,
                                logic,
                                ref_callback,
                                pattern,
                            );
                        }
                        // Call logic after second operand
                        logic(*op, 2);
                    }
                }
            }
        }

        // Start walking from the root node (last node in the vector, as it's typically the top-level op)
        if !self.nodes.is_empty() {
            let root_idx = self.nodes.len() - 1;
            walk_node(
                &self.nodes,
                &self.patterns,
                &self.refs,
                root_idx,
                &mut logic,
                &mut ref_callback,
                &mut pattern,
            );
        }
    }

    /// Walks this expression's syntax tree, providing the full op stack context.
    ///
    /// Similar to `walk()` but the `logic` callback receives the entire
    /// op stack (as a slice of `(PathExpressionOp, i32)`) instead of just the
    /// current op and arg_index. The top of the stack is the last element.
    ///
    /// Matches C++ `SdfPathExpression::WalkWithOpStack()`.
    pub fn walk_with_op_stack<FLogic, FRef, FPattern>(
        &self,
        mut logic: FLogic,
        mut ref_callback: FRef,
        mut pattern: FPattern,
    ) where
        FLogic: FnMut(&[(PathExpressionOp, i32)]),
        FRef: FnMut(&ExpressionReference),
        FPattern: FnMut(&PathPattern),
    {
        if self.is_empty() {
            return;
        }

        fn walk_node_with_stack<FLogic, FRef, FPattern>(
            nodes: &[ExpressionNode],
            patterns: &[PathPattern],
            refs: &[ExpressionReference],
            node_idx: usize,
            op_stack: &mut Vec<(PathExpressionOp, i32)>,
            logic: &mut FLogic,
            ref_callback: &mut FRef,
            pattern: &mut FPattern,
        ) where
            FLogic: FnMut(&[(PathExpressionOp, i32)]),
            FRef: FnMut(&ExpressionReference),
            FPattern: FnMut(&PathPattern),
        {
            if node_idx >= nodes.len() {
                return;
            }

            match &nodes[node_idx] {
                ExpressionNode::Pattern(pattern_idx) => {
                    if *pattern_idx < patterns.len() {
                        pattern(&patterns[*pattern_idx]);
                    }
                }
                ExpressionNode::Ref(ref_idx) => {
                    if *ref_idx < refs.len() {
                        ref_callback(&refs[*ref_idx]);
                    }
                }
                ExpressionNode::Op { op, left, right } => {
                    op_stack.push((*op, 0));
                    logic(op_stack);

                    if *op == PathExpressionOp::Complement {
                        walk_node_with_stack(
                            nodes,
                            patterns,
                            refs,
                            *left,
                            op_stack,
                            logic,
                            ref_callback,
                            pattern,
                        );
                        if let Some(last) = op_stack.last_mut() {
                            last.1 = 1;
                        }
                        logic(op_stack);
                    } else {
                        walk_node_with_stack(
                            nodes,
                            patterns,
                            refs,
                            *left,
                            op_stack,
                            logic,
                            ref_callback,
                            pattern,
                        );
                        if let Some(last) = op_stack.last_mut() {
                            last.1 = 1;
                        }
                        logic(op_stack);

                        if let Some(right_idx) = right {
                            walk_node_with_stack(
                                nodes,
                                patterns,
                                refs,
                                *right_idx,
                                op_stack,
                                logic,
                                ref_callback,
                                pattern,
                            );
                        }
                        if let Some(last) = op_stack.last_mut() {
                            last.1 = 2;
                        }
                        logic(op_stack);
                    }

                    op_stack.pop();
                }
            }
        }

        if !self.nodes.is_empty() {
            let root_idx = self.nodes.len() - 1;
            let mut op_stack: Vec<(PathExpressionOp, i32)> = Vec::new();
            walk_node_with_stack(
                &self.nodes,
                &self.patterns,
                &self.refs,
                root_idx,
                &mut op_stack,
                &mut logic,
                &mut ref_callback,
                &mut pattern,
            );
        }
    }

    /// Returns a text representation of this expression.
    ///
    /// Matches C++ `SdfPathExpression::GetText()`.
    pub fn get_text(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        if self.nodes.is_empty() {
            return String::new();
        }
        self.node_to_text(self.nodes.len() - 1)
    }

    fn node_to_text(&self, idx: usize) -> String {
        if idx >= self.nodes.len() {
            return String::new();
        }
        match &self.nodes[idx] {
            ExpressionNode::Pattern(pattern_idx) => {
                if *pattern_idx < self.patterns.len() {
                    Self::pattern_to_text(&self.patterns[*pattern_idx])
                } else {
                    String::new()
                }
            }
            ExpressionNode::Ref(ref_idx) => {
                if *ref_idx < self.refs.len() {
                    let r = &self.refs[*ref_idx];
                    if r.is_weaker() {
                        "%_".to_string()
                    } else {
                        format!("%{}:{}", r.path.as_str(), r.name)
                    }
                } else {
                    String::new()
                }
            }
            ExpressionNode::Op { op, left, right } => match op {
                PathExpressionOp::Complement => {
                    let inner = self.node_to_text(*left);
                    format!("~{}", inner)
                }
                _ => {
                    let left_str = self.node_to_text(*left);
                    let right_str = if let Some(r) = right {
                        self.node_to_text(*r)
                    } else {
                        String::new()
                    };
                    let op_str = match op {
                        PathExpressionOp::Union => " + ",
                        PathExpressionOp::ImpliedUnion => " ",
                        PathExpressionOp::Intersection => " & ",
                        PathExpressionOp::Difference => " - ",
                        _ => " ",
                    };
                    format!("{}{}{}", left_str, op_str, right_str)
                }
            },
        }
    }

    fn pattern_to_text(pattern: &PathPattern) -> String {
        let mut result = pattern.prefix.as_str().to_string();
        for comp in &pattern.components {
            if !result.is_empty() && !result.ends_with('/') {
                result.push('/');
            }
            match comp {
                PatternComponent::Literal(s) => result.push_str(s),
                PatternComponent::Wildcard => result.push('*'),
                PatternComponent::RecursiveWildcard => result.push('/'),
                PatternComponent::Glob(g) => result.push_str(g),
                PatternComponent::Predicate(p) => {
                    result.push('{');
                    result.push_str(p);
                    result.push('}');
                }
            }
        }
        result
    }

    /// Maps paths in the expression through a mapping function.
    ///
    /// Applies the mapping function to all path patterns and expression references.
    /// Returns None if any path cannot be mapped.
    pub fn map_paths<F>(&self, map_fn: F) -> Option<Self>
    where
        F: Fn(&Path) -> Option<Path>,
    {
        let mut result = self.clone();
        let mut has_unmappable = false;

        // Map all path patterns
        for pattern in &mut result.patterns {
            if let Some(mapped_prefix) = map_fn(&pattern.prefix) {
                pattern.prefix = mapped_prefix;
            } else {
                has_unmappable = true;
            }
        }

        // Map expression references
        for ref_expr in &mut result.refs {
            if ref_expr.path.is_empty() {
                // Empty path refs (like %_) are preserved
                continue;
            }
            if let Some(mapped_path) = map_fn(&ref_expr.path) {
                ref_expr.path = mapped_path;
            } else {
                has_unmappable = true;
            }
        }

        if has_unmappable { None } else { Some(result) }
    }

    /// Makes relative paths absolute using the given anchor.
    pub fn make_absolute(&self, anchor: &Path) -> Self {
        let mut result = self.clone();
        for pattern in &mut result.patterns {
            if !pattern.is_absolute() {
                if let Some(abs_path) = pattern.prefix.make_absolute(anchor) {
                    pattern.prefix = abs_path;
                }
            }
        }
        result
    }

    /// Resolves all expression references in this expression.
    ///
    /// Matches C++ `ResolveReferences()`.
    /// Walks the expression tree and replaces each `ExpressionReference` with
    /// the result of calling `resolve` on that reference.
    ///
    /// # Arguments
    /// * `resolve` - A function that maps an `ExpressionReference` to a `PathExpression`
    ///
    /// # Returns
    /// A new `PathExpression` with all references resolved.
    pub fn resolve_references<F>(&self, resolve: F) -> Self
    where
        F: Fn(&ExpressionReference) -> PathExpression,
    {
        if self.is_empty() {
            return Self::new();
        }

        // Stack-based approach matching C++ implementation
        use std::cell::RefCell;
        use std::rc::Rc;
        let stack: Rc<RefCell<Vec<PathExpression>>> = Rc::new(RefCell::new(Vec::new()));

        // Logic callback handles operations
        let stack_clone = stack.clone();
        let mut logic = move |op: PathExpressionOp, arg_index: i32| {
            match op {
                PathExpressionOp::Complement => {
                    if arg_index == 1 {
                        // After processing the operand, apply complement
                        let expr = stack_clone.borrow_mut().pop().unwrap_or_default();
                        stack_clone.borrow_mut().push(Self::make_complement(expr));
                    }
                }
                PathExpressionOp::Union
                | PathExpressionOp::ImpliedUnion
                | PathExpressionOp::Intersection
                | PathExpressionOp::Difference => {
                    if arg_index == 2 {
                        // After processing both operands, apply binary operation
                        let right = stack_clone.borrow_mut().pop().unwrap_or_default();
                        let left = stack_clone.borrow_mut().pop().unwrap_or_default();
                        stack_clone
                            .borrow_mut()
                            .push(Self::make_op(op, left, right));
                    }
                }
                _ => {
                    // Other operations don't need special handling here
                }
            }
        };

        // Reference callback resolves references and pushes results
        let stack_clone2 = stack.clone();
        let mut resolve_ref = move |ref_expr: &ExpressionReference| {
            let resolved = resolve(ref_expr);
            stack_clone2.borrow_mut().push(resolved);
        };

        // Pattern callback pushes patterns as atoms
        let stack_clone3 = stack.clone();
        let mut pattern_ident = move |pattern: &PathPattern| {
            stack_clone3
                .borrow_mut()
                .push(Self::make_atom_pattern(pattern.clone()));
        };

        // Walk the expression tree, resolving references
        self.walk(&mut logic, &mut resolve_ref, &mut pattern_ident);

        // Stack should contain exactly one element - the resolved expression
        let stack_borrowed = stack.borrow();
        if stack_borrowed.len() == 1 {
            stack_borrowed[0].clone()
        } else if stack_borrowed.is_empty() {
            Self::new()
        } else {
            // Multiple elements on stack - this shouldn't happen with correct walk
            // Combine them with union as fallback
            let mut result = stack_borrowed[stack_borrowed.len() - 1].clone();
            drop(stack_borrowed);
            let mut stack_borrowed = stack.borrow_mut();
            while stack_borrowed.len() > 1 {
                let right = stack_borrowed.pop().expect("stack has items");
                let left = stack_borrowed.pop().expect("stack has items");
                result = Self::make_op(PathExpressionOp::Union, left, right);
                stack_borrowed.push(result.clone());
            }
            result
        }
    }

    /// Composes this expression over a weaker expression.
    pub fn compose_over(&self, weaker: &PathExpression) -> Self {
        if !self.contains_weaker_expression_reference() {
            return self.clone();
        }

        // Use resolve_references to replace %_ with weaker expression
        self.resolve_references(|ref_expr| {
            if ref_expr.is_weaker() {
                weaker.clone()
            } else {
                // For non-weaker references, return as-is (as atom)
                Self::make_atom_ref(ref_expr.clone())
            }
        })
    }
}

/// Adjusts indices in a node when merging expressions.
fn adjust_node(
    node: ExpressionNode,
    pattern_offset: usize,
    ref_offset: usize,
    node_offset: usize,
) -> ExpressionNode {
    match node {
        ExpressionNode::Pattern(idx) => ExpressionNode::Pattern(idx + pattern_offset),
        ExpressionNode::Ref(idx) => ExpressionNode::Ref(idx + ref_offset),
        ExpressionNode::Op { op, left, right } => ExpressionNode::Op {
            op,
            left: left + node_offset,
            right: right.map(|r| r + node_offset),
        },
    }
}

impl Default for PathExpression {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PathExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_text())
    }
}

impl From<&str> for PathExpression {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

impl From<String> for PathExpression {
    fn from(s: String) -> Self {
        Self::parse(&s)
    }
}

impl From<Path> for PathExpression {
    fn from(path: Path) -> Self {
        Self::make_atom_path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_expression() {
        let expr = PathExpression::new();
        assert!(expr.is_empty());
    }

    #[test]
    fn test_everything() {
        let expr = PathExpression::everything();
        assert!(!expr.is_empty());
        assert!(expr.is_absolute());
    }

    #[test]
    fn test_parse_path() {
        let expr = PathExpression::parse("/World/Cube");
        assert!(!expr.is_empty());
        assert!(expr.is_absolute());
        assert!(expr.is_complete());
    }

    #[test]
    fn test_weaker_ref() {
        let expr = PathExpression::weaker_ref();
        assert!(!expr.is_empty());
        assert!(expr.contains_weaker_expression_reference());
    }

    #[test]
    fn test_expression_reference() {
        let weaker = ExpressionReference::weaker();
        assert!(weaker.is_weaker());
        assert_eq!(weaker.name, "_");
    }

    #[test]
    fn test_pattern_matches() {
        let pattern = PathPattern::from_path(Path::from_string("/World/Cube").unwrap());
        assert!(pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World/Sphere").unwrap()));
    }

    #[test]
    fn test_everything_pattern() {
        let pattern = PathPattern::everything();
        assert!(pattern.matches(&Path::from_string("/World").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/Cube/Child").unwrap()));
    }

    #[test]
    fn test_wildcard_pattern() {
        // /World/* should match /World/Cube, /World/Sphere, but not /World/Cube/Child
        let pattern = PathPattern {
            prefix: Path::from_string("/World").unwrap(),
            components: vec![PatternComponent::Wildcard],
        };
        assert!(pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/Sphere").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World/Cube/Child").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World").unwrap()));
    }

    #[test]
    fn test_glob_pattern() {
        // /World/Cube* should match /World/Cube, /World/CubeShape, but not /World/Sphere
        let pattern = PathPattern {
            prefix: Path::from_string("/World").unwrap(),
            components: vec![PatternComponent::Glob("Cube*".to_string())],
        };
        assert!(pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/CubeShape").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World/Sphere").unwrap()));
    }

    #[test]
    fn test_literal_component() {
        // /World/Cube/Child as prefix + literal
        let pattern = PathPattern {
            prefix: Path::from_string("/World").unwrap(),
            components: vec![PatternComponent::Literal("Cube".to_string())],
        };
        assert!(pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World/Sphere").unwrap()));
    }

    #[test]
    fn test_recursive_wildcard_pattern() {
        // /World// matches anything under /World
        let pattern = PathPattern {
            prefix: Path::from_string("/World").unwrap(),
            components: vec![PatternComponent::RecursiveWildcard],
        };
        assert!(pattern.matches(&Path::from_string("/World").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/Cube/Child").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/A/B/C/D").unwrap()));
    }

    #[test]
    fn test_multi_component_pattern() {
        // /World/*/Child - matches /World/X/Child for any X
        let pattern = PathPattern {
            prefix: Path::from_string("/World").unwrap(),
            components: vec![
                PatternComponent::Wildcard,
                PatternComponent::Literal("Child".to_string()),
            ],
        };
        assert!(pattern.matches(&Path::from_string("/World/Cube/Child").unwrap()));
        assert!(pattern.matches(&Path::from_string("/World/Sphere/Child").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(!pattern.matches(&Path::from_string("/World/Cube/Other").unwrap()));
    }

    #[test]
    fn test_glob_match_fn() {
        assert!(glob_match("foo*", "foobar"));
        assert!(glob_match("foo*", "foo"));
        assert!(!glob_match("foo*", "bar"));
        assert!(glob_match("*bar", "foobar"));
        assert!(glob_match("f?o", "foo"));
        assert!(!glob_match("f?o", "fooo"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
    }

    #[test]
    fn test_parse_union() {
        let expr = PathExpression::parse("/World/Cube + /World/Sphere");
        assert!(!expr.is_empty());
        assert!(expr.is_complete());
    }

    #[test]
    fn test_parse_intersection() {
        let expr = PathExpression::parse("/World/Cube & /World/Sphere");
        assert!(!expr.is_empty());
    }

    #[test]
    fn test_parse_difference() {
        let expr = PathExpression::parse("/World/Cube - /World/Sphere");
        assert!(!expr.is_empty());
    }

    #[test]
    fn test_parse_complement() {
        let expr = PathExpression::parse("~ /World/Cube");
        assert!(!expr.is_empty());
    }

    #[test]
    fn test_parse_parentheses() {
        let expr = PathExpression::parse("(/World/Cube + /World/Sphere) & /World");
        assert!(!expr.is_empty());
    }

    #[test]
    fn test_parse_expression_reference() {
        let expr = PathExpression::parse("%/World:myExpr");
        assert!(!expr.is_empty());
        assert!(expr.contains_expression_references());
    }

    #[test]
    fn test_parse_implied_union() {
        let expr = PathExpression::parse("/World/Cube /World/Sphere");
        assert!(!expr.is_empty());
    }

    #[test]
    fn test_resolve_references() {
        // Test resolving weaker reference
        let weaker_expr = PathExpression::parse("/World/Sphere");
        let expr_with_ref = PathExpression::parse("%_");
        let resolved = expr_with_ref.resolve_references(|ref_expr| {
            if ref_expr.is_weaker() {
                weaker_expr.clone()
            } else {
                PathExpression::new()
            }
        });
        assert!(!resolved.is_empty());
        assert!(resolved.is_complete());
    }

    #[test]
    fn test_resolve_references_empty() {
        let empty = PathExpression::new();
        let resolved = empty.resolve_references(|_| PathExpression::everything());
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_resolve_references_pattern() {
        // Expression with just a pattern (no references) should remain unchanged
        let expr = PathExpression::parse("/World/Cube");
        let resolved = expr.resolve_references(|_| PathExpression::new());
        assert_eq!(expr.is_complete(), resolved.is_complete());
    }

    // Port of test_ComposeOver -- compose_over replaces %_ with weaker expression.
    #[test]
    fn test_compose_over() {
        let a = PathExpression::parse("/a");
        let b = PathExpression::parse("%_ /b");
        let c = PathExpression::parse("%_ /c");

        // Only b and c contain expression references (%_).
        assert!(!a.contains_expression_references());
        assert!(!a.contains_weaker_expression_reference());
        assert!(b.contains_expression_references());
        assert!(b.contains_weaker_expression_reference());
        assert!(c.contains_expression_references());
        assert!(c.contains_weaker_expression_reference());

        // Compose c over b over a: c(%_=/b(%_=/a)) => /a /b /c
        let composed = c.compose_over(&b.compose_over(&a));

        // After full composition the result must be complete (no references, absolute).
        assert!(!composed.contains_expression_references());
        assert!(!composed.contains_weaker_expression_reference());
        assert!(composed.is_complete());

        // Verify the composed expression matches /a, /b, /c.
        let text = composed.get_text();
        assert!(!text.is_empty(), "composed expression must have text");

        // The composed text must contain each contributing path.
        assert!(
            text.contains("/a"),
            "composed text should mention /a, got: {text}"
        );
        assert!(
            text.contains("/b"),
            "composed text should mention /b, got: {text}"
        );
        assert!(
            text.contains("/c"),
            "composed text should mention /c, got: {text}"
        );
    }

    // Port of test_MakeAbsolute -- relative paths are anchored to the given path.
    #[test]
    fn test_make_absolute() {
        // "foo ../bar baz//qux" relative to /World/test
        // => "/World/test/foo /World/bar /World/test/baz//qux"
        let anchor = Path::from_string("/World/test").expect("valid path");

        let relative = PathExpression::parse("foo");
        assert!(!relative.is_absolute());

        let absolute = relative.make_absolute(&anchor);
        assert!(
            absolute.is_absolute(),
            "make_absolute should produce absolute expression"
        );
        assert!(absolute.is_complete());

        // The absolutized text must start with the anchor prefix.
        let text = absolute.get_text();
        assert!(
            text.starts_with("/World/test"),
            "expected /World/test prefix, got: {text}"
        );
    }

    // Port of test_ReplacePrefix -- path prefixes are renamed throughout.
    #[test]
    fn test_replace_prefix() {
        let expr = PathExpression::parse("/World/test/foo /World/bar");
        let old_prefix = Path::from_string("/World").expect("valid path");
        let new_prefix = Path::from_string("/Home").expect("valid path");

        let replaced = expr.replace_prefix(&old_prefix, &new_prefix);

        let text = replaced.get_text();
        assert!(
            !text.contains("/World"),
            "no /World paths should remain after replace_prefix, got: {text}"
        );
        assert!(
            text.contains("/Home"),
            "replaced text should contain /Home, got: {text}"
        );
    }

    // Port of test_SceneDescription -- parse and round-trip a path expression string.
    // (Scene description integration without a live Layer requires only parse/get_text.)
    #[test]
    fn test_scene_description_round_trip() {
        // Relative path "child" is a valid path expression.
        let expr = PathExpression::parse("child");
        assert!(!expr.is_empty(), "parse should succeed for 'child'");
        assert!(expr.parse_error().is_none(), "no parse error expected");

        // An absolute path expression round-trips through get_text.
        let abs_expr = PathExpression::parse("/prim/child");
        assert!(abs_expr.is_absolute());
        assert!(abs_expr.is_complete());
        let text = abs_expr.get_text();
        assert_eq!(text, "/prim/child", "get_text round-trip failed: {text}");
    }
}

// ============================================================================
// Parser Implementation
// ============================================================================

/// Parser state for path expressions.
struct Parser {
    /// Input string being parsed.
    input: Vec<char>,
    /// Current position in input.
    pos: usize,
    /// Current character.
    current: Option<char>,
}

impl Parser {
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

    /// Returns the first non-whitespace character ahead without consuming input.
    fn peek_non_ws(&self) -> Option<char> {
        let mut i = self.pos;
        while i < self.input.len() {
            let ch = self.input[i];
            if !ch.is_whitespace() {
                return Some(ch);
            }
            i += 1;
        }
        None
    }

    /// Checks if we're at the end of input.
    fn is_eof(&self) -> bool {
        self.current.is_none()
    }

    /// Parses a complete expression.
    fn parse_expression(&mut self) -> Result<PathExpression, String> {
        self.skip_whitespace();
        if self.is_eof() {
            return Ok(PathExpression::new());
        }

        // Parse with operator precedence
        // Precedence (lowest to highest):
        // 1. Implied union (whitespace)
        // 2. Union (+)
        // 3. Difference (-)
        // 4. Intersection (&)
        // 5. Complement (~)
        // 6. Atoms (patterns, refs, parentheses)

        self.parse_union_expression()
    }

    /// Parses union expressions (lowest precedence).
    fn parse_union_expression(&mut self) -> Result<PathExpression, String> {
        let mut left = self.parse_difference_expression()?;

        // Do NOT skip whitespace here — whitespace is the implied-union operator
        // and must be detected inside the loop, not consumed before it.
        while let Some(ch) = self.current {
            if ch == '+' {
                self.advance();
                self.skip_whitespace();
                let right = self.parse_difference_expression()?;
                left = PathExpression::make_op(PathExpressionOp::Union, left, right);
            } else if ch.is_whitespace() {
                // Peek past whitespace to decide what comes next.
                let next = self.peek_non_ws();
                match next {
                    // EOF or closing paren: trailing whitespace, stop.
                    None | Some(')') => break,
                    // Explicit binary operators with surrounding spaces
                    // (e.g. "a + b", "a & b", "a - b"): skip the leading
                    // whitespace and let the next loop iteration handle them.
                    Some('+') => {
                        self.skip_whitespace();
                        // Loop will re-enter and match ch == '+'
                    }
                    // '&' and '-' are handled by deeper sub-parsers via
                    // peek_non_ws, so they should not reach here. If they do
                    // (e.g. top-level "a & b" without parentheses), treat the
                    // whitespace as trailing and stop.
                    Some(c) if is_operator_char(c) && c != '~' => break,
                    // Anything else is an implied union.
                    _ => {
                        self.skip_whitespace();
                        let right = self.parse_difference_expression()?;
                        left = PathExpression::make_op(PathExpressionOp::ImpliedUnion, left, right);
                    }
                }
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parses difference expressions (-).
    fn parse_difference_expression(&mut self) -> Result<PathExpression, String> {
        let mut left = self.parse_intersection_expression()?;

        // Peek past optional whitespace to check for '-'. If found, consume
        // whitespace + operator. If not found, leave whitespace in place so
        // the implied-union level above can detect it.
        while self.peek_non_ws() == Some('-') {
            self.skip_whitespace();
            self.advance(); // consume '-'
            self.skip_whitespace();
            let right = self.parse_intersection_expression()?;
            left = PathExpression::make_op(PathExpressionOp::Difference, left, right);
        }

        Ok(left)
    }

    /// Parses intersection expressions (&).
    fn parse_intersection_expression(&mut self) -> Result<PathExpression, String> {
        let mut left = self.parse_complement_expression()?;

        // Peek past optional whitespace to check for '&'. If found, consume
        // whitespace + operator. If not found, leave whitespace in place.
        while self.peek_non_ws() == Some('&') {
            self.skip_whitespace();
            self.advance(); // consume '&'
            self.skip_whitespace();
            let right = self.parse_complement_expression()?;
            left = PathExpression::make_op(PathExpressionOp::Intersection, left, right);
        }

        Ok(left)
    }

    /// Parses complement expressions (~).
    fn parse_complement_expression(&mut self) -> Result<PathExpression, String> {
        self.skip_whitespace();

        let mut count = 0;
        while let Some('~') = self.current {
            count += 1;
            self.advance();
            self.skip_whitespace();
        }

        let mut expr = self.parse_atom()?;

        // Apply complement operators (right-associative)
        for _ in 0..count {
            expr = PathExpression::make_complement(expr);
        }

        Ok(expr)
    }

    /// Parses an atom (pattern, reference, or parenthesized expression).
    fn parse_atom(&mut self) -> Result<PathExpression, String> {
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
        // Check for expression reference (%path:name or %_)
        else if let Some('%') = self.current {
            self.parse_reference()
        }
        // Otherwise parse as a path pattern
        else {
            self.parse_pattern()
        }
    }

    /// Parses an expression reference (%path:name or %_).
    fn parse_reference(&mut self) -> Result<PathExpression, String> {
        if let Some('%') = self.current {
            self.advance();
        } else {
            return Err("Expected '%' for expression reference".to_string());
        }

        // Check for weaker reference (%_)
        if let Some('_') = self.current {
            self.advance();
            return Ok(PathExpression::weaker_ref());
        }

        // Parse path:name reference
        // Parse path string, but stop at ':' (don't include it)
        let path_str = self.parse_path_string_for_ref()?;

        if let Some(':') = self.current {
            self.advance();
            let name = self.parse_identifier()?;

            let path = Path::from_string(&path_str)
                .ok_or_else(|| format!("Invalid path in expression reference: {}", path_str))?;

            let ref_expr = ExpressionReference::new(path, name);
            Ok(PathExpression::make_atom_ref(ref_expr))
        } else {
            Err("Expected ':' after path in expression reference".to_string())
        }
    }

    /// Parses a path string for expression references (stops at ':').
    fn parse_path_string_for_ref(&mut self) -> Result<String, String> {
        let mut result = String::new();

        while let Some(ch) = self.current {
            // Stop at ':' for expression references
            if ch == ':' {
                break;
            }

            // Stop at operators or whitespace
            if is_operator_char(ch) || ch.is_whitespace() {
                break;
            }

            result.push(ch);
            self.advance();
        }

        if result.is_empty() {
            Err("Expected path in expression reference".to_string())
        } else {
            Ok(result)
        }
    }

    /// Parses a path pattern.
    fn parse_pattern(&mut self) -> Result<PathExpression, String> {
        let pattern_str = self.parse_path_string()?;

        // Try to parse as a simple path first
        if let Some(path) = Path::from_string(&pattern_str) {
            Ok(PathExpression::make_atom_path(path))
        } else {
            // Try parsing as a pattern with wildcards
            let pattern = parse_path_pattern(&pattern_str)?;
            Ok(PathExpression::make_atom_pattern(pattern))
        }
    }

    /// Parses a path string (may contain wildcards, predicates, etc.).
    fn parse_path_string(&mut self) -> Result<String, String> {
        let mut result = String::new();
        let mut in_predicate = false;
        let mut predicate_depth = 0;

        while let Some(ch) = self.current {
            // Handle predicate expressions { ... }
            if ch == '{' {
                in_predicate = true;
                predicate_depth = 1;
                result.push(ch);
                self.advance();
                continue;
            }

            if in_predicate {
                if ch == '{' {
                    predicate_depth += 1;
                } else if ch == '}' {
                    predicate_depth -= 1;
                    if predicate_depth == 0 {
                        in_predicate = false;
                    }
                }
                result.push(ch);
                self.advance();
                continue;
            }

            // Stop at operators or whitespace (unless in predicate)
            if is_operator_char(ch) || (ch.is_whitespace() && !in_predicate) {
                break;
            }

            // Stop at closing paren (unless in predicate)
            if ch == ')' && !in_predicate {
                break;
            }

            result.push(ch);
            self.advance();
        }

        if result.is_empty() {
            Err("Expected path pattern".to_string())
        } else {
            Ok(result)
        }
    }

    /// Parses an identifier (for expression reference names).
    fn parse_identifier(&mut self) -> Result<String, String> {
        let mut result = String::new();

        while let Some(ch) = self.current {
            if ch.is_alphanumeric() || ch == '_' {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if result.is_empty() {
            Err("Expected identifier".to_string())
        } else {
            Ok(result)
        }
    }
}

/// Checks if a character is an operator.
fn is_operator_char(ch: char) -> bool {
    matches!(ch, '+' | '-' | '&' | '~' | '(' | ')')
}

/// Parses a path pattern from a string.
fn parse_path_pattern(pattern_str: &str) -> Result<PathPattern, String> {
    // Try to parse as a simple path first
    if let Some(path) = Path::from_string(pattern_str) {
        return Ok(PathPattern::from_path(path));
    }

    // Parse pattern with wildcards
    // Split by '/' to get components
    let parts: Vec<&str> = pattern_str.split('/').collect();

    if parts.is_empty() {
        return Err("Empty pattern".to_string());
    }

    // Determine if absolute
    let is_absolute = pattern_str.starts_with('/');
    let start_idx = if is_absolute { 1 } else { 0 };

    // Build prefix (non-wildcard part)
    let mut prefix_parts = Vec::new();
    let mut components = Vec::new();
    let mut in_wildcard = false;

    for (i, part) in parts.iter().enumerate().skip(start_idx) {
        if part.is_empty() {
            // An empty part at any position > start_idx means we hit a `//`
            // separator, which represents a recursive wildcard in USD path
            // expressions. E.g. "/World//*" → prefix=/World, components=[//, *].
            // We avoid pushing duplicates if consecutive empty parts appear.
            if i >= start_idx {
                in_wildcard = true;
                if components.last() != Some(&PatternComponent::RecursiveWildcard) {
                    components.push(PatternComponent::RecursiveWildcard);
                }
            }
            continue;
        }

        // Check for wildcards
        if part.contains('*') || part.contains('?') || part.contains('{') {
            in_wildcard = true;
            if part == &"*" {
                components.push(PatternComponent::Wildcard);
            } else if part.contains('*') || part.contains('?') {
                components.push(PatternComponent::Glob(part.to_string()));
            } else {
                // Has predicate
                components.push(PatternComponent::Predicate(part.to_string()));
            }
        } else if !in_wildcard {
            prefix_parts.push(*part);
        } else {
            components.push(PatternComponent::Literal(part.to_string()));
        }
    }

    // Build prefix path
    let prefix_str = if is_absolute {
        "/".to_string() + &prefix_parts.join("/")
    } else {
        prefix_parts.join("/")
    };

    let prefix = if prefix_str.is_empty() {
        if is_absolute {
            Path::absolute_root()
        } else {
            Path::reflexive_relative()
        }
    } else {
        Path::from_string(&prefix_str)
            .ok_or_else(|| format!("Invalid prefix path: {}", prefix_str))?
    };

    Ok(PathPattern { prefix, components })
}

/// Parses a path expression from a string.
fn parse_expression(expr: &str) -> Result<PathExpression, String> {
    let mut parser = Parser::new(expr);
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
