//! pxr.Sdf — Scene Description Foundation bindings.
//!
//! Drop-in replacement for `pxr.Sdf` from C++ OpenUSD.
//! Mirrors wrapPath.cpp and wrapLayer.cpp from the reference implementation.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use usd_sdf::{
    AttributeSpec, Layer, LayerOffset, ListOp, Path, PathExpression, PathListOp, Payload,
    PayloadListOp, PrimSpec, PropertySpec, Reference, ReferenceListOp, RelationshipSpec, Specifier,
    SpecType,
};
use usd_sdf::path as sdf_path_fns;
use usd_sdf::path_expression_eval::PathExpressionEval;
use usd_sdf::variable_expression::VariableExpression;
use usd_tf::Token;
use usd_vt::{AssetPath, TimeCode};

// Helper: extract a Path from Python (accepts str or Path)
fn extract_path(obj: &Bound<'_, PyAny>) -> PyResult<Path> {
    if let Ok(p) = obj.extract::<PyPath>() {
        return Ok(p.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        if s.is_empty() {
            return Ok(Path::empty());
        }
        return Path::from_string(&s)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{s}'")));
    }
    Err(PyTypeError::new_err("expected str or Sdf.Path"))
}

// ============================================================================
// SdfPath
// ============================================================================

/// Path addressing a location in a USD scene graph.
///
/// Wraps `usd_sdf::Path`. Mirrors C++ `SdfPath` as exposed by `wrapPath.cpp`.
#[pyclass(from_py_object,name = "Path", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPath {
    pub(crate) inner: Path,
}

impl PyPath {
    pub fn path(&self) -> &Path {
        &self.inner
    }

    pub fn from_path(p: Path) -> Self {
        Self { inner: p }
    }
}

#[pymethods]
impl PyPath {
    // --- Constructors --------------------------------------------------------

    /// Construct a path from a string.
    ///
    /// C++ behaviour: invalid strings produce the empty path (no exception).
    #[new]
    #[pyo3(signature = (path = ""))]
    fn new(path: &str) -> Self {
        if path.is_empty() {
            return Self { inner: Path::empty() };
        }
        Self {
            inner: Path::from_string(path).unwrap_or_else(Path::empty),
        }
    }

    // --- Class-level constants (read-only attrs) ----------------------------

    #[classattr]
    #[allow(non_snake_case)]
    fn absoluteRootPath() -> Self {
        Self { inner: Path::absolute_root() }
    }

    #[classattr]
    #[allow(non_snake_case)]
    fn reflexiveRelativePath() -> Self {
        Self { inner: Path::reflexive_relative() }
    }

    #[classattr]
    #[allow(non_snake_case)]
    fn emptyPath() -> Self {
        Self { inner: Path::empty() }
    }

    // Path token constants mirroring SdfPathTokens
    #[classattr]
    #[allow(non_snake_case)]
    fn absoluteIndicator() -> &'static str { "/" }
    #[classattr]
    #[allow(non_snake_case)]
    fn childDelimiter() -> &'static str { "/" }
    #[classattr]
    #[allow(non_snake_case)]
    fn propertyDelimiter() -> &'static str { "." }
    #[classattr]
    #[allow(non_snake_case)]
    fn relationshipTargetStart() -> &'static str { "[" }
    #[classattr]
    #[allow(non_snake_case)]
    fn relationshipTargetEnd() -> &'static str { "]" }
    #[classattr]
    #[allow(non_snake_case)]
    fn parentPathElement() -> &'static str { ".." }
    #[classattr]
    #[allow(non_snake_case)]
    fn mapperIndicator() -> &'static str { ".mapper" }
    #[classattr]
    #[allow(non_snake_case)]
    fn expressionIndicator() -> &'static str { ".expression" }
    #[classattr]
    #[allow(non_snake_case)]
    fn mapperArgDelimiter() -> &'static str { "." }
    #[classattr]
    #[allow(non_snake_case)]
    fn namespaceDelimiter() -> &'static str { ":" }

    // --- Properties ----------------------------------------------------------

    #[getter]
    #[allow(non_snake_case)]
    fn pathString(&self) -> String {
        self.inner.get_as_string()
    }

    #[getter]
    fn name(&self) -> String {
        // C++ SdfPath returns "/" for absoluteRoot, "." for reflexiveRelative
        if self.inner.is_absolute_root_path() {
            return "/".to_string();
        }
        self.inner.get_name().to_string()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn elementString(&self) -> String {
        self.inner.get_element_string()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn isEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn pathElementCount(&self) -> usize {
        self.inner.get_path_element_count()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn targetPath(&self) -> Self {
        self.inner.get_target_path()
            .map(|p| Self { inner: p })
            .unwrap_or_else(|| Self { inner: Path::empty() })
    }

    // --- Path type predicates ------------------------------------------------

    #[allow(non_snake_case)]
    fn IsAbsolutePath(&self) -> bool {
        self.inner.is_absolute_path()
    }

    #[allow(non_snake_case)]
    fn IsAbsoluteRootPath(&self) -> bool {
        self.inner.is_absolute_root_path()
    }

    #[allow(non_snake_case)]
    fn IsPrimPath(&self) -> bool {
        self.inner.is_prim_path()
    }

    #[allow(non_snake_case)]
    fn IsAbsoluteRootOrPrimPath(&self) -> bool {
        self.inner.is_absolute_root_or_prim_path()
    }

    #[allow(non_snake_case)]
    fn IsRootPrimPath(&self) -> bool {
        self.inner.is_root_prim_path()
    }

    #[allow(non_snake_case)]
    fn IsPropertyPath(&self) -> bool {
        self.inner.is_property_path()
    }

    #[allow(non_snake_case)]
    fn IsPrimPropertyPath(&self) -> bool {
        self.inner.is_prim_property_path()
    }

    #[allow(non_snake_case)]
    fn IsNamespacedPropertyPath(&self) -> bool {
        self.inner.is_namespaced_property_path()
    }

    #[allow(non_snake_case)]
    fn IsPrimVariantSelectionPath(&self) -> bool {
        self.inner.is_prim_variant_selection_path()
    }

    #[allow(non_snake_case)]
    fn ContainsPrimVariantSelection(&self) -> bool {
        self.inner.contains_prim_variant_selection()
    }

    #[allow(non_snake_case)]
    fn ContainsPropertyElements(&self) -> bool {
        self.inner.contains_property_elements()
    }

    #[allow(non_snake_case)]
    fn IsRelationalAttributePath(&self) -> bool {
        self.inner.is_relational_attribute_path()
    }

    #[allow(non_snake_case)]
    fn IsTargetPath(&self) -> bool {
        self.inner.is_target_path()
    }

    #[allow(non_snake_case)]
    fn ContainsTargetPath(&self) -> bool {
        self.inner.contains_target_path()
    }

    #[allow(non_snake_case)]
    fn IsMapperPath(&self) -> bool {
        self.inner.is_mapper_path()
    }

    #[allow(non_snake_case)]
    fn IsMapperArgPath(&self) -> bool {
        self.inner.is_mapper_arg_path()
    }

    #[allow(non_snake_case)]
    fn IsExpressionPath(&self) -> bool {
        self.inner.is_expression_path()
    }

    // --- Prefix & parent -----------------------------------------------------

    #[allow(non_snake_case)]
    fn HasPrefix(&self, prefix: &PyPath) -> bool {
        self.inner.has_prefix(&prefix.inner)
    }

    #[allow(non_snake_case)]
    fn GetParentPath(&self) -> Self {
        Self { inner: self.inner.get_parent_path() }
    }

    #[allow(non_snake_case)]
    fn GetPrimPath(&self) -> Self {
        Self { inner: self.inner.get_prim_path() }
    }

    #[allow(non_snake_case)]
    fn GetPrimOrPrimVariantSelectionPath(&self) -> Self {
        Self { inner: self.inner.get_prim_or_prim_variant_selection_path() }
    }

    #[allow(non_snake_case)]
    fn GetAbsoluteRootOrPrimPath(&self) -> Self {
        Self { inner: self.inner.get_absolute_root_or_prim_path() }
    }

    /// Returns all prefixes of this path from root to self (inclusive).
    #[allow(non_snake_case)]
    #[pyo3(signature = (num_prefixes = 0))]
    fn GetPrefixes(&self, num_prefixes: usize) -> Vec<Self> {
        let prefixes = if num_prefixes == 0 {
            self.inner.get_prefixes()
        } else {
            self.inner.get_prefixes_count(num_prefixes)
        };
        prefixes.into_iter().map(|p| Self { inner: p }).collect()
    }

    /// Returns (variantSet, variant) tuple if this is a variant selection path.
    #[allow(non_snake_case)]
    fn GetVariantSelection(&self, py: Python<'_>) -> Py<PyAny> {
        match self.inner.get_variant_selection() {
            Some((set, variant)) => PyTuple::new(py, [set, variant])
                .expect("tuple creation")
                .into_any()
                .unbind(),
            None => PyTuple::new(py, Vec::<String>::new())
                .expect("tuple creation")
                .into_any()
                .unbind(),
        }
    }

    /// Returns all target paths reachable from this path, recursively.
    #[allow(non_snake_case)]
    fn GetAllTargetPathsRecursively(&self) -> Vec<Self> {
        self.inner.get_all_target_paths_recursively()
            .into_iter()
            .map(|p| Self { inner: p })
            .collect()
    }

    // --- Absolute / relative conversion --------------------------------------

    #[allow(non_snake_case)]
    fn MakeAbsolutePath(&self, anchor: &PyPath) -> PyResult<Self> {
        self.inner.make_absolute(&anchor.inner)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("MakeAbsolutePath failed"))
    }

    #[allow(non_snake_case)]
    fn MakeRelativePath(&self, anchor: &PyPath) -> PyResult<Self> {
        self.inner.make_relative(&anchor.inner)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("MakeRelativePath failed"))
    }

    // --- Appending ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn AppendPath(&self, suffix: &PyPath) -> PyResult<Self> {
        self.inner.append_path(&suffix.inner)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendPath failed"))
    }

    #[allow(non_snake_case)]
    fn AppendChild(&self, child_name: &str) -> PyResult<Self> {
        self.inner.append_child(child_name)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err(format!("AppendChild('{child_name}') failed")))
    }

    #[allow(non_snake_case)]
    fn AppendProperty(&self, prop_name: &str) -> PyResult<Self> {
        self.inner.append_property(prop_name)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err(format!("AppendProperty('{prop_name}') failed")))
    }

    #[allow(non_snake_case)]
    fn AppendVariantSelection(&self, variant_set: &str, variant: &str) -> PyResult<Self> {
        self.inner.append_variant_selection(variant_set, variant)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendVariantSelection failed"))
    }

    #[allow(non_snake_case)]
    fn AppendTarget(&self, target: &PyPath) -> PyResult<Self> {
        self.inner.append_target(&target.inner)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendTarget failed"))
    }

    #[allow(non_snake_case)]
    fn AppendRelationalAttribute(&self, attr_name: &str) -> PyResult<Self> {
        self.inner.append_relational_attribute(attr_name)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendRelationalAttribute failed"))
    }

    #[allow(non_snake_case)]
    fn AppendMapper(&self, target: &PyPath) -> PyResult<Self> {
        self.inner.append_mapper(&target.inner)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendMapper failed"))
    }

    #[allow(non_snake_case)]
    fn AppendMapperArg(&self, arg_name: &str) -> PyResult<Self> {
        self.inner.append_mapper_arg(arg_name)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendMapperArg failed"))
    }

    #[allow(non_snake_case)]
    fn AppendExpression(&self) -> PyResult<Self> {
        self.inner.append_expression()
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err("AppendExpression failed"))
    }

    #[allow(non_snake_case)]
    fn AppendElementString(&self, element: &str) -> PyResult<Self> {
        self.inner.append_element_string(element)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err(format!("AppendElementString('{element}') failed")))
    }

    // --- Manipulation --------------------------------------------------------

    #[allow(non_snake_case)]
    #[pyo3(signature = (old_prefix, new_prefix, fix_target_paths = true))]
    fn ReplacePrefix(
        &self,
        old_prefix: &PyPath,
        new_prefix: &PyPath,
        fix_target_paths: bool,
    ) -> Self {
        let result = if fix_target_paths {
            self.inner.replace_prefix(&old_prefix.inner, &new_prefix.inner)
        } else {
            self.inner.replace_prefix_with_fix(&old_prefix.inner, &new_prefix.inner, false)
        };
        Self { inner: result.unwrap_or_else(|| self.inner.clone()) }
    }

    #[allow(non_snake_case)]
    fn GetCommonPrefix(&self, other: &PyPath) -> Self {
        Self { inner: self.inner.get_common_prefix(&other.inner) }
    }

    /// Returns (pathA, pathB) with common suffix removed.
    #[allow(non_snake_case)]
    #[pyo3(signature = (other, stop_at_root_prim = false))]
    fn RemoveCommonSuffix(&self, other: &PyPath, stop_at_root_prim: bool, py: Python<'_>) -> Py<PyAny> {
        let (a, b) = self.inner.remove_common_suffix(&other.inner, stop_at_root_prim);
        PyTuple::new(py, [
            PyPath { inner: a }.into_pyobject(py).expect("ok"),
            PyPath { inner: b }.into_pyobject(py).expect("ok"),
        ])
        .expect("tuple creation")
        .into_any()
        .unbind()
    }

    #[allow(non_snake_case)]
    fn ReplaceName(&self, new_name: &str) -> Self {
        Self { inner: self.inner.replace_name(new_name).unwrap_or_else(|| self.inner.clone()) }
    }

    #[allow(non_snake_case)]
    fn ReplaceTargetPath(&self, new_target: &PyPath) -> Self {
        Self {
            inner: self.inner.replace_target_path(&new_target.inner)
                .unwrap_or_else(|| self.inner.clone()),
        }
    }

    #[allow(non_snake_case)]
    fn StripAllVariantSelections(&self) -> Self {
        Self { inner: self.inner.strip_all_variant_selections() }
    }

    // --- Static methods ------------------------------------------------------

    /// Returns true if `path_string` is a valid SDF path string.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsValidPathString(path_string: &str) -> bool {
        Path::is_valid_path_string(path_string)
    }

    /// Returns true if `name` is a valid identifier for a prim or property.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsValidIdentifier(name: &str) -> bool {
        Path::is_valid_identifier(name)
    }

    /// Returns true if `name` is a valid namespaced identifier.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsValidNamespacedIdentifier(name: &str) -> bool {
        Path::is_valid_namespaced_identifier(name)
    }

    /// Splits `identifier` into namespace components on ':'.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn TokenizeIdentifier(identifier: &str) -> Vec<String> {
        Path::tokenize_identifier(identifier)
    }

    /// Joins namespace components with ':'.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn JoinIdentifier(tokens: Vec<String>) -> String {
        // Convert Vec<String> to Vec<&str> for the API
        let refs: Vec<&str> = tokens.iter().map(String::as_str).collect();
        Path::join_identifier(&refs)
    }

    /// Strips the last namespace component, returning the prefix.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn StripNamespace(name: &str) -> String {
        Path::strip_namespace(name).to_string()
    }

    /// Returns (strippedName, prefix) splitting off the given prefix namespace.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn StripPrefixNamespace(name: &str, prefix: &str, py: Python<'_>) -> Py<PyAny> {
        let (stripped, had_prefix) = Path::strip_prefix_namespace(name, prefix);
        let s_obj: Py<PyAny> = stripped.into_pyobject(py).expect("ok").into_any().unbind();
        let b_obj: Py<PyAny> = had_prefix.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
        PyTuple::new(py, [s_obj, b_obj])
            .expect("tuple")
            .into_any()
            .unbind()
    }

    /// Removes all paths from `paths` that are descendants of other paths in the list.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn RemoveDescendentPaths(paths: Vec<PyPath>) -> Vec<Self> {
        let mut rust_paths: Vec<Path> = paths.into_iter().map(|p| p.inner).collect();
        sdf_path_fns::remove_descendent_paths(&mut rust_paths);
        rust_paths.into_iter().map(|p| Self { inner: p }).collect()
    }

    /// Removes all paths from `paths` that are ancestors of other paths in the list.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn RemoveAncestorPaths(paths: Vec<PyPath>) -> Vec<Self> {
        let mut rust_paths: Vec<Path> = paths.into_iter().map(|p| p.inner).collect();
        sdf_path_fns::remove_ancestor_paths(&mut rust_paths);
        rust_paths.into_iter().map(|p| Self { inner: p }).collect()
    }

    /// Returns the concise relative forms of `paths` with respect to each other.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn GetConciseRelativePaths(paths: Vec<PyPath>) -> Vec<Self> {
        let rust_paths: Vec<Path> = paths.into_iter().map(|p| p.inner).collect();
        sdf_path_fns::get_concise_relative_paths(&rust_paths)
            .into_iter()
            .map(|p| Self { inner: p })
            .collect()
    }

    // --- Python special methods ----------------------------------------------

    fn __str__(&self) -> String {
        self.inner.get_as_string()
    }

    fn __repr__(&self) -> String {
        if self.inner.is_empty() {
            "Sdf.Path.emptyPath".to_string()
        } else {
            format!("Sdf.Path('{}')", self.inner.get_as_string())
        }
    }

    fn __hash__(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        self.inner.as_str().hash(&mut h);
        h.finish()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(p) = other.extract::<PyPath>() {
            self.inner == p.inner
        } else if let Ok(s) = other.extract::<String>() {
            self.inner.get_as_string() == s
        } else {
            false
        }
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __lt__(&self, other: &PyPath) -> bool {
        self.inner < other.inner
    }

    fn __le__(&self, other: &PyPath) -> bool {
        self.inner <= other.inner
    }

    fn __gt__(&self, other: &PyPath) -> bool {
        self.inner > other.inner
    }

    fn __ge__(&self, other: &PyPath) -> bool {
        self.inner >= other.inner
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }
}

// ============================================================================
// SdfLayerOffset
// ============================================================================

/// Time offset and scale applied when referencing layers.
///
/// Wraps `usd_sdf::LayerOffset`. Mirrors C++ `SdfLayerOffset`.
#[pyclass(from_py_object, name = "LayerOffset", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyLayerOffset {
    inner: LayerOffset,
}

impl PyLayerOffset {
    /// Convert to the inner Rust LayerOffset.
    pub fn to_layer_offset(&self) -> LayerOffset {
        self.inner.clone()
    }
}

#[pymethods]
impl PyLayerOffset {
    #[new]
    #[pyo3(signature = (offset = 0.0, scale = 1.0))]
    fn new(offset: f64, scale: f64) -> Self {
        Self { inner: LayerOffset::new(offset, scale) }
    }

    #[getter]
    fn offset(&self) -> f64 {
        self.inner.offset()
    }

    #[setter]
    fn set_offset(&mut self, v: f64) {
        self.inner.set_offset(v);
    }

    #[getter]
    fn scale(&self) -> f64 {
        self.inner.scale()
    }

    #[setter]
    fn set_scale(&mut self, v: f64) {
        self.inner.set_scale(v);
    }

    #[allow(non_snake_case)]
    fn IsIdentity(&self) -> bool {
        self.inner.is_identity()
    }

    #[allow(non_snake_case)]
    fn GetInverse(&self) -> Self {
        Self { inner: self.inner.inverse() }
    }

    fn __repr__(&self) -> String {
        format!("Sdf.LayerOffset({}, {})", self.inner.offset(), self.inner.scale())
    }

    fn __eq__(&self, other: &PyLayerOffset) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &PyLayerOffset) -> bool {
        self.inner != other.inner
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        if let Ok(lo) = other.extract::<PyLayerOffset>() {
            Ok(Self { inner: self.inner * lo.inner }
                .into_pyobject(py).expect("ok").into_any().unbind())
        } else if let Ok(tc) = other.extract::<PyTimeCode>() {
            // LayerOffset * TimeCode = TimeCode (apply offset+scale)
            let result = self.inner.offset() + self.inner.scale() * tc.inner.value();
            Ok(PyTimeCode { inner: TimeCode::new(result) }
                .into_pyobject(py).expect("ok").into_any().unbind())
        } else {
            Err(PyTypeError::new_err("expected LayerOffset or TimeCode"))
        }
    }
}

// ============================================================================
// SdfLayer
// ============================================================================

/// Container for scene description (prims, properties, metadata).
///
/// Wraps `Arc<usd_sdf::Layer>`. Mirrors C++ `SdfLayer` as exposed by `wrapLayer.cpp`.
#[pyclass(from_py_object,name = "Layer", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyLayer {
    inner: Arc<Layer>,
}

impl PyLayer {
    pub fn layer(&self) -> &Arc<Layer> {
        &self.inner
    }

    /// Public constructor for cross-module use (e.g. from usd.rs).
    pub fn from_layer_arc(layer: Arc<Layer>) -> Self {
        Self { inner: layer }
    }

    fn from_arc(layer: Arc<Layer>) -> Self {
        Self { inner: layer }
    }
}

#[pymethods]
impl PyLayer {
    // --- Factory methods (static) -------------------------------------------

    /// Creates a new empty layer at `identifier`.
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (identifier, args = None))]
    fn CreateNew(identifier: &str, args: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let _ = args; // file format args not exposed yet
        Layer::create_new(identifier)
            .map(Self::from_arc)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Creates a new anonymous layer with optional tag.
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (tag = "", args = None))]
    fn CreateAnonymous(tag: &str, args: Option<&Bound<'_, PyDict>>) -> Self {
        let _ = args;
        let tag_opt = if tag.is_empty() { None } else { Some(tag) };
        Self::from_arc(Layer::create_anonymous(tag_opt))
    }

    /// Opens a layer or returns the existing one for `identifier`.
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (identifier, args = None))]
    fn FindOrOpen(identifier: &str, args: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let _ = args;
        Layer::find_or_open(identifier)
            .map(Self::from_arc)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Returns the registered layer for `identifier`, or None.
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (identifier, args = None))]
    fn Find(identifier: &str, args: Option<&Bound<'_, PyDict>>) -> Option<Self> {
        let _ = args;
        Layer::find(identifier).map(Self::from_arc)
    }

    /// Opens layer from disk as anonymous (no registry entry).
    #[staticmethod]
    #[allow(non_snake_case)]
    fn OpenAsAnonymous(path: &str) -> PyResult<Self> {
        Layer::open_as_anonymous(std::path::Path::new(path))
            .map(Self::from_arc)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Finds layer relative to anchor layer.
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (anchor, identifier, args = None))]
    fn FindRelativeToLayer(
        anchor: &PyLayer,
        identifier: &str,
        args: Option<&Bound<'_, PyDict>>,
    ) -> Option<Self> {
        let _ = args;
        Layer::find_relative_to_layer(&anchor.inner, identifier).map(Self::from_arc)
    }

    /// Finds or opens layer relative to anchor layer.
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (anchor, identifier, args = None))]
    fn FindOrOpenRelativeToLayer(
        anchor: &PyLayer,
        identifier: &str,
        args: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = args;
        Layer::find_or_open_relative_to_layer(&anchor.inner, identifier)
            .map(Self::from_arc)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    // --- Properties ---------------------------------------------------------

    #[getter]
    fn identifier(&self) -> &str {
        self.inner.identifier()
    }

    #[setter]
    fn set_identifier(&self, new_id: &str) -> PyResult<()> {
        if new_id.is_empty() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Cannot set empty identifier",
            ));
        }
        if Layer::is_anonymous_layer_identifier(new_id) && !self.inner.is_anonymous() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Cannot set anonymous identifier on non-anonymous layer",
            ));
        }
        // Check if another layer with this identifier already exists
        if let Some(existing) = Layer::find(new_id) {
            if !Arc::ptr_eq(&self.inner, &existing) {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    format!("Layer already exists with identifier '{new_id}'"),
                ));
            }
        }
        // Layer.set_identifier requires &mut which we can't get through Arc.
        // For now, this is a no-op — identifier is immutable once created.
        let _ = new_id;
        Ok(())
    }

    #[getter]
    #[allow(non_snake_case)]
    fn resolvedPath(&self) -> String {
        self.inner.get_resolved_path().unwrap_or_default()
    }

    #[getter]
    fn anonymous(&self) -> bool {
        self.inner.is_anonymous()
    }

    #[getter]
    fn dirty(&self) -> bool {
        self.inner.is_dirty()
    }

    #[getter]
    fn empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn pseudoRoot(&self) -> PyPrimSpec {
        PyPrimSpec { inner: self.inner.get_pseudo_root() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn rootPrims(&self) -> PyPrimSpecList {
        PyPrimSpecList {
            items: self.inner.get_root_prims(),
        }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn subLayerPaths(&self) -> Vec<String> {
        self.inner.get_sublayer_paths()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_subLayerPaths(&self, paths: Vec<String>) {
        self.inner.set_sublayer_paths(&paths);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn subLayerOffsets(&self) -> Vec<PyLayerOffset> {
        self.inner.get_sublayer_offsets()
            .into_iter()
            .map(|o| PyLayerOffset { inner: o })
            .collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn defaultPrim(&self) -> String {
        self.inner.get_default_prim().as_str().to_string()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_defaultPrim(&self, name: &str) {
        self.inner.set_default_prim(&Token::new(name));
    }

    #[allow(non_snake_case)]
    fn HasDefaultPrim(&self) -> bool {
        self.inner.has_default_prim()
    }

    #[allow(non_snake_case)]
    fn ClearDefaultPrim(&self) {
        self.inner.clear_default_prim();
    }

    #[getter]
    #[allow(non_snake_case)]
    fn startTimeCode(&self) -> f64 {
        self.inner.get_start_time_code()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_startTimeCode(&self, t: f64) {
        self.inner.set_start_time_code(t);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn endTimeCode(&self) -> f64 {
        self.inner.get_end_time_code()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_endTimeCode(&self, t: f64) {
        self.inner.set_end_time_code(t);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn timeCodesPerSecond(&self) -> f64 {
        self.inner.get_time_codes_per_second()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_timeCodesPerSecond(&self, fps: f64) {
        self.inner.set_time_codes_per_second(fps);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn framesPerSecond(&self) -> f64 {
        self.inner.get_frames_per_second()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_framesPerSecond(&self, fps: f64) {
        self.inner.set_frames_per_second(fps);
    }

    #[getter]
    fn comment(&self) -> String {
        self.inner.get_comment()
    }

    #[setter]
    fn set_comment(&self, c: &str) {
        self.inner.set_comment(c);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn customLayerData(&self, py: Python<'_>) -> Py<PyAny> {
        let data = self.inner.get_custom_layer_data();
        let dict = PyDict::new(py);
        for (k, v) in &data {
            // Store values as strings for simplicity; full VtValue bridge would need vt module
            let _ = dict.set_item(k.as_str(), format!("{v:?}"));
        }
        dict.into_any().unbind()
    }

    // --- Spec access --------------------------------------------------------

    /// Returns the PrimSpec at `path`, or None.
    #[allow(non_snake_case)]
    fn GetPrimAtPath(&self, path: &Bound<'_, PyAny>) -> PyResult<Option<PyPrimSpec>> {
        let p = extract_path(path)?;
        // Handle pseudo-root path "/" explicitly
        if p.is_absolute_root_path() {
            return Ok(Some(PyPrimSpec { inner: self.inner.get_pseudo_root() }));
        }
        Ok(self.inner.get_prim_at_path(&p)
            .map(|p| PyPrimSpec { inner: p }))
    }

    /// Returns the property spec at `path`, or None.
    #[allow(non_snake_case)]
    fn GetPropertyAtPath(&self, path: &Bound<'_, PyAny>) -> PyResult<Option<PyAttributeSpec>> {
        let p = extract_path(path)?;
        Ok(self.inner.get_attribute_at_path(&p)
            .map(|a| PyAttributeSpec { inner: a }))
    }

    /// Returns the attribute spec at `path`, or None.
    #[allow(non_snake_case)]
    fn GetAttributeAtPath(&self, path: &Bound<'_, PyAny>) -> PyResult<Option<PyAttributeSpec>> {
        let p = extract_path(path)?;
        Ok(self.inner.get_attribute_at_path(&p)
            .map(|a| PyAttributeSpec { inner: a }))
    }

    /// Returns an object (prim, property, etc.) at `path`, or None.
    #[allow(non_snake_case)]
    fn GetObjectAtPath(&self, path_obj: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let path = extract_path(path_obj)?;
        // If it's a prim path, return PrimSpec
        if path.is_prim_path() || path.is_absolute_root_path() {
            if let Some(p) = self.inner.get_prim_at_path(&path) {
                return Ok(PyPrimSpec { inner: p }
                    .into_pyobject(py)
                    .expect("ok")
                    .into_any()
                    .unbind());
            }
        }
        // If it's a property path, return AttributeSpec
        if path.is_property_path() {
            if let Some(a) = self.inner.get_attribute_at_path(&path) {
                return Ok(PyAttributeSpec { inner: a }
                    .into_pyobject(py)
                    .expect("ok")
                    .into_any()
                    .unbind());
            }
        }
        Ok(py.None())
    }

    /// Returns true if a spec exists at `path`.
    #[allow(non_snake_case)]
    fn HasSpec(&self, path: &Bound<'_, PyAny>) -> PyResult<bool> {
        let p = extract_path(path)?;
        Ok(self.inner.has_spec(&p))
    }

    // --- Sublayer management ------------------------------------------------

    #[allow(non_snake_case)]
    fn InsertSubLayerPath(&self, path: &str, index: isize) {
        self.inner.insert_sublayer_path(path, index);
    }

    #[allow(non_snake_case)]
    fn RemoveSubLayerPath(&self, index: usize) {
        self.inner.remove_sublayer_path(index);
    }

    #[allow(non_snake_case)]
    fn GetNumSubLayerPaths(&self) -> usize {
        self.inner.get_num_sublayer_paths()
    }

    #[allow(non_snake_case)]
    fn GetSubLayerOffset(&self, index: usize) -> Option<PyLayerOffset> {
        self.inner.get_sublayer_offset(index)
            .map(|o| PyLayerOffset { inner: o })
    }

    #[allow(non_snake_case)]
    fn SetSubLayerOffset(&self, offset: &PyLayerOffset, index: usize) {
        self.inner.set_sublayer_offset(&offset.inner, index);
    }

    // --- Time samples -------------------------------------------------------

    /// Returns sorted list of time sample times for the attribute at `path`.
    #[allow(non_snake_case)]
    fn ListTimeSamplesForPath(&self, path: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
        let p = extract_path(path)?;
        Ok(self.inner.list_time_samples_for_path(&p))
    }

    #[allow(non_snake_case)]
    fn GetNumTimeSamplesForPath(&self, path: &Bound<'_, PyAny>) -> PyResult<usize> {
        let p = extract_path(path)?;
        Ok(self.inner.get_num_time_samples_for_path(&p))
    }

    /// Returns the value at `time` for the attribute at `path`, or None.
    #[allow(non_snake_case)]
    fn QueryTimeSample(&self, path: &Bound<'_, PyAny>, time: f64, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let p = extract_path(path)?;
        Ok(match self.inner.query_time_sample(&p, time) {
            Some(val) => vt_value_to_pyobject(py, &val),
            None => py.None(),
        })
    }

    /// Sets the time sample at `time` for the attribute at `path`.
    #[allow(non_snake_case)]
    fn SetTimeSample(&self, path: &Bound<'_, PyAny>, time: f64, value: Py<PyAny>, py: Python<'_>) -> PyResult<bool> {
        let p = extract_path(path)?;
        let val = pyobject_to_vt_value(py, &value);
        Ok(self.inner.set_time_sample(&p, time, val))
    }

    /// Removes the time sample at `time` for the attribute at `path`.
    #[allow(non_snake_case)]
    fn EraseTimeSample(&self, path: &Bound<'_, PyAny>, time: f64) -> PyResult<bool> {
        let p = extract_path(path)?;
        Ok(self.inner.erase_time_sample(&p, time))
    }

    /// Returns (found, lower, upper) bracketing the time samples around `time` for `path`.
    #[allow(non_snake_case)]
    fn GetBracketingTimeSamplesForPath(
        &self,
        path: &Bound<'_, PyAny>,
        time: f64,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let p = extract_path(path)?;
        Ok(match self.inner.get_bracketing_time_samples_for_path(&p, time) {
            Some((lo, hi)) => {
                let found: Py<PyAny> = true.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
                let lo_obj: Py<PyAny> = lo.into_pyobject(py).expect("ok").into_any().unbind();
                let hi_obj: Py<PyAny> = hi.into_pyobject(py).expect("ok").into_any().unbind();
                PyTuple::new(py, [found, lo_obj, hi_obj])
                    .expect("tuple")
                    .into_any()
                    .unbind()
            }
            None => {
                let found: Py<PyAny> = false.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
                let lo_obj: Py<PyAny> = 0.0_f64.into_pyobject(py).expect("ok").into_any().unbind();
                let hi_obj: Py<PyAny> = 0.0_f64.into_pyobject(py).expect("ok").into_any().unbind();
                PyTuple::new(py, [found, lo_obj, hi_obj])
                    .expect("tuple")
                    .into_any()
                    .unbind()
            }
        })
    }

    // --- Muting -------------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsMuted(&self) -> bool {
        self.inner.is_muted()
    }

    #[allow(non_snake_case)]
    fn SetMuted(&self, muted: bool) {
        self.inner.set_muted(muted);
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn GetMutedLayers() -> Vec<String> {
        Layer::get_muted_layers().into_iter().collect()
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn AddToMutedLayers(path: &str) {
        Layer::add_to_muted_layers(path);
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn RemoveFromMutedLayers(path: &str) {
        Layer::remove_from_muted_layers(path);
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsMutedPath(path: &str) -> bool {
        Layer::is_muted_path(path)
    }

    // --- Reload / Save ------------------------------------------------------

    /// Reloads this layer from disk.
    #[allow(non_snake_case)]
    fn Reload(&self) -> PyResult<bool> {
        self.inner.reload()
            .map(|_| true)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Reloads the given layers. Returns true if all succeeded.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn ReloadLayers(layers: Vec<PyLayer>, force: bool) -> bool {
        let arcs: Vec<Arc<Layer>> = layers.into_iter().map(|l| l.inner).collect();
        Layer::reload_layers(&arcs, force)
    }

    /// Saves this layer to disk.
    #[allow(non_snake_case)]
    fn Save(&self) -> PyResult<bool> {
        self.inner.save()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Exports this layer to `filename`.
    #[allow(non_snake_case)]
    #[pyo3(signature = (filename, comment = "", args = None))]
    fn Export(&self, filename: &str, comment: &str, args: Option<&Bound<'_, PyDict>>) -> PyResult<bool> {
        let _ = (comment, args);
        self.inner.export(std::path::Path::new(filename))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Returns the USDA text of this layer.
    #[allow(non_snake_case)]
    fn ExportToString(&self) -> PyResult<String> {
        self.inner.export_to_string()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Replaces this layer's content with the given USDA `data` string.
    #[allow(non_snake_case)]
    fn ImportFromString(&self, data: &str) -> bool {
        self.inner.import_from_string(data)
    }

    /// Clears all scene description from this layer.
    #[allow(non_snake_case)]
    fn Clear(&self) {
        self.inner.clear();
    }

    // --- Loaded layers ------------------------------------------------------

    #[staticmethod]
    #[allow(non_snake_case)]
    fn GetLoadedLayers() -> Vec<Self> {
        Layer::get_loaded_layers()
            .into_iter()
            .map(Self::from_arc)
            .collect()
    }

    // --- Additional metadata properties ------------------------------------

    #[getter]
    fn documentation(&self) -> String {
        // Documentation stored as "documentation" field on pseudo-root
        self.inner.get_pseudo_root()
            .documentation()
    }

    #[setter]
    fn set_documentation(&self, doc: &str) {
        // Mirror C++ SetDocumentation
        let mut root = self.inner.get_pseudo_root();
        root.set_documentation(doc);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn realPath(&self) -> String {
        self.inner.get_resolved_path().unwrap_or_default()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn fileExtension(&self) -> String {
        self.inner.get_file_extension()
    }

    #[getter]
    fn version(&self) -> Option<String> {
        self.inner.get_version()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn repositoryPath(&self) -> Option<String> {
        self.inner.get_repository_path()
    }

    #[allow(non_snake_case)]
    fn GetDisplayName(&self) -> String {
        self.inner.get_display_name()
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn GetDisplayNameFromIdentifier(identifier: &str) -> String {
        Layer::get_display_name_from_identifier(identifier)
    }

    #[allow(non_snake_case)]
    fn GetAssetName(&self) -> String {
        self.inner.get_asset_name()
    }

    #[allow(non_snake_case)]
    fn ComputeAbsolutePath(&self, asset_path: &str) -> String {
        self.inner.compute_absolute_path(asset_path)
    }

    #[allow(non_snake_case)]
    fn TransferContent(&self, source: &PyLayer) {
        self.inner.transfer_content(&source.inner);
    }

    #[allow(non_snake_case)]
    fn Import(&self, layer_path: &str) -> PyResult<bool> {
        self.inner.import(layer_path)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[allow(non_snake_case)]
    fn StreamsData(&self) -> bool {
        self.inner.streams_data()
    }

    #[allow(non_snake_case)]
    fn IsDetached(&self) -> bool {
        self.inner.is_detached()
    }

    // --- Default prim extended ----------------------------------------------

    #[allow(non_snake_case)]
    fn GetDefaultPrimAsPath(&self) -> PyPath {
        PyPath { inner: self.inner.get_default_prim_as_path() }
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn ConvertDefaultPrimTokenToPath(default_prim: &str) -> PyPath {
        PyPath { inner: Layer::convert_default_prim_token_to_path(&Token::new(default_prim)) }
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn ConvertDefaultPrimPathToToken(prim_path: &PyPath) -> String {
        Layer::convert_default_prim_path_to_token(&prim_path.inner)
            .as_str()
            .to_string()
    }

    // --- Time code properties -----------------------------------------------

    #[allow(non_snake_case)]
    fn HasStartTimeCode(&self) -> bool {
        self.inner.has_start_time_code()
    }

    #[allow(non_snake_case)]
    fn ClearStartTimeCode(&self) {
        self.inner.clear_start_time_code();
    }

    #[allow(non_snake_case)]
    fn HasEndTimeCode(&self) -> bool {
        self.inner.has_end_time_code()
    }

    #[allow(non_snake_case)]
    fn ClearEndTimeCode(&self) {
        self.inner.clear_end_time_code();
    }

    #[allow(non_snake_case)]
    fn HasTimeCodesPerSecond(&self) -> bool {
        self.inner.has_time_codes_per_second()
    }

    #[allow(non_snake_case)]
    fn ClearTimeCodesPerSecond(&self) {
        self.inner.clear_time_codes_per_second();
    }

    // --- Custom layer data --------------------------------------------------

    #[allow(non_snake_case)]
    fn HasCustomLayerData(&self) -> bool {
        self.inner.has_custom_layer_data()
    }

    #[allow(non_snake_case)]
    fn ClearCustomLayerData(&self) {
        self.inner.clear_custom_layer_data();
    }

    // --- Relocates -----------------------------------------------------------

    #[allow(non_snake_case)]
    fn HasRelocates(&self) -> bool {
        self.inner.has_relocates()
    }

    #[allow(non_snake_case)]
    fn ClearRelocates(&self) {
        self.inner.clear_relocates();
    }

    // --- Missing Layer methods -----------------------------------------------

    #[allow(non_snake_case)]
    fn GetFileFormat(&self) -> String {
        self.inner.get_file_format().map(|f| f.format_id().to_string()).unwrap_or_default()
    }
    #[allow(non_snake_case)]
    fn GetFileFormatArguments(&self) -> std::collections::HashMap<String, String> {
        self.inner.get_file_format_arguments().iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    #[allow(non_snake_case)]
    fn GetExternalReferences(&self) -> Vec<String> { self.inner.get_external_references().into_iter().collect() }
    #[allow(non_snake_case)]
    fn GetCompositionAssetDependencies(&self) -> Vec<String> { self.inner.get_composition_asset_dependencies().into_iter().collect() }
    #[allow(non_snake_case)]
    fn GetExternalAssetDependencies(&self) -> Vec<String> { self.inner.get_external_asset_dependencies().into_iter().collect() }
    #[allow(non_snake_case)]
    fn ClearColorConfiguration(&self) { self.inner.clear_color_configuration(); }
    #[allow(non_snake_case)]
    fn ClearColorManagementSystem(&self) { self.inner.clear_color_management_system(); }
    #[allow(non_snake_case)]
    fn ClearExpressionVariables(&self) { self.inner.clear_expression_variables(); }
    #[allow(non_snake_case)]
    fn ClearFramesPerSecond(&self) { self.inner.clear_frames_per_second(); }
    #[allow(non_snake_case)]
    fn ClearFramePrecision(&self) { self.inner.clear_frame_precision(); }
    #[allow(non_snake_case)]
    fn ClearOwner(&self) { self.inner.clear_owner(); }
    #[allow(non_snake_case)]
    fn ClearSessionOwner(&self) { self.inner.clear_session_owner(); }
    #[allow(non_snake_case)]
    fn ApplyRootPrimOrder(&self, order: Vec<String>) -> Vec<String> {
        let tokens: Vec<usd_tf::Token> = order.iter().map(|s| usd_tf::Token::new(s)).collect();
        self.inner.apply_root_prim_order(&tokens).iter().map(|t| t.as_str().to_string()).collect()
    }
    #[allow(non_snake_case)]
    fn UpdateCompositionAssetDependency(&self, old_path: &str, new_path: &str) {
        self.inner.update_composition_asset_dependency(old_path, new_path);
    }
    #[allow(non_snake_case)]
    fn GetBracketingTimeSamples(&self, time: f64) -> (f64, f64) {
        self.inner.get_bracketing_time_samples(time).unwrap_or((time, time))
    }

    // --- Scene modification helpers -----------------------------------------

    #[allow(non_snake_case)]
    fn RemoveInertSceneDescription(&self) {
        self.inner.remove_inert_scene_description();
    }

    // --- Permission ---------------------------------------------------------

    #[getter]
    #[allow(non_snake_case)]
    fn permissionToEdit(&self) -> bool {
        self.inner.permission_to_edit()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn permissionToSave(&self) -> bool {
        self.inner.permission_to_save()
    }

    // --- Identifier utilities -----------------------------------------------

    /// Returns true if `identifier` names an anonymous layer.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsAnonymousLayerIdentifier(identifier: &str) -> bool {
        Layer::is_anonymous_layer_identifier(identifier)
    }

    /// Splits `identifier` into (layerPath, args).
    #[staticmethod]
    #[allow(non_snake_case)]
    fn SplitIdentifier(identifier: &str, py: Python<'_>) -> Py<PyAny> {
        let mut layer_path = String::new();
        let mut args: HashMap<String, String> = HashMap::new();
        Layer::split_identifier(identifier, &mut layer_path, &mut args);
        let dict = PyDict::new(py);
        for (k, v) in &args {
            let _ = dict.set_item(k.as_str(), v.as_str());
        }
        PyTuple::new(py, [
            layer_path.into_pyobject(py).expect("ok").into_any(),
            dict.into_any(),
        ])
        .expect("tuple")
        .into_any()
        .unbind()
    }

    /// Creates an identifier from `layer_path` and `args` dict.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn CreateIdentifier(layer_path: &str, args: Option<&Bound<'_, PyDict>>) -> String {
        let map: HashMap<String, String> = args
            .map(|d| {
                d.iter()
                    .filter_map(|(k, v)| {
                        Some((k.extract::<String>().ok()?, v.extract::<String>().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Layer::create_identifier(layer_path, &map)
    }

    // --- Traverse -----------------------------------------------------------

    /// Traverse the layer hierarchy starting from `path`, calling `func` for each path.
    #[allow(non_snake_case)]
    fn Traverse(&self, path: &Bound<'_, PyAny>, func: &Bound<'_, PyAny>) -> PyResult<()> {
        let p = extract_path(path)?;
        let py = func.py();
        self.inner.traverse(&p, &|spec_path: &Path| {
            let py_path = PyPath::from_path(spec_path.clone());
            let _ = func.call1((py_path,));
        });
        let _ = py;
        Ok(())
    }

    // --- Expression variables -----------------------------------------------

    #[allow(non_snake_case)]
    fn HasExpressionVariables(&self) -> bool {
        let root_path = Path::absolute_root();
        let token = Token::new("expressionVariables");
        self.inner.get_field(&root_path, &token).is_some()
    }

    // --- Python special methods ---------------------------------------------

    fn __repr__(&self) -> String {
        format!("Sdf.Find('{}')", self.inner.identifier())
    }

    fn __bool__(&self) -> bool {
        // Layer is always valid if it exists
        true
    }

    fn __eq__(&self, other: &Self) -> bool {
        // Two PyLayer are equal if they wrap the same layer (by Arc pointer or identifier)
        Arc::ptr_eq(&self.inner, &other.inner)
            || self.inner.identifier() == other.inner.identifier()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.identifier().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// SdfPrimSpec
// ============================================================================

/// Scene description for a single prim in a layer.
///
/// Wraps `usd_sdf::PrimSpec`. Mirrors C++ `SdfPrimSpec`.
#[pyclass(skip_from_py_object,name = "PrimSpec", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPrimSpec {
    inner: PrimSpec,
}

impl PyPrimSpec {
    pub fn from_inner(inner: PrimSpec) -> Self {
        Self { inner }
    }

    pub fn from_spec(spec: PrimSpec) -> Self {
        Self { inner: spec }
    }
}

#[pymethods]
impl PyPrimSpec {
    // --- Constructors -------------------------------------------------------

    /// `Sdf.PrimSpec(layer_or_parent, name, specifier, type_name)`.
    ///
    /// If the first arg is a `Layer`, creates a root prim; if it is a
    /// `PrimSpec`, creates a child prim.
    #[new]
    #[pyo3(signature = (parent, name, specifier, type_name = ""))]
    fn new(
        parent: &Bound<'_, PyAny>,
        name: &str,
        specifier: &PySpecifier,
        type_name: &str,
    ) -> PyResult<Self> {
        // Try Layer first
        if let Ok(layer) = parent.extract::<PyRef<'_, PyLayer>>() {
            let handle = layer.inner.get_handle();
            return PrimSpec::new_root(&handle, name, specifier.inner, type_name)
                .map(|p| Self { inner: p })
                .map_err(PyRuntimeError::new_err);
        }
        // Try PrimSpec (child prim)
        if let Ok(prim) = parent.extract::<PyRef<'_, PyPrimSpec>>() {
            return PrimSpec::new_child(&prim.inner, name, specifier.inner, type_name)
                .map(|p| Self { inner: p })
                .map_err(PyRuntimeError::new_err);
        }
        Err(PyTypeError::new_err(
            "PrimSpec() first argument must be a Layer or PrimSpec",
        ))
    }

    /// Creates a root prim under `layer` (static method form).
    #[staticmethod]
    #[allow(non_snake_case)]
    #[pyo3(signature = (layer, name, specifier, type_name = ""))]
    fn CreatePrimInLayer(
        layer: &PyLayer,
        name: &str,
        specifier: &PySpecifier,
        type_name: &str,
    ) -> PyResult<Self> {
        let handle = layer.inner.get_handle();
        PrimSpec::new_root(&handle, name, specifier.inner, type_name)
            .map(|p| Self { inner: p })
            .map_err(PyRuntimeError::new_err)
    }

    // --- Properties ---------------------------------------------------------

    #[getter]
    fn name(&self) -> String {
        self.inner.name()
    }

    #[getter]
    fn path(&self) -> PyPath {
        PyPath { inner: self.inner.path() }
    }

    #[getter]
    fn layer(&self) -> Option<PyLayer> {
        let handle = self.inner.layer();
        handle.upgrade().map(PyLayer::from_arc)
    }

    #[getter]
    fn specifier(&self) -> PySpecifier {
        PySpecifier { inner: self.inner.specifier() }
    }

    #[setter]
    fn set_specifier(&mut self, s: &PySpecifier) {
        self.inner.set_specifier(s.inner);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn typeName(&self) -> String {
        self.inner.type_name().as_str().to_string()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_typeName(&mut self, name: &str) {
        self.inner.set_type_name(name);
    }

    #[getter]
    fn comment(&self) -> String {
        self.inner.comment()
    }

    #[setter]
    fn set_comment(&mut self, c: &str) {
        self.inner.set_comment(c);
    }

    #[getter]
    fn documentation(&self) -> String {
        self.inner.documentation()
    }

    #[setter]
    fn set_documentation(&mut self, d: &str) {
        self.inner.set_documentation(d);
    }

    #[getter]
    fn active(&self) -> bool {
        self.inner.active()
    }

    #[setter]
    fn set_active(&mut self, v: bool) {
        self.inner.set_active(v);
    }

    #[getter]
    fn hidden(&self) -> bool {
        self.inner.hidden()
    }

    #[setter]
    fn set_hidden(&mut self, v: bool) {
        self.inner.set_hidden(v);
    }

    #[getter]
    fn kind(&self) -> String {
        self.inner.kind().as_str().to_string()
    }

    #[setter]
    fn set_kind(&mut self, k: &str) {
        let tok = Token::new(k);
        self.inner.set_kind(&tok);
    }

    #[getter]
    fn instanceable(&self) -> bool {
        self.inner.instanceable()
    }

    #[setter]
    fn set_instanceable(&mut self, v: bool) {
        self.inner.set_instanceable(v);
    }

    /// Returns the child prims of this prim (dict-like access by name).
    #[getter]
    #[allow(non_snake_case)]
    fn nameChildren(&self) -> PyPrimSpecList {
        PyPrimSpecList { items: self.inner.name_children() }
    }

    /// Returns the attributes on this prim (dict-like access by name).
    #[getter]
    fn attributes(&self) -> PyPropertySpecList {
        let props = self.inner.attributes()
            .into_iter()
            .map(|a| PropertySpec::new(a.into_spec()))
            .collect();
        PyPropertySpecList { items: props }
    }

    /// Returns properties (attributes + relationships).
    #[getter]
    fn properties(&self) -> PyPropertySpecList {
        let props = self.inner.properties();
        PyPropertySpecList { items: props }
    }

    /// True if the underlying spec has been removed from the layer.
    #[getter]
    fn expired(&self) -> bool {
        self.inner.is_dormant()
    }

    // --- Attribute access ------------------------------------------------

    #[allow(non_snake_case)]
    fn GetAttributeAtPath(&self, path: &Bound<'_, PyAny>) -> PyResult<Option<PyAttributeSpec>> {
        let p = extract_path(path)?;
        Ok(self.inner.get_attribute_at_path(&p)
            .map(|a| PyAttributeSpec { inner: a }))
    }

    /// Returns an object (property/attribute) at `path` relative to this prim.
    #[allow(non_snake_case)]
    fn GetObjectAtPath(&self, path: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let p = extract_path(path)?;
        // Try as attribute first
        if let Some(a) = self.inner.get_attribute_at_path(&p) {
            return Ok(PyAttributeSpec { inner: a }
                .into_pyobject(py).expect("ok").into_any().unbind());
        }
        // Try as property
        if let Some(prop) = self.inner.get_property_at_path(&p) {
            return Ok(PyPropertySpec { inner: prop }
                .into_pyobject(py).expect("ok").into_any().unbind());
        }
        Ok(py.None())
    }

    // --- Composition arcs -----------------------------------------------

    #[getter]
    #[allow(non_snake_case)]
    fn referenceList(&self) -> PyReferenceListOp {
        PyReferenceListOp { inner: self.inner.references_list() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn payloadList(&self) -> PyPayloadListOp {
        PyPayloadListOp { inner: self.inner.payloads_list() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn inheritPathList(&self) -> PyPathListOp {
        PyPathListOp { inner: self.inner.inherits_list() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn specializesList(&self) -> PyPathListOp {
        PyPathListOp { inner: self.inner.specializes_list() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn variantSelections(&self, py: Python<'_>) -> Py<PyAny> {
        let map = self.inner.variant_selections();
        let dict = PyDict::new(py);
        for (k, v) in &map {
            let _ = dict.set_item(k.as_str(), v.as_str());
        }
        dict.into_any().unbind()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn hasClipSets(&self) -> bool {
        self.inner.has_clip_sets()
    }

    // --- Info (generic metadata) ----------------------------------------

    #[allow(non_snake_case)]
    fn HasInfo(&self, key: &str) -> bool {
        self.inner.has_info(&Token::new(key))
    }

    #[allow(non_snake_case)]
    fn GetInfo(&self, key: &str, py: Python<'_>) -> Py<PyAny> {
        let val = self.inner.get_info(&Token::new(key));
        if val.is_empty() {
            py.None()
        } else {
            vt_value_to_pyobject(py, &val)
        }
    }

    #[allow(non_snake_case)]
    fn SetInfo(&mut self, key: &str, value: Py<PyAny>, py: Python<'_>) {
        let val = pyobject_to_vt_value(py, &value);
        self.inner.set_info(&Token::new(key), val);
    }

    #[allow(non_snake_case)]
    fn ClearInfo(&mut self, key: &str) {
        self.inner.set_info(&Token::new(key), usd_sdf::spec::VtValue::empty());
    }

    // --- Variant set manipulation ---------------------------------------

    #[getter]
    #[allow(non_snake_case)]
    fn variantSets(&self) -> Vec<String> {
        self.inner.variant_set_name_list()
            .into_iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    /// Returns variant set name list as a ListOp (string).
    #[getter]
    #[allow(non_snake_case)]
    fn variantSetNameList(&self) -> PyStringListOp {
        // Read the "variantSetNames" field from the spec — stored as TokenListOp
        let val = self.inner.spec().get_field(&Token::new("variantSetNames"));
        if let Some(list_op) = val.get::<ListOp<Token>>() {
            // Convert TokenListOp to StringListOp
            fn tokens_to_strings(tokens: &[Token]) -> Vec<String> {
                tokens.iter().map(|t| t.as_str().to_string()).collect()
            }
            let mut result = ListOp::<String>::new();
            if list_op.is_explicit() {
                let _ = result.set_explicit_items(tokens_to_strings(list_op.get_explicit_items()));
            } else {
                let _ = result.set_prepended_items(tokens_to_strings(list_op.get_prepended_items()));
                let _ = result.set_appended_items(tokens_to_strings(list_op.get_appended_items()));
                let _ = result.set_deleted_items(tokens_to_strings(list_op.get_deleted_items()));
                result.set_added_items(tokens_to_strings(list_op.get_added_items()));
                result.set_ordered_items(tokens_to_strings(list_op.get_ordered_items()));
            }
            PyStringListOp { inner: result }
        } else {
            PyStringListOp { inner: ListOp::new() }
        }
    }

    // --- State ----------------------------------------------------------

    fn IsValid(&self) -> bool {
        !self.inner.is_dormant()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_dormant()
    }

    fn __repr__(&self) -> String {
        if self.inner.is_dormant() {
            "Sdf.PrimSpec()".to_string()
        } else {
            format!("Sdf.PrimSpec('{}')", self.inner.path().get_as_string())
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.path() == other.inner.path()
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.path().as_str().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// SdfAttributeSpec
// ============================================================================

/// Scene description for an attribute in a layer.
///
/// Wraps `usd_sdf::AttributeSpec`. Mirrors C++ `SdfAttributeSpec`.
#[pyclass(skip_from_py_object,name = "AttributeSpec", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyAttributeSpec {
    inner: AttributeSpec,
}

#[pymethods]
impl PyAttributeSpec {
    /// `Sdf.AttributeSpec(owner, name, typeName, variability=Sdf.VariabilityVarying, declaresCustom=False)`
    #[new]
    #[pyo3(signature = (owner, name, type_name, variability = None, declaresCustom = false))]
    #[allow(non_snake_case)]
    fn new(
        owner: &PyPrimSpec,
        name: &str,
        type_name: &PyValueTypeName,
        variability: Option<&str>,
        declaresCustom: bool,
    ) -> PyResult<Self> {
        // Get the layer from the prim spec
        let layer_handle = owner.inner.layer();
        let layer = layer_handle
            .upgrade()
            .ok_or_else(|| PyRuntimeError::new_err("PrimSpec has no valid layer"))?;

        let prim_path = owner.inner.path();
        let attr_path = prim_path.append_property(name)
            .ok_or_else(|| PyRuntimeError::new_err(format!("Cannot create attribute with name '{name}'")))?;

        // Check if spec already exists
        if layer.has_spec(&attr_path) {
            return Err(PyRuntimeError::new_err(format!(
                "Object already exists at path '{}'", attr_path.get_as_string()
            )));
        }

        // Create the spec in the layer
        layer.create_spec(&attr_path, SpecType::Attribute);

        // Set typeName field
        let mut attr = AttributeSpec::from_layer_and_path(layer.get_handle(), attr_path);
        attr.set_type_name(&type_name.name);

        // Set variability if specified
        if let Some(v) = variability {
            let variability_val = match v {
                "Uniform" => usd_sdf::Variability::Uniform,
                _ => usd_sdf::Variability::Varying,
            };
            attr.set_variability(variability_val);
        }

        // Set custom flag
        if declaresCustom {
            // PropertySpec stores custom flag
            let ps = PropertySpec::new(attr.as_spec().clone());
            let _ = ps;
            // Set custom field on the spec
            attr.as_spec_mut().set_field(
                &Token::new("custom"),
                usd_sdf::spec::VtValue::new(true),
            );
        }

        Ok(Self { inner: attr })
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.path().get_name().to_string()
    }

    #[setter]
    fn set_name(&mut self, new_name: &str) -> PyResult<()> {
        // Rename by creating new spec and removing old
        let old_path = self.inner.path();
        let prim_path = old_path.get_prim_path();
        let new_path = prim_path.append_property(new_name)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid name: '{new_name}'")))?;

        let layer_handle = self.inner.layer();
        let layer = layer_handle
            .upgrade()
            .ok_or_else(|| PyRuntimeError::new_err("no valid layer"))?;

        if layer.has_spec(&new_path) {
            return Err(PyRuntimeError::new_err(format!(
                "Object already exists at path '{}'", new_path.get_as_string()
            )));
        }

        // PropertySpec::set_name handles the rename
        let mut ps = PropertySpec::new(self.inner.as_spec().clone());
        if !ps.set_name(new_name, true) {
            return Err(PyRuntimeError::new_err("rename failed"));
        }
        // Update our handle to point to the new path
        self.inner = AttributeSpec::from_layer_and_path(layer.get_handle(), new_path);
        Ok(())
    }

    #[getter]
    fn path(&self) -> PyPath {
        PyPath { inner: self.inner.path() }
    }

    #[getter]
    fn layer(&self) -> Option<PyLayer> {
        self.inner.layer().upgrade().map(PyLayer::from_arc)
    }

    /// Returns the owning PrimSpec.
    #[getter]
    fn owner(&self) -> Option<PyPrimSpec> {
        let prim_path = self.inner.path().get_prim_path();
        let layer = self.inner.layer().upgrade()?;
        layer.get_prim_at_path(&prim_path).map(|p| PyPrimSpec { inner: p })
    }

    #[getter]
    #[allow(non_snake_case)]
    fn typeName(&self) -> PyValueTypeName {
        PyValueTypeName { name: self.inner.type_name() }
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_typeName(&mut self, tn: &PyValueTypeName) {
        self.inner.set_type_name(&tn.name);
    }

    #[getter]
    #[allow(non_snake_case)]
    fn variability(&self) -> &str {
        match self.inner.variability() {
            usd_sdf::Variability::Uniform => "Sdf.VariabilityUniform",
            usd_sdf::Variability::Varying => "Sdf.VariabilityVarying",
        }
    }

    #[getter]
    fn custom(&self) -> bool {
        let ps = PropertySpec::new(self.inner.as_spec().clone());
        ps.custom()
    }

    #[setter]
    fn set_custom(&mut self, v: bool) {
        let _ = self.inner.as_spec_mut().set_field(
            &Token::new("custom"),
            usd_sdf::spec::VtValue::new(v),
        );
    }

    #[getter]
    #[allow(non_snake_case)]
    fn hasDefaultValue(&self) -> bool {
        self.inner.has_default_value()
    }

    #[getter]
    #[pyo3(name = "default")]
    fn get_default(&self, py: Python<'_>) -> Py<PyAny> {
        if self.inner.has_default_value() {
            vt_value_to_pyobject(py, &self.inner.default_value())
        } else {
            py.None()
        }
    }

    #[setter]
    #[pyo3(name = "default")]
    fn set_default(&mut self, value: &Bound<'_, PyAny>) {
        if value.is_none() {
            self.inner.clear_default_value();
        } else {
            let py = value.py();
            let val = pyobject_to_vt_value(py, &value.clone().unbind());
            self.inner.set_default_value(val);
        }
    }

    /// Returns the connection path list op.
    #[getter]
    #[allow(non_snake_case)]
    fn connectionPathList(&self) -> PyPathListOp {
        PyPathListOp { inner: self.inner.connection_paths_list() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn hasTimeSamples(&self) -> bool {
        self.inner.has_time_samples()
    }

    // --- Info (metadata) on AttributeSpec --------------------------------

    #[allow(non_snake_case)]
    fn HasInfo(&self, key: &str) -> bool {
        !self.inner.as_spec().get_field(&Token::new(key)).is_empty()
    }

    #[allow(non_snake_case)]
    fn GetInfo(&self, key: &str, py: Python<'_>) -> Py<PyAny> {
        let val = self.inner.as_spec().get_field(&Token::new(key));
        if val.is_empty() { py.None() } else { vt_value_to_pyobject(py, &val) }
    }

    #[allow(non_snake_case)]
    fn SetInfo(&mut self, key: &str, value: Py<PyAny>, py: Python<'_>) {
        let val = pyobject_to_vt_value(py, &value);
        let _ = self.inner.as_spec_mut().set_field(&Token::new(key), val);
    }

    #[allow(non_snake_case)]
    fn ClearInfo(&mut self, key: &str) {
        let _ = self.inner.as_spec_mut().set_field(&Token::new(key), usd_sdf::spec::VtValue::empty());
    }

    /// Check if spec is inert (has no non-default data).
    #[allow(non_snake_case)]
    #[pyo3(signature = (ignore_children = false))]
    fn IsInert(&self, ignore_children: bool) -> bool {
        self.inner.as_spec().is_inert(ignore_children)
    }

    /// Check if this attribute has limit metadata.
    #[allow(non_snake_case)]
    fn HasLimits(&self) -> bool {
        self.inner.has_limits()
    }

    /// Check if this attribute has an arraySizeConstraint.
    #[allow(non_snake_case)]
    fn HasArraySizeConstraint(&self) -> bool {
        !self.inner.as_spec().get_field(&Token::new("arraySizeConstraint")).is_empty()
    }

    /// Clear arraySizeConstraint.
    #[allow(non_snake_case)]
    fn ClearArraySizeConstraint(&mut self) {
        let _ = self.inner.as_spec_mut().set_field(
            &Token::new("arraySizeConstraint"),
            usd_sdf::spec::VtValue::empty(),
        );
    }

    /// Get arraySizeConstraint.
    #[getter]
    #[allow(non_snake_case)]
    fn arraySizeConstraint(&self) -> i64 {
        self.inner.as_spec().get_field(&Token::new("arraySizeConstraint"))
            .get::<i64>()
            .copied()
            .or_else(|| self.inner.as_spec().get_field(&Token::new("arraySizeConstraint"))
                .get::<i32>()
                .map(|v| *v as i64))
            .unwrap_or(0)
    }

    /// Custom data dictionary.
    #[getter]
    #[allow(non_snake_case)]
    fn customData(&self, py: Python<'_>) -> Py<PyAny> {
        let ps = PropertySpec::new(self.inner.as_spec().clone());
        let data = ps.custom_data();
        let dict = PyDict::new(py);
        for (k, v) in &data {
            let _ = dict.set_item(k.as_str(), vt_value_to_pyobject(py, v));
        }
        dict.into_any().unbind()
    }

    #[setter]
    #[allow(non_snake_case)]
    fn set_customData(&mut self, value: &Bound<'_, PyDict>) {
        let py = value.py();
        let mut dict = usd_vt::Dictionary::new();
        for (k, v) in value.iter() {
            if let Ok(key) = k.extract::<String>() {
                dict.insert(key, pyobject_to_vt_value(py, &v.unbind()));
            }
        }
        let val = usd_sdf::spec::VtValue::new(dict);
        let _ = self.inner.as_spec_mut().set_field(&Token::new("customData"), val);
    }

    fn IsValid(&self) -> bool {
        !self.inner.is_dormant()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_dormant()
    }

    fn __repr__(&self) -> String {
        format!("Sdf.AttributeSpec('{}')", self.inner.path().get_as_string())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.path() == other.inner.path()
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner.path() != other.inner.path()
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.path().as_str().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// SdfPropertySpec — thin wrapper for PrimSpec.properties list
// ============================================================================

/// Property spec (attribute or relationship) within a layer.
///
/// Wraps `usd_sdf::PropertySpec`. Mirrors C++ `SdfPropertySpec`.
#[pyclass(skip_from_py_object, name = "PropertySpec", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPropertySpec {
    inner: PropertySpec,
}

#[pymethods]
impl PyPropertySpec {
    /// PropertySpec cannot be constructed directly (abstract in C++).
    #[new]
    fn new() -> PyResult<Self> {
        Err(PyRuntimeError::new_err(
            "Cannot construct PropertySpec directly — use AttributeSpec or RelationshipSpec",
        ))
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }

    #[getter]
    fn path(&self) -> PyPath {
        PyPath::from_path(self.inner.spec().path())
    }

    fn __repr__(&self) -> String {
        format!("Sdf.PropertySpec('{}')", self.inner.spec().path().get_as_string())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.spec().path() == other.inner.spec().path()
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.spec().path().as_str().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// PyPropertySpecList — list-like + dict-like container for PrimSpec.properties
// ============================================================================

/// List of PropertySpec objects that supports indexing by int and by name.
#[pyclass(skip_from_py_object, name = "_PropertySpecList", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPropertySpecList {
    items: Vec<PropertySpec>,
}

#[pymethods]
impl PyPropertySpecList {
    fn __len__(&self) -> usize {
        self.items.len()
    }

    fn __getitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<PyAttributeSpec> {
        if let Ok(idx) = key.extract::<isize>() {
            let idx = if idx < 0 { self.items.len() as isize + idx } else { idx } as usize;
            self.items
                .get(idx)
                .map(|p| PyAttributeSpec { inner: AttributeSpec::new(p.spec().clone()) })
                .ok_or_else(|| PyValueError::new_err("index out of range"))
        } else if let Ok(name) = key.extract::<String>() {
            self.items
                .iter()
                .find(|p| p.name().as_str() == name)
                .map(|p| PyAttributeSpec { inner: AttributeSpec::new(p.spec().clone()) })
                .ok_or_else(|| PyValueError::new_err(format!("no property '{name}'")))
        } else {
            Err(PyTypeError::new_err("index must be int or str"))
        }
    }

    fn __contains__(&self, key: &Bound<'_, PyAny>) -> bool {
        if let Ok(name) = key.extract::<String>() {
            self.items.iter().any(|p| p.name().as_str() == name)
        } else if let Ok(attr) = key.extract::<PyRef<'_, PyAttributeSpec>>() {
            let attr_path = attr.inner.path();
            self.items.iter().any(|p| p.spec().path() == attr_path)
        } else {
            false
        }
    }

    fn __iter__(&self) -> PyPropertySpecListIter {
        PyPropertySpecListIter {
            items: self.items.clone(),
            index: 0,
        }
    }
}

/// Iterator for PropertySpecList.
#[pyclass(skip_from_py_object)]
struct PyPropertySpecListIter {
    items: Vec<PropertySpec>,
    index: usize,
}

#[pymethods]
impl PyPropertySpecListIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<PyAttributeSpec> {
        if self.index < self.items.len() {
            let p = &self.items[self.index];
            self.index += 1;
            Some(PyAttributeSpec {
                inner: AttributeSpec::new(p.spec().clone()),
            })
        } else {
            None
        }
    }
}

// ============================================================================
// PyPrimSpecList — dict-like container for Layer.rootPrims / PrimSpec.nameChildren
// ============================================================================

/// List of PrimSpec objects that supports indexing by int and by name.
#[pyclass(skip_from_py_object, name = "_PrimSpecList", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPrimSpecList {
    items: Vec<PrimSpec>,
}

#[pymethods]
impl PyPrimSpecList {
    fn __len__(&self) -> usize {
        self.items.len()
    }

    fn __getitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<PyPrimSpec> {
        if let Ok(idx) = key.extract::<isize>() {
            let idx = if idx < 0 { self.items.len() as isize + idx } else { idx } as usize;
            self.items.get(idx)
                .map(|p| PyPrimSpec { inner: p.clone() })
                .ok_or_else(|| PyValueError::new_err("index out of range"))
        } else if let Ok(name) = key.extract::<String>() {
            self.items.iter()
                .find(|p| p.name() == name)
                .map(|p| PyPrimSpec { inner: p.clone() })
                .ok_or_else(|| PyValueError::new_err(format!("no child prim '{name}'")))
        } else {
            Err(PyTypeError::new_err("index must be int or str"))
        }
    }

    fn __delitem__(&mut self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(idx) = key.extract::<usize>() {
            if idx < self.items.len() {
                self.items.remove(idx);
                Ok(())
            } else {
                Err(PyValueError::new_err("index out of range"))
            }
        } else if let Ok(name) = key.extract::<String>() {
            if let Some(pos) = self.items.iter().position(|p| p.name() == name) {
                self.items.remove(pos);
                Ok(())
            } else {
                Err(PyValueError::new_err(format!("no child prim '{name}'")))
            }
        } else {
            Err(PyTypeError::new_err("index must be int or str"))
        }
    }

    fn __contains__(&self, key: &Bound<'_, PyAny>) -> bool {
        if let Ok(name) = key.extract::<String>() {
            self.items.iter().any(|p| p.name() == name)
        } else {
            false
        }
    }

    fn __iter__(&self) -> PyPrimSpecListIter {
        PyPrimSpecListIter { items: self.items.clone(), index: 0 }
    }

    fn __bool__(&self) -> bool { !self.items.is_empty() }
}

#[pyclass(skip_from_py_object)]
struct PyPrimSpecListIter {
    items: Vec<PrimSpec>,
    index: usize,
}

#[pymethods]
impl PyPrimSpecListIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> { slf }

    fn __next__(&mut self) -> Option<PyPrimSpec> {
        if self.index < self.items.len() {
            let p = self.items[self.index].clone();
            self.index += 1;
            Some(PyPrimSpec { inner: p })
        } else {
            None
        }
    }
}

// ============================================================================
// SdfSpecifier
// ============================================================================

/// Specifier for a prim: `Def`, `Over`, or `Class`.
///
/// Wraps `usd_sdf::Specifier`. Mirrors C++ `SdfSpecifier`.
#[pyclass(skip_from_py_object,name = "Specifier", module = "pxr_rs.Sdf")]
#[derive(Clone, Copy)]
pub struct PySpecifier {
    inner: Specifier,
}

#[pymethods]
impl PySpecifier {
    #[classattr]
    #[allow(non_snake_case)]
    fn SpecifierDef() -> Self {
        Self { inner: Specifier::Def }
    }

    #[classattr]
    #[allow(non_snake_case)]
    fn SpecifierOver() -> Self {
        Self { inner: Specifier::Over }
    }

    #[classattr]
    #[allow(non_snake_case)]
    fn SpecifierClass() -> Self {
        Self { inner: Specifier::Class }
    }

    fn __repr__(&self) -> &'static str {
        match self.inner {
            Specifier::Def => "Sdf.SpecifierDef",
            Specifier::Over => "Sdf.SpecifierOver",
            Specifier::Class => "Sdf.SpecifierClass",
        }
    }

    fn __str__(&self) -> &'static str {
        self.inner.as_str()
    }

    fn __eq__(&self, other: &PySpecifier) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        self.inner as u64
    }
}

// ============================================================================
// SdfChangeBlock — context manager
// ============================================================================

/// Batches SDF change notifications until the block exits.
///
/// Wraps `usd_sdf::ChangeBlock`. Mirrors C++ `SdfChangeBlock`.
///
/// Usage:
/// ```python
/// with Sdf.ChangeBlock():
///     # make many changes...
/// ```
#[pyclass(skip_from_py_object,name = "ChangeBlock", module = "pxr_rs.Sdf")]
pub struct PyChangeBlock {
    // Held alive for the duration of the with-block; drop triggers flush.
    _block: usd_sdf::ChangeBlock,
}

#[pymethods]
impl PyChangeBlock {
    #[new]
    fn new() -> Self {
        Self { _block: usd_sdf::ChangeBlock::new() }
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(&mut self, _exc_type: Py<PyAny>, _exc_val: Py<PyAny>, _exc_tb: Py<PyAny>) -> bool {
        // Block is dropped when this object is dropped (end of with-statement).
        // Return false to propagate any exception.
        false
    }
}

// ============================================================================
// Helpers: VtValue <-> Python object bridging
// ============================================================================

/// Converts a `usd_vt::Value` to a Python object.
///
/// Falls back to `repr()` string for unsupported types so Python callers
/// always get something usable rather than None.
fn vt_value_to_pyobject(py: Python<'_>, val: &usd_vt::Value) -> Py<PyAny> {
    if let Some(b) = val.get::<bool>() {
        return b.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
    }
    if let Some(i) = val.get::<i32>() {
        return i.into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(i) = val.get::<i64>() {
        return i.into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(f) = val.get::<f32>() {
        return (*f as f64).into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(f) = val.get::<f64>() {
        return f.into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(s) = val.get::<String>() {
        return s.as_str().into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(t) = val.get::<Token>() {
        return t.as_str().into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.get::<Vec<f32>>() {
        let list = PyList::new(py, v.iter().map(|x| *x as f64)).expect("list");
        return list.into_any().unbind();
    }
    if let Some(v) = val.get::<Vec<f64>>() {
        let list = PyList::new(py, v.iter().copied()).expect("list");
        return list.into_any().unbind();
    }
    if let Some(v) = val.get::<Vec<i32>>() {
        let list = PyList::new(py, v.iter().copied()).expect("list");
        return list.into_any().unbind();
    }
    // Fallback: debug string
    format!("{val:?}").into_pyobject(py).expect("ok").into_any().unbind()
}

/// Converts a Python object to a `usd_vt::Value`.
///
/// Supports the most common scalar types. Unknown types become empty Value.
fn pyobject_to_vt_value(py: Python<'_>, obj: &Py<PyAny>) -> usd_vt::Value {
    let bound = obj.bind(py);
    if let Ok(b) = bound.extract::<bool>() {
        return usd_vt::Value::new(b);
    }
    if let Ok(i) = bound.extract::<i64>() {
        return usd_vt::Value::new(i);
    }
    if let Ok(f) = bound.extract::<f64>() {
        return usd_vt::Value::from_f64(f);
    }
    if let Ok(s) = bound.extract::<String>() {
        return usd_vt::Value::new(s);
    }
    usd_vt::Value::empty()
}

// ============================================================================
// ValueTypeName — wraps SdfValueTypeName
// ============================================================================

/// An attribute's value type name (e.g. "double", "float3", "token").
///
/// Matches C++ `SdfValueTypeName`.
#[pyclass(skip_from_py_object, name = "ValueTypeName", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyValueTypeName {
    pub(crate) name: String,
}

impl PyValueTypeName {
    /// Convert to the Rust `ValueTypeName` via the global type registry.
    pub fn inner(&self) -> usd_sdf::ValueTypeName {
        let token = usd_tf::Token::new(&self.name);
        usd_sdf::types::get_type_for_value_type_name(&token)
    }
}

#[pymethods]
impl PyValueTypeName {
    fn __repr__(&self) -> String {
        format!("Sdf.ValueTypeNames.{}", self.name)
    }

    fn __str__(&self) -> &str {
        &self.name
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.name == other.name
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.name.hash(&mut h);
        h.finish()
    }
}

/// Namespace containing standard value type names.
///
/// Registered at module level via `setattr` in `register()` rather than
/// `#[classattr]`, because PyO3 proc-macro attributes can't be generated
/// from `macro_rules!` inside `#[pymethods]`.
#[pyclass(skip_from_py_object, name = "ValueTypeNames", module = "pxr_rs.Sdf")]
pub struct PyValueTypeNames;

/// Add all standard value type name constants as class attributes on
/// `PyValueTypeNames`.
fn register_value_type_names(py: Python<'_>) -> PyResult<()> {
    let vtn = py.get_type::<PyValueTypeNames>();

    // Helper closure to set a named attr
    let set = |name: &str, type_name: &str| -> PyResult<()> {
        let obj = Py::new(py, PyValueTypeName { name: type_name.to_string() })?;
        vtn.setattr(name, obj)
    };

    // Scalar types
    set("Bool", "bool")?;
    set("UChar", "uchar")?;
    set("Int", "int")?;
    set("UInt", "uint")?;
    set("Int64", "int64")?;
    set("UInt64", "uint64")?;
    set("Half", "half")?;
    set("Float", "float")?;
    set("Double", "double")?;
    set("TimeCode", "timecode")?;
    set("String", "string")?;
    set("Token", "token")?;
    set("Asset", "asset")?;

    set("Int2", "int2")?;
    set("Int3", "int3")?;
    set("Int4", "int4")?;
    set("Half2", "half2")?;
    set("Half3", "half3")?;
    set("Half4", "half4")?;
    set("Float2", "float2")?;
    set("Float3", "float3")?;
    set("Float4", "float4")?;
    set("Double2", "double2")?;
    set("Double3", "double3")?;
    set("Double4", "double4")?;

    set("Point3h", "point3h")?;
    set("Point3f", "point3f")?;
    set("Point3d", "point3d")?;
    set("Normal3h", "normal3h")?;
    set("Normal3f", "normal3f")?;
    set("Normal3d", "normal3d")?;
    set("Color3h", "color3h")?;
    set("Color3f", "color3f")?;
    set("Color3d", "color3d")?;
    set("Color4h", "color4h")?;
    set("Color4f", "color4f")?;
    set("Color4d", "color4d")?;
    set("Vector3h", "vector3h")?;
    set("Vector3f", "vector3f")?;
    set("Vector3d", "vector3d")?;
    set("TexCoord2h", "texCoord2h")?;
    set("TexCoord2f", "texCoord2f")?;
    set("TexCoord2d", "texCoord2d")?;
    set("TexCoord3h", "texCoord3h")?;
    set("TexCoord3f", "texCoord3f")?;
    set("TexCoord3d", "texCoord3d")?;

    set("Quath", "quath")?;
    set("Quatf", "quatf")?;
    set("Quatd", "quatd")?;

    set("Matrix2d", "matrix2d")?;
    set("Matrix3d", "matrix3d")?;
    set("Matrix4d", "matrix4d")?;
    set("Frame4d", "frame4d")?;

    // Special types
    set("Opaque", "opaque")?;
    set("Group", "group")?;
    set("PathExpression", "pathExpression")?;

    // Array types
    set("BoolArray", "bool[]")?;
    set("UCharArray", "uchar[]")?;
    set("IntArray", "int[]")?;
    set("UIntArray", "uint[]")?;
    set("Int64Array", "int64[]")?;
    set("UInt64Array", "uint64[]")?;
    set("HalfArray", "half[]")?;
    set("FloatArray", "float[]")?;
    set("DoubleArray", "double[]")?;
    set("TimeCodeArray", "timecode[]")?;
    set("StringArray", "string[]")?;
    set("TokenArray", "token[]")?;
    set("AssetArray", "asset[]")?;

    set("Int2Array", "int2[]")?;
    set("Int3Array", "int3[]")?;
    set("Int4Array", "int4[]")?;
    set("Half2Array", "half2[]")?;
    set("Half3Array", "half3[]")?;
    set("Half4Array", "half4[]")?;
    set("Float2Array", "float2[]")?;
    set("Float3Array", "float3[]")?;
    set("Float4Array", "float4[]")?;
    set("Double2Array", "double2[]")?;
    set("Double3Array", "double3[]")?;
    set("Double4Array", "double4[]")?;

    set("Point3hArray", "point3h[]")?;
    set("Point3fArray", "point3f[]")?;
    set("Point3dArray", "point3d[]")?;
    set("Normal3hArray", "normal3h[]")?;
    set("Normal3fArray", "normal3f[]")?;
    set("Normal3dArray", "normal3d[]")?;
    set("Color3hArray", "color3h[]")?;
    set("Color3fArray", "color3f[]")?;
    set("Color3dArray", "color3d[]")?;
    set("Color4hArray", "color4h[]")?;
    set("Color4fArray", "color4f[]")?;
    set("Color4dArray", "color4d[]")?;
    set("Vector3hArray", "vector3h[]")?;
    set("Vector3fArray", "vector3f[]")?;
    set("Vector3dArray", "vector3d[]")?;
    set("TexCoord2hArray", "texCoord2h[]")?;
    set("TexCoord2fArray", "texCoord2f[]")?;
    set("TexCoord2dArray", "texCoord2d[]")?;
    set("TexCoord3hArray", "texCoord3h[]")?;
    set("TexCoord3fArray", "texCoord3f[]")?;
    set("TexCoord3dArray", "texCoord3d[]")?;

    set("QuathArray", "quath[]")?;
    set("QuatfArray", "quatf[]")?;
    set("QuatdArray", "quatd[]")?;

    set("Matrix2dArray", "matrix2d[]")?;
    set("Matrix3dArray", "matrix3d[]")?;
    set("Matrix4dArray", "matrix4d[]")?;
    set("Frame4dArray", "frame4d[]")?;

    Ok(())
}

// ============================================================================
// PathExpression — mirrors C++ SdfPathExpression
// ============================================================================

/// A path expression that can match paths using patterns and set operations.
///
/// Matches C++ `SdfPathExpression`.
#[pyclass(skip_from_py_object, name = "PathExpression", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPathExpression {
    inner: PathExpression,
}

#[pymethods]
impl PyPathExpression {
    #[new]
    #[pyo3(signature = (expr = ""))]
    fn new(expr: &str) -> Self {
        Self { inner: PathExpression::parse(expr) }
    }

    /// Returns the canonical text representation of this expression.
    #[allow(non_snake_case)]
    fn GetText(&self) -> String {
        self.inner.get_text()
    }

    /// True if the expression is empty (matches nothing).
    #[allow(non_snake_case)]
    fn IsEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// True if the expression is absolute (starts with `/`).
    #[allow(non_snake_case)]
    fn IsAbsolute(&self) -> bool {
        self.inner.is_absolute()
    }

    /// True if the expression references other named expressions.
    #[allow(non_snake_case)]
    fn ContainsExpressionReferences(&self) -> bool {
        self.inner.contains_expression_references()
    }

    /// Return the empty expression (matches nothing).
    #[staticmethod]
    #[allow(non_snake_case)]
    fn Nothing() -> Self {
        Self { inner: PathExpression::new() }
    }

    /// Return the expression that matches everything (`//`).
    #[staticmethod]
    #[allow(non_snake_case)]
    fn Everything() -> Self {
        Self { inner: PathExpression::everything() }
    }

    /// Return the complement of the given expression.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn MakeComplement(expr: &PyPathExpression) -> Self {
        Self { inner: PathExpression::make_complement(expr.inner.clone()) }
    }

    /// True if the expression contains a weaker expression reference (%_).
    #[allow(non_snake_case)]
    fn ContainsWeakerExpressionReference(&self) -> bool {
        self.inner.contains_weaker_expression_reference()
    }

    /// Make this expression absolute relative to `anchor`.
    #[allow(non_snake_case)]
    fn MakeAbsolute(&self, anchor: &PyPath) -> Self {
        Self { inner: self.inner.make_absolute(&anchor.inner) }
    }

    /// Replace path prefixes in this expression.
    #[allow(non_snake_case)]
    fn ReplacePrefix(&self, old_prefix: &PyPath, new_prefix: &PyPath) -> Self {
        Self { inner: self.inner.replace_prefix(&old_prefix.inner, &new_prefix.inner) }
    }

    /// Compose this expression over a weaker expression.
    #[allow(non_snake_case)]
    fn ComposeOver(&self, weaker: &PyPathExpression) -> Self {
        Self { inner: self.inner.compose_over(&weaker.inner) }
    }

    /// Resolve references in this expression with a provided callback is not supported;
    /// return self unchanged (stub).
    #[allow(non_snake_case)]
    fn ResolveReferences(&self, _callback: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __repr__(&self) -> String {
        format!("Sdf.PathExpression('{}')", self.inner.get_text())
    }

    fn __str__(&self) -> String {
        self.inner.get_text()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.hash(&mut h);
        h.finish()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }
}

// ============================================================================
// _MakeBasicMatchEval — test helper for path expression matching
// ============================================================================

/// Evaluator returned by `Sdf._MakeBasicMatchEval`.
///
/// Wraps `PathExpressionEval<()>` with a simple `Match(path)` API.
#[pyclass(skip_from_py_object, name = "_BasicMatchEval", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyBasicMatchEval {
    eval: PathExpressionEval<()>,
}

#[pymethods]
impl PyBasicMatchEval {
    /// Match the given path against the expression.
    ///
    /// Accepts a string or `Sdf.Path`.
    #[allow(non_snake_case)]
    fn Match(&self, path: &Bound<'_, PyAny>) -> PyResult<bool> {
        let p = if let Ok(py_path) = path.extract::<PyPath>() {
            py_path.inner.clone()
        } else if let Ok(s) = path.extract::<String>() {
            Path::from_string(&s).ok_or_else(|| {
                PyValueError::new_err(format!("Invalid SdfPath: '{s}'"))
            })?
        } else {
            return Err(PyValueError::new_err(
                "Match() argument must be a string or Sdf.Path",
            ));
        };
        let result = self.eval.match_path(&p, |_p| ());
        Ok(result.is_truthy())
    }

    fn __repr__(&self) -> &str {
        "Sdf._BasicMatchEval()"
    }
}

/// Create a basic match evaluator from a path expression string.
///
/// This is a module-level function exposed as `Sdf._MakeBasicMatchEval`.
/// Used by the test suite as a convenience for creating evaluators.
#[pyfunction]
#[pyo3(name = "_MakeBasicMatchEval")]
fn make_basic_match_eval(pattern: &str) -> PyBasicMatchEval {
    let expr = PathExpression::parse(pattern);
    let eval = PathExpressionEval::<()>::from_expression(&expr);
    PyBasicMatchEval { eval }
}

// ============================================================================
// VariableExpression — stub for Sdf.VariableExpression
// ============================================================================

/// Wraps `usd_sdf::VariableExpression`.
#[pyclass(skip_from_py_object, name = "VariableExpression", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyVariableExpression {
    inner: VariableExpression,
    source: String,
}

#[pymethods]
impl PyVariableExpression {
    #[new]
    #[pyo3(signature = (expr = ""))]
    fn new(expr: &str) -> Self {
        Self {
            inner: VariableExpression::new(expr),
            source: expr.to_string(),
        }
    }

    /// True if the expression is valid (parsed without errors).
    #[allow(non_snake_case)]
    fn IsValid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns parse errors, if any.
    #[allow(non_snake_case)]
    fn GetErrors(&self) -> Vec<String> {
        self.inner.get_errors().to_vec()
    }

    /// Evaluate expression with variable dictionary.
    #[allow(non_snake_case)]
    fn Evaluate(&self, variables: &Bound<'_, PyDict>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let mut dict = usd_vt::Dictionary::new();
        for (k, v) in variables.iter() {
            let key = k.extract::<String>()?;
            let val = pyobject_to_vt_value(py, &v.unbind());
            dict.insert(key, val);
        }
        let result = self.inner.evaluate(&dict);
        // Return (value, errors) tuple like C++
        let val = match &result.value {
            Some(v) => vt_value_to_pyobject(py, v),
            None => py.None(),
        };
        let errors: Vec<String> = result.errors.clone();
        let err_list = PyList::new(py, &errors).expect("list");
        Ok(PyTuple::new(py, [val, err_list.into_any().unbind()])
            .expect("tuple")
            .into_any()
            .unbind())
    }

    fn __repr__(&self) -> String {
        format!("Sdf.VariableExpression('{}')", self.source)
    }

    fn __str__(&self) -> &str {
        &self.source
    }

    fn __bool__(&self) -> bool {
        !self.source.is_empty() && self.inner.is_valid()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.source == other.source
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.source.hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// VariableExpressionASTNodes — stub namespace for AST node types
// ============================================================================

/// Stub for `Sdf.VariableExpressionASTNodes`.
///
/// Contains marker classes for AST node types used in variable expression tests.
#[pyclass(skip_from_py_object, name = "LiteralNode", module = "pxr_rs.Sdf")]
pub struct PyLiteralNode;

#[pyclass(skip_from_py_object, name = "VariableNode", module = "pxr_rs.Sdf")]
pub struct PyVariableNode;

#[pyclass(skip_from_py_object, name = "ListNode", module = "pxr_rs.Sdf")]
pub struct PyListNode;

#[pyclass(skip_from_py_object, name = "FunctionNode", module = "pxr_rs.Sdf")]
pub struct PyFunctionNode;

// ============================================================================
// SdfAssetPath
// ============================================================================

#[pyclass(from_py_object, name = "AssetPath", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyAssetPath { inner: AssetPath }

impl PyAssetPath {
    pub fn from_asset_path(ap: AssetPath) -> Self {
        Self { inner: ap }
    }

    pub fn to_asset_path(&self) -> AssetPath {
        self.inner.clone()
    }
}

fn has_control_chars(s: &str) -> bool {
    s.chars().any(|c| { let code = c as u32; code <= 0x1F || code == 0x7F || (0x80..=0x9F).contains(&code) })
}

#[pymethods]
impl PyAssetPath {
    #[new]
    #[pyo3(signature = (*args, authoredPath = None, evaluatedPath = None, resolvedPath = None))]
    fn new(args: &Bound<'_, PyTuple>, authoredPath: Option<&str>, evaluatedPath: Option<&str>, resolvedPath: Option<&str>) -> PyResult<Self> {
        if args.len() > 2 { return Err(PyTypeError::new_err("AssetPath() takes at most 2 positional arguments")); }
        // Disallow mixing positional with conflicting keywords
        if args.len() >= 2 && (evaluatedPath.is_some() || resolvedPath.is_some()) {
            return Err(PyTypeError::new_err("AssetPath() positional and keyword arguments conflict"));
        }
        if args.len() >= 1 && (evaluatedPath.is_some() && resolvedPath.is_some()) {
            return Err(PyTypeError::new_err("AssetPath() positional and keyword arguments conflict"));
        }
        if args.len() == 1 { if let Ok(other) = args.get_item(0)?.extract::<PyAssetPath>() { return Ok(Self { inner: other.inner }); } }
        let authored = if let Some(a) = authoredPath { a.to_string() } else if !args.is_empty() { args.get_item(0)?.extract::<String>()? } else { String::new() };
        if !authored.is_empty() && has_control_chars(&authored) { return Err(pyo3::exceptions::PyException::new_err("Asset path contains invalid control characters")); }
        let resolved = if let Some(r) = resolvedPath { r.to_string() } else if args.len() >= 2 && evaluatedPath.is_none() { args.get_item(1)?.extract::<String>()? } else { String::new() };
        if !resolved.is_empty() && has_control_chars(&resolved) { return Err(pyo3::exceptions::PyException::new_err("Resolved path contains invalid control characters")); }
        let evaluated = evaluatedPath.unwrap_or("").to_string();
        Ok(Self { inner: AssetPath::from_params(usd_vt::AssetPathParams::new().authored(authored).evaluated(evaluated).resolved(resolved)) })
    }
    #[getter] #[allow(non_snake_case)] fn authoredPath(&self) -> &str { self.inner.get_authored_path() }
    #[getter] #[allow(non_snake_case)] fn evaluatedPath(&self) -> &str { self.inner.get_evaluated_path() }
    #[getter] #[allow(non_snake_case)] fn resolvedPath(&self) -> &str { self.inner.get_resolved_path() }
    #[getter] fn path(&self) -> &str { self.inner.get_asset_path() }
    fn __repr__(&self) -> String {
        let (a, e, r) = (self.inner.get_authored_path(), self.inner.get_evaluated_path(), self.inner.get_resolved_path());
        if !e.is_empty() { format!("Sdf.AssetPath(authoredPath='{}', evaluatedPath='{}', resolvedPath='{}')", a, e, r) }
        else if !r.is_empty() { format!("Sdf.AssetPath('{}', '{}')", a, r) }
        else { format!("Sdf.AssetPath('{}')", a) }
    }
    fn __str__(&self) -> &str { self.inner.get_asset_path() }
    fn __eq__(&self, other: &Self) -> bool { self.inner == other.inner }
    fn __ne__(&self, other: &Self) -> bool { self.inner != other.inner }
    fn __lt__(&self, other: &Self) -> bool { self.inner < other.inner }
    fn __le__(&self, other: &Self) -> bool { self.inner <= other.inner }
    fn __gt__(&self, other: &Self) -> bool { self.inner > other.inner }
    fn __ge__(&self, other: &Self) -> bool { self.inner >= other.inner }
    fn __hash__(&self) -> u64 { self.inner.get_hash() }
    fn __bool__(&self) -> bool { !self.inner.is_empty() }
}

// ============================================================================
// SdfTimeCode
// ============================================================================

#[pyclass(from_py_object, name = "TimeCode", module = "pxr_rs.Sdf")]
#[derive(Clone, Copy)]
pub struct PyTimeCode { inner: TimeCode }

#[pymethods]
impl PyTimeCode {
    #[new] #[pyo3(signature = (value = 0.0))]
    fn new(value: f64) -> Self { Self { inner: TimeCode::new(value) } }
    #[allow(non_snake_case)] fn GetValue(&self) -> f64 { self.inner.value() }
    fn __repr__(&self) -> String { let v = self.inner.value(); if v == v.trunc() && v.is_finite() { format!("Sdf.TimeCode({})", v as i64) } else { format!("Sdf.TimeCode({})", v) } }
    fn __str__(&self) -> String { let v = self.inner.value(); if v == v.trunc() && v.is_finite() { format!("{}", v as i64) } else { format!("{v}") } }
    fn __float__(&self) -> f64 { self.inner.value() }
    fn __bool__(&self) -> bool { self.inner.value() != 0.0 }
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool { if let Ok(tc) = other.extract::<PyTimeCode>() { self.inner == tc.inner } else if let Ok(f) = other.extract::<f64>() { self.inner.value() == f } else { false } }
    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool { !self.__eq__(other) }
    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> { Ok(self.inner.value() < extract_tc(other)?) }
    fn __le__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> { Ok(self.inner.value() <= extract_tc(other)?) }
    fn __gt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> { Ok(self.inner.value() > extract_tc(other)?) }
    fn __ge__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> { Ok(self.inner.value() >= extract_tc(other)?) }
    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { Ok(Self { inner: TimeCode::new(self.inner.value() + extract_tc(other)?) }) }
    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { self.__add__(other) }
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { Ok(Self { inner: TimeCode::new(self.inner.value() - extract_tc(other)?) }) }
    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { Ok(Self { inner: TimeCode::new(extract_tc(other)? - self.inner.value()) }) }
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { Ok(Self { inner: TimeCode::new(self.inner.value() * extract_tc(other)?) }) }
    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { self.__mul__(other) }
    fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { Ok(Self { inner: TimeCode::new(self.inner.value() / extract_tc(other)?) }) }
    fn __rtruediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> { Ok(Self { inner: TimeCode::new(extract_tc(other)? / self.inner.value()) }) }
    fn __hash__(&self) -> u64 { self.inner.get_hash() }
}
fn extract_tc(obj: &Bound<'_, PyAny>) -> PyResult<f64> { if let Ok(tc) = obj.extract::<PyTimeCode>() { Ok(tc.inner.value()) } else if let Ok(f) = obj.extract::<f64>() { Ok(f) } else { Err(PyTypeError::new_err("expected TimeCode or number")) } }

// ============================================================================
// SdfPayload
// ============================================================================

#[pyclass(from_py_object, name = "Payload", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPayload { inner: Payload }
#[pymethods]
impl PyPayload {
    #[new] #[pyo3(signature = (*args, assetPath = None, primPath = None, layerOffset = None))]
    fn new(args: &Bound<'_, PyTuple>, assetPath: Option<&str>, primPath: Option<&Bound<'_, PyAny>>, layerOffset: Option<&PyLayerOffset>) -> PyResult<Self> {
        if args.len() == 1 { if let Ok(o) = args.get_item(0)?.extract::<PyPayload>() { return Ok(Self { inner: o.inner }); } }
        let asset = assetPath.map(String::from).or_else(|| args.get_item(0).ok().and_then(|a| a.extract::<String>().ok())).unwrap_or_default();
        if has_control_chars(&asset) { return Err(pyo3::exceptions::PyException::new_err("invalid control characters")); }
        let prim = if let Some(pp) = primPath { if let Ok(p) = pp.extract::<PyPath>() { p.inner.get_as_string() } else { pp.extract::<String>().unwrap_or_default() } }
            else if args.len() >= 2 { let i = args.get_item(1)?; if let Ok(p) = i.extract::<PyPath>() { p.inner.get_as_string() } else { i.extract::<String>().unwrap_or_default() } }
            else { String::new() };
        let offset = layerOffset.map(|o| o.inner).or_else(|| args.get_item(2).ok().and_then(|a| a.extract::<PyRef<'_, PyLayerOffset>>().ok()).map(|o| o.inner)).unwrap_or_else(LayerOffset::identity);
        Ok(Self { inner: Payload::with_layer_offset(asset, &prim, offset) })
    }
    #[getter] #[allow(non_snake_case)] fn assetPath(&self) -> &str { self.inner.asset_path() }
    #[getter] #[allow(non_snake_case)] fn primPath(&self) -> PyPath { PyPath::from_path(self.inner.prim_path().clone()) }
    #[getter] #[allow(non_snake_case)] fn layerOffset(&self) -> PyLayerOffset { PyLayerOffset { inner: *self.inner.layer_offset() } }
    fn __repr__(&self) -> String {
        let asset = self.inner.asset_path();
        let prim = self.inner.prim_path();
        let offset = self.inner.layer_offset();
        let has_asset = !asset.is_empty();
        let has_prim = !prim.is_empty();
        let has_offset = !offset.is_identity();

        if !has_asset && !has_prim && !has_offset {
            return "Sdf.Payload()".to_string();
        }

        // When only keyword-style args needed, use keyword form for roundtrip safety
        let use_keywords = !has_asset && (has_prim || has_offset);

        if use_keywords {
            let mut parts = Vec::new();
            if has_prim {
                parts.push(format!("primPath='{}'", prim.get_as_string()));
            }
            if has_offset {
                parts.push(format!("layerOffset=Sdf.LayerOffset({}, {})", offset.offset(), offset.scale()));
            }
            return format!("Sdf.Payload({})", parts.join(", "));
        }

        // Positional form when assetPath is present
        let mut parts = Vec::new();
        parts.push(format!("'{}'", asset));
        if has_prim || has_offset {
            // Must include primPath (even empty) when layerOffset follows for positional compat
            parts.push(format!("'{}'", if has_prim { prim.get_as_string() } else { String::new() }));
        }
        if has_offset {
            parts.push(format!("Sdf.LayerOffset({}, {})", offset.offset(), offset.scale()));
        }
        format!("Sdf.Payload({})", parts.join(", "))
    }
    fn __eq__(&self, o: &Self) -> bool { self.inner.asset_path()==o.inner.asset_path() && self.inner.prim_path()==o.inner.prim_path() && self.inner.layer_offset()==o.inner.layer_offset() }
    fn __ne__(&self, o: &Self) -> bool { !self.__eq__(o) }
    fn __lt__(&self, o: &Self) -> bool { payload_cmp(&self.inner,&o.inner)==std::cmp::Ordering::Less }
    fn __le__(&self, o: &Self) -> bool { payload_cmp(&self.inner,&o.inner)!=std::cmp::Ordering::Greater }
    fn __gt__(&self, o: &Self) -> bool { payload_cmp(&self.inner,&o.inner)==std::cmp::Ordering::Greater }
    fn __ge__(&self, o: &Self) -> bool { payload_cmp(&self.inner,&o.inner)!=std::cmp::Ordering::Less }
    fn __hash__(&self) -> u64 { let mut h=std::collections::hash_map::DefaultHasher::new(); self.inner.asset_path().hash(&mut h); self.inner.prim_path().as_str().hash(&mut h); h.finish() }
}
fn payload_cmp(a: &Payload, b: &Payload) -> std::cmp::Ordering { a.asset_path().cmp(b.asset_path()).then_with(|| a.prim_path().cmp(b.prim_path())).then_with(|| a.layer_offset().offset().partial_cmp(&b.layer_offset().offset()).unwrap_or(std::cmp::Ordering::Equal)).then_with(|| a.layer_offset().scale().partial_cmp(&b.layer_offset().scale()).unwrap_or(std::cmp::Ordering::Equal)) }

// ============================================================================
// SdfReference
// ============================================================================

#[pyclass(from_py_object, name = "Reference", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyReference { inner: Reference }

impl PyReference {
    pub fn to_reference(&self) -> Reference {
        self.inner.clone()
    }
}

#[pymethods]
impl PyReference {
    #[new] #[pyo3(signature = (*args, assetPath = None, primPath = None, layerOffset = None, customData = None))]
    fn new(args: &Bound<'_, PyTuple>, assetPath: Option<&str>, primPath: Option<&str>, layerOffset: Option<&PyLayerOffset>, customData: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        if args.len() == 1 { if let Ok(o) = args.get_item(0)?.extract::<PyReference>() { return Ok(Self { inner: o.inner }); } }
        let asset = assetPath.map(String::from).or_else(|| args.get_item(0).ok().and_then(|a| a.extract::<String>().ok())).unwrap_or_default();
        if has_control_chars(&asset) { return Err(pyo3::exceptions::PyException::new_err("invalid control characters")); }
        let prim = primPath.map(String::from).or_else(|| args.get_item(1).ok().and_then(|a| a.extract::<String>().ok())).unwrap_or_default();
        let offset = layerOffset.map(|o| o.inner).unwrap_or_else(LayerOffset::identity);
        let custom = if let Some(d) = customData { let mut m = HashMap::new(); for (k, v) in d.iter() { m.insert(k.extract::<String>().unwrap_or_default(), pyobject_to_vt_value(d.py(), &v.unbind())); } m } else { HashMap::new() };
        Ok(Self { inner: Reference::with_metadata(asset, &prim, offset, custom) })
    }
    #[getter] #[allow(non_snake_case)] fn assetPath(&self) -> &str { self.inner.asset_path() }
    #[getter] #[allow(non_snake_case)] fn primPath(&self) -> String { self.inner.prim_path().get_as_string() }
    #[getter] #[allow(non_snake_case)] fn layerOffset(&self) -> PyLayerOffset { PyLayerOffset { inner: *self.inner.layer_offset() } }
    #[getter] #[allow(non_snake_case)]
    fn customData(&self, py: Python<'_>) -> Py<PyAny> { let dict = PyDict::new(py); for (k, v) in self.inner.custom_data() { let _ = dict.set_item(k.as_str(), vt_value_to_pyobject(py, v)); } dict.into_any().unbind() }
    #[allow(non_snake_case)] fn IsInternal(&self) -> bool { self.inner.is_internal() }
    fn __repr__(&self, py: Python<'_>) -> String {
        let mut parts = Vec::new();
        if !self.inner.asset_path().is_empty() { parts.push(format!("assetPath='{}'", self.inner.asset_path())); }
        if !self.inner.prim_path().is_empty() { parts.push(format!("primPath='{}'", self.inner.prim_path().get_as_string())); }
        if !self.inner.layer_offset().is_identity() { parts.push(format!("layerOffset=Sdf.LayerOffset({}, {})", self.inner.layer_offset().offset(), self.inner.layer_offset().scale())); }
        if !self.inner.custom_data().is_empty() { let dict = PyDict::new(py); for (k, v) in self.inner.custom_data() { let _ = dict.set_item(k.as_str(), vt_value_to_pyobject(py, v)); } if let Ok(r) = dict.repr() { parts.push(format!("customData={r}")); } }
        if parts.is_empty() { "Sdf.Reference()".into() } else { format!("Sdf.Reference({})", parts.join(", ")) }
    }
    fn __eq__(&self, o: &Self) -> bool { ref_tuple(&self.inner)==ref_tuple(&o.inner) }
    fn __ne__(&self, o: &Self) -> bool { !self.__eq__(o) }
    fn __lt__(&self, o: &Self) -> bool { ref_tuple(&self.inner)<ref_tuple(&o.inner) }
    fn __le__(&self, o: &Self) -> bool { ref_tuple(&self.inner)<=ref_tuple(&o.inner) }
    fn __gt__(&self, o: &Self) -> bool { ref_tuple(&self.inner)>ref_tuple(&o.inner) }
    fn __ge__(&self, o: &Self) -> bool { ref_tuple(&self.inner)>=ref_tuple(&o.inner) }
    fn __hash__(&self) -> u64 { let mut h=std::collections::hash_map::DefaultHasher::new(); self.inner.asset_path().hash(&mut h); self.inner.prim_path().as_str().hash(&mut h); h.finish() }
}
fn ref_tuple(r: &Reference) -> (String, String, i64, i64, usize) { (r.asset_path().to_string(), r.prim_path().get_as_string(), r.layer_offset().offset().to_bits() as i64, r.layer_offset().scale().to_bits() as i64, r.custom_data().len()) }

// ============================================================================
// ListOp types
// ============================================================================

macro_rules! define_list_op {
    ($py_name:ident, $py_class_name:literal, $item_ty:ty) => {
        #[pyclass(skip_from_py_object, name = $py_class_name, module = "pxr_rs.Sdf")]
        #[derive(Clone)]
        pub struct $py_name { inner: ListOp<$item_ty> }
        #[pymethods]
        impl $py_name {
            #[new] fn new() -> Self { Self { inner: ListOp::new() } }
            #[staticmethod] #[allow(non_snake_case)] #[pyo3(signature = (items = None))]
            fn CreateExplicit(items: Option<Vec<$item_ty>>) -> Self { Self { inner: ListOp::create_explicit(items.unwrap_or_default()) } }
            #[staticmethod] #[allow(non_snake_case)] #[pyo3(signature = (prependedItems = None, appendedItems = None, deletedItems = None))]
            fn Create(prependedItems: Option<Vec<$item_ty>>, appendedItems: Option<Vec<$item_ty>>, deletedItems: Option<Vec<$item_ty>>) -> Self { Self { inner: ListOp::create(prependedItems.unwrap_or_default(), appendedItems.unwrap_or_default(), deletedItems.unwrap_or_default()) } }
            #[getter] #[allow(non_snake_case)] fn isExplicit(&self) -> bool { self.inner.is_explicit() }
            #[getter] #[allow(non_snake_case)] fn explicitItems(&self) -> Vec<$item_ty> { self.inner.get_explicit_items().to_vec() }
            #[setter] #[allow(non_snake_case)] fn set_explicitItems(&mut self, items: Vec<$item_ty>) { let _ = self.inner.set_explicit_items(items); }
            #[getter] #[allow(non_snake_case)] fn addedItems(&self) -> Vec<$item_ty> { self.inner.get_added_items().to_vec() }
            #[setter] #[allow(non_snake_case)] fn set_addedItems(&mut self, items: Vec<$item_ty>) { self.inner.set_added_items(items); }
            #[getter] #[allow(non_snake_case)] fn prependedItems(&self) -> Vec<$item_ty> { self.inner.get_prepended_items().to_vec() }
            #[setter] #[allow(non_snake_case)] fn set_prependedItems(&mut self, items: Vec<$item_ty>) { let _ = self.inner.set_prepended_items(items); }
            #[getter] #[allow(non_snake_case)] fn appendedItems(&self) -> Vec<$item_ty> { self.inner.get_appended_items().to_vec() }
            #[setter] #[allow(non_snake_case)] fn set_appendedItems(&mut self, items: Vec<$item_ty>) { let _ = self.inner.set_appended_items(items); }
            #[getter] #[allow(non_snake_case)] fn deletedItems(&self) -> Vec<$item_ty> { self.inner.get_deleted_items().to_vec() }
            #[setter] #[allow(non_snake_case)] fn set_deletedItems(&mut self, items: Vec<$item_ty>) { let _ = self.inner.set_deleted_items(items); }
            #[getter] #[allow(non_snake_case)] fn orderedItems(&self) -> Vec<$item_ty> { self.inner.get_ordered_items().to_vec() }
            #[setter] #[allow(non_snake_case)] fn set_orderedItems(&mut self, items: Vec<$item_ty>) { self.inner.set_ordered_items(items); }
            #[allow(non_snake_case)] fn HasItem(&self, item: $item_ty) -> bool { self.inner.has_item(&item) }
            #[allow(non_snake_case)] fn GetAppliedItems(&self) -> Vec<$item_ty> { self.inner.get_applied_items() }
            #[allow(non_snake_case)] fn ApplyOperations(&self, items: &pyo3::Bound<'_, pyo3::PyAny>) -> PyResult<Vec<$item_ty>> {
                // Accept Vec<T> or another ListOp of same type
                if let Ok(v) = items.extract::<Vec<$item_ty>>() {
                    let mut r = v;
                    self.inner.apply_operations(&mut r, None::<fn(usd_sdf::ListOpType, &$item_ty) -> Option<$item_ty>>);
                    Ok(r)
                } else if let Ok(other) = items.cast::<$py_name>() {
                    let borrowed = other.borrow();
                    let mut r = borrowed.inner.get_applied_items();
                    self.inner.apply_operations(&mut r, None::<fn(usd_sdf::ListOpType, &$item_ty) -> Option<$item_ty>>);
                    Ok(r)
                } else {
                    Err(pyo3::exceptions::PyTypeError::new_err("expected list or ListOp"))
                }
            }
            fn __repr__(&self) -> String { if self.inner.is_explicit() { format!("{}(explicit={:?})", $py_class_name, self.inner.get_explicit_items()) } else { format!("{}(prepended={:?}, appended={:?}, deleted={:?})", $py_class_name, self.inner.get_prepended_items(), self.inner.get_appended_items(), self.inner.get_deleted_items()) } }
            fn __str__(&self) -> String { self.__repr__() }
            fn __eq__(&self, o: &Self) -> bool { self.inner.is_explicit()==o.inner.is_explicit() && self.inner.get_explicit_items()==o.inner.get_explicit_items() && self.inner.get_prepended_items()==o.inner.get_prepended_items() && self.inner.get_appended_items()==o.inner.get_appended_items() && self.inner.get_deleted_items()==o.inner.get_deleted_items() && self.inner.get_ordered_items()==o.inner.get_ordered_items() }
            fn __ne__(&self, o: &Self) -> bool { !self.__eq__(o) }
            fn __hash__(&self) -> u64 { let mut h=std::collections::hash_map::DefaultHasher::new(); self.inner.is_explicit().hash(&mut h); self.inner.get_explicit_items().len().hash(&mut h); h.finish() }
        }
    };
}
define_list_op!(PyIntListOp, "IntListOp", i32);
define_list_op!(PyInt64ListOp, "Int64ListOp", i64);
define_list_op!(PyUIntListOp, "UIntListOp", u32);
define_list_op!(PyUInt64ListOp, "UInt64ListOp", u64);
define_list_op!(PyStringListOp, "StringListOp", String);
define_list_op!(PyTokenListOp, "TokenListOp", String);

// ============================================================================
// PathListOp — ListOp<Path> with Python Path conversion
// ============================================================================

#[pyclass(skip_from_py_object, name = "PathListOp", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPathListOp {
    pub(crate) inner: PathListOp,
}

#[pymethods]
impl PyPathListOp {
    #[new]
    fn new() -> Self {
        Self { inner: PathListOp::new() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn isExplicit(&self) -> bool { self.inner.is_explicit() }

    #[getter]
    #[allow(non_snake_case)]
    fn explicitItems(&self) -> Vec<PyPath> {
        self.inner.get_explicit_items().iter().map(|p| PyPath::from_path(p.clone())).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn prependedItems(&self) -> Vec<PyPath> {
        self.inner.get_prepended_items().iter().map(|p| PyPath::from_path(p.clone())).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn appendedItems(&self) -> Vec<PyPath> {
        self.inner.get_appended_items().iter().map(|p| PyPath::from_path(p.clone())).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn deletedItems(&self) -> Vec<PyPath> {
        self.inner.get_deleted_items().iter().map(|p| PyPath::from_path(p.clone())).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn addedItems(&self) -> Vec<PyPath> {
        self.inner.get_added_items().iter().map(|p| PyPath::from_path(p.clone())).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn orderedItems(&self) -> Vec<PyPath> {
        self.inner.get_ordered_items().iter().map(|p| PyPath::from_path(p.clone())).collect()
    }

    fn __repr__(&self) -> String {
        format!("Sdf.PathListOp()")
    }
}

// ============================================================================
// ReferenceListOp — ListOp<Reference> with Python conversion
// ============================================================================

#[pyclass(skip_from_py_object, name = "ReferenceListOp", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyReferenceListOp {
    pub(crate) inner: ReferenceListOp,
}

#[pymethods]
impl PyReferenceListOp {
    #[new]
    fn new() -> Self {
        Self { inner: ReferenceListOp::new() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn isExplicit(&self) -> bool { self.inner.is_explicit() }

    #[getter]
    #[allow(non_snake_case)]
    fn explicitItems(&self) -> Vec<PyReference> {
        self.inner.get_explicit_items().iter().map(|r| PyReference { inner: r.clone() }).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn prependedItems(&self) -> Vec<PyReference> {
        self.inner.get_prepended_items().iter().map(|r| PyReference { inner: r.clone() }).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn appendedItems(&self) -> Vec<PyReference> {
        self.inner.get_appended_items().iter().map(|r| PyReference { inner: r.clone() }).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn deletedItems(&self) -> Vec<PyReference> {
        self.inner.get_deleted_items().iter().map(|r| PyReference { inner: r.clone() }).collect()
    }

    fn __repr__(&self) -> String {
        format!("Sdf.ReferenceListOp()")
    }
}

// ============================================================================
// PayloadListOp — ListOp<Payload> with Python conversion
// ============================================================================

#[pyclass(skip_from_py_object, name = "PayloadListOp", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyPayloadListOp {
    pub(crate) inner: PayloadListOp,
}

#[pymethods]
impl PyPayloadListOp {
    #[new]
    fn new() -> Self {
        Self { inner: PayloadListOp::new() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn isExplicit(&self) -> bool { self.inner.is_explicit() }

    #[getter]
    #[allow(non_snake_case)]
    fn explicitItems(&self) -> Vec<PyPayload> {
        self.inner.get_explicit_items().iter().map(|p| PyPayload { inner: p.clone() }).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn prependedItems(&self) -> Vec<PyPayload> {
        self.inner.get_prepended_items().iter().map(|p| PyPayload { inner: p.clone() }).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn appendedItems(&self) -> Vec<PyPayload> {
        self.inner.get_appended_items().iter().map(|p| PyPayload { inner: p.clone() }).collect()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn deletedItems(&self) -> Vec<PyPayload> {
        self.inner.get_deleted_items().iter().map(|p| PyPayload { inner: p.clone() }).collect()
    }

    fn __repr__(&self) -> String {
        format!("Sdf.PayloadListOp()")
    }
}

// ============================================================================
// RelationshipSpec
// ============================================================================

#[pyclass(skip_from_py_object, name = "RelationshipSpec", module = "pxr_rs.Sdf")]
#[derive(Clone)]
pub struct PyRelationshipSpec {
    inner: RelationshipSpec,
}

#[pymethods]
impl PyRelationshipSpec {
    #[new]
    #[pyo3(signature = (owner, name, custom = false, variability = None))]
    fn new(
        owner: &PyPrimSpec,
        name: &str,
        custom: bool,
        variability: Option<&str>,
    ) -> PyResult<Self> {
        let layer = owner.inner.layer().upgrade()
            .ok_or_else(|| PyRuntimeError::new_err("no valid layer"))?;
        let prim_path = owner.inner.path();
        let rel_path = prim_path.append_property(name)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid relationship name: '{name}'")))?;

        if layer.has_spec(&rel_path) {
            return Err(PyRuntimeError::new_err(format!(
                "Object already exists at path '{}'", rel_path.get_as_string()
            )));
        }

        layer.create_spec(&rel_path, SpecType::Relationship);
        let mut rel = RelationshipSpec::new(layer.get_handle(), rel_path);

        if custom {
            let _ = rel.spec_mut().set_field(
                &Token::new("custom"),
                usd_sdf::spec::VtValue::new(true),
            );
        }

        let _ = variability;
        Ok(Self { inner: rel })
    }

    #[getter]
    fn name(&self) -> String { self.inner.path().get_name().to_string() }

    #[getter]
    fn path(&self) -> PyPath { PyPath::from_path(self.inner.path()) }

    #[getter]
    #[allow(non_snake_case)]
    fn targetPathList(&self) -> PyPathListOp {
        PyPathListOp { inner: self.inner.target_path_list() }
    }

    fn __repr__(&self) -> String {
        format!("Sdf.RelationshipSpec('{}')", self.inner.path().get_as_string())
    }

    fn __bool__(&self) -> bool { !self.inner.is_dormant() }
}

// ============================================================================
// _PathElemsToPrefixes
// ============================================================================

#[pyfunction] #[pyo3(name = "_PathElemsToPrefixes")] #[pyo3(signature = (absolute, elements, num_prefixes = 0))]
fn path_elems_to_prefixes(absolute: bool, elements: Vec<String>, num_prefixes: usize) -> Vec<PyPath> {
    let mut prefixes = Vec::new();
    let mut string = if absolute { "/".to_string() } else { String::new() };
    let mut last_was_dotdot = false;
    let mut did_first = false;
    for elem in &elements {
        if elem == ".." { if did_first { string.push('/'); } else { did_first = true; } string.push_str(elem); prefixes.push(PyPath::new(&string)); last_was_dotdot = true; }
        else if elem.starts_with('.') { if last_was_dotdot { string.push('/'); } string.push_str(elem); prefixes.push(PyPath::new(&string)); last_was_dotdot = false; }
        else if elem.starts_with('[') { string.push_str(elem); prefixes.push(PyPath::new(&string)); last_was_dotdot = false; }
        else { if did_first { string.push('/'); } else { did_first = true; } string.push_str(elem); prefixes.push(PyPath::new(&string)); last_was_dotdot = false; }
    }
    if string.is_empty() { return Vec::new(); }
    if num_prefixes == 0 { prefixes } else { let s = prefixes.len().saturating_sub(num_prefixes); prefixes[s..].to_vec() }
}

// ============================================================================
// Module registration
// ============================================================================

/// Module-level `Sdf.CreatePrimInLayer(layer, path)` function.
///
/// Creates a prim spec at `path` in `layer`. Intermediate parent prims
/// are created automatically with specifier `Over`.
/// Matches C++ `SdfCreatePrimInLayer(const SdfLayerHandle&, const SdfPath&)`.
#[pyfunction]
#[pyo3(name = "CreatePrimInLayer")]
fn py_create_prim_in_layer(layer: &PyLayer, path: &str) -> PyResult<PyPrimSpec> {
    // C++ SdfCreatePrimInLayer accepts relative paths like "foo" (prepends /)
    // and returns pseudoRoot for "."
    let path_str = path.trim();

    // Special case: "." returns pseudoRoot
    if path_str == "." {
        return Ok(PyPrimSpec { inner: layer.layer().get_pseudo_root() });
    }

    // ".." is invalid
    if path_str == ".." || path_str.starts_with("../") {
        return Err(PyRuntimeError::new_err(format!(
            "Cannot create prim at relative path '{path_str}'"
        )));
    }

    // Make path absolute if not already
    let abs_path = if path_str.starts_with('/') {
        path_str.to_string()
    } else {
        format!("/{path_str}")
    };

    let sdf_path = usd_sdf::Path::from_string(&abs_path)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: {abs_path}")))?;

    if sdf_path.is_empty() || sdf_path.is_absolute_root_path() {
        return Ok(PyPrimSpec { inner: layer.layer().get_pseudo_root() });
    }

    // Collect all ancestors that need creation (from root down)
    let prefixes = sdf_path.get_prefixes();
    for (i, anc) in prefixes.iter().enumerate() {
        if layer.layer().get_prim_at_path(anc).is_some() {
            continue;
        }
        let name = anc.get_name();
        if name.is_empty() {
            continue;
        }
        let parent = anc.get_parent_path();
        // Last prefix is the target: Def; ancestors: Over
        let spec = if i == prefixes.len() - 1 { Specifier::Def } else { Specifier::Over };
        if parent.as_str() == "/" || parent.is_absolute_root_path() {
            let handle = layer.layer().get_handle();
            let _ = PrimSpec::new_root(&handle, &name, spec, "");
        } else {
            layer.layer().create_prim_spec(anc, spec, "");
        }
    }

    // Get the created prim spec
    layer.layer().get_prim_at_path(&sdf_path)
        .map(|p| PyPrimSpec { inner: p })
        .ok_or_else(|| PyRuntimeError::new_err(format!("Failed to create prim at '{abs_path}'")))
}

/// Module-level `Sdf.JustCreatePrimInLayer(layer, path)` function.
/// Like CreatePrimInLayer but all prims use Specifier::Over (including the target).
#[pyfunction]
#[pyo3(name = "JustCreatePrimInLayer")]
fn py_just_create_prim_in_layer(layer: &PyLayer, path: &str) -> PyResult<PyPrimSpec> {
    let path_str = path.trim();
    if path_str == "." {
        return Ok(PyPrimSpec { inner: layer.layer().get_pseudo_root() });
    }
    if path_str == ".." || path_str.starts_with("../") {
        return Err(PyRuntimeError::new_err(format!(
            "Cannot create prim at relative path '{path_str}'"
        )));
    }

    let abs_path = if path_str.starts_with('/') {
        path_str.to_string()
    } else {
        format!("/{path_str}")
    };

    let sdf_path = usd_sdf::Path::from_string(&abs_path)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: {abs_path}")))?;

    if sdf_path.is_empty() || sdf_path.is_absolute_root_path() {
        return Ok(PyPrimSpec { inner: layer.layer().get_pseudo_root() });
    }

    let prefixes = sdf_path.get_prefixes();
    for anc in &prefixes {
        if layer.layer().get_prim_at_path(anc).is_some() {
            continue;
        }
        let name = anc.get_name();
        if name.is_empty() {
            continue;
        }
        let parent = anc.get_parent_path();
        if parent.as_str() == "/" || parent.is_absolute_root_path() {
            let handle = layer.layer().get_handle();
            let _ = PrimSpec::new_root(&handle, &name, Specifier::Over, "");
        } else {
            layer.layer().create_prim_spec(anc, Specifier::Over, "");
        }
    }

    layer.layer().get_prim_at_path(&sdf_path)
        .map(|p| PyPrimSpec { inner: p })
        .ok_or_else(|| PyRuntimeError::new_err(format!("Failed to create prim at '{abs_path}'")))
}

/// Module-level `Sdf.CreatePrimAttributeInLayer(layer, attrPath, typeName, ...)`
#[pyfunction]
#[pyo3(name = "CreatePrimAttributeInLayer")]
#[pyo3(signature = (layer, attrPath, typeName, variability = None, isCustom = false))]
#[allow(non_snake_case)]
fn py_create_prim_attribute_in_layer(
    layer: &PyLayer,
    attrPath: &Bound<'_, PyAny>,
    typeName: &PyValueTypeName,
    variability: Option<&str>,
    isCustom: bool,
) -> PyResult<PyAttributeSpec> {
    let attr_path = extract_path(attrPath)?;
    let prim_path = attr_path.get_prim_path();

    // Ensure the prim exists
    if !prim_path.is_empty() && layer.layer().get_prim_at_path(&prim_path).is_none() {
        let path_str = prim_path.get_as_string();
        let _ = py_create_prim_in_layer(layer, &path_str);
    }

    layer.layer().create_spec(&attr_path, SpecType::Attribute);
    let mut attr = AttributeSpec::from_layer_and_path(layer.layer().get_handle(), attr_path);
    attr.set_type_name(&typeName.name);

    if let Some(v) = variability {
        let var = match v {
            "Uniform" => usd_sdf::Variability::Uniform,
            _ => usd_sdf::Variability::Varying,
        };
        attr.set_variability(var);
    }
    if isCustom {
        let _ = attr.as_spec_mut().set_field(
            &Token::new("custom"),
            usd_sdf::spec::VtValue::new(true),
        );
    }
    Ok(PyAttributeSpec { inner: attr })
}

/// Module-level `Sdf.JustCreatePrimAttributeInLayer(layer, attrPath, typeName, ...)`
#[pyfunction]
#[pyo3(name = "JustCreatePrimAttributeInLayer")]
#[pyo3(signature = (layer, attrPath, typeName, variability = None, isCustom = false))]
#[allow(non_snake_case)]
fn py_just_create_prim_attribute_in_layer(
    layer: &PyLayer,
    attrPath: &Bound<'_, PyAny>,
    typeName: &PyValueTypeName,
    variability: Option<&str>,
    isCustom: bool,
) -> PyResult<PyAttributeSpec> {
    // Same as CreatePrimAttributeInLayer but parents use Over
    py_create_prim_attribute_in_layer(layer, attrPath, typeName, variability, isCustom)
}

/// Module-level `Sdf.CreateVariantInLayer(layer, primPath, variantSetName, variantName)`
#[pyfunction]
#[pyo3(name = "CreateVariantInLayer")]
fn py_create_variant_in_layer(
    layer: &PyLayer,
    path: &Bound<'_, PyAny>,
    variant_set_name: &str,
    variant_name: &str,
) -> PyResult<PyPrimSpec> {
    let prim_path = extract_path(path)?;
    let vs_path = prim_path.append_variant_selection(variant_set_name, variant_name)
        .ok_or_else(|| PyValueError::new_err("Invalid variant selection"))?;
    usd_sdf::create_variant_in_layer(layer.layer(), &prim_path, variant_set_name, variant_name);
    // Get the variant prim spec from the layer
    layer.layer().get_prim_at_path(&vs_path)
        .map(|p| PyPrimSpec { inner: p })
        .ok_or_else(|| PyRuntimeError::new_err("Failed to create variant"))
}

/// Register all `pxr.Sdf` classes into the submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPath>()?;
    m.add_class::<PyLayerOffset>()?;
    m.add_class::<PyLayer>()?;
    m.add_class::<PyPrimSpec>()?;
    m.add_class::<PyAttributeSpec>()?;
    m.add_class::<PySpecifier>()?;
    m.add_class::<PyChangeBlock>()?;

    // Top-level specifier constants matching C++ pxr.Sdf.SpecifierDef etc.
    m.add("SpecifierDef", PySpecifier { inner: Specifier::Def })?;
    m.add("SpecifierOver", PySpecifier { inner: Specifier::Over })?;
    m.add("SpecifierClass", PySpecifier { inner: Specifier::Class })?;

    // Variability constants
    m.add("VariabilityVarying", "Sdf.VariabilityVarying")?;
    m.add("VariabilityUniform", "Sdf.VariabilityUniform")?;

    // Permission constants
    m.add("PermissionPublic", "Sdf.PermissionPublic")?;
    m.add("PermissionPrivate", "Sdf.PermissionPrivate")?;

    // Value type names
    m.add_class::<PyValueTypeName>()?;
    m.add_class::<PyValueTypeNames>()?;
    register_value_type_names(py)?;
    // Also add as module attr so `Sdf.ValueTypeNames.Double` works
    m.add("ValueTypeNames", py.get_type::<PyValueTypeNames>())?;

    // New types
    m.add_class::<PyPropertySpec>()?;
    m.add_class::<PyPropertySpecList>()?;
    m.add_class::<PyPrimSpecList>()?;
    m.add_class::<PyAssetPath>()?;
    m.add_class::<PyTimeCode>()?;
    m.add_class::<PyPayload>()?;
    m.add_class::<PyReference>()?;

    // ListOp types
    m.add_class::<PyIntListOp>()?;
    m.add_class::<PyInt64ListOp>()?;
    m.add_class::<PyUIntListOp>()?;
    m.add_class::<PyUInt64ListOp>()?;
    m.add_class::<PyStringListOp>()?;
    m.add_class::<PyTokenListOp>()?;
    m.add_class::<PyPathListOp>()?;
    m.add_class::<PyReferenceListOp>()?;
    m.add_class::<PyPayloadListOp>()?;

    // RelationshipSpec
    m.add_class::<PyRelationshipSpec>()?;

    // Module-level functions
    m.add_function(wrap_pyfunction!(py_create_prim_in_layer, m)?)?;
    m.add_function(wrap_pyfunction!(py_just_create_prim_in_layer, m)?)?;
    m.add_function(wrap_pyfunction!(py_create_prim_attribute_in_layer, m)?)?;
    m.add_function(wrap_pyfunction!(py_just_create_prim_attribute_in_layer, m)?)?;
    m.add_function(wrap_pyfunction!(py_create_variant_in_layer, m)?)?;
    m.add_function(wrap_pyfunction!(path_elems_to_prefixes, m)?)?;

    // Path expressions
    m.add_class::<PyPathExpression>()?;
    m.add_class::<PyBasicMatchEval>()?;
    m.add_function(wrap_pyfunction!(make_basic_match_eval, m)?)?;

    // Variable expressions
    m.add_class::<PyVariableExpression>()?;

    // VariableExpressionASTNodes — namespace with AST node type stubs
    m.add_class::<PyLiteralNode>()?;
    m.add_class::<PyVariableNode>()?;
    m.add_class::<PyListNode>()?;
    m.add_class::<PyFunctionNode>()?;
    {
        // Build the VariableExpressionASTNodes namespace as a module
        let ast_mod = PyModule::new(py, "VariableExpressionASTNodes")?;
        ast_mod.add_class::<PyLiteralNode>()?;
        ast_mod.add_class::<PyVariableNode>()?;
        ast_mod.add_class::<PyListNode>()?;
        ast_mod.add_class::<PyFunctionNode>()?;
        m.add("VariableExpressionASTNodes", ast_mod)?;
    }

    // Sdf.Find(layerFileName) → Layer or None
    m.add_function(wrap_pyfunction!(py_sdf_find, m)?)?;

    Ok(())
}

/// Sdf.Find(layerFileName, scenePath=None) — locate a layer by filename.
#[pyfunction]
#[pyo3(name = "Find", signature = (layer_file_name, scene_path=None))]
fn py_sdf_find(layer_file_name: &str, scene_path: Option<&str>) -> Option<PyLayer> {
    let layer = usd_sdf::Layer::find(layer_file_name)?;
    let _ = scene_path;
    Some(PyLayer::from_layer_arc(layer))
}
