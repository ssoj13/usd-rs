//! pxr.Pcp — Prim Cache Population Python bindings.
//!
//! Drop-in replacement for `pxr.Pcp` from C++ OpenUSD.
//! Covers: Cache, PrimIndex, NodeRef, LayerStack, LayerStackIdentifier,
//! MapFunction, Site, ArcType, and related types.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use usd_pcp::{ArcType, Cache, LayerStack, LayerStackIdentifier, MapFunction, PrimIndex, Site};
use usd_sdf::Path;

// ============================================================================
// ArcType
// ============================================================================

/// Describes the type of arc connecting two nodes in the prim index.
///
/// Mirrors `pxr.Pcp.ArcType` / `PcpArcType` from C++ OpenUSD.
#[pyclass(skip_from_py_object, name = "ArcType", module = "pxr.Pcp")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyArcType {
    inner: ArcType,
}

#[pymethods]
impl PyArcType {
    #[classattr]
    #[pyo3(name = "ArcTypeRoot")]
    fn arc_root() -> Self {
        Self {
            inner: ArcType::Root,
        }
    }

    #[classattr]
    #[pyo3(name = "ArcTypeInherit")]
    fn arc_inherit() -> Self {
        Self {
            inner: ArcType::Inherit,
        }
    }

    #[classattr]
    #[pyo3(name = "ArcTypeVariant")]
    fn arc_variant() -> Self {
        Self {
            inner: ArcType::Variant,
        }
    }

    #[classattr]
    #[pyo3(name = "ArcTypeRelocate")]
    fn arc_relocate() -> Self {
        Self {
            inner: ArcType::Relocate,
        }
    }

    #[classattr]
    #[pyo3(name = "ArcTypeReference")]
    fn arc_reference() -> Self {
        Self {
            inner: ArcType::Reference,
        }
    }

    #[classattr]
    #[pyo3(name = "ArcTypePayload")]
    fn arc_payload() -> Self {
        Self {
            inner: ArcType::Payload,
        }
    }

    #[classattr]
    #[pyo3(name = "ArcTypeSpecialize")]
    fn arc_specialize() -> Self {
        Self {
            inner: ArcType::Specialize,
        }
    }

    fn __repr__(&self) -> String {
        format!("Pcp.ArcType.{}", self.inner.display_name())
    }

    fn __str__(&self) -> &str {
        self.inner.display_name()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __hash__(&self) -> u64 {
        self.inner.strength_index() as u64
    }

    fn __int__(&self) -> u64 {
        self.inner.strength_index() as u64
    }

    /// True if this is a class-based composition arc (inherit or specialize).
    #[pyo3(name = "IsClassBased")]
    fn is_class_based(&self) -> bool {
        self.inner.is_class_based()
    }

    /// True if this is a composition arc (not root).
    #[pyo3(name = "IsCompositionArc")]
    fn is_composition_arc(&self) -> bool {
        self.inner.is_composition_arc()
    }
}

// ============================================================================
// LayerStackIdentifier
// ============================================================================

/// Identifies a layer stack by its root layer, session layer, and context.
///
/// Mirrors `pxr.Pcp.LayerStackIdentifier` / `PcpLayerStackIdentifier`.
#[pyclass(
    skip_from_py_object,
    name = "LayerStackIdentifier",
    module = "pxr.Pcp"
)]
#[derive(Clone)]
pub struct PyLayerStackIdentifier {
    inner: LayerStackIdentifier,
}

#[pymethods]
impl PyLayerStackIdentifier {
    /// Create a LayerStackIdentifier from a root layer path.
    ///
    /// ```python
    /// id = Pcp.LayerStackIdentifier("root.usda")
    /// id2 = Pcp.LayerStackIdentifier("root.usda", "session.usda")
    /// ```
    #[new]
    #[pyo3(signature = (root_layer, session_layer = None))]
    fn new(root_layer: &str, session_layer: Option<&str>) -> Self {
        let inner = match session_layer {
            Some(s) => LayerStackIdentifier::with_session(root_layer, Some(s)),
            None => LayerStackIdentifier::new(root_layer),
        };
        Self { inner }
    }

    /// The root layer path string.
    #[getter]
    #[pyo3(name = "rootLayer")]
    fn root_layer(&self) -> &str {
        self.inner.root_layer.get_authored_path()
    }

    /// The session layer path string, or None.
    #[getter]
    #[pyo3(name = "sessionLayer")]
    fn session_layer(&self) -> Option<&str> {
        self.inner
            .session_layer
            .as_ref()
            .map(|p| p.get_authored_path())
    }

    /// True if this identifier has a non-empty root layer.
    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "Pcp.LayerStackIdentifier('{}')",
            self.inner.root_layer.get_authored_path()
        )
    }

    fn __str__(&self) -> String {
        format!("{}", self.inner)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __hash__(&self) -> u64 {
        self.inner.get_hash()
    }

    fn __lt__(&self, other: &Self) -> bool {
        self.inner < other.inner
    }

    fn __le__(&self, other: &Self) -> bool {
        self.inner <= other.inner
    }

    fn __gt__(&self, other: &Self) -> bool {
        self.inner > other.inner
    }

    fn __ge__(&self, other: &Self) -> bool {
        self.inner >= other.inner
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

impl PyLayerStackIdentifier {
    pub fn from_inner(inner: LayerStackIdentifier) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &LayerStackIdentifier {
        &self.inner
    }
}

// ============================================================================
// PcpSite
// ============================================================================

/// A site specifies a path in a layer stack of scene description.
///
/// Mirrors `pxr.Pcp.Site` / `PcpSite`.
#[pyclass(skip_from_py_object, name = "Site", module = "pxr.Pcp")]
#[derive(Clone)]
pub struct PySite {
    inner: Site,
}

#[pymethods]
impl PySite {
    #[new]
    fn new(layer_stack_id: &PyLayerStackIdentifier, path: &str) -> PyResult<Self> {
        let sdf_path = Path::from_string(path)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{}'", path)))?;
        Ok(Self {
            inner: Site::new(layer_stack_id.inner.clone(), sdf_path),
        })
    }

    /// The layer stack identifier for this site.
    #[getter]
    #[pyo3(name = "layerStackIdentifier")]
    fn layer_stack_identifier(&self) -> PyLayerStackIdentifier {
        PyLayerStackIdentifier::from_inner(self.inner.layer_stack_identifier.clone())
    }

    /// The path within the layer stack.
    #[getter]
    fn path(&self) -> &str {
        self.inner.path.as_str()
    }

    /// True if both layer stack and path are valid and non-empty.
    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        format!(
            "Pcp.Site('{}', '{}')",
            self.inner
                .layer_stack_identifier
                .root_layer
                .get_authored_path(),
            self.inner.path.as_str()
        )
    }

    fn __str__(&self) -> String {
        format!("{}", self.inner)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __hash__(&self) -> u64 {
        usd_pcp::SiteHash::hash(&self.inner)
    }
}

// ============================================================================
// PcpMapFunction
// ============================================================================

/// Maps values from one namespace (and time domain) to another.
///
/// Mirrors `pxr.Pcp.MapFunction` / `PcpMapFunction`.
#[pyclass(skip_from_py_object, name = "MapFunction", module = "pxr.Pcp")]
#[derive(Clone)]
pub struct PyMapFunction {
    inner: MapFunction,
}

#[pymethods]
impl PyMapFunction {
    /// Return the identity map function.
    #[classmethod]
    #[pyo3(name = "Identity")]
    fn identity(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self {
            inner: MapFunction::identity().clone(),
        }
    }

    /// Return the null (empty) map function.
    #[classmethod]
    #[pyo3(name = "Null")]
    fn null(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self {
            inner: MapFunction::null(),
        }
    }

    /// Map a source path to a target path, returning None if unmappable.
    #[pyo3(name = "MapSourceToTarget")]
    fn map_source_to_target(&self, path: &str) -> PyResult<Option<String>> {
        let sdf_path = Path::from_string(path)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid path: '{}'", path)))?;
        Ok(self
            .inner
            .map_source_to_target(&sdf_path)
            .map(|p| p.as_str().to_string()))
    }

    /// Map a target path to a source path, returning None if unmappable.
    #[pyo3(name = "MapTargetToSource")]
    fn map_target_to_source(&self, path: &str) -> PyResult<Option<String>> {
        let sdf_path = Path::from_string(path)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid path: '{}'", path)))?;
        Ok(self
            .inner
            .map_target_to_source(&sdf_path)
            .map(|p| p.as_str().to_string()))
    }

    /// Compose this map function with an inner map function.
    ///
    /// Result applies `inner` first, then `self`.
    #[pyo3(name = "Compose")]
    fn compose(&self, inner: &Self) -> Self {
        Self {
            inner: self.inner.compose(&inner.inner),
        }
    }

    /// True if this is an identity map function.
    #[pyo3(name = "IsIdentity")]
    fn is_identity(&self) -> bool {
        self.inner.is_identity()
    }

    /// True if this is a null (empty) map function.
    #[pyo3(name = "IsNull")]
    fn is_null(&self) -> bool {
        self.inner.is_null()
    }

    /// True if this is an identity mapping without time offset.
    #[pyo3(name = "IsIdentityPathMapping")]
    fn is_identity_path_mapping(&self) -> bool {
        self.inner.is_identity_path_mapping()
    }

    /// The time offset component as (offset_seconds, scale) tuple.
    #[getter]
    #[pyo3(name = "timeOffset")]
    fn time_offset(&self) -> (f64, f64) {
        let off = self.inner.time_offset();
        (off.offset(), off.scale())
    }

    fn __repr__(&self) -> String {
        if self.inner.is_identity() {
            "Pcp.MapFunction.Identity()".to_string()
        } else if self.inner.is_null() {
            "Pcp.MapFunction.Null()".to_string()
        } else {
            format!("Pcp.MapFunction(offset={:?})", self.inner.time_offset())
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_null()
    }
}

impl PyMapFunction {
    pub fn from_inner(inner: MapFunction) -> Self {
        Self { inner }
    }
}

// ============================================================================
// PcpLayerStack
// ============================================================================

/// A composed stack of layers contributing opinions.
///
/// Mirrors `pxr.Pcp.LayerStack` / `PcpLayerStack`.
#[pyclass(skip_from_py_object, name = "LayerStack", module = "pxr.Pcp")]
#[derive(Clone)]
pub struct PyLayerStack {
    inner: std::sync::Arc<LayerStack>,
}

#[pymethods]
impl PyLayerStack {
    /// The identifier for this layer stack.
    #[getter]
    #[pyo3(name = "identifier")]
    fn identifier(&self) -> PyLayerStackIdentifier {
        PyLayerStackIdentifier::from_inner(self.inner.identifier().clone())
    }

    /// The layers in strong-to-weak order, as identifier strings.
    #[getter]
    #[pyo3(name = "layers")]
    fn layers(&self) -> Vec<String> {
        self.inner
            .get_layers()
            .iter()
            .map(|l| l.identifier().to_string())
            .collect()
    }

    /// Muted layer identifiers.
    #[getter]
    #[pyo3(name = "mutedLayers")]
    fn muted_layers(&self) -> Vec<String> {
        self.inner.get_muted_layers()
    }

    /// Relocates source-to-target map as dict of path strings.
    ///
    /// Stub: relocates are architecture-level; return empty dict currently.
    #[getter]
    #[pyo3(name = "relocatesSourceToTarget")]
    fn relocates_source_to_target<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        PyDict::new(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "Pcp.LayerStack('{}')",
            self.inner.identifier().root_layer.get_authored_path()
        )
    }

    fn __str__(&self) -> String {
        format!("Pcp.LayerStack({})", self.inner.get_layers().len())
    }
}

impl PyLayerStack {
    pub fn from_arc(inner: std::sync::Arc<LayerStack>) -> Self {
        Self { inner }
    }
}

// ============================================================================
// PcpNodeRef
// ============================================================================

/// A reference to a node in the prim index graph.
///
/// Mirrors `pxr.Pcp.NodeRef` / `PcpNodeRef`.
#[pyclass(skip_from_py_object, name = "NodeRef", module = "pxr.Pcp")]
#[derive(Clone)]
pub struct PyNodeRef {
    inner: usd_pcp::NodeRef,
}

#[pymethods]
impl PyNodeRef {
    /// True if this node reference is valid.
    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// The arc type connecting this node to its parent.
    #[getter]
    #[pyo3(name = "arcType")]
    fn arc_type(&self) -> PyArcType {
        PyArcType {
            inner: self.inner.arc_type(),
        }
    }

    /// The path at this node's site.
    #[getter]
    fn path(&self) -> String {
        self.inner.path().as_str().to_string()
    }

    /// True if this is the root node.
    #[pyo3(name = "IsRootNode")]
    fn is_root_node(&self) -> bool {
        self.inner.is_root_node()
    }

    /// True if this node can contribute opinions.
    #[pyo3(name = "CanContributeSpecs")]
    fn can_contribute_specs(&self) -> bool {
        self.inner.can_contribute_specs()
    }

    /// True if this node has any specs.
    #[pyo3(name = "HasSpecs")]
    fn has_specs(&self) -> bool {
        self.inner.has_specs()
    }

    /// The map-to-parent function: evaluates MapExpression to MapFunction.
    #[getter]
    #[pyo3(name = "mapToParent")]
    fn map_to_parent(&self) -> PyMapFunction {
        // NodeRef.map_to_parent() returns MapExpression, evaluate to MapFunction.
        PyMapFunction::from_inner(self.inner.map_to_parent().evaluate())
    }

    /// The map-to-root function: evaluates MapExpression to MapFunction.
    #[getter]
    #[pyo3(name = "mapToRoot")]
    fn map_to_root(&self) -> PyMapFunction {
        PyMapFunction::from_inner(self.inner.map_to_root().evaluate())
    }

    /// The parent node, or an invalid NodeRef if this is root.
    #[getter]
    fn parent(&self) -> Self {
        Self {
            inner: self.inner.parent_node(),
        }
    }

    /// Direct child nodes of this node.
    #[getter]
    fn children(&self) -> Vec<Self> {
        self.inner
            .children()
            .into_iter()
            .map(|n| Self { inner: n })
            .collect()
    }

    /// Unique identifier for stable node identity within a session.
    fn unique_identifier(&self) -> usize {
        self.inner.unique_identifier()
    }

    fn __repr__(&self) -> String {
        if self.inner.is_valid() {
            format!(
                "Pcp.NodeRef('{}', arcType={})",
                self.inner.path().as_str(),
                self.inner.arc_type().display_name()
            )
        } else {
            "Pcp.NodeRef(invalid)".to_string()
        }
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.unique_identifier() == other.inner.unique_identifier()
    }

    fn __hash__(&self) -> usize {
        self.inner.unique_identifier()
    }
}

// ============================================================================
// PcpPrimIndex
// ============================================================================

/// An index of all sites of scene description contributing opinions to a prim.
///
/// Mirrors `pxr.Pcp.PrimIndex` / `PcpPrimIndex`.
#[pyclass(skip_from_py_object, name = "PrimIndex", module = "pxr.Pcp")]
#[derive(Clone)]
pub struct PyPrimIndex {
    pub(crate) inner: PrimIndex,
}

impl PyPrimIndex {
    pub fn from_index(idx: PrimIndex) -> Self {
        Self { inner: idx }
    }
}

#[pymethods]
impl PyPrimIndex {
    /// True if this prim index is valid (has a graph).
    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// The root node of the composition graph.
    #[getter]
    #[pyo3(name = "rootNode")]
    fn root_node(&self) -> PyNodeRef {
        PyNodeRef {
            inner: self.inner.root_node(),
        }
    }

    /// The path this index was computed for.
    #[getter]
    fn path(&self) -> String {
        self.inner.path().as_str().to_string()
    }

    /// True if this prim has any payload arcs.
    #[pyo3(name = "HasAnyPayloads")]
    fn has_any_payloads(&self) -> bool {
        self.inner.has_any_payloads()
    }

    /// Authored variant selections composed from all nodes, as dict {setName: selection}.
    #[pyo3(name = "ComposeAuthoredVariantSelections")]
    fn compose_authored_variant_selections<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let d = PyDict::new(py);
        for (set_name, selection) in self.inner.compose_authored_variant_selections() {
            let _ = d.set_item(set_name, selection);
        }
        d
    }

    /// Return a string description of this prim index for debugging.
    #[pyo3(name = "DumpToString")]
    #[pyo3(signature = (include_inherit_origin = true, include_maps = true))]
    fn dump_to_string(&self, include_inherit_origin: bool, include_maps: bool) -> String {
        self.inner
            .dump_to_string(include_inherit_origin, include_maps)
    }

    /// The prim stack as list of (node_index, layer_index) tuples.
    ///
    /// Note: in C++ API these are (SdfLayer*, SdfPath) pairs. Here we expose
    /// the compressed form (node_idx, layer_idx) pending full layer resolution.
    #[getter]
    #[pyo3(name = "primStack")]
    fn prim_stack(&self) -> Vec<(usize, usize)> {
        self.inner
            .prim_stack()
            .iter()
            .map(|s| (s.node_index, s.layer_index))
            .collect()
    }

    /// All nodes in strength order.
    #[getter]
    #[pyo3(name = "nodes")]
    fn nodes(&self) -> Vec<PyNodeRef> {
        self.inner
            .nodes()
            .into_iter()
            .map(|n| PyNodeRef { inner: n })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Pcp.PrimIndex('{}', valid={})",
            self.inner.path().as_str(),
            self.inner.is_valid()
        )
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ============================================================================
// PcpCache
// ============================================================================

/// Context for making requests of the Pcp composition algorithm.
///
/// Mirrors `pxr.Pcp.Cache` / `PcpCache`.
#[pyclass(skip_from_py_object, name = "Cache", module = "pxr.Pcp")]
pub struct PyCache {
    inner: std::sync::Arc<Cache>,
}

#[pymethods]
impl PyCache {
    /// Create a new PcpCache for the given layer stack identifier.
    ///
    /// ```python
    /// layer_id = Pcp.LayerStackIdentifier("root.usda")
    /// cache = Pcp.Cache(layer_id)
    /// cache_usd = Pcp.Cache(layer_id, usd=True)
    /// ```
    #[new]
    #[pyo3(signature = (layer_stack_identifier, usd = false))]
    fn new(layer_stack_identifier: &PyLayerStackIdentifier, usd: bool) -> Self {
        Self {
            inner: Cache::new(layer_stack_identifier.inner.clone(), usd),
        }
    }

    /// Return the layer stack identifier for this cache.
    #[pyo3(name = "GetLayerStackIdentifier")]
    fn get_layer_stack_identifier(&self) -> PyLayerStackIdentifier {
        PyLayerStackIdentifier::from_inner(self.inner.layer_stack_identifier().clone())
    }

    /// Compute the prim index for the given path.
    ///
    /// Returns a PrimIndex (errors are discarded; use Rust API for full diagnostics).
    #[pyo3(name = "ComputePrimIndex")]
    fn compute_prim_index(&self, path: &str) -> PyResult<PyPrimIndex> {
        let sdf_path = Path::from_string(path)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{}'", path)))?;
        let (index, _errors) = self.inner.compute_prim_index(&sdf_path);
        Ok(PyPrimIndex { inner: index })
    }

    /// Return a previously computed prim index, or None if not yet computed.
    #[pyo3(name = "FindPrimIndex")]
    fn find_prim_index(&self, path: &str) -> PyResult<Option<PyPrimIndex>> {
        let sdf_path = Path::from_string(path)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{}'", path)))?;
        Ok(self
            .inner
            .find_prim_index(&sdf_path)
            .map(|idx| PyPrimIndex { inner: idx }))
    }

    /// Return all layer identifiers used by this cache.
    #[pyo3(name = "GetUsedLayers")]
    fn get_used_layers(&self) -> Vec<String> {
        self.inner.get_used_layers().into_iter().collect()
    }

    /// Include or exclude paths from payload loading.
    ///
    /// `include` is a list of path strings to include.
    /// `exclude` is a list of path strings to exclude.
    #[pyo3(name = "RequestPayloads")]
    #[pyo3(signature = (include, exclude))]
    fn request_payloads(&self, include: Vec<String>, exclude: Vec<String>) -> PyResult<()> {
        let include_paths: Vec<Path> = include
            .iter()
            .map(|p| {
                Path::from_string(p)
                    .ok_or_else(|| PyValueError::new_err(format!("Invalid include path: '{}'", p)))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let exclude_paths: Vec<Path> = exclude
            .iter()
            .map(|p| {
                Path::from_string(p)
                    .ok_or_else(|| PyValueError::new_err(format!("Invalid exclude path: '{}'", p)))
            })
            .collect::<PyResult<Vec<_>>>()?;
        // None = apply changes immediately (no external CacheChanges tracking).
        self.inner
            .request_payloads(&include_paths, &exclude_paths, None);
        Ok(())
    }

    /// Request layer muting.
    ///
    /// `mute` is a list of layer identifiers to mute.
    /// `unmute` is a list to unmute.
    #[pyo3(name = "RequestLayerMuting")]
    #[pyo3(signature = (mute, unmute))]
    fn request_layer_muting(&self, mute: Vec<String>, unmute: Vec<String>) {
        // None for all optional args: apply changes immediately, discard newly muted/unmuted lists.
        self.inner
            .request_layer_muting(&mute, &unmute, None, None, None);
    }

    /// Return true if the payload for the given path is included.
    #[pyo3(name = "IsPayloadIncluded")]
    fn is_payload_included(&self, path: &str) -> PyResult<bool> {
        let sdf_path = Path::from_string(path)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{}'", path)))?;
        Ok(self.inner.is_payload_included(&sdf_path))
    }

    /// Reload all mutable layers in the cache.
    #[pyo3(name = "Reload")]
    fn reload(&self) {
        // None = apply changes immediately without external tracking.
        self.inner.reload(None);
    }

    fn __repr__(&self) -> String {
        format!(
            "Pcp.Cache('{}')",
            self.inner
                .layer_stack_identifier()
                .root_layer
                .get_authored_path()
        )
    }
}

// ============================================================================
// PcpError base
// ============================================================================

/// Base class for all PCP errors.
///
/// Mirrors `pxr.Pcp.Error` / `PcpError`.
#[pyclass(skip_from_py_object, name = "Error", module = "pxr.Pcp", subclass)]
pub struct PyPcpError {
    pub message: String,
    pub error_type: String,
}

#[pymethods]
impl PyPcpError {
    #[new]
    #[pyo3(signature = (message = "", error_type = ""))]
    fn new(message: &str, error_type: &str) -> Self {
        Self {
            message: message.to_string(),
            error_type: error_type.to_string(),
        }
    }

    /// Human-readable error description.
    #[getter]
    fn message(&self) -> &str {
        &self.message
    }

    /// Error type string.
    #[getter]
    #[pyo3(name = "errorType")]
    fn error_type(&self) -> &str {
        &self.error_type
    }

    fn __repr__(&self) -> String {
        format!("Pcp.Error('{}', type='{}')", self.message, self.error_type)
    }

    fn __str__(&self) -> &str {
        &self.message
    }
}

// ============================================================================
// PcpDependency
// ============================================================================

/// Records the dependency of a prim index on a site.
///
/// Mirrors `pxr.Pcp.Dependency` / `PcpDependency`.
#[pyclass(skip_from_py_object, name = "Dependency", module = "pxr.Pcp")]
#[derive(Clone)]
pub struct PyDependency {
    index_path: String,
    site_path: String,
    map_func: PyMapFunction,
}

#[pymethods]
impl PyDependency {
    #[new]
    fn new(index_path: &str, site_path: &str) -> Self {
        Self {
            index_path: index_path.to_string(),
            site_path: site_path.to_string(),
            map_func: PyMapFunction {
                inner: MapFunction::null(),
            },
        }
    }

    /// The prim index path that has this dependency.
    #[getter]
    #[pyo3(name = "indexPath")]
    fn index_path(&self) -> &str {
        &self.index_path
    }

    /// The site path that is depended upon.
    #[getter]
    #[pyo3(name = "sitePath")]
    fn site_path(&self) -> &str {
        &self.site_path
    }

    /// The map function from site to index namespace.
    #[getter]
    #[pyo3(name = "mapFunc")]
    fn map_func(&self) -> PyMapFunction {
        self.map_func.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Pcp.Dependency(indexPath='{}', sitePath='{}')",
            self.index_path, self.site_path
        )
    }
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Pcp classes and free functions into the `pxr.Pcp` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;

    // Enum
    m.add_class::<PyArcType>()?;

    // Core types
    m.add_class::<PyLayerStackIdentifier>()?;
    m.add_class::<PySite>()?;
    m.add_class::<PyMapFunction>()?;
    m.add_class::<PyLayerStack>()?;
    m.add_class::<PyNodeRef>()?;
    m.add_class::<PyPrimIndex>()?;
    m.add_class::<PyCache>()?;

    // Error and dependency types
    m.add_class::<PyPcpError>()?;
    m.add_class::<PyDependency>()?;

    // Module-level ArcType constants (matches pxr.Pcp.ArcTypeRoot, etc.)
    m.add(
        "ArcTypeRoot",
        PyArcType {
            inner: ArcType::Root,
        },
    )?;
    m.add(
        "ArcTypeInherit",
        PyArcType {
            inner: ArcType::Inherit,
        },
    )?;
    m.add(
        "ArcTypeVariant",
        PyArcType {
            inner: ArcType::Variant,
        },
    )?;
    m.add(
        "ArcTypeRelocate",
        PyArcType {
            inner: ArcType::Relocate,
        },
    )?;
    m.add(
        "ArcTypeReference",
        PyArcType {
            inner: ArcType::Reference,
        },
    )?;
    m.add(
        "ArcTypePayload",
        PyArcType {
            inner: ArcType::Payload,
        },
    )?;
    m.add(
        "ArcTypeSpecialize",
        PyArcType {
            inner: ArcType::Specialize,
        },
    )?;

    // Module-level DependencyType constants (matches pxr.Pcp.DependencyType*, bitmask ints)
    m.add("DependencyTypeNone", usd_pcp::DependencyType::NONE.bits())?;
    m.add("DependencyTypeRoot", usd_pcp::DependencyType::ROOT.bits())?;
    m.add(
        "DependencyTypePurelyDirect",
        usd_pcp::DependencyType::PURELY_DIRECT.bits(),
    )?;
    m.add(
        "DependencyTypePartlyDirect",
        usd_pcp::DependencyType::PARTLY_DIRECT.bits(),
    )?;
    m.add(
        "DependencyTypeDirect",
        usd_pcp::DependencyType::DIRECT.bits(),
    )?;
    m.add(
        "DependencyTypeAncestral",
        usd_pcp::DependencyType::ANCESTRAL.bits(),
    )?;
    m.add(
        "DependencyTypeVirtual",
        usd_pcp::DependencyType::VIRTUAL.bits(),
    )?;
    m.add(
        "DependencyTypeNonVirtual",
        usd_pcp::DependencyType::NON_VIRTUAL.bits(),
    )?;
    m.add(
        "DependencyTypeAnyNonVirtual",
        usd_pcp::DependencyType::ANY_NON_VIRTUAL.bits(),
    )?;
    m.add(
        "DependencyTypeAnyIncludingVirtual",
        usd_pcp::DependencyType::ANY_INCLUDING_VIRTUAL.bits(),
    )?;

    Ok(())
}
