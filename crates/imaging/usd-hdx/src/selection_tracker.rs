//! Selection tracking for interactive applications
//!
//! Maintains the set of selected objects and communicates changes
//! to rendering tasks for highlight visualization.
//!
//! # Buffer Layout (GetSelectionOffsetBuffer)
//!
//! The selection offset buffer has the following layout:
//!
//! ```text
//! [# modes] [per-mode start offsets] [mode0 data] [mode1 data] ...
//! ```
//!
//! - Index 0: number of highlight modes (currently 2)
//! - Index 1..=modes: start offset in the buffer for each mode's data,
//!   or 0 if the mode has no selected items.
//! - Per-mode data: prim range [min, max+1] followed by per-prim seloffsets.
//!   Each seloffset encodes `(next_offset << 1) | is_selected`.

use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::Arc;
use usd_sdf::Path;

/// Selection highlight modes, matching HdSelection::HighlightMode.
///
/// The order must match C++ enum exactly — used as array indices in the
/// GPU offset buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightMode {
    /// Standard selection highlighting (mouse click).
    Select = 0,
    /// Locate / rollover highlighting (hover).
    Locate = 1,
}

impl HighlightMode {
    /// Total number of highlight modes.
    pub const COUNT: usize = 2;
}

/// Encode a selection offset value following the C++ _EncodeSelOffset convention.
///
/// Bit 0 = is_selected flag, bits 31:1 = offset to next level in hierarchy.
/// If offset is 0 there is nothing further to decode.
#[inline]
fn encode_sel_offset(offset: usize, is_selected: bool) -> i32 {
    ((offset as i32) << 1) | (is_selected as i32)
}

/// Selection state for a scene
///
/// Tracks which objects are selected and provides notifications
/// when selection changes. Used by selection highlighting tasks.
#[derive(Debug, Clone)]
pub struct HdxSelection {
    /// Currently selected prim paths
    selected_paths: HashSet<Path>,
    /// Currently located / rollover prim paths.
    located_paths: HashSet<Path>,

    /// Selection version counter (incremented on changes)
    version: u64,
}

impl HdxSelection {
    /// Create new empty selection
    pub fn new() -> Self {
        Self {
            selected_paths: HashSet::new(),
            located_paths: HashSet::new(),
            version: 0,
        }
    }

    /// Add path to selection
    pub fn add(&mut self, path: Path) -> bool {
        if self.selected_paths.insert(path) {
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Remove path from selection
    pub fn remove(&mut self, path: &Path) -> bool {
        if self.selected_paths.remove(path) {
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Clear all selections
    pub fn clear(&mut self) {
        if !self.selected_paths.is_empty() || !self.located_paths.is_empty() {
            self.selected_paths.clear();
            self.located_paths.clear();
            self.version += 1;
        }
    }

    /// Check if path is selected
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected_paths.contains(path)
    }

    /// Get all selected paths
    pub fn get_selected_paths(&self) -> Vec<Path> {
        self.selected_paths.iter().cloned().collect()
    }

    /// Set the rollover / locate path set.
    pub fn set_located_paths(&mut self, paths: &[Path]) {
        let new_paths: HashSet<Path> = paths.iter().cloned().collect();
        if self.located_paths != new_paths {
            self.located_paths = new_paths;
            self.version += 1;
        }
    }

    /// Replace only the primary selection set while preserving locate paths.
    pub fn set_selected_paths(&mut self, paths: &[Path]) {
        let new_paths: HashSet<Path> = paths.iter().cloned().collect();
        if self.selected_paths != new_paths {
            self.selected_paths = new_paths;
            self.version += 1;
        }
    }

    /// Clear only the primary selection set while preserving locate paths.
    pub fn clear_selected_paths(&mut self) {
        if !self.selected_paths.is_empty() {
            self.selected_paths.clear();
            self.version += 1;
        }
    }

    /// Get all located paths.
    pub fn get_located_paths(&self) -> Vec<Path> {
        self.located_paths.iter().cloned().collect()
    }

    /// Get number of selected items
    pub fn count(&self) -> usize {
        self.selected_paths.len() + self.located_paths.len()
    }

    /// Check if selection is empty
    pub fn is_empty(&self) -> bool {
        self.selected_paths.is_empty() && self.located_paths.is_empty()
    }

    /// Get current selection version
    ///
    /// Version increments on each change, allowing consumers
    /// to detect when selection has been modified.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Build a selection offset buffer for tracker-owned path sets.
    ///
    /// This keeps the same top-level buffer layout as C++
    /// `HdxSelectionTracker::GetSelectionOffsetBuffer`, but full render-index-aware
    /// mode population is finalized later by `HdxSelectionTask`.
    ///
    /// # Buffer layout
    ///
    /// ```text
    /// [num_modes] [mode0_start] [mode1_start] ... [mode0_data...] [mode1_data...]
    /// ```
    ///
    /// - `offsets[0]` = number of highlight modes (2).
    /// - `offsets[1..=modes]` = absolute start index for each mode's data, or 0
    ///   if that mode has no selection.
    /// - Per-mode data: `[min] [max+1] [seloffset per prim-id...]`
    ///   Each seloffset: bit 0 = is_selected, bits 31:1 = offset to subprim data.
    ///
    /// `enable_selection` / `enable_locate` gate their respective modes.
    ///
    /// Returns `true` when at least one prim is selected (any mode).
    pub fn get_selection_offset_buffer(
        &self,
        enable_selection: bool,
        enable_locate: bool,
    ) -> (Vec<i32>, bool) {
        // Minimum buffer size required by GPU (UBO/SSBO alignment).
        const MIN_SIZE: usize = 8;
        const NUM_MODES: usize = HighlightMode::COUNT;
        // Header: 1 (num_modes) + NUM_MODES (per-mode start offsets).
        const HEADER_SIZE: usize = NUM_MODES + 1;
        const SELECT_NONE: i32 = 0;

        let mut offsets: Vec<i32> = vec![0; MIN_SIZE.max(HEADER_SIZE)];
        offsets[0] = NUM_MODES as i32;

        // Nothing selected or located — fill header with SELECT_NONE and return early.
        if (self.selected_paths.is_empty() && self.located_paths.is_empty())
            || (!enable_selection && !enable_locate)
        {
            for mode in 0..NUM_MODES {
                offsets[mode + 1] = SELECT_NONE;
            }
            return (offsets, false);
        }

        let mode_enabled = [enable_selection, enable_locate];
        let mut has_selection = false;
        let mut copy_offset = HEADER_SIZE;

        for mode in 0..NUM_MODES {
            if !mode_enabled[mode] {
                offsets[mode + 1] = SELECT_NONE;
                continue;
            }
            let paths = if mode == HighlightMode::Select as usize {
                &self.selected_paths
            } else {
                &self.located_paths
            };

            // Collect integer prim IDs. Without a render index we use a
            // deterministic per-mode ordering so the buffer is stable and
            // round-trippable in tests. Real render-index prim IDs are filled by
            // `HdxSelectionTask`.
            let mut sorted_paths: Vec<_> = paths.iter().collect();
            sorted_paths.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            let ids: Vec<i32> = sorted_paths
                .iter()
                .enumerate()
                .map(|(i, _)| i as i32)
                .collect();

            if ids.is_empty() {
                offsets[mode + 1] = SELECT_NONE;
                continue;
            }

            let min_id = *ids.iter().min().unwrap();
            let max_id = *ids.iter().max().unwrap();
            let range = (max_id - min_id + 1) as usize;

            // Prim data: [min] [max+1] [seloffset per id in range].
            const PRIM_HEADER: usize = 2;
            let needed = copy_offset + PRIM_HEADER + range;
            if offsets.len() < needed {
                offsets.resize(needed, 0);
            }

            offsets[mode + 1] = copy_offset as i32;

            offsets[copy_offset] = min_id;
            offsets[copy_offset + 1] = max_id + 1;

            // Zero-init seloffsets in the range (SELECT_NONE).
            for i in 0..range {
                offsets[copy_offset + PRIM_HEADER + i] = encode_sel_offset(0, false);
            }
            // Mark each selected prim.
            for id in &ids {
                let slot = (id - min_id) as usize;
                offsets[copy_offset + PRIM_HEADER + slot] = encode_sel_offset(0, true);
            }

            copy_offset += PRIM_HEADER + range;
            has_selection = true;
        }

        (offsets, has_selection)
    }
}

impl Default for HdxSelection {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe selection tracker
///
/// Provides shared access to selection state across tasks and threads.
pub type HdxSelectionTracker = Arc<RwLock<HdxSelection>>;

/// Create new selection tracker
pub fn create_selection_tracker() -> HdxSelectionTracker {
    Arc::new(RwLock::new(HdxSelection::new()))
}

/// Selection tracker interface for tasks.
///
/// Provides convenient methods for working with selection from tasks.
pub trait SelectionTrackerExt {
    /// Get current selection version.
    fn get_version(&self) -> i32;

    /// Check if selection is empty.
    fn is_empty(&self) -> bool;

    /// Get number of selected items.
    fn count(&self) -> usize;

    /// Get snapshot of selected paths.
    fn get_selected_paths(&self) -> Vec<Path>;

    /// Get snapshot of locate / rollover paths.
    fn get_located_paths(&self) -> Vec<Path>;

    /// Check if selection contains path.
    fn contains(&self, path: &Path) -> bool;

    /// Set entire selection to new set of paths.
    fn set_selection(&self, paths: &[Path]);

    /// Add path to selection.
    fn select(&self, path: Path);

    /// Remove path from selection.
    fn deselect(&self, path: &Path);

    /// Clear entire selection.
    fn clear_selection(&self);

    /// Replace the locate / rollover set.
    fn set_locate_selection(&self, paths: &[Path]);

    /// Clear locate / rollover state.
    fn clear_locate_selection(&self);

    /// Build GPU selection offset buffer (see HdxSelection::get_selection_offset_buffer).
    ///
    /// Returns `(buffer, has_selection)`.
    fn get_selection_offset_buffer(
        &self,
        enable_selection: bool,
        enable_locate: bool,
    ) -> (Vec<i32>, bool);
}

impl SelectionTrackerExt for HdxSelectionTracker {
    fn get_version(&self) -> i32 {
        self.read().version() as i32
    }

    fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    fn count(&self) -> usize {
        self.read().count()
    }

    fn get_selected_paths(&self) -> Vec<Path> {
        self.read().get_selected_paths()
    }

    fn get_located_paths(&self) -> Vec<Path> {
        self.read().get_located_paths()
    }

    fn contains(&self, path: &Path) -> bool {
        self.read().is_selected(path)
    }

    fn set_selection(&self, paths: &[Path]) {
        self.write().set_selected_paths(paths);
    }

    fn select(&self, path: Path) {
        self.write().add(path);
    }

    fn deselect(&self, path: &Path) {
        self.write().remove(path);
    }

    fn clear_selection(&self) {
        self.write().clear_selected_paths();
    }

    fn set_locate_selection(&self, paths: &[Path]) {
        self.write().set_located_paths(paths);
    }

    fn clear_locate_selection(&self) {
        self.write().set_located_paths(&[]);
    }

    fn get_selection_offset_buffer(
        &self,
        enable_selection: bool,
        enable_locate: bool,
    ) -> (Vec<i32>, bool) {
        self.read()
            .get_selection_offset_buffer(enable_selection, enable_locate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_new() {
        let sel = HdxSelection::new();
        assert!(sel.is_empty());
        assert_eq!(sel.count(), 0);
        assert_eq!(sel.version(), 0);
    }

    #[test]
    fn test_selection_add() {
        let mut sel = HdxSelection::new();
        let path = Path::from_string("/test/prim").unwrap();

        assert!(sel.add(path.clone()));
        assert!(sel.is_selected(&path));
        assert_eq!(sel.count(), 1);
        assert_eq!(sel.version(), 1);

        // Adding again should return false
        assert!(!sel.add(path.clone()));
        assert_eq!(sel.version(), 1); // Version unchanged
    }

    #[test]
    fn test_selection_remove() {
        let mut sel = HdxSelection::new();
        let path = Path::from_string("/test/prim").unwrap();

        sel.add(path.clone());
        assert!(sel.remove(&path));
        assert!(!sel.is_selected(&path));
        assert_eq!(sel.count(), 0);
        assert_eq!(sel.version(), 2); // Added + removed

        // Removing again should return false
        assert!(!sel.remove(&path));
        assert_eq!(sel.version(), 2); // Version unchanged
    }

    #[test]
    fn test_selection_clear() {
        let mut sel = HdxSelection::new();
        sel.add(Path::from_string("/a").unwrap());
        sel.add(Path::from_string("/b").unwrap());
        sel.add(Path::from_string("/c").unwrap());

        let version_before = sel.version();
        sel.clear();

        assert!(sel.is_empty());
        assert_eq!(sel.count(), 0);
        assert_eq!(sel.version(), version_before + 1);
    }

    #[test]
    fn test_selection_version_tracking() {
        let mut sel = HdxSelection::new();
        assert_eq!(sel.version(), 0);

        sel.add(Path::from_string("/a").unwrap());
        assert_eq!(sel.version(), 1);

        sel.add(Path::from_string("/b").unwrap());
        assert_eq!(sel.version(), 2);

        sel.remove(&Path::from_string("/a").unwrap());
        assert_eq!(sel.version(), 3);

        sel.clear();
        assert_eq!(sel.version(), 4);

        // Clear on empty doesn't increment
        sel.clear();
        assert_eq!(sel.version(), 4);
    }

    #[test]
    fn test_selection_tracker() {
        let tracker = create_selection_tracker();

        tracker.select(Path::from_string("/test/a").unwrap());
        tracker.select(Path::from_string("/test/b").unwrap());

        assert_eq!(tracker.get_selected_paths().len(), 2);
        assert!(tracker.contains(&Path::from_string("/test/a").unwrap()));

        tracker.deselect(&Path::from_string("/test/a").unwrap());
        assert!(!tracker.contains(&Path::from_string("/test/a").unwrap()));
        assert_eq!(tracker.get_selected_paths().len(), 1);

        tracker.clear_selection();
        assert_eq!(tracker.get_selected_paths().len(), 0);
    }

    #[test]
    fn test_selection_tracker_set() {
        let tracker = create_selection_tracker();

        let paths = vec![
            Path::from_string("/a").unwrap(),
            Path::from_string("/b").unwrap(),
            Path::from_string("/c").unwrap(),
        ];

        tracker.set_selection(&paths);
        assert_eq!(tracker.get_selected_paths().len(), 3);

        for path in &paths {
            assert!(tracker.contains(path));
        }
    }

    #[test]
    fn test_selection_tracker_locate_set() {
        let tracker = create_selection_tracker();
        let paths = vec![
            Path::from_string("/a").unwrap(),
            Path::from_string("/b").unwrap(),
        ];

        tracker.set_locate_selection(&paths);
        let located = tracker.get_located_paths();
        assert_eq!(located.len(), 2);
        for path in &paths {
            assert!(located.contains(path));
        }

        tracker.clear_locate_selection();
        assert!(tracker.get_located_paths().is_empty());
    }

    #[test]
    fn test_set_selection_preserves_locate_paths() {
        let tracker = create_selection_tracker();
        let selected = Path::from_string("/selected").unwrap();
        let located = Path::from_string("/located").unwrap();

        tracker.set_locate_selection(std::slice::from_ref(&located));
        let version_before = tracker.get_version();
        tracker.set_selection(std::slice::from_ref(&selected));

        assert_eq!(tracker.get_selected_paths(), vec![selected]);
        assert_eq!(tracker.get_located_paths(), vec![located]);
        assert!(tracker.get_version() > version_before);
    }

    #[test]
    fn test_set_selection_is_stable_when_unchanged() {
        let tracker = create_selection_tracker();
        let selected = Path::from_string("/selected").unwrap();

        tracker.set_selection(std::slice::from_ref(&selected));
        let version_before = tracker.get_version();
        tracker.set_selection(std::slice::from_ref(&selected));

        assert_eq!(tracker.get_selected_paths(), vec![selected]);
        assert_eq!(tracker.get_version(), version_before);
    }

    // -----------------------------------------------------------------------
    // get_selection_offset_buffer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_offset_buffer_empty_selection() {
        let sel = HdxSelection::new();
        let (buf, has_sel) = sel.get_selection_offset_buffer(true, true);

        // Buffer must be at least MIN_SIZE=8.
        assert!(buf.len() >= 8);
        // offsets[0] = num_modes = 2.
        assert_eq!(buf[0], 2);
        // Both modes: SELECT_NONE = 0.
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 0);
        assert!(!has_sel);
    }

    #[test]
    fn test_offset_buffer_disabled_modes() {
        let mut sel = HdxSelection::new();
        sel.add(Path::from_string("/a").unwrap());

        // Both modes disabled -> no selection encoded.
        let (buf, has_sel) = sel.get_selection_offset_buffer(false, false);
        assert!(!has_sel);
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 0);
    }

    #[test]
    fn test_offset_buffer_header_layout() {
        // num_modes must be 2 (HighlightMode::COUNT).
        let sel = HdxSelection::new();
        let (buf, _) = sel.get_selection_offset_buffer(true, true);
        assert_eq!(buf[0], HighlightMode::COUNT as i32);
        // header size = 1 + num_modes = 3 slots consumed.
        assert!(buf.len() >= 3);
    }

    #[test]
    fn test_offset_buffer_single_selected_prim() {
        let mut sel = HdxSelection::new();
        sel.add(Path::from_string("/World/Prim").unwrap());

        let (buf, has_sel) = sel.get_selection_offset_buffer(true, true);
        assert!(has_sel);
        // mode=Select (index 1) should point into the buffer (non-zero).
        assert!(buf[1] > 0, "select mode must have non-zero start offset");
        // mode=Locate (index 2) should be SELECT_NONE in this implementation.
        assert_eq!(buf[2], 0);

        // The prim at id=0 must have is_selected=1 (LSB set).
        let select_start = buf[1] as usize;
        // select_start -> [min=0, max+1=1, seloffset for id 0].
        assert_eq!(buf[select_start], 0); // min
        assert_eq!(buf[select_start + 1], 1); // max+1
        // seloffset LSB = 1 (is_selected).
        assert_eq!(buf[select_start + 2] & 1, 1);
    }

    #[test]
    fn test_offset_buffer_multiple_prims() {
        let mut sel = HdxSelection::new();
        sel.add(Path::from_string("/a").unwrap());
        sel.add(Path::from_string("/b").unwrap());
        sel.add(Path::from_string("/c").unwrap());

        let (buf, has_sel) = sel.get_selection_offset_buffer(true, false);
        assert!(has_sel);

        let select_start = buf[1] as usize;
        let min_id = buf[select_start];
        let max_plus1 = buf[select_start + 1];
        let range = (max_plus1 - min_id) as usize;

        // 3 prims -> range must be at least 1 (could be 3 if min=0,max=2).
        assert!(range >= 1);

        // All prims in range must have is_selected=1.
        for i in 0..range {
            let seloffset = buf[select_start + 2 + i];
            assert_eq!(seloffset & 1, 1, "prim at offset {} must be selected", i);
        }
    }

    #[test]
    fn test_offset_buffer_encode_decode() {
        // Verify the bit encoding: (offset << 1) | is_selected.
        assert_eq!(encode_sel_offset(0, false), 0b00);
        assert_eq!(encode_sel_offset(0, true), 0b01);
        assert_eq!(encode_sel_offset(1, false), 0b10);
        assert_eq!(encode_sel_offset(1, true), 0b11);
        assert_eq!(encode_sel_offset(5, true), 0b1011);

        // Decode: is_selected = value & 1, offset = value >> 1.
        let encoded = encode_sel_offset(42, true);
        assert_eq!(encoded & 1, 1);
        assert_eq!(encoded >> 1, 42);
    }

    #[test]
    fn test_offset_buffer_via_tracker() {
        let tracker = create_selection_tracker();
        tracker.select(Path::from_string("/test/prim").unwrap());

        let (buf, has_sel) = tracker.get_selection_offset_buffer(true, true);
        assert!(has_sel);
        assert_eq!(buf[0], 2); // num_modes
    }
}
