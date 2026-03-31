//! CameraAdapter - Adapter for UsdGeomCamera.
//!
//! Port of pxr/usdImaging/usdImaging/cameraAdapter.h/cpp
//!
//! Provides imaging support for UsdGeomCamera prims, including:
//! - Projection type (perspective/orthographic)
//! - Focal length, aperture, clipping range
//! - Transform from camera to world space

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_gf::Matrix4d;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedSampledDataSource,
};
use usd_sdf::{Path, TimeCode as SdfTimeCode};
use usd_tf::Token;
use usd_vt;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static CAMERA: LazyLock<Token> = LazyLock::new(|| Token::new("camera"));
    pub static PROJECTION: LazyLock<Token> = LazyLock::new(|| Token::new("projection"));
    pub static HORIZONTAL_APERTURE: LazyLock<Token> =
        LazyLock::new(|| Token::new("horizontalAperture"));
    pub static VERTICAL_APERTURE: LazyLock<Token> =
        LazyLock::new(|| Token::new("verticalAperture"));
    pub static HORIZONTAL_APERTURE_OFFSET: LazyLock<Token> =
        LazyLock::new(|| Token::new("horizontalApertureOffset"));
    pub static VERTICAL_APERTURE_OFFSET: LazyLock<Token> =
        LazyLock::new(|| Token::new("verticalApertureOffset"));
    pub static FOCAL_LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("focalLength"));
    pub static CLIPPING_RANGE: LazyLock<Token> = LazyLock::new(|| Token::new("clippingRange"));
    pub static CLIPPING_PLANES: LazyLock<Token> = LazyLock::new(|| Token::new("clippingPlanes"));
    pub static F_STOP: LazyLock<Token> = LazyLock::new(|| Token::new("fStop"));
    pub static FOCUS_DISTANCE: LazyLock<Token> = LazyLock::new(|| Token::new("focusDistance"));
    pub static SHUTTER_OPEN: LazyLock<Token> = LazyLock::new(|| Token::new("shutterOpen"));
    pub static SHUTTER_CLOSE: LazyLock<Token> = LazyLock::new(|| Token::new("shutterClose"));
    pub static EXPOSURE: LazyLock<Token> = LazyLock::new(|| Token::new("exposure"));
}

// ============================================================================
// DataSourceCamera
// ============================================================================

/// Data source for camera parameters.
#[derive(Clone)]
pub struct DataSourceCamera {
    prim: Prim,
    #[allow(dead_code)] // For future time-sampled camera parameter reading
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceCamera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCamera")
            .field("prim", &self.prim)
            .finish()
    }
}

impl DataSourceCamera {
    /// Create new camera data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceCamera {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceCamera {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::PROJECTION.clone(),
            tokens::HORIZONTAL_APERTURE.clone(),
            tokens::VERTICAL_APERTURE.clone(),
            tokens::HORIZONTAL_APERTURE_OFFSET.clone(),
            tokens::VERTICAL_APERTURE_OFFSET.clone(),
            tokens::FOCAL_LENGTH.clone(),
            tokens::CLIPPING_RANGE.clone(),
            tokens::CLIPPING_PLANES.clone(),
            tokens::F_STOP.clone(),
            tokens::FOCUS_DISTANCE.clone(),
            tokens::SHUTTER_OPEN.clone(),
            tokens::SHUTTER_CLOSE.clone(),
            tokens::EXPOSURE.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Read camera attribute from USD prim and return as a typed sampled data source.
        // C++ reference: UsdImagingDataSourceCamera::Get() in dataSourceCamera.cpp.
        let sdf_time = SdfTimeCode::default(); // default time (0.0)
        let n = name.as_str();

        // Map Hydra token names to USD attribute names.
        let usd_attr_name = match n {
            "projection" => "projection",
            "horizontalAperture" => "horizontalAperture",
            "verticalAperture" => "verticalAperture",
            "horizontalApertureOffset" => "horizontalApertureOffset",
            "verticalApertureOffset" => "verticalApertureOffset",
            "focalLength" => "focalLength",
            "clippingRange" => "clippingRange",
            "clippingPlanes" => "clippingPlanes",
            "fStop" => "fStop",
            "focusDistance" => "focusDistance",
            "shutterOpen" => "shutter:open",
            "shutterClose" => "shutter:close",
            "exposure" => "exposure",
            _ => return None,
        };

        let attr = self.prim.get_attribute(usd_attr_name)?;
        let val = attr.get(sdf_time)?;

        // Wrap value in a retained sampled data source.
        Some(HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle)
    }
}

/// Compute world transform matrix for a prim using XformCache.
fn compute_prim_world_matrix(prim: &Prim) -> Matrix4d {
    use usd_geom::XformCache;
    let mut cache = XformCache::new(SdfTimeCode::default());
    cache.get_local_to_world_transform(prim)
}

// ============================================================================
// DataSourceXformMatrix
// ============================================================================

/// Minimal xform data source returning a matrix4d under "matrix" key.
#[derive(Clone, Debug)]
struct DataSourceXform {
    matrix: Matrix4d,
}

impl DataSourceXform {
    fn new(matrix: Matrix4d) -> Arc<Self> {
        Arc::new(Self { matrix })
    }
}

impl HdDataSourceBase for DataSourceXform {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceXform {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("matrix")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "matrix" {
            let val = usd_vt::Value::from(self.matrix);
            return Some(HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle);
        }
        None
    }
}

// ============================================================================
// DataSourceCameraPrim
// ============================================================================

/// Prim data source for UsdGeomCamera.
#[derive(Clone)]
pub struct DataSourceCameraPrim {
    #[allow(dead_code)] // For future path-based operations
    scene_index_path: Path,
    #[allow(dead_code)] // For future camera attribute reading
    prim: Prim,
    camera_ds: Arc<DataSourceCamera>,
}

impl std::fmt::Debug for DataSourceCameraPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCameraPrim")
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl DataSourceCameraPrim {
    /// Create new camera prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let camera_ds = DataSourceCamera::new(prim.clone(), stage_globals);
        Arc::new(Self {
            scene_index_path,
            prim,
            camera_ds,
        })
    }

    /// Compute invalidation for property changes.
    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();

        for prop in properties {
            let prop_str = prop.as_str();
            match prop_str {
                "projection"
                | "horizontalAperture"
                | "verticalAperture"
                | "horizontalApertureOffset"
                | "verticalApertureOffset"
                | "focalLength"
                | "clippingRange"
                | "clippingPlanes"
                | "fStop"
                | "focusDistance"
                | "shutterOpen"
                | "shutterClose"
                | "exposure" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        Token::new("camera"),
                        prop.clone(),
                    ));
                }
                "xformOpOrder" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        Token::new("xform"),
                        Token::new("matrix"),
                    ));
                }
                name if name.starts_with("xformOp:") => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        Token::new("xform"),
                        Token::new("matrix"),
                    ));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceCameraPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceCameraPrim {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::CAMERA.clone(), Token::new("xform")]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::CAMERA {
            return Some(Arc::clone(&self.camera_ds) as HdDataSourceBaseHandle);
        }
        if name == "xform" {
            // Build xform data source from camera prim's world transform.
            // C++ UsdImagingDataSourceCameraPrim::Get() returns HdXformSchema.
            let matrix = compute_prim_world_matrix(&self.prim);
            return Some(DataSourceXform::new(matrix) as HdContainerDataSourceHandle);
        }
        None
    }
}

// ============================================================================
// CameraAdapter
// ============================================================================

/// Adapter for UsdGeomCamera prims.
///
/// Converts UsdGeomCamera to Hydra camera primitives with:
/// - Projection parameters (perspective/orthographic)
/// - Aperture and focal length
/// - Depth of field settings
/// - Clipping planes
#[derive(Debug, Clone, Default)]
pub struct CameraAdapter;

impl CameraAdapter {
    /// Create a new camera adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for CameraAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::CAMERA.clone()
        } else {
            Token::new("")
        }
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(DataSourceCameraPrim::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
            ))
        } else {
            None
        }
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceCameraPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Arc-wrapped CameraAdapter for sharing
pub type CameraAdapterHandle = Arc<CameraAdapter>;

/// Factory function for creating camera adapters.
pub fn create_camera_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(CameraAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_camera_adapter_creation() {
        let adapter = CameraAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "camera");
    }

    #[test]
    fn test_camera_adapter_subprims() {
        let adapter = CameraAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_camera_adapter_data_source() {
        let adapter = CameraAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_camera_data_source_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceCamera::new(prim, globals);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "projection"));
        assert!(names.iter().any(|n| n == "focalLength"));
        assert!(names.iter().any(|n| n == "clippingRange"));
    }

    #[test]
    fn test_camera_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("focalLength"), Token::new("projection")];

        let locators = DataSourceCameraPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }
}
