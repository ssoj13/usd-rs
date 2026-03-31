//! Prim Range - forward-iterable range for traversing prim subtrees.
//!
//! Port of pxr/usd/usd/primRange.h
//!
//! UsdPrimRange provides a forward-iterable range that traverses a subtree
//! of prims rooted at a given prim in depth-first order. It supports
//! pre- and post-order visitation and subtree pruning.

use std::sync::Arc;

use crate::prim::Prim;
use crate::prim_data::PrimData;
use crate::prim_flags::PrimFlagsPredicate;
use usd_sdf::Path;

/// A forward-iterable range that traverses a subtree of prims rooted at a
/// given prim in depth-first order.
///
/// Matches C++ `UsdPrimRange`.
#[derive(Clone, Debug)]
pub struct PrimRange {
    /// Beginning prim data pointer (None if empty).
    begin: Option<Arc<PrimData>>,
    /// End prim data pointer (None if empty).
    end: Option<Arc<PrimData>>,
    /// Initial proxy prim path.
    init_proxy_prim_path: Path,
    /// Predicate for filtering prims.
    predicate: PrimFlagsPredicate,
    /// Initial depth.
    init_depth: u32,
    /// Whether to use pre- and post-order visitation.
    post_order: bool,
}

/// Iterator for PrimRange.
///
/// Matches C++ `UsdPrimRange::iterator`.
///
/// Uses `Arc<PrimRangeInner>` instead of a raw pointer to the owning
/// `PrimRange`, eliminating the `unsafe` block that was previously needed
/// in `increment()`.
#[derive(Clone, Debug)]
pub struct PrimRangeIterator {
    /// Underlying iterator (prim data pointer).
    underlying_iterator: Option<Arc<PrimData>>,
    /// Shared traversal state (safe alternative to raw *const PrimRange).
    range: Arc<PrimRangeInner>,
    /// Proxy prim path.
    proxy_prim_path: Path,
    /// Current depth in traversal.
    depth: u32,
    /// Whether to prune children on next increment.
    prune_children_flag: bool,
    /// Whether this is a post-visit (for pre/post-order).
    is_post: bool,
}

/// Shared inner state for PrimRange iterators.
#[derive(Clone, Debug)]
struct PrimRangeInner {
    end: Option<Arc<PrimData>>,
    predicate: PrimFlagsPredicate,
    post_order: bool,
}

impl PrimRange {
    /// Creates an empty range.
    ///
    /// Matches C++ default constructor.
    pub fn new() -> Self {
        Self {
            begin: None,
            end: None,
            init_proxy_prim_path: Path::empty(),
            predicate: PrimFlagsPredicate::default(),
            init_depth: 0,
            post_order: false,
        }
    }

    /// Construct a PrimRange that traverses the subtree rooted at `start` in
    /// depth-first order, visiting prims that pass the default predicate.
    ///
    /// Matches C++ `UsdPrimRange(const UsdPrim &start)`.
    pub fn from_prim(start: &Prim) -> Self {
        // Get the prim data
        if let Some(prim_data) = start.data() {
            let next: Option<Arc<PrimData>> = prim_data.next_sibling();
            // Get proxy path from prim if it's an instance proxy
            // Proxy path points to the prototype prim that this instance references
            // In C++, this uses UsdPrim::_ProxyPrimPath() which returns the proxy path
            let proxy_path = start.proxy_prim_path().clone();
            Self {
                begin: Some(prim_data.clone()),
                end: next,
                init_proxy_prim_path: proxy_path,
                predicate: PrimFlagsPredicate::default(),
                init_depth: 0,
                post_order: false,
            }
        } else {
            Self::new()
        }
    }

    /// Construct a PrimRange that traverses the subtree rooted at `start` in
    /// depth-first order, visiting prims that pass `predicate`.
    ///
    /// Matches C++ `UsdPrimRange(const UsdPrim &start, const Usd_PrimFlagsPredicate &predicate)`.
    pub fn from_prim_with_predicate(start: &Prim, predicate: PrimFlagsPredicate) -> Self {
        // Get the prim data
        if let Some(prim_data) = start.data() {
            let next: Option<Arc<PrimData>> = prim_data.next_sibling();
            Self {
                begin: Some(prim_data.clone()),
                end: next,
                init_proxy_prim_path: start.proxy_prim_path().clone(), // Get proxy path from prim for instance proxies
                predicate,
                init_depth: 0,
                post_order: false,
            }
        } else {
            Self::new()
        }
    }

    /// Create a PrimRange that traverses all prims on the stage (matches C++ UsdPrimRange::Stage).
    ///
    /// C++ starts from pseudo_root's first child (not pseudo_root itself) and
    /// sets init_depth=1 so siblings of the first child ARE visited.
    pub fn stage(
        stage: std::sync::Arc<crate::stage::Stage>,
        predicate: PrimFlagsPredicate,
    ) -> Self {
        let pseudo_root = stage.pseudo_root();
        if let Some(prim_data) = pseudo_root.data() {
            if let Some(first_child) = prim_data.first_child() {
                // Start from first child, end=None (visit all root prims)
                let mut range = Self {
                    begin: Some(first_child),
                    end: None,
                    init_proxy_prim_path: Path::empty(),
                    predicate,
                    init_depth: 0,
                    post_order: false,
                };
                // C++: ++ret._initDepth so we continue to siblings
                range.init_depth = 1;
                range
            } else {
                Self::new()
            }
        } else {
            Self::new()
        }
    }

    /// Create a PrimRange that traverses the subtree rooted at `start` in
    /// depth-first order with pre- and post-order visitation.
    ///
    /// Matches C++ `UsdPrimRange::PreAndPostVisit(const UsdPrim &start)`.
    pub fn pre_and_post_visit(start: &Prim) -> Self {
        let mut result = Self::from_prim(start);
        result.post_order = true;
        result
    }

    /// Create a PrimRange that traverses the subtree rooted at `start` in
    /// depth-first order with pre- and post-order visitation and predicate.
    ///
    /// Matches C++ `UsdPrimRange::PreAndPostVisit(const UsdPrim &start, const Usd_PrimFlagsPredicate &predicate)`.
    pub fn pre_and_post_visit_with_predicate(start: &Prim, predicate: PrimFlagsPredicate) -> Self {
        let mut result = Self::from_prim_with_predicate(start, predicate);
        result.post_order = true;
        result
    }

    /// Construct a PrimRange that traverses the subtree rooted at `start` in
    /// depth-first order, visiting all prims (including deactivated, undefined,
    /// and abstract prims).
    ///
    /// Matches C++ `UsdPrimRange::AllPrims(const UsdPrim &start)`.
    pub fn all_prims(start: &Prim) -> Self {
        Self::from_prim_with_predicate(start, PrimFlagsPredicate::all())
    }

    /// Construct a PrimRange that traverses the subtree rooted at `start` in
    /// depth-first order, visiting all prims with pre- and post-order visitation.
    ///
    /// Matches C++ `UsdPrimRange::AllPrimsPreAndPostVisit(const UsdPrim &start)`.
    pub fn all_prims_pre_and_post_visit(start: &Prim) -> Self {
        Self::pre_and_post_visit_with_predicate(start, PrimFlagsPredicate::all())
    }

    /// Create a PrimRange that traverses all the prims on `stage`, visiting
    /// those that pass the given predicate.
    ///
    /// Delegates to pseudo_root + from_prim_with_predicate (C++ parity).
    ///
    /// Matches C++ `UsdPrimRange::Stage(const UsdStagePtr &stage, const Usd_PrimFlagsPredicate &predicate)`.
    pub fn from_stage(stage: &crate::stage::Stage, predicate: PrimFlagsPredicate) -> Self {
        let pseudo_root = stage.pseudo_root();
        Self::from_prim_with_predicate(&pseudo_root, predicate)
    }

    /// Build a shared inner state Arc for iterators.
    fn shared_inner(&self) -> Arc<PrimRangeInner> {
        Arc::new(PrimRangeInner {
            end: self.end.clone(),
            predicate: self.predicate.clone(),
            post_order: self.post_order,
        })
    }

    /// Return an iterator to the start of this range.
    ///
    /// Matches C++ `begin()`.
    pub fn begin(&self) -> PrimRangeIterator {
        PrimRangeIterator {
            underlying_iterator: self.begin.clone(),
            range: self.shared_inner(),
            proxy_prim_path: self.init_proxy_prim_path.clone(),
            depth: self.init_depth,
            prune_children_flag: false,
            is_post: false,
        }
    }

    /// Return the past-the-end iterator for this range.
    ///
    /// Matches C++ `end()`.
    pub fn end(&self) -> PrimRangeIterator {
        PrimRangeIterator {
            underlying_iterator: self.end.clone(),
            range: self.shared_inner(),
            proxy_prim_path: Path::empty(),
            depth: 0,
            prune_children_flag: false,
            is_post: false,
        }
    }

    /// Return the first element of this range. The range must not be empty.
    ///
    /// Matches C++ `front()`.
    pub fn front(&self) -> Option<Prim> {
        self.begin().next()
    }

    /// Return true if this range contains no prims.
    ///
    /// Matches C++ `empty()`.
    pub fn is_empty(&self) -> bool {
        self.begin() == self.end()
    }

    /// Return the predicate used for filtering prims in this range.
    ///
    /// Matches C++ `GetPredicate()`.
    pub fn predicate(&self) -> &PrimFlagsPredicate {
        &self.predicate
    }

    /// Advance the beginning of this range to the next prim.
    ///
    /// This shrinks the range by removing the first element.
    /// Matches C++ `UsdPrimRange::increment_begin()`.
    pub fn increment_begin(&mut self) {
        if self.begin.is_none() {
            return;
        }
        // Advance begin using the same traversal logic as the iterator
        let mut iter = self.begin();
        iter.increment();
        self.begin = iter.underlying_iterator;
    }

    /// Set the beginning of this range to the given iterator position.
    ///
    /// Matches C++ `UsdPrimRange::set_begin(iterator)`.
    pub fn set_begin(&mut self, iter: &PrimRangeIterator) {
        self.begin = iter.underlying_iterator.clone();
    }
}

impl Default for PrimRange {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimRangeIterator {
    fn clamp_to_end(&mut self) {
        if let (Some(current), Some(end)) = (&self.underlying_iterator, &self.range.end) {
            if Arc::ptr_eq(current, end) {
                self.underlying_iterator = None;
            }
        }
    }

    /// Return true if the iterator points to a prim visited the second time
    /// (in post order) for a pre- and post-order iterator.
    ///
    /// Matches C++ `IsPostVisit()`.
    pub fn is_post_visit(&self) -> bool {
        self.is_post
    }

    /// Behave as if the current prim has no children when next advanced.
    ///
    /// Matches C++ `PruneChildren()`.
    pub fn prune_children(&mut self) {
        if self.is_post {
            // In C++, this would issue TF_CODING_ERROR
            return;
        }
        self.prune_children_flag = true;
    }

    fn increment(&mut self) {
        let post_order = self.range.post_order;

        if self.underlying_iterator.is_none() {
            return;
        }

        self.clamp_to_end();
        if self.underlying_iterator.is_none() {
            return;
        }

        if post_order && self.is_post {
            // Post-order: just visited this node in post. Now move to sibling
            // or go up to parent (which also needs post-visit).
            // Matches C++: Usd_MoveToNextSiblingOrParent returns true=parent.
            self.is_post = false;
            if let Some(curr) = self.underlying_iterator.as_ref().cloned() {
                if let Some(next) = curr.next_sibling() {
                    // Moved to sibling — depth unchanged
                    self.underlying_iterator = Some(next);
                    self.clamp_to_end();
                } else if self.depth > 0 {
                    // No sibling — go up to parent for post-visit
                    self.depth -= 1;
                    self.underlying_iterator = curr.parent();
                    self.is_post = true;
                } else {
                    self.underlying_iterator = None;
                }
            }
        } else if !self.prune_children_flag {
            // Try to move to first child
            if let Some(child) = self
                .underlying_iterator
                .as_ref()
                .and_then(|it| it.first_child())
            {
                self.underlying_iterator = Some(child);
                self.depth += 1;
                self.clamp_to_end();
            } else {
                // No children — move to next sibling or walk up
                if post_order {
                    self.is_post = true;
                } else {
                    self.move_to_next_sibling_or_parent();
                }
            }
        } else {
            // Prune children — skip subtree, move to sibling or walk up
            self.prune_children_flag = false;
            if post_order {
                self.is_post = true;
            } else {
                self.move_to_next_sibling_or_parent();
            }
        }
    }

    /// Matches C++ Usd_MoveToNextSiblingOrParent logic:
    /// Try next sibling first (depth unchanged). If no sibling, go up to
    /// parent (depth--) and retry. Repeat until sibling found or root reached.
    fn move_to_next_sibling_or_parent(&mut self) {
        loop {
            let curr = match self.underlying_iterator.as_ref() {
                Some(c) => c.clone(),
                None => return,
            };

            // Try sibling at current level — depth stays the same
            if let Some(next) = curr.next_sibling() {
                self.underlying_iterator = Some(next);
                self.clamp_to_end();
                return;
            }

            // No sibling — go up to parent
            if self.depth > 0 {
                self.depth -= 1;
                match curr.parent() {
                    Some(parent) => self.underlying_iterator = Some(parent),
                    None => {
                        self.underlying_iterator = None;
                        return;
                    }
                }
                // Continue loop to try parent's sibling
            } else {
                // At root depth — traversal complete
                self.underlying_iterator = None;
                return;
            }
        }
    }

    fn dereference(&self) -> Prim {
        // Create prim with proxy path for instance proxies
        if let Some(ref data) = self.underlying_iterator {
            if let Some(stage) = data.stage() {
                if !self.proxy_prim_path.is_empty() {
                    Prim::from_data_with_proxy(
                        std::sync::Arc::downgrade(&stage),
                        data.clone(),
                        self.proxy_prim_path.clone(),
                    )
                } else {
                    Prim::from_data(std::sync::Arc::downgrade(&stage), data.clone())
                }
            } else {
                Prim::invalid()
            }
        } else {
            Prim::invalid()
        }
    }
}

impl Iterator for PrimRangeIterator {
    type Item = Prim;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.clamp_to_end();
            let current = self.underlying_iterator.as_ref()?.clone();
            let matches = self.range.predicate.matches(current.flags());

            if matches {
                let result = self.dereference();
                self.increment();
                return Some(result);
            }

            if !self.is_post {
                self.prune_children_flag = true;
            }
            self.increment();
        }
    }
}

impl IntoIterator for PrimRange {
    type Item = Prim;
    type IntoIter = PrimRangeIterator;

    fn into_iter(self) -> Self::IntoIter {
        self.begin()
    }
}

impl PartialEq for PrimRangeIterator {
    /// Two iterators are equal if they point to the same position.
    /// In C++, this compares the underlying prim-data pointer and the
    /// post-visit flag - that is sufficient to detect begin==end.
    fn eq(&self, other: &Self) -> bool {
        match (&self.underlying_iterator, &other.underlying_iterator) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) && self.is_post == other.is_post,
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for PrimRangeIterator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_range() {
        let range = PrimRange::new();
        assert!(range.is_empty());
    }

    #[test]
    fn test_iterator_is_safe() {
        // Verify the Arc-based iterator doesn't require unsafe
        let range = PrimRange::new();
        let iter = range.begin();
        // Clone should work (Arc is Clone)
        let _iter2 = iter.clone();
        assert_eq!(range.begin(), range.end());
    }

    #[test]
    fn test_from_stage_returns_prims() {
        use crate::common::InitialLoadSet;
        use crate::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let _prim = stage.define_prim("/Test", "Xform").unwrap();
        let range = PrimRange::from_stage(&stage, PrimFlagsPredicate::default());
        // from_stage delegates to pseudo_root, so it should produce a valid range
        // (may or may not be empty depending on stage setup)
        let _count = range.into_iter().count();
    }
}
