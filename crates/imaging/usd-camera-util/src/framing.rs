//! Camera framing information.
//!
//! Framing determines how the filmback plane maps to pixels and what pixels
//! will be filled by the renderer.

use super::conform_window::{ConformWindowPolicy, conform_window_range2f};
use usd_gf::{Matrix4d, Range2f, Rect2i, Vec2f, Vec2i, Vec3d, Vec4d};

/// Framing information for camera rendering.
///
/// Determines how the filmback plane of a camera maps to the pixels of the rendered
/// image (via displayWindow, pixelAspectRatio, and window policy) and which pixels
/// will be filled by the renderer (dataWindow).
///
/// The concepts are similar to OpenEXR:
/// - displayWindow: The rectangle in pixel space that corresponds to the camera's filmback
/// - dataWindow: The rectangle of pixels that will actually be rendered
/// - pixelAspectRatio: Width/height ratio of a pixel (1.0 = square pixels)
///
/// # Overscan
///
/// Overscan can be achieved by making the dataWindow larger than the displayWindow.
///
/// # Window Policy
///
/// If the aspect ratios differ between displayWindow and the camera's filmback,
/// a window policy is applied to determine the mapping. For example, with `Fit` policy,
/// the largest rectangle that fits into displayWindow with the camera's aspect ratio
/// is used.
///
/// # Examples
///
/// ```ignore
/// use usd_camera_util::{Framing, ConformWindowPolicy};
/// use usd_gf::{Range2f, Rect2i, Vec2f, Vec2i};
///
/// // Create framing with 1920x1080 resolution
/// let display_window = Range2f::new(
///     Vec2f::new(0.0, 0.0),
///     Vec2f::new(1920.0, 1080.0)
/// );
/// let data_window = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(1919, 1079));
///
/// let framing = Framing::new(display_window, data_window, 1.0);
/// assert!(framing.is_valid());
///
/// // Compute filmback window for 16:9 camera
/// let camera_aspect = 16.0 / 9.0;
/// let filmback = framing.compute_filmback_window(
///     camera_aspect,
///     ConformWindowPolicy::Fit
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Framing {
    /// The display window in pixel space.
    /// Defines the rectangle that corresponds to the camera's filmback plane.
    pub display_window: Range2f,

    /// The data window in pixel coordinates.
    /// Defines which pixels will actually be rendered.
    pub data_window: Rect2i,

    /// The pixel aspect ratio (width / height).
    /// 1.0 means square pixels, 2.0 means pixels twice as wide as tall.
    pub pixel_aspect_ratio: f32,
}

impl Framing {
    /// Creates an invalid framing with empty windows.
    pub fn new_empty() -> Self {
        Self {
            display_window: Range2f::new(Vec2f::new(0.0, 0.0), Vec2f::new(0.0, 0.0)),
            data_window: Rect2i::new(Vec2i::new(0, 0), Vec2i::new(-1, -1)),
            pixel_aspect_ratio: 1.0,
        }
    }

    /// Creates a framing with the given display window, data window, and pixel aspect ratio.
    ///
    /// # Arguments
    ///
    /// * `display_window` - The display window in pixel space.
    /// * `data_window` - The data window in pixel coordinates.
    /// * `pixel_aspect_ratio` - The pixel aspect ratio (default: 1.0).
    pub fn new(display_window: Range2f, data_window: Rect2i, pixel_aspect_ratio: f32) -> Self {
        Self {
            display_window,
            data_window,
            pixel_aspect_ratio,
        }
    }

    /// Creates a framing with equal display and data windows (square pixels assumed).
    ///
    /// # Arguments
    ///
    /// * `data_window` - The data window in pixel coordinates.
    pub fn from_data_window(data_window: Rect2i) -> Self {
        let display_window = Range2f::new(
            Vec2f::new(data_window.min_x() as f32, data_window.min_y() as f32),
            Vec2f::new(
                (data_window.max_x() + 1) as f32,
                (data_window.max_y() + 1) as f32,
            ),
        );
        Self::new(display_window, data_window, 1.0)
    }

    /// Checks if the framing is valid (non-empty windows and non-zero pixel aspect ratio).
    pub fn is_valid(&self) -> bool {
        !self.data_window.is_empty()
            && !self.display_window.is_empty()
            && self.pixel_aspect_ratio != 0.0
    }

    /// Computes the filmback window in pixel space.
    ///
    /// The filmback window is the rectangle corresponding to the camera's filmback plane
    /// after applying the window policy to conform to the display window's aspect ratio.
    ///
    /// # Arguments
    ///
    /// * `camera_aspect_ratio` - The camera's aspect ratio (width / height).
    /// * `window_policy` - The policy for conforming the window.
    ///
    /// # Returns
    ///
    /// The filmback window as a Range2f in pixel space.
    pub fn compute_filmback_window(
        &self,
        camera_aspect_ratio: f32,
        window_policy: ConformWindowPolicy,
    ) -> Range2f {
        // Invert policy: if camera needs to Fit, filmback needs to Crop and vice versa
        let inverted_policy = invert_policy(window_policy);

        conform_window_range2f(
            self.display_window,
            inverted_policy,
            safe_div(camera_aspect_ratio, self.pixel_aspect_ratio),
        )
    }

    /// Applies the framing to a projection matrix.
    ///
    /// Takes a projection matrix computed from a camera and applies the framing information.
    /// To obtain correct results, the rasterizer should use the resulting projection matrix
    /// and set the viewport to the data window.
    ///
    /// # Arguments
    ///
    /// * `projection_matrix` - The original projection matrix from the camera.
    /// * `window_policy` - The policy for conforming the window.
    ///
    /// # Returns
    ///
    /// The adjusted projection matrix.
    pub fn apply_to_projection_matrix(
        &self,
        projection_matrix: Matrix4d,
        window_policy: ConformWindowPolicy,
    ) -> Matrix4d {
        let disp_size = self.display_window.size();
        let data_size = self.data_window.size();
        let aspect = (self.pixel_aspect_ratio as f64)
            * safe_div_f64(disp_size[0] as f64, disp_size[1] as f64);

        let t =
            2.0 * (compute_center(&self.display_window) - compute_center_rect(&self.data_window));

        // Apply transformations in order:
        // 1. Conform frustum to display window aspect ratio
        let conformed =
            super::conform_window::conform_window_matrix(projection_matrix, window_policy, aspect);

        // 2. Transform NDC to space where unit is two pixels
        let scale = Matrix4d::from_diagonal_vec(&Vec4d::new(
            disp_size[0] as f64,
            disp_size[1] as f64,
            1.0,
            1.0,
        ));

        // 3. Apply translation (note: y-axis flips between eye space and window space)
        let translate = Matrix4d::from_translation(Vec3d::new(t[0] as f64, -(t[1] as f64), 0.0));

        // 4. Transform from pixel to NDC with respect to data window
        let to_ndc = Matrix4d::from_diagonal_vec(&Vec4d::new(
            1.0 / data_size[0] as f64,
            1.0 / data_size[1] as f64,
            1.0,
            1.0,
        ));

        conformed * scale * translate * to_ndc
    }
}

impl Default for Framing {
    fn default() -> Self {
        Self::new_empty()
    }
}

// Helper: compute center of Range2f
fn compute_center(window: &Range2f) -> Vec2f {
    let min = *window.min();
    let size = window.size();
    Vec2f::new(min.x + 0.5 * size.x, min.y + 0.5 * size.y)
}

// Helper: compute center of Rect2i
fn compute_center_rect(window: &Rect2i) -> Vec2f {
    Vec2f::new(
        window.min_x() as f32 + 0.5 * window.size()[0] as f32,
        window.min_y() as f32 + 0.5 * window.size()[1] as f32,
    )
}

// Helper: safe division for f64
fn safe_div_f64(a: f64, b: f64) -> f64 {
    if b == 0.0 { 1.0 } else { a / b }
}

// Helper: safe division for f32
fn safe_div(a: f32, b: f32) -> f32 {
    if b == 0.0 { 1.0 } else { a / b }
}

// Helper: invert Fit <-> Crop policy
fn invert_policy(policy: ConformWindowPolicy) -> ConformWindowPolicy {
    match policy {
        ConformWindowPolicy::Fit => ConformWindowPolicy::Crop,
        ConformWindowPolicy::Crop => ConformWindowPolicy::Fit,
        _ => policy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framing_new_empty() {
        let framing = Framing::new_empty();
        assert!(!framing.is_valid());
    }

    #[test]
    fn test_framing_from_data_window() {
        let data_window = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(1919, 1079));
        let framing = Framing::from_data_window(data_window);

        assert!(framing.is_valid());
        assert_eq!(framing.data_window, data_window);
        assert!((framing.display_window.min()[0] - 0.0).abs() < 1e-6);
        assert!((framing.display_window.max()[0] - 1920.0).abs() < 1e-6);
        assert!((framing.display_window.min()[1] - 0.0).abs() < 1e-6);
        assert!((framing.display_window.max()[1] - 1080.0).abs() < 1e-6);
    }

    #[test]
    fn test_framing_is_valid() {
        let valid = Framing::new(
            Range2f::new(Vec2f::new(0.0, 0.0), Vec2f::new(100.0, 100.0)),
            Rect2i::new(Vec2i::new(0, 0), Vec2i::new(99, 99)),
            1.0,
        );
        assert!(valid.is_valid());

        let invalid = Framing::new(
            Range2f::new(Vec2f::new(0.0, 0.0), Vec2f::new(0.0, 0.0)),
            Rect2i::new(Vec2i::new(0, 0), Vec2i::new(-1, -1)),
            1.0,
        );
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_compute_filmback_window() {
        let framing = Framing::new(
            Range2f::new(Vec2f::new(0.0, 0.0), Vec2f::new(1920.0, 1080.0)),
            Rect2i::new(Vec2i::new(0, 0), Vec2i::new(1919, 1079)),
            1.0,
        );

        let camera_aspect = 16.0 / 9.0;
        let filmback = framing.compute_filmback_window(camera_aspect, ConformWindowPolicy::Fit);

        // Display window already has 16:9 aspect, so filmback should match
        assert!((filmback.min()[0] - 0.0).abs() < 1e-3);
        assert!((filmback.max()[0] - 1920.0).abs() < 1e-3);
    }
}
