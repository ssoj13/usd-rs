//! UsdCollectionMembershipQuery - query object for collection membership.
//!
//! Port of pxr/usd/usd/collectionMembershipQuery.h/cpp
//!
//! Represents a flattened view of a collection. A CollectionMembershipQuery
//! object can be used to answer queries about membership of paths in the
//! collection efficiently.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use usd_sdf::Path;
use usd_tf::Token;

/// Re-export of HashSet<Path> as SdfPathSet for compatibility with C++ API.
pub type SdfPathSet = HashSet<Path>;

// ============================================================================
// CollectionMembershipQueryTokens
// ============================================================================

/// Tokens for collection membership query.
///
/// Provides static token values used in collection membership queries.
/// Matches C++ `UsdCollectionMembershipQueryTokens`.
pub struct CollectionMembershipQueryTokens;

impl CollectionMembershipQueryTokens {
    /// Returns the token for "IncludedByMembershipExpression".
    ///
    /// This token is used to indicate that a path is included by a membership expression.
    pub fn included_by_membership_expression() -> Token {
        Token::new("IncludedByMembershipExpression")
    }

    /// Returns the token for "ExcludedByMembershipExpression".
    ///
    /// This token is used to indicate that a path is excluded by a membership expression.
    pub fn excluded_by_membership_expression() -> Token {
        Token::new("ExcludedByMembershipExpression")
    }
}

// ============================================================================
// PathExpansionRuleMap
// ============================================================================

/// Map describing membership of paths in a collection and associated expansion rules.
///
/// Matches C++ `Usd_CollectionMembershipQueryBase::PathExpansionRuleMap`.
pub type PathExpansionRuleMap = HashMap<Path, Token>;

// ============================================================================
// CollectionMembershipQueryBase
// ============================================================================

/// Base class for collection membership queries.
///
/// Matches C++ `Usd_CollectionMembershipQueryBase`.
#[derive(Debug, Clone)]
pub struct CollectionMembershipQueryBase {
    /// Top-level expansion rule.
    top_expansion_rule: Token,
    /// Map of paths to expansion rules.
    path_expansion_rule_map: PathExpansionRuleMap,
    /// Set of included collection paths.
    included_collections: SdfPathSet,
    /// Cached flag indicating whether path_expansion_rule_map contains any exclude rules.
    has_excludes: bool,
}

impl CollectionMembershipQueryBase {
    /// Default constructor, creates an empty query object.
    pub fn new() -> Self {
        Self {
            top_expansion_rule: Token::new("expandPrims"),
            path_expansion_rule_map: HashMap::new(),
            included_collections: HashSet::new(),
            has_excludes: false,
        }
    }

    /// Constructor that takes a path expansion rule map.
    pub fn new_with_map(
        path_expansion_rule_map: PathExpansionRuleMap,
        included_collections: SdfPathSet,
    ) -> Self {
        let has_excludes = path_expansion_rule_map
            .values()
            .any(|rule| rule.get_text() == "exclude");

        Self {
            top_expansion_rule: Token::new("expandPrims"),
            path_expansion_rule_map,
            included_collections,
            has_excludes,
        }
    }

    /// Constructor that takes a path expansion rule map and top-level expansion rule.
    pub fn new_with_map_and_rule(
        path_expansion_rule_map: PathExpansionRuleMap,
        included_collections: SdfPathSet,
        top_expansion_rule: Token,
    ) -> Self {
        let has_excludes = path_expansion_rule_map
            .values()
            .any(|rule| rule.get_text() == "exclude");

        Self {
            top_expansion_rule,
            path_expansion_rule_map,
            included_collections,
            has_excludes,
        }
    }

    /// Returns true if the collection excludes one or more paths.
    ///
    /// Matches C++ `HasExcludes()`.
    pub fn has_excludes(&self) -> bool {
        self.has_excludes
    }

    /// Returns a raw map of the paths included or excluded in the collection.
    ///
    /// Matches C++ `GetAsPathExpansionRuleMap()`.
    pub fn get_as_path_expansion_rule_map(&self) -> &PathExpansionRuleMap {
        &self.path_expansion_rule_map
    }

    /// Returns a set of paths for all collections that were included.
    ///
    /// Matches C++ `GetIncludedCollections()`.
    pub fn get_included_collections(&self) -> &SdfPathSet {
        &self.included_collections
    }

    /// Return the top expansion rule for this query object.
    ///
    /// Matches C++ `GetTopExpansionRule()`.
    pub fn get_top_expansion_rule(&self) -> &Token {
        &self.top_expansion_rule
    }

    /// Sets the top expansion rule.
    ///
    /// # Arguments
    ///
    /// * `rule` - The expansion rule token to set (e.g., "expandPrims", "explicitOnly")
    pub fn set_top_expansion_rule(&mut self, rule: Token) {
        self.top_expansion_rule = rule;
    }

    /// Returns true if the path expansion rule map is empty.
    ///
    /// Matches C++ `_HasEmptyRuleMap()`.
    pub fn has_empty_rule_map(&self) -> bool {
        self.path_expansion_rule_map.is_empty()
    }

    /// Returns whether the given path is included by the rule map.
    ///
    /// Matches C++ `_IsPathIncludedByRuleMap(const SdfPath &path, TfToken *expansionRule)`.
    pub fn is_path_included_by_rule_map(
        &self,
        path: &Path,
        expansion_rule: &mut Option<Token>,
    ) -> bool {
        // Check exact match first
        if let Some(rule) = self.path_expansion_rule_map.get(path) {
            if let Some(rule_out) = expansion_rule {
                *rule_out = rule.clone();
            }
            return rule.get_text() != "exclude";
        }

        // Check parent paths (walk up to and including absolute root)
        let mut current = path.clone();
        loop {
            let parent = current.get_parent_path();
            if parent == current || parent.is_empty() {
                break;
            }
            current = parent;
            if let Some(rule) = self.path_expansion_rule_map.get(&current) {
                let rule_text = rule.get_text();
                if rule_text == "exclude" {
                    if let Some(rule_out) = expansion_rule {
                        *rule_out = rule.clone();
                    }
                    return false;
                } else if rule_text == "explicitOnly" {
                    if let Some(rule_out) = expansion_rule {
                        *rule_out = rule.clone();
                    }
                    return false;
                } else if rule_text == "expandPrims" || rule_text == "expandPrimsAndProperties" {
                    if let Some(rule_out) = expansion_rule {
                        *rule_out = rule.clone();
                    }
                    return true;
                }
            }
        }

        if let Some(rule_out) = expansion_rule {
            *rule_out = Token::new("exclude");
        }
        false
    }

    /// Returns whether the given path is included by the rule map with parent expansion rule.
    ///
    /// Matches C++ `_IsPathIncludedByRuleMap(const SdfPath &path, const TfToken &parentExpansionRule, TfToken *expansionRule)`.
    pub fn is_path_included_by_rule_map_with_parent(
        &self,
        path: &Path,
        parent_expansion_rule: &Token,
        expansion_rule: &mut Option<Token>,
    ) -> bool {
        // Check exact match first
        if let Some(rule) = self.path_expansion_rule_map.get(path) {
            if let Some(rule_out) = expansion_rule {
                *rule_out = rule.clone();
            }
            return rule.get_text() != "exclude";
        }

        // Use parent expansion rule
        let parent_rule_text = parent_expansion_rule.get_text();
        if parent_rule_text == "exclude" {
            if let Some(rule_out) = expansion_rule {
                *rule_out = parent_expansion_rule.clone();
            }
            return false;
        } else if parent_rule_text == "explicitOnly" {
            if let Some(rule_out) = expansion_rule {
                *rule_out = parent_expansion_rule.clone();
            }
            return false;
        } else if parent_rule_text == "expandPrims"
            || parent_rule_text == "expandPrimsAndProperties"
        {
            if let Some(rule_out) = expansion_rule {
                *rule_out = parent_expansion_rule.clone();
            }
            return true;
        }

        if let Some(rule_out) = expansion_rule {
            *rule_out = Token::new("exclude");
        }
        false
    }
}

impl Default for CollectionMembershipQueryBase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ObjectCollectionExpressionEvaluator
// ============================================================================

/// Evaluates SdfPathExpressions with objects from a given UsdStage.
///
/// Matches C++ `UsdObjectCollectionExpressionEvaluator`.
///
/// This evaluator can be used to efficiently test whether paths or objects
/// match a given path expression. It is typically created by
/// `CollectionAPI::compute_membership_query()` for use with
/// `CollectionMembershipQuery`.
///
/// # Example
///
/// ```ignore
/// use usd_core::Stage;
/// use usd_sdf::PathExpression;
///
/// let stage = Stage::open("scene.usd").unwrap();
/// // Create evaluator from collection API
/// ```
pub struct ObjectCollectionExpressionEvaluator {
    /// Strong reference to the stage (needed for Clone requirement in match_path).
    stage: Option<std::sync::Arc<crate::object::Stage>>,
    /// Path expression evaluator.
    evaluator: usd_sdf::path_expression_eval::PathExpressionEval<crate::object::Object>,
}

impl ObjectCollectionExpressionEvaluator {
    /// Construct an empty evaluator.
    pub fn new() -> Self {
        Self {
            stage: None,
            evaluator: usd_sdf::path_expression_eval::PathExpressionEval::new(),
        }
    }

    /// Construct an evaluator that evaluates expr on objects from stage.
    ///
    /// Matches C++ `UsdObjectCollectionExpressionEvaluator(UsdStageWeakPtr, SdfPathExpression)`.
    pub fn new_with_stage(
        stage: std::sync::Weak<crate::object::Stage>,
        expr: usd_sdf::PathExpression,
    ) -> Self {
        // Create evaluator from expression
        // Note: In C++, this uses UsdGetCollectionPredicateLibrary() for predicate library
        // In Rust, we use the default predicate library (empty for now)
        let evaluator = usd_sdf::path_expression_eval::PathExpressionEval::from_expression(&expr);

        // Upgrade weak reference to strong reference for Clone requirement
        let stage_arc = stage.upgrade();

        Self {
            stage: stage_arc,
            evaluator,
        }
    }

    /// Return true if this evaluator has an invalid stage or an empty expression.
    ///
    /// Matches C++ `IsEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.stage.is_none() || self.evaluator.is_empty()
    }

    /// Return the stage this evaluator was constructed with.
    ///
    /// Matches C++ `GetStage()`.
    pub fn get_stage(&self) -> Option<std::sync::Arc<crate::object::Stage>> {
        self.stage.clone()
    }

    /// Return the result of evaluating the expression against path.
    ///
    /// Matches C++ `Match(SdfPath const &path)`.
    ///
    /// Returns `true` if the given path matches the path expression, `false` otherwise.
    /// If the stage is invalid or the expression is empty, returns `false`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to test against the expression
    ///
    /// # Returns
    ///
    /// `true` if the path matches the expression, `false` otherwise
    pub fn match_path(&self, path: &Path) -> bool {
        if self.evaluator.is_empty() {
            return false;
        }

        if let Some(ref stage) = self.stage {
            // Use inline closure for path-to-object mapping.
            // This maps paths to USD objects from the stage, returning invalid objects for missing paths.
            let stage_clone = stage.clone();
            let path_to_obj = move |p: &Path| -> crate::object::Object {
                stage_clone
                    .get_object_at_path(p)
                    .unwrap_or_else(crate::object::Object::invalid)
            };
            let result = self.evaluator.match_path(path, path_to_obj);
            result.value
        } else {
            false
        }
    }

    /// Return the result of evaluating the expression against object.
    ///
    /// Matches C++ `Match(UsdObject const &object)`.
    ///
    /// Returns `true` if the given object's path matches the path expression,
    /// `false` otherwise. If the object is invalid, returns `false`.
    ///
    /// # Arguments
    ///
    /// * `object` - The object to test against the expression
    ///
    /// # Returns
    ///
    /// `true` if the object's path matches the expression, `false` otherwise
    pub fn match_object(&self, object: &crate::object::Object) -> bool {
        if object.is_valid() {
            self.match_path(object.path())
        } else {
            false
        }
    }

    /// Create an incremental searcher from this evaluator.
    ///
    /// Matches C++ `MakeIncrementalSearcher()`.
    /// Note: This is a simplified implementation - full implementation would
    /// require storing the stage in the searcher for path-to-object mapping.
    /// For now, returns None as incremental search requires Clone on the closure.
    pub fn make_incremental_searcher(&self) -> Option<()> {
        // Note: Full impl requires stage ref in searcher for path-to-object mapping.
        // Returns None as incremental search requires complex lifetime management.
        // Use match_path() or match_object() for non-incremental matching.
        None
    }
}

impl Default for ObjectCollectionExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ObjectCollectionExpressionEvaluator {
    fn clone(&self) -> Self {
        Self {
            stage: self.stage.clone(),
            evaluator: self.evaluator.clone(),
        }
    }
}

impl std::fmt::Debug for ObjectCollectionExpressionEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectCollectionExpressionEvaluator")
            .field(
                "stage",
                &if self.stage.is_some() {
                    "Some(Stage)"
                } else {
                    "None"
                },
            )
            .field("evaluator_is_empty", &self.evaluator.is_empty())
            .finish()
    }
}

// ============================================================================
// CollectionMembershipQuery
// ============================================================================

/// Represents a flattened view of a collection.
///
/// Matches C++ `UsdCollectionMembershipQuery`.
#[derive(Debug, Clone)]
pub struct CollectionMembershipQuery {
    /// Base query data.
    base: CollectionMembershipQueryBase,
    /// Expression evaluator.
    expr_eval: ObjectCollectionExpressionEvaluator,
}

impl CollectionMembershipQuery {
    /// Creates a new empty query.
    pub fn new() -> Self {
        Self {
            base: CollectionMembershipQueryBase::new(),
            expr_eval: ObjectCollectionExpressionEvaluator::new(),
        }
    }

    /// Creates a new query with path expansion rule map.
    pub fn new_with_map(
        path_expansion_rule_map: PathExpansionRuleMap,
        included_collections: SdfPathSet,
    ) -> Self {
        Self {
            base: CollectionMembershipQueryBase::new_with_map(
                path_expansion_rule_map,
                included_collections,
            ),
            expr_eval: ObjectCollectionExpressionEvaluator::new(),
        }
    }

    /// Creates a new query with path expansion rule map and top expansion rule.
    pub fn new_with_map_and_rule(
        path_expansion_rule_map: PathExpansionRuleMap,
        included_collections: SdfPathSet,
        top_expansion_rule: Token,
    ) -> Self {
        Self {
            base: CollectionMembershipQueryBase::new_with_map_and_rule(
                path_expansion_rule_map,
                included_collections,
                top_expansion_rule,
            ),
            expr_eval: ObjectCollectionExpressionEvaluator::new(),
        }
    }

    /// Returns whether the given path is included in the collection.
    ///
    /// Matches C++ `IsPathIncluded(const SdfPath &path, TfToken *expansionRule)`.
    pub fn is_path_included(&self, path: &Path, expansion_rule: Option<&mut Token>) -> bool {
        if self.uses_path_expansion_rule_map() {
            let mut rule_out: Option<Token> = None;
            let result = self.base.is_path_included_by_rule_map(path, &mut rule_out);
            if let (Some(rule_out_val), Some(rule_in)) = (rule_out, expansion_rule) {
                *rule_in = rule_out_val;
            }
            result
        } else {
            // Use expression evaluator
            let result = self.expr_eval.match_path(path);
            if let Some(rule_out) = expansion_rule {
                *rule_out = if result {
                    CollectionMembershipQueryTokens::included_by_membership_expression()
                } else {
                    CollectionMembershipQueryTokens::excluded_by_membership_expression()
                };
            }
            result
        }
    }

    /// Returns whether the given path is included with parent expansion rule.
    ///
    /// Matches C++ `IsPathIncluded(const SdfPath &path, const TfToken &parentExpansionRule, TfToken *expansionRule)`.
    pub fn is_path_included_with_parent(
        &self,
        path: &Path,
        parent_expansion_rule: &Token,
        expansion_rule: Option<&mut Token>,
    ) -> bool {
        if self.uses_path_expansion_rule_map() {
            let mut rule_out: Option<Token> = None;
            let result = self.base.is_path_included_by_rule_map_with_parent(
                path,
                parent_expansion_rule,
                &mut rule_out,
            );
            if let (Some(rule_out_val), Some(rule_in)) = (rule_out, expansion_rule) {
                *rule_in = rule_out_val;
            }
            result
        } else {
            // Use expression evaluator
            let result = self.expr_eval.match_path(path);
            if let Some(rule_out) = expansion_rule {
                *rule_out = if result {
                    CollectionMembershipQueryTokens::included_by_membership_expression()
                } else {
                    CollectionMembershipQueryTokens::excluded_by_membership_expression()
                };
            }
            result
        }
    }

    /// Returns true if this query uses the explicit path-expansion rule method.
    ///
    /// Matches C++ `UsesPathExpansionRuleMap()`.
    pub fn uses_path_expansion_rule_map(&self) -> bool {
        !self.base.has_empty_rule_map()
    }

    /// Sets the expression evaluator.
    pub fn set_expression_evaluator(&mut self, expr_eval: ObjectCollectionExpressionEvaluator) {
        self.expr_eval = expr_eval;
    }

    /// Returns the expression evaluator.
    pub fn get_expression_evaluator(&self) -> &ObjectCollectionExpressionEvaluator {
        &self.expr_eval
    }

    /// Returns true if the expression evaluator is not empty.
    pub fn has_expression(&self) -> bool {
        !self.expr_eval.is_empty()
    }

    /// Returns the path expansion rule map.
    pub fn get_as_path_expansion_rule_map(&self) -> &PathExpansionRuleMap {
        self.base.get_as_path_expansion_rule_map()
    }

    /// Returns the included collections.
    pub fn get_included_collections(&self) -> &SdfPathSet {
        self.base.get_included_collections()
    }

    /// Returns the top expansion rule.
    pub fn get_top_expansion_rule(&self) -> &Token {
        self.base.get_top_expansion_rule()
    }

    /// Returns true if the collection has excludes.
    pub fn has_excludes(&self) -> bool {
        self.base.has_excludes()
    }
}

impl Default for CollectionMembershipQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CollectionMembershipQuery {
    fn eq(&self, other: &Self) -> bool {
        self.base.get_top_expansion_rule() == other.base.get_top_expansion_rule()
            && self.base.has_excludes() == other.base.has_excludes()
            && self.base.get_as_path_expansion_rule_map()
                == other.base.get_as_path_expansion_rule_map()
            && self.base.get_included_collections() == other.base.get_included_collections()
            && self.expr_eval.is_empty() == other.expr_eval.is_empty()
    }
}

impl Eq for CollectionMembershipQuery {}

impl Hash for CollectionMembershipQuery {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base.get_top_expansion_rule().get_text().hash(state);
        self.base.has_excludes().hash(state);
        // Hash map contents
        let mut map_entries: Vec<_> = self.base.get_as_path_expansion_rule_map().iter().collect();
        map_entries.sort_by(|a, b| a.0.cmp(b.0));
        for (path, rule) in map_entries {
            path.hash(state);
            rule.get_text().hash(state);
        }
        // Hash collections - convert to Vec and sort for deterministic hashing
        let mut collections: Vec<_> = self.base.get_included_collections().iter().collect();
        collections.sort();
        for path in collections {
            path.hash(state);
        }
        self.expr_eval.is_empty().hash(state);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute an SdfPathExpression that matches the same paths as ruleMap.
///
/// Computes a path expression from a collection membership query rule map.
///
/// Matches C++ `UsdComputePathExpressionFromCollectionMembershipQueryRuleMap`.
///
/// This function constructs a `PathExpression` that represents the same
/// membership rules as the given `PathExpansionRuleMap`. The resulting
/// expression can be used to recreate the collection membership query.
///
/// # Arguments
///
/// * `rule_map` - The path expansion rule map to convert
///
/// # Returns
///
/// A `PathExpression` representing the membership rules
///
/// # Note
///
/// Constructs a path expression from expansion rules:
/// - `explicitOnly` paths become exact path matches
/// - `expandPrims`/`expandPrimsAndProperties` paths become path + descendants matches
/// - `exclude` paths are subtracted from the result
pub fn compute_path_expression_from_rule_map(
    rule_map: &PathExpansionRuleMap,
) -> usd_sdf::PathExpression {
    use usd_sdf::PathExpression;
    use usd_sdf::path_expression::PathExpressionOp;

    if rule_map.is_empty() {
        return PathExpression::new();
    }

    let mut includes = PathExpression::new();
    let mut excludes = PathExpression::new();

    for (path, rule) in rule_map {
        let rule_text = rule.get_text();

        // Create expression for this path based on rule
        let expr = match rule_text {
            "explicitOnly" => {
                // Exact path only
                PathExpression::make_atom_path(path.clone())
            }
            "expandPrims" | "expandPrimsAndProperties" => {
                // Path + all descendants
                // Parse pattern like "/World//" to match path and descendants
                let pattern_str = format!("{}//*", path.as_str());
                let descendants = PathExpression::parse(&pattern_str);

                // Union the exact path with its descendants
                let exact = PathExpression::make_atom_path(path.clone());
                PathExpression::make_op(PathExpressionOp::Union, exact, descendants)
            }
            "exclude" => {
                // Add to excludes - path + descendants
                let pattern_str = format!("{}//*", path.as_str());
                let descendants = PathExpression::parse(&pattern_str);
                let exact = PathExpression::make_atom_path(path.clone());
                let exclude_expr =
                    PathExpression::make_op(PathExpressionOp::Union, exact, descendants);

                excludes = PathExpression::make_op(PathExpressionOp::Union, excludes, exclude_expr);
                continue;
            }
            _ => {
                // Unknown rule, treat as explicit
                PathExpression::make_atom_path(path.clone())
            }
        };

        includes = PathExpression::make_op(PathExpressionOp::Union, includes, expr);
    }

    // Subtract excludes from includes
    if !excludes.is_empty() {
        PathExpression::make_op(PathExpressionOp::Difference, includes, excludes)
    } else {
        includes
    }
}
