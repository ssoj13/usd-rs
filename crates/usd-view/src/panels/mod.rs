//! UI panels for the viewer.
//!
//! Each panel corresponds to a DockTab.

pub mod attr_editor;
pub mod attributes_enhanced;
pub mod camera_controls;
pub mod composition;
pub mod debug_flags;
pub mod hud;
pub mod layer_stack_enhanced;
pub mod overlays;
pub mod pick;
pub mod preferences;
pub mod prim_tree;
pub mod prim_tree_enhanced;
pub mod spline_viewer;
pub mod validation;
pub mod viewport;

pub use prim_tree::PrimTreeAction;
