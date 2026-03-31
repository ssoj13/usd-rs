//! # UsdImagingGL - OpenGL/Hydra Rendering for USD
//!
//! This module provides the main API for rendering USD scenes using OpenGL
//! and Pixar's Hydra rendering framework.
//!
//! ## Overview
//!
//! `usdImagingGL` is the primary interface for applications that want to render
//! USD content. It provides:
//!
//! - High-level rendering API through [`Engine`]
//! - Configurable rendering parameters via [`RenderParams`]
//! - Renderer-specific settings through [`RendererSetting`]
//! - Picking and selection support
//! - Progressive rendering for ray tracers
//!
//! ## Main Components
//!
//! - [`Engine`]: The main rendering engine that manages Hydra delegates and scene indices
//! - [`EngineParameters`]: Configuration for engine construction
//! - [`RenderParams`]: Per-frame rendering parameters
//! - [`DrawMode`]: Geometry drawing modes (points, wireframe, shaded, etc.)
//! - [`CullStyle`]: Backface culling options
//! - [`RendererSetting`]: Renderer-specific configuration options
//!
//! ## Example Usage
//!
//! ```ignore
//! use usd_imaging::gl::{Engine, EngineParameters, RenderParams, DrawMode};
//! use usd_core::{Stage, common::InitialLoadSet};
//!
//! // Create engine with default settings
//! let engine_params = EngineParameters::new()
//!     .with_gpu_enabled(true);
//! let mut engine = Engine::new(engine_params);
//!
//! // Open a USD stage
//! let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)
//!     .expect("Failed to open stage");
//! let root = stage.get_pseudo_root();
//!
//! // Configure rendering
//! let render_params = RenderParams::new()
//!     .with_draw_mode(DrawMode::ShadedSmooth)
//!     .with_lighting(true)
//!     .with_complexity(1.5);
//!
//! // Render the scene
//! engine.render(&root, &render_params);
//!
//! // Check if progressive rendering is complete
//! if !engine.is_converged() {
//!     // Need more rendering passes
//!     engine.render(&root, &render_params);
//! }
//! ```
//!
//! ## Batch Rendering
//!
//! For rendering multiple objects efficiently:
//!
//! ```ignore
//! use usd_imaging::gl::{Engine, EngineParameters, RenderParams};
//! use usd_sdf::Path;
//! use usd_core::{Stage, common::InitialLoadSet};
//!
//! let mut engine = Engine::with_defaults();
//! let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)
//!     .expect("Failed to open");
//! let root = stage.get_pseudo_root();
//! let params = RenderParams::default();
//!
//! // Each frame: `PrepareBatch` then `RenderBatch` (C++ `UsdImagingGLEngine::Render`).
//! // `Engine::render` calls both; if you call `render_batch` alone, run `prepare_batch` first
//! // with an up-to-date `RenderParams::frame`.
//! engine.prepare_batch(&root, &params);
//!
//! // Render specific paths
//! let paths = vec![
//!     Path::from_string("/World/Cube").expect("valid path"),
//!     Path::from_string("/World/Sphere").expect("valid path"),
//! ];
//! engine.render_batch(&paths, &params);
//! ```
//!
//! ## Selection and Picking
//!
//! ```ignore
//! use usd_imaging::gl::{Engine, PickParams};
//! use usd_gf::{Matrix4d, Vec4f};
//! use usd_sdf::Path;
//!
//! let mut engine = Engine::with_defaults();
//!
//! // Set selected prims for highlighting
//! let selected = vec![
//!     Path::from_string("/World/Cube").expect("valid path"),
//! ];
//! engine.set_selected(selected);
//!
//! // Configure selection color
//! engine.set_selection_color(Vec4f::new(1.0, 1.0, 0.0, 1.0));
//! ```
//!
//! ## API Version
//!
//! This module implements UsdImagingGL API version 11.
//! See [`version::API_VERSION`] for details.

// Module declarations
pub mod engine;
pub mod render_index_backend;
pub mod render_params;
pub mod renderer_settings;
pub mod version;

// Re-exports
pub use engine::{Engine, EngineParameters, IntersectionResult, PickParams};
pub use render_params::{CullStyle, DrawMode, RenderParams};
pub use renderer_settings::{RendererSetting, RendererSettingType, RendererSettingsList};
pub use version::API_VERSION;
