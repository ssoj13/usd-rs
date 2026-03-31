
//! NURBS approximating scene index.
//!
//! Converts NURBS curves to basis curves and NURBS patches to meshes.
//! Port of pxr/imaging/hdsi/nurbsApproximatingSceneIndex.cpp.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdBlockDataSource, HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase,
    HdDataSourceBaseHandle, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexObserverHandle,
    HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_hd::schema::{
    HdBasisCurvesSchema, HdBasisCurvesTopologySchema, HdDependenciesSchema,
    HdDependencySchemaBuilder, HdMeshSchema, HdMeshTopologySchema, HdNurbsCurvesSchema,
    HdNurbsPatchSchema,
};
use usd_hd::tokens;
use usd_px_osd::tokens::CATMULL_CLARK;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Array;

// --- NURBS Curves → Basis Curves ---

fn compute_nurbs_curves_to_basis_curves_dependencies(
    prim_path: &SdfPath,
) -> HdContainerDataSourceHandle {
    let curve_vertex_counts_source_locator = HdNurbsCurvesSchema::get_curve_vertex_counts_locator();
    let affected_locator = HdBasisCurvesTopologySchema::get_default_locator()
        .append(&TfToken::new("curveVertexCounts"));

    let dep = HdDependencySchemaBuilder::default()
        .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::new(prim_path.clone())
            as usd_hd::schema::HdPathDataSourceHandle)
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            curve_vertex_counts_source_locator,
        ) as usd_hd::schema::HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(affected_locator)
            as usd_hd::schema::HdLocatorDataSourceHandle)
        .build();
    HdDependenciesSchema::build_retained(
        &[TfToken::new("curveVertexCounts")],
        &[dep as HdDataSourceBaseHandle],
    )
}

/// Basis curves topology: curveVertexCounts from nurbs, basis=linear, type=linear, wrap=nonperiodic.
#[derive(Debug, Clone)]
struct BasisCurvesTopologyDataSource {
    prim_data_source: HdContainerDataSourceHandle,
}

impl HdDataSourceBase for BasisCurvesTopologyDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(BasisCurvesTopologyDataSource {
            prim_data_source: self.prim_data_source.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for BasisCurvesTopologyDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new("curveVertexCounts"),
            TfToken::new("basis"),
            TfToken::new("type"),
            TfToken::new("wrap"),
        ]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if *name == TfToken::new("curveVertexCounts") {
            let schema = HdNurbsCurvesSchema::get_from_parent(&self.prim_data_source);
            return schema.get_curve_vertex_counts().map(|ds| {
                let val = ds.get_typed_value(0.0);
                HdRetainedTypedSampledDataSource::new(val) as HdDataSourceBaseHandle
            });
        }
        if *name == TfToken::new("basis") || *name == TfToken::new("type") {
            return Some(
                HdRetainedTypedSampledDataSource::new(tokens::LINEAR.clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == TfToken::new("wrap") {
            return Some(
                HdRetainedTypedSampledDataSource::new(TfToken::new("nonperiodic"))
                    as HdDataSourceBaseHandle,
            );
        }
        None
    }
}

fn compute_nurbs_curves_to_basis_curves_prim_data_source(
    prim_path: &SdfPath,
    prim_data_source: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let block = HdBlockDataSource::new();
    let basis_curves_topo = HdRetainedContainerDataSource::new_1(
        TfToken::new("topology"),
        Arc::new(BasisCurvesTopologyDataSource {
            prim_data_source: prim_data_source.clone(),
        }) as HdDataSourceBaseHandle,
    );
    let basis_curves_container = HdRetainedContainerDataSource::new_1(
        (*HdBasisCurvesSchema::get_schema_token()).clone(),
        basis_curves_topo as HdDataSourceBaseHandle,
    );
    let dependencies = compute_nurbs_curves_to_basis_curves_dependencies(prim_path);

    let overlay_top = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdNurbsCurvesSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdBasisCurvesSchema::get_schema_token()).clone(),
            basis_curves_container as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            dependencies as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay_top, prim_data_source.clone())
}

// --- NURBS Patch → Mesh ---

fn get_uv_vertex_counts(prim_data_source: &HdContainerDataSourceHandle) -> (i32, i32) {
    let schema = HdNurbsPatchSchema::get_from_parent(prim_data_source);
    let u = schema
        .get_u_vertex_count()
        .map(|ds| ds.get_typed_value(0.0))
        .unwrap_or(0);
    let v = schema
        .get_v_vertex_count()
        .map(|ds| ds.get_typed_value(0.0))
        .unwrap_or(0);
    (u, v)
}

fn compute_nurbs_patch_to_mesh_dependencies(prim_path: &SdfPath) -> HdContainerDataSourceHandle {
    let basis_curves_to_mesh = TfToken::new("basisCurvesToMesh");
    let depended_on_locator = HdNurbsPatchSchema::get_default_locator();
    let affected_locator = HdMeshSchema::get_default_locator();

    let dep = HdDependencySchemaBuilder::default()
        .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::new(prim_path.clone())
            as usd_hd::schema::HdPathDataSourceHandle)
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            depended_on_locator,
        ) as usd_hd::schema::HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(affected_locator)
            as usd_hd::schema::HdLocatorDataSourceHandle)
        .build();
    HdDependenciesSchema::build_retained(&[basis_curves_to_mesh], &[dep as HdDataSourceBaseHandle])
}

/// Mesh topology from NURBS patch: faceVertexCounts (4 per quad), faceVertexIndices, orientation.
#[derive(Debug, Clone)]
struct NurbsPatchMeshTopologyDataSource {
    prim_data_source: HdContainerDataSourceHandle,
}

impl HdDataSourceBase for NurbsPatchMeshTopologyDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(NurbsPatchMeshTopologyDataSource {
            prim_data_source: self.prim_data_source.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for NurbsPatchMeshTopologyDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new("faceVertexCounts"),
            TfToken::new("faceVertexIndices"),
            TfToken::new("orientation"),
        ]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let (u, v) = get_uv_vertex_counts(&self.prim_data_source);
        let u_safe = u.max(0) as usize;
        let v_safe = v.max(0) as usize;
        let num_faces = (u as usize).saturating_sub(1) * (v as usize).saturating_sub(1);

        if *name == TfToken::new("faceVertexCounts") {
            let face_vertex_count_vals: Array<i32> = (0..num_faces).map(|_| 4i32).collect();
            return Some(
                HdRetainedTypedSampledDataSource::new(face_vertex_count_vals)
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == TfToken::new("faceVertexIndices") {
            let num_indices = 4 * num_faces;
            let mut face_vertex_indices: Vec<i32> = Vec::with_capacity(num_indices);
            if num_faces > 0 && u_safe > 1 && v_safe > 1 {
                for row in 0..(v_safe - 1) {
                    for col in 0..(u_safe - 1) {
                        let vertex_idx = row * u_safe + col;
                        face_vertex_indices.push(vertex_idx as i32);
                        face_vertex_indices.push((vertex_idx + 1) as i32);
                        face_vertex_indices.push((vertex_idx + u_safe + 1) as i32);
                        face_vertex_indices.push((vertex_idx + u_safe) as i32);
                    }
                }
            } else {
                face_vertex_indices.resize(num_indices, 0);
            }
            let arr: Array<i32> = face_vertex_indices.into_iter().collect();
            return Some(HdRetainedTypedSampledDataSource::new(arr) as HdDataSourceBaseHandle);
        }
        if *name == TfToken::new("orientation") {
            let schema = HdNurbsPatchSchema::get_from_parent(&self.prim_data_source);
            return schema.get_orientation().map(|ds| {
                let val = ds.get_typed_value(0.0);
                let arc = HdRetainedTypedSampledDataSource::new(val);
                arc as HdDataSourceBaseHandle
            });
        }
        None
    }
}

fn compute_nurbs_patch_mesh_data_source(
    prim_data_source: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topology = HdRetainedContainerDataSource::new_1(
        HdMeshTopologySchema::get_default_locator()
            .first_element()
            .cloned()
            .unwrap_or_else(|| TfToken::new("topology")),
        Arc::new(NurbsPatchMeshTopologyDataSource {
            prim_data_source: prim_data_source.clone(),
        }) as HdDataSourceBaseHandle,
    );

    let schema = HdNurbsPatchSchema::get_from_parent(prim_data_source);
    let double_sided = schema.get_double_sided();

    HdMeshSchema::build_retained(
        Some(topology),
        Some(
            HdRetainedTypedSampledDataSource::new((*CATMULL_CLARK).clone())
                as usd_hd::schema::HdMeshTopologyTokenDataSourceHandle,
        ),
        None,
        double_sided,
    )
}

fn compute_nurbs_patch_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_data_source: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let block = HdBlockDataSource::new();
    let mesh_data = compute_nurbs_patch_mesh_data_source(prim_data_source);
    let dependencies = compute_nurbs_patch_to_mesh_dependencies(prim_path);

    let overlay_top = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdNurbsPatchSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_data as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            dependencies as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay_top, prim_data_source.clone())
}

/// Scene index that converts NURBS curves to basis curves and NURBS patches to meshes.
pub struct HdsiNurbsApproximatingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiNurbsApproximatingSceneIndex {
    /// Creates a new NURBS approximating scene index.
    /// Creates a new NURBS approximating scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdSceneIndexBase for HdsiNurbsApproximatingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let prim = si_ref(&input).get_prim(prim_path);

        if prim.prim_type == *tokens::RPRIM_NURBS_CURVES {
            return HdSceneIndexPrim {
                prim_type: tokens::RPRIM_BASIS_CURVES.clone(),
                data_source: prim
                    .data_source
                    .as_ref()
                    .map(|ds| compute_nurbs_curves_to_basis_curves_prim_data_source(prim_path, ds)),
            };
        }
        if prim.prim_type == *tokens::RPRIM_NURBS_PATCH {
            return HdSceneIndexPrim {
                prim_type: tokens::RPRIM_MESH.clone(),
                data_source: prim
                    .data_source
                    .as_ref()
                    .map(|ds| compute_nurbs_patch_to_mesh_prim_data_source(prim_path, ds)),
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

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiNurbsApproximatingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiNurbsApproximatingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }

        let mut indices = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if e.prim_type == *tokens::RPRIM_NURBS_CURVES
                || e.prim_type == *tokens::RPRIM_NURBS_PATCH
            {
                indices.push(i);
            }
        }

        if indices.is_empty() {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let mut new_entries = entries.to_vec();
        for i in indices {
            if new_entries[i].prim_type == *tokens::RPRIM_NURBS_CURVES {
                new_entries[i].prim_type = tokens::RPRIM_BASIS_CURVES.clone();
            } else {
                new_entries[i].prim_type = tokens::RPRIM_MESH.clone();
            }
        }
        self.base.forward_prims_added(self, &new_entries);
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
        if entries.len() >= 1000 {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[nurbs_approximating] on_prims_dirtied in={} sender={} first={}",
                entries.len(),
                sender.get_display_name(),
                first,
            );
        }
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        self.base.forward_prims_renamed(self, entries);
    }
}
