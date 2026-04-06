//! pxr.Sdf — Scene Description Foundation bindings.
//!
//! Drop-in replacement for `pxr.Sdf` from C++ OpenUSD.
//! Mirrors wrapPath.cpp and wrapLayer.cpp from the reference implementation.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use usd_sdf::{
    AttributeSpec, Layer, LayerOffset, Path, PrimSpec, Specifier,
};
use usd_sdf::path as sdf_path_fns;
use usd_tf::Token;

// ============================================================================
// SdfPath
// ============================================================================

/// Path addressing a location in a USD scene graph.
///
/// Wraps `usd_sdf::Path`. Mirrors C++ `SdfPath` as exposed by `wrapPath.cpp`.
#[pyclass(name = "Path", module = "pxr.Sdf")]
#[derive(Clone)]
pub struct PyPath {
    inner: Path,
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

    #[new]
    #[pyo3(signature = (path = ""))]
    fn new(path: &str) -> PyResult<Self> {
        if path.is_empty() {
            return Ok(Self { inner: Path::empty() });
        }
        Path::from_string(path)
            .map(|p| Self { inner: p })
            .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{path}'")))
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
    fn name(&self) -> &str {
        // Lifetime is tied to inner path_string — return static via get_name()
        // We must return owned here for pyo3 safety.
        // SAFETY: get_name() borrows path_string which lives in Self.
        // pyo3 requires 'static or owned, so we return owned String here via __str__ impl.
        // We return &str with #[getter] only when pyo3 can bridge it (it can with &str).
        self.inner.get_name()
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
    fn GetVariantSelection(&self, py: Python<'_>) -> PyObject {
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
    fn RemoveCommonSuffix(&self, other: &PyPath, stop_at_root_prim: bool, py: Python<'_>) -> PyObject {
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
    fn StripPrefixNamespace(name: &str, prefix: &str, py: Python<'_>) -> PyObject {
        let (stripped, had_prefix) = Path::strip_prefix_namespace(name, prefix);
        let s_obj: PyObject = stripped.into_pyobject(py).expect("ok").into_any().unbind();
        let b_obj: PyObject = had_prefix.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
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

    fn __eq__(&self, other: &PyPath) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &PyPath) -> bool {
        self.inner != other.inner
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
#[pyclass(name = "LayerOffset", module = "pxr.Sdf")]
#[derive(Clone)]
pub struct PyLayerOffset {
    inner: LayerOffset,
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

    fn __mul__(&self, other: &PyLayerOffset) -> Self {
        Self { inner: self.inner * other.inner }
    }
}

// ============================================================================
// SdfLayer
// ============================================================================

/// Container for scene description (prims, properties, metadata).
///
/// Wraps `Arc<usd_sdf::Layer>`. Mirrors C++ `SdfLayer` as exposed by `wrapLayer.cpp`.
#[pyclass(name = "Layer", module = "pxr.Sdf")]
#[derive(Clone)]
pub struct PyLayer {
    inner: Arc<Layer>,
}

impl PyLayer {
    pub fn layer(&self) -> &Arc<Layer> {
        &self.inner
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
    fn rootPrims(&self) -> Vec<PyPrimSpec> {
        self.inner.get_root_prims()
            .into_iter()
            .map(|p| PyPrimSpec { inner: p })
            .collect()
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
    fn customLayerData(&self, py: Python<'_>) -> PyObject {
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
    fn GetPrimAtPath(&self, path: &PyPath) -> Option<PyPrimSpec> {
        self.inner.get_prim_at_path(&path.inner)
            .map(|p| PyPrimSpec { inner: p })
    }

    /// Returns the property spec at `path`, or None.
    #[allow(non_snake_case)]
    fn GetPropertyAtPath(&self, path: &PyPath) -> Option<PyAttributeSpec> {
        self.inner.get_attribute_at_path(&path.inner)
            .map(|a| PyAttributeSpec { inner: a })
    }

    /// Returns true if a spec exists at `path`.
    #[allow(non_snake_case)]
    fn HasSpec(&self, path: &PyPath) -> bool {
        self.inner.has_spec(&path.inner)
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
    fn ListTimeSamplesForPath(&self, path: &PyPath) -> Vec<f64> {
        self.inner.list_time_samples_for_path(&path.inner)
    }

    #[allow(non_snake_case)]
    fn GetNumTimeSamplesForPath(&self, path: &PyPath) -> usize {
        self.inner.get_num_time_samples_for_path(&path.inner)
    }

    /// Returns the value at `time` for the attribute at `path`, or None.
    #[allow(non_snake_case)]
    fn QueryTimeSample(&self, path: &PyPath, time: f64, py: Python<'_>) -> PyObject {
        match self.inner.query_time_sample(&path.inner, time) {
            Some(val) => vt_value_to_pyobject(py, &val),
            None => py.None(),
        }
    }

    /// Sets the time sample at `time` for the attribute at `path`.
    #[allow(non_snake_case)]
    fn SetTimeSample(&self, path: &PyPath, time: f64, value: PyObject, py: Python<'_>) -> bool {
        let val = pyobject_to_vt_value(py, &value);
        self.inner.set_time_sample(&path.inner, time, val)
    }

    /// Removes the time sample at `time` for the attribute at `path`.
    #[allow(non_snake_case)]
    fn EraseTimeSample(&self, path: &PyPath, time: f64) -> bool {
        self.inner.erase_time_sample(&path.inner, time)
    }

    /// Returns (found, lower, upper) bracketing the time samples around `time` for `path`.
    #[allow(non_snake_case)]
    fn GetBracketingTimeSamplesForPath(
        &self,
        path: &PyPath,
        time: f64,
        py: Python<'_>,
    ) -> PyObject {
        match self.inner.get_bracketing_time_samples_for_path(&path.inner, time) {
            Some((lo, hi)) => {
                let found: PyObject = true.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
                let lo_obj: PyObject = lo.into_pyobject(py).expect("ok").into_any().unbind();
                let hi_obj: PyObject = hi.into_pyobject(py).expect("ok").into_any().unbind();
                PyTuple::new(py, [found, lo_obj, hi_obj])
                    .expect("tuple")
                    .into_any()
                    .unbind()
            }
            None => {
                let found: PyObject = false.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
                let lo_obj: PyObject = 0.0_f64.into_pyobject(py).expect("ok").into_any().unbind();
                let hi_obj: PyObject = 0.0_f64.into_pyobject(py).expect("ok").into_any().unbind();
                PyTuple::new(py, [found, lo_obj, hi_obj])
                    .expect("tuple")
                    .into_any()
                    .unbind()
            }
        }
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
    fn SplitIdentifier(identifier: &str, py: Python<'_>) -> PyObject {
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

    // --- Python special methods ---------------------------------------------

    fn __repr__(&self) -> String {
        format!("Sdf.Find('{}')", self.inner.identifier())
    }

    fn __bool__(&self) -> bool {
        // Layer is always valid if it exists
        true
    }
}

// ============================================================================
// SdfPrimSpec
// ============================================================================

/// Scene description for a single prim in a layer.
///
/// Wraps `usd_sdf::PrimSpec`. Mirrors C++ `SdfPrimSpec`.
#[pyclass(name = "PrimSpec", module = "pxr.Sdf")]
#[derive(Clone)]
pub struct PyPrimSpec {
    inner: PrimSpec,
}

#[pymethods]
impl PyPrimSpec {
    // --- Constructors -------------------------------------------------------

    /// Creates a root prim under `layer`.
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

    /// Returns the child prims of this prim.
    #[getter]
    #[allow(non_snake_case)]
    fn nameChildren(&self) -> Vec<PyPrimSpec> {
        self.inner.name_children()
            .into_iter()
            .map(|p| PyPrimSpec { inner: p })
            .collect()
    }

    /// Returns the attributes on this prim.
    #[getter]
    fn attributes(&self) -> Vec<PyAttributeSpec> {
        self.inner.attributes()
            .into_iter()
            .map(|a| PyAttributeSpec { inner: a })
            .collect()
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
}

// ============================================================================
// SdfAttributeSpec
// ============================================================================

/// Scene description for an attribute in a layer.
///
/// Wraps `usd_sdf::AttributeSpec`. Mirrors C++ `SdfAttributeSpec`.
#[pyclass(name = "AttributeSpec", module = "pxr.Sdf")]
#[derive(Clone)]
pub struct PyAttributeSpec {
    inner: AttributeSpec,
}

#[pymethods]
impl PyAttributeSpec {
    #[getter]
    fn name(&self) -> String {
        // AttributeSpec has no dedicated name() — extract from path
        self.inner.path().get_name().to_string()
    }

    #[getter]
    fn path(&self) -> PyPath {
        PyPath { inner: self.inner.path() }
    }

    #[getter]
    #[allow(non_snake_case)]
    fn typeName(&self) -> String {
        self.inner.type_name()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn variability(&self) -> String {
        format!("{:?}", self.inner.variability())
    }

    #[getter]
    #[allow(non_snake_case)]
    fn hasDefaultValue(&self) -> bool {
        self.inner.has_default_value()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn default_(&self, py: Python<'_>) -> PyObject {
        // default_value() returns Value (not Option); check has_default_value() first
        if self.inner.has_default_value() {
            vt_value_to_pyobject(py, &self.inner.default_value())
        } else {
            py.None()
        }
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
}

// ============================================================================
// SdfSpecifier
// ============================================================================

/// Specifier for a prim: `Def`, `Over`, or `Class`.
///
/// Wraps `usd_sdf::Specifier`. Mirrors C++ `SdfSpecifier`.
#[pyclass(name = "Specifier", module = "pxr.Sdf")]
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
#[pyclass(name = "ChangeBlock", module = "pxr.Sdf")]
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

    fn __exit__(&mut self, _exc_type: PyObject, _exc_val: PyObject, _exc_tb: PyObject) -> bool {
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
fn vt_value_to_pyobject(py: Python<'_>, val: &usd_vt::Value) -> PyObject {
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
fn pyobject_to_vt_value(py: Python<'_>, obj: &PyObject) -> usd_vt::Value {
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
// Module registration
// ============================================================================

/// Register all `pxr.Sdf` classes into the submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
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

    Ok(())
}
