//! pxr.Sdr — Shader Definition Registry (`usd-sdr`).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use std::collections::HashMap;
use usd_tf::Token;

use usd_sdr::declare::SdrIdentifier;
use usd_sdr::parser_plugin::SdrParserPluginRef;
use usd_sdr::sdrosl_parser::SdrOslParserPlugin;
use usd_sdr::shader_node::SdrShaderNode;
use usd_sdr::shader_property::SdrShaderProperty;
use usd_sdr::tokens;
use usd_sdr::{SdrRegistry, SdrSdfTypeIndicator, SdrShaderNodeDiscoveryResult, SdrVersion};

use crate::sdf::PyValueTypeName;
use crate::sdr_shader_parser_test_utils;
use crate::tf::PyType;
use crate::vt::value_to_py;

fn token_map_to_dict(py: Python<'_>, m: &HashMap<Token, String>) -> PyResult<Py<PyAny>> {
    let d = PyDict::new(py);
    for (k, v) in m {
        d.set_item(k.as_str(), v.as_str())?;
    }
    Ok(d.into_any().unbind())
}

// --- Version ----------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Version", module = "pxr.Sdr")]
#[derive(Clone, Copy)]
pub struct PySdrVersion {
    inner: SdrVersion,
}

#[pymethods]
impl PySdrVersion {
    #[new]
    #[pyo3(signature = (major=None, minor=None))]
    fn new(major: Option<i32>, minor: Option<i32>) -> Self {
        let inner = match (major, minor) {
            (Some(ma), Some(mi)) => SdrVersion::new(ma, mi),
            _ => SdrVersion::default(),
        };
        Self { inner }
    }
}

// --- NodeDiscoveryResult ----------------------------------------------------

#[pyclass(name = "NodeDiscoveryResult", module = "pxr.Sdr")]
pub struct PyNodeDiscoveryResult {
    pub(crate) inner: SdrShaderNodeDiscoveryResult,
}

#[pymethods]
impl PyNodeDiscoveryResult {
    #[new]
    #[pyo3(
        signature = (
            identifier,
            version,
            name,
            family,
            discovery_type,
            source_type,
            uri,
            resolved_uri,
            *,
            source_code = None,
            metadata = None,
            blind_data = None,
            sub_identifier = None,
        )
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        identifier: &str,
        version: PyRef<PySdrVersion>,
        name: &str,
        family: &str,
        discovery_type: &str,
        source_type: &str,
        uri: &str,
        resolved_uri: &str,
        source_code: Option<&str>,
        metadata: Option<HashMap<String, String>>,
        blind_data: Option<&str>,
        sub_identifier: Option<&str>,
    ) -> PyResult<Self> {
        let mut meta: HashMap<Token, String> = HashMap::new();
        if let Some(m) = metadata {
            for (k, v) in m {
                meta.insert(Token::new(&k), v);
            }
        }
        let inner = SdrShaderNodeDiscoveryResult::new(
            Token::new(identifier),
            version.inner,
            name.to_string(),
            Token::new(family),
            Token::new(discovery_type),
            Token::new(source_type),
            uri.to_string(),
            resolved_uri.to_string(),
            source_code.unwrap_or("").to_string(),
            meta,
            blind_data.unwrap_or("").to_string(),
            Token::new(sub_identifier.unwrap_or("")),
        );
        Ok(Self { inner })
    }
}

// --- Shader property / node wrappers ----------------------------------------

#[pyclass(name = "ShaderProperty", module = "pxr.Sdr")]
pub struct PyShaderProperty {
    pub(crate) inner: SdrShaderProperty,
}

impl PyShaderProperty {
    fn new_prop(p: SdrShaderProperty) -> Self {
        Self { inner: p }
    }
}

#[pymethods]
impl PyShaderProperty {
    #[pyo3(name = "GetName")]
    fn get_name(&self) -> String {
        self.inner.get_name().as_str().to_string()
    }

    #[pyo3(name = "GetType")]
    fn get_type(&self) -> String {
        self.inner.get_type().as_str().to_string()
    }

    #[pyo3(name = "GetDefaultValue")]
    fn get_default_value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        value_to_py(py, self.inner.get_default_value())
    }

    #[pyo3(name = "IsOutput")]
    fn is_output(&self) -> bool {
        self.inner.is_output()
    }

    #[pyo3(name = "IsArray")]
    fn is_array(&self) -> bool {
        self.inner.is_array()
    }

    #[pyo3(name = "IsDynamicArray")]
    fn is_dynamic_array(&self) -> bool {
        self.inner.is_dynamic_array()
    }

    #[pyo3(name = "GetArraySize")]
    fn get_array_size(&self) -> i32 {
        self.inner.get_array_size()
    }

    #[pyo3(name = "GetTupleSize")]
    fn get_tuple_size(&self) -> i32 {
        self.inner.get_tuple_size()
    }

    #[pyo3(name = "GetInfoString")]
    fn get_info_string(&self) -> String {
        self.inner.get_info_string()
    }

    #[pyo3(name = "IsConnectable")]
    fn is_connectable(&self) -> bool {
        self.inner.is_connectable()
    }

    #[pyo3(name = "CanConnectTo")]
    fn can_connect_to(&self, other: &PyShaderProperty) -> bool {
        self.inner.can_connect_to(&other.inner)
    }

    #[pyo3(name = "GetMetadata")]
    fn get_metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        token_map_to_dict(py, self.inner.get_metadata())
    }

    #[pyo3(name = "GetLabel")]
    fn get_label(&self) -> String {
        self.inner.get_label().as_str().to_string()
    }

    #[pyo3(name = "GetHelp")]
    fn get_help(&self) -> String {
        self.inner.get_help().to_string()
    }

    #[pyo3(name = "GetPage")]
    fn get_page(&self) -> String {
        self.inner.get_page().as_str().to_string()
    }

    #[pyo3(name = "GetWidget")]
    fn get_widget(&self) -> String {
        self.inner.get_widget().as_str().to_string()
    }

    #[pyo3(name = "GetHints")]
    fn get_hints(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        token_map_to_dict(py, self.inner.get_hints())
    }

    #[pyo3(name = "GetOptions")]
    fn get_options(&self) -> Vec<(String, String)> {
        self.inner
            .get_options()
            .iter()
            .map(|(a, b)| (a.as_str().to_string(), b.as_str().to_string()))
            .collect()
    }

    #[pyo3(name = "GetVStructMemberOf")]
    fn get_vstruct_member_of(&self) -> String {
        self.inner.get_vstruct_member_of().as_str().to_string()
    }

    #[pyo3(name = "GetVStructMemberName")]
    fn get_vstruct_member_name(&self) -> String {
        self.inner.get_vstruct_member_name().as_str().to_string()
    }

    #[pyo3(name = "IsVStructMember")]
    fn is_vstruct_member(&self) -> bool {
        self.inner.is_vstruct_member()
    }

    #[pyo3(name = "IsVStruct")]
    fn is_vstruct(&self) -> bool {
        self.inner.is_vstruct()
    }

    #[pyo3(name = "GetValidConnectionTypes")]
    fn get_valid_connection_types(&self) -> Vec<String> {
        self.inner
            .get_valid_connection_types()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetTypeAsSdfType")]
    fn get_type_as_sdf_type(&self, py: Python<'_>) -> PyResult<Py<PySdfTypeIndicator>> {
        Py::new(
            py,
            PySdfTypeIndicator::new(self.inner.get_type_as_sdf_type()),
        )
    }

    #[pyo3(name = "IsAssetIdentifier")]
    fn is_asset_identifier(&self) -> bool {
        self.inner.is_asset_identifier()
    }

    #[pyo3(name = "GetImplementationName")]
    fn get_implementation_name(&self) -> String {
        self.inner.get_implementation_name()
    }
}

#[pyclass(name = "SdfTypeIndicator", module = "pxr.Sdr")]
pub struct PySdfTypeIndicator {
    inner: SdrSdfTypeIndicator,
}

impl PySdfTypeIndicator {
    fn new(inner: SdrSdfTypeIndicator) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySdfTypeIndicator {
    #[pyo3(name = "GetSdfType")]
    fn get_sdf_type(&self) -> PyValueTypeName {
        let sdf = self.inner.get_sdf_type();
        PyValueTypeName {
            name: sdf.as_token().as_str().to_string(),
        }
    }

    #[pyo3(name = "GetSdrType")]
    fn get_sdr_type(&self) -> String {
        self.inner.get_sdr_type().as_str().to_string()
    }
}

#[pyclass(name = "ShaderNode", module = "pxr.Sdr")]
pub struct PyShaderNode {
    pub(crate) inner: SdrShaderNode,
}

impl PyShaderNode {
    fn new_node(n: SdrShaderNode) -> Self {
        Self { inner: n }
    }
}

#[pymethods]
impl PyShaderNode {
    #[pyo3(name = "GetMetadata")]
    fn get_metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        token_map_to_dict(py, self.inner.get_metadata())
    }

    #[pyo3(name = "GetName")]
    fn get_name(&self) -> String {
        self.inner.get_name().to_string()
    }

    #[pyo3(name = "GetContext")]
    fn get_context(&self) -> String {
        self.inner.get_context().as_str().to_string()
    }

    #[pyo3(name = "GetSourceType")]
    fn get_source_type(&self) -> String {
        self.inner.get_source_type().as_str().to_string()
    }

    #[pyo3(name = "GetFunction")]
    fn get_function(&self) -> String {
        // C++ `SdrShaderNode::GetFunction` — entry point in the shader source; aligns with
        // `get_implementation_name()` in `usd_sdr` (see `shader_node.rs`).
        self.inner.get_implementation_name()
    }

    #[pyo3(name = "GetResolvedDefinitionURI")]
    fn get_resolved_definition_uri(&self) -> String {
        self.inner.get_resolved_definition_uri().to_string()
    }

    #[pyo3(name = "GetResolvedImplementationURI")]
    fn get_resolved_implementation_uri(&self) -> String {
        self.inner.get_resolved_implementation_uri().to_string()
    }

    #[pyo3(name = "GetSourceCode")]
    fn get_source_code(&self) -> String {
        self.inner.get_source_code().to_string()
    }

    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    #[pyo3(name = "GetShaderInputNames")]
    fn get_shader_input_names(&self) -> Vec<String> {
        self.inner
            .get_shader_input_names()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetShaderOutputNames")]
    fn get_shader_output_names(&self) -> Vec<String> {
        self.inner
            .get_shader_output_names()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetShaderInput")]
    fn get_shader_input(&self, py: Python<'_>, name: &str) -> Option<Py<PyShaderProperty>> {
        let t = Token::new(name);
        self.inner
            .get_shader_input(&t)
            .cloned()
            .map(PyShaderProperty::new_prop)
            .and_then(|w| Py::new(py, w).ok())
    }

    #[pyo3(name = "GetShaderOutput")]
    fn get_shader_output(&self, py: Python<'_>, name: &str) -> Option<Py<PyShaderProperty>> {
        let t = Token::new(name);
        self.inner
            .get_shader_output(&t)
            .cloned()
            .map(PyShaderProperty::new_prop)
            .and_then(|w| Py::new(py, w).ok())
    }

    #[pyo3(name = "GetLabel")]
    fn get_label(&self) -> String {
        self.inner.get_label().as_str().to_string()
    }

    #[pyo3(name = "GetCategory")]
    fn get_category(&self) -> String {
        self.inner.get_category().as_str().to_string()
    }

    #[pyo3(name = "GetHelp")]
    fn get_help(&self) -> String {
        self.inner.get_help()
    }

    #[pyo3(name = "GetDepartments")]
    fn get_departments(&self) -> Vec<String> {
        self.inner
            .get_departments()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetPages")]
    fn get_pages(&self) -> Vec<String> {
        self.inner
            .get_pages()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetOpenPages")]
    fn get_open_pages(&self) -> Vec<String> {
        self.inner
            .get_open_pages()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetPrimvars")]
    fn get_primvars(&self) -> Vec<String> {
        self.inner
            .get_primvars()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetAdditionalPrimvarProperties")]
    fn get_additional_primvar_properties(&self) -> Vec<String> {
        self.inner
            .get_additional_primvar_properties()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetPropertyNamesForPage")]
    fn get_property_names_for_page(&self, page: &str) -> Vec<String> {
        self.inner
            .get_property_names_for_page(page)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetAllVstructNames")]
    fn get_all_vstruct_names(&self) -> Vec<String> {
        self.inner
            .get_all_vstruct_names()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }
}

// --- Registry ---------------------------------------------------------------

#[pyclass(name = "Registry", module = "pxr.Sdr")]
pub struct PySdrRegistry;

#[pymethods]
impl PySdrRegistry {
    #[new]
    fn new() -> Self {
        Self
    }

    #[pyo3(name = "AddDiscoveryResult")]
    fn add_discovery_result(&self, res: &PyNodeDiscoveryResult) {
        SdrRegistry::get_instance().add_discovery_result(res.inner.clone());
    }

    #[pyo3(name = "SetExtraParserPlugins")]
    fn set_extra_parser_plugins(&self, plugins: Bound<'_, PyAny>) -> PyResult<()> {
        let list = plugins.cast::<PyList>()?;
        let mut out: Vec<SdrParserPluginRef> = Vec::new();
        for item in list.iter() {
            if let Ok(pyty) = item.cast::<PyType>() {
                let name = pyty.borrow().inner.type_name();
                if name.contains("SdrOslParserPlugin") || name.contains("OslParser") {
                    out.push(Box::new(SdrOslParserPlugin::new()) as SdrParserPluginRef);
                }
            }
        }
        if !out.is_empty() {
            SdrRegistry::get_instance().set_extra_parser_plugins(out);
        }
        Ok(())
    }

    #[pyo3(name = "GetShaderNodeByIdentifier")]
    fn get_shader_node_by_identifier(
        &self,
        py: Python<'_>,
        id: &str,
        type_priority: Option<Vec<String>>,
    ) -> PyResult<Option<Py<PyShaderNode>>> {
        let ident: SdrIdentifier = Token::new(id);
        let priority: Vec<Token> = type_priority
            .unwrap_or_default()
            .into_iter()
            .map(|s| Token::new(s.as_str()))
            .collect();
        let node = SdrRegistry::get_instance().get_shader_node_by_identifier(&ident, &priority);
        Ok(node
            .map(|n| Py::new(py, PyShaderNode::new_node(n.clone())))
            .transpose()?)
    }
}

#[pyfunction]
#[pyo3(name = "_ValidateProperty")]
fn validate_property(node: &PyShaderNode, prop: &PyShaderProperty) -> bool {
    let name = prop.inner.get_name();
    node.inner.get_shader_input(name).is_some() || node.inner.get_shader_output(name).is_some()
}

/// `Sdr.PropertyTypes` — string tokens for shader property kinds (native, no `types.SimpleNamespace`).
#[pyclass(name = "PropertyTypes", module = "pxr.Sdr")]
pub struct PySdrPropertyTypes;

#[pymethods]
impl PySdrPropertyTypes {
    #[classattr]
    #[pyo3(name = "Int")]
    fn int_token() -> &'static str {
        tokens().property_types.int.as_str()
    }
    #[classattr]
    #[pyo3(name = "String")]
    fn string_token() -> &'static str {
        tokens().property_types.string.as_str()
    }
    #[classattr]
    #[pyo3(name = "Float")]
    fn float_token() -> &'static str {
        tokens().property_types.float.as_str()
    }
    #[classattr]
    #[pyo3(name = "Color")]
    fn color_token() -> &'static str {
        tokens().property_types.color.as_str()
    }
    #[classattr]
    #[pyo3(name = "Color4")]
    fn color4_token() -> &'static str {
        tokens().property_types.color4.as_str()
    }
    #[classattr]
    #[pyo3(name = "Point")]
    fn point_token() -> &'static str {
        tokens().property_types.point.as_str()
    }
    #[classattr]
    #[pyo3(name = "Normal")]
    fn normal_token() -> &'static str {
        tokens().property_types.normal.as_str()
    }
    #[classattr]
    #[pyo3(name = "Vector")]
    fn vector_token() -> &'static str {
        tokens().property_types.vector.as_str()
    }
    #[classattr]
    #[pyo3(name = "Matrix")]
    fn matrix_token() -> &'static str {
        tokens().property_types.matrix.as_str()
    }
    #[classattr]
    #[pyo3(name = "Struct")]
    fn struct_token() -> &'static str {
        tokens().property_types.struct_type.as_str()
    }
    #[classattr]
    #[pyo3(name = "Terminal")]
    fn terminal_token() -> &'static str {
        tokens().property_types.terminal.as_str()
    }
    #[classattr]
    #[pyo3(name = "Vstruct")]
    fn vstruct_token() -> &'static str {
        tokens().property_types.vstruct.as_str()
    }
    #[classattr]
    #[pyo3(name = "Unknown")]
    fn unknown_token() -> &'static str {
        tokens().property_types.unknown.as_str()
    }
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySdrVersion>()?;
    m.add_class::<PyNodeDiscoveryResult>()?;
    m.add_class::<PyShaderProperty>()?;
    m.add_class::<PySdfTypeIndicator>()?;
    m.add_class::<PyShaderNode>()?;
    m.add_class::<PySdrRegistry>()?;
    m.add_function(wrap_pyfunction!(validate_property, m)?)?;
    m.add_class::<PySdrPropertyTypes>()?;
    sdr_shader_parser_test_utils::register(py, m)?;
    Ok(())
}
