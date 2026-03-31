//! DataSourceCamera - Camera data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceCamera.h

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_prim::DataSourcePrim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdSampledDataSource, HdSampledDataSourceTime,
};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

/// GfCamera aperture unit: mm to cm conversion factor
const APERTURE_UNIT: f32 = 0.1;
/// GfCamera focal length unit: mm to cm conversion factor
const FOCAL_LENGTH_UNIT: f32 = 0.1;

// ============================================================================
// ScalingDataSource - multiplies attribute value by a constant scale factor
// ============================================================================

/// Wraps a sampled data source and scales its float value by a constant.
///
/// Used to convert camera lens attributes from USD units (mm) to Hydra (cm).
#[derive(Clone)]
struct ScalingDataSource {
    inner: Arc<DataSourceAttribute<Value>>,
    scale: f32,
}

impl ScalingDataSource {
    fn new(inner: Arc<DataSourceAttribute<Value>>, scale: f32) -> Self {
        Self { inner, scale }
    }
}

impl std::fmt::Debug for ScalingDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScalingDataSource")
            .field("scale", &self.scale)
            .finish()
    }
}

impl HdDataSourceBase for ScalingDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for ScalingDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let val = self.inner.get_value(shutter_offset);
        if let Some(v) = val.get::<f32>() {
            Value::from(*v * self.scale)
        } else {
            val
        }
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.inner
            .get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

// ============================================================================
// Vec4fToVec4dDataSource - converts Vec4f arrays to Vec4d
// ============================================================================

/// Wraps a sampled data source and converts Vec4f array values to Vec4d.
///
/// C++ parity: _Vec4fArrayToVec4dArrayDataSource.
/// Used for clippingPlanes which are stored as Vec4f in USD but Hydra expects Vec4d.
#[derive(Clone)]
struct Vec4fToVec4dDataSource {
    inner: Arc<DataSourceAttribute<Value>>,
}

impl Vec4fToVec4dDataSource {
    fn new(inner: Arc<DataSourceAttribute<Value>>) -> Self {
        Self { inner }
    }
}

impl std::fmt::Debug for Vec4fToVec4dDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vec4fToVec4dDataSource").finish()
    }
}

impl HdDataSourceBase for Vec4fToVec4dDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for Vec4fToVec4dDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        use usd_gf::{Vec4d, Vec4f};
        let val = self.inner.get_value(shutter_offset);
        // Convert Vec<Vec4f> to Vec<Vec4d>
        if let Some(arr) = val.get::<Vec<Vec4f>>() {
            let converted: Vec<Vec4d> = arr
                .iter()
                .map(|v| Vec4d::new(v.x as f64, v.y as f64, v.z as f64, v.w as f64))
                .collect();
            return Value::from(converted);
        }
        // Fallback: return as-is (may already be f64 or empty)
        val
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.inner
            .get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

// ============================================================================
// LinearExposureScaleDataSource - computes linearExposureScale from exposure
// ============================================================================

/// Computes linear exposure scale = 2^exposure.
///
/// C++ parity: _CameraLinearExposureScaleDataSource.
/// Simplified: only uses the "exposure" attribute (not the full EV formula
/// with exposureTime/ISO/fStop/responsivity which require UsdGeomCamera API).
#[derive(Clone)]
struct LinearExposureScaleDataSource {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl LinearExposureScaleDataSource {
    fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl std::fmt::Debug for LinearExposureScaleDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearExposureScaleDataSource").finish()
    }
}

impl HdDataSourceBase for LinearExposureScaleDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for LinearExposureScaleDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let base_time = self.stage_globals.get_time();
        let t = TimeCode::new(base_time.value() + shutter_offset as f64);
        // Read "exposure" attr and compute 2^exposure
        let exposure = self
            .prim
            .get_attribute("exposure")
            .and_then(|a| a.get(t))
            .and_then(|v| v.get::<f32>().copied())
            .unwrap_or(0.0f32);
        Value::from(2.0f32.powf(exposure))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        // Delegate to exposure attr sample times
        if let Some(attr) = self.prim.get_attribute("exposure") {
            let ds =
                DataSourceAttribute::<Value>::new(attr, self.stage_globals.clone(), Path::empty());
            return ds.get_contributing_sample_times(start_time, end_time, out);
        }
        false
    }
}

#[allow(dead_code)]
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
    // Hydra schema names - differ from USD attr names (shutter:open / shutter:close)
    pub static SHUTTER_OPEN: LazyLock<Token> = LazyLock::new(|| Token::new("shutterOpen"));
    pub static SHUTTER_CLOSE: LazyLock<Token> = LazyLock::new(|| Token::new("shutterClose"));
    // USD attribute names for shutter (colon-namespaced)
    pub static USD_SHUTTER_OPEN: LazyLock<Token> = LazyLock::new(|| Token::new("shutter:open"));
    pub static USD_SHUTTER_CLOSE: LazyLock<Token> = LazyLock::new(|| Token::new("shutter:close"));
    pub static EXPOSURE: LazyLock<Token> = LazyLock::new(|| Token::new("exposure"));
    // Exposure sub-attributes (Hydra schema)
    pub static EXPOSURE_TIME: LazyLock<Token> = LazyLock::new(|| Token::new("exposureTime"));
    pub static EXPOSURE_ISO: LazyLock<Token> = LazyLock::new(|| Token::new("exposureIso"));
    pub static EXPOSURE_F_STOP: LazyLock<Token> = LazyLock::new(|| Token::new("exposureFStop"));
    pub static EXPOSURE_RESPONSIVITY: LazyLock<Token> =
        LazyLock::new(|| Token::new("exposureResponsivity"));
    // Computed value: 2^(exposure + exposureTime*...) - separate from raw "exposure"
    pub static LINEAR_EXPOSURE_SCALE: LazyLock<Token> =
        LazyLock::new(|| Token::new("linearExposureScale"));
    // stereoRole
    pub static STEREO_ROLE: LazyLock<Token> = LazyLock::new(|| Token::new("stereoRole"));
}

/// Container data source representing camera info.
#[derive(Clone)]
pub struct DataSourceCamera {
    #[allow(dead_code)]
    scene_index_path: Path,
    #[allow(dead_code)]
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceCamera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCamera").finish()
    }
}

impl DataSourceCamera {
    /// Creates a new camera data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
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
        // C++ parity: UsdGeomCamera::GetSchemaAttributeNames(includeInherited=false)
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
            tokens::EXPOSURE_TIME.clone(),
            tokens::EXPOSURE_ISO.clone(),
            tokens::EXPOSURE_F_STOP.clone(),
            tokens::EXPOSURE_RESPONSIVITY.clone(),
            tokens::LINEAR_EXPOSURE_SCALE.clone(),
            tokens::STEREO_ROLE.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // C++ parity: computed linearExposureScale (not a raw attr)
        if name == &*tokens::LINEAR_EXPOSURE_SCALE {
            return Some(LinearExposureScaleDataSource::new(
                self.prim.clone(),
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }

        // C++ parity: Hydra "shutterOpen" -> USD "shutter:open", "shutterClose" -> USD "shutter:close"
        let usd_name = if name == &*tokens::SHUTTER_OPEN {
            &*tokens::USD_SHUTTER_OPEN
        } else if name == &*tokens::SHUTTER_CLOSE {
            &*tokens::USD_SHUTTER_CLOSE
        } else {
            name
        };

        // Read the attribute from the prim using (possibly remapped) USD name
        let attr = self.prim.get_attribute(usd_name.as_str())?;
        let base_ds = DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            self.scene_index_path.clone(),
        );

        // Apply unit scaling for aperture/focal length attributes
        // GfCamera uses mm internally, Hydra expects cm
        // APERTURE_UNIT = 0.1 (mm -> cm), FOCAL_LENGTH_UNIT = 0.1
        if name == &*tokens::HORIZONTAL_APERTURE
            || name == &*tokens::VERTICAL_APERTURE
            || name == &*tokens::HORIZONTAL_APERTURE_OFFSET
            || name == &*tokens::VERTICAL_APERTURE_OFFSET
        {
            return Some(Arc::new(ScalingDataSource::new(base_ds, APERTURE_UNIT)));
        }

        if name == &*tokens::FOCAL_LENGTH {
            return Some(Arc::new(ScalingDataSource::new(base_ds, FOCAL_LENGTH_UNIT)));
        }

        // clippingPlanes: convert Vec4f array to Vec4d array (C++ _Vec4fArrayToVec4dArrayDataSource)
        if name == &*tokens::CLIPPING_PLANES {
            return Some(Arc::new(Vec4fToVec4dDataSource::new(base_ds)));
        }

        // For all other camera attributes, return the attribute data source directly
        Some(base_ds as HdDataSourceBaseHandle)
    }
}

/// Handle type for DataSourceCamera.
pub type DataSourceCameraHandle = Arc<DataSourceCamera>;

/// Prim data source representing UsdGeomCamera.
#[derive(Clone)]
pub struct DataSourceCameraPrim {
    base: DataSourcePrim,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceCameraPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCameraPrim").finish()
    }
}

impl DataSourceCameraPrim {
    /// Creates a new camera prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourcePrim::new(prim.clone(), scene_index_path, stage_globals.clone()),
            prim,
            stage_globals,
        }
    }

    /// Returns the list of data source names.
    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(tokens::CAMERA.clone());
        names
    }

    /// Gets a data source by name.
    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::CAMERA {
            let camera_ds = DataSourceCamera::new(
                self.base.hydra_path().clone(),
                self.prim.clone(),
                self.stage_globals.clone(),
            );
            return Some(Arc::new(camera_ds) as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }

    /// Computes invalidation locators for property changes.
    /// C++ parity: UsdImagingDataSourceCameraPrim::Invalidate - iterates
    /// UsdGeomCamera::GetSchemaAttributeNames() and handles exposure->linearExposureScale
    /// cross-dirtying.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);

        // All camera USD schema attribute names that map to the camera locator
        let camera_usd_attrs = [
            "projection",
            "horizontalAperture",
            "verticalAperture",
            "horizontalApertureOffset",
            "verticalApertureOffset",
            "focalLength",
            "clippingRange",
            "clippingPlanes",
            "fStop",
            "focusDistance",
            "shutter:open",
            "shutter:close", // USD names
            "exposure",
            "exposureTime",
            "exposureIso",
            "exposureFStop",
            "exposureResponsivity",
            "stereoRole",
        ];

        // Exposure attrs that also dirty linearExposureScale (C++ parity)
        let exposure_attrs = [
            "exposure",
            "exposureTime",
            "exposureIso",
            "exposureFStop",
            "exposureResponsivity",
        ];

        for prop in properties {
            let prop_str = prop.as_str();
            if camera_usd_attrs.contains(&prop_str) {
                // shutterOpen/shutterClose: use specific locators (C++ parity)
                if prop_str == "shutter:open" {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::CAMERA.clone(),
                        tokens::SHUTTER_OPEN.clone(),
                    ));
                } else if prop_str == "shutter:close" {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::CAMERA.clone(),
                        tokens::SHUTTER_CLOSE.clone(),
                    ));
                } else {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::CAMERA.clone(),
                        Token::new(prop_str),
                    ));
                }

                // Exposure attrs also dirty the computed linearExposureScale
                if exposure_attrs.contains(&prop_str) {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::CAMERA.clone(),
                        tokens::LINEAR_EXPOSURE_SCALE.clone(),
                    ));
                }
            }
        }
        locators
    }
}

/// Handle type for DataSourceCameraPrim.
pub type DataSourceCameraPrimHandle = Arc<DataSourceCameraPrim>;

/// Creates a new camera prim data source.
pub fn create_data_source_camera_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceCameraPrimHandle {
    Arc::new(DataSourceCameraPrim::new(
        scene_index_path,
        prim,
        stage_globals,
    ))
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
    fn test_camera_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceCamera::new(Path::absolute_root(), prim, globals);
        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "projection"));
    }
}
