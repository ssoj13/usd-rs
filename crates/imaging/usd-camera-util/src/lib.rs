//! Camera utilities for USD imaging.
//!
//! This module provides utilities for working with cameras in the context of rendering:
//!
//! - **Window conforming**: Adjust camera frustums to match target aspect ratios
//! - **Framing**: Define how filmback maps to pixels and what pixels to render
//! - **Screen window parameters**: Compute RenderMan-compatible rendering parameters

mod conform_window;
mod framing;
mod screen_window_parameters;

// Re-export public API
pub use conform_window::{
    ConformWindowPolicy, conform_camera, conform_frustum, conform_window_matrix,
    conform_window_range2d, conform_window_range2f, conform_window_vec2, conform_window_vec4,
};
pub use framing::Framing;
pub use screen_window_parameters::ScreenWindowParameters;

// Convenience type aliases matching C++ naming
pub use ConformWindowPolicy as CameraUtilConformWindowPolicy;
pub use Framing as CameraUtilFraming;
pub use ScreenWindowParameters as CameraUtilScreenWindowParameters;
