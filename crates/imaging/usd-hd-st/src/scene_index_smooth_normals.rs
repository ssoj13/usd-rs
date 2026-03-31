
//! HdSt_SmoothNormalsSceneIndex - smooth normal computation for meshes.
//!
//! Filtering scene index that augments mesh prims with computed smooth normals.
//! When a mesh prim is queried, if it is a mesh type, the scene index overlays
//! a computed normals data source on top of the input prim's data source.
//!
//! Smooth normals are per-vertex normals computed by averaging the normals
//! of adjacent faces, giving a smooth shaded appearance. The actual computation
//! is performed lazily by `SmoothNormalsComputationCpu` from smooth_normals.rs.
//!
//! Port of C++ `HdSt_SmoothNormalsComputationCPU`/`GPU` + scene index pattern.

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
    /// Locator for the overlay container that holds computed normals metadata.
    pub const COMPUTED_SMOOTH_NORMALS: &str = "computedSmoothNormals";
    /// Whether smooth normals are requested for this mesh.
    pub const NEEDS_SMOOTH_NORMALS: &str = "needsSmoothNormals";
}

// ---------------------------------------------------------------------------
// Helper: build a sampled data source handle from a value
// ---------------------------------------------------------------------------

fn make_sampled(value: Value) -> HdDataSourceBaseHandle {
    HdRetainedSampledDataSource::new(value).clone_box()
}

// ---------------------------------------------------------------------------
// Computed normals data source
// ---------------------------------------------------------------------------

/// Container data source that signals smooth normals are needed.
///
/// Overlaid on mesh prim data sources so that the mesh Rprim's sync
/// function knows to schedule `SmoothNormalsComputationCpu` or GPU.
///
/// This matches the C++ pattern where `HdSt_SmoothNormalsComputationCPU` is
/// added to the mesh's computation list during `HdStMesh::_PopulateVertexPrimvars`.
#[derive(Clone, Debug)]
struct SmoothNormalsDataSource {
    /// Whether packed 10-10-10-2 format is requested.
    packed: bool,
    /// Source attribute name for positions.
    src_name: Token,
    /// Destination attribute name for computed normals.
    dst_name: Token,
}

impl SmoothNormalsDataSource {
    fn new(packed: bool, src_name: Token, dst_name: Token) -> Arc<Self> {
        Arc::new(Self {
            packed,
            src_name,
            dst_name,
        })
    }
}

usd_hd::impl_container_datasource_base!(SmoothNormalsDataSource);

impl HdContainerDataSource for SmoothNormalsDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new(tokens::NEEDS_SMOOTH_NORMALS),
            Token::new("srcName"),
            Token::new("dstName"),
            Token::new("packed"),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            tokens::NEEDS_SMOOTH_NORMALS => Some(make_sampled(Value::from(true))),
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

/// Smooth normals scene index for Storm.
///
/// Augments mesh prims with smooth normal computation metadata.
/// When a mesh needs smooth normals (smooth subdivision scheme, no authored
/// normals), this scene index overlays a `computedSmoothNormals` data source
/// on the prim so that HdStMesh::_PopulateVertexPrimvars can schedule the
/// computation.
///
/// Port of C++ `HdSt_SmoothNormalsComputationCPU`/`GPU` into scene index pattern.
pub struct HdStSmoothNormalsSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Whether to request packed 10-10-10-2 normal output.
    packed: bool,
}

impl HdStSmoothNormalsSceneIndex {
    /// Create a new smooth normals scene index (unpacked float3 output).
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

    /// Check if prim is a mesh type that may need smooth normals.
    pub fn is_mesh_prim(prim_type: &Token) -> bool {
        prim_type == "mesh" || prim_type == "Mesh"
    }

    /// Dirty locators relevant to smooth normal computation.
    fn normals_dirty_locators() -> HdDataSourceLocatorSet {
        let mut set = HdDataSourceLocatorSet::new();
        set.insert(HdDataSourceLocator::from_token(Token::new("points")));
        set.insert(HdDataSourceLocator::from_token(Token::new("meshTopology")));
        set.insert(HdDataSourceLocator::from_token(Token::new("adjacency")));
        set
    }

    /// Build an overlay data source that injects computed smooth normals metadata.
    ///
    /// The overlay adds a `computedSmoothNormals` container child to the prim
    /// data source, signaling HdStMesh to schedule smooth normal computation.
    fn build_normals_overlay(
        &self,
        base_ds: HdContainerDataSourceHandle,
    ) -> HdContainerDataSourceHandle {
        let normals_ds =
            SmoothNormalsDataSource::new(self.packed, Token::new("points"), Token::new("normals"));

        let normals_container = HdRetainedContainerDataSource::new_1(
            Token::new(tokens::COMPUTED_SMOOTH_NORMALS),
            normals_ds.clone_box(),
        );

        // Overlay: normals_container (higher priority) over base prim data
        HdOverlayContainerDataSource::new_2(normals_container, base_ds)
    }
}

impl HdSceneIndexBase for HdStSmoothNormalsSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);

                // Only augment mesh prims with a data source
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
        "HdSt_SmoothNormalsSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStSmoothNormalsSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let normals_locs = Self::normals_dirty_locators();
        let normals_out =
            HdDataSourceLocator::from_token(Token::new(tokens::COMPUTED_SMOOTH_NORMALS));
        let mut augmented = Vec::with_capacity(entries.len());

        for entry in entries {
            // If points, topology, or adjacency changed, also dirty computed smooth normals
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
        let si = HdStSmoothNormalsSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_SmoothNormalsSceneIndex");
    }

    #[test]
    fn test_is_mesh_prim() {
        assert!(HdStSmoothNormalsSceneIndex::is_mesh_prim(&Token::new(
            "mesh"
        )));
        assert!(HdStSmoothNormalsSceneIndex::is_mesh_prim(&Token::new(
            "Mesh"
        )));
        assert!(!HdStSmoothNormalsSceneIndex::is_mesh_prim(&Token::new(
            "points"
        )));
        assert!(!HdStSmoothNormalsSceneIndex::is_mesh_prim(&Token::new(
            "basisCurves"
        )));
    }

    #[test]
    fn test_normals_data_source() {
        let ds = SmoothNormalsDataSource::new(false, Token::new("points"), Token::new("normals"));
        let names = ds.get_names();
        assert!(names.contains(&Token::new(tokens::NEEDS_SMOOTH_NORMALS)));
        assert!(names.contains(&Token::new("srcName")));
    }

    #[test]
    fn test_dirty_propagation() {
        let normals_locs = HdStSmoothNormalsSceneIndex::normals_dirty_locators();
        let points_loc = HdDataSourceLocator::from_token(Token::new("points"));
        assert!(normals_locs.intersects_locator(&points_loc));
    }

    #[test]
    fn test_packed_variant() {
        let si = HdStSmoothNormalsSceneIndex::with_packed(None, true);
        let lock = si.read();
        assert!(lock.packed);
    }
}
