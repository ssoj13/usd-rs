
//! HdSceneIndexPrimView - depth-first iterator over scene index prims.
//!
//! Port of pxr/imaging/hd/sceneIndexPrimView.{h,cpp}

use super::{HdSceneIndexHandle, si_ref};
use usd_sdf::Path as SdfPath;

/// Iterator over all descendants of a prim in depth-first order.
///
/// Children are expanded lazily: a prim's children are only fetched from the
/// scene index when the iterator is about to descend into them.  Calling
/// `skip_descendants()` immediately after receiving a path prevents the
/// iterator from ever fetching or visiting that prim's children.
pub struct HdSceneIndexPrimView {
    input_scene_index: HdSceneIndexHandle,
    root: SdfPath,
}

impl HdSceneIndexPrimView {
    /// Create a view over the entire scene (from absolute root).
    pub fn new(scene_index: HdSceneIndexHandle) -> Self {
        Self::with_root(scene_index, SdfPath::absolute_root())
    }

    /// Create a view rooted at `root` (root itself is included as the first item).
    pub fn with_root(scene_index: HdSceneIndexHandle, root: SdfPath) -> Self {
        Self {
            input_scene_index: scene_index,
            root,
        }
    }

    /// Returns a depth-first iterator.
    pub fn iter(&self) -> HdSceneIndexPrimViewIter<'_> {
        HdSceneIndexPrimViewIter::new(&self.input_scene_index, self.root.clone())
    }
}

// ---------------------------------------------------------------------------
// Stack frame: a sibling list at a given depth, plus the current index.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct StackFrame {
    siblings: Vec<SdfPath>,
    index: usize,
}

impl StackFrame {
    fn current(&self) -> Option<&SdfPath> {
        self.siblings.get(self.index)
    }

    fn advance(&mut self) {
        self.index += 1;
    }
}

// ---------------------------------------------------------------------------
// HdSceneIndexPrimViewIter
// ---------------------------------------------------------------------------

/// Depth-first iterator with lazy child expansion.
///
/// The iterator maintains a stack of sibling lists.  On each call to `next()`:
/// 1. Return the current path.
/// 2. If `skip_descendants` is set, just advance the index; don't fetch children.
/// 3. Otherwise, fetch children of the returned path.  If there are children,
///    push them onto the stack (the current index is NOT advanced yet — it will
///    be advanced when we pop back up to this level).  If there are no children,
///    advance the current index.
///
/// This design means `skip_descendants()` correctly suppresses the child fetch
/// for the path that was just returned.
pub struct HdSceneIndexPrimViewIter<'a> {
    scene: &'a HdSceneIndexHandle,
    /// Stack of sibling lists; the top frame holds the siblings being visited.
    stack: Vec<StackFrame>,
    /// When true, the next call to `next()` will skip expanding the just-returned path's children.
    skip_descendants: bool,
    /// The path returned by the most recent `next()` call, whose children may
    /// need to be expanded on the NEXT call.
    last_path: Option<SdfPath>,
    /// Whether `last_path` has already been expanded (children pushed or skipped).
    expanded: bool,
}

impl<'a> HdSceneIndexPrimViewIter<'a> {
    fn new(scene: &'a HdSceneIndexHandle, root: SdfPath) -> Self {
        Self {
            scene,
            stack: vec![StackFrame {
                siblings: vec![root],
                index: 0,
            }],
            skip_descendants: false,
            last_path: None,
            expanded: true, // nothing to expand before first next()
        }
    }

    /// Prevent the iterator from visiting descendants of the most recently
    /// returned path.
    ///
    /// Must be called between consecutive `next()` calls.
    pub fn skip_descendants(&mut self) {
        self.skip_descendants = true;
    }

    /// Expand (or skip) the last returned path, then advance past it.
    fn expand_last(&mut self) {
        if self.expanded {
            return;
        }
        self.expanded = true;

        let path = match &self.last_path {
            Some(p) => p.clone(),
            None => return,
        };

        if self.skip_descendants {
            self.skip_descendants = false;
            // Just advance past the current item; don't push children.
            if let Some(frame) = self.stack.last_mut() {
                frame.advance();
            }
        } else {
            // Fetch children.
            let children = si_ref(&self.scene).get_child_prim_paths(&path);

            if children.is_empty() {
                // Leaf node: advance within current frame.
                if let Some(frame) = self.stack.last_mut() {
                    frame.advance();
                }
            } else {
                // Push children as a new frame; current index stays put
                // (it will be advanced when we pop this child frame later).
                self.stack.push(StackFrame {
                    siblings: children,
                    index: 0,
                });
            }
        }

        // Pop exhausted frames.
        loop {
            match self.stack.last() {
                None => break,
                Some(f) if f.index < f.siblings.len() => break,
                _ => {
                    self.stack.pop();
                    // Advance the parent frame's index now that we're done with its subtree.
                    if let Some(parent) = self.stack.last_mut() {
                        parent.advance();
                    }
                }
            }
        }
    }
}

impl<'a> Iterator for HdSceneIndexPrimViewIter<'a> {
    type Item = SdfPath;

    fn next(&mut self) -> Option<Self::Item> {
        // Expand (or skip) the previously returned path before moving on.
        self.expand_last();

        // Pick the current path from the top frame.
        let frame = self.stack.last()?;
        let path = frame.current()?.clone();

        self.last_path = Some(path.clone());
        self.expanded = false;

        Some(path)
    }
}
