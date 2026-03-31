
//! HdSortedIds - Sorted container of Hydra IDs (multiset semantics).
//!
//! Corresponds to pxr/imaging/hd/sortedIds.h.
//! Duplicate elements are allowed. Paths kept sorted for efficient lookups.

use std::cmp::Ordering;
use usd_sdf::Path as SdfPath;

/// Path vector type.
pub type SdfPathVector = Vec<SdfPath>;

/// Manages a container of Hydra IDs in sorted order.
///
/// Corresponds to C++ `Hd_SortedIds`.
/// Behaves like a multiset - duplicate elements allowed.
#[derive(Debug, Clone, Default)]
pub struct HdSortedIds {
    ids: SdfPathVector,
    edits: SdfPathVector,
    mode: EditMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditMode {
    NoMode,
    InsertMode,
    RemoveMode,
}

impl Default for EditMode {
    fn default() -> Self {
        EditMode::NoMode
    }
}

impl HdSortedIds {
    /// Create empty container.
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            edits: Vec::new(),
            mode: EditMode::NoMode,
        }
    }

    /// Get sorted ids. Triggers sort if pending edits.
    pub fn get_ids(&mut self) -> &SdfPathVector {
        self.sort();
        &self.ids
    }

    /// Add an id. Duplicates are allowed.
    pub fn insert(&mut self, id: SdfPath) {
        match self.mode {
            EditMode::NoMode => {
                self.mode = EditMode::InsertMode;
                self.edits.push(id);
            }
            EditMode::InsertMode => {
                self.edits.push(id);
            }
            EditMode::RemoveMode => {
                self.sort();
                self.mode = EditMode::InsertMode;
                self.edits.clear();
                self.edits.push(id);
            }
        }
    }

    /// Remove one occurrence of id. No-op if not present.
    pub fn remove(&mut self, id: SdfPath) {
        match self.mode {
            EditMode::NoMode => {
                self.mode = EditMode::RemoveMode;
                self.edits.push(id);
            }
            EditMode::RemoveMode => {
                self.edits.push(id);
            }
            EditMode::InsertMode => {
                self.sort();
                self.mode = EditMode::RemoveMode;
                self.edits.clear();
                self.edits.push(id);
            }
        }
    }

    /// Remove range by position index (start..=end inclusive).
    pub fn remove_range(&mut self, start: usize, end: usize) {
        if !self.edits.is_empty() {
            self.sort();
        }
        let len = self.ids.len();
        if start > end || start >= len {
            return;
        }
        let end_idx = (end + 1).min(len);
        self.ids.drain(start..end_idx);
    }

    /// Remove all ids.
    pub fn clear(&mut self) {
        self.ids.clear();
        self.edits.clear();
        self.mode = EditMode::NoMode;
    }

    fn sort(&mut self) {
        if self.mode == EditMode::NoMode && self.edits.is_empty() {
            return;
        }
        self.edits.sort();
        let removing = self.mode == EditMode::RemoveMode;
        if !removing && self.ids.is_empty() {
            std::mem::swap(&mut self.ids, &mut self.edits);
            self.mode = EditMode::NoMode;
            return;
        }
        if removing {
            let mut to_remove = self.edits.clone();
            self.ids.retain(|p| {
                if let Some(pos) = to_remove.iter().position(|r| r == p) {
                    to_remove.remove(pos);
                    false
                } else {
                    true
                }
            });
        } else {
            let mut merged = Vec::with_capacity(self.ids.len() + self.edits.len());
            let mut i = 0;
            let mut j = 0;
            while i < self.ids.len() && j < self.edits.len() {
                match self.ids[i].cmp(&self.edits[j]) {
                    Ordering::Less => {
                        merged.push(self.ids[i].clone());
                        i += 1;
                    }
                    Ordering::Equal | Ordering::Greater => {
                        merged.push(self.edits[j].clone());
                        j += 1;
                    }
                }
            }
            merged.extend(self.ids[i..].iter().cloned());
            merged.extend(self.edits[j..].iter().cloned());
            self.ids = merged;
        }
        self.edits.clear();
        self.mode = EditMode::NoMode;
    }
}
