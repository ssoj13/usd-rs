//! pxr.Sdr — Shader Definition Registry (`usd-sdr`).

use pyo3::basic::CompareOp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use usd_tf::Token;
use usd_vt::Value;

use usd_sdr::declare::{SdrIdentifier, SdrVersionFilter};
use usd_sdr::discovery_plugin::SdrDiscoveryPluginRef;
use usd_sdr::osl_parser::OslParserPlugin;
use usd_sdr::parser_plugin::SdrParserPluginRef;
use usd_sdr::shader_node::SdrShaderNode;
use usd_sdr::shader_node_query::{SdrShaderNodeQuery, SdrShaderNodeQueryResult};
use usd_sdr::shader_node_query_utils::{group_query_results, GroupedQueryResult};
use usd_sdr::shader_property::SdrShaderProperty;
use usd_sdr::tokens;
use usd_sdr::{
    fs_helpers_discover_files, split_shader_identifier, SdrDiscoveryPlugin, SdrDiscoveryUri,
    SdrFilesystemDiscoveryPlugin, SdrRegistry, SdrSdfTypeIndicator, SdrShaderNodeDiscoveryResult,
    SdrStandardFilesystemDiscoveryContext, SdrVersion,
};
use usd_shade::UsdShadeShaderDefParserPlugin;

use crate::sdf::PyValueTypeName;
use crate::sdr_shader_parser_test_utils;
use crate::tf::PyType;
use crate::vt::{py_to_value, value_to_py};

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
            (Some(ma), None) => SdrVersion::new(ma, 0),
            (None, None) => SdrVersion::default(),
            (None, Some(_)) => SdrVersion::default(),
        };
        Self { inner }
    }

    #[pyo3(name = "GetMajor")]
    fn get_major(&self) -> i32 {
        self.inner.major()
    }

    #[pyo3(name = "GetMinor")]
    fn get_minor(&self) -> i32 {
        self.inner.minor()
    }

    #[pyo3(name = "IsDefault")]
    fn is_default_version(&self) -> bool {
        self.inner.is_default()
    }

    #[pyo3(name = "GetAsDefault")]
    fn get_as_default(&self) -> Self {
        Self {
            inner: self.inner.as_default(),
        }
    }

    #[pyo3(name = "GetStringSuffix")]
    fn get_string_suffix(&self) -> String {
        self.inner.get_string_suffix()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __str__(&self) -> String {
        self.inner.get_string()
    }

    /// Matches `pxr/usd/sdr/wrapDeclare.cpp` `_Repr` (`TF_PY_REPR_PREFIX` → `Sdr.` for `pxr.Sdr`).
    fn __repr__(&self) -> String {
        let mut s = String::from("Sdr.");
        if !self.inner.is_valid() {
            s.push_str("Version()");
        } else {
            let _ = write!(
                s,
                "Version({}, {})",
                self.inner.major(),
                self.inner.minor()
            );
        }
        if self.inner.is_default() {
            s.push_str(".GetAsDefault()");
        }
        s
    }

    fn __hash__(&self) -> u64 {
        self.inner.get_hash()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other = match other.extract::<PyRef<PySdrVersion>>() {
            Ok(o) => o,
            Err(_) => {
                return match op {
                    CompareOp::Eq => Ok(false),
                    CompareOp::Ne => Ok(true),
                    _ => Err(PyTypeError::new_err(
                        "ordering is not supported between these types",
                    )),
                };
            }
        };
        let ord = self.inner.cmp(&other.inner);
        Ok(match op {
            CompareOp::Eq => self.inner == other.inner,
            CompareOp::Ne => self.inner != other.inner,
            CompareOp::Lt => ord == Ordering::Less,
            CompareOp::Le => matches!(ord, Ordering::Less | Ordering::Equal),
            CompareOp::Gt => ord == Ordering::Greater,
            CompareOp::Ge => matches!(ord, Ordering::Greater | Ordering::Equal),
        })
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
    #[allow(non_snake_case)]
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
            sourceCode = None,
            metadata = None,
            blindData = None,
            subIdentifier = None,
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
        sourceCode: Option<&str>,
        metadata: Option<HashMap<String, String>>,
        blindData: Option<&str>,
        subIdentifier: Option<&str>,
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
            sourceCode.unwrap_or("").to_string(),
            meta,
            blindData.unwrap_or("").to_string(),
            Token::new(subIdentifier.unwrap_or("")),
        );
        Ok(Self { inner })
    }

    #[getter]
    fn identifier(&self) -> String {
        self.inner.identifier.as_str().to_string()
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn family(&self) -> String {
        self.inner.family.as_str().to_string()
    }

    #[getter]
    fn version(&self) -> PySdrVersion {
        PySdrVersion {
            inner: self.inner.version,
        }
    }

    #[getter]
    fn uri(&self) -> String {
        self.inner.uri.clone()
    }

    #[getter]
    #[pyo3(name = "resolvedUri")]
    fn resolved_uri(&self) -> String {
        self.inner.resolved_uri.clone()
    }

    #[getter]
    #[pyo3(name = "discoveryType")]
    fn discovery_type(&self) -> String {
        self.inner.discovery_type.as_str().to_string()
    }

    #[getter]
    #[pyo3(name = "sourceType")]
    fn source_type(&self) -> String {
        self.inner.source_type.as_str().to_string()
    }
}

// --- Filesystem discovery (parity with pxr.Sdr filesystem helpers) -----------

#[pyclass(name = "FilesystemDiscoveryPluginContext", module = "pxr.Sdr")]
pub struct PyFilesystemDiscoveryPluginContext;

#[pymethods]
impl PyFilesystemDiscoveryPluginContext {
    #[new]
    fn new() -> Self {
        Self
    }
}

#[pyclass(name = "_FilesystemDiscoveryPlugin", module = "pxr.Sdr")]
pub struct PyFilesystemDiscoveryPlugin {
    pub(crate) inner: SdrFilesystemDiscoveryPlugin,
}

#[pymethods]
impl PyFilesystemDiscoveryPlugin {
    #[new]
    fn new() -> Self {
        Self {
            inner: SdrFilesystemDiscoveryPlugin::new(),
        }
    }

    #[pyo3(name = "DiscoverShaderNodes")]
    fn discover_shader_nodes(
        &self,
        _context: &PyFilesystemDiscoveryPluginContext,
    ) -> Vec<PyNodeDiscoveryResult> {
        let ctx = SdrStandardFilesystemDiscoveryContext;
        self.inner
            .discover_shader_nodes(&ctx)
            .into_iter()
            .map(|inner| PyNodeDiscoveryResult { inner })
            .collect()
    }
}

#[pyclass(module = "pxr.Sdr")]
pub struct PyDiscoveryUri {
    inner: SdrDiscoveryUri,
}

#[pymethods]
impl PyDiscoveryUri {
    #[getter]
    fn uri(&self) -> String {
        self.inner.uri.clone()
    }

    #[getter]
    #[pyo3(name = "resolvedUri")]
    fn resolved_uri(&self) -> String {
        self.inner.resolved_uri.clone()
    }
}

#[pyfunction]
#[pyo3(name = "FsHelpersSplitShaderIdentifier")]
fn fs_helpers_split_shader_identifier(
    identifier: &str,
) -> Option<(String, String, PySdrVersion)> {
    let mut family = Token::default();
    let mut name = Token::default();
    let mut version = SdrVersion::default();
    let id = Token::new(identifier);
    if !split_shader_identifier(&id, &mut family, &mut name, &mut version) {
        return None;
    }
    Some((
        family.as_str().to_string(),
        name.as_str().to_string(),
        PySdrVersion { inner: version },
    ))
}

#[pyfunction]
#[pyo3(name = "FsHelpersDiscoverFiles")]
fn fs_helpers_discover_files_py(
    search_paths: Vec<String>,
    allowed_extensions: Vec<String>,
    follow_symlinks: bool,
) -> Vec<PyDiscoveryUri> {
    fs_helpers_discover_files(&search_paths, &allowed_extensions, follow_symlinks)
        .into_iter()
        .map(|inner| PyDiscoveryUri { inner })
        .collect()
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

    #[pyo3(name = "GetShownIf")]
    fn get_shown_if(&self) -> String {
        self.inner.get_shown_if()
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
    pub(crate) fn new_node(n: SdrShaderNode) -> Self {
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
        // C++ `SdrShaderNode::GetFunction` / `_function` — from discovery `function` (Rust: `family`).
        self.inner.get_family().as_str().to_string()
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

    #[pyo3(name = "GetIdentifier")]
    fn get_identifier(&self) -> String {
        self.inner.get_identifier().as_str().to_string()
    }

    #[pyo3(name = "GetDataForKey")]
    fn get_data_for_key(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        let t = Token::new(key);
        value_to_py(py, &self.inner.get_data_for_key(&t))
    }
}

fn drain_stored_py_err(slot: &Arc<Mutex<Option<PyErr>>>) -> PyResult<()> {
    if let Some(e) = slot.lock().expect("mutex poisoned").take() {
        Err(e)
    } else {
        Ok(())
    }
}

fn grouped_query_to_py(py: Python<'_>, g: &GroupedQueryResult) -> PyResult<Py<PyAny>> {
    match g {
        GroupedQueryResult::Nodes(nodes) => {
            let mut list = Vec::new();
            for n in nodes {
                list.push(Py::new(py, PyShaderNode::new_node((**n).clone()))?);
            }
            Ok(PyList::new(py, list)?.into_any().unbind())
        }
        GroupedQueryResult::Nested(map) => {
            let d = PyDict::new(py);
            for (k, v) in map {
                d.set_item(k, grouped_query_to_py(py, v)?)?;
            }
            Ok(d.into_any().unbind())
        }
    }
}

#[pyclass(name = "NodeFieldKey", module = "pxr.Sdr")]
pub struct PySdrNodeFieldKey;

#[pymethods]
impl PySdrNodeFieldKey {
    #[classattr]
    #[pyo3(name = "Identifier")]
    fn identifier() -> &'static str {
        tokens().node_field_key.identifier.as_str()
    }
    #[classattr]
    #[pyo3(name = "Name")]
    fn name() -> &'static str {
        tokens().node_field_key.name.as_str()
    }
    #[classattr]
    #[pyo3(name = "Family")]
    fn family() -> &'static str {
        tokens().node_field_key.family.as_str()
    }
    #[classattr]
    #[pyo3(name = "SourceType")]
    fn source_type() -> &'static str {
        tokens().node_field_key.source_type.as_str()
    }
}

#[pyclass(name = "ShaderNodeQuery", module = "pxr.Sdr", skip_from_py_object)]
pub struct PyShaderNodeQuery {
    inner: Arc<Mutex<SdrShaderNodeQuery>>,
    py_custom_filters: Arc<Mutex<Vec<Py<PyAny>>>>,
}

impl Clone for PyShaderNodeQuery {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            py_custom_filters: Arc::clone(&self.py_custom_filters),
        }
    }
}

#[pymethods]
impl PyShaderNodeQuery {
    #[new]
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SdrShaderNodeQuery::new())),
            py_custom_filters: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[pyo3(name = "SelectDistinct")]
    fn select_distinct(&self, keys: Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(s) = keys.extract::<String>() {
            let mut g = self.inner.lock().expect("mutex poisoned");
            *g = std::mem::take(&mut *g).select_distinct(&Token::new(s.as_str()));
        } else if let Ok(list) = keys.cast::<PyList>() {
            let mut g = self.inner.lock().expect("mutex poisoned");
            let mut q = std::mem::take(&mut *g);
            for item in list.iter() {
                let s: String = item.extract()?;
                q = q.select_distinct(&Token::new(s.as_str()));
            }
            *g = q;
        } else {
            return Err(PyTypeError::new_err(
                "SelectDistinct expects str or list of str",
            ));
        }
        Ok(self.clone())
    }

    #[pyo3(name = "NodeValueIs")]
    fn node_value_is(&self, key: &str, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let v = py_to_value(&value)?;
        let mut g = self.inner.lock().expect("mutex poisoned");
        *g = std::mem::take(&mut *g).node_value_is(&Token::new(key), v);
        Ok(self.clone())
    }

    #[pyo3(name = "NodeValueIsNot")]
    fn node_value_is_not(&self, key: &str, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let v = py_to_value(&value)?;
        let mut g = self.inner.lock().expect("mutex poisoned");
        *g = std::mem::take(&mut *g).node_value_is_not(&Token::new(key), v);
        Ok(self.clone())
    }

    #[pyo3(name = "NodeValueIsIn")]
    fn node_value_is_in(&self, key: &str, values: Bound<'_, PyAny>) -> PyResult<Self> {
        let list = values
            .cast::<PyList>()
            .map_err(|_| PyTypeError::new_err("NodeValueIsIn expects a list of values"))?;
        let mut out: Vec<Value> = Vec::new();
        for item in list.iter() {
            out.push(py_to_value(&item)?);
        }
        let mut g = self.inner.lock().expect("mutex poisoned");
        *g = std::mem::take(&mut *g).node_value_is_in(&Token::new(key), out);
        Ok(self.clone())
    }

    #[pyo3(name = "NodeValueIsNotIn")]
    fn node_value_is_not_in(&self, key: &str, values: Bound<'_, PyAny>) -> PyResult<Self> {
        let list = values
            .cast::<PyList>()
            .map_err(|_| PyTypeError::new_err("NodeValueIsNotIn expects a list of values"))?;
        let mut out: Vec<Value> = Vec::new();
        for item in list.iter() {
            out.push(py_to_value(&item)?);
        }
        let mut g = self.inner.lock().expect("mutex poisoned");
        *g = std::mem::take(&mut *g).node_value_is_not_in(&Token::new(key), out);
        Ok(self.clone())
    }

    #[pyo3(name = "NodeHasValueFor")]
    fn node_has_value_for(&self, key: &str) -> Self {
        let mut g = self.inner.lock().expect("mutex poisoned");
        *g = std::mem::take(&mut *g).node_has_value_for(&Token::new(key));
        self.clone()
    }

    #[pyo3(name = "NodeHasNoValueFor")]
    fn node_has_no_value_for(&self, key: &str) -> Self {
        let mut g = self.inner.lock().expect("mutex poisoned");
        *g = std::mem::take(&mut *g).node_has_no_value_for(&Token::new(key));
        self.clone()
    }

    #[pyo3(name = "CustomFilter")]
    fn custom_filter(&self, cb: Py<PyAny>) -> Self {
        self.py_custom_filters
            .lock()
            .expect("mutex poisoned")
            .push(cb);
        self.clone()
    }

    #[pyo3(name = "Run")]
    fn run(&self, py: Python<'_>) -> PyResult<Py<PyShaderNodeQueryResult>> {
        let registry = SdrRegistry::get_instance();
        let base = self
            .inner
            .lock()
            .expect("mutex poisoned")
            .clone_without_custom_filters();
        let filters: Vec<Py<PyAny>> = self
            .py_custom_filters
            .lock()
            .expect("mutex poisoned")
            .iter()
            .map(|p| p.clone_ref(py))
            .collect();
        let err_slot: Arc<Mutex<Option<PyErr>>> = Arc::new(Mutex::new(None));
        let err_slot_cb = err_slot.clone();
        let result = registry.run_query_with_match_fn(&base, |node| {
            if !base.matches_inclusion(node) || !base.matches_exclusion(node) {
                return false;
            }
            for f in &filters {
                let wrapped = match Py::new(py, PyShaderNode::new_node(node.clone())) {
                    Ok(w) => w,
                    Err(e) => {
                        *err_slot_cb.lock().expect("mutex poisoned") = Some(e);
                        return false;
                    }
                };
                match f.call1(py, (wrapped,)) {
                    Ok(v) => match v.bind(py).extract::<bool>() {
                        Ok(b) => {
                            if !b {
                                return false;
                            }
                        }
                        Err(_) => {
                            *err_slot_cb.lock().expect("mutex poisoned") = Some(
                                PyTypeError::new_err(
                                    "Sdr.ShaderNodeQuery.CustomFilter callback must return bool",
                                ),
                            );
                            return false;
                        }
                    },
                    Err(e) => {
                        *err_slot_cb.lock().expect("mutex poisoned") = Some(e);
                        return false;
                    }
                }
            }
            true
        });
        drain_stored_py_err(&err_slot)?;
        Py::new(py, PyShaderNodeQueryResult { inner: result })
    }
}

#[pyclass(name = "ShaderNodeQueryResult", module = "pxr.Sdr")]
pub struct PyShaderNodeQueryResult {
    inner: SdrShaderNodeQueryResult,
}

#[pymethods]
impl PyShaderNodeQueryResult {
    #[pyo3(name = "GetKeys")]
    fn get_keys(&self) -> Vec<String> {
        self.inner
            .get_keys()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetValues")]
    fn get_values(&self, py: Python<'_>) -> PyResult<Vec<Vec<Py<PyAny>>>> {
        let mut rows = Vec::new();
        for row in self.inner.get_values() {
            let mut out = Vec::new();
            for v in row {
                out.push(value_to_py(py, v)?);
            }
            rows.push(out);
        }
        Ok(rows)
    }

    #[pyo3(name = "GetStringifiedValues")]
    fn get_stringified_values(&self) -> Vec<Vec<String>> {
        self.inner.get_stringified_values()
    }

    #[pyo3(name = "GetAllShaderNodes")]
    fn get_all_shader_nodes(&self, py: Python<'_>) -> PyResult<Vec<Py<PyShaderNode>>> {
        let mut out = Vec::new();
        for n in self.inner.get_all_shader_nodes() {
            out.push(Py::new(py, PyShaderNode::new_node((*n).clone()))?);
        }
        Ok(out)
    }

    #[pyo3(name = "GetShaderNodesByValues")]
    fn get_shader_nodes_by_values(
        &self,
        py: Python<'_>,
    ) -> PyResult<Vec<Vec<Py<PyShaderNode>>>> {
        let mut rows = Vec::new();
        for group in self.inner.get_shader_nodes_by_values() {
            let mut row = Vec::new();
            for n in group {
                row.push(Py::new(py, PyShaderNode::new_node((**n).clone()))?);
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

#[pyclass(name = "ShaderNodeQueryUtils", module = "pxr.Sdr")]
pub struct PyShaderNodeQueryUtils;

#[pymethods]
impl PyShaderNodeQueryUtils {
    #[staticmethod]
    #[pyo3(name = "GroupQueryResults")]
    fn group_query_results(
        py: Python<'_>,
        result: PyRef<PyShaderNodeQueryResult>,
    ) -> PyResult<Py<PyAny>> {
        let grouped = group_query_results(&result.inner);
        grouped_query_to_py(py, &grouped)
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
                let name = pyty.borrow().effective_name_for_sdr_plugin_match();
                if name.contains("SdrOslParserPlugin") || name.contains("OslParser") {
                    out.push(Box::new(OslParserPlugin::new()) as SdrParserPluginRef);
                } else if name.contains("ShaderDefParser") || name.contains("UsdShadeShaderDef") {
                    out.push(
                        Box::new(UsdShadeShaderDefParserPlugin::new()) as SdrParserPluginRef,
                    );
                }
            }
        }
        if !out.is_empty() {
            SdrRegistry::get_instance().set_extra_parser_plugins(out);
        }
        Ok(())
    }

    #[pyo3(name = "SetExtraDiscoveryPlugins")]
    fn set_extra_discovery_plugins(&self, plugins: Bound<'_, PyAny>) -> PyResult<()> {
        let list = plugins.cast::<PyList>()?;
        let mut out: Vec<SdrDiscoveryPluginRef> = Vec::new();
        for item in list.iter() {
            if let Ok(plugin) = item.extract::<PyRef<PyFilesystemDiscoveryPlugin>>() {
                out.push(Box::new(plugin.inner.clone_config()) as SdrDiscoveryPluginRef);
            }
        }
        if !out.is_empty() {
            SdrRegistry::get_instance().set_extra_discovery_plugins(out);
        }
        Ok(())
    }

    #[pyo3(name = "RunQuery")]
    fn run_query(
        &self,
        py: Python<'_>,
        query: &PyShaderNodeQuery,
    ) -> PyResult<Py<PyShaderNodeQueryResult>> {
        query.run(py)
    }

    #[pyo3(name = "GetShaderNodeIdentifiers")]
    #[pyo3(signature = (family = None))]
    fn get_shader_node_identifiers(&self, family: Option<String>) -> Vec<String> {
        let fam = family.map(|s| Token::new(s.as_str()));
        let ids = SdrRegistry::get_instance().get_shader_node_identifiers(
            fam.as_ref(),
            SdrVersionFilter::default(),
        );
        ids.iter()
            .map(|id| id.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "GetShaderNodeByIdentifierAndType")]
    fn get_shader_node_by_identifier_and_type(
        &self,
        py: Python<'_>,
        identifier: &str,
        source_type: &str,
    ) -> PyResult<Option<Py<PyShaderNode>>> {
        let id = Token::new(identifier);
        let st = Token::new(source_type);
        let node =
            SdrRegistry::get_instance().get_shader_node_by_identifier_and_type(&id, &st);
        Ok(node
            .map(|n| Py::new(py, PyShaderNode::new_node(n.clone())))
            .transpose()?)
    }

    #[pyo3(name = "GetShaderNodeByIdentifier")]
    #[pyo3(signature = (id, type_priority = None))]
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
    m.add_class::<PyFilesystemDiscoveryPlugin>()?;
    m.add_class::<PyFilesystemDiscoveryPluginContext>()?;
    m.add_class::<PyDiscoveryUri>()?;
    m.add_function(wrap_pyfunction!(fs_helpers_split_shader_identifier, m)?)?;
    m.add_function(wrap_pyfunction!(fs_helpers_discover_files_py, m)?)?;
    m.getattr("_FilesystemDiscoveryPlugin")?
        .setattr("Context", m.getattr("FilesystemDiscoveryPluginContext")?)?;
    m.add_class::<PyShaderProperty>()?;
    m.add_class::<PySdfTypeIndicator>()?;
    m.add_class::<PyShaderNode>()?;
    m.add_class::<PySdrNodeFieldKey>()?;
    m.add_class::<PyShaderNodeQuery>()?;
    m.add_class::<PyShaderNodeQueryResult>()?;
    m.add_class::<PyShaderNodeQueryUtils>()?;
    m.add_class::<PySdrRegistry>()?;
    m.add_function(wrap_pyfunction!(validate_property, m)?)?;
    m.add_class::<PySdrPropertyTypes>()?;
    sdr_shader_parser_test_utils::register(py, m)?;
    Ok(())
}
