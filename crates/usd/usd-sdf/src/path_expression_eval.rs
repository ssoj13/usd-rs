//! SdfPathExpressionEval - evaluator for path expressions.
//!
//! Port of pxr/usd/sdf/pathExpressionEval.h
//!
//! Objects of this class evaluate complete SdfPathExpressions. Supports
//! incremental depth-first search over domain objects.

use crate::{Path, PathExpression, PredicateExpression};
use regex::Regex;
use std::marker::PhantomData;

/// Result of predicate function evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredicateFunctionResult {
    /// Whether the predicate matched.
    pub value: bool,
    /// Whether the result is constant (won't change for descendants).
    pub is_constant: bool,
}

impl PredicateFunctionResult {
    /// Creates a constant result.
    pub fn make_constant(value: bool) -> Self {
        Self {
            value,
            is_constant: true,
        }
    }

    /// Creates a varying result.
    pub fn make_varying(value: bool) -> Self {
        Self {
            value,
            is_constant: false,
        }
    }

    /// Returns true if the result is truthy.
    pub fn is_truthy(&self) -> bool {
        self.value
    }
}

impl Default for PredicateFunctionResult {
    fn default() -> Self {
        Self::make_constant(false)
    }
}

/// Component type in a pattern.
#[derive(Debug, Clone)]
enum ComponentType {
    /// An explicit name (not a glob pattern).
    ExplicitName(String),
    /// A glob pattern (handled via regex).
    Regex(Regex),
}

/// A component in a path pattern.
#[derive(Debug, Clone)]
struct PatternComponent {
    /// The component type.
    component_type: ComponentType,
    /// Predicate index, -1 if none.
    predicate_index: i32,
}

impl PatternComponent {
    /// Returns the predicate index for this component.
    /// Returns -1 if no predicate is associated.
    fn predicate_index(&self) -> i32 {
        self.predicate_index
    }
}

/// A segment in a path pattern (half-open interval in components).
#[derive(Debug, Clone, Default)]
struct PatternSegment {
    /// Start index (inclusive).
    begin: usize,
    /// End index (exclusive).
    end: usize,
}

impl PatternSegment {
    #[allow(dead_code)] // C++ parity - used for pattern matching
    fn is_empty(&self) -> bool {
        self.begin == self.end
    }

    #[allow(dead_code)] // C++ parity - used for pattern matching
    fn size(&self) -> usize {
        self.end - self.begin
    }
}

/// Match object type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchObjType {
    /// Match prims or properties.
    PrimOrProp,
    /// Match prims only.
    PrimOnly,
    /// Match properties only.
    PropOnly,
}

/// Internal pattern implementation.
#[derive(Clone)]
struct PatternImpl<D: 'static> {
    /// Prefix path.
    prefix: Path,
    /// Pattern components.
    components: Vec<PatternComponent>,
    /// Segments.
    segments: Vec<PatternSegment>,
    /// Whether pattern stretches at beginning.
    stretch_begin: bool,
    /// Whether pattern stretches at end.
    stretch_end: bool,
    /// What object types this pattern matches.
    match_obj_type: MatchObjType,
    /// Predicate expressions (unlinked).
    predicates: Vec<PredicateExpression>,
    /// Bound predicate programs (linked from PredicateLibrary).
    /// Each program handles compound predicates (And/Or/Not).
    bound_predicates: Vec<Option<crate::predicate_program::PredicateProgram<D>>>,
    /// Phantom for domain type.
    _phantom: PhantomData<D>,
}

impl<D: 'static> std::fmt::Debug for PatternImpl<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatternImpl")
            .field("prefix", &self.prefix)
            .field("components", &self.components)
            .field("stretch_begin", &self.stretch_begin)
            .field("stretch_end", &self.stretch_end)
            .field("match_obj_type", &self.match_obj_type)
            .field("predicates", &self.predicates)
            .field(
                "bound_predicates_count",
                &self.bound_predicates.iter().filter(|p| p.is_some()).count(),
            )
            .finish()
    }
}

impl<D: 'static> Default for PatternImpl<D> {
    fn default() -> Self {
        Self {
            prefix: Path::empty(),
            components: Vec::new(),
            segments: Vec::new(),
            stretch_begin: false,
            stretch_end: false,
            match_obj_type: MatchObjType::PrimOrProp,
            predicates: Vec::new(),
            bound_predicates: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<D: 'static> PatternImpl<D> {
    /// Sets the match object type for this pattern.
    pub fn set_match_obj_type(&mut self, match_type: MatchObjType) {
        self.match_obj_type = match_type;
    }

    /// Returns the match object type for this pattern.
    pub fn get_match_obj_type(&self) -> MatchObjType {
        self.match_obj_type
    }

    /// Returns the predicates for this pattern.
    pub fn get_predicates(&self) -> &[PredicateExpression] {
        &self.predicates
    }

    /// Sets bound predicate programs from a PredicateLibrary.
    pub fn set_bound_predicates(
        &mut self,
        bound: Vec<Option<crate::predicate_program::PredicateProgram<D>>>,
    ) {
        self.bound_predicates = bound;
    }

    /// Checks if a path matches the object type constraint.
    fn matches_obj_type(&self, path: &Path) -> bool {
        match self.get_match_obj_type() {
            MatchObjType::PrimOrProp => true,
            MatchObjType::PrimOnly => !path.is_property_path(),
            MatchObjType::PropOnly => path.is_property_path(),
        }
    }

    /// Evaluates a predicate on the given object.
    ///
    /// If bound predicate functions are available (linked via PredicateLibrary),
    /// the bound function is called. Otherwise, returns true (unlinked predicate
    /// is treated as a pass-through).
    fn evaluate_predicate<F>(
        &self,
        predicate_idx: i32,
        path: &Path,
        path_to_obj: &F,
    ) -> PredicateFunctionResult
    where
        F: Fn(&Path) -> D,
    {
        if predicate_idx < 0 {
            return PredicateFunctionResult::make_constant(true);
        }

        let idx = predicate_idx as usize;

        // Try bound predicate programs first
        if idx < self.bound_predicates.len() {
            if let Some(ref program) = self.bound_predicates[idx] {
                let obj = path_to_obj(path);
                let lib_result = program.evaluate(&obj);
                return PredicateFunctionResult {
                    value: lib_result.value,
                    is_constant: lib_result.constancy
                        == crate::predicate_library::Constancy::ConstantOverDescendants,
                };
            }
        }

        // No bound function available - treat as pass-through
        PredicateFunctionResult::make_varying(true)
    }

    /// Creates a new pattern impl from a path pattern.
    pub fn new(pattern: &crate::PathPattern) -> Self {
        let mut impl_ = Self::default();
        impl_.prefix = pattern.prefix().clone();
        impl_.stretch_begin = pattern.has_leading_stretch();
        impl_.stretch_end = pattern.has_trailing_stretch();

        // Determine object type from pattern characteristics
        // If pattern ends with a property name (contains '.'), match properties only
        // This is a heuristic - full implementation would parse pattern syntax
        if pattern.prefix().is_property_path() {
            impl_.match_obj_type = MatchObjType::PropOnly;
        } else if !pattern.components().is_empty() {
            // Check if any component suggests property matching
            for comp in pattern.components() {
                if let crate::PatternComponent::Literal(name) = comp {
                    if name.contains('.') {
                        impl_.match_obj_type = MatchObjType::PropOnly;
                        break;
                    }
                }
            }
        }

        // Build components from pattern
        // Note: regex compilation in loop is acceptable here as patterns are typically small
        #[allow(clippy::regex_creation_in_loops)]
        for comp in pattern.components() {
            let component = match comp {
                crate::PatternComponent::Literal(s) => PatternComponent {
                    component_type: ComponentType::ExplicitName(s.clone()),
                    predicate_index: -1,
                },
                crate::PatternComponent::Wildcard => PatternComponent {
                    component_type: ComponentType::Regex(
                        Regex::new(r"^[^/]+$").expect("valid regex"),
                    ),
                    predicate_index: -1,
                },
                crate::PatternComponent::RecursiveWildcard => PatternComponent {
                    component_type: ComponentType::Regex(Regex::new(r".*").expect("valid regex")),
                    predicate_index: -1,
                },
                crate::PatternComponent::Glob(g) => {
                    // Convert glob to regex
                    let regex_str = glob_to_regex(g);
                    PatternComponent {
                        component_type: ComponentType::Regex(
                            Regex::new(&regex_str)
                                .unwrap_or_else(|_| Regex::new(".*").expect("valid regex")),
                        ),
                        predicate_index: -1,
                    }
                }
                crate::PatternComponent::Predicate(pred_text) => {
                    // Strip surrounding braces if present, then parse as
                    // PredicateExpression so link_predicates() can bind it.
                    let inner = pred_text
                        .strip_prefix('{')
                        .and_then(|s| s.strip_suffix('}'))
                        .unwrap_or(pred_text);
                    let pred_expr = crate::PredicateExpression::parse(inner);
                    let pred_idx = impl_.predicates.len() as i32;
                    impl_.predicates.push(pred_expr);
                    // A predicate component matches any single child name
                    // (like `*`), then the predicate further filters.
                    PatternComponent {
                        component_type: ComponentType::Regex(
                            Regex::new(r"^[^/]+$").expect("valid regex"),
                        ),
                        predicate_index: pred_idx,
                    }
                }
            };
            impl_.components.push(component);
        }

        // Build segments (for now, one segment covering all components)
        if !impl_.components.is_empty() {
            impl_.segments.push(PatternSegment {
                begin: 0,
                end: impl_.components.len(),
            });
        }

        impl_
    }

    /// Matches a path against this pattern.
    pub fn match_path<F>(&self, path: &Path, path_to_obj: F) -> PredicateFunctionResult
    where
        F: Fn(&Path) -> D,
    {
        // Check object type constraint first.
        if !self.matches_obj_type(path) {
            return PredicateFunctionResult::make_constant(false);
        }

        let prefix_str = self.prefix.get_string();
        let path_str = path.get_string();

        // The path must start with the prefix (or equal it for exact matches).
        // For root prefix "/" every absolute path qualifies.
        let suffix_start = if prefix_str == "/" {
            // Root prefix: suffix starts at position 1 (skip the leading '/').
            if !path_str.starts_with('/') {
                return PredicateFunctionResult::make_constant(false);
            }
            1usize
        } else if prefix_str.is_empty() {
            0
        } else {
            // Non-root prefix: path must equal or start with "prefix/...".
            if path_str == prefix_str {
                prefix_str.len()
            } else if path_str.starts_with(prefix_str)
                && path_str.as_bytes().get(prefix_str.len()) == Some(&b'/')
            {
                prefix_str.len() + 1 // skip the '/' separator
            } else {
                return PredicateFunctionResult::make_constant(false);
            }
        };

        // Build the suffix component list from the path.
        let suffix_str = &path_str[suffix_start..];
        let suffix_parts: Vec<&str> = if suffix_str.is_empty() {
            Vec::new()
        } else {
            suffix_str.split('/').filter(|s| !s.is_empty()).collect()
        };

        // No components → exact path match (prefix must equal the full path).
        if self.components.is_empty() {
            return if suffix_parts.is_empty() {
                PredicateFunctionResult::make_varying(true)
            } else {
                PredicateFunctionResult::make_constant(false)
            };
        }

        // Recursive component-vs-suffix matching.
        // Returns true if components[comp_idx..] can consume suffix_parts[part_idx..].
        fn match_components<D: 'static, F>(
            components: &[PatternComponent],
            comp_idx: usize,
            parts: &[&str],
            part_idx: usize,
            path: &Path,
            path_to_obj: &F,
            pattern: &PatternImpl<D>,
        ) -> bool
        where
            F: Fn(&Path) -> D,
        {
            if comp_idx >= components.len() {
                // All components consumed — only matches if all parts were consumed too.
                return part_idx >= parts.len();
            }

            let comp = &components[comp_idx];

            match &comp.component_type {
                ComponentType::Regex(regex) if regex.as_str() == ".*" => {
                    // RecursiveWildcard (//): matches zero or more path components.
                    // Try consuming 0, 1, 2, … parts.
                    for skip in 0..=(parts.len() - part_idx) {
                        if match_components(
                            components,
                            comp_idx + 1,
                            parts,
                            part_idx + skip,
                            path,
                            path_to_obj,
                            pattern,
                        ) {
                            return true;
                        }
                    }
                    false
                }
                _ => {
                    // Regular component: must consume exactly one part.
                    if part_idx >= parts.len() {
                        return false;
                    }

                    let part_matches = match &comp.component_type {
                        ComponentType::ExplicitName(name) => {
                            name.is_empty() || parts[part_idx] == name.as_str()
                        }
                        ComponentType::Regex(regex) => regex.is_match(parts[part_idx]),
                    };

                    if !part_matches {
                        return false;
                    }

                    // Evaluate bound predicate if present.
                    if comp.predicate_index() >= 0 {
                        let pred =
                            pattern.evaluate_predicate(comp.predicate_index(), path, path_to_obj);
                        if !pred.value {
                            return false;
                        }
                    }

                    match_components(
                        components,
                        comp_idx + 1,
                        parts,
                        part_idx + 1,
                        path,
                        path_to_obj,
                        pattern,
                    )
                }
            }
        }

        let start_part = if self.stretch_begin {
            // stretch_begin means the pattern can match at any suffix depth.
            // Try anchoring the non-stretch component sequence at each position.
            // Find the first non-recursive component range (skip leading //).
            // For now treat stretch_begin as "skip zero or more leading parts".
            0
        } else {
            0
        };

        let matched = if self.stretch_begin {
            // Try anchoring from every possible starting position.
            let mut found = false;
            for start in 0..=suffix_parts.len() {
                if match_components(
                    &self.components,
                    0,
                    &suffix_parts,
                    start,
                    path,
                    &path_to_obj,
                    self,
                ) {
                    found = true;
                    break;
                }
            }
            found
        } else {
            let _ = start_part; // suppress unused warning
            match_components(
                &self.components,
                0,
                &suffix_parts,
                0,
                path,
                &path_to_obj,
                self,
            )
        };

        if matched {
            PredicateFunctionResult::make_varying(true)
        } else {
            PredicateFunctionResult::make_varying(false)
        }
    }
}

/// Converts a glob pattern to a regex string.
fn glob_to_regex(glob: &str) -> String {
    let mut regex = String::with_capacity(glob.len() * 2);
    regex.push('^');
    for c in glob.chars() {
        match c {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(c);
            }
            _ => regex.push(c),
        }
    }
    regex.push('$');
    regex
}

/// Operation type in expression evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalOp {
    /// Evaluate a pattern.
    EvalPattern,
    /// Logical NOT.
    Not,
    /// Open parenthesis.
    Open,
    /// Close parenthesis.
    Close,
    /// Logical OR.
    Or,
    /// Logical AND.
    And,
}

/// State for incremental pattern search.
#[derive(Debug, Clone, Default)]
pub struct PatternIncrSearchState {
    /// Segment match depths.
    segment_match_depths: Vec<i32>,
    /// Constant depth.
    constant_depth: i32,
    /// Constant value.
    constant_value: bool,
}

impl PatternIncrSearchState {
    /// Pops state to a new depth.
    pub fn pop(&mut self, new_depth: i32) {
        while let Some(&depth) = self.segment_match_depths.last() {
            if depth >= new_depth {
                self.segment_match_depths.pop();
            } else {
                break;
            }
        }
        if self.constant_depth >= new_depth {
            self.constant_depth = -1;
            self.constant_value = false;
        }
    }
}

/// Evaluator for path expressions.
///
/// Objects of this class evaluate complete SdfPathExpressions.
#[derive(Debug, Clone)]
pub struct PathExpressionEval<D: 'static> {
    /// Pattern implementations.
    pattern_impls: Vec<PatternImpl<D>>,
    /// Operations for expression evaluation.
    ops: Vec<EvalOp>,
}

impl<D: 'static> Default for PathExpressionEval<D> {
    fn default() -> Self {
        Self {
            pattern_impls: Vec::new(),
            ops: Vec::new(),
        }
    }
}

impl<D: 'static> PathExpressionEval<D> {
    /// Creates an empty evaluator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this is the empty evaluator.
    pub fn is_empty(&self) -> bool {
        self.pattern_impls.is_empty()
    }

    /// Creates an evaluator from a path expression.
    pub fn from_expression(expr: &PathExpression) -> Self {
        Self::from_expression_with_match_type(expr, MatchObjType::PrimOrProp)
    }

    /// Creates an evaluator that matches only prims.
    pub fn from_expression_prims_only(expr: &PathExpression) -> Self {
        Self::from_expression_with_match_type(expr, MatchObjType::PrimOnly)
    }

    /// Creates an evaluator that matches only properties.
    pub fn from_expression_props_only(expr: &PathExpression) -> Self {
        Self::from_expression_with_match_type(expr, MatchObjType::PropOnly)
    }

    /// Creates an evaluator from a path expression with a specific match type.
    pub fn from_expression_with_match_type(
        expr: &PathExpression,
        default_match_type: MatchObjType,
    ) -> Self {
        let mut eval = Self::new();

        if expr.is_empty() {
            return eval;
        }

        // Build pattern implementations from expression
        for pattern in expr.get_patterns() {
            let mut pattern_impl = PatternImpl::new(pattern);
            // If pattern didn't auto-detect a specific type, use the default
            if pattern_impl.get_match_obj_type() == MatchObjType::PrimOrProp
                && default_match_type != MatchObjType::PrimOrProp
            {
                pattern_impl.set_match_obj_type(default_match_type);
            }
            eval.pattern_impls.push(pattern_impl);
            eval.ops.push(EvalOp::EvalPattern);
        }

        // Add operators based on expression structure
        // Use Open/Close for grouping when we have complex expressions
        match expr.op() {
            Some(crate::PathExpressionOp::Union) => {
                if expr.get_patterns().len() > 2 {
                    eval.ops.insert(0, EvalOp::Open);
                }
                eval.ops.push(EvalOp::Or);
                if expr.get_patterns().len() > 2 {
                    eval.ops.push(EvalOp::Close);
                }
            }
            Some(crate::PathExpressionOp::Intersection) => {
                if expr.get_patterns().len() > 2 {
                    eval.ops.insert(0, EvalOp::Open);
                }
                eval.ops.push(EvalOp::And);
                if expr.get_patterns().len() > 2 {
                    eval.ops.push(EvalOp::Close);
                }
            }
            Some(crate::PathExpressionOp::Difference) => {
                eval.ops.insert(0, EvalOp::Open);
                eval.ops.push(EvalOp::Not);
                eval.ops.push(EvalOp::And);
                eval.ops.push(EvalOp::Close);
            }
            Some(crate::PathExpressionOp::Complement) => eval.ops.push(EvalOp::Not),
            // These operations are not returned by op() but included for exhaustiveness
            Some(crate::PathExpressionOp::ImpliedUnion)
            | Some(crate::PathExpressionOp::ExpressionRef)
            | Some(crate::PathExpressionOp::Pattern)
            | None => {}
        }

        eval
    }

    /// Links predicate functions from a PredicateLibrary.
    ///
    /// This binds predicate expressions in the patterns to actual
    /// predicate functions from the library, enabling predicate evaluation
    /// during matching.
    pub fn link_predicates(&mut self, lib: &crate::predicate_library::PredicateLibrary<D>)
    where
        D: Send + Sync,
    {
        for pattern_impl in &mut self.pattern_impls {
            let mut bound = Vec::new();
            for pred_expr in pattern_impl.get_predicates() {
                let program = crate::predicate_program::link(pred_expr, lib);
                if program.is_valid() {
                    bound.push(Some(program));
                } else {
                    bound.push(None);
                }
            }
            pattern_impl.set_bound_predicates(bound);
        }
    }

    /// Tests a path for a match with this expression.
    pub fn match_path<F>(&self, path: &Path, path_to_obj: F) -> PredicateFunctionResult
    where
        F: Fn(&Path) -> D + Clone,
    {
        if self.is_empty() {
            return PredicateFunctionResult::make_constant(false);
        }

        // Simple evaluation: check all patterns
        let mut result = PredicateFunctionResult::make_constant(false);
        let mut op_stack: Vec<EvalOp> = Vec::new();
        let mut value_stack: Vec<bool> = Vec::new();

        for (idx, op) in self.ops.iter().enumerate() {
            match op {
                EvalOp::EvalPattern => {
                    if idx < self.pattern_impls.len() {
                        let pattern_result =
                            self.pattern_impls[idx].match_path(path, path_to_obj.clone());
                        value_stack.push(pattern_result.value);
                        result = pattern_result;
                    }
                }
                EvalOp::Not => {
                    if let Some(val) = value_stack.pop() {
                        value_stack.push(!val);
                    }
                }
                EvalOp::And => {
                    if value_stack.len() >= 2 {
                        let b = value_stack.pop().expect("stack has items");
                        let a = value_stack.pop().expect("stack has items");
                        value_stack.push(a && b);
                    }
                }
                EvalOp::Or => {
                    if value_stack.len() >= 2 {
                        let b = value_stack.pop().expect("stack has items");
                        let a = value_stack.pop().expect("stack has items");
                        value_stack.push(a || b);
                    }
                }
                EvalOp::Open => op_stack.push(*op),
                EvalOp::Close => {
                    op_stack.pop();
                }
            }
        }

        if let Some(&final_val) = value_stack.last() {
            result.value = final_val;
        }

        result
    }

    /// Creates an incremental searcher.
    ///
    /// Note: Requires F: Clone for the searcher to work properly.
    pub fn make_incremental_searcher<F>(&self, path_to_obj: F) -> IncrementalSearcher<'_, D, F>
    where
        F: Fn(&Path) -> D + Clone,
    {
        IncrementalSearcher::new(self, path_to_obj)
    }
}

/// Incremental searcher for depth-first traversal.
#[derive(Debug)]
pub struct IncrementalSearcher<'a, D: 'static, F>
where
    F: Fn(&Path) -> D,
{
    /// Reference to evaluator.
    eval: &'a PathExpressionEval<D>,
    /// Search states per pattern.
    states: Vec<PatternIncrSearchState>,
    /// Path to object mapper.
    path_to_obj: F,
    /// Last path depth.
    last_depth: i32,
}

impl<'a, D: 'static, F> IncrementalSearcher<'a, D, F>
where
    F: Fn(&Path) -> D + Clone,
{
    /// Creates a new incremental searcher.
    pub fn new(eval: &'a PathExpressionEval<D>, path_to_obj: F) -> Self {
        Self {
            eval,
            states: vec![PatternIncrSearchState::default(); eval.pattern_impls.len()],
            path_to_obj,
            last_depth: 0,
        }
    }

    /// Advances the search to the next path.
    pub fn next(&mut self, path: &Path) -> PredicateFunctionResult {
        let new_depth = path.get_path_element_count() as i32;
        let pop = new_depth <= self.last_depth;

        if pop {
            for state in &mut self.states {
                state.pop(new_depth);
            }
        }

        self.last_depth = new_depth;
        self.eval.match_path(path, self.path_to_obj.clone())
    }

    /// Resets the searcher for a new search.
    pub fn reset(&mut self) {
        self.states = vec![PatternIncrSearchState::default(); self.eval.pattern_impls.len()];
        self.last_depth = 0;
    }
}

/// Creates a path expression evaluator.
pub fn make_path_expression_eval<D: 'static>(expr: &PathExpression) -> PathExpressionEval<D> {
    PathExpressionEval::from_expression(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_eval() {
        let eval: PathExpressionEval<()> = PathExpressionEval::new();
        assert!(eval.is_empty());
    }

    #[test]
    fn test_predicate_result() {
        let r1 = PredicateFunctionResult::make_constant(true);
        assert!(r1.is_truthy());
        assert!(r1.is_constant);

        let r2 = PredicateFunctionResult::make_varying(false);
        assert!(!r2.is_truthy());
        assert!(!r2.is_constant);
    }

    #[test]
    fn test_glob_to_regex() {
        assert_eq!(glob_to_regex("foo"), "^foo$");
        assert_eq!(glob_to_regex("foo*"), "^foo.*$");
        assert_eq!(glob_to_regex("*.txt"), "^.*\\.txt$");
        assert_eq!(glob_to_regex("f?o"), "^f.o$");
    }
}
