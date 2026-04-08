//! HdSelection - Collection of selected items per selection mode.
//!
//! Corresponds to pxr/imaging/hd/selection.h.
//! Supports rprims, instances, elements (faces), edges, points.

use std::collections::HashMap;
use usd_gf::vec4::Vec4f;
use usd_sdf::Path as SdfPath;

/// Integer array (indices).
pub type VtIntArray = Vec<i32>;

/// Selection mode for highlight behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HdSelectionHighlightMode {
    /// Active selection.
    Select = 0,
    /// Rollover/hover selection.
    Locate = 1,
}

impl HdSelectionHighlightMode {
    /// Number of highlight modes.
    pub const COUNT: usize = 2;
}

impl Default for HdSelectionHighlightMode {
    fn default() -> Self {
        Self::Select
    }
}

/// Selection state for one rprim.
#[derive(Debug, Clone, Default)]
pub struct HdPrimSelectionState {
    /// True if the entire prim is selected.
    pub fully_selected: bool,
    /// Selected instance index arrays.
    pub instance_indices: Vec<VtIntArray>,
    /// Selected element (face) index arrays.
    pub element_indices: Vec<VtIntArray>,
    /// Selected edge index arrays.
    pub edge_indices: Vec<VtIntArray>,
    /// Selected point index arrays.
    pub point_indices: Vec<VtIntArray>,
    /// Color indices for selected points.
    pub point_color_indices: Vec<i32>,
}

/// Shared pointer to selection.
pub type HdSelectionSharedPtr = std::sync::Arc<parking_lot::RwLock<HdSelection>>;

/// Holds selected items per selection mode.
///
/// Corresponds to C++ `HdSelection`.
#[derive(Debug, Default, Clone)]
pub struct HdSelection {
    sel_map: [HashMap<SdfPath, HdPrimSelectionState>; HdSelectionHighlightMode::COUNT],
    selected_point_colors: Vec<Vec4f>,
}

impl HdSelection {
    /// Create new empty selection.
    pub fn new() -> Self {
        Self {
            sel_map: Default::default(),
            selected_point_colors: Vec::new(),
        }
    }

    /// Add rprim to selection.
    pub fn add_rprim(&mut self, mode: HdSelectionHighlightMode, render_index_path: SdfPath) {
        let m = mode as usize;
        self.sel_map[m]
            .entry(render_index_path)
            .or_default()
            .fully_selected = true;
    }

    /// Add instance selection.
    ///
    /// Matches C++ HdSelection::AddInstance: empty instanceIndices means all
    /// instances selected (fullySelected = true). Always appends the array
    /// to instanceIndices regardless of whether it is empty.
    pub fn add_instance(
        &mut self,
        mode: HdSelectionHighlightMode,
        render_index_path: SdfPath,
        instance_indices: VtIntArray,
    ) {
        let m = mode as usize;
        let state = self.sel_map[m].entry(render_index_path).or_default();
        // C++: empty instanceIndices -> fullySelected = true (all instances).
        if instance_indices.is_empty() {
            state.fully_selected = true;
        }
        state.instance_indices.push(instance_indices);
    }

    /// Add element (face) selection.
    ///
    /// Matches C++ HdSelection::AddElements: an empty elementIndices array
    /// is shorthand for selecting all elements (fullySelected = true) and
    /// does NOT push an empty array. Non-empty arrays are appended normally.
    pub fn add_elements(
        &mut self,
        mode: HdSelectionHighlightMode,
        render_index_path: SdfPath,
        element_indices: VtIntArray,
    ) {
        let m = mode as usize;
        let state = self.sel_map[m].entry(render_index_path).or_default();
        if element_indices.is_empty() {
            // C++: empty -> fullySelected = true, do NOT push the empty array.
            state.fully_selected = true;
        } else {
            state.element_indices.push(element_indices);
        }
    }

    /// Add edge selection.
    ///
    /// Matches C++ HdSelection::AddEdges: empty edgeIndices arrays are
    /// silently skipped (no change to selection state).
    pub fn add_edges(
        &mut self,
        mode: HdSelectionHighlightMode,
        render_index_path: SdfPath,
        edge_indices: VtIntArray,
    ) {
        // C++: skip empty edge index arrays.
        if edge_indices.is_empty() {
            return;
        }
        let m = mode as usize;
        let state = self.sel_map[m].entry(render_index_path).or_default();
        state.edge_indices.push(edge_indices);
    }

    /// Add point selection without a custom color.
    ///
    /// Matches C++ HdSelection::AddPoints(mode, path, pointIndices): uses -1
    /// as the point color index, signaling "no explicit color".
    pub fn add_points(
        &mut self,
        mode: HdSelectionHighlightMode,
        render_index_path: SdfPath,
        point_indices: VtIntArray,
    ) {
        // C++: _AddPoints with pointColorIndex = -1 (no color assigned).
        self.add_points_impl(mode, render_index_path, point_indices, -1);
    }

    /// Add point selection with a custom color.
    ///
    /// Matches C++ HdSelection::AddPoints(mode, path, pointIndices, pointColor).
    /// Deduplicates entries in selected_point_colors (std::find before push_back).
    pub fn add_points_with_color(
        &mut self,
        mode: HdSelectionHighlightMode,
        render_index_path: SdfPath,
        point_indices: VtIntArray,
        point_color: Vec4f,
    ) {
        // C++: find existing color to avoid duplicate entries.
        let color_idx = if let Some(pos) = self
            .selected_point_colors
            .iter()
            .position(|c| *c == point_color)
        {
            pos as i32
        } else {
            let idx = self.selected_point_colors.len() as i32;
            self.selected_point_colors.push(point_color);
            idx
        };
        self.add_points_impl(mode, render_index_path, point_indices, color_idx);
    }

    /// Internal helper: add points with an explicit color index (-1 = no color).
    ///
    /// Matches C++ HdSelection::_AddPoints. Empty pointIndices arrays are skipped.
    fn add_points_impl(
        &mut self,
        mode: HdSelectionHighlightMode,
        render_index_path: SdfPath,
        point_indices: VtIntArray,
        point_color_index: i32,
    ) {
        // C++: skip empty point index arrays (same guard as AddEdges).
        if point_indices.is_empty() {
            return;
        }
        let m = mode as usize;
        let state = self.sel_map[m].entry(render_index_path).or_default();
        state.point_indices.push(point_indices);
        state.point_color_indices.push(point_color_index);
    }

    /// Get prim selection state for path and mode.
    pub fn get_prim_selection_state(
        &self,
        mode: HdSelectionHighlightMode,
        render_index_path: &SdfPath,
    ) -> Option<&HdPrimSelectionState> {
        let m = mode as usize;
        self.sel_map[m].get(render_index_path)
    }

    /// Get all selected prim paths (all modes, may have duplicates).
    pub fn get_all_selected_prim_paths(&self) -> Vec<SdfPath> {
        let mut paths = Vec::new();
        for map in &self.sel_map {
            paths.extend(map.keys().cloned());
        }
        paths
    }

    /// Get selected prim paths for mode.
    pub fn get_selected_prim_paths(&self, mode: HdSelectionHighlightMode) -> Vec<SdfPath> {
        let m = mode as usize;
        self.sel_map[m].keys().cloned().collect()
    }

    /// Get selected point colors.
    pub fn get_selected_point_colors(&self) -> &[Vec4f] {
        &self.selected_point_colors
    }

    /// Check if selection is empty.
    pub fn is_empty(&self) -> bool {
        self.sel_map.iter().all(|m| m.is_empty())
    }

    /// Merge two selections.
    ///
    /// Matches C++ `HdSelection::Merge`: starts from a copy of `a`, then
    /// OR-merges `b` into it. For paths present in both, `fully_selected` is
    /// OR-ed and all index vectors are appended. Point color indices from `b`
    /// are offset by the number of point colors already in `a` so they remain
    /// consistent after the color arrays are concatenated.
    pub fn merge(a: &HdSelection, b: &HdSelection) -> HdSelection {
        if a.is_empty() {
            return b.clone();
        }
        if b.is_empty() {
            return a.clone();
        }

        // Start from a full copy of a (matches C++ `result = std::make_shared<HdSelection>(*a)`).
        let mut result = a.clone();

        // Offset for b's point color indices — they shift by how many colors are already in a.
        let pt_offset = result.selected_point_colors.len();

        // Append b's point colors.
        result
            .selected_point_colors
            .extend(b.selected_point_colors.iter().cloned());

        for m in 0..HdSelectionHighlightMode::COUNT {
            for (path, state) in &b.sel_map[m] {
                let result_state = result.sel_map[m].entry(path.clone()).or_default();

                // OR the fully_selected flag (matches C++ `resultState.fullySelected |= state.fullySelected`).
                result_state.fully_selected |= state.fully_selected;

                // Append all index vectors from b (matches C++ `_Append`).
                result_state
                    .instance_indices
                    .extend(state.instance_indices.iter().cloned());
                result_state
                    .element_indices
                    .extend(state.element_indices.iter().cloned());
                result_state
                    .edge_indices
                    .extend(state.edge_indices.iter().cloned());
                result_state
                    .point_indices
                    .extend(state.point_indices.iter().cloned());

                // Append point color indices with offset (matches C++ `_AppendWithOffset`).
                result_state.point_color_indices.extend(
                    state
                        .point_color_indices
                        .iter()
                        .map(|&idx| idx + pt_offset as i32),
                );
            }
        }

        result
    }
}
