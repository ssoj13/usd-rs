
//! HdCollectionExpressionEvaluator - evaluates SdfPathExpression against scene index.
//!
//! Port of pxr/imaging/hd/collectionExpressionEvaluator.h/cpp

use crate::collection_predicate_library::{
    HdCollectionPredicateLibrary, hd_get_collection_predicate_library,
};
use crate::scene_index::base::{HdSceneIndexHandle, si_ref};
use usd_sdf::path_expression_eval::{PathExpressionEval, PredicateFunctionResult};
use usd_sdf::{Path as SdfPath, PathExpression};

use crate::scene_index::HdSceneIndexPrim;
use crate::scene_index::prim_view::HdSceneIndexPrimView;

// ---------------------------------------------------------------------------
// MatchKind
// ---------------------------------------------------------------------------

/// Configures which matching paths are returned by `populate_matches`.
///
/// Port of `HdCollectionExpressionEvaluator::MatchKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// Return all prims that match the expression.
    MatchAll,
    /// Return only the shallowest (topmost in hierarchy) matching prims.
    ShallowestMatches,
    /// Return the shallowest matching prims plus all of their descendants.
    ShallowestMatchesAndAllDescendants,
}

// ---------------------------------------------------------------------------
// HdCollectionExpressionEvaluator
// ---------------------------------------------------------------------------

/// Evaluates an `SdfPathExpression` against prims from an `HdSceneIndex`.
///
/// Port of `HdCollectionExpressionEvaluator` from
/// `pxr/imaging/hd/collectionExpressionEvaluator.h`.
pub struct HdCollectionExpressionEvaluator {
    /// Scene index to query prims from. `None` for an empty evaluator or
    /// when doing path-only matching without predicate evaluation.
    scene_index: Option<HdSceneIndexHandle>,
    /// Compiled path expression evaluator (generic over `HdSceneIndexPrim`).
    eval: PathExpressionEval<HdSceneIndexPrim>,
}

impl HdCollectionExpressionEvaluator {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Construct an empty evaluator (matches nothing).
    pub fn empty() -> Self {
        Self {
            scene_index: None,
            eval: PathExpressionEval::new(),
        }
    }

    /// Construct from a plain expression string (no scene index).
    ///
    /// Path-only patterns (no predicates) work without a scene index.
    /// Predicate components will see an empty prim and return false.
    pub fn new(expression: &str) -> Self {
        if expression.is_empty() {
            return Self::empty();
        }
        let path_expr = PathExpression::parse(expression);
        if path_expr.is_empty() {
            return Self::empty();
        }
        let mut eval: PathExpressionEval<HdSceneIndexPrim> =
            PathExpressionEval::from_expression(&path_expr);
        // Link with an empty library — path patterns still evaluate correctly;
        // unlinked predicate placeholders are treated as pass-through.
        eval.link_predicates(&HdCollectionPredicateLibrary::new());
        Self {
            scene_index: None,
            eval,
        }
    }

    /// Construct with a scene index and an explicit predicate library.
    pub fn with_scene(
        scene: HdSceneIndexHandle,
        expression: &str,
        lib: &HdCollectionPredicateLibrary,
    ) -> Self {
        if expression.is_empty() {
            return Self::empty();
        }
        let path_expr = PathExpression::parse(expression);
        if path_expr.is_empty() {
            return Self::empty();
        }
        let mut eval: PathExpressionEval<HdSceneIndexPrim> =
            PathExpressionEval::from_expression(&path_expr);
        eval.link_predicates(lib);
        Self {
            scene_index: Some(scene),
            eval,
        }
    }

    /// Construct with a scene index using the default predicate library.
    pub fn with_scene_default_lib(scene: HdSceneIndexHandle, expression: &str) -> Self {
        Self::with_scene(scene, expression, hd_get_collection_predicate_library())
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Returns `true` if this evaluator has an empty compiled expression and
    /// will never match anything.
    pub fn is_empty(&self) -> bool {
        self.eval.is_empty()
    }

    /// Returns the scene index handle, if any.
    pub fn get_scene_index(&self) -> Option<&HdSceneIndexHandle> {
        self.scene_index.as_ref()
    }

    // -----------------------------------------------------------------------
    // Match
    // -----------------------------------------------------------------------

    /// Evaluate the expression against the prim at `path`.
    ///
    /// The returned `PredicateFunctionResult`:
    /// - `.value` — whether the prim matches.
    /// - `.is_constant` — whether the result is guaranteed to be the same for
    ///   all descendants (used to prune traversal in `populate_matches`).
    pub fn match_path(&self, path: &SdfPath) -> PredicateFunctionResult {
        if self.is_empty() {
            return PredicateFunctionResult::make_constant(false);
        }

        if let Some(scene_handle) = &self.scene_index {
            let scene = scene_handle.clone();
            self.eval
                .match_path(path, move |p| si_ref(&scene).get_prim(p))
        } else {
            // No scene: path-only matching; predicates see an empty prim.
            self.eval.match_path(path, |_p| HdSceneIndexPrim::empty())
        }
    }

    /// Returns `true` when the prim at `path` matches the expression.
    #[inline]
    pub fn matches(&self, path: &SdfPath) -> bool {
        self.match_path(path).value
    }

    // -----------------------------------------------------------------------
    // PopulateAllMatches / PopulateMatches
    // -----------------------------------------------------------------------

    /// Append all prim paths under `root_path` (inclusive) that match the
    /// expression to `result`.
    ///
    /// Port of `HdCollectionExpressionEvaluator::PopulateAllMatches`.
    pub fn populate_all_matches(&self, root_path: &SdfPath, result: &mut Vec<SdfPath>) {
        self.populate_matches(root_path, MatchKind::MatchAll, result);
    }

    /// Append matching prim paths to `result`, respecting `match_kind`.
    ///
    /// Port of `HdCollectionExpressionEvaluator::PopulateMatches`.
    pub fn populate_matches(
        &self,
        root_path: &SdfPath,
        match_kind: MatchKind,
        result: &mut Vec<SdfPath>,
    ) {
        if self.is_empty() {
            return;
        }

        let scene_handle = match &self.scene_index {
            Some(s) => s.clone(),
            None => return,
        };

        let view = HdSceneIndexPrimView::with_root(scene_handle.clone(), root_path.clone());
        let mut iter = view.iter();

        while let Some(prim_path) = iter.next() {
            let r = self.match_path(&prim_path);
            let matches = r.value;
            let constant_over_descendants = r.is_constant;

            if matches {
                result.push(prim_path.clone());

                // Collect all descendants without evaluating them when:
                // - constant-true over descendants AND mode is MatchAll, or
                // - mode is ShallowestMatchesAndAllDescendants.
                let add_descendants = (constant_over_descendants
                    && match_kind == MatchKind::MatchAll)
                    || match_kind == MatchKind::ShallowestMatchesAndAllDescendants;

                if add_descendants {
                    add_all_descendants(&scene_handle, &prim_path, result);
                }

                // Skip the subtree when descendants were added directly, or
                // when only shallowest matches are requested.
                let skip = add_descendants || match_kind == MatchKind::ShallowestMatches;
                if skip {
                    iter.skip_descendants();
                }
            } else if constant_over_descendants {
                // Constant false over the whole subtree — prune it.
                iter.skip_descendants();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Add all descendants of `root_path` (excluding root itself) to `result`.
///
/// Port of the anonymous `_AddAllDescendants` in the C++ implementation.
fn add_all_descendants(scene: &HdSceneIndexHandle, root_path: &SdfPath, result: &mut Vec<SdfPath>) {
    let view = HdSceneIndexPrimView::with_root(scene.clone(), root_path.clone());
    let mut iter = view.iter();
    iter.next(); // skip root_path itself
    for path in iter {
        result.push(path);
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> SdfPath {
        SdfPath::from_string(s).unwrap()
    }

    #[test]
    fn test_empty_evaluator() {
        let eval = HdCollectionExpressionEvaluator::new("");
        assert!(eval.is_empty());
        assert!(!eval.matches(&p("/A")));
    }

    #[test]
    fn test_exact_match() {
        let eval = HdCollectionExpressionEvaluator::new("/World/Mesh");
        assert!(eval.matches(&p("/World/Mesh")));
        assert!(!eval.matches(&p("/World/Light")));
    }

    #[test]
    fn test_single_wildcard() {
        let eval = HdCollectionExpressionEvaluator::new("/World/*");
        assert!(eval.matches(&p("/World/Mesh")));
        assert!(!eval.matches(&p("/World/Sub/Mesh")));
    }

    #[test]
    fn test_recursive_wildcard() {
        let eval = HdCollectionExpressionEvaluator::new("/World//*");
        assert!(eval.matches(&p("/World/Mesh")));
        assert!(eval.matches(&p("/World/Sub/Deep/Mesh")));
        assert!(!eval.matches(&p("/Other/Mesh")));
    }
}
