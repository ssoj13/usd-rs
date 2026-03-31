//! Camera utilities for USD applications.
//!
//! Port of pxr/usdImaging/usdAppUtils/camera.h
//!
//! Collection of module-scoped utilities for applications that operate using
//! USD cameras.

use usd_core::Stage;
use usd_geom::Camera;
use usd_sdf::Path;

/// Gets the UsdGeomCamera matching `camera_path` from the USD stage.
///
/// If `camera_path` is an absolute path, this is equivalent to
/// `Camera::get()`. Otherwise, if `camera_path` is a single-element path
/// representing just the name of a camera prim, then the stage will be searched
/// looking for a UsdGeomCamera matching that name.
///
/// # Arguments
/// * `stage` - The USD stage to search
/// * `camera_path` - The path to the camera (absolute or relative name)
///
/// # Returns
/// The camera if found, or None if not found.
///
/// # Example
///
/// ```rust,ignore
/// use usd_imaging::app_utils::get_camera_at_path;
///
/// // Get camera by absolute path
/// let camera = get_camera_at_path(&stage, &SdfPath::new("/World/Camera"));
///
/// // Get camera by name only
/// let camera = get_camera_at_path(&stage, &SdfPath::new("Camera"));
/// ```
pub fn get_camera_at_path(stage: &Stage, camera_path: &Path) -> Option<Camera> {
    if camera_path.is_empty() {
        return None;
    }

    // If it's an absolute path, get directly
    if camera_path.is_absolute_path() {
        if let Some(prim) = stage.get_prim_at_path(camera_path) {
            if prim.is_valid() {
                let camera = Camera::get(stage, camera_path);
                if camera.prim().is_valid() {
                    return Some(camera);
                }
            }
        }
        return None;
    }

    // For relative paths, search the stage
    let camera_name = camera_path.get_name();
    if camera_name.is_empty() {
        return None;
    }

    // Search for a camera with matching name
    find_camera_by_name(stage, &camera_name)
}

/// Search the stage for a camera with the given name.
fn find_camera_by_name(stage: &Stage, name: &str) -> Option<Camera> {
    // Traverse all prims looking for a camera with matching name
    let root = stage.get_pseudo_root();
    find_camera_recursive(&root, name, stage)
}

/// Recursively search for a camera.
fn find_camera_recursive(prim: &usd_core::Prim, name: &str, stage: &Stage) -> Option<Camera> {
    // Check if this prim is a camera with matching name
    if prim.name() == name {
        let camera = Camera::get(stage, &prim.get_path());
        if camera.prim().is_valid() {
            return Some(camera);
        }
    }

    // Search children
    for child in prim.get_children() {
        if let Some(camera) = find_camera_recursive(&child, name, stage) {
            return Some(camera);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;

    #[test]
    fn test_get_camera_empty_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let result = get_camera_at_path(&stage, &Path::empty());
        assert!(result.is_none());
    }

    #[test]
    fn test_get_camera_invalid_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let path = Path::from_string("/NonExistent/Camera").unwrap();
        let result = get_camera_at_path(&stage, &path);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_camera_by_name_not_found() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let path = Path::from_string("NonExistentCamera").unwrap();
        let result = get_camera_at_path(&stage, &path);
        assert!(result.is_none());
    }
}
