
//! Tetrahedral mesh conversion scene index.
//!
//! Converts tetrahedral mesh prims to triangle meshes for rendering.
//! Port of pxr/imaging/hdsi/tetMeshConversionSceneIndex.cpp.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_gf::Vec3i;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_hd::schema::{
    HdMeshSchema, HdMeshTopologySchema, HdMeshTopologyTokenDataSourceHandle, HdTetMeshSchema,
};
use usd_hd::tokens;
use usd_px_osd::tokens::CATMULL_CLARK;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Array;

/// Computes mesh topology data source from tet mesh topology.
///
/// Converts surfaceFaceVertexIndices (Vec<Vec3i>) to faceVertexCounts and faceVertexIndices.
fn compute_mesh_topology_data_source(
    prim_data_source: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let tet_mesh_schema = HdTetMeshSchema::get_from_parent(prim_data_source);
    let mesh_topo_schema = match tet_mesh_schema.get_topology() {
        Some(t) => t,
        None => {
            return HdMeshTopologySchema::build_retained(None, None, None, None);
        }
    };

    let surface_face_indices_ds = match mesh_topo_schema.get_surface_face_vertex_indices() {
        Some(ds) => ds,
        None => {
            let orientation = mesh_topo_schema.get_orientation().map(|ds| {
                let val = ds.get_typed_value(0.0);
                let arc = HdRetainedTypedSampledDataSource::new(val);
                arc as HdMeshTopologyTokenDataSourceHandle
            });
            return HdMeshTopologySchema::build_retained(None, None, None, orientation);
        }
    };

    let surface_face_indices = surface_face_indices_ds.get_typed_value(0.0);
    let n = surface_face_indices.len();

    // faceVertexCounts: all 3 (triangles)
    let face_vertex_count_vals: Array<i32> = (0..n).map(|_| 3i32).collect();
    let face_vertex_counts_ds = HdRetainedTypedSampledDataSource::new(face_vertex_count_vals) as _;

    // faceVertexIndices: flatten Vec3i to i32
    let face_vertex_indices: Array<i32> = surface_face_indices
        .iter()
        .flat_map(|v: &Vec3i| [v[0], v[1], v[2]])
        .collect();
    let face_vertex_indices_ds = HdRetainedTypedSampledDataSource::new(face_vertex_indices) as _;

    let orientation = mesh_topo_schema.get_orientation().map(|ds| {
        let val = ds.get_typed_value(0.0);
        let arc = HdRetainedTypedSampledDataSource::new(val);
        arc as HdMeshTopologyTokenDataSourceHandle
    });

    HdMeshTopologySchema::build_retained(
        Some(face_vertex_counts_ds),
        Some(face_vertex_indices_ds),
        None,
        orientation,
    )
}

/// Mesh data source: topology, subdivisionScheme (catmullClark), doubleSided from tetMesh.
#[derive(Debug, Clone)]
struct MeshDataSource {
    prim_data_source: HdContainerDataSourceHandle,
}

impl HdDataSourceBase for MeshDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(MeshDataSource {
            prim_data_source: self.prim_data_source.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for MeshDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new("topology"),
            TfToken::new("subdivisionScheme"),
            TfToken::new("doubleSided"),
        ]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if *name == TfToken::new("topology") {
            return Some(
                compute_mesh_topology_data_source(&self.prim_data_source) as HdDataSourceBaseHandle
            );
        }
        if *name == TfToken::new("subdivisionScheme") {
            return Some(
                HdRetainedTypedSampledDataSource::new((*CATMULL_CLARK).clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == TfToken::new("doubleSided") {
            let tet_mesh_schema = HdTetMeshSchema::get_from_parent(&self.prim_data_source);
            return tet_mesh_schema
                .get_double_sided()
                .map(|ds| ds as HdDataSourceBaseHandle);
        }
        None
    }
}

fn compute_prim_data_source(
    _prim_path: &SdfPath,
    prim_data_source: Option<&HdContainerDataSourceHandle>,
) -> HdContainerDataSourceHandle {
    let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
    let prim_ds = prim_data_source.unwrap_or(&empty);

    let mesh_container = HdRetainedContainerDataSource::new_1(
        (*HdMeshSchema::get_schema_token()).clone(),
        Arc::new(MeshDataSource {
            prim_data_source: prim_ds.clone(),
        }) as HdContainerDataSourceHandle,
    );
    HdOverlayContainerDataSource::new_2(mesh_container, prim_ds.clone())
}

/// Scene index that converts tetrahedral meshes to triangle meshes for rendering.
pub struct HdsiTetMeshConversionSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiTetMeshConversionSceneIndex {
    /// Creates a new tet mesh conversion scene index.
    /// Creates a new tet mesh conversion scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            let input_guard = input_scene.write();
            input_guard.add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdSceneIndexBase for HdsiTetMeshConversionSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let prim = si_ref(&input).get_prim(prim_path);

        if prim.prim_type == *tokens::RPRIM_TET_MESH {
            return HdSceneIndexPrim {
                prim_type: tokens::RPRIM_MESH.clone(),
                data_source: Some(compute_prim_data_source(
                    prim_path,
                    prim.data_source.as_ref(),
                )),
            };
        }
        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiTetMeshConversionSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiTetMeshConversionSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }

        let tet_mesh_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.prim_type == *tokens::RPRIM_TET_MESH)
            .map(|(i, _)| i)
            .collect();

        if tet_mesh_indices.is_empty() {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let mut entries_to_add = entries.to_vec();
        for i in tet_mesh_indices {
            entries_to_add[i].prim_type = tokens::RPRIM_MESH.clone();
        }
        self.base.forward_prims_added(self, &entries_to_add);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }

        let tet_mesh_schema_locator = HdTetMeshSchema::get_default_locator();
        let double_sided_locator = HdTetMeshSchema::get_double_sided_locator();
        let topology_locator = HdTetMeshSchema::get_topology_locator();
        let mesh_double_sided_locator = HdMeshSchema::get_double_sided_locator();
        let mesh_topology_locator = HdMeshSchema::get_topology_locator();

        let tet_mesh_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if e.dirty_locators.is_universal() {
                    return false;
                }
                e.dirty_locators
                    .intersects_locator(&tet_mesh_schema_locator)
            })
            .map(|(i, _)| i)
            .collect();

        if entries.len() >= 1000 {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[tet_mesh_conversion] on_prims_dirtied in={} tet_hits={} sender={} first={}",
                entries.len(),
                tet_mesh_indices.len(),
                sender.get_display_name(),
                first,
            );
        }

        if tet_mesh_indices.is_empty() {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        let mut new_entries = entries.to_vec();
        for i in tet_mesh_indices {
            let entry = &mut new_entries[i];
            if entry
                .dirty_locators
                .intersects_locator(&double_sided_locator)
            {
                entry
                    .dirty_locators
                    .insert(mesh_double_sided_locator.clone());
            }
            if entry.dirty_locators.intersects_locator(&topology_locator) {
                entry.dirty_locators.insert(mesh_topology_locator.clone());
            }
        }
        self.base.forward_prims_dirtied(self, &new_entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        self.base.forward_prims_renamed(self, entries);
    }
}
