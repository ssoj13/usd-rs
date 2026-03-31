//! Window conforming policies and functions.
//!
//! This module provides utilities for conforming windows to a target aspect ratio.
//! The conforming policy determines how the window dimensions are adjusted to match
//! the target aspect ratio.

use usd_gf::{Matrix4d, Range2d, Range2f, Vec2d, Vec4d};

/// Policy for conforming a window to a target aspect ratio.
///
/// Determines how a window's dimensions are adjusted to match a desired aspect ratio.
///
/// # ASCII Art Examples
///
/// ```text
///                 Original window:
///
///                        w
///                 |<----------->|
///
///                 ***************  ---
///                 *   O     o   *   ^
///                 * --|-- --|-- *   | h
///                 *   |     |   *   |
///                 *  / \   / \  *   v
///                 ***************  ---
///
///
/// When target aspect > original aspect:
///
/// MatchVertically:
///  ******************* ---
///  *     O     o     *  ^
///  *   --|-- --|--   *  | h (unchanged)
///  *     |     |     *  |
///  *    / \   / \    *  v
///  ******************* ---
///
/// MatchHorizontally:
///           w
///    |<----------->|
///    ***************
///    * --|-- --|-- *
///    *   |     |   *
///    ***************
///
/// Fit:
///  ******************* ---
///  *     O     o     *  ^
///  *   --|-- --|--   *  | h
///  *     |     |     *  |
///  *    / \   / \    *  v
///  ******************* ---
///
/// Crop:
///           w
///    |<----------->|
///    ***************
///    * --|-- --|-- *
///    *   |     |   *
///    ***************
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConformWindowPolicy {
    /// Modify width to match target aspect ratio (keep height constant).
    MatchVertically,
    /// Modify height to match target aspect ratio (keep width constant).
    MatchHorizontally,
    /// Increase width or height to fit target aspect ratio (never decrease).
    Fit,
    /// Decrease width or height to crop to target aspect ratio (never increase).
    Crop,
    /// Leave unchanged (can result in stretching/shrinking if aspect ratios differ).
    DontConform,
}

/// Returns a window with the specified target aspect ratio by applying the given policy.
///
/// The window is represented as a 2D vector (width, height).
///
/// # Arguments
///
/// * `window` - The original window dimensions as (width, height).
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
///
/// # Returns
///
/// A new Vec2d with dimensions adjusted according to the policy.
pub fn conform_window_vec2(
    window: Vec2d,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) -> Vec2d {
    if policy == ConformWindowPolicy::DontConform {
        return window;
    }

    let resolved_policy = resolve_conform_window_policy(window, policy, target_aspect);

    if resolved_policy == ConformWindowPolicy::MatchHorizontally {
        Vec2d::new(window[0], safe_div(window[0], target_aspect))
    } else {
        Vec2d::new(window[1] * target_aspect, window[1])
    }
}

/// Returns a window with the specified target aspect ratio by applying the given policy.
///
/// The window is represented as a Range2d.
///
/// # Arguments
///
/// * `window` - The original window as a 2D range.
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
///
/// # Returns
///
/// A new Range2d with dimensions adjusted according to the policy.
pub fn conform_window_range2d(
    window: Range2d,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) -> Range2d {
    if policy == ConformWindowPolicy::DontConform {
        return window;
    }

    let size = window.size();
    let center = (*window.min() + *window.max()) / 2.0;

    let resolved_policy = resolve_conform_window_policy(size, policy, target_aspect);

    if resolved_policy == ConformWindowPolicy::MatchHorizontally {
        let height = safe_div(size[0], target_aspect);
        Range2d::new(
            Vec2d::new(window.min()[0], center[1] - height / 2.0),
            Vec2d::new(window.max()[0], center[1] + height / 2.0),
        )
    } else {
        let width = size[1] * target_aspect;
        Range2d::new(
            Vec2d::new(center[0] - width / 2.0, window.min()[1]),
            Vec2d::new(center[0] + width / 2.0, window.max()[1]),
        )
    }
}

/// Returns a window with the specified target aspect ratio by applying the given policy.
///
/// The window is represented as a Range2f.
///
/// # Arguments
///
/// * `window` - The original window as a 2D range (f32 precision).
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
///
/// # Returns
///
/// A new Range2f with dimensions adjusted according to the policy.
pub fn conform_window_range2f(
    window: Range2f,
    policy: ConformWindowPolicy,
    target_aspect: f32,
) -> Range2f {
    if policy == ConformWindowPolicy::DontConform {
        return window;
    }

    let size = window.size();
    let center = (*window.min() + *window.max()) / 2.0;

    let resolved_policy = resolve_conform_window_policy_f32(size, policy, target_aspect);

    if resolved_policy == ConformWindowPolicy::MatchHorizontally {
        let height = safe_div_f32(size[0], target_aspect);
        Range2f::new(
            usd_gf::Vec2f::new(window.min()[0], center[1] - height / 2.0),
            usd_gf::Vec2f::new(window.max()[0], center[1] + height / 2.0),
        )
    } else {
        let width = size[1] * target_aspect;
        Range2f::new(
            usd_gf::Vec2f::new(center[0] - width / 2.0, window.min()[1]),
            usd_gf::Vec2f::new(center[0] + width / 2.0, window.max()[1]),
        )
    }
}

/// Returns a window with the specified target aspect ratio by applying the given policy.
///
/// The window is encoded as a vector (left, right, bottom, top) similar to RenderMan's RiScreenWindow.
///
/// # Arguments
///
/// * `window` - The original window as Vec4d (left, right, bottom, top).
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
///
/// # Returns
///
/// A new Vec4d with dimensions adjusted according to the policy.
pub fn conform_window_vec4(
    window: Vec4d,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) -> Vec4d {
    if policy == ConformWindowPolicy::DontConform {
        return window;
    }

    let original = Range2d::new(
        Vec2d::new(window[0], window[2]),
        Vec2d::new(window[1], window[3]),
    );

    let conformed = conform_window_range2d(original, policy, target_aspect);

    Vec4d::new(
        conformed.min()[0],
        conformed.max()[0],
        conformed.min()[1],
        conformed.max()[1],
    )
}

/// Conforms the given projection matrix to have the specified target aspect ratio.
///
/// Applies the conforming policy to adjust the projection matrix's aspect ratio.
/// Supports mirroring about the x- or y-axis.
///
/// # Arguments
///
/// * `projection_matrix` - The original projection matrix.
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
///
/// # Returns
///
/// A new Matrix4d with adjusted aspect ratio.
pub fn conform_window_matrix(
    projection_matrix: Matrix4d,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) -> Matrix4d {
    if policy == ConformWindowPolicy::DontConform {
        return projection_matrix;
    }

    let mut result = projection_matrix;

    // Extract aspect ratio from projection matrix diagonal
    let window = Vec2d::new(projection_matrix[1][1].abs(), projection_matrix[0][0].abs());

    let resolved_policy = resolve_conform_window_policy(window, policy, target_aspect);

    if resolved_policy == ConformWindowPolicy::MatchHorizontally {
        // Adjust vertical size
        result[1][1] = sign(projection_matrix[1][1]) * window[1] * target_aspect;

        // Scale factor for asymmetric frustum adjustment
        let scale_factor = safe_div(result[1][1], projection_matrix[1][1]);

        // Apply to offsets (important for asymmetric frustums)
        result[2][1] *= scale_factor;
        result[3][1] *= scale_factor;
    } else {
        // Adjust horizontal size
        result[0][0] = sign(projection_matrix[0][0]) * safe_div(window[0], target_aspect);

        let scale_factor = safe_div(result[0][0], projection_matrix[0][0]);

        result[2][0] *= scale_factor;
        result[3][0] *= scale_factor;
    }

    result
}

// Helper: safe division returning 1.0 if divisor is zero
#[inline]
fn safe_div_one(a: f64, b: f64) -> f64 {
    if b != 0.0 { a / b } else { 1.0 }
}

// Helper: safe division returning dividend if divisor is zero
#[inline]
fn safe_div(a: f64, b: f64) -> f64 {
    if b != 0.0 { a / b } else { a }
}

// Helper: safe division for f32
#[inline]
fn safe_div_f32(a: f32, b: f32) -> f32 {
    if b != 0.0 { a / b } else { a }
}

// Helper: get sign of a number
#[inline]
fn sign(x: f64) -> f64 {
    if x < 0.0 { -1.0 } else { 1.0 }
}

// Resolve Fit/Crop policies to MatchVertically/MatchHorizontally
fn resolve_conform_window_policy(
    size: Vec2d,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) -> ConformWindowPolicy {
    if policy == ConformWindowPolicy::MatchVertically
        || policy == ConformWindowPolicy::MatchHorizontally
    {
        return policy;
    }

    let aspect = safe_div_one(size[0], size[1]);

    // XOR logic: (Fit && aspect <= target) || (Crop && aspect > target)
    if (policy == ConformWindowPolicy::Fit) ^ (aspect > target_aspect) {
        ConformWindowPolicy::MatchVertically
    } else {
        ConformWindowPolicy::MatchHorizontally
    }
}

// Resolve for f32 (Range2f)
fn resolve_conform_window_policy_f32(
    size: usd_gf::Vec2f,
    policy: ConformWindowPolicy,
    target_aspect: f32,
) -> ConformWindowPolicy {
    if policy == ConformWindowPolicy::MatchVertically
        || policy == ConformWindowPolicy::MatchHorizontally
    {
        return policy;
    }

    let aspect = if size[1] != 0.0 {
        size[0] / size[1]
    } else {
        1.0
    };

    if (policy == ConformWindowPolicy::Fit) ^ (aspect > target_aspect) {
        ConformWindowPolicy::MatchVertically
    } else {
        ConformWindowPolicy::MatchHorizontally
    }
}

/// Conforms the given camera to have the specified target aspect ratio.
///
/// Adjusts the camera's aperture dimensions in-place according to the policy.
/// This is the C++ `CameraUtilConformWindow(GfCamera*, policy, targetAspect)` overload.
///
/// # Arguments
///
/// * `camera` - The camera to conform (modified in-place).
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
pub fn conform_camera(
    camera: &mut usd_gf::Camera,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) {
    if policy == ConformWindowPolicy::DontConform {
        return;
    }

    let window = Vec2d::new(
        camera.horizontal_aperture() as f64,
        camera.vertical_aperture() as f64,
    );
    let conformed = conform_window_vec2(window, policy, target_aspect);

    // C++ CameraUtilConformWindow(GfCamera*) only sets aperture values — no offset scaling.
    camera.set_horizontal_aperture(conformed[0] as f32);
    camera.set_vertical_aperture(conformed[1] as f32);
}

/// Conforms the given frustum to have the specified target aspect ratio.
///
/// Adjusts the frustum's window in-place according to the policy.
/// This is the C++ `CameraUtilConformWindow(GfFrustum*, policy, targetAspect)` overload.
///
/// # Arguments
///
/// * `frustum` - The frustum to conform (modified in-place).
/// * `policy` - The conforming policy to apply.
/// * `target_aspect` - The desired aspect ratio (width / height).
pub fn conform_frustum(
    frustum: &mut usd_gf::Frustum,
    policy: ConformWindowPolicy,
    target_aspect: f64,
) {
    if policy == ConformWindowPolicy::DontConform {
        return;
    }

    let conformed = conform_window_range2d(frustum.window().clone(), policy, target_aspect);
    frustum.set_window(conformed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conform_window_vec2_match_vertically() {
        let window = Vec2d::new(100.0, 100.0);
        let result = conform_window_vec2(window, ConformWindowPolicy::MatchVertically, 2.0);
        assert!((result[0] - 200.0).abs() < 1e-10);
        assert!((result[1] - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_conform_window_vec2_match_horizontally() {
        let window = Vec2d::new(100.0, 100.0);
        let result = conform_window_vec2(window, ConformWindowPolicy::MatchHorizontally, 2.0);
        assert!((result[0] - 100.0).abs() < 1e-10);
        assert!((result[1] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_conform_window_vec2_fit() {
        // Original aspect = 1.0, target aspect = 2.0
        // Fit increases size, so width should increase
        let window = Vec2d::new(100.0, 100.0);
        let result = conform_window_vec2(window, ConformWindowPolicy::Fit, 2.0);
        assert!((result[0] - 200.0).abs() < 1e-10);
        assert!((result[1] - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_conform_window_vec2_crop() {
        // Original aspect = 1.0, target aspect = 2.0
        // Crop decreases size, so height should decrease
        let window = Vec2d::new(100.0, 100.0);
        let result = conform_window_vec2(window, ConformWindowPolicy::Crop, 2.0);
        assert!((result[0] - 100.0).abs() < 1e-10);
        assert!((result[1] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_conform_window_vec2_dont_conform() {
        let window = Vec2d::new(100.0, 100.0);
        let result = conform_window_vec2(window, ConformWindowPolicy::DontConform, 2.0);
        assert_eq!(result, window);
    }

    #[test]
    fn test_conform_window_range2d() {
        let window = Range2d::new(Vec2d::new(-50.0, -50.0), Vec2d::new(50.0, 50.0));
        let result = conform_window_range2d(window, ConformWindowPolicy::MatchVertically, 2.0);

        assert!((result.min()[0] - (-100.0)).abs() < 1e-10);
        assert!((result.max()[0] - 100.0).abs() < 1e-10);
        assert!((result.min()[1] - (-50.0)).abs() < 1e-10);
        assert!((result.max()[1] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_conform_window_vec4() {
        let window = Vec4d::new(-50.0, 50.0, -50.0, 50.0);
        let result = conform_window_vec4(window, ConformWindowPolicy::MatchVertically, 2.0);

        assert!((result[0] - (-100.0)).abs() < 1e-10);
        assert!((result[1] - 100.0).abs() < 1e-10);
        assert!((result[2] - (-50.0)).abs() < 1e-10);
        assert!((result[3] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_conform_window_matrix() {
        let mut proj = Matrix4d::identity();
        proj[0][0] = 1.0;
        proj[1][1] = 1.0;

        let result = conform_window_matrix(proj, ConformWindowPolicy::MatchHorizontally, 2.0);

        // Vertical component should be scaled
        assert!((result[1][1] - 2.0).abs() < 1e-10);
        assert!((result[0][0] - 1.0).abs() < 1e-10);
    }
}
