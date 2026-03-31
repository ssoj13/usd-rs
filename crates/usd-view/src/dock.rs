//! Dock layout using egui_dock.
//!
//! Layout inspired by usdview:
//! - Central viewport
//! - Left: Prim tree
//! - Right: Attributes, Composition, Layer stack
//! - Bottom: Optional properties panel

use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockState, NodeIndex, TabViewer};
use serde::{Deserialize, Serialize};

use crate::app::ViewerApp;

/// Tab identifiers for dock panels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockTab {
    /// 3D viewport.
    Viewport,
    /// Prim tree (scene hierarchy).
    PrimTree,
    /// Attribute inspector.
    Attributes,
    /// Layer stack.
    LayerStack,
    /// Composition arcs.
    Composition,
    /// Settings / view options.
    Settings,
    /// Spline / animation curve viewer.
    SplineViewer,
}

impl DockTab {
    /// Display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Viewport => "Viewport",
            Self::PrimTree => "Prim Tree",
            Self::Attributes => "Attributes",
            Self::LayerStack => "Layer Stack",
            Self::Composition => "Composition",
            Self::Settings => "Settings",
            Self::SplineViewer => "Spline Viewer",
        }
    }

    /// Keyboard shortcut hint.
    pub fn shortcut(&self) -> Option<&'static str> {
        match self {
            Self::Viewport => Some("V"),
            Self::PrimTree => Some("T"),
            Self::Attributes => Some("A"),
            Self::LayerStack => Some("L"),
            Self::Composition => Some("C"),
            _ => None,
        }
    }
}

/// Default dock layout (with viewport).
pub fn default_dock_state() -> DockState<DockTab> {
    let mut dock = DockState::new(vec![DockTab::Viewport]);

    // Left: Prim tree
    let [_viewport, _prim_tree] =
        dock.main_surface_mut()
            .split_left(NodeIndex::root(), 0.22, vec![DockTab::PrimTree]);

    // Right: Attributes + Composition as tabs, Layer stack below
    let [_center, right] = dock.main_surface_mut().split_right(
        NodeIndex::root(),
        0.72,
        vec![DockTab::Attributes, DockTab::Composition],
    );

    dock.main_surface_mut()
        .split_below(right, 0.5, vec![DockTab::LayerStack]);

    dock
}

/// Default dock layout for --norender (hierarchy only, no viewport).
pub fn default_dock_state_no_render() -> DockState<DockTab> {
    let mut dock = DockState::new(vec![DockTab::PrimTree]);

    // Right: Attributes + Composition as tabs, Layer stack below
    let [_center, right] = dock.main_surface_mut().split_right(
        NodeIndex::root(),
        0.72,
        vec![DockTab::Attributes, DockTab::Composition],
    );

    dock.main_surface_mut()
        .split_below(right, 0.5, vec![DockTab::LayerStack]);

    dock
}

/// Tab viewer implementation.
pub struct DockTabViewer<'a> {
    pub app: &'a mut ViewerApp,
}

impl<'a> TabViewer for DockTabViewer<'a> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut DockTab) -> egui::WidgetText {
        let name = tab.name();
        if let Some(shortcut) = tab.shortcut() {
            format!("{} ({})", name, shortcut).into()
        } else {
            name.into()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut DockTab) {
        let tab_t0 = std::time::Instant::now();
        match tab {
            DockTab::Viewport => self.app.ui_viewport(ui),
            DockTab::PrimTree => self.app.ui_prim_tree(ui),
            DockTab::Attributes => self.app.ui_attributes(ui),
            DockTab::LayerStack => self.app.ui_layer_stack(ui),
            DockTab::Composition => self.app.ui_composition(ui),
            DockTab::Settings => self.app.ui_settings(ui),
            DockTab::SplineViewer => self.app.ui_spline_viewer(ui),
        }
        let tab_ms = tab_t0.elapsed().as_secs_f64() * 1000.0;
        if tab_ms > 10.0 {
            log::info!("[TRACE] dock_tab {:?}: {:.1}ms", tab, tab_ms);
        }
    }

    fn is_closeable(&self, tab: &DockTab) -> bool {
        *tab != DockTab::Viewport
    }

    fn on_close(&mut self, tab: &mut DockTab) -> OnCloseResponse {
        if *tab == DockTab::Viewport {
            OnCloseResponse::Ignore
        } else {
            OnCloseResponse::Close
        }
    }
}
