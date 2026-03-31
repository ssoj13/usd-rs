//! UsdUI - User interface schemas for USD.
//!
//! This module provides schemas for UI-related metadata:
//!
//! - **AccessibilityAPI** - Accessibility information for assistive tech
//! - **Backdrop** - Visual group-box for node graph organization
//! - **NodeGraphNodeAPI** - Node positioning/sizing in node graphs
//! - **SceneGraphPrimAPI** - Display properties for scene graph prims
//!
//! # Node Graph Support
//!
//! The NodeGraphNodeAPI provides attributes for positioning nodes in
//! visual node graph editors (position, size, color, expansion state).
//!
//! # Accessibility
//!
//! The AccessibilityAPI is a multiple-apply schema providing label,
//! description, and priority for assistive technologies.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUI/` module.

mod accessibility_api;
mod attribute_hints;
mod backdrop;
mod node_graph_node_api;
mod object_hints;
mod prim_hints;
mod property_hints;
mod scene_graph_prim_api;
mod tokens;

// Public re-exports
pub use accessibility_api::{AccessibilityAPI, Priority};
pub use attribute_hints::AttributeHints;
pub use backdrop::Backdrop;
pub use node_graph_node_api::{ExpansionState, NodeGraphNodeAPI};
pub use object_hints::{HintKeys, ObjectHints};
pub use prim_hints::PrimHints;
pub use property_hints::PropertyHints;
pub use scene_graph_prim_api::SceneGraphPrimAPI;
pub use tokens::{USD_UI_TOKENS, UsdUITokensType};
