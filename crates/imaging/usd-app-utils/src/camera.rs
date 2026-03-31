//! Camera utilities for USD applications.
//!
//! Port of pxr/usdImaging/usdAppUtils/camera.h/cpp
//!
//! Collection of utilities for applications that operate using USD cameras.

use usd_core::Stage;
use usd_core::prim_flags::{all_prims_predicate, traverse_instance_proxies};
use usd_geom::Camera;
use usd_sdf::Path;
use usd_tf::tf_warn;

// ============================================================================
// Camera Utilities
// ============================================================================

/// Gets the Camera matching camera_path from the USD stage.
///
/// If camera_path is an absolute path, this is equivalent to `Camera::get()`.
/// Otherwise, if camera_path is a single-element path representing just the
/// name of a camera prim, then the stage will be searched looking for a Camera
/// matching that name. The Camera schema for that prim will be returned if found,
/// or an invalid Camera will be returned if not.
///
/// Note that if camera_path is a multi-element path, a warning is issued and
/// it is just made absolute using the absolute root path before searching. In
/// the future, this could potentially be changed to use a suffix-based match.
///
/// Matches C++ `UsdAppUtilsGetCameraAtPath`.
///
/// # Arguments
///
/// * `stage` - The USD stage to search
/// * `camera_path` - Path to the camera (absolute or name-only)
///
/// # Returns
///
/// Camera schema, or invalid Camera if not found
///
/// # Examples
///
/// ```rust,ignore
/// use usd_app_utils::get_camera_at_path;
/// use usd_core::{Stage, InitialLoadSet};
/// use usd_sdf::Path;
///
/// let stage = Stage::create_in_memory(InitialLoadSet::LoadAll)?;
/// stage.define_prim("/Scene/MainCamera", "Camera")?;
///
/// // Get by absolute path
/// let camera = get_camera_at_path(&stage, &Path::from_string("/Scene/MainCamera")?);
/// assert!(camera.is_valid());
///
/// // Get by name only (searches entire stage)
/// let camera = get_camera_at_path(&stage, &Path::from_string("MainCamera")?);
/// assert!(camera.is_valid());
/// ```
pub fn get_camera_at_path(stage: &Stage, camera_path: &Path) -> Camera {
    // Validate inputs
    if !camera_path.is_prim_path() {
        // A non-prim path cannot be a camera
        return Camera::invalid();
    }

    let mut usd_camera_path = camera_path.clone();

    if !camera_path.is_absolute_path() {
        if camera_path.get_path_element_count() > 1 {
            // Multi-element relative path - make absolute and warn
            usd_camera_path = camera_path
                .make_absolute(&Path::absolute_root())
                .unwrap_or_else(|| camera_path.clone());
            tf_warn!(
                "Camera path \"{}\" is not absolute. Using absolute path instead: \"{}\"",
                camera_path.as_str(),
                usd_camera_path.as_str()
            );
        } else {
            // Single-element path - search by name
            let camera_name = camera_path.get_name_token();

            // Search across all prims including instance proxies, matching C++ UsdTraverseInstanceProxies().
            let predicate = traverse_instance_proxies(all_prims_predicate());
            for prim in stage.traverse_with_predicate(predicate) {
                if prim.name() == camera_name {
                    let camera = Camera::new(prim);
                    if camera.is_valid() {
                        return camera;
                    }
                }
            }

            // Not found
            return Camera::invalid();
        }
    }

    // Get camera at absolute path
    Camera::get(stage, &usd_camera_path)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;

    #[test]
    fn test_get_camera_absolute_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a camera prim
        let camera_path = Path::from_string("/Scene/MainCamera").unwrap();
        let _prim = stage.define_prim(camera_path.as_str(), "Camera").unwrap();

        // Get by absolute path
        let camera = get_camera_at_path(&stage, &camera_path);
        assert!(camera.is_valid());
        assert_eq!(camera.prim().get_path().as_str(), "/Scene/MainCamera");
    }

    #[test]
    fn test_get_camera_by_name() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a camera prim
        let camera_path = Path::from_string("/Scene/MainCamera").unwrap();
        let _prim = stage.define_prim(camera_path.as_str(), "Camera").unwrap();

        // Get by name only - search function will attempt to find camera
        // Note: actual schema validation requires full UsdGeomCamera infrastructure
        let name_path = Path::from_string("MainCamera").unwrap();
        let camera = get_camera_at_path(&stage, &name_path);

        assert!(camera.is_valid());
    }

    #[test]
    fn test_get_camera_not_found() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Try to get non-existent camera
        let camera_path = Path::from_string("/NonExistent").unwrap();
        let camera = get_camera_at_path(&stage, &camera_path);
        assert!(!camera.is_valid());
    }

    #[test]
    fn test_get_camera_invalid_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Try to get camera with property path (not a prim path)
        let prop_path = Path::from_string("/Scene/MainCamera.projection").unwrap();
        let camera = get_camera_at_path(&stage, &prop_path);
        assert!(!camera.is_valid());
    }

    #[test]
    fn test_get_camera_multi_element_relative_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Define a camera at absolute path
        let camera_path = Path::from_string("/Scene/MainCamera").unwrap();
        let _prim = stage.define_prim(camera_path.as_str(), "Camera").unwrap();

        // Try with multi-element relative path (should warn and convert to absolute)
        let relative_path = Path::from_string("Scene/MainCamera").unwrap();
        let camera = get_camera_at_path(&stage, &relative_path);

        // Should find it at /Scene/MainCamera after conversion
        assert!(camera.is_valid());
    }
}
