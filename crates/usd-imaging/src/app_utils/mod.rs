//! UsdAppUtils - Utility classes for USD application support.
//!
//! Port of pxr/usdImaging/usdAppUtils
//!
//! This module provides utility classes for applications that work with USD,
//! including camera utilities and frame recording functionality.
//!
//! # Components
//!
//! - [`camera`] - Camera lookup utilities
//! - [`frame_recorder`] - Frame recording for generating images from USD stages
//!
//! # Example
//!
//! ```rust,ignore
//! use usd_imaging::app_utils::{get_camera_at_path, FrameRecorder};
//!
//! // Find a camera by name
//! let camera = get_camera_at_path(&stage, &SdfPath::new("/Camera"));
//!
//! // Record a frame
//! let mut recorder = FrameRecorder::new(None, true, true);
//! recorder.record(&stage, &camera, TimeCode::default(), "output.png");
//! ```

pub mod camera;
pub mod frame_recorder;

pub use camera::get_camera_at_path;
pub use frame_recorder::FrameRecorder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_imports() {
        // Verify module structure
        let _ = get_camera_at_path;
    }
}
