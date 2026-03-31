
//! Free camera prim data source - Camera data from matrices or GfCamera.
//!
//! A data source conforming to HdCameraSchema and HdXformSchema,
//! populated from camera matrices and a window policy.
//! Intended to replace HdxFreeCameraSceneDelegate for Hydra 2.0.
//!
//! Port of pxr/imaging/hdx/freeCameraPrimDataSource.h/cpp

use usd_gf::{Matrix4d, Vec4f};
use usd_tf::Token;

use super::render_setup_task::CameraUtilConformWindowPolicy;

/// Free camera prim data source.
///
/// Provides camera data (xform, projection, clip planes) as a container
/// data source for use with scene indices (Hydra 2.0).
///
/// Can be initialized from:
/// - View + projection matrices
/// - A GfCamera (represented here as matrices + params)
///
/// Port of HdxFreeCameraPrimDataSource from pxr/imaging/hdx/freeCameraPrimDataSource.h
pub struct HdxFreeCameraPrimDataSource {
    /// Camera view (world-to-camera) matrix.
    view_matrix: Matrix4d,
    /// Camera projection matrix.
    projection_matrix: Matrix4d,
    /// Window conform policy.
    window_policy: CameraUtilConformWindowPolicy,
    /// Clipping planes (each Vec4f = plane equation ax+by+cz+d=0).
    clipping_planes: Vec<Vec4f>,
    /// Dirty locator tracking for incremental updates.
    version: u64,
}

impl HdxFreeCameraPrimDataSource {
    /// Create from view and projection matrices.
    pub fn from_matrices(
        view_matrix: Matrix4d,
        projection_matrix: Matrix4d,
        policy: CameraUtilConformWindowPolicy,
    ) -> Self {
        Self {
            view_matrix,
            projection_matrix,
            window_policy: policy,
            clipping_planes: Vec::new(),
            version: 0,
        }
    }

    /// Create with default identity matrices.
    pub fn new() -> Self {
        Self {
            view_matrix: Matrix4d::identity(),
            projection_matrix: Matrix4d::identity(),
            window_policy: CameraUtilConformWindowPolicy::Fit,
            clipping_planes: Vec::new(),
            version: 0,
        }
    }

    /// Set view and projection matrices.
    ///
    /// Returns dirty locator tokens that changed.
    pub fn set_matrices(
        &mut self,
        view_matrix: Matrix4d,
        projection_matrix: Matrix4d,
    ) -> Vec<Token> {
        self.view_matrix = view_matrix;
        self.projection_matrix = projection_matrix;
        self.version += 1;
        vec![Token::new("xform"), Token::new("camera")]
    }

    /// Set window conform policy.
    pub fn set_window_policy(&mut self, policy: CameraUtilConformWindowPolicy) -> Vec<Token> {
        self.window_policy = policy;
        self.version += 1;
        vec![Token::new("camera")]
    }

    /// Set clipping planes.
    pub fn set_clipping_planes(&mut self, clipping_planes: Vec<Vec4f>) -> Vec<Token> {
        self.clipping_planes = clipping_planes;
        self.version += 1;
        vec![Token::new("camera")]
    }

    /// Get data source value by name.
    ///
    /// Supports: "xform", "camera", "clippingPlanes"
    pub fn get(&self, name: &Token) -> Option<DataSourceValue> {
        match name.as_str() {
            "xform" => Some(DataSourceValue::Matrix(self.view_matrix)),
            "camera" => Some(DataSourceValue::Projection(self.projection_matrix)),
            "clippingPlanes" => Some(DataSourceValue::ClipPlanes(self.clipping_planes.clone())),
            _ => None,
        }
    }

    /// Get names of available data sources.
    pub fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new("xform"),
            Token::new("camera"),
            Token::new("clippingPlanes"),
        ]
    }

    /// Get view matrix.
    pub fn get_view_matrix(&self) -> &Matrix4d {
        &self.view_matrix
    }

    /// Get projection matrix.
    pub fn get_projection_matrix(&self) -> &Matrix4d {
        &self.projection_matrix
    }

    /// Get window policy.
    pub fn get_window_policy(&self) -> CameraUtilConformWindowPolicy {
        self.window_policy
    }

    /// Get clipping planes.
    pub fn get_clipping_planes(&self) -> &[Vec4f] {
        &self.clipping_planes
    }

    /// Get version for change tracking.
    pub fn get_version(&self) -> u64 {
        self.version
    }
}

impl Default for HdxFreeCameraPrimDataSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Data source value types returned by the free camera data source.
#[derive(Debug, Clone)]
pub enum DataSourceValue {
    /// Transform matrix (view matrix).
    Matrix(Matrix4d),
    /// Projection matrix.
    Projection(Matrix4d),
    /// Clipping planes.
    ClipPlanes(Vec<Vec4f>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_camera_prim_data_source_default() {
        let ds = HdxFreeCameraPrimDataSource::new();
        assert_eq!(*ds.get_view_matrix(), Matrix4d::identity());
        assert_eq!(*ds.get_projection_matrix(), Matrix4d::identity());
        assert_eq!(ds.get_window_policy(), CameraUtilConformWindowPolicy::Fit);
        assert!(ds.get_clipping_planes().is_empty());
    }

    #[test]
    fn test_free_camera_prim_data_source_from_matrices() {
        let view = Matrix4d::identity();
        let proj = Matrix4d::identity();
        let ds = HdxFreeCameraPrimDataSource::from_matrices(
            view,
            proj,
            CameraUtilConformWindowPolicy::MatchVertically,
        );
        assert_eq!(
            ds.get_window_policy(),
            CameraUtilConformWindowPolicy::MatchVertically
        );
    }

    #[test]
    fn test_free_camera_prim_data_source_set_matrices() {
        let mut ds = HdxFreeCameraPrimDataSource::new();
        let dirty = ds.set_matrices(Matrix4d::identity(), Matrix4d::identity());
        assert_eq!(dirty.len(), 2);
        assert_eq!(ds.get_version(), 1);
    }

    #[test]
    fn test_free_camera_prim_data_source_get_names() {
        let ds = HdxFreeCameraPrimDataSource::new();
        let names = ds.get_names();
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_free_camera_prim_data_source_get() {
        let ds = HdxFreeCameraPrimDataSource::new();
        assert!(ds.get(&Token::new("xform")).is_some());
        assert!(ds.get(&Token::new("camera")).is_some());
        assert!(ds.get(&Token::new("clippingPlanes")).is_some());
        assert!(ds.get(&Token::new("nonexistent")).is_none());
    }

    #[test]
    fn test_free_camera_prim_data_source_clipping_planes() {
        let mut ds = HdxFreeCameraPrimDataSource::new();
        let planes = vec![
            Vec4f::new(0.0, 1.0, 0.0, 0.0),
            Vec4f::new(0.0, -1.0, 0.0, 10.0),
        ];
        let dirty = ds.set_clipping_planes(planes.clone());
        assert_eq!(dirty.len(), 1);
        assert_eq!(ds.get_clipping_planes().len(), 2);
    }
}
