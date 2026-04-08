//! pxr.UsdLux — Python bindings for the UsdLux lighting schema library.
//!
//! Mirrors the C++ API: UsdLuxLightAPI, UsdLuxShapingAPI, UsdLuxShadowAPI,
//! all concrete light types, LightListAPI, MeshLightAPI, VolumeLightAPI,
//! and the BlackbodyTemperatureAsRgb free function.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

use usd_core::{Prim, Stage};
#[allow(deprecated)]
use usd_lux::geometry_light::GeometryLight;
use usd_lux::{
    blackbody::blackbody_temperature_as_rgb,
    boundable_light_base::BoundableLightBase,
    cylinder_light::CylinderLight,
    disk_light::DiskLight,
    distant_light::DistantLight,
    dome_light::DomeLight,
    dome_light_1::DomeLight1,
    light_api::LightAPI,
    light_filter::LightFilter,
    light_list_api::{ComputeMode, LightListAPI},
    mesh_light_api::MeshLightAPI,
    nonboundable_light_base::NonboundableLightBase,
    plugin_light::PluginLight,
    plugin_light_filter::PluginLightFilter,
    portal_light::PortalLight,
    rect_light::RectLight,
    shadow_api::ShadowAPI,
    shaping_api::ShapingAPI,
    sphere_light::SphereLight,
    volume_light_api::VolumeLightAPI,
};
use usd_sdf::Path;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn path_from_str(s: &str) -> PyResult<Path> {
    Path::from_string(s).ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: {s}")))
}

fn get_prim(stage: &Arc<Stage>, path: &str) -> PyResult<Prim> {
    let p = path_from_str(path)?;
    stage
        .get_prim_at_path(&p)
        .ok_or_else(|| PyValueError::new_err(format!("No prim at path: {path}")))
}

// ---------------------------------------------------------------------------
// Shared stage wrapper
// ---------------------------------------------------------------------------

#[pyclass(name = "Stage", module = "pxr_rs.UsdLux")]
struct PyStage {
    inner: Arc<Stage>,
}

// ---------------------------------------------------------------------------
// PyLightAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "LightAPI", module = "pxr_rs.UsdLux")]
struct PyLightAPI {
    inner: LightAPI,
}

#[pymethods]
impl PyLightAPI {
    /// LightAPI.Apply(stage, path) -> LightAPI or None
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let prim = get_prim(&stage.inner, path)?;
        Ok(LightAPI::apply(&prim).map(|api| Self { inner: api }))
    }

    /// LightAPI.Get(stage, path) -> LightAPI
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: LightAPI::get(&*stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_intensity_attr(&self) -> Option<String> {
        self.inner
            .get_intensity_attr()
            .map(|a| a.path().to_string())
    }

    fn get_exposure_attr(&self) -> Option<String> {
        self.inner.get_exposure_attr().map(|a| a.path().to_string())
    }

    fn get_color_attr(&self) -> Option<String> {
        self.inner.get_color_attr().map(|a| a.path().to_string())
    }

    fn get_normalize_attr(&self) -> Option<String> {
        self.inner
            .get_normalize_attr()
            .map(|a| a.path().to_string())
    }

    fn get_enable_color_temperature_attr(&self) -> Option<String> {
        self.inner
            .get_enable_color_temperature_attr()
            .map(|a| a.path().to_string())
    }

    fn get_color_temperature_attr(&self) -> Option<String> {
        self.inner
            .get_color_temperature_attr()
            .map(|a| a.path().to_string())
    }

    fn get_diffuse_attr(&self) -> Option<String> {
        self.inner.get_diffuse_attr().map(|a| a.path().to_string())
    }

    fn get_specular_attr(&self) -> Option<String> {
        self.inner.get_specular_attr().map(|a| a.path().to_string())
    }

    /// CreateInput(name, type_name) -> full input name string or None
    fn create_input(&self, name: &str, type_name: &str) -> Option<String> {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        self.inner
            .create_input(&Token::new(name), &tn)
            .map(|inp| inp.get_full_name().as_str().to_string())
    }

    fn __repr__(&self) -> String {
        "UsdLux.LightAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyShapingAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "ShapingAPI", module = "pxr_rs.UsdLux")]
struct PyShapingAPI {
    inner: ShapingAPI,
}

#[pymethods]
impl PyShapingAPI {
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let prim = get_prim(&stage.inner, path)?;
        Ok(ShapingAPI::apply(&prim).map(|api| Self { inner: api }))
    }

    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: ShapingAPI::get(&*stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_shaping_focus_attr(&self) -> Option<String> {
        self.inner
            .get_shaping_focus_attr()
            .map(|a| a.path().to_string())
    }

    fn get_shaping_cone_angle_attr(&self) -> Option<String> {
        self.inner
            .get_shaping_cone_angle_attr()
            .map(|a| a.path().to_string())
    }

    fn get_shaping_cone_softness_attr(&self) -> Option<String> {
        self.inner
            .get_shaping_cone_softness_attr()
            .map(|a| a.path().to_string())
    }

    fn get_shaping_ies_file_attr(&self) -> Option<String> {
        self.inner
            .get_shaping_ies_file_attr()
            .map(|a| a.path().to_string())
    }

    fn __repr__(&self) -> String {
        "UsdLux.ShapingAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyShadowAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "ShadowAPI", module = "pxr_rs.UsdLux")]
struct PyShadowAPI {
    inner: ShadowAPI,
}

#[pymethods]
impl PyShadowAPI {
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let prim = get_prim(&stage.inner, path)?;
        Ok(ShadowAPI::apply(&prim).map(|api| Self { inner: api }))
    }

    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: ShadowAPI::get(&*stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_shadow_enable_attr(&self) -> Option<String> {
        self.inner
            .get_shadow_enable_attr()
            .map(|a| a.path().to_string())
    }

    fn get_shadow_color_attr(&self) -> Option<String> {
        self.inner
            .get_shadow_color_attr()
            .map(|a| a.path().to_string())
    }

    fn get_shadow_distance_attr(&self) -> Option<String> {
        self.inner
            .get_shadow_distance_attr()
            .map(|a| a.path().to_string())
    }

    fn get_shadow_falloff_attr(&self) -> Option<String> {
        self.inner
            .get_shadow_falloff_attr()
            .map(|a| a.path().to_string())
    }

    fn __repr__(&self) -> String {
        "UsdLux.ShadowAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyBoundableLightBase
// ---------------------------------------------------------------------------

#[pyclass(name = "BoundableLightBase", module = "pxr_rs.UsdLux")]
struct PyBoundableLightBase {
    inner: BoundableLightBase,
}

#[pymethods]
impl PyBoundableLightBase {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: BoundableLightBase::get(&*stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }

    fn __repr__(&self) -> String {
        "UsdLux.BoundableLightBase".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyNonboundableLightBase
// ---------------------------------------------------------------------------

#[pyclass(name = "NonboundableLightBase", module = "pxr_rs.UsdLux")]
struct PyNonboundableLightBase {
    inner: NonboundableLightBase,
}

#[pymethods]
impl PyNonboundableLightBase {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: NonboundableLightBase::get(&*stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }

    fn __repr__(&self) -> String {
        "UsdLux.NonboundableLightBase".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// Concrete light types — each written out explicitly since pyo3 forbids
// macros inside #[pymethods] blocks.
// ---------------------------------------------------------------------------

// --- DiskLight ---

#[pyclass(name = "DiskLight", module = "pxr_rs.UsdLux")]
struct PyDiskLight {
    inner: DiskLight,
}

#[pymethods]
impl PyDiskLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: DiskLight::get(&*stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(DiskLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }
    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.DiskLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }
}

// --- RectLight ---

#[pyclass(name = "RectLight", module = "pxr_rs.UsdLux")]
struct PyRectLight {
    inner: RectLight,
}

#[pymethods]
impl PyRectLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: RectLight::get(&*stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(RectLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }
    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.RectLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }
}

// --- SphereLight ---

#[pyclass(name = "SphereLight", module = "pxr_rs.UsdLux")]
struct PySphereLight {
    inner: SphereLight,
}

#[pymethods]
impl PySphereLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: SphereLight::get(&*stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(SphereLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }
    // SphereLight exposes both light_api() and get_light_api() names
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.get_light_api(),
        }
    }
    fn get_light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.get_light_api(),
        }
    }
    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.SphereLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }
}

// --- CylinderLight ---

#[pyclass(name = "CylinderLight", module = "pxr_rs.UsdLux")]
struct PyCylinderLight {
    inner: CylinderLight,
}

#[pymethods]
impl PyCylinderLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: CylinderLight::get(&*stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(CylinderLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }
    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.CylinderLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }
}

// --- DistantLight ---

#[pyclass(name = "DistantLight", module = "pxr_rs.UsdLux")]
struct PyDistantLight {
    inner: DistantLight,
}

#[pymethods]
impl PyDistantLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: DistantLight::get(&*stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(DistantLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }
    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.DistantLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }
}

// --- DomeLight ---

#[pyclass(name = "DomeLight", module = "pxr_rs.UsdLux")]
struct PyDomeLight {
    inner: DomeLight,
}

#[pymethods]
impl PyDomeLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: DomeLight::get(&*stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(DomeLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: self.inner.light_api(),
        }
    }
    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.DomeLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }
}

// --- DomeLight_1 (uses Arc<Stage> -> Option) ---

#[pyclass(name = "DomeLight_1", module = "pxr_rs.UsdLux")]
struct PyDomeLight1 {
    inner: DomeLight1,
}

#[pymethods]
impl PyDomeLight1 {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(DomeLight1::get(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(DomeLight1::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }

    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: LightAPI::new(self.inner.get_prim().clone()),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.DomeLight_1('{}')",
            self.inner.get_prim().path().to_string()
        )
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// --- GeometryLight (Arc<Stage>->Option) ---
// Deprecated upstream in favour of MeshLightAPI, kept for API parity.

#[allow(deprecated)]
#[pyclass(name = "GeometryLight", module = "pxr_rs.UsdLux")]
struct PyGeometryLight {
    inner: GeometryLight,
}

#[allow(deprecated)]
#[pymethods]
impl PyGeometryLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(GeometryLight::get(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(GeometryLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.prim().path().to_string()
    }

    // GeometryLight doesn't have light_api() — use LightAPI::new separately if needed
    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: LightAPI::new(self.inner.prim().clone()),
        }
    }

    fn __repr__(&self) -> String {
        format!("UsdLux.GeometryLight('{}')", self.inner.prim().path())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// --- PortalLight (Arc<Stage>->Option) ---

#[pyclass(name = "PortalLight", module = "pxr_rs.UsdLux")]
struct PyPortalLight {
    inner: PortalLight,
}

#[pymethods]
impl PyPortalLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(PortalLight::get(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(PortalLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.prim().path().to_string()
    }

    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: LightAPI::new(self.inner.prim().clone()),
        }
    }

    fn __repr__(&self) -> String {
        format!("UsdLux.PortalLight('{}')", self.inner.prim().path())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// --- PluginLight (Arc<Stage>->Option) ---

#[pyclass(name = "PluginLight", module = "pxr_rs.UsdLux")]
struct PyPluginLight {
    inner: PluginLight,
}

#[pymethods]
impl PyPluginLight {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(PluginLight::get(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(PluginLight::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.get_prim().path().to_string()
    }

    fn light_api(&self) -> PyLightAPI {
        PyLightAPI {
            inner: LightAPI::new(self.inner.get_prim().clone()),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.PluginLight('{}')",
            self.inner.get_prim().path().to_string()
        )
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyLightFilter
// ---------------------------------------------------------------------------

#[pyclass(name = "LightFilter", module = "pxr_rs.UsdLux")]
struct PyLightFilter {
    inner: LightFilter,
}

#[pymethods]
impl PyLightFilter {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(LightFilter::get(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(LightFilter::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        // LightFilter uses prim() not get_prim()
        self.inner.prim().path().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "UsdLux.LightFilter('{}')",
            self.inner.prim().path().to_string()
        )
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyPluginLightFilter
// ---------------------------------------------------------------------------

#[pyclass(name = "PluginLightFilter", module = "pxr_rs.UsdLux")]
struct PyPluginLightFilter {
    inner: PluginLightFilter,
}

#[pymethods]
impl PyPluginLightFilter {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(PluginLightFilter::get(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(PluginLightFilter::define(&stage.inner, &p).map(|l| Self { inner: l }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        "UsdLux.PluginLightFilter".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyMeshLightAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "MeshLightAPI", module = "pxr_rs.UsdLux")]
struct PyMeshLightAPI {
    inner: MeshLightAPI,
}

#[pymethods]
impl PyMeshLightAPI {
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let prim = get_prim(&stage.inner, path)?;
        Ok(MeshLightAPI::apply(&prim).map(|api| Self { inner: api }))
    }

    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(MeshLightAPI::get(&stage.inner, &p).map(|api| Self { inner: api }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        "UsdLux.MeshLightAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyVolumeLightAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "VolumeLightAPI", module = "pxr_rs.UsdLux")]
struct PyVolumeLightAPI {
    inner: VolumeLightAPI,
}

#[pymethods]
impl PyVolumeLightAPI {
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let prim = get_prim(&stage.inner, path)?;
        Ok(VolumeLightAPI::apply(&prim).map(|api| Self { inner: api }))
    }

    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(VolumeLightAPI::get(&stage.inner, &p).map(|api| Self { inner: api }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        "UsdLux.VolumeLightAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyLightListAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "LightListAPI", module = "pxr_rs.UsdLux")]
struct PyLightListAPI {
    inner: LightListAPI,
}

#[pymethods]
impl PyLightListAPI {
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let prim = get_prim(&stage.inner, path)?;
        Ok(LightListAPI::apply(&prim).map(|api| Self { inner: api }))
    }

    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Option<Self>> {
        let p = path_from_str(path)?;
        Ok(LightListAPI::get(&stage.inner, &p).map(|api| Self { inner: api }))
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// ComputeLightList(mode="ignoreCache") -> list of light prim path strings
    #[pyo3(signature = (mode = "ignoreCache"))]
    fn compute_light_list(&self, mode: &str) -> Vec<String> {
        let compute_mode = if mode.contains("onsult") {
            ComputeMode::ConsultModelHierarchyCache
        } else {
            ComputeMode::IgnoreCache
        };
        self.inner
            .compute_light_list(compute_mode)
            .into_iter()
            .map(|p| p.to_string())
            .collect()
    }

    fn __repr__(&self) -> String {
        "UsdLux.LightListAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// Tokens — mirrors UsdLuxTokens
// ---------------------------------------------------------------------------

#[pyclass(name = "Tokens", module = "pxr_rs.UsdLux")]
struct PyTokens;

#[pymethods]
impl PyTokens {
    #[getter]
    fn angular(&self) -> &str {
        "angular"
    }
    #[getter]
    fn automatic(&self) -> &str {
        "automatic"
    }
    #[getter]
    fn consumeAndContinue(&self) -> &str {
        "consumeAndContinue"
    }
    #[getter]
    fn consumeAndHalt(&self) -> &str {
        "consumeAndHalt"
    }
    #[getter]
    fn cubeMapVerticalCross(&self) -> &str {
        "cubeMapVerticalCross"
    }
    #[getter]
    fn filterLink(&self) -> &str {
        "filterLink"
    }
    #[getter]
    fn geometry(&self) -> &str {
        "geometry"
    }
    #[getter]
    fn guideRadius(&self) -> &str {
        "guideRadius"
    }
    #[getter]
    fn ignore(&self) -> &str {
        "ignore"
    }
    #[getter]
    fn independent(&self) -> &str {
        "independent"
    }
    #[getter]
    fn inputsColor(&self) -> &str {
        "inputs:color"
    }
    #[getter]
    fn inputsColorTemperature(&self) -> &str {
        "inputs:colorTemperature"
    }
    #[getter]
    fn inputsDiffuse(&self) -> &str {
        "inputs:diffuse"
    }
    #[getter]
    fn inputsEnableColorTemperature(&self) -> &str {
        "inputs:enableColorTemperature"
    }
    #[getter]
    fn inputsExposure(&self) -> &str {
        "inputs:exposure"
    }
    #[getter]
    fn inputsHeight(&self) -> &str {
        "inputs:height"
    }
    #[getter]
    fn inputsIntensity(&self) -> &str {
        "inputs:intensity"
    }
    #[getter]
    fn inputsNormalize(&self) -> &str {
        "inputs:normalize"
    }
    #[getter]
    fn inputsRadius(&self) -> &str {
        "inputs:radius"
    }
    #[getter]
    fn inputsSpecular(&self) -> &str {
        "inputs:specular"
    }
    #[getter]
    fn inputsWidth(&self) -> &str {
        "inputs:width"
    }
    #[getter]
    fn lightFilters(&self) -> &str {
        "light:filters"
    }
    #[getter]
    fn lightLink(&self) -> &str {
        "lightLink"
    }
    #[getter]
    fn lightList(&self) -> &str {
        "lightList"
    }
    #[getter]
    fn lightListCacheBehavior(&self) -> &str {
        "lightList:cacheBehavior"
    }
    #[getter]
    fn lightShaderId(&self) -> &str {
        "light:shaderId"
    }
    #[getter]
    fn materialGlowTintsLight(&self) -> &str {
        "materialGlowTintsLight"
    }
    #[getter]
    fn noMaterialResponse(&self) -> &str {
        "noMaterialResponse"
    }
    #[getter]
    fn orientToStageUpAxis(&self) -> &str {
        "orientToStageUpAxis"
    }
    #[getter]
    fn portals(&self) -> &str {
        "portals"
    }
    #[getter]
    fn shadowLink(&self) -> &str {
        "shadowLink"
    }
    #[getter]
    fn treatAsLine(&self) -> &str {
        "treatAsLine"
    }
    #[getter]
    fn treatAsPoint(&self) -> &str {
        "treatAsPoint"
    }
    #[getter]
    fn inputsTextureFile(&self) -> &str {
        "inputs:texture:file"
    }
    #[getter]
    fn inputsTextureFormat(&self) -> &str {
        "inputs:texture:format"
    }
}

// ---------------------------------------------------------------------------
// BlackbodyTemperatureAsRgb free function
// ---------------------------------------------------------------------------

/// Compute RGB color for a blackbody radiator at `temperature_kelvin` (1000–10000K).
///
/// Returns `(r, g, b)` tuple.
#[pyfunction]
#[pyo3(name = "BlackbodyTemperatureAsRgb")]
fn py_blackbody_temperature_as_rgb(temperature_kelvin: f32) -> (f32, f32, f32) {
    let rgb = blackbody_temperature_as_rgb(temperature_kelvin);
    (rgb.x, rgb.y, rgb.z)
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStage>()?;
    m.add_class::<PyLightAPI>()?;
    m.add_class::<PyShapingAPI>()?;
    m.add_class::<PyShadowAPI>()?;
    m.add_class::<PyBoundableLightBase>()?;
    m.add_class::<PyNonboundableLightBase>()?;

    // Concrete lights
    m.add_class::<PyDiskLight>()?;
    m.add_class::<PyRectLight>()?;
    m.add_class::<PySphereLight>()?;
    m.add_class::<PyCylinderLight>()?;
    m.add_class::<PyDistantLight>()?;
    m.add_class::<PyDomeLight>()?;
    m.add_class::<PyDomeLight1>()?;
    m.add_class::<PyGeometryLight>()?;
    m.add_class::<PyPortalLight>()?;
    m.add_class::<PyPluginLight>()?;

    // Filters
    m.add_class::<PyLightFilter>()?;
    m.add_class::<PyPluginLightFilter>()?;

    // API schemas
    m.add_class::<PyMeshLightAPI>()?;
    m.add_class::<PyVolumeLightAPI>()?;
    m.add_class::<PyLightListAPI>()?;

    // Tokens singleton
    m.add_class::<PyTokens>()?;
    m.add("Tokens", PyTokens)?;

    // Free function (both snake_case and CamelCase names)
    m.add_function(wrap_pyfunction!(py_blackbody_temperature_as_rgb, m)?)?;

    Ok(())
}
