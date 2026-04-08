//! HDSI utility functions.
//!
//! Port of pxr/imaging/hdsi/utils.h and utils.cpp.
//!
//! Provides collection compilation and pruning utilities for scene indices.

use usd_hd::scene_index::HdSceneIndexPrim;
use usd_hd::scene_index::{HdSceneIndexHandle, si_ref};
use usd_hd::schema::HdCollectionsSchema;
use usd_sdf::Path as SdfPath;
use usd_sdf::PathExpression;
use usd_sdf::path_expression_eval::{PathExpressionEval, PredicateFunctionResult};
use usd_tf::Token as TfToken;

// ---------------------------------------------------------------------------
// Collection utilities (C++ parity: HdsiUtilsCompileCollection, etc.)
// ---------------------------------------------------------------------------

/// Evaluates SdfPathExpressions with prims from a scene index.
///
/// Port of HdCollectionExpressionEvaluator.
/// Requires HdCollectionPredicateLibrary for full predicate support.
#[derive(Clone, Default)]
pub struct HdCollectionExpressionEvaluator {
    scene_index: Option<HdSceneIndexHandle>,
    eval: PathExpressionEval<HdSceneIndexPrim>,
}

impl HdCollectionExpressionEvaluator {
    /// Creates an empty evaluator.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates evaluator from scene index and path expression.
    pub fn new(scene_index: HdSceneIndexHandle, expr: &PathExpression) -> Self {
        let eval = PathExpressionEval::from_expression(expr);
        Self {
            scene_index: Some(scene_index),
            eval,
        }
    }

    /// Returns true if evaluator has invalid scene index or empty expression.
    pub fn is_empty(&self) -> bool {
        self.scene_index.is_none() || self.eval.is_empty()
    }

    /// Returns the scene index, or None if default-constructed.
    pub fn get_scene_index(&self) -> Option<&HdSceneIndexHandle> {
        self.scene_index.as_ref()
    }

    /// Evaluates the expression at the given path.
    pub fn match_path(&self, path: &SdfPath) -> PredicateFunctionResult {
        if self.is_empty() {
            return PredicateFunctionResult::make_constant(false);
        }
        let si = self.scene_index.as_ref().unwrap();
        self.eval.match_path(path, |p| si_ref(&si).get_prim(p))
    }

    /// Populates `result` with all prim paths under `root` that match the expression.
    /// Port of HdCollectionExpressionEvaluator::PopulateMatches with MatchAll.
    pub fn populate_all_matches(&self, root: &SdfPath, result: &mut Vec<SdfPath>) {
        if self.is_empty() {
            return;
        }
        let si = self.scene_index.as_ref().unwrap();
        let all_paths = collect_prim_paths(si, root);
        for path in all_paths {
            if path.is_prim_path() || path.is_absolute_root_path() {
                let m = self.match_path(&path);
                if m.value {
                    result.push(path);
                }
            }
        }
    }
}

/// Extracts and compiles the membership expression of the collection.
///
/// If the collection exists and has a membership expression, populates
/// `expr` and `eval`. Port of HdsiUtilsCompileCollection.
pub fn compile_collection(
    collections: &HdCollectionsSchema,
    collection_name: &TfToken,
    scene_index: &HdSceneIndexHandle,
    expr: &mut PathExpression,
    eval: &mut Option<HdCollectionExpressionEvaluator>,
) {
    let collection = collections.get_collection(collection_name);
    if !collection.is_defined() {
        return;
    }
    let path_expr_ds = match collection.get_membership_expression() {
        Some(ds) => ds,
        None => return,
    };
    if let Some(sampled) = path_expr_ds.as_ref().as_sampled() {
        let val = sampled.get_value(0.0);
        if let Some(pe) = val.downcast_clone::<PathExpression>() {
            if !pe.is_empty() {
                *expr = pe;
                *eval = Some(HdCollectionExpressionEvaluator::new(
                    scene_index.clone(),
                    &*expr,
                ));
            }
        }
    }
}

/// Returns whether the prim at `prim_path` is pruned by the evaluator.
///
/// Port of HdsiUtilsIsPruned.
pub fn is_pruned(prim_path: &SdfPath, eval: &HdCollectionExpressionEvaluator) -> bool {
    if eval.is_empty() || prim_path.is_empty() {
        return false;
    }
    let result = get_prune_match_result(prim_path, eval);
    result.value
}

/// Prunes the given list of children using the evaluator.
///
/// Port of HdsiUtilsRemovePrunedChildren.
pub fn remove_pruned_children(
    parent_path: &SdfPath,
    eval: &HdCollectionExpressionEvaluator,
    children: &mut Vec<SdfPath>,
) {
    if eval.is_empty() {
        return;
    }
    if children.is_empty() {
        return;
    }
    let result = get_prune_match_result(parent_path, eval);
    if result.value {
        children.clear();
        return;
    }
    if result.is_constant {
        return;
    }
    children.retain(|child_path| !eval.match_path(child_path).value);
}

/// For pruning: ancestral match counts. Evaluates path and its prefixes.
fn get_prune_match_result(
    prim_path: &SdfPath,
    eval: &HdCollectionExpressionEvaluator,
) -> PredicateFunctionResult {
    let mut prefixes = Vec::new();
    let mut p = prim_path.clone();
    while !p.is_empty() {
        prefixes.push(p.clone());
        if p.is_absolute_root_path() {
            break;
        }
        p = p.get_parent_path();
    }
    for path in prefixes {
        let result = eval.match_path(&path);
        if result.value {
            return result;
        }
        if result.is_constant {
            return result;
        }
    }
    PredicateFunctionResult::make_constant(false)
}

// ---------------------------------------------------------------------------
// Coord sys and path utilities
// ---------------------------------------------------------------------------

/// Prefix for coordinate system child prim names (C++ parity: __coordSys_).
/// Coord sys prims are child prims: /Target/__coordSys_FOO
pub const COORD_SYS_PRIM_PREFIX: &str = "__coordSys_";

/// Check if a path is a prim path (vs property path).
pub fn is_prim_path(path: &SdfPath) -> bool {
    path.is_prim_path()
}

/// Check if a path is under a given prefix.
pub fn is_path_under_prefix(path: &SdfPath, prefix: &SdfPath) -> bool {
    path.has_prefix(prefix)
}

/// Check if a path is a coordinate system prim path (C++ parity).
///
/// Coord sys prims are child prims: /path/to/target/__coordSys_NAME
pub fn is_coord_sys_prim_path(path: &SdfPath) -> bool {
    if !path.is_prim_path() || path.is_absolute_root_path() {
        return false;
    }
    let name = path.get_name();
    name.starts_with(COORD_SYS_PRIM_PREFIX)
}

/// Extract coord sys name from coord sys prim path (C++ parity).
///
/// For path /path/to/target/__coordSys_NAME returns NAME.
pub fn extract_coord_sys_name(path: &SdfPath) -> Option<TfToken> {
    if !is_coord_sys_prim_path(path) {
        return None;
    }
    let name = path.get_name();
    let suffix = &name[COORD_SYS_PRIM_PREFIX.len()..];
    if suffix.is_empty() {
        return None;
    }
    Some(TfToken::new(suffix))
}

/// Construct coord sys prim path from target path and name (C++ parity).
///
/// Creates child prim path: /target/__coordSys_NAME
pub fn make_coord_sys_prim_path(target: &SdfPath, name: &TfToken) -> SdfPath {
    let child_name = format!("{}{}", COORD_SYS_PRIM_PREFIX, name.as_str());
    target
        .append_child(&child_name)
        .unwrap_or_else(|| target.clone())
}

/// Depth-first collect all prim paths under root (including root).
/// Port of HdSceneIndexPrimView iteration.
pub fn collect_prim_paths(scene: &HdSceneIndexHandle, root: &SdfPath) -> Vec<SdfPath> {
    let mut result = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(path) = stack.pop() {
        result.push(path.clone());
        let children = si_ref(&scene).get_child_prim_paths(&path);
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }
    result
}

/// Get the target prim path from a coord sys prim path.
///
/// Given /World/Cube/__coordSys_modelSpace, returns /World/Cube
pub fn get_coord_sys_target_path(path: &SdfPath) -> Option<SdfPath> {
    if !is_coord_sys_prim_path(path) {
        return None;
    }
    Some(path.get_parent_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_prim_path() {
        let path = SdfPath::from_string("/World/Cube").unwrap();
        assert!(is_prim_path(&path));

        let prop_path = SdfPath::from_string("/World/Cube.visibility").unwrap();
        assert!(!is_prim_path(&prop_path));
    }

    #[test]
    fn test_is_path_under_prefix() {
        let path = SdfPath::from_string("/World/Cube/Mesh").unwrap();
        let prefix = SdfPath::from_string("/World").unwrap();
        assert!(is_path_under_prefix(&path, &prefix));

        let other = SdfPath::from_string("/Other/Thing").unwrap();
        assert!(!is_path_under_prefix(&other, &prefix));
    }

    #[test]
    fn test_is_coord_sys_prim_path() {
        let coord_sys = SdfPath::from_string("/World/Cube/__coordSys_modelSpace").unwrap();
        assert!(is_coord_sys_prim_path(&coord_sys));

        let regular = SdfPath::from_string("/World/Cube/Child").unwrap();
        assert!(!is_coord_sys_prim_path(&regular));

        let prim = SdfPath::from_string("/World/Cube").unwrap();
        assert!(!is_coord_sys_prim_path(&prim));
    }

    #[test]
    fn test_extract_coord_sys_name() {
        let path = SdfPath::from_string("/World/Cube/__coordSys_modelSpace").unwrap();
        let name = extract_coord_sys_name(&path);
        assert!(name.is_some());
        assert_eq!(name.unwrap().as_str(), "modelSpace");

        let regular = SdfPath::from_string("/World/Cube/Child").unwrap();
        assert!(extract_coord_sys_name(&regular).is_none());
    }

    #[test]
    fn test_make_coord_sys_prim_path() {
        let target = SdfPath::from_string("/World/Cube").unwrap();
        let name = TfToken::new("modelSpace");
        let result = make_coord_sys_prim_path(&target, &name);
        assert_eq!(result.as_str(), "/World/Cube/__coordSys_modelSpace");
    }

    #[test]
    fn test_get_coord_sys_target_path() {
        let coord_sys = SdfPath::from_string("/World/Cube/__coordSys_modelSpace").unwrap();
        let target = get_coord_sys_target_path(&coord_sys);
        assert!(target.is_some());
        assert_eq!(target.unwrap().as_str(), "/World/Cube");

        let regular = SdfPath::from_string("/World/Cube/Child").unwrap();
        assert!(get_coord_sys_target_path(&regular).is_none());
    }

    #[test]
    fn test_roundtrip_coord_sys_path() {
        let target = SdfPath::from_string("/World/Camera").unwrap();
        let name = TfToken::new("worldSpace");

        let coord_sys = make_coord_sys_prim_path(&target, &name);
        assert_eq!(
            extract_coord_sys_name(&coord_sys).unwrap().as_str(),
            "worldSpace"
        );
        assert_eq!(
            get_coord_sys_target_path(&coord_sys).unwrap().as_str(),
            "/World/Camera"
        );
    }
}
