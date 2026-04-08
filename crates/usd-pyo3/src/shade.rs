//! pxr.UsdShade — Python bindings for the UsdShade schema library.
//!
//! Mirrors the C++ API: UsdShadeMaterial, UsdShadeShader, UsdShadeNodeGraph,
//! UsdShadeInput, UsdShadeOutput, UsdShadeConnectableAPI, UsdShadeMaterialBindingAPI,
//! UsdShadeCoordSysAPI.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

use usd_core::{Relationship, Stage};
use usd_sdf::{Path, TimeCode};
use usd_shade::{
    ConnectableAPI, CoordSysAPI, Input, Material, MaterialBindingAPI, NodeGraph, Output, Shader,
};
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn path_from_str(s: &str) -> PyResult<Path> {
    Path::from_string(s).ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: {s}")))
}

// ---------------------------------------------------------------------------
// Shared PyStage wrapper
// We carry Arc<Stage> so the stage stays alive as long as any Python object holds it.
// ---------------------------------------------------------------------------

#[pyclass(name = "Stage", module = "pxr_rs.UsdShade")]
struct PyStage {
    inner: Arc<Stage>,
}

// ---------------------------------------------------------------------------
// PyMaterial — wraps UsdShadeMaterial
// ---------------------------------------------------------------------------

#[pyclass(name = "Material", module = "pxr_rs.UsdShade")]
struct PyMaterial {
    inner: Material,
}

#[pymethods]
impl PyMaterial {
    /// Material.Get(stage, path) -> Material
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: Material::get(&stage.inner, &p),
        })
    }

    /// Material.Define(stage, path) -> Material
    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: Material::define(&stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.path().to_string()
    }

    /// CreateSurfaceOutput(render_context="") -> Output
    #[pyo3(signature = (render_context = ""))]
    fn create_surface_output(&self, render_context: &str) -> PyOutput {
        PyOutput {
            inner: self
                .inner
                .create_surface_output(&Token::new(render_context)),
        }
    }

    /// GetSurfaceOutput(render_context="") -> Output
    #[pyo3(signature = (render_context = ""))]
    fn get_surface_output(&self, render_context: &str) -> PyOutput {
        PyOutput {
            inner: self.inner.get_surface_output(&Token::new(render_context)),
        }
    }

    /// ComputeSurfaceSource(render_contexts=[]) -> (Shader, outputName, outputType) or None
    ///
    /// Returns (Shader, name, attr_type_str) if a source is found, else None.
    #[pyo3(signature = (render_contexts = vec![]))]
    fn compute_surface_source(
        &self,
        render_contexts: Vec<String>,
    ) -> Option<(PyShader, String, String)> {
        let contexts: Vec<Token> = render_contexts.iter().map(|s| Token::new(s)).collect();
        let mut source_name = Token::new("");
        let mut source_type = usd_shade::types::AttributeType::Invalid;
        let shader =
            self.inner
                .compute_surface_source(&contexts, &mut source_name, &mut source_type);
        if shader.is_valid() {
            Some((
                PyShader { inner: shader },
                source_name.as_str().to_string(),
                format!("{source_type:?}"),
            ))
        } else {
            None
        }
    }

    /// ComputeDisplacementSource(render_contexts=[]) -> (Shader, outputName, outputType) or None
    #[pyo3(signature = (render_contexts = vec![]))]
    fn compute_displacement_source(
        &self,
        render_contexts: Vec<String>,
    ) -> Option<(PyShader, String, String)> {
        let contexts: Vec<Token> = render_contexts.iter().map(|s| Token::new(s)).collect();
        let mut source_name = Token::new("");
        let mut source_type = usd_shade::types::AttributeType::Invalid;
        let shader =
            self.inner
                .compute_displacement_source(&contexts, &mut source_name, &mut source_type);
        if shader.is_valid() {
            Some((
                PyShader { inner: shader },
                source_name.as_str().to_string(),
                format!("{source_type:?}"),
            ))
        } else {
            None
        }
    }

    /// SetBaseMaterial(base_material)
    fn set_base_material(&self, base: &PyMaterial) {
        self.inner.set_base_material(&base.inner);
    }

    /// GetBaseMaterial() -> Material
    fn get_base_material(&self) -> PyMaterial {
        PyMaterial {
            inner: self.inner.get_base_material(),
        }
    }

    /// ClearBaseMaterial()
    fn clear_base_material(&self) {
        self.inner.clear_base_material();
    }

    fn __repr__(&self) -> String {
        format!("UsdShade.Material('{}')", self.inner.path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyShader — wraps UsdShadeShader
// ---------------------------------------------------------------------------

#[pyclass(name = "Shader", module = "pxr_rs.UsdShade")]
struct PyShader {
    inner: Shader,
}

#[pymethods]
impl PyShader {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: Shader::get(&stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: Shader::define(&stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.path().to_string()
    }

    /// GetShaderId() -> str or None
    fn get_shader_id(&self) -> Option<String> {
        self.inner.get_shader_id().map(|t| t.as_str().to_string())
    }

    /// SetShaderId(id) -> bool
    fn set_shader_id(&self, id: &str) -> bool {
        self.inner.set_shader_id(&Token::new(id))
    }

    /// CreateInput(name, type_name) -> Input
    fn create_input(&self, name: &str, type_name: &str) -> PyInput {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        PyInput {
            inner: self.inner.create_input(&Token::new(name), &tn),
        }
    }

    /// CreateOutput(name, type_name) -> Output
    fn create_output(&self, name: &str, type_name: &str) -> PyOutput {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        PyOutput {
            inner: self.inner.create_output(&Token::new(name), &tn),
        }
    }

    /// GetInput(name) -> Input
    fn get_input(&self, name: &str) -> PyInput {
        PyInput {
            inner: self.inner.get_input(&Token::new(name)),
        }
    }

    /// GetOutput(name) -> Output
    fn get_output(&self, name: &str) -> PyOutput {
        PyOutput {
            inner: self.inner.get_output(&Token::new(name)),
        }
    }

    /// GetInputs(only_authored=False) -> list[Input]
    #[pyo3(signature = (only_authored = false))]
    fn get_inputs(&self, only_authored: bool) -> Vec<PyInput> {
        self.inner
            .get_inputs(only_authored)
            .into_iter()
            .map(|i| PyInput { inner: i })
            .collect()
    }

    /// GetOutputs(only_authored=False) -> list[Output]
    #[pyo3(signature = (only_authored = false))]
    fn get_outputs(&self, only_authored: bool) -> Vec<PyOutput> {
        self.inner
            .get_outputs(only_authored)
            .into_iter()
            .map(|o| PyOutput { inner: o })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!("UsdShade.Shader('{}')", self.inner.path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyNodeGraph — wraps UsdShadeNodeGraph
// ---------------------------------------------------------------------------

#[pyclass(name = "NodeGraph", module = "pxr_rs.UsdShade")]
struct PyNodeGraph {
    inner: NodeGraph,
}

#[pymethods]
impl PyNodeGraph {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: NodeGraph::get(&stage.inner, &p),
        })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: NodeGraph::define(&stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.path().to_string()
    }

    fn create_input(&self, name: &str, type_name: &str) -> PyInput {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        PyInput {
            inner: self.inner.create_input(&Token::new(name), &tn),
        }
    }

    fn create_output(&self, name: &str, type_name: &str) -> PyOutput {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        PyOutput {
            inner: self.inner.create_output(&Token::new(name), &tn),
        }
    }

    fn get_input(&self, name: &str) -> PyInput {
        PyInput {
            inner: self.inner.get_input(&Token::new(name)),
        }
    }

    fn get_output(&self, name: &str) -> PyOutput {
        PyOutput {
            inner: self.inner.get_output(&Token::new(name)),
        }
    }

    /// ComputeOutputSource(output_name) -> (Shader, outputName, outputType) or None
    fn compute_output_source(&self, output_name: &str) -> Option<(PyShader, String, String)> {
        let mut source_name = Token::new("");
        let mut source_type = usd_shade::types::AttributeType::Invalid;
        let shader = self.inner.compute_output_source(
            &Token::new(output_name),
            &mut source_name,
            &mut source_type,
        );
        if shader.is_valid() {
            Some((
                PyShader { inner: shader },
                source_name.as_str().to_string(),
                format!("{source_type:?}"),
            ))
        } else {
            None
        }
    }

    fn __repr__(&self) -> String {
        format!("UsdShade.NodeGraph('{}')", self.inner.path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyInput — wraps UsdShadeInput
// ---------------------------------------------------------------------------

#[pyclass(name = "Input", module = "pxr_rs.UsdShade")]
struct PyInput {
    inner: Input,
}

#[pymethods]
impl PyInput {
    fn is_valid(&self) -> bool {
        self.inner.is_defined()
    }

    fn get_full_name(&self) -> String {
        self.inner.get_full_name().as_str().to_string()
    }

    fn get_base_name(&self) -> String {
        self.inner.get_base_name().as_str().to_string()
    }

    fn get_type_name(&self) -> String {
        self.inner.get_type_name().to_string()
    }

    /// Set(value, time=default_time) — sets float value; other types not yet bridged via VtValue
    #[pyo3(signature = (value, time = 0.0))]
    fn set(&self, value: f64, time: f64) -> bool {
        self.inner
            .set(Value::from(value as f32), TimeCode::new(time))
    }

    /// ConnectToSource(source_path) -> bool
    fn connect_to_source(&self, source_path: &str) -> PyResult<bool> {
        let p = path_from_str(source_path)?;
        Ok(self.inner.connect_to_source_path(&p))
    }

    /// HasConnectedSource() -> bool
    fn has_connected_source(&self) -> bool {
        self.inner.has_connected_source()
    }

    /// GetAttr() — returns full attribute name string (bridging without full Attribute binding)
    fn get_attr(&self) -> String {
        self.inner.get_full_name().as_str().to_string()
    }

    /// CanConnect — stub; returns True for a valid, defined input
    fn can_connect(&self) -> bool {
        self.inner.is_defined()
    }

    /// IsInput(full_name) — class-level predicate
    #[staticmethod]
    fn is_input(full_name: &str) -> bool {
        Input::is_interface_input_name(full_name)
    }

    fn __repr__(&self) -> String {
        format!("UsdShade.Input('{}')", self.inner.get_full_name().as_str())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_defined()
    }
}

// ---------------------------------------------------------------------------
// PyOutput — wraps UsdShadeOutput
// ---------------------------------------------------------------------------

#[pyclass(name = "Output", module = "pxr_rs.UsdShade")]
struct PyOutput {
    inner: Output,
}

#[pymethods]
impl PyOutput {
    fn is_valid(&self) -> bool {
        self.inner.is_defined()
    }

    fn get_full_name(&self) -> String {
        self.inner.get_full_name().as_str().to_string()
    }

    fn get_base_name(&self) -> String {
        self.inner.get_base_name().as_str().to_string()
    }

    fn get_type_name(&self) -> String {
        self.inner.get_type_name().to_string()
    }

    fn connect_to_source(&self, source_path: &str) -> PyResult<bool> {
        let p = path_from_str(source_path)?;
        Ok(self.inner.connect_to_source_path(&p))
    }

    fn has_connected_source(&self) -> bool {
        self.inner.has_connected_source()
    }

    /// GetAttr() — returns full attribute name string
    fn get_attr(&self) -> String {
        self.inner.get_full_name().as_str().to_string()
    }

    /// IsOutput(full_name) — class-level predicate
    #[staticmethod]
    fn is_output(full_name: &str) -> bool {
        full_name.starts_with("outputs:")
    }

    fn __repr__(&self) -> String {
        format!("UsdShade.Output('{}')", self.inner.get_full_name().as_str())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_defined()
    }
}

// ---------------------------------------------------------------------------
// PyConnectableAPI — wraps UsdShadeConnectableAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "ConnectableAPI", module = "pxr_rs.UsdShade")]
struct PyConnectableAPI {
    inner: ConnectableAPI,
}

#[pymethods]
impl PyConnectableAPI {
    /// ConnectableAPI.Get(stage, path) -> ConnectableAPI
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at path: {path}")))?;
        Ok(Self {
            inner: ConnectableAPI::new(prim),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn create_input(&self, name: &str, type_name: &str) -> PyInput {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        PyInput {
            inner: self.inner.create_input(&Token::new(name), &tn),
        }
    }

    fn create_output(&self, name: &str, type_name: &str) -> PyOutput {
        let tn = usd_sdf::ValueTypeRegistry::instance().find_type(type_name);
        PyOutput {
            inner: self.inner.create_output(&Token::new(name), &tn),
        }
    }

    fn __repr__(&self) -> String {
        "UsdShade.ConnectableAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyMaterialBindingAPI — wraps UsdShadeMaterialBindingAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "MaterialBindingAPI", module = "pxr_rs.UsdShade")]
struct PyMaterialBindingAPI {
    inner: MaterialBindingAPI,
}

#[pymethods]
impl PyMaterialBindingAPI {
    /// MaterialBindingAPI.Apply(prim) -> MaterialBindingAPI
    ///
    /// Python: MaterialBindingAPI.Apply(stage, path)
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at path: {path}")))?;
        Ok(Self {
            inner: MaterialBindingAPI::apply(&prim),
        })
    }

    /// MaterialBindingAPI.Get(stage, path)
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: MaterialBindingAPI::get(&stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Bind(material, strength="weakerThanDescendants", purpose="") -> bool
    #[pyo3(signature = (material, strength = "weakerThanDescendants", purpose = ""))]
    fn bind(&self, material: &PyMaterial, strength: &str, purpose: &str) -> bool {
        self.inner
            .bind(&material.inner, &Token::new(strength), &Token::new(purpose))
    }

    /// UnbindDirectBinding(purpose="") -> bool
    #[pyo3(signature = (purpose = ""))]
    fn unbind_direct_binding(&self, purpose: &str) -> bool {
        self.inner.unbind_direct_binding(&Token::new(purpose))
    }

    /// ComputeBoundMaterial(purpose="") -> Material or None
    ///
    /// Returns the resolved Material (may be invalid/None if nothing is bound).
    #[pyo3(signature = (purpose = ""))]
    fn compute_bound_material(&self, purpose: &str) -> Option<PyMaterial> {
        let mut binding_rel: Option<Relationship> = None;
        let mat = self
            .inner
            .compute_bound_material(&Token::new(purpose), &mut binding_rel, false);
        if mat.is_valid() {
            Some(PyMaterial { inner: mat })
        } else {
            None
        }
    }

    /// GetDirectBinding(purpose="") -> Material or None
    #[pyo3(signature = (purpose = ""))]
    fn get_direct_binding(&self, purpose: &str) -> Option<PyMaterial> {
        let binding = self.inner.get_direct_binding(&Token::new(purpose));
        let mat = binding.get_material();
        if mat.is_valid() {
            Some(PyMaterial { inner: mat })
        } else {
            None
        }
    }

    fn __repr__(&self) -> String {
        "UsdShade.MaterialBindingAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyCoordSysAPI — wraps UsdShadeCoordSysAPI (MultipleApply)
// ---------------------------------------------------------------------------

#[pyclass(name = "CoordSysAPI", module = "pxr_rs.UsdShade")]
struct PyCoordSysAPI {
    inner: CoordSysAPI,
}

#[pymethods]
impl PyCoordSysAPI {
    /// CoordSysAPI.Apply(stage, path, name) -> CoordSysAPI
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str, name: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at path: {path}")))?;
        Ok(Self {
            inner: CoordSysAPI::apply(&prim, &Token::new(name)),
        })
    }

    /// CoordSysAPI.Get(stage, path) -> CoordSysAPI
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self {
            inner: CoordSysAPI::get(&stage.inner, &p),
        })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_name(&self) -> String {
        self.inner.name().as_str().to_string()
    }

    /// Bind(target_path) -> bool
    fn bind(&self, target_path: &str) -> PyResult<bool> {
        let target = path_from_str(target_path)?;
        Ok(self.inner.bind(&target))
    }

    /// ClearBinding(remove_spec=False) -> bool
    #[pyo3(signature = (remove_spec = false))]
    fn clear_binding(&self, remove_spec: bool) -> bool {
        self.inner.clear_binding(remove_spec)
    }

    fn __repr__(&self) -> String {
        format!("UsdShade.CoordSysAPI('{}')", self.inner.name().as_str())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// Tokens — mirrors UsdShadeTokens
// ---------------------------------------------------------------------------

#[pyclass(name = "Tokens", module = "pxr_rs.UsdShade")]
struct PyTokens;

#[pymethods]
impl PyTokens {
    #[getter]
    fn allPurpose(&self) -> &str {
        "allPurpose"
    }
    #[getter]
    fn displacement(&self) -> &str {
        "displacement"
    }
    #[getter]
    fn full(&self) -> &str {
        "full"
    }
    #[getter]
    fn inputs(&self) -> &str {
        "inputs:"
    }
    #[getter]
    fn interfaceOnly(&self) -> &str {
        "interfaceOnly"
    }
    #[getter]
    fn materialBind(&self) -> &str {
        "materialBind"
    }
    #[getter]
    fn outputs(&self) -> &str {
        "outputs:"
    }
    #[getter]
    fn surface(&self) -> &str {
        "surface"
    }
    #[getter]
    fn volume(&self) -> &str {
        "volume"
    }
    #[getter]
    fn id(&self) -> &str {
        "info:id"
    }
    #[getter]
    fn sdrMetadata(&self) -> &str {
        "sdrMetadata"
    }
    #[getter]
    fn weakerThanDescendants(&self) -> &str {
        "weakerThanDescendants"
    }
    #[getter]
    fn strongerThanDescendants(&self) -> &str {
        "strongerThanDescendants"
    }
    #[getter]
    fn preview(&self) -> &str {
        "preview"
    }
}

// ---------------------------------------------------------------------------
// AttributeType — mirrors UsdShadeAttributeType enum
// ---------------------------------------------------------------------------

#[pyclass(name = "AttributeType", module = "pxr_rs.UsdShade")]
struct PyAttributeType;

#[pymethods]
impl PyAttributeType {
    #[classattr]
    fn Invalid() -> i32 {
        0
    }
    #[classattr]
    fn Input() -> i32 {
        1
    }
    #[classattr]
    fn Output() -> i32 {
        2
    }
}

// ---------------------------------------------------------------------------
// Utils — mirrors UsdShadeUtils free functions
// ---------------------------------------------------------------------------

#[pyclass(name = "Utils", module = "pxr_rs.UsdShade")]
struct PyUtils;

#[pymethods]
impl PyUtils {
    /// GetType(name) -> AttributeType enum value (0=Invalid, 1=Input, 2=Output)
    #[staticmethod]
    #[pyo3(name = "GetType")]
    fn get_type(name: &str) -> i32 {
        if name.starts_with("inputs:") {
            1
        } else if name.starts_with("outputs:") {
            2
        } else {
            0
        }
    }

    /// GetBaseNameAndType(name) -> (baseName, attrType)
    #[staticmethod]
    #[pyo3(name = "GetBaseNameAndType")]
    fn get_base_name_and_type(name: &str) -> (String, i32) {
        if let Some(base) = name.strip_prefix("inputs:") {
            (base.to_string(), 1)
        } else if let Some(base) = name.strip_prefix("outputs:") {
            (base.to_string(), 2)
        } else {
            (name.to_string(), 0)
        }
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStage>()?;
    m.add_class::<PyMaterial>()?;
    m.add_class::<PyShader>()?;
    m.add_class::<PyNodeGraph>()?;
    m.add_class::<PyInput>()?;
    m.add_class::<PyOutput>()?;
    m.add_class::<PyConnectableAPI>()?;
    m.add_class::<PyMaterialBindingAPI>()?;
    m.add_class::<PyCoordSysAPI>()?;
    m.add_class::<PyTokens>()?;
    m.add_class::<PyAttributeType>()?;
    m.add_class::<PyUtils>()?;

    // Singleton, matching `UsdShadeTokens` in C++
    m.add("Tokens", PyTokens)?;

    Ok(())
}
