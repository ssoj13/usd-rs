//! PCP Map Function for namespace mapping.
//!
//! A `MapFunction` maps values from one namespace (and time domain) to another.
//! It represents the transformation that an arc (such as a reference) applies
//! as it incorporates values across the arc.
//!
//! # Overview
//!
//! Consider a reference arc where source path `/Model` is referenced as target
//! path `/Model_1`. Values in the model that refer to paths relative to `/Model`
//! must be transformed to be relative to `/Model_1` instead. The `MapFunction`
//! for the arc provides this service.
//!
//! # Domain
//!
//! Map functions have a specific *domain*, or set of values they can operate on.
//! Any values outside the domain cannot be mapped. The domain precisely tracks
//! what areas of namespace can be referred to across various forms of arcs.
//!
//! # Examples
//!
//! ```
//! use usd_pcp::MapFunction;
//! use usd_sdf::{Path, LayerOffset};
//!
//! // Create identity map function
//! let identity = MapFunction::identity();
//! assert!(identity.is_identity());
//!
//! // Map a path through identity (returns unchanged)
//! let path = Path::from_string("/World/Cube").unwrap();
//! let mapped = identity.map_source_to_target(&path);
//! assert_eq!(mapped, Some(path));
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use usd_sdf::path_expression::{ExpressionReference, PathPattern};
use usd_sdf::{LayerOffset, Path, PathExpression};

/// A pair of paths (source, target) for mapping.
pub type PathPair = (Path, Path);

/// A vector of path pairs.
pub type PathPairVector = Vec<PathPair>;

/// A mapping from source paths to target paths.
///
/// Uses `BTreeMap` to ensure consistent ordering (equivalent to C++ `std::map`
/// with custom comparator).
pub type PathMap = BTreeMap<Path, Path>;

/// A function that maps values from one namespace (and time domain) to another.
///
/// It represents the transformation that an arc such as a reference arc applies
/// as it incorporates values across the arc.
///
/// Map functions can be chained to represent a series of map operations applied
/// in sequence. The map function represents the cumulative effect as efficiently
/// as possible.
///
/// Map functions can be inverted. Formally, map functions are bijections
/// (one-to-one and onto), which ensures that they can be inverted.
///
/// # Invariant
///
/// `pairs` is sorted in _PathPairOrder: non-decreasing by source path element count,
/// then by SdfPath::FastLessThan. This ordering is required for efficient prefix matching.
#[derive(Clone)]
pub struct MapFunction {
    /// The path mappings (source -> target), sorted by source element count.
    pairs: Vec<PathPair>,
    /// Whether the mapping includes root identity (/ -> /).
    has_root_identity: bool,
    /// The time offset applied from source to target.
    offset: LayerOffset,
}

impl Default for MapFunction {
    fn default() -> Self {
        Self::null()
    }
}

// ============================================================================
// Internal helpers - matching C++ _PathPairOrder, _GetBestSourceMatch, etc.
// ============================================================================

/// Sort pairs by source element count, then lexicographically.
fn sort_pairs(pairs: &mut Vec<PathPair>) {
    pairs.sort_by(|a, b| {
        let ac = a.0.get_path_element_count();
        let bc = b.0.get_path_element_count();
        if ac != bc {
            return ac.cmp(&bc);
        }
        a.0.as_str().cmp(b.0.as_str())
    });
}

/// Find the entry whose source best matches `path` (longest source prefix of `path`).
///
/// `pairs` must be sorted by source element count (ascending). Returns index into
/// `pairs`, or `pairs.len()` if not found.
///
/// `min_element_count` is a short-circuit: stop if we reach sources shorter than this.
fn get_best_source_match(pairs: &[PathPair], path: &Path, min_element_count: usize) -> usize {
    let path_len = path.get_path_element_count();

    // Find first entry whose source length exceeds path length (upper_bound).
    // Since pairs are sorted by source count ascending, we search from the last
    // entry with source_count <= path_len.
    let upper = pairs.partition_point(|p| p.0.get_path_element_count() <= path_len);

    // Iterate backwards from upper - 1
    let mut i = upper;
    while i > 0 {
        i -= 1;
        let source_count = pairs[i].0.get_path_element_count();
        if source_count < min_element_count {
            return pairs.len(); // no match satisfying minimum
        }
        if path.has_prefix(&pairs[i].0) {
            return i;
        }
    }
    pairs.len()
}

/// Find the entry whose target best matches `path` (longest target prefix of `path`).
///
/// Targets are not sorted by element count so we do a linear scan.
/// `min_element_count` means we only consider entries with target count >= this value.
fn get_best_target_match(pairs: &[PathPair], path: &Path, min_element_count: usize) -> usize {
    let mut best_idx = pairs.len();
    let mut best_count = min_element_count;

    for (i, pair) in pairs.iter().enumerate() {
        let count = pair.1.get_path_element_count();
        if count >= best_count && path.has_prefix(&pair.1) {
            best_count = count;
            best_idx = i;
        }
    }
    best_idx
}

/// Returns true if there is a map entry that matches `target_path` better (on the
/// target side) than `best_source_match` -- which would mean the mapping is not
/// bijective and the path cannot be mapped.
///
/// `invert` swaps the meaning of source and target (for MapTargetToSource).
fn has_better_target_match(
    pairs: &[PathPair],
    target_path: &Path,
    best_source_match: usize,
    invert: bool,
) -> bool {
    // For a target match to be "better", its target element count must be >
    // the target of best_source_match.
    let min_element_count = if best_source_match == pairs.len() {
        0
    } else if invert {
        // In inverted mode, source/target swap: "target" here is the source path
        pairs[best_source_match].0.get_path_element_count()
    } else {
        pairs[best_source_match].1.get_path_element_count()
    };

    let best_target = if invert {
        get_best_source_match(pairs, target_path, min_element_count)
    } else {
        get_best_target_match(pairs, target_path, min_element_count)
    };

    best_target != pairs.len() && best_target != best_source_match
}

/// Remove redundant entries from `pairs` in-place. Also detect and remove the
/// root identity pair (/ -> /), returning `true` if it was present.
///
/// This matches C++ `_Canonicalize`. `pairs` must already be sorted.
fn canonicalize(pairs: &mut Vec<PathPair>) -> bool {
    let mut i = 0;
    while i < pairs.len() {
        if is_redundant(pairs, i) {
            pairs.remove(i);
            // don't increment i - recheck same position with shifted array
        } else {
            i += 1;
        }
    }

    // Remove root identity (/ -> /) which is stored separately in has_root_identity.
    let abs_root = Path::absolute_root();
    if !pairs.is_empty() && pairs[0].0 == abs_root && pairs[0].1 == abs_root {
        pairs.remove(0);
        return true;
    }
    false
}

/// Returns true if entry at `idx` is redundant (can be removed without changing semantics).
fn is_redundant(pairs: &[PathPair], idx: usize) -> bool {
    let entry_source = &pairs[idx].0;
    let entry_target = &pairs[idx].1;
    let is_block = entry_target.is_empty();

    // Temporary view excluding the entry at idx for finding "other" matches
    let other_pairs: Vec<&PathPair> = pairs
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != idx)
        .map(|(_, p)| p)
        .collect();

    if is_block {
        // A block is redundant if the source path wouldn't map anyway.
        // Check if parent would be affected by another mapping.
        let parent = entry_source.get_parent_path();
        let best = find_best_source_match_linear(&other_pairs, &parent);
        if best.is_none() || best.unwrap().1.is_empty() {
            return true;
        }
        let best_pair = best.unwrap();
        // Compute what the target would be without the block
        if let Some(target_path) = entry_source.replace_prefix(&best_pair.0, &best_pair.1) {
            // Block is redundant if a better inverse mapping exists
            return has_better_target_match_linear(&other_pairs, &target_path, Some(best_pair));
        }
        true
    } else {
        // Non-block mapping
        // Fast check: if it renames, it can't be redundant
        if entry_source.get_name_token() != entry_target.get_name_token() {
            return false;
        }

        let parent = entry_source.get_parent_path();
        let best = find_best_source_match_linear(&other_pairs, &parent);
        if best.is_none() || best.unwrap().1.is_empty() {
            return false;
        }
        let best_pair = best.unwrap();

        // Check namespace depth difference matches
        let target_depth_diff = entry_target
            .get_path_element_count()
            .wrapping_sub(best_pair.1.get_path_element_count());
        let source_depth_diff = entry_source
            .get_path_element_count()
            .wrapping_sub(best_pair.0.get_path_element_count());
        if target_depth_diff != source_depth_diff {
            return false;
        }

        // Check that the ancestor path names all match
        let mut src_anc = entry_source.get_parent_path();
        let mut tgt_anc = entry_target.get_parent_path();
        while src_anc != best_pair.0 {
            if src_anc.get_name_token() != tgt_anc.get_name_token() {
                return false;
            }
            src_anc = src_anc.get_parent_path();
            tgt_anc = tgt_anc.get_parent_path();
        }
        if best_pair.1 != tgt_anc {
            return false;
        }

        // Check no better inverse match for parent target
        let target_parent = entry_target.get_parent_path();
        !has_better_target_match_linear(&other_pairs, &target_parent, Some(best_pair))
    }
}

/// Linear best-source-match helper for canonicalization (used on subset views).
fn find_best_source_match_linear<'a>(pairs: &[&'a PathPair], path: &Path) -> Option<&'a PathPair> {
    let path_len = path.get_path_element_count();
    let mut best: Option<&PathPair> = None;
    let mut best_count = 0usize;

    for pair in pairs {
        let count = pair.0.get_path_element_count();
        if count <= path_len && count >= best_count && path.has_prefix(&pair.0) {
            best_count = count;
            best = Some(pair);
        }
    }
    best
}

/// Linear best-target-match helper for canonicalization.
///
/// Matches C++ `_HasBetterTargetMatch`: finds any entry (other than `current_best`)
/// whose target has element count >= min_count and is a prefix of `target_path`.
/// Such an entry would be "better" or "equal" — meaning the source-to-target
/// mapping is not unique and a block or entry may be redundant.
fn has_better_target_match_linear(
    pairs: &[&PathPair],
    target_path: &Path,
    current_best: Option<&PathPair>,
) -> bool {
    // min_count mirrors C++ minElementCount: element count of best_source_match's target.
    // Any other match with count >= min_count is "better" (or ties at the same depth).
    let min_count = current_best
        .map(|p| p.1.get_path_element_count())
        .unwrap_or(0);

    for pair in pairs {
        // Skip the entry we already know about (current_best).
        if Some(*pair) == current_best {
            continue;
        }
        let count = pair.1.get_path_element_count();
        // C++ _GetBestTargetMatch: `if (count >= bestElementCount && path.HasPrefix(target))`
        // Any match with count >= min_count that we haven't seen is a "better target match".
        if count >= min_count && target_path.has_prefix(&pair.1) {
            return true;
        }
    }
    false
}

/// Core path mapping function. Matches C++ `_Map()`.
fn map_path(
    path: &Path,
    pairs: &[PathPair],
    has_root_identity: bool,
    invert: bool,
) -> Option<Path> {
    // Find longest prefix match in the mapping
    let best_match = if invert {
        get_best_target_match(pairs, path, 0)
    } else {
        get_best_source_match(pairs, path, 0)
    };

    let result = if best_match == pairs.len() {
        if has_root_identity {
            // Use the root identity: path maps to itself
            Some(path.clone())
        } else {
            None
        }
    } else {
        let (source, target) = &pairs[best_match];
        if invert {
            // Map target -> source: replace target prefix with source prefix.
            // C++ passes fixTargetPaths=false — embedded target paths are NOT fixed.
            if target.is_empty() {
                None // blocked mapping
            } else {
                path.replace_prefix_with_fix(target, source, false)
            }
        } else {
            // Map source -> target: replace source prefix with target prefix.
            // C++ passes fixTargetPaths=false — embedded target paths are NOT fixed.
            if target.is_empty() {
                None // blocked mapping
            } else {
                path.replace_prefix_with_fix(source, target, false)
            }
        }
    };

    let result = match result {
        Some(r) => r,
        None => return None,
    };

    // Bijectivity check: ensure result maps back to the original path.
    // If a better (closer) inverse match exists, the mapping is non-invertible.
    if has_better_target_match(pairs, &result, best_match, invert) {
        return None;
    }

    Some(result)
}

impl MapFunction {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Constructs a null map function.
    ///
    /// For a null function, `map_source_to_target()` always returns `None`.
    pub fn null() -> Self {
        Self {
            pairs: Vec::new(),
            has_root_identity: false,
            offset: LayerOffset::identity(),
        }
    }

    /// Constructs a map function with the given arguments.
    ///
    /// Returns `None` if the map is invalid (e.g., invalid paths).
    ///
    /// Matches C++ `PcpMapFunction::Create()`. Validates paths, sorts entries,
    /// and canonicalizes to remove redundant mappings.
    pub fn create(source_to_target: PathMap, offset: LayerOffset) -> Option<Self> {
        // Fast path: if it's just root -> root with identity offset, return identity
        let abs_root = Path::absolute_root();
        if source_to_target.len() == 1 && offset.is_identity() {
            if let Some(target) = source_to_target.get(&abs_root) {
                if *target == abs_root {
                    return Some(Self::identity().clone());
                }
            }
        }

        // Validate paths: source and target must be absolute prim paths (or empty target for blocks)
        for (source, target) in &source_to_target {
            let is_valid_map_path = |p: &Path| {
                p.is_absolute_path()
                    && (p.is_absolute_root_or_prim_path() || p.is_prim_variant_selection_path())
            };
            if !is_valid_map_path(source) {
                return None;
            }
            if !target.is_empty() && !is_valid_map_path(target) {
                return None;
            }
        }

        // Convert to pairs and sort by source element count (_PathPairOrder)
        let mut pairs: Vec<PathPair> = source_to_target.into_iter().collect();
        sort_pairs(&mut pairs);

        // Canonicalize: remove redundant entries and extract root identity
        let has_root_identity = canonicalize(&mut pairs);

        Some(Self {
            pairs,
            has_root_identity,
            offset,
        })
    }

    /// Constructs an identity map function.
    ///
    /// The identity function has an identity path mapping and time offset.
    pub fn identity() -> &'static Self {
        static IDENTITY: OnceLock<MapFunction> = OnceLock::new();
        IDENTITY.get_or_init(|| Self {
            pairs: Vec::new(),
            has_root_identity: true,
            offset: LayerOffset::identity(),
        })
    }

    /// Returns an identity path mapping.
    pub fn identity_path_map() -> &'static PathMap {
        static IDENTITY_MAP: OnceLock<PathMap> = OnceLock::new();
        IDENTITY_MAP.get_or_init(|| {
            let mut map = PathMap::new();
            map.insert(Path::absolute_root(), Path::absolute_root());
            map
        })
    }

    // ========================================================================
    // Query
    // ========================================================================

    /// Returns `true` if this is the null function.
    ///
    /// For a null function, `map_source_to_target()` always returns `None`.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.pairs.is_empty() && !self.has_root_identity
    }

    /// Returns `true` if this is the identity function.
    ///
    /// The identity function has an identity path mapping and time offset.
    #[inline]
    pub fn is_identity(&self) -> bool {
        self.pairs.is_empty() && self.has_root_identity && self.offset.is_identity()
    }

    /// Returns `true` if this function uses the identity path mapping.
    ///
    /// If true, `map_source_to_target()` always returns the path unchanged.
    /// However, this map function may have a non-identity time offset.
    #[inline]
    pub fn is_identity_path_mapping(&self) -> bool {
        self.pairs.is_empty() && self.has_root_identity
    }

    /// Returns `true` if the map function maps the absolute root path to itself.
    #[inline]
    pub fn has_root_identity(&self) -> bool {
        self.has_root_identity
    }

    /// Returns the time offset of the mapping.
    #[inline]
    pub fn time_offset(&self) -> &LayerOffset {
        &self.offset
    }

    // ========================================================================
    // Mapping Operations
    // ========================================================================

    /// Maps a path in the source namespace to the target.
    ///
    /// Returns `None` if the path is not in the domain.
    pub fn map_source_to_target(&self, path: &Path) -> Option<Path> {
        if self.is_null() {
            return None;
        }
        map_path(path, &self.pairs, self.has_root_identity, false)
    }

    /// Maps a path in the target namespace to the source.
    ///
    /// Returns `None` if the path is not in the co-domain.
    pub fn map_target_to_source(&self, path: &Path) -> Option<Path> {
        if self.is_null() {
            return None;
        }
        map_path(path, &self.pairs, self.has_root_identity, true)
    }

    /// Maps a PathExpression from source namespace to target namespace.
    pub fn map_source_to_target_path_expression(
        &self,
        expr: &PathExpression,
    ) -> Option<PathExpression> {
        self.map_source_to_target_path_expression_with_unmapped(expr, None, None)
    }

    /// Maps a PathExpression from source namespace to target namespace, with unmapped tracking.
    pub fn map_source_to_target_path_expression_with_unmapped(
        &self,
        expr: &PathExpression,
        unmapped_patterns: Option<&mut Vec<PathPattern>>,
        unmapped_refs: Option<&mut Vec<ExpressionReference>>,
    ) -> Option<PathExpression> {
        if self.is_null() {
            return None;
        }
        if self.is_identity_path_mapping() {
            return Some(expr.clone());
        }
        self.map_path_expression_impl(expr, false, unmapped_patterns, unmapped_refs)
    }

    /// Maps a PathExpression from target namespace to source namespace.
    pub fn map_target_to_source_path_expression(
        &self,
        expr: &PathExpression,
    ) -> Option<PathExpression> {
        if self.is_null() {
            return None;
        }
        if self.is_identity_path_mapping() {
            return Some(expr.clone());
        }
        self.map_path_expression_impl(expr, true, None, None)
    }

    /// Maps a PathExpression from target namespace to source namespace, with unmapped tracking.
    pub fn map_target_to_source_path_expression_with_unmapped(
        &self,
        expr: &PathExpression,
        unmapped_patterns: Option<&mut Vec<PathPattern>>,
        unmapped_refs: Option<&mut Vec<ExpressionReference>>,
    ) -> Option<PathExpression> {
        if self.is_null() {
            return None;
        }
        if self.is_identity_path_mapping() {
            return Some(expr.clone());
        }
        self.map_path_expression_impl(expr, true, unmapped_patterns, unmapped_refs)
    }

    /// Internal implementation of PathExpression mapping. Matches C++ `_MapPathExpressionImpl`.
    fn map_path_expression_impl(
        &self,
        expr: &PathExpression,
        invert: bool,
        unmapped_patterns: Option<&mut Vec<PathPattern>>,
        unmapped_refs: Option<&mut Vec<ExpressionReference>>,
    ) -> Option<PathExpression> {
        use std::cell::RefCell;
        use usd_sdf::PathExpression;
        use usd_sdf::path_expression::{ExpressionReference, PathExpressionOp, PathPattern};

        if expr.is_empty() {
            return Some(PathExpression::nothing());
        }

        let stack: RefCell<Vec<PathExpression>> = RefCell::new(Vec::new());
        let unmapped_patterns_cell = unmapped_patterns.map(RefCell::new);
        let unmapped_refs_cell = unmapped_refs.map(RefCell::new);

        let map = |path: &Path| map_path(path, &self.pairs, self.has_root_identity, invert);

        let logic = {
            let stack = &stack;
            move |op: PathExpressionOp, arg_index: i32| {
                if op == PathExpressionOp::Complement {
                    if arg_index == 1 {
                        let mut s = stack.borrow_mut();
                        if let Some(expr) = s.pop() {
                            s.push(PathExpression::make_complement(expr));
                        }
                    }
                } else if arg_index == 2 {
                    let mut s = stack.borrow_mut();
                    if let (Some(arg2), Some(arg1)) = (s.pop(), s.pop()) {
                        s.push(PathExpression::make_op(op, arg1, arg2));
                    }
                }
            }
        };

        let map_ref = {
            let stack = &stack;
            let unmapped_refs_cell = &unmapped_refs_cell;
            move |ref_expr: &ExpressionReference| {
                if ref_expr.path.is_empty() {
                    stack
                        .borrow_mut()
                        .push(PathExpression::make_atom_ref(ref_expr.clone()));
                } else {
                    let mapped = map(&ref_expr.path);
                    if mapped.is_none() {
                        if let Some(cell) = unmapped_refs_cell {
                            cell.borrow_mut().push(ref_expr.clone());
                        }
                        stack.borrow_mut().push(PathExpression::nothing());
                    } else {
                        let mut mapped_ref = ref_expr.clone();
                        mapped_ref.path = mapped.expect("checked above");
                        stack
                            .borrow_mut()
                            .push(PathExpression::make_atom_ref(mapped_ref));
                    }
                }
            }
        };

        let map_pattern = {
            let stack = &stack;
            let unmapped_patterns_cell = &unmapped_patterns_cell;
            move |pattern: &PathPattern| {
                // Leading stretch (//) patterns are preserved unchanged (C++ behavior)
                let has_leading_stretch = pattern
                    .components()
                    .first()
                    .map(|c| {
                        matches!(
                            c,
                            usd_sdf::path_expression::PatternComponent::RecursiveWildcard
                        )
                    })
                    .unwrap_or(false);

                if has_leading_stretch {
                    stack
                        .borrow_mut()
                        .push(PathExpression::make_atom_pattern(pattern.clone()));
                } else {
                    let mapped = map(pattern.prefix());
                    if mapped.is_none() {
                        if let Some(cell) = unmapped_patterns_cell {
                            cell.borrow_mut().push(pattern.clone());
                        }
                        stack.borrow_mut().push(PathExpression::nothing());
                    } else {
                        // Map the prefix, preserving wildcard components (C++ parity)
                        let mapped_path = mapped.expect("checked above");
                        let mapped_pattern = pattern.with_prefix(mapped_path);
                        stack
                            .borrow_mut()
                            .push(PathExpression::make_atom_pattern(mapped_pattern));
                    }
                }
            }
        };

        expr.walk(logic, map_ref, map_pattern);

        let mut s = stack.borrow_mut();
        s.pop().or_else(|| Some(PathExpression::nothing()))
    }

    // ========================================================================
    // Composition
    // ========================================================================

    /// Composes this map over the given map function `inner`.
    ///
    /// The result represents: first apply `inner`, then apply `self`.
    ///
    /// Matches C++ `PcpMapFunction::Compose()`. Properly handles bijectivity,
    /// de-duplication via binary search, and canonicalization.
    pub fn compose(&self, inner: &MapFunction) -> MapFunction {
        // Fast path identities
        if self.is_identity() {
            return inner.clone();
        }
        if inner.is_identity() {
            return self.clone();
        }

        let mut scratch: Vec<PathPair> = Vec::new();
        scratch.reserve(
            inner.pairs.len()
                + usize::from(inner.has_root_identity)
                + self.pairs.len()
                + usize::from(self.has_root_identity),
        );

        // Root identity if and only if both functions have it
        if self.has_root_identity && inner.has_root_identity {
            scratch.push((Path::absolute_root(), Path::absolute_root()));
        }

        // Apply outer (self) to the output range of inner.
        // The inner function's sources are already in sorted order so the
        // resulting entries are also sorted (by source element count from inner).
        for (inner_source, inner_target) in &inner.pairs {
            // Map inner's target through self
            let composed_target = self.map_source_to_target(inner_target);
            scratch.push((inner_source.clone(), composed_target.unwrap_or_default()));
        }

        // Track the sorted portion of scratch (the entries added above)
        let scratch_sorted_end = scratch.len();

        // Apply the inverse of inner to the domain of self.
        for (self_source, self_target) in &self.pairs {
            let source = match inner.map_target_to_source(self_source) {
                Some(s) => s,
                None => continue,
            };

            // Check if this source was already added (binary search in sorted prefix)
            let already_present = scratch[..scratch_sorted_end].binary_search_by(|p| {
                let p_count = p.0.get_path_element_count();
                let s_count = source.get_path_element_count();
                match p_count.cmp(&s_count) {
                    std::cmp::Ordering::Equal => p.0.as_str().cmp(source.as_str()),
                    other => other,
                }
            });

            if already_present.is_ok() {
                continue;
            }

            scratch.push((source, self_target.clone()));
        }

        // Sort the unsorted portion (from second step), then merge with sorted prefix.
        // Matches C++: std::sort(scratchSortedEnd, scratch) + std::inplace_merge(...).
        if scratch_sorted_end < scratch.len() {
            scratch[scratch_sorted_end..].sort_by(|a, b| {
                let ac = a.0.get_path_element_count();
                let bc = b.0.get_path_element_count();
                if ac != bc {
                    return ac.cmp(&bc);
                }
                a.0.as_str().cmp(b.0.as_str())
            });
            // Stable merge the two sorted halves (inplace_merge semantics)
            // to preserve relative ordering within each half.
            let pair_cmp = |a: &PathPair, b: &PathPair| -> std::cmp::Ordering {
                let ac = a.0.get_path_element_count();
                let bc = b.0.get_path_element_count();
                if ac != bc {
                    return ac.cmp(&bc);
                }
                a.0.as_str().cmp(b.0.as_str())
            };
            scratch.sort_by(pair_cmp);
        }

        let has_root_identity = canonicalize(&mut scratch);

        let composed_offset = self.offset.compose(&inner.offset);

        MapFunction {
            pairs: scratch,
            has_root_identity,
            offset: composed_offset,
        }
    }

    /// Composes this map function over a hypothetical map function that has
    /// an identity path mapping and the given offset.
    pub fn compose_offset(&self, new_offset: &LayerOffset) -> MapFunction {
        MapFunction {
            pairs: self.pairs.clone(),
            has_root_identity: self.has_root_identity,
            offset: self.offset.compose(new_offset),
        }
    }

    /// Returns the inverse of this map function.
    ///
    /// Matches C++ `GetInverse()`. Swaps source/target, sorts, and inverts offset.
    pub fn inverse(&self) -> MapFunction {
        if self.is_null() {
            return MapFunction::null();
        }

        // Swap source and target in all pairs
        let mut inverted_pairs: Vec<PathPair> = self
            .pairs
            .iter()
            .map(|(source, target)| (target.clone(), source.clone()))
            .collect();

        // Re-sort since target ordering != source ordering
        sort_pairs(&mut inverted_pairs);

        MapFunction {
            pairs: inverted_pairs,
            has_root_identity: self.has_root_identity,
            offset: self.offset.inverse(),
        }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Returns the set of path mappings, from source to target.
    pub fn source_to_target_map(&self) -> PathMap {
        let mut map = PathMap::new();
        if self.has_root_identity {
            map.insert(Path::absolute_root(), Path::absolute_root());
        }
        for (source, target) in &self.pairs {
            map.insert(source.clone(), target.clone());
        }
        map
    }

    /// Returns the set of path mappings, from target to source.
    pub fn target_to_source_map(&self) -> PathMap {
        self.inverse().source_to_target_map()
    }

    /// Returns a string representation of this mapping for debugging purposes.
    pub fn debug_string(&self) -> String {
        let mut lines: Vec<String> = Vec::new();

        if !self.offset.is_identity() {
            lines.push(format!(
                "offset: scale={}, offset={}",
                self.offset.scale(),
                self.offset.offset()
            ));
        }

        let source_to_target = self.source_to_target_map();
        // Sort by path string for deterministic output
        let mut sorted: Vec<(&Path, &Path)> = source_to_target.iter().collect();
        sorted.sort_by_key(|(k, _)| k.as_str());
        for (source, target) in sorted {
            lines.push(format!("{} -> {}", source.as_str(), target.as_str()));
        }

        lines.join("\n")
    }

    /// Swaps the contents of this map function with another.
    pub fn swap(&mut self, other: &mut MapFunction) {
        std::mem::swap(self, other);
    }
}

impl PartialEq for MapFunction {
    fn eq(&self, other: &Self) -> bool {
        self.has_root_identity == other.has_root_identity
            && self.offset == other.offset
            && self.pairs == other.pairs
    }
}

impl Eq for MapFunction {}

impl Hash for MapFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.has_root_identity.hash(state);
        self.pairs.len().hash(state);
        for (source, target) in &self.pairs {
            source.hash(state);
            target.hash(state);
        }
        self.offset.scale().to_bits().hash(state);
        self.offset.offset().to_bits().hash(state);
    }
}

impl fmt::Debug for MapFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapFunction")
            .field("pairs", &self.pairs)
            .field("has_root_identity", &self.has_root_identity)
            .field("offset", &self.offset)
            .finish()
    }
}

impl fmt::Display for MapFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.debug_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null() {
        let null = MapFunction::null();
        assert!(null.is_null());
        assert!(!null.is_identity());

        let path = Path::from_string("/Test").unwrap();
        assert!(null.map_source_to_target(&path).is_none());
    }

    #[test]
    fn test_identity() {
        let identity = MapFunction::identity();
        assert!(identity.is_identity());
        assert!(!identity.is_null());
        assert!(identity.is_identity_path_mapping());
        assert!(identity.has_root_identity());

        let path = Path::from_string("/Test/Mesh").unwrap();
        let mapped = identity.map_source_to_target(&path);
        assert_eq!(mapped, Some(path));
    }

    #[test]
    fn test_create_simple() {
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/Model").unwrap(),
            Path::from_string("/World/Model_1").unwrap(),
        );

        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();
        assert!(!map_fn.is_null());
        assert!(!map_fn.is_identity());

        // Map exact match
        let source = Path::from_string("/Model").unwrap();
        let target = map_fn.map_source_to_target(&source);
        assert_eq!(target, Path::from_string("/World/Model_1"));

        // Map child path
        let source_child = Path::from_string("/Model/Mesh").unwrap();
        let target_child = map_fn.map_source_to_target(&source_child);
        assert_eq!(target_child, Path::from_string("/World/Model_1/Mesh"));
    }

    #[test]
    fn test_inverse() {
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );

        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();
        let inverse = map_fn.inverse();

        let a = Path::from_string("/A").unwrap();
        let b = Path::from_string("/B").unwrap();

        assert_eq!(map_fn.map_source_to_target(&a), Some(b.clone()));
        assert_eq!(inverse.map_source_to_target(&b), Some(a));
    }

    #[test]
    fn test_compose() {
        // f: /A -> /B
        let mut map1 = PathMap::new();
        map1.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let f = MapFunction::create(map1, LayerOffset::identity()).unwrap();

        // g: /B -> /C
        let mut map2 = PathMap::new();
        map2.insert(
            Path::from_string("/B").unwrap(),
            Path::from_string("/C").unwrap(),
        );
        let g = MapFunction::create(map2, LayerOffset::identity()).unwrap();

        // g.compose(f) should give /A -> /C
        let composed = g.compose(&f);
        let a = Path::from_string("/A").unwrap();
        let c = Path::from_string("/C").unwrap();
        assert_eq!(composed.map_source_to_target(&a), Some(c));
    }

    #[test]
    fn test_compose_with_identity() {
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let f = MapFunction::create(path_map, LayerOffset::identity()).unwrap();
        let identity = MapFunction::identity();

        // identity.compose(f) == f
        let composed1 = identity.compose(&f);
        assert_eq!(
            composed1.map_source_to_target(&Path::from_string("/A").unwrap()),
            f.map_source_to_target(&Path::from_string("/A").unwrap())
        );

        // f.compose(identity) == f
        let composed2 = f.compose(identity);
        assert_eq!(
            composed2.map_source_to_target(&Path::from_string("/A").unwrap()),
            f.map_source_to_target(&Path::from_string("/A").unwrap())
        );
    }

    #[test]
    fn test_bijectivity_check() {
        // From C++ comment bug 74847:
        // { / -> /, /_class_Model -> /Model }
        // Mapping /Model should fail (noninvertible):
        //   source->target: /Model -> /Model (identity)
        //   target->source: /Model -> /_class_Model
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());
        path_map.insert(
            Path::from_string("/_class_Model").unwrap(),
            Path::from_string("/Model").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        // /Model should NOT map (noninvertible)
        let model = Path::from_string("/Model").unwrap();
        assert!(
            map_fn.map_source_to_target(&model).is_none(),
            "/Model should not map due to bijectivity check"
        );

        // /_class_Model/Child should map to /Model/Child
        let class_child = Path::from_string("/_class_Model/Child").unwrap();
        assert_eq!(
            map_fn.map_source_to_target(&class_child),
            Path::from_string("/Model/Child")
        );
    }

    #[test]
    fn test_bijectivity_check2() {
        // From C++ comment bug 112645:
        // { /A -> /B/B } -- /A/B should map to /A/B/B (invertible)
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B/B").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        let source = Path::from_string("/A/B").unwrap();
        let result = map_fn.map_source_to_target(&source);
        // /A/B -> /B/B/B (invertible since /B/B/B -> /A/B via inverse)
        assert_eq!(result, Path::from_string("/B/B/B"));
    }

    #[test]
    fn test_time_offset() {
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());

        let offset = LayerOffset::new(10.0, 2.0);
        let map_fn = MapFunction::create(path_map, offset).unwrap();

        assert_eq!(map_fn.time_offset().scale(), 2.0);
        assert_eq!(map_fn.time_offset().offset(), 10.0);
    }

    #[test]
    fn test_compose_offset() {
        let identity = MapFunction::identity().clone();
        let offset = LayerOffset::new(5.0, 2.0);
        let composed = identity.compose_offset(&offset);

        assert!(composed.is_identity_path_mapping());
        assert!(!composed.is_identity());
        assert_eq!(composed.time_offset().scale(), 2.0);
        assert_eq!(composed.time_offset().offset(), 5.0);
    }

    #[test]
    fn test_swap() {
        let mut map1 = MapFunction::null();
        let mut map2 = MapFunction::identity().clone();

        map1.swap(&mut map2);

        assert!(map1.is_identity());
        assert!(map2.is_null());
    }

    #[test]
    fn test_equality() {
        let identity1 = MapFunction::identity();
        let identity2 = MapFunction::identity();
        assert_eq!(identity1, identity2);

        let null1 = MapFunction::null();
        let null2 = MapFunction::null();
        assert_eq!(null1, null2);

        assert_ne!(identity1, &null1);
    }

    #[test]
    fn test_source_to_target_map() {
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        path_map.insert(Path::absolute_root(), Path::absolute_root());

        let map_fn = MapFunction::create(path_map.clone(), LayerOffset::identity()).unwrap();
        let retrieved_map = map_fn.source_to_target_map();

        assert!(retrieved_map.contains_key(&Path::absolute_root()));
        assert!(retrieved_map.contains_key(&Path::from_string("/A").unwrap()));
    }

    #[test]
    fn test_map_target_to_source() {
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/Model").unwrap(),
            Path::from_string("/World/Model_1").unwrap(),
        );

        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        // /World/Model_1/Mesh -> /Model/Mesh
        let target = Path::from_string("/World/Model_1/Mesh").unwrap();
        let source = map_fn.map_target_to_source(&target);
        assert_eq!(source, Path::from_string("/Model/Mesh"));
    }

    #[test]
    fn test_root_identity_mapping() {
        // With root identity only, every absolute path maps to itself
        let map_fn = MapFunction::identity().clone();
        let path = Path::from_string("/Any/Path/Here").unwrap();
        assert_eq!(map_fn.map_source_to_target(&path), Some(path));
    }

    // =========================================================================
    // Tests for new / fixed APIs (bijection check, compose, bijectivity)
    // =========================================================================

    /// Non-bijective mapping: `_HasBetterTargetMatch` blocks ambiguous paths.
    ///
    /// Setup: { / -> /, /_class_Model -> /Model }
    ///   /Model maps via root-identity (target elem count = 0), but /_class_Model -> /Model
    ///   gives target elem count 1 > 0 on the inverse side, so the bijection check
    ///   blocks the mapping and returns None.
    #[test]
    fn test_map_path_bijection_check_blocks_non_invertible() {
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());
        path_map.insert(
            Path::from_string("/_class_Model").unwrap(),
            Path::from_string("/Model").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        // /_class_Model/Mesh -> /Model/Mesh via the explicit pair: OK
        assert_eq!(
            map_fn.map_source_to_target(&Path::from_string("/_class_Model/Mesh").unwrap()),
            Path::from_string("/Model/Mesh"),
            "explicit pair child must map correctly"
        );

        // /Model via root-identity would give /Model (target elem count = 0),
        // but /_class_Model -> /Model has target elem count 1 > 0.
        // _HasBetterTargetMatch returns true => None.
        assert_eq!(
            map_fn.map_source_to_target(&Path::from_string("/Model").unwrap()),
            None,
            "non-bijective path must not map (bijection check blocks it)"
        );
    }

    /// compose() with overlapping / chained pairs.
    ///
    /// f: / -> /, /A -> /B, /C -> /D
    /// g: / -> /, /B -> /E
    /// g.compose(f) must give: /A -> /E  (chain), /C -> /D (unchanged)
    #[test]
    fn test_compose_overlapping_pairs() {
        let mut map_f = PathMap::new();
        map_f.insert(Path::absolute_root(), Path::absolute_root());
        map_f.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        map_f.insert(
            Path::from_string("/C").unwrap(),
            Path::from_string("/D").unwrap(),
        );
        let f = MapFunction::create(map_f, LayerOffset::identity()).unwrap();

        let mut map_g = PathMap::new();
        map_g.insert(Path::absolute_root(), Path::absolute_root());
        map_g.insert(
            Path::from_string("/B").unwrap(),
            Path::from_string("/E").unwrap(),
        );
        let g = MapFunction::create(map_g, LayerOffset::identity()).unwrap();

        let composed = g.compose(&f);

        assert_eq!(
            composed.map_source_to_target(&Path::from_string("/A").unwrap()),
            Path::from_string("/E"),
            "/A should chain through f->/B then g->/E"
        );
        assert_eq!(
            composed.map_source_to_target(&Path::from_string("/A/child").unwrap()),
            Path::from_string("/E/child"),
            "/A/child should follow same chain"
        );
        assert_eq!(
            composed.map_source_to_target(&Path::from_string("/C").unwrap()),
            Path::from_string("/D"),
            "/C should remain /D (g has no rule for /D)"
        );
    }

    /// Round-trip via inverse: source->target->source must be identity.
    #[test]
    fn test_compose_round_trip_via_inverse() {
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());
        path_map.insert(
            Path::from_string("/Model").unwrap(),
            Path::from_string("/World/Model_1").unwrap(),
        );
        let fwd = MapFunction::create(path_map, LayerOffset::identity()).unwrap();
        let inv = fwd.inverse();

        let source = Path::from_string("/Model/Geom").unwrap();
        let target = fwd
            .map_source_to_target(&source)
            .expect("forward map failed");
        let back = inv
            .map_source_to_target(&target)
            .expect("inverse map failed");
        assert_eq!(
            back, source,
            "round-trip source->target->source must be identity"
        );
    }

    /// compose() time offsets: scale = outer.scale * inner.scale, offset = outer.offset + outer.scale * inner.offset.
    #[test]
    fn test_compose_time_offsets() {
        let mut map1 = PathMap::new();
        map1.insert(Path::absolute_root(), Path::absolute_root());
        // f: offset=10, scale=2
        let f = MapFunction::create(map1.clone(), LayerOffset::new(10.0, 2.0)).unwrap();
        // g: offset=5, scale=3
        let g = MapFunction::create(map1, LayerOffset::new(5.0, 3.0)).unwrap();

        // g.compose(f): scale = 3*2 = 6, offset = 5 + 3*10 = 35
        let composed = g.compose(&f);
        let off = composed.time_offset();
        assert!(
            (off.scale() - 6.0).abs() < 1e-9,
            "composed scale must be 6.0"
        );
        assert!(
            (off.offset() - 35.0).abs() < 1e-9,
            "composed offset must be 35.0"
        );
    }

    /// map_target_to_source is the right inverse of map_source_to_target.
    #[test]
    fn test_map_target_to_source_is_right_inverse() {
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());
        path_map.insert(
            Path::from_string("/Src").unwrap(),
            Path::from_string("/Dst").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        let dst = Path::from_string("/Dst/child").unwrap();
        let src = map_fn
            .map_target_to_source(&dst)
            .expect("target_to_source failed");
        assert_eq!(src, Path::from_string("/Src/child").unwrap());

        let dst2 = map_fn
            .map_source_to_target(&src)
            .expect("round-trip failed");
        assert_eq!(dst2, dst);
    }

    /// Composing with null always yields null.
    #[test]
    fn test_null_compose_gives_null() {
        let null = MapFunction::null();
        let mut path_map = PathMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let f = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        assert!(null.compose(&f).is_null(), "null.compose(f) must be null");
        assert!(f.compose(&null).is_null(), "f.compose(null) must be null");
        assert!(
            null.compose(&null).is_null(),
            "null.compose(null) must be null"
        );
    }

    /// Longest-prefix match: more specific pair beats root identity.
    #[test]
    fn test_longest_prefix_match_wins() {
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());
        path_map.insert(
            Path::from_string("/World").unwrap(),
            Path::from_string("/Scene").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        assert_eq!(
            map_fn.map_source_to_target(&Path::from_string("/World/Cube").unwrap()),
            Path::from_string("/Scene/Cube")
        );
        assert_eq!(
            map_fn.map_source_to_target(&Path::from_string("/Other").unwrap()),
            Path::from_string("/Other")
        );
    }

    /// target_to_source_map() is the exact inverse of source_to_target_map().
    #[test]
    fn test_target_to_source_map_is_inverse() {
        let mut path_map = PathMap::new();
        path_map.insert(Path::absolute_root(), Path::absolute_root());
        path_map.insert(
            Path::from_string("/X").unwrap(),
            Path::from_string("/Y").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();
        let t2s = map_fn.target_to_source_map();
        assert_eq!(
            t2s.get(&Path::from_string("/Y").unwrap()),
            Some(&Path::from_string("/X").unwrap())
        );
        assert_eq!(
            t2s.get(&Path::absolute_root()),
            Some(&Path::absolute_root())
        );
    }

    // =========================================================================
    // Tests ported from C++ testPcpMapFunction.py
    // =========================================================================

    /// Helper to create a MapFunction from path string pairs.
    fn make_map(pairs: &[(&str, &str)]) -> MapFunction {
        let mut path_map = PathMap::new();
        for (src, tgt) in pairs {
            let source = Path::from_string(src).unwrap();
            let target = if tgt.is_empty() {
                Path::empty()
            } else {
                Path::from_string(tgt).unwrap()
            };
            path_map.insert(source, target);
        }
        MapFunction::create(path_map, LayerOffset::identity()).unwrap()
    }

    fn make_map_with_offset(pairs: &[(&str, &str)], offset: LayerOffset) -> MapFunction {
        let mut path_map = PathMap::new();
        for (src, tgt) in pairs {
            let source = Path::from_string(src).unwrap();
            let target = if tgt.is_empty() {
                Path::empty()
            } else {
                Path::from_string(tgt).unwrap()
            };
            path_map.insert(source, target);
        }
        MapFunction::create(path_map, offset).unwrap()
    }

    fn p(s: &str) -> Path {
        if s.is_empty() {
            Path::empty()
        } else {
            Path::from_string(s).unwrap()
        }
    }

    fn map_s2t(f: &MapFunction, s: &str) -> Option<Path> {
        f.map_source_to_target(&p(s))
    }

    fn map_t2s(f: &MapFunction, s: &str) -> Option<Path> {
        f.map_target_to_source(&p(s))
    }

    /// Port of test_Null from testPcpMapFunction.py
    #[test]
    fn test_py_null() {
        let null = MapFunction::null();
        assert!(null.is_null());
        assert!(!null.is_identity());
        assert!(!null.is_identity_path_mapping());
        assert!(null.time_offset().is_identity());

        for path_str in ["", "/", "/foo", "/a/b/c"] {
            if path_str.is_empty() {
                assert!(null.map_source_to_target(&Path::empty()).is_none());
            } else {
                assert!(map_s2t(&null, path_str).is_none());
            }
        }
    }

    /// Port of test_Identity from testPcpMapFunction.py
    #[test]
    fn test_py_identity() {
        let identity = MapFunction::identity();
        assert!(!identity.is_null());
        assert!(identity.is_identity());
        assert!(identity.is_identity_path_mapping());
        assert!(identity.time_offset().is_identity());

        for path_str in ["/", "/foo", "/a/b/c"] {
            assert_eq!(map_s2t(identity, path_str), Some(p(path_str)));
        }

        // Identity path mapping with non-identity offset
        let identity_with_offset = make_map_with_offset(&[("/", "/")], LayerOffset::new(0.0, 10.0));
        assert!(!identity_with_offset.is_null());
        assert!(!identity_with_offset.is_identity());
        assert!(identity_with_offset.is_identity_path_mapping());
        assert_eq!(identity_with_offset.time_offset().scale(), 10.0);
        for path_str in ["/", "/foo", "/a/b/c"] {
            assert_eq!(map_s2t(&identity_with_offset, path_str), Some(p(path_str)));
        }
    }

    /// Port of test_Simple from testPcpMapFunction.py
    #[test]
    fn test_py_simple() {
        let m = make_map(&[("/Model", "/Model_1")]);
        assert!(!m.is_null());
        assert!(!m.is_identity());
        assert!(!m.is_identity_path_mapping());

        // Forward mapping
        assert!(map_s2t(&m, "/").is_none());
        assert_eq!(map_s2t(&m, "/Model"), Some(p("/Model_1")));
        assert_eq!(map_s2t(&m, "/Model/anim"), Some(p("/Model_1/anim")));
        assert!(map_s2t(&m, "/Model_1").is_none());
        assert!(map_s2t(&m, "/Model_1/anim").is_none());

        // Reverse mapping
        assert!(map_t2s(&m, "/").is_none());
        assert!(map_t2s(&m, "/Model").is_none());
        assert!(map_t2s(&m, "/Model/anim").is_none());
        assert_eq!(map_t2s(&m, "/Model_1"), Some(p("/Model")));
        assert_eq!(map_t2s(&m, "/Model_1/anim"), Some(p("/Model/anim")));
    }

    /// Port of test_NestedRef from testPcpMapFunction.py
    #[test]
    fn test_py_nested_ref() {
        let m2 = make_map(&[("/CharRig", "/Model/Rig")]);
        assert!(!m2.is_null());
        assert!(!m2.is_identity());

        // Forward
        assert!(map_s2t(&m2, "/").is_none());
        assert_eq!(map_s2t(&m2, "/CharRig"), Some(p("/Model/Rig")));
        assert_eq!(map_s2t(&m2, "/CharRig/rig"), Some(p("/Model/Rig/rig")));
        assert!(map_s2t(&m2, "/Model").is_none());
        assert!(map_s2t(&m2, "/Model/Rig").is_none());
        assert!(map_s2t(&m2, "/Model/Rig/rig").is_none());

        // Reverse
        assert!(map_t2s(&m2, "/").is_none());
        assert!(map_t2s(&m2, "/CharRig").is_none());
        assert!(map_t2s(&m2, "/CharRig/rig").is_none());
        assert!(map_t2s(&m2, "/Model").is_none());
        assert_eq!(map_t2s(&m2, "/Model/Rig"), Some(p("/CharRig")));
        assert_eq!(map_t2s(&m2, "/Model/Rig/rig"), Some(p("/CharRig/rig")));
    }

    /// Port of test_Composition from testPcpMapFunction.py
    #[test]
    fn test_py_composition() {
        let m = make_map(&[("/Model", "/Model_1")]);
        let m2 = make_map(&[("/CharRig", "/Model/Rig")]);
        let m3 = m.compose(&m2);

        assert!(!m3.is_null());
        assert!(!m3.is_identity());

        // Forward
        assert!(map_s2t(&m3, "/").is_none());
        assert_eq!(map_s2t(&m3, "/CharRig"), Some(p("/Model_1/Rig")));
        assert_eq!(map_s2t(&m3, "/CharRig/rig"), Some(p("/Model_1/Rig/rig")));
        assert!(map_s2t(&m3, "/Model").is_none());
        assert!(map_s2t(&m3, "/Model/Rig").is_none());
        assert!(map_s2t(&m3, "/Model_1").is_none());
        assert!(map_s2t(&m3, "/Model_1/Rig").is_none());

        // Reverse
        assert!(map_t2s(&m3, "/").is_none());
        assert!(map_t2s(&m3, "/CharRig").is_none());
        assert!(map_t2s(&m3, "/Model").is_none());
        assert!(map_t2s(&m3, "/Model_1").is_none());
        assert_eq!(map_t2s(&m3, "/Model_1/Rig"), Some(p("/CharRig")));
        assert_eq!(map_t2s(&m3, "/Model_1/Rig/rig"), Some(p("/CharRig/rig")));

        // Compose that should produce identity
        let m_ab = make_map(&[("/", "/"), ("/a", "/b")]);
        let m_ba = make_map(&[("/", "/"), ("/b", "/a")]);
        assert_eq!(m_ab.compose(&m_ba), *MapFunction::identity());
        assert_eq!(m_ba.compose(&m_ab), *MapFunction::identity());
    }

    /// Port of test_InheritRelocateChain from testPcpMapFunction.py
    #[test]
    fn test_py_inherit_relocate_chain() {
        // Chain: instance -> relocate -> class -> reverse instance
        let m4 = make_map(&[("/M", "/M_1")]).compose(
            &make_map(&[("/M/Rig/Inst/Scope", "/M/Anim/Scope")]).compose(
                &make_map(&[("/M/Rig/Class", "/M/Rig/Inst")]).compose(&make_map(&[("/M_1", "/M")])),
            ),
        );

        let expected = make_map(&[("/M_1/Rig/Class/Scope", "/M_1/Anim/Scope")]);
        assert_eq!(m4, expected);

        assert_eq!(
            map_s2t(&m4, "/M_1/Rig/Class/Scope/x"),
            Some(p("/M_1/Anim/Scope/x"))
        );
        assert_eq!(
            map_t2s(&m4, "/M_1/Anim/Scope/x"),
            Some(p("/M_1/Rig/Class/Scope/x"))
        );
    }

    /// Port of test_LayerOffsets from testPcpMapFunction.py
    #[test]
    fn test_py_layer_offsets() {
        let offset1 = LayerOffset::new(0.0, 2.0);
        let offset2 = LayerOffset::new(10.0, 1.0);
        let m5 = make_map_with_offset(&[("/", "/")], offset1);
        let m6 = make_map_with_offset(&[("/", "/")], offset2);

        assert_eq!(m5.time_offset().scale(), offset1.scale());
        assert_eq!(m5.time_offset().offset(), offset1.offset());
        assert_eq!(m6.time_offset().scale(), offset2.scale());
        assert_eq!(m6.time_offset().offset(), offset2.offset());

        let composed = m5.compose(&m6);
        let expected_offset = offset1.compose(&offset2);
        assert_eq!(composed.time_offset().scale(), expected_offset.scale());
        assert_eq!(composed.time_offset().offset(), expected_offset.offset());

        let compose_offset_result = m5.compose_offset(&m6.time_offset());
        assert_eq!(
            compose_offset_result.time_offset().scale(),
            expected_offset.scale()
        );
        assert_eq!(
            compose_offset_result.time_offset().offset(),
            expected_offset.offset()
        );
    }

    /// Port of test_MapFunctionsWithBlocks from testPcpMapFunction.py
    #[test]
    fn test_py_blocks() {
        // Identity mapping with explicit block of /Model
        let f = make_map(&[("/", "/"), ("/Model", "")]);

        // Non /Model paths map to themselves
        assert_eq!(map_s2t(&f, "/foo"), Some(p("/foo")));
        assert_eq!(map_t2s(&f, "/foo"), Some(p("/foo")));
        assert_eq!(map_s2t(&f, "/foo/bar"), Some(p("/foo/bar")));
        assert_eq!(map_t2s(&f, "/foo/bar"), Some(p("/foo/bar")));

        // /Model and descendants do not map
        assert!(map_s2t(&f, "/Model").is_none());
        assert!(map_t2s(&f, "/Model").is_none());
        assert!(map_s2t(&f, "/Model/Bar").is_none());
        assert!(map_t2s(&f, "/Model/Bar").is_none());

        // Rename + block + move
        let f2 = make_map(&[
            ("/CharRig", "/Char"),
            ("/CharRig/Rig", ""),
            ("/CharRig/Rig/Anim", "/Char/Anim"),
        ]);

        // Forward: CharRig -> Char
        assert_eq!(map_s2t(&f2, "/CharRig"), Some(p("/Char")));
        assert_eq!(map_s2t(&f2, "/CharRig/Foo"), Some(p("/Char/Foo")));
        // Forward: /CharRig/Rig blocked
        assert!(map_s2t(&f2, "/CharRig/Rig").is_none());
        assert!(map_s2t(&f2, "/CharRig/Rig/Foo").is_none());
        // Forward: /CharRig/Rig/Anim -> /Char/Anim
        assert_eq!(map_s2t(&f2, "/CharRig/Rig/Anim"), Some(p("/Char/Anim")));
        assert_eq!(
            map_s2t(&f2, "/CharRig/Rig/Anim/Foo"),
            Some(p("/Char/Anim/Foo"))
        );

        // Reverse: /Char -> /CharRig
        assert_eq!(map_t2s(&f2, "/Char"), Some(p("/CharRig")));
        assert_eq!(map_t2s(&f2, "/Char/Foo"), Some(p("/CharRig/Foo")));
        // Reverse: /Char/Rig fails (block prevents forward mapping to /Char/Rig)
        assert!(map_t2s(&f2, "/Char/Rig").is_none());
        assert!(map_t2s(&f2, "/Char/Rig/Foo").is_none());
        assert!(map_t2s(&f2, "/Char/Rig/Anim").is_none());
        // Reverse: /Char/Anim -> /CharRig/Rig/Anim
        assert_eq!(map_t2s(&f2, "/Char/Anim"), Some(p("/CharRig/Rig/Anim")));
        assert_eq!(
            map_t2s(&f2, "/Char/Anim/Foo"),
            Some(p("/CharRig/Rig/Anim/Foo"))
        );
    }

    /// Port of test_Canonicalization from testPcpMapFunction.py
    #[test]
    fn test_py_canonicalization() {
        // Empty mapping
        let f0 = make_map(&[]);
        assert!(f0.source_to_target_map().is_empty());

        // /A -> /A stays
        let f1 = make_map(&[("/A", "/A")]);
        let m1 = f1.source_to_target_map();
        assert_eq!(m1.len(), 1);
        assert_eq!(m1.get(&p("/A")), Some(&p("/A")));

        // / -> / makes /A -> /A redundant
        let f2 = make_map(&[("/", "/"), ("/A", "/A")]);
        let m2 = f2.source_to_target_map();
        assert_eq!(m2.len(), 1);
        assert!(m2.contains_key(&p("/")));

        // / -> / does NOT make /A -> /B redundant
        let f3 = make_map(&[("/", "/"), ("/A", "/B")]);
        let m3 = f3.source_to_target_map();
        assert_eq!(m3.len(), 2);
        assert!(m3.contains_key(&p("/")));
        assert_eq!(m3.get(&p("/A")), Some(&p("/B")));

        // /A -> /B makes /A/X -> /B/X redundant, but /A/X/Y1 -> /B/X/Y2 is not
        let f4 = make_map(&[("/A", "/B"), ("/A/X", "/B/X"), ("/A/X/Y1", "/B/X/Y2")]);
        let m4 = f4.source_to_target_map();
        assert_eq!(m4.len(), 2);
        assert_eq!(m4.get(&p("/A")), Some(&p("/B")));
        assert_eq!(m4.get(&p("/A/X/Y1")), Some(&p("/B/X/Y2")));

        // /A -> /B makes /A/X1/C -> /B/X1/C redundant
        let f5 = make_map(&[("/A", "/B"), ("/A/X1/C", "/B/X1/C")]);
        let m5 = f5.source_to_target_map();
        assert_eq!(m5.len(), 1);
        assert_eq!(m5.get(&p("/A")), Some(&p("/B")));

        // Block /A -> empty with no other mappings is redundant
        let f6 = make_map(&[("/A", "")]);
        assert!(f6.source_to_target_map().is_empty());

        // / -> / with block /A -> empty is not redundant
        let f7 = make_map(&[("/", "/"), ("/A", "")]);
        let m7 = f7.source_to_target_map();
        assert_eq!(m7.len(), 2);
        assert_eq!(m7.get(&p("/")), Some(&p("/")));
        assert_eq!(m7.get(&p("/A")), Some(&Path::empty()));
    }

    /// Port of test_Bug74847 from testPcpMapFunction.py
    #[test]
    fn test_py_bug74847() {
        let m = make_map(&[("/A", "/A/B")]);
        assert_eq!(map_s2t(&m, "/A/B"), Some(p("/A/B/B")));
        assert_eq!(map_t2s(&m, "/A/B/B"), Some(p("/A/B")));
    }

    /// Port of test_Bug112645 from testPcpMapFunction.py
    #[test]
    fn test_py_bug112645() {
        let f1 = make_map(&[
            ("/GuitarRig", "/GuitarRigX"),
            (
                "/GuitarRig/Rig/StringsRig/_Class_StringRig/String",
                "/GuitarRigX/Anim/Strings/String1",
            ),
        ]);

        let f2 = make_map(&[
            (
                "/StringsRig/String1Rig/String",
                "/GuitarRig/Anim/Strings/String1",
            ),
            ("/StringsRig", "/GuitarRig/Rig/StringsRig"),
        ]);

        let composed = f1.compose(&f2);
        let expected = make_map(&[
            ("/StringsRig", "/GuitarRigX/Rig/StringsRig"),
            ("/StringsRig/String1Rig/String", ""),
            (
                "/StringsRig/_Class_StringRig/String",
                "/GuitarRigX/Anim/Strings/String1",
            ),
        ]);
        assert_eq!(composed, expected);
    }

    /// Port of test_BugComposedMapFunction from testPcpMapFunction.py
    #[test]
    fn test_py_bug_composed_map_function() {
        let f1 = make_map(&[
            ("/PathRig", "/CharRig/Rig/PathRig"),
            ("/PathRig/Path", "/Path"),
        ]);

        let f2 = make_map(&[("/CharRig", "/Model")]);

        let composed = f2.compose(&f1);

        let expected = make_map(&[("/PathRig", "/Model/Rig/PathRig"), ("/PathRig/Path", "")]);
        assert_eq!(composed, expected);

        // Verify composed function matches calling f2(f1(path))
        // /Bogus fails
        assert!(map_s2t(&composed, "/Bogus").is_none());

        // /PathRig -> /Model/Rig/PathRig
        assert_eq!(
            map_s2t(&composed, "/PathRig"),
            Some(p("/Model/Rig/PathRig"))
        );
        assert_eq!(
            map_t2s(&composed, "/Model/Rig/PathRig"),
            Some(p("/PathRig"))
        );

        // /PathRig/Rig -> /Model/Rig/PathRig/Rig
        assert_eq!(
            map_s2t(&composed, "/PathRig/Rig"),
            Some(p("/Model/Rig/PathRig/Rig"))
        );

        // /PathRig/Path fails (blocked by compose)
        assert!(map_s2t(&composed, "/PathRig/Path").is_none());

        // Inverse of blocked: /Model/Rig/PathRig/Path also fails
        assert!(map_t2s(&composed, "/Model/Rig/PathRig/Path").is_none());
    }

    /// Port of test_Basics equality matrix from testPcpMapFunction.py
    #[test]
    fn test_py_basics_equality() {
        let test_fns = vec![
            MapFunction::null(),
            MapFunction::identity().clone(),
            make_map_with_offset(&[("/", "/")], LayerOffset::new(0.0, 10.0)),
            make_map(&[("/Model", "/Model_1")]),
            make_map(&[("/CharRig", "/Model/Rig")]),
        ];

        // Equality/inequality matrix
        for i in 0..test_fns.len() {
            for j in 0..test_fns.len() {
                if i == j {
                    assert_eq!(test_fns[i], test_fns[j], "i={}, j={}", i, j);
                } else {
                    assert_ne!(test_fns[i], test_fns[j], "i={}, j={}", i, j);
                }
            }
        }

        // Composing with identity returns itself
        let identity = MapFunction::identity();
        for f in &test_fns {
            assert_eq!(f.compose(identity), f.clone());
            assert_eq!(identity.compose(f), f.clone());
        }
    }
}
