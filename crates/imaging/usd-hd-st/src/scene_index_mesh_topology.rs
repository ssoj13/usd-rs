
//! HdSt_MeshTopologySceneIndex - mesh topology processing for Storm.
//!
//! Filtering scene index that processes mesh topology, handling:
//! - Triangulation of polygonal meshes for non-subdivided rendering
//! - Quadrangulation for subdivision surfaces (catmullClark, loop)
//! - Index buffer generation metadata signaling
//! - Face-varying primvar processing support
//! - Geom subset management
//!
//! Port of C++ `HdSt_MeshTopology` into scene index pattern.
//!
//! The scene index adds a `computedTopology` container to mesh prims that
//! carries metadata about what processing is needed (triangulate/quadrangulate).
//! The actual computation is done by `HdStMeshTopology` during Rprim sync.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedSampledDataSource,
};
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Mesh refinement mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefineMode {
    /// Uniform subdivision
    Uniform,
    /// Patch-based subdivision (tessellation shaders)
    Patches,
}

/// Quads processing mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuadsMode {
    /// Triangulate quads for rendering (faster, compatible with all hardware).
    Triangulated,
    /// Keep quads untriangulated (for GPU quad rendering via adjacency shaders).
    Untriangulated,
}

// ---------------------------------------------------------------------------
// Token names
// ---------------------------------------------------------------------------

pub mod tokens {
    /// Overlay container key for computed topology processing metadata.
    pub const COMPUTED_TOPOLOGY: &str = "computedTopology";
    /// Whether triangulation has been requested.
    pub const NEEDS_TRIANGULATION: &str = "needsTriangulation";
    /// Whether quadrangulation has been requested.
    pub const NEEDS_QUADRANGULATION: &str = "needsQuadrangulation";
    /// Subdivision scheme token key.
    pub const SUBDIVISION_SCHEME: &str = "subdivisionScheme";
}

// ---------------------------------------------------------------------------
// Helper: build a sampled data source handle from a value
// ---------------------------------------------------------------------------

fn make_sampled(value: Value) -> HdDataSourceBaseHandle {
    HdRetainedSampledDataSource::new(value).clone_box()
}

// ---------------------------------------------------------------------------
// Computed topology data source
// ---------------------------------------------------------------------------

/// Container data source for computed topology processing metadata.
///
/// Signals to `HdStMesh::_PopulateTopology` what processing is needed:
/// - `needsTriangulation`: true if non-subdivided non-quad mesh
/// - `needsQuadrangulation`: true if CatmullClark/Loop subdivision
///
/// This matches the C++ pattern in `HdStMesh::_PopulateTopology` which
/// calls `HdSt_MeshTopology::RefinesToTriangles()` / `GetScheme()`.
#[derive(Clone, Debug)]
struct ComputedTopologyDataSource {
    /// Whether triangulation is needed (polygon to triangle fan).
    needs_triangulation: bool,
    /// Whether quadrangulation is needed (for catmullClark/loop).
    needs_quadrangulation: bool,
    /// Subdivision scheme (e.g. "none", "catmullClark", "loop").
    subdivision_scheme: Token,
}

impl ComputedTopologyDataSource {
    fn new(
        needs_triangulation: bool,
        needs_quadrangulation: bool,
        subdivision_scheme: Token,
    ) -> Arc<Self> {
        Arc::new(Self {
            needs_triangulation,
            needs_quadrangulation,
            subdivision_scheme,
        })
    }
}

usd_hd::impl_container_datasource_base!(ComputedTopologyDataSource);

impl HdContainerDataSource for ComputedTopologyDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new(tokens::NEEDS_TRIANGULATION),
            Token::new(tokens::NEEDS_QUADRANGULATION),
            Token::new(tokens::SUBDIVISION_SCHEME),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            tokens::NEEDS_TRIANGULATION => {
                Some(make_sampled(Value::from(self.needs_triangulation)))
            }
            tokens::NEEDS_QUADRANGULATION => {
                Some(make_sampled(Value::from(self.needs_quadrangulation)))
            }
            tokens::SUBDIVISION_SCHEME => Some(make_sampled(Value::from(
                self.subdivision_scheme.as_str().to_string(),
            ))),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Subdivision scheme helpers
// ---------------------------------------------------------------------------

/// Returns true if the subdivision scheme is a smooth scheme (catmullClark/loop).
///
/// These schemes require quadrangulation before refinement, unlike "bilinear"
/// or "none" which go directly to triangulation.
fn is_smooth_subdivision(scheme: &str) -> bool {
    matches!(scheme, "catmullClark" | "loop")
}

/// Returns true if the scheme triangulates directly (none/bilinear/empty).
fn is_linear_subdivision(scheme: &str) -> bool {
    matches!(scheme, "none" | "bilinear" | "")
}

// ---------------------------------------------------------------------------
// Scene index
// ---------------------------------------------------------------------------

/// Mesh topology scene index for Storm.
///
/// Processes mesh topology to produce GPU-ready index buffers.
/// Handles triangulation of arbitrary polygons, quadrangulation
/// for subdivision surfaces, and geom subset index remapping.
///
/// Port of C++ `HdSt_MeshTopology` into scene index pattern.
pub struct HdStMeshTopologySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Default quads mode for non-subdivided meshes.
    default_quads_mode: QuadsMode,
}

impl HdStMeshTopologySceneIndex {
    /// Create a new mesh topology scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Self::with_quads_mode(input_scene, QuadsMode::Untriangulated)
    }

    /// Create with specific quads mode.
    pub fn with_quads_mode(
        input_scene: Option<HdSceneIndexHandle>,
        quads_mode: QuadsMode,
    ) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            default_quads_mode: quads_mode,
        }))
    }

    /// Check if prim type is a mesh.
    pub fn is_mesh_prim(prim_type: &Token) -> bool {
        prim_type == "mesh" || prim_type == "Mesh"
    }

    /// Get the default quads mode.
    pub fn get_quads_mode(&self) -> QuadsMode {
        self.default_quads_mode
    }

    /// Topology-related dirty locators.
    fn topology_dirty_locators() -> HdDataSourceLocatorSet {
        let mut set = HdDataSourceLocatorSet::new();
        set.insert(HdDataSourceLocator::from_token(Token::new("meshTopology")));
        set.insert(HdDataSourceLocator::from_token(Token::new(
            "subdivisionScheme",
        )));
        set.insert(HdDataSourceLocator::from_token(Token::new("geomSubsets")));
        set
    }

    /// Read the subdivision scheme from a prim's meshTopology data source.
    ///
    /// Attempts to read `meshTopology.subdivisionScheme` from the data source.
    /// Returns "none" as default if not found (no subdivision).
    fn read_subdivision_scheme(data_source: &HdContainerDataSourceHandle) -> Token {
        // Try meshTopology child container
        if let Some(topo_ds) = data_source.get(&Token::new("meshTopology")) {
            // Use as_sampled + sample_at_zero path for simple cases
            // For container access, use as_any downcast to HdRetainedContainerDataSource
            if let Some(container) = topo_ds
                .as_any()
                .downcast_ref::<HdRetainedContainerDataSource>()
            {
                if let Some(scheme_ds) = container.get(&Token::new("subdivisionScheme")) {
                    // sample_at_zero() on HdDataSourceBase; get::<String>() for string value
                    if let Some(val) = scheme_ds.sample_at_zero() {
                        if let Some(s) = val.get::<String>() {
                            return Token::new(s.as_str());
                        }
                    }
                }
            }
        }
        Token::new("none")
    }

    /// Build overlay data source with computed topology metadata.
    ///
    /// Inspects the prim's data source to determine the subdivision scheme,
    /// then adds appropriate processing flags for HdStMesh::_PopulateTopology.
    fn build_topology_overlay(
        &self,
        base_ds: HdContainerDataSourceHandle,
    ) -> HdContainerDataSourceHandle {
        let scheme = Self::read_subdivision_scheme(&base_ds);
        let scheme_str = scheme.as_str();

        // Smooth subdivision needs quadrangulation before GPU refinement.
        // Linear/none meshes need triangulation when Triangulated mode is requested.
        let needs_quad = is_smooth_subdivision(scheme_str);
        let needs_tri =
            is_linear_subdivision(scheme_str) && self.default_quads_mode == QuadsMode::Triangulated;

        let topo_ds = ComputedTopologyDataSource::new(needs_tri, needs_quad, scheme);

        let topo_container = HdRetainedContainerDataSource::new_1(
            Token::new(tokens::COMPUTED_TOPOLOGY),
            topo_ds.clone_box(),
        );

        // Overlay: topo_container (higher priority) over base prim data
        HdOverlayContainerDataSource::new_2(topo_container, base_ds)
    }
}

impl HdSceneIndexBase for HdStMeshTopologySceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);

                // Only process mesh prims with a data source
                if Self::is_mesh_prim(&prim.prim_type) {
                    if let Some(base_ds) = prim.data_source {
                        return HdSceneIndexPrim {
                            prim_type: prim.prim_type,
                            data_source: Some(self.build_topology_overlay(base_ds)),
                        };
                    }
                }
                return prim;
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                return input_lock.get_child_prim_paths(prim_path);
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _msg: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdSt_MeshTopologySceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStMeshTopologySceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let topo_locs = Self::topology_dirty_locators();
        let computed_topo = HdDataSourceLocator::from_token(Token::new(tokens::COMPUTED_TOPOLOGY));
        let mut augmented = Vec::with_capacity(entries.len());

        for entry in entries {
            // If mesh topology changed, also dirty computed topology metadata
            if entry.dirty_locators.intersects(&topo_locs) {
                let mut locs = entry.dirty_locators.clone();
                locs.insert(computed_topo.clone());
                augmented.push(DirtiedPrimEntry::new(entry.prim_path.clone(), locs));
            } else {
                augmented.push(entry.clone());
            }
        }

        self.base.forward_prims_dirtied(self, &augmented);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = HdStMeshTopologySceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_MeshTopologySceneIndex");
        assert_eq!(lock.get_quads_mode(), QuadsMode::Untriangulated);
    }

    #[test]
    fn test_is_mesh_prim() {
        assert!(HdStMeshTopologySceneIndex::is_mesh_prim(&Token::new(
            "mesh"
        )));
        assert!(HdStMeshTopologySceneIndex::is_mesh_prim(&Token::new(
            "Mesh"
        )));
        assert!(!HdStMeshTopologySceneIndex::is_mesh_prim(&Token::new(
            "basisCurves"
        )));
    }

    #[test]
    fn test_custom_quads_mode() {
        let si = HdStMeshTopologySceneIndex::with_quads_mode(None, QuadsMode::Triangulated);
        let lock = si.read();
        assert_eq!(lock.get_quads_mode(), QuadsMode::Triangulated);
    }

    #[test]
    fn test_subdivision_scheme_detection() {
        assert!(is_smooth_subdivision("catmullClark"));
        assert!(is_smooth_subdivision("loop"));
        assert!(!is_smooth_subdivision("none"));
        assert!(!is_smooth_subdivision("bilinear"));
        assert!(is_linear_subdivision("none"));
        assert!(is_linear_subdivision("bilinear"));
        assert!(!is_linear_subdivision("catmullClark"));
    }

    #[test]
    fn test_computed_topology_data_source() {
        let ds = ComputedTopologyDataSource::new(true, false, Token::new("none"));
        let names = ds.get_names();
        assert!(names.contains(&Token::new(tokens::NEEDS_TRIANGULATION)));
        assert!(names.contains(&Token::new(tokens::NEEDS_QUADRANGULATION)));
        assert!(names.contains(&Token::new(tokens::SUBDIVISION_SCHEME)));

        let tri_ds = ds.get(&Token::new(tokens::NEEDS_TRIANGULATION));
        assert!(tri_ds.is_some());
    }

    #[test]
    fn test_dirty_locators_include_topology() {
        let locs = HdStMeshTopologySceneIndex::topology_dirty_locators();
        let topo_loc = HdDataSourceLocator::from_token(Token::new("meshTopology"));
        let geom_loc = HdDataSourceLocator::from_token(Token::new("geomSubsets"));
        assert!(locs.intersects_locator(&topo_loc));
        assert!(locs.intersects_locator(&geom_loc));
    }
}
