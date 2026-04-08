//! XformResolver - Transform from prim local to common space.
//!
//! Port of pxr/usdImaging/usdSkelImaging/xformResolver.h/cpp
//!
//! Given a prim, computes transform from prim local space to a space common to
//! all descendants of a skel root. Handles ni/pi instancing.

use super::binding_schema::BindingSchema;
use super::data_source_utils::get_typed_value_from_container_vec_path;
use std::sync::Arc;
use usd_gf::Matrix4d;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedTypedSampledDataSource, HdSampledDataSource, HdSampledDataSourceTime,
    HdTypedSampledDataSource, HdValueExtract,
};
use usd_hd::schema::HdMatrixDataSourceHandle;
use usd_hd::schema::{HdInstancedBySchema, HdPrimvarsSchema, HdXformSchema};
use usd_hd::tokens::INSTANCE_TRANSFORMS;
use usd_sdf::Path;
use usd_skel::tokens::tokens as usd_skel_tokens;
use usd_tf::Token;
use usd_vt::Value;

type HdSceneIndexHandle = usd_hd::scene_index::HdSceneIndexHandle;
use usd_hd::scene_index::si_ref;

#[derive(Clone)]
struct MatrixProductDataSource {
    matrices: Vec<HdMatrixDataSourceHandle>,
}

impl MatrixProductDataSource {
    fn new(matrices: Vec<HdMatrixDataSourceHandle>) -> Arc<Self> {
        Arc::new(Self { matrices })
    }
}

impl std::fmt::Debug for MatrixProductDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixProductDataSource").finish()
    }
}

impl HdDataSourceBase for MatrixProductDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from(self.get_typed_value(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn as_matrix_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Matrix4d> + Send + Sync>> {
        Some(Arc::new(self.clone()))
    }
}

impl HdSampledDataSource for MatrixProductDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let mut all_times = Vec::new();
        let mut varying = false;
        for matrix in &self.matrices {
            let mut sample_times = Vec::new();
            if matrix.get_contributing_sample_times(start_time, end_time, &mut sample_times) {
                varying = true;
                all_times.extend(sample_times);
            }
        }
        if !varying {
            out_sample_times.clear();
            return false;
        }
        all_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        all_times.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
        *out_sample_times = all_times;
        true
    }
}

impl HdTypedSampledDataSource<Matrix4d> for MatrixProductDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Matrix4d {
        self.matrices
            .iter()
            .fold(Matrix4d::identity(), |acc, matrix| {
                acc * matrix.get_typed_value(shutter_offset)
            })
    }
}

/// Compute instancer paths by following instancedBy schema.
/// Stops when prototype has no SkelRoot.
fn compute_instancer_paths(
    scene_handle: &HdSceneIndexHandle,
    prim_source: &HdContainerDataSourceHandle,
) -> Vec<Path> {
    let mut result = Vec::new();
    let mut current_source = prim_source.clone();

    loop {
        let instanced_by = HdInstancedBySchema::get_from_parent(&current_source);
        if !instanced_by.is_defined() {
            break;
        }
        let Some(prototype_roots_ds) = instanced_by.get_prototype_roots() else {
            break;
        };
        let prototype_roots = prototype_roots_ds.get_typed_value(0.0);
        if prototype_roots.is_empty() {
            break;
        }
        let prototype_root = &prototype_roots[0];
        let prototype_prim = si_ref(&scene_handle).get_prim(prototype_root);
        let Some(ref proto_ds) = prototype_prim.data_source else {
            break;
        };
        let binding = BindingSchema::get_from_parent(proto_ds);
        if !binding.get_has_skel_root() {
            break;
        }
        let Some(paths_ds) = instanced_by.get_paths() else {
            break;
        };
        let paths = paths_ds.get_typed_value(0.0);
        if paths.is_empty() {
            break;
        }
        let instancer_path = paths[0].clone();
        let instancer_prim = si_ref(&scene_handle).get_prim(&instancer_path);
        let Some(instancer_ds) = instancer_prim.data_source else {
            break;
        };
        current_source = instancer_ds;
        result.push(instancer_path);
    }
    result
}

/// Resolves prim local space to common space (world or prototype).
///
/// Common space is world without instancing. If skel root is inside
/// ni/pi prototype, common space is that of the prototype.
pub struct DataSourceXformResolver {
    scene_handle: HdSceneIndexHandle,
    prim_source: HdContainerDataSourceHandle,
    instancer_paths: Vec<Path>,
}

impl std::fmt::Debug for DataSourceXformResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceXformResolver")
            .field("instancer_paths", &self.instancer_paths)
            .finish()
    }
}

impl DataSourceXformResolver {
    /// Construct from scene index handle and prim data source.
    pub fn new(scene_handle: HdSceneIndexHandle, prim_source: HdContainerDataSourceHandle) -> Self {
        let instancer_paths = compute_instancer_paths(&scene_handle, &prim_source);
        Self {
            scene_handle,
            prim_source,
            instancer_paths,
        }
    }

    /// Data source for the transform (prim local to common space).
    ///
    /// Composes xform from prim and instancer instance transforms.
    pub fn get_prim_local_to_common_space(&self) -> Option<HdMatrixDataSourceHandle> {
        let prim_xform = HdXformSchema::get_from_parent(&self.prim_source);
        let prim_matrix = prim_xform.get_matrix();

        if self.instancer_paths.is_empty() {
            return prim_matrix;
        }

        let mut xform_srcs: Vec<HdMatrixDataSourceHandle> = Vec::new();
        if let Some(ref m) = prim_matrix {
            xform_srcs.push(m.clone());
        }

        let guard = self.scene_handle.read();

        for instancer_path in &self.instancer_paths {
            let instancer_prim = guard.get_prim(instancer_path);
            let Some(ref instancer_ds) = instancer_prim.data_source else {
                continue;
            };
            let instancer_primvars = HdPrimvarsSchema::get_from_parent(instancer_ds);
            let instance_transforms = instancer_primvars.get_primvar(&INSTANCE_TRANSFORMS);
            if let Some(primvar_container) = instance_transforms {
                if let Some(value_ds) = primvar_container.get(&Token::new("primvarValue")) {
                    if let Some(sampled) = value_ds.as_sampled() {
                        let value = sampled.get_value(0.0);
                        if let Some(mats) = Vec::<Matrix4d>::extract(&value) {
                            if let Some(first) = mats.first().copied() {
                                let retained = HdRetainedTypedSampledDataSource::new(first);
                                let matrix_ds: HdMatrixDataSourceHandle =
                                    retained as Arc<dyn HdTypedSampledDataSource<Matrix4d>>;
                                xform_srcs.push(matrix_ds);
                            }
                        }
                    }
                }
            }
            let instancer_xform = HdXformSchema::get_from_parent(instancer_ds);
            if let Some(m) = instancer_xform.get_matrix() {
                xform_srcs.push(m);
            }
        }

        match xform_srcs.len() {
            0 => None,
            1 => Some(xform_srcs.into_iter().next().unwrap()),
            _ => Some(MatrixProductDataSource::new(xform_srcs) as HdMatrixDataSourceHandle),
        }
    }

    /// Get skel:animationSource instance primvar value from instancer.
    pub fn get_instance_animation_source(&self) -> Vec<Path> {
        let guard = self.scene_handle.read();
        let skel_anim = usd_skel_tokens().skel_animation_source.clone();
        for instancer_path in &self.instancer_paths {
            let instancer_prim = guard.get_prim(instancer_path);
            let Some(ref instancer_ds) = instancer_prim.data_source else {
                continue;
            };
            let primvars = HdPrimvarsSchema::get_from_parent(instancer_ds);
            let anim_primvar = primvars.get_primvar(&skel_anim);
            if let Some(pv_container) = anim_primvar {
                if let Some(paths) = get_typed_value_from_container_vec_path(
                    &pv_container,
                    &Token::new("primvarValue"),
                ) {
                    if !paths.is_empty() {
                        return paths;
                    }
                }
            }
        }
        Vec::new()
    }

    /// Paths of instancers contributing to the transform.
    pub fn instancer_paths(&self) -> &[Path] {
        &self.instancer_paths
    }

    /// Locator requiring resolver reconstruction when dirty.
    pub fn get_instanced_by_locator() -> HdDataSourceLocator {
        HdInstancedBySchema::get_default_locator()
    }

    /// Locator requiring transform refetch when dirty.
    pub fn get_xform_locator() -> HdDataSourceLocator {
        HdXformSchema::get_default_locator()
    }

    /// Instance xform locator (on instancer). primvars/hydra:instanceTransforms
    pub fn get_instance_xform_locator() -> HdDataSourceLocator {
        HdPrimvarsSchema::get_default_locator().append(&*INSTANCE_TRANSFORMS)
    }

    /// Instance animation source locator.
    pub fn get_instance_animation_source_locator() -> HdDataSourceLocator {
        HdPrimvarsSchema::get_default_locator().append(&usd_skel_tokens().skel_animation_source)
    }
}
