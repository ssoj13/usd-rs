//! UsdAppUtils - USD Application Utilities.
//!
//! Port of pxr/usdImaging/usdAppUtils
//!
//! UsdAppUtils contains utilities and common functionality for applications
//! that view and/or record images of USD stages.
//!
//! # Modules
//!
//! - [`camera`] - Camera utilities for finding and working with USD cameras
//! - [`frame_recorder`] - Frame recording for playblasts and rendering
//!
//! # Camera Utilities
//!
//! The camera module provides utilities for locating cameras on a USD stage:
//!
//! ```rust,ignore
//! use usd_app_utils::get_camera_at_path;
//! use usd_core::{Stage, InitialLoadSet};
//! use usd_sdf::Path;
//!
//! let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)?;
//!
//! // Get camera by absolute path
//! let camera = get_camera_at_path(&stage, &Path::from_string("/cameras/main")?);
//!
//! // Get camera by name (searches entire stage)
//! let camera = get_camera_at_path(&stage, &Path::from_string("main")?);
//! ```
//!
//! # Frame Recording
//!
//! The frame recorder provides functionality for rendering USD stages to images:
//!
//! ```rust,ignore
//! use usd_app_utils::FrameRecorder;
//! use usd_core::{Stage, InitialLoadSet, TimeCode};
//! use usd_geom::Camera;
//! use usd_sdf::Path;
//!
//! let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)?;
//! let camera = Camera::get(&stage, &Path::from_string("/cameras/main")?);
//!
//! let recorder = FrameRecorder::builder()
//!     .image_width(1920)
//!     .complexity(2.0)
//!     .camera_light_enabled(true)
//!     .build()?;
//!
//! recorder.record(&stage, &camera, TimeCode::default(), "output.png")?;
//! ```
//!
pub mod camera;
pub mod frame_recorder;

// Re-export main types and functions
pub use camera::get_camera_at_path;
pub use frame_recorder::{FrameRecorder, FrameRecorderBuilder};
