//! Selection task - Highlight selected objects.
//!
//! Renders selection highlight overlay for interactive applications.
//! Port of pxr/imaging/hdx/selectionTask.h/cpp

use super::selection_tracker::{HdxSelectionTracker, SelectionTrackerExt};
use usd_gf::Vec4f;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Selection task parameters.
///
/// Port of HdxSelectionTaskParams from pxr/imaging/hdx/selectionTask.h
///
/// Note: `PartialEq` matches C++ `operator==` which excludes
/// `occluded_selection_opacity` (only enable flags and colors are compared).
#[derive(Debug, Clone)]
pub struct HdxSelectionTaskParams {
    /// Whether to enable selection highlighting.
    pub enable_selection_highlight: bool,

    /// Whether to enable locate (rollover) highlighting.
    pub enable_locate_highlight: bool,

    /// Opacity for occluded (hidden) selections.
    /// Allows showing selection outline behind geometry.
    /// Not included in PartialEq (matches C++ operator==).
    pub occluded_selection_opacity: f32,

    /// Selection highlight color (RGBA).
    pub selection_color: Vec4f,

    /// Locate/rollover highlight color (RGBA).
    pub locate_color: Vec4f,
}

impl PartialEq for HdxSelectionTaskParams {
    /// Matches C++ `operator==`: compares enable flags and colors only.
    ///
    /// `occluded_selection_opacity` is intentionally excluded, matching the
    /// reference implementation in hdx/selectionTask.cpp.
    fn eq(&self, other: &Self) -> bool {
        self.enable_selection_highlight == other.enable_selection_highlight
            && self.enable_locate_highlight == other.enable_locate_highlight
            && self.selection_color == other.selection_color
            && self.locate_color == other.locate_color
    }
}

impl Default for HdxSelectionTaskParams {
    fn default() -> Self {
        Self {
            enable_selection_highlight: true,
            enable_locate_highlight: true,
            occluded_selection_opacity: 0.5,
            selection_color: Vec4f::new(1.0, 1.0, 0.0, 0.5), // Yellow with 50% opacity
            locate_color: Vec4f::new(0.0, 0.0, 1.0, 0.5),    // Blue with 50% opacity
        }
    }
}

/// Selection highlighting task.
///
/// A task for rendering selection overlay highlighting for interactive
/// applications. Uses the selection state from HdxSelectionTracker.
///
/// Port of HdxSelectionTask from pxr/imaging/hdx/selectionTask.h
pub struct HdxSelectionTask {
    /// Task path.
    id: Path,

    /// Last known selection version.
    last_version: i32,

    /// Whether there is an active selection.
    has_selection: bool,

    /// Selection parameters.
    params: HdxSelectionTaskParams,

    /// Selection offset buffer (GPU resource).
    /// Stores per-prim selection offset indices.
    #[allow(dead_code)]
    sel_offset_bar: Option<BufferArrayRange>,

    /// Selection uniform buffer (GPU resource).
    /// Stores selection colors and other uniforms.
    #[allow(dead_code)]
    sel_uniform_bar: Option<BufferArrayRange>,

    /// Size of point colors buffer for point cloud selection.
    #[allow(dead_code)]
    point_colors_buffer_size: usize,

    /// Selection tracker reference.
    selection_tracker: Option<HdxSelectionTracker>,

    /// Cached tracker version used to avoid rebuilding selection offsets every frame.
    cached_selection_version: i32,

    /// Cached selection offsets for the current tracker version.
    cached_selection_offsets: Option<Vec<i32>>,
}

/// Placeholder for GPU buffer array range.
/// In full Storm implementation, this would be HdBufferArrayRangeSharedPtr.
#[derive(Debug, Clone)]
pub struct BufferArrayRange {
    /// Buffer identifier.
    pub buffer_id: u32,
    /// Offset in the buffer.
    pub offset: usize,
    /// Size of the range.
    pub size: usize,
}

impl HdxSelectionTask {
    /// Create new selection task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            last_version: -1,
            has_selection: false,
            params: HdxSelectionTaskParams::default(),
            sel_offset_bar: None,
            sel_uniform_bar: None,
            point_colors_buffer_size: 0,
            selection_tracker: None,
            cached_selection_version: -1,
            cached_selection_offsets: None,
        }
    }

    /// Set selection tracker.
    pub fn set_selection_tracker(&mut self, tracker: HdxSelectionTracker) {
        self.selection_tracker = Some(tracker);
    }

    /// Get selection tracker.
    pub fn get_selection_tracker(&self) -> Option<&HdxSelectionTracker> {
        self.selection_tracker.as_ref()
    }

    /// Set selection parameters.
    pub fn set_params(&mut self, params: HdxSelectionTaskParams) {
        if self.params != params {
            self.params = params;
            self.cached_selection_version = -1;
        }
    }

    /// Get selection parameters.
    pub fn get_params(&self) -> &HdxSelectionTaskParams {
        &self.params
    }

    /// Check if selection has changed since last sync.
    fn selection_changed(&self, tracker: &HdxSelectionTracker) -> bool {
        tracker.get_version() != self.last_version
    }

    fn build_selection_offsets(&self, render_index: &dyn HdRenderIndexTrait) -> Option<Vec<i32>> {
        let tracker = self.selection_tracker.as_ref()?;
        let selected_paths = tracker.get_selected_paths();
        let located_paths = tracker.get_located_paths();
        if (selected_paths.is_empty() && located_paths.is_empty())
            || (!self.params.enable_selection_highlight && !self.params.enable_locate_highlight)
        {
            return None;
        }

        const NUM_MODES: usize = 2;
        const HEADER_SIZE: usize = NUM_MODES + 1;
        const MIN_SIZE: usize = 8;

        let mut offsets = vec![0i32; MIN_SIZE.max(HEADER_SIZE)];
        offsets[0] = NUM_MODES as i32;
        let mut copy_offset = HEADER_SIZE;
        let mut has_any = false;

        let mode_inputs = [
            (self.params.enable_selection_highlight, selected_paths),
            (self.params.enable_locate_highlight, located_paths),
        ];
        for (mode_index, (enabled, paths)) in mode_inputs.into_iter().enumerate() {
            if !enabled || paths.is_empty() {
                offsets[mode_index + 1] = 0;
                continue;
            }
            let mut selected_ids = Vec::new();
            for rprim_path in render_index.get_rprim_ids() {
                if paths
                    .iter()
                    .any(|path| &rprim_path == path || rprim_path.has_prefix(path))
                {
                    if let Some(prim_id) = render_index.get_prim_id_for_rprim_path(&rprim_path) {
                        selected_ids.push(prim_id);
                    }
                }
            }
            selected_ids.sort_unstable();
            selected_ids.dedup();
            if selected_ids.is_empty() {
                offsets[mode_index + 1] = 0;
                continue;
            }

            let min_id = *selected_ids.first()?;
            let max_id = *selected_ids.last()?;
            let range = (max_id - min_id + 1) as usize;
            let needed = copy_offset + 2 + range;
            if offsets.len() < needed {
                offsets.resize(needed, 0);
            }
            offsets[mode_index + 1] = copy_offset as i32;
            offsets[copy_offset] = min_id;
            offsets[copy_offset + 1] = max_id + 1;
            for selected_id in selected_ids {
                let slot = copy_offset + 2 + (selected_id - min_id) as usize;
                offsets[slot] = 1;
            }
            copy_offset += 2 + range;
            has_any = true;
        }

        has_any.then_some(offsets)
    }
}

impl HdTask for HdxSelectionTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // In full implementation:
        // Pull params from scene delegate:
        // _params = delegate->Get<HdxSelectionTaskParams>(id, HdTokens->params);

        // Check if selection version changed
        if let Some(ref tracker) = self.selection_tracker {
            if self.selection_changed(tracker) {
                self.last_version = tracker.get_version();
                self.has_selection = !tracker.is_empty();
            }
        }

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, render_index: &dyn HdRenderIndexTrait) {
        // Skip if selection highlighting is disabled
        if !self.params.enable_selection_highlight && !self.params.enable_locate_highlight {
            return;
        }

        let current_version = self
            .selection_tracker
            .as_ref()
            .map(|tracker| tracker.get_version())
            .unwrap_or(-1);
        if current_version != self.cached_selection_version {
            self.cached_selection_offsets = self.build_selection_offsets(render_index);
            self.cached_selection_version = current_version;
        }

        if let Some(selection_offsets) = self.cached_selection_offsets.clone() {
            self.has_selection = true;
            ctx.insert(Token::new("hasSelection"), Value::from(true));
            ctx.insert(Token::new("selectionState"), Value::from(true));
            ctx.insert(
                Token::new("selectionBuffer"),
                Value::new(selection_offsets.clone()),
            );
            ctx.insert(
                Token::new("selectionOffsets"),
                Value::new(selection_offsets),
            );
            ctx.insert(Token::new("selectionUniforms"), Value::from(true));
        } else {
            self.has_selection = false;
            ctx.insert(Token::new("hasSelection"), Value::from(false));
            ctx.remove(&Token::new("selectionState"));
            ctx.remove(&Token::new("selectionBuffer"));
            ctx.remove(&Token::new("selectionOffsets"));
            ctx.remove(&Token::new("selectionUniforms"));
        }
    }

    fn execute(&mut self, _ctx: &mut HdTaskContext) {
        // C++ Execute() is intentionally empty (just HD_TRACE_FUNCTION).
        // All real work (selection buffer upload to GPU) happens in Sync().
        // The selection state is already bound to the render pass state
        // before render_task executes.
    }

    fn get_render_tags(&self) -> &[Token] {
        &[]
    }

    fn is_converged(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::super::selection_tracker::create_selection_tracker;
    use super::*;

    #[test]
    fn test_selection_task_params_default() {
        let params = HdxSelectionTaskParams::default();
        assert!(params.enable_selection_highlight);
        assert!(params.enable_locate_highlight);
        assert_eq!(params.occluded_selection_opacity, 0.5);
    }

    #[test]
    fn test_selection_task_params_equality() {
        let params1 = HdxSelectionTaskParams::default();
        let params2 = HdxSelectionTaskParams::default();
        assert_eq!(params1, params2);

        let mut params3 = HdxSelectionTaskParams::default();
        params3.enable_selection_highlight = false;
        assert_ne!(params1, params3);
    }

    #[test]
    fn test_selection_task_params_opacity_excluded_from_eq() {
        // C++ operator== does NOT compare occluded_selection_opacity.
        let params1 = HdxSelectionTaskParams::default();
        let mut params2 = HdxSelectionTaskParams::default();
        params2.occluded_selection_opacity = 0.99; // different from default 0.5
        assert_eq!(
            params1, params2,
            "occluded_selection_opacity must be excluded from PartialEq"
        );
    }

    #[test]
    fn test_selection_task_params_colors_included_in_eq() {
        let params1 = HdxSelectionTaskParams::default();
        let mut params2 = HdxSelectionTaskParams::default();
        params2.selection_color = Vec4f::new(1.0, 0.0, 0.0, 1.0); // red instead of yellow
        assert_ne!(
            params1, params2,
            "selection_color must be included in PartialEq"
        );
    }

    #[test]
    fn test_selection_task_creation() {
        let task = HdxSelectionTask::new(Path::from_string("/selection").unwrap());
        assert_eq!(task.last_version, -1);
        assert!(!task.has_selection);
        assert!(task.sel_offset_bar.is_none());
        assert!(task.sel_uniform_bar.is_none());
    }

    #[test]
    fn test_selection_task_set_params() {
        let mut task = HdxSelectionTask::new(Path::from_string("/selection").unwrap());

        let mut params = HdxSelectionTaskParams::default();
        params.selection_color = Vec4f::new(1.0, 0.0, 0.0, 1.0); // Red
        params.occluded_selection_opacity = 0.75;

        task.set_params(params.clone());
        assert_eq!(task.get_params(), &params);
    }

    #[test]
    fn test_selection_task_tracker() {
        let mut task = HdxSelectionTask::new(Path::from_string("/selection").unwrap());
        assert!(task.get_selection_tracker().is_none());

        let tracker = create_selection_tracker();
        task.set_selection_tracker(tracker);
        assert!(task.get_selection_tracker().is_some());
    }

    #[test]
    fn test_buffer_array_range() {
        let range = BufferArrayRange {
            buffer_id: 1,
            offset: 0,
            size: 1024,
        };
        assert_eq!(range.buffer_id, 1);
        assert_eq!(range.size, 1024);
    }
}
