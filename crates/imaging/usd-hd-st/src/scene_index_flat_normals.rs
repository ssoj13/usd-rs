
//! HdSt_FlatNormalsSceneIndex - flat normal computation for meshes.
//!
//! Filtering scene index that augments mesh prims with computed flat normals.
//! Flat normals are per-face normals computed from the face vertices, giving
//! a faceted appearance. The actual computation is performed lazily by
//! `FlatNormalsComputationCpu` from flat_normals.rs.
//!
//! When a mesh prim is queried, if it is a mesh type, the scene index overlays
//! a `computedFlatNormals` data source on top of the input prim's data source.
//! This signals `HdStMesh::_PopulateVertexPrimvars` to schedule flat normal
//! computation when no authored normals are present.
//!
//! Port of C++ `HdSt_FlatNormalsComputationCPU`/`GPU` into scene index pattern.

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
// Token names
// ---------------------------------------------------------------------------

pub mod tokens {
    /// Locator for the overlay container holding flat normals metadata.
    pub const COMPUTED_FLAT_NORMALS: &str = "computedFlatNormals";
    /// Whether flat normals are requested for this mesh.
    pub const NEEDS_FLAT_NORMALS: &str = "needsFlatNormals";
}

// ---------------------------------------------------------------------------
// Helper: make a sampled data source handle from a value
// ---------------------------------------------------------------------------

fn make_sampled(value: Value) -> HdDataSourceBaseHandle {
    HdRetainedSampledDataSource::new(value).clone_box()
}

// ---------------------------------------------------------------------------
// Computed flat normals data source
// ---------------------------------------------------------------------------

/// Container data source that signals flat normals are needed.
///
/// Overlaid on mesh prim data sources so that the mesh Rprim's sync
/// function knows to schedule `FlatNormalsComputationCpu` or GPU.
///
/// This matches the C++ pattern where `HdSt_FlatNormalsComputationCPU` is
/// added to the mesh's computation list during `HdStMesh::_PopulateElementPrimvars`.
#[derive(Clone, Debug)]
struct FlatNormalsDataSource {
    /// Whether packed 10-10-10-2 format is requested.
    packed: bool,
    /// Source attribute name for positions.
    src_name: Token,
    /// Destination attribute name for computed normals.
    dst_name: Token,
}

impl FlatNormalsDataSource {
    fn new(packed: bool, src_name: Token, dst_name: Token) -> Arc<Self> {
        Arc::new(Self {
            packed,
            src_name,
            dst_name,
        })
    }
}

usd_hd::impl_container_datasource_base!(FlatNormalsDataSource);

impl HdContainerDataSource for FlatNormalsDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new(tokens::NEEDS_FLAT_NORMALS),
            Token::new("srcName"),
            Token::new("dstName"),
            Token::new("packed"),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            tokens::NEEDS_FLAT_NORMALS => Some(make_sampled(Value::from(true))),
            "srcName" => Some(make_sampled(Value::from(
                self.src_name.as_str().to_string(),
            ))),
            "dstName" => Some(make_sampled(Value::from(
                self.dst_name.as_str().to_string(),
            ))),
            "packed" => Some(make_sampled(Value::from(self.packed))),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Scene index
// ---------------------------------------------------------------------------

/// Flat normals scene index for Storm.
///
/// Augments mesh prims with flat normal computation metadata.
/// When a mesh needs flat normals (no authored normals, flat shading requested),
/// this scene index overlays a `computedFlatNormals` data source on the prim
/// so that HdStMesh can schedule the computation during primvar population.
///
/// Port of C++ `HdSt_FlatNormalsComputationCPU`/`GPU` into scene index pattern.
pub struct HdStFlatNormalsSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Whether to request packed 10-10-10-2 normal output.
    packed: bool,
}

impl HdStFlatNormalsSceneIndex {
    /// Create a new flat normals scene index (unpacked float3 output).
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Self::with_packed(input_scene, false)
    }

    /// Create with configurable packed/unpacked output.
    pub fn with_packed(input_scene: Option<HdSceneIndexHandle>, packed: bool) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            packed,
        }))
    }

    /// Check if prim is a mesh type that may need flat normals.
    pub fn is_mesh_prim(prim_type: &Token) -> bool {
        prim_type == "mesh" || prim_type == "Mesh"
    }

    /// Dirty locators relevant to flat normal computation (topology + points).
    fn normals_dirty_locators() -> HdDataSourceLocatorSet {
        let mut set = HdDataSourceLocatorSet::new();
        set.insert(HdDataSourceLocator::from_token(Token::new("points")));
        set.insert(HdDataSourceLocator::from_token(Token::new("meshTopology")));
        set
    }

    /// Build an overlay data source that injects computed flat normals metadata.
    ///
    /// The overlay adds a `computedFlatNormals` container child to the prim
    /// data source, signaling HdStMesh to schedule flat normal computation.
    fn build_normals_overlay(
        &self,
        base_ds: HdContainerDataSourceHandle,
    ) -> HdContainerDataSourceHandle {
        let normals_ds =
            FlatNormalsDataSource::new(self.packed, Token::new("points"), Token::new("normals"));

        // Wrap as single-key container: computedFlatNormals -> normals_ds
        let normals_container = HdRetainedContainerDataSource::new_1(
            Token::new(tokens::COMPUTED_FLAT_NORMALS),
            normals_ds.clone_box(),
        );

        // Overlay: normals_container (higher priority) over base prim data
        HdOverlayContainerDataSource::new_2(normals_container, base_ds)
    }
}

impl HdSceneIndexBase for HdStFlatNormalsSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);

                // Only augment mesh prims that have a data source
                if Self::is_mesh_prim(&prim.prim_type) {
                    if let Some(base_ds) = prim.data_source {
                        return HdSceneIndexPrim {
                            prim_type: prim.prim_type,
                            data_source: Some(self.build_normals_overlay(base_ds)),
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
        "HdSt_FlatNormalsSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStFlatNormalsSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let normals_locs = Self::normals_dirty_locators();
        let normals_out =
            HdDataSourceLocator::from_token(Token::new(tokens::COMPUTED_FLAT_NORMALS));
        let mut augmented = Vec::with_capacity(entries.len());

        for entry in entries {
            // If points or topology changed, also dirty computed flat normals
            if entry.dirty_locators.intersects(&normals_locs) {
                let mut locs = entry.dirty_locators.clone();
                locs.insert(normals_out.clone());
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
        let si = HdStFlatNormalsSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_FlatNormalsSceneIndex");
    }

    #[test]
    fn test_is_mesh_prim() {
        assert!(HdStFlatNormalsSceneIndex::is_mesh_prim(&Token::new("mesh")));
        assert!(HdStFlatNormalsSceneIndex::is_mesh_prim(&Token::new("Mesh")));
        assert!(!HdStFlatNormalsSceneIndex::is_mesh_prim(&Token::new(
            "basisCurves"
        )));
    }

    #[test]
    fn test_flat_normals_data_source() {
        let ds = FlatNormalsDataSource::new(false, Token::new("points"), Token::new("normals"));
        let names = ds.get_names();
        assert!(names.contains(&Token::new(tokens::NEEDS_FLAT_NORMALS)));
        assert!(names.contains(&Token::new("srcName")));
        assert!(names.contains(&Token::new("dstName")));
        assert!(names.contains(&Token::new("packed")));

        let val = ds.get(&Token::new(tokens::NEEDS_FLAT_NORMALS));
        assert!(val.is_some());
    }

    #[test]
    fn test_dirty_propagation_includes_topology() {
        let locs = HdStFlatNormalsSceneIndex::normals_dirty_locators();
        let topo_loc = HdDataSourceLocator::from_token(Token::new("meshTopology"));
        assert!(locs.intersects_locator(&topo_loc));
    }

    #[test]
    fn test_packed_variant() {
        let si = HdStFlatNormalsSceneIndex::with_packed(None, true);
        let lock = si.read();
        assert!(lock.packed);
    }
}
