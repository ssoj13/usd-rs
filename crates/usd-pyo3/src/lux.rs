//! pxr.UsdLux — Python bindings for the UsdLux lighting schema library.
//!
//! Mirrors the C++ API: UsdLuxLightAPI, UsdLuxShapingAPI, UsdLuxShadowAPI,
//! all concrete light types, LightListAPI, MeshLightAPI, VolumeLightAPI,
//! and the BlackbodyTemperatureAsRgb free function.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use std::sync::Arc;

use crate::sdf::value_type_from_py_any;
use crate::shade::{PyConnectableAPI, PyInput, PyOutput};
use crate::usd::{PyAttribute, PyPrim};
use usd_core::{Attribute, Prim, Stage};
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

fn lux_optional_default(
    default_value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<usd_vt::Value>> {
    match default_value {
        None => Ok(None),
        Some(o) => Ok(Some(crate::vt::py_to_value(o)?)),
    }
}

fn lux_attr_or_invalid(attr: Option<Attribute>) -> PyAttribute {
    PyAttribute::from_attr(attr.unwrap_or_else(Attribute::invalid))
}

// ---------------------------------------------------------------------------
// Shared stage wrapper
// ---------------------------------------------------------------------------

#[pyclass(name = "Stage", module = "pxr.UsdLux")]
struct PyStage {
    inner: Arc<Stage>,
}

// ---------------------------------------------------------------------------
// PyLightAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "LightAPI", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetShaderIdAttr")]
    fn get_shader_id_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shader_id_attr())
    }

    #[pyo3(name = "CreateShaderIdAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shader_id_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shader_id_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShaderIdAttrForRenderContext")]
    fn get_shader_id_attr_for_render_context_py(&self, render_context: &str) -> PyAttribute {
        let t = Token::new(render_context);
        lux_attr_or_invalid(self.inner.get_shader_id_attr_for_render_context(&t))
    }

    #[pyo3(
        name = "CreateShaderIdAttrForRenderContext",
        signature = (render_context, default_value=None, write_sparsely=false)
    )]
    fn create_shader_id_attr_for_render_context_py(
        &self,
        render_context: &str,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(self.inner.create_shader_id_attr_for_render_context(
            &Token::new(render_context),
            lux_optional_default(default_value)?,
            write_sparsely,
        )))
    }

    #[pyo3(name = "GetMaterialSyncModeAttr")]
    fn get_material_sync_mode_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_material_sync_mode_attr())
    }

    #[pyo3(name = "CreateMaterialSyncModeAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_material_sync_mode_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_material_sync_mode_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetIntensityAttr")]
    fn get_intensity_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_intensity_attr())
    }

    #[pyo3(name = "CreateIntensityAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_intensity_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_intensity_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetExposureAttr")]
    fn get_exposure_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_exposure_attr())
    }

    #[pyo3(name = "CreateExposureAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_exposure_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_exposure_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetDiffuseAttr")]
    fn get_diffuse_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_diffuse_attr())
    }

    #[pyo3(name = "CreateDiffuseAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_diffuse_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_diffuse_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetSpecularAttr")]
    fn get_specular_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_specular_attr())
    }

    #[pyo3(name = "CreateSpecularAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_specular_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_specular_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetColorAttr")]
    fn get_color_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_color_attr())
    }

    #[pyo3(name = "CreateColorAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_color_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_color_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetEnableColorTemperatureAttr")]
    fn get_enable_color_temperature_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_enable_color_temperature_attr())
    }

    #[pyo3(
        name = "CreateEnableColorTemperatureAttr",
        signature = (default_value=None, write_sparsely=false)
    )]
    fn create_enable_color_temperature_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_enable_color_temperature_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    #[pyo3(name = "GetColorTemperatureAttr")]
    fn get_color_temperature_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_color_temperature_attr())
    }

    #[pyo3(name = "CreateColorTemperatureAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_color_temperature_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_color_temperature_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    #[pyo3(name = "GetNormalizeAttr")]
    fn get_normalize_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_normalize_attr())
    }

    #[pyo3(name = "CreateNormalizeAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_normalize_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_normalize_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "ConnectableAPI")]
    fn connectable_api_py(&self) -> PyConnectableAPI {
        PyConnectableAPI::from_inner(self.inner.connectable_api())
    }

    #[pyo3(name = "CreateInput")]
    fn create_input_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyInput> {
        let tn = value_type_from_py_any(type_name)?;
        let inp = self
            .inner
            .create_input(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateInput failed"))?;
        Ok(PyInput::from_inner(inp))
    }

    #[pyo3(name = "CreateOutput")]
    fn create_output_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyOutput> {
        let tn = value_type_from_py_any(type_name)?;
        let out = self
            .inner
            .create_output(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateOutput failed"))?;
        Ok(PyOutput::from_inner(out))
    }

    #[pyo3(name = "GetInput")]
    fn get_input_py(&self, name: &str) -> Option<PyInput> {
        self.inner
            .get_input(&Token::new(name))
            .map(PyInput::from_inner)
    }

    #[pyo3(name = "GetOutput")]
    fn get_output_py(&self, name: &str) -> Option<PyOutput> {
        self.inner
            .get_output(&Token::new(name))
            .map(PyOutput::from_inner)
    }

    #[pyo3(name = "GetInputs", signature = (only_authored = false))]
    fn get_inputs_py(&self, only_authored: bool) -> Vec<PyInput> {
        self.inner
            .connectable_api()
            .get_inputs(only_authored)
            .into_iter()
            .map(PyInput::from_inner)
            .collect()
    }

    #[pyo3(name = "GetOutputs", signature = (only_authored = false))]
    fn get_outputs_py(&self, only_authored: bool) -> Vec<PyOutput> {
        self.inner
            .connectable_api()
            .get_outputs(only_authored)
            .into_iter()
            .map(PyOutput::from_inner)
            .collect()
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

#[pyclass(name = "ShapingAPI", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetShapingFocusAttr")]
    fn get_shaping_focus_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_focus_attr())
    }

    #[pyo3(name = "CreateShapingFocusAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_focus_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shaping_focus_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShapingFocusTintAttr")]
    fn get_shaping_focus_tint_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_focus_tint_attr())
    }

    #[pyo3(name = "CreateShapingFocusTintAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_focus_tint_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shaping_focus_tint_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShapingConeAngleAttr")]
    fn get_shaping_cone_angle_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_cone_angle_attr())
    }

    #[pyo3(name = "CreateShapingConeAngleAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_cone_angle_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shaping_cone_angle_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShapingConeSoftnessAttr")]
    fn get_shaping_cone_softness_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_cone_softness_attr())
    }

    #[pyo3(name = "CreateShapingConeSoftnessAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_cone_softness_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_shaping_cone_softness_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    #[pyo3(name = "GetShapingIesFileAttr")]
    fn get_shaping_ies_file_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_ies_file_attr())
    }

    #[pyo3(name = "CreateShapingIesFileAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_ies_file_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shaping_ies_file_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShapingIesAngleScaleAttr")]
    fn get_shaping_ies_angle_scale_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_ies_angle_scale_attr())
    }

    #[pyo3(name = "CreateShapingIesAngleScaleAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_ies_angle_scale_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_shaping_ies_angle_scale_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    #[pyo3(name = "GetShapingIesNormalizeAttr")]
    fn get_shaping_ies_normalize_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shaping_ies_normalize_attr())
    }

    #[pyo3(name = "CreateShapingIesNormalizeAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shaping_ies_normalize_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_shaping_ies_normalize_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    #[pyo3(name = "ConnectableAPI")]
    fn connectable_api_py(&self) -> PyConnectableAPI {
        PyConnectableAPI::from_inner(self.inner.connectable_api())
    }

    #[pyo3(name = "CreateInput")]
    fn create_input_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyInput> {
        let tn = value_type_from_py_any(type_name)?;
        let inp = self
            .inner
            .create_input(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateInput failed"))?;
        Ok(PyInput::from_inner(inp))
    }

    #[pyo3(name = "CreateOutput")]
    fn create_output_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyOutput> {
        let tn = value_type_from_py_any(type_name)?;
        let out = self
            .inner
            .create_output(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateOutput failed"))?;
        Ok(PyOutput::from_inner(out))
    }

    #[pyo3(name = "GetInput")]
    fn get_input_py(&self, name: &str) -> Option<PyInput> {
        self.inner
            .get_input(&Token::new(name))
            .map(PyInput::from_inner)
    }

    #[pyo3(name = "GetOutput")]
    fn get_output_py(&self, name: &str) -> Option<PyOutput> {
        self.inner
            .get_output(&Token::new(name))
            .map(PyOutput::from_inner)
    }

    #[pyo3(name = "GetInputs", signature = (only_authored = false))]
    fn get_inputs_py(&self, only_authored: bool) -> Vec<PyInput> {
        self.inner
            .connectable_api()
            .get_inputs(only_authored)
            .into_iter()
            .map(PyInput::from_inner)
            .collect()
    }

    #[pyo3(name = "GetOutputs", signature = (only_authored = false))]
    fn get_outputs_py(&self, only_authored: bool) -> Vec<PyOutput> {
        self.inner
            .connectable_api()
            .get_outputs(only_authored)
            .into_iter()
            .map(PyOutput::from_inner)
            .collect()
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

#[pyclass(name = "ShadowAPI", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetShadowEnableAttr")]
    fn get_shadow_enable_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shadow_enable_attr())
    }

    #[pyo3(name = "CreateShadowEnableAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shadow_enable_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shadow_enable_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShadowColorAttr")]
    fn get_shadow_color_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shadow_color_attr())
    }

    #[pyo3(name = "CreateShadowColorAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shadow_color_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shadow_color_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShadowDistanceAttr")]
    fn get_shadow_distance_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shadow_distance_attr())
    }

    #[pyo3(name = "CreateShadowDistanceAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shadow_distance_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shadow_distance_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShadowFalloffAttr")]
    fn get_shadow_falloff_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shadow_falloff_attr())
    }

    #[pyo3(name = "CreateShadowFalloffAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shadow_falloff_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shadow_falloff_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShadowFalloffGammaAttr")]
    fn get_shadow_falloff_gamma_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shadow_falloff_gamma_attr())
    }

    #[pyo3(name = "CreateShadowFalloffGammaAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shadow_falloff_gamma_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_shadow_falloff_gamma_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    #[pyo3(name = "ConnectableAPI")]
    fn connectable_api_py(&self) -> PyConnectableAPI {
        PyConnectableAPI::from_inner(self.inner.connectable_api())
    }

    #[pyo3(name = "CreateInput")]
    fn create_input_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyInput> {
        let tn = value_type_from_py_any(type_name)?;
        let inp = self
            .inner
            .create_input(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateInput failed"))?;
        Ok(PyInput::from_inner(inp))
    }

    #[pyo3(name = "CreateOutput")]
    fn create_output_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyOutput> {
        let tn = value_type_from_py_any(type_name)?;
        let out = self
            .inner
            .create_output(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateOutput failed"))?;
        Ok(PyOutput::from_inner(out))
    }

    #[pyo3(name = "GetInput")]
    fn get_input_py(&self, name: &str) -> Option<PyInput> {
        self.inner
            .get_input(&Token::new(name))
            .map(PyInput::from_inner)
    }

    #[pyo3(name = "GetOutput")]
    fn get_output_py(&self, name: &str) -> Option<PyOutput> {
        self.inner
            .get_output(&Token::new(name))
            .map(PyOutput::from_inner)
    }

    #[pyo3(name = "GetInputs", signature = (only_authored = false))]
    fn get_inputs_py(&self, only_authored: bool) -> Vec<PyInput> {
        self.inner
            .connectable_api()
            .get_inputs(only_authored)
            .into_iter()
            .map(PyInput::from_inner)
            .collect()
    }

    #[pyo3(name = "GetOutputs", signature = (only_authored = false))]
    fn get_outputs_py(&self, only_authored: bool) -> Vec<PyOutput> {
        self.inner
            .connectable_api()
            .get_outputs(only_authored)
            .into_iter()
            .map(PyOutput::from_inner)
            .collect()
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

#[pyclass(name = "BoundableLightBase", module = "pxr.UsdLux")]
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

#[pyclass(name = "NonboundableLightBase", module = "pxr.UsdLux")]
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

#[pyclass(name = "DiskLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetRadiusAttr")]
    fn get_radius_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_radius_attr())
    }

    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_radius_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_radius_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetTextureFileAttr")]
    fn get_texture_file_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_texture_file_attr())
    }

    #[pyo3(name = "CreateTextureFileAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_texture_file_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_texture_file_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "RectLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetWidthAttr")]
    fn get_width_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_width_attr())
    }

    #[pyo3(name = "CreateWidthAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_width_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_width_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetHeightAttr")]
    fn get_height_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_height_attr())
    }

    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_height_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_height_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetTextureFileAttr")]
    fn get_texture_file_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_texture_file_attr())
    }

    #[pyo3(name = "CreateTextureFileAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_texture_file_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_texture_file_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "SphereLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetRadiusAttr")]
    fn get_radius_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_radius_attr())
    }

    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_radius_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_radius_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetTreatAsPointAttr")]
    fn get_treat_as_point_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_treat_as_point_attr())
    }

    #[pyo3(name = "CreateTreatAsPointAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_treat_as_point_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_treat_as_point_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "CylinderLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetLengthAttr")]
    fn get_length_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_length_attr())
    }

    #[pyo3(name = "CreateLengthAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_length_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_length_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetRadiusAttr")]
    fn get_radius_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_radius_attr())
    }

    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_radius_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_radius_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetTreatAsLineAttr")]
    fn get_treat_as_line_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_treat_as_line_attr())
    }

    #[pyo3(name = "CreateTreatAsLineAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_treat_as_line_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_treat_as_line_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "DistantLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetAngleAttr")]
    fn get_angle_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_angle_attr())
    }

    #[pyo3(name = "CreateAngleAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_angle_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_angle_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "DomeLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetTextureFileAttr")]
    fn get_texture_file_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_texture_file_attr())
    }

    #[pyo3(name = "CreateTextureFileAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_texture_file_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_texture_file_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetTextureFormatAttr")]
    fn get_texture_format_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_texture_format_attr())
    }

    #[pyo3(name = "CreateTextureFormatAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_texture_format_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_texture_format_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetGuideRadiusAttr")]
    fn get_guide_radius_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_guide_radius_attr())
    }

    #[pyo3(name = "CreateGuideRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_guide_radius_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_guide_radius_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "DomeLight_1", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetTextureFileAttr")]
    fn get_texture_file_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_texture_file_attr())
    }

    #[pyo3(name = "CreateTextureFileAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_texture_file_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_texture_file_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetTextureFormatAttr")]
    fn get_texture_format_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_texture_format_attr())
    }

    #[pyo3(name = "CreateTextureFormatAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_texture_format_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_texture_format_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetGuideRadiusAttr")]
    fn get_guide_radius_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_guide_radius_attr())
    }

    #[pyo3(name = "CreateGuideRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_guide_radius_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_guide_radius_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetPoleAxisAttr")]
    fn get_pole_axis_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_pole_axis_attr())
    }

    #[pyo3(name = "CreatePoleAxisAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_pole_axis_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_pole_axis_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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
#[pyclass(name = "GeometryLight", module = "pxr.UsdLux")]
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

#[pyclass(name = "PortalLight", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.prim().clone())
    }

    #[pyo3(name = "GetWidthAttr")]
    fn get_width_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_width_attr())
    }

    #[pyo3(name = "CreateWidthAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_width_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_width_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetHeightAttr")]
    fn get_height_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_height_attr())
    }

    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_height_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_height_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
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

#[pyclass(name = "PluginLight", module = "pxr.UsdLux")]
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

#[pyclass(name = "LightFilter", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.prim().clone())
    }

    #[pyo3(name = "GetShaderIdAttr")]
    fn get_shader_id_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_shader_id_attr())
    }

    #[pyo3(name = "CreateShaderIdAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_shader_id_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner
                .create_shader_id_attr(lux_optional_default(default_value)?, write_sparsely),
        ))
    }

    #[pyo3(name = "GetShaderIdAttrForRenderContext")]
    fn get_shader_id_attr_for_render_context_py(&self, render_context: &str) -> PyAttribute {
        let t = Token::new(render_context);
        lux_attr_or_invalid(self.inner.get_shader_id_attr_for_render_context(&t))
    }

    #[pyo3(
        name = "CreateShaderIdAttrForRenderContext",
        signature = (render_context, default_value=None, write_sparsely=false)
    )]
    fn create_shader_id_attr_for_render_context_py(
        &self,
        render_context: &str,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(self.inner.create_shader_id_attr_for_render_context(
            &Token::new(render_context),
            lux_optional_default(default_value)?,
            write_sparsely,
        )))
    }

    #[pyo3(name = "ConnectableAPI")]
    fn connectable_api_py(&self) -> PyConnectableAPI {
        PyConnectableAPI::from_inner(self.inner.connectable_api())
    }

    #[pyo3(name = "CreateInput")]
    fn create_input_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyInput> {
        let tn = value_type_from_py_any(type_name)?;
        let inp = self
            .inner
            .create_input(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateInput failed"))?;
        Ok(PyInput::from_inner(inp))
    }

    #[pyo3(name = "CreateOutput")]
    fn create_output_py(
        &self,
        name: &str,
        type_name: &Bound<'_, PyAny>,
    ) -> PyResult<PyOutput> {
        let tn = value_type_from_py_any(type_name)?;
        let out = self
            .inner
            .create_output(&Token::new(name), &tn)
            .ok_or_else(|| PyValueError::new_err("CreateOutput failed"))?;
        Ok(PyOutput::from_inner(out))
    }

    #[pyo3(name = "GetInput")]
    fn get_input_py(&self, name: &str) -> Option<PyInput> {
        self.inner
            .get_input(&Token::new(name))
            .map(PyInput::from_inner)
    }

    #[pyo3(name = "GetOutput")]
    fn get_output_py(&self, name: &str) -> Option<PyOutput> {
        self.inner
            .get_output(&Token::new(name))
            .map(PyOutput::from_inner)
    }

    #[pyo3(name = "GetInputs", signature = (only_authored = false))]
    fn get_inputs_py(&self, only_authored: bool) -> Vec<PyInput> {
        self.inner
            .connectable_api()
            .get_inputs(only_authored)
            .into_iter()
            .map(PyInput::from_inner)
            .collect()
    }

    #[pyo3(name = "GetOutputs", signature = (only_authored = false))]
    fn get_outputs_py(&self, only_authored: bool) -> Vec<PyOutput> {
        self.inner
            .connectable_api()
            .get_outputs(only_authored)
            .into_iter()
            .map(PyOutput::from_inner)
            .collect()
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

#[pyclass(name = "PluginLightFilter", module = "pxr.UsdLux")]
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

#[pyclass(name = "MeshLightAPI", module = "pxr.UsdLux")]
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

#[pyclass(name = "VolumeLightAPI", module = "pxr.UsdLux")]
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

#[pyclass(name = "LightListAPI", module = "pxr.UsdLux")]
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

    #[pyo3(name = "GetPrim")]
    fn get_prim_py(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.inner.get_prim().clone())
    }

    #[pyo3(name = "GetLightListCacheBehaviorAttr")]
    fn get_light_list_cache_behavior_attr_py(&self) -> PyAttribute {
        lux_attr_or_invalid(self.inner.get_light_list_cache_behavior_attr())
    }

    #[pyo3(name = "CreateLightListCacheBehaviorAttr", signature = (default_value=None, write_sparsely=false))]
    fn create_light_list_cache_behavior_attr_py(
        &self,
        default_value: Option<&Bound<'_, PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        Ok(PyAttribute::from_attr(
            self.inner.create_light_list_cache_behavior_attr(
                lux_optional_default(default_value)?,
                write_sparsely,
            ),
        ))
    }

    /// ComputeLightList(mode="ignoreCache") -> list of light prim path strings
    #[pyo3(name = "ComputeLightList", signature = (mode = "ignoreCache"))]
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

#[pyclass(name = "Tokens", module = "pxr.UsdLux")]
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
