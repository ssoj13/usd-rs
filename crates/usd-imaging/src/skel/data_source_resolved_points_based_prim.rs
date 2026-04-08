//! DataSourceResolvedPointsBasedPrim - Resolved data for skinned points-based prims.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceResolvedPointsBasedPrim.h
//!
//! Provides resolved data for mesh/basisCurves/points deformed by skeleton.

use super::binding_schema::BindingSchema;
use super::blend_shape_data::BlendShapeData;
use super::blend_shape_schema::BlendShapeSchema;
use super::data_source_utils::get_typed_value_from_container_token;
use super::joint_influences_data::JointInfluencesData;
use super::resolved_skeleton_schema::ResolvedSkeletonSchema;
use super::tokens::{EXT_COMPUTATION_INPUT_TOKENS, EXT_COMPUTATION_NAME_TOKENS, PRIM_TYPE_TOKENS};
use super::xform_resolver::DataSourceXformResolver;
use std::sync::Arc;
use usd_gf::vec4::Vec4f;
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdDataSourceLocator, HdRetainedTypedSampledDataSource,
    HdSampledDataSource, HdValueExtract,
};
use usd_hd::scene_index::observer::DirtiedPrimEntry;
use usd_hd::schema::{HdExtComputationSchema, HdMeshSchema, HdPrimvarsSchema};
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_sdf::Path;
use usd_tf::Token;

type HdSceneIndexHandle = usd_hd::scene_index::HdSceneIndexHandle;

fn points_primvar_value_locator() -> HdDataSourceLocator {
    HdPrimvarsSchema::get_default_locator()
        .append(&Token::new("points"))
        .append(&Token::new("primvarValue"))
}

fn normals_primvar_value_locator() -> HdDataSourceLocator {
    HdPrimvarsSchema::get_default_locator()
        .append(&Token::new("normals"))
        .append(&Token::new("primvarValue"))
}

fn ext_comp_input_locator(name: &Token) -> HdDataSourceLocator {
    HdExtComputationSchema::get_input_values_locator().append(name)
}

fn ext_comp_prim_path(prim_path: &Path, name: &Token) -> Option<Path> {
    prim_path.append_child(name.as_str())
}

/// Resolved data source for points-based prim (mesh, basisCurves, points) deformed by skeleton.
///
/// Populates HdExtComputationPrimvarsSchema for points, removes points from HdPrimvarsSchema.
pub struct DataSourceResolvedPointsBasedPrim {
    /// Path of prim in input scene.
    pub prim_path: Path,

    /// Path of bound skeleton.
    pub skeleton_path: Path,

    /// Paths to BlendShape prims.
    pub blend_shape_target_paths: Vec<Path>,

    /// Primvars schema from input.
    pub primvars: HdPrimvarsSchema,

    /// Resolved skeleton schema.
    pub resolved_skeleton_schema: ResolvedSkeletonSchema,

    /// Skinning method.
    pub skinning_method: Token,

    /// Whether prim is under a SkelRoot.
    pub has_skel_root: bool,

    /// Prim source (input data source) for joint influences lookup.
    pub prim_source: HdContainerDataSourceHandle,

    /// Skeleton prim source for joint influences lookup.
    pub skeleton_prim_source: HdContainerDataSourceHandle,

    /// Xform resolver for prim local to common space and instancer paths.
    pub xform_resolver: DataSourceXformResolver,

    /// Scene index handle for blend shape data and other lookups.
    scene_handle: HdSceneIndexHandle,
}

impl std::fmt::Debug for DataSourceResolvedPointsBasedPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceResolvedPointsBasedPrim")
            .field("prim_path", &self.prim_path)
            .field("skeleton_path", &self.skeleton_path)
            .field("blend_shape_target_paths", &self.blend_shape_target_paths)
            .field("skinning_method", &self.skinning_method)
            .field("has_skel_root", &self.has_skel_root)
            .field("xform_resolver", &self.xform_resolver)
            .finish()
    }
}

impl DataSourceResolvedPointsBasedPrim {
    /// Create resolved points-based prim from scene (port of New overload).
    ///
    /// Resolves binding schema from the prim data source and fetches the bound
    /// skeleton from the input scene.
    ///
    /// This intentionally matches `_ref`:
    /// `UsdSkelImagingDataSourceResolvedPointsBasedPrim::New(...)` expects the
    /// incoming prim data source to already carry the resolved binding schema
    /// (`hasSkelRoot`, `skeleton`, `blendShapeTargets`). It does not walk
    /// ancestors trying to recover missing binding state. Doing that in Rust
    /// introduced a second, divergent binding-resolution policy on top of the
    /// upstream scene-index chain and is the same class of relative-path bug we
    /// already hit in `UsdSkelTopology`.
    pub fn new_from_scene(
        scene_handle: HdSceneIndexHandle,
        prim_path: &Path,
        prim_source: &HdContainerDataSourceHandle,
    ) -> Option<Arc<Self>> {
        let (
            skeleton_path,
            has_skel_root,
            blend_shape_target_paths,
            skeleton_prim_source,
            resolved_skeleton_schema,
        ) = {
            let scene_guard = scene_handle.read();
            let scene_index = &*scene_guard;

            let binding_schema = BindingSchema::get_from_parent(prim_source);
            if !binding_schema.is_defined() {
                return None;
            }
            let skeleton_path = binding_schema.get_skeleton()?;
            let has_skel_root = binding_schema.get_has_skel_root();
            let blend_shape_target_paths = binding_schema.get_blend_shape_targets();
            if skeleton_path.is_empty() {
                return None;
            }

            let skeleton_prim = scene_index.get_prim(&skeleton_path);
            let skeleton_prim_source = skeleton_prim.data_source.clone()?;
            let resolved_skeleton_schema =
                ResolvedSkeletonSchema::get_from_parent(&skeleton_prim_source);
            if !resolved_skeleton_schema.is_defined() {
                return None;
            }

            (
                skeleton_path,
                has_skel_root,
                blend_shape_target_paths,
                skeleton_prim_source,
                resolved_skeleton_schema,
            )
        };

        Self::new(
            scene_handle,
            prim_path.clone(),
            prim_source.clone(),
            has_skel_root,
            blend_shape_target_paths,
            skeleton_path,
            skeleton_prim_source,
            resolved_skeleton_schema,
        )
    }

    /// Create new resolved points-based prim with pre-resolved values.
    ///
    /// Returns None if has_skel_root is false.
    pub fn new(
        scene_handle: HdSceneIndexHandle,
        prim_path: Path,
        prim_source: HdContainerDataSourceHandle,
        has_skel_root: bool,
        blend_shape_target_paths: Vec<Path>,
        skeleton_path: Path,
        skeleton_prim_source: HdContainerDataSourceHandle,
        resolved_skeleton_schema: ResolvedSkeletonSchema,
    ) -> Option<Arc<Self>> {
        if !has_skel_root {
            return None;
        }
        let primvars = HdPrimvarsSchema::get_from_parent(&prim_source);
        let skinning_method = Self::get_skinning_method_from_primvars(&primvars, &prim_path);
        let xform_resolver =
            DataSourceXformResolver::new(scene_handle.clone(), prim_source.clone());
        Some(Arc::new(Self {
            prim_path: prim_path.clone(),
            skeleton_path,
            blend_shape_target_paths,
            primvars,
            resolved_skeleton_schema,
            skinning_method,
            has_skel_root,
            prim_source,
            skeleton_prim_source,
            xform_resolver,
            scene_handle,
        }))
    }

    fn get_skinning_method_from_primvars(primvars: &HdPrimvarsSchema, _prim_path: &Path) -> Token {
        use super::data_source_utils::{PRIMVAR_VALUE, get_typed_value_from_container_token};

        let method_token = BindingSchema::get_skinning_method_primvar_token();
        let primvar = primvars.get_primvar(&method_token);
        let Some(primvar_cont) = primvar else {
            return Token::new("classicLinear");
        };
        get_typed_value_from_container_token(&primvar_cont, &*PRIMVAR_VALUE)
            .filter(|m| !m.as_str().is_empty())
            .filter(|m| m == "classicLinear" || m == "dualQuaternion")
            .unwrap_or_else(|| Token::new("classicLinear"))
    }

    /// Get prim path.
    pub fn get_prim_path(&self) -> &Path {
        &self.prim_path
    }

    /// Get skeleton path.
    pub fn get_skeleton_path(&self) -> &Path {
        &self.skeleton_path
    }

    /// Get blend shape target paths.
    pub fn get_blend_shape_target_paths(&self) -> &[Path] {
        &self.blend_shape_target_paths
    }

    /// Paths to instancers instancing this prim (not including ones outside skel root).
    pub fn get_instancer_paths(&self) -> &[Path] {
        self.xform_resolver.instancer_paths()
    }

    /// Get primvars schema.
    pub fn get_primvars(&self) -> &HdPrimvarsSchema {
        &self.primvars
    }

    /// Get resolved skeleton schema.
    pub fn get_resolved_skeleton_schema(&self) -> &ResolvedSkeletonSchema {
        &self.resolved_skeleton_schema
    }

    /// Get skinning method.
    pub fn get_skinning_method(&self) -> &Token {
        &self.skinning_method
    }

    /// Get locators this prim depends on.
    pub fn get_dependendend_on_data_source_locators() -> HdDataSourceLocatorSet {
        let mut result = HdDataSourceLocatorSet::new();
        result.insert(BindingSchema::get_default_locator());
        result.insert(HdPrimvarsSchema::get_default_locator());
        result.insert(DataSourceXformResolver::get_xform_locator());
        result
    }

    /// Process dirty locators and emit translated dirties for resolved outputs.
    ///
    /// Returns `true` when the resolved prim must be reconstructed.
    pub fn process_dirty_locators(
        &self,
        dirtied_prim_type: &Token,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: Option<&mut Vec<DirtiedPrimEntry>>,
    ) -> bool {
        let mut dirty_computation_inputs = HdDataSourceLocatorSet::new();
        let mut dirty_aggregator_inputs = HdDataSourceLocatorSet::new();
        let mut dirty_primvar_values = HdDataSourceLocatorSet::new();

        let mut mark_points_and_normals_dirty = || {
            dirty_primvar_values.insert(points_primvar_value_locator());
            dirty_primvar_values.insert(normals_primvar_value_locator());
        };

        let mut resync = false;

        if *dirtied_prim_type == PRIM_TYPE_TOKENS.skeleton {
            if dirty_locators.contains(&ResolvedSkeletonSchema::get_default_locator()) {
                resync = true;
            }
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                ResolvedSkeletonSchema::get_skel_local_to_common_space_locator(),
            )) {
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.skel_local_to_common_space,
                ));
                mark_points_and_normals_dirty();
            }
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                ResolvedSkeletonSchema::get_skinning_transforms_locator(),
            )) {
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms,
                ));
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.skinning_scale_xforms,
                ));
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.skinning_dual_quats,
                ));
                mark_points_and_normals_dirty();
            }
            let mut blend_locators = HdDataSourceLocatorSet::new();
            blend_locators.insert(ResolvedSkeletonSchema::get_blend_shapes_locator());
            blend_locators.insert(ResolvedSkeletonSchema::get_blend_shape_weights_locator());
            blend_locators.insert(ResolvedSkeletonSchema::get_blend_shape_ranges_locator());
            if dirty_locators.intersects(&blend_locators) {
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights,
                ));
                mark_points_and_normals_dirty();
            }
        } else if *dirtied_prim_type == PRIM_TYPE_TOKENS.skel_blend_shape {
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                BlendShapeSchema::get_default_locator(),
            )) {
                dirty_aggregator_inputs.insert(HdExtComputationSchema::get_input_values_locator());
                mark_points_and_normals_dirty();
            }
        } else if *dirtied_prim_type == Token::new("instancer") {
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                DataSourceXformResolver::get_instanced_by_locator(),
            )) {
                resync = true;
            }
            let mut instancer_locators = HdDataSourceLocatorSet::new();
            instancer_locators.insert(DataSourceXformResolver::get_xform_locator());
            instancer_locators.insert(DataSourceXformResolver::get_instance_xform_locator());
            if dirty_locators.intersects(&instancer_locators) {
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.common_space_to_prim_local,
                ));
                mark_points_and_normals_dirty();
            }
        } else {
            if dirty_locators.contains(&BindingSchema::get_skeleton_locator())
                || dirty_locators.contains(&HdPrimvarsSchema::get_default_locator())
                || dirty_locators.contains(&DataSourceXformResolver::get_instanced_by_locator())
            {
                resync = true;
            }

            let mut joint_influence_locators = HdDataSourceLocatorSet::new();
            joint_influence_locators.insert(
                HdPrimvarsSchema::get_default_locator()
                    .append(&BindingSchema::get_joint_indices_primvar_token()),
            );
            joint_influence_locators.insert(
                HdPrimvarsSchema::get_default_locator()
                    .append(&BindingSchema::get_joint_weights_primvar_token()),
            );
            joint_influence_locators.insert(BindingSchema::get_joints_locator());
            if dirty_locators.intersects(&joint_influence_locators) {
                dirty_aggregator_inputs.insert(HdExtComputationSchema::get_input_values_locator());
                mark_points_and_normals_dirty();
            }

            let points_locator =
                HdPrimvarsSchema::get_default_locator().append(&Token::new("points"));
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(points_locator)) {
                dirty_aggregator_inputs.insert(HdExtComputationSchema::get_input_values_locator());
                mark_points_and_normals_dirty();
            }

            let normals_locator =
                HdPrimvarsSchema::get_default_locator().append(&Token::new("normals"));
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(normals_locator)) {
                dirty_aggregator_inputs.insert(HdExtComputationSchema::get_input_values_locator());
                mark_points_and_normals_dirty();
            }

            let geom_bind_locator = HdPrimvarsSchema::get_default_locator()
                .append(&BindingSchema::get_geom_bind_transform_primvar_token());
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(geom_bind_locator)) {
                dirty_aggregator_inputs.insert(HdExtComputationSchema::get_input_values_locator());
                mark_points_and_normals_dirty();
            }

            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                DataSourceXformResolver::get_xform_locator(),
            )) {
                dirty_computation_inputs.insert(ext_comp_input_locator(
                    &EXT_COMPUTATION_INPUT_TOKENS.common_space_to_prim_local,
                ));
                mark_points_and_normals_dirty();
            }
        }

        if let Some(entries) = entries {
            if self.has_ext_computations() {
                if !dirty_aggregator_inputs.is_empty() {
                    if let Some(points_agg) = ext_comp_prim_path(
                        &self.prim_path,
                        &EXT_COMPUTATION_NAME_TOKENS.points_aggregator_computation,
                    ) {
                        entries.push(DirtiedPrimEntry::new(
                            points_agg,
                            dirty_aggregator_inputs.clone(),
                        ));
                    }
                    if let Some(normals_agg) = ext_comp_prim_path(
                        &self.prim_path,
                        &EXT_COMPUTATION_NAME_TOKENS.normals_aggregator_computation,
                    ) {
                        entries.push(DirtiedPrimEntry::new(
                            normals_agg,
                            dirty_aggregator_inputs.clone(),
                        ));
                    }
                }
                if !dirty_computation_inputs.is_empty() {
                    if let Some(points_comp) = ext_comp_prim_path(
                        &self.prim_path,
                        &EXT_COMPUTATION_NAME_TOKENS.points_computation,
                    ) {
                        entries.push(DirtiedPrimEntry::new(
                            points_comp,
                            dirty_computation_inputs.clone(),
                        ));
                    }
                    if let Some(normals_comp) = ext_comp_prim_path(
                        &self.prim_path,
                        &EXT_COMPUTATION_NAME_TOKENS.normals_computation,
                    ) {
                        entries.push(DirtiedPrimEntry::new(
                            normals_comp,
                            dirty_computation_inputs.clone(),
                        ));
                    }
                }
            }

            if !dirty_primvar_values.is_empty() {
                entries.push(DirtiedPrimEntry::new(
                    self.prim_path.clone(),
                    dirty_primvar_values,
                ));
            }
        }

        resync
    }

    /// Check if this prim has ext computations.
    ///
    /// Ext computations are used when skinning is not deferred and this prim
    /// binds a skeleton (and is not the Skeleton prim itself) under a SkelRoot.
    pub fn has_ext_computations(&self) -> bool {
        !usd_hd::skinning_settings::is_skinning_deferred()
            && self.resolved_skeleton_schema.is_defined()
            && self.prim_path != self.skeleton_path
            && self.has_skel_root
    }

    /// Get blend shape data (cached).
    pub fn get_blend_shape_data(&self) -> Arc<BlendShapeData> {
        Arc::new(super::blend_shape_data::compute_blend_shape_data(
            &self.scene_handle,
            &self.prim_path,
        ))
    }

    /// Get joint influences data (cached).
    pub fn get_joint_influences_data(&self) -> Arc<JointInfluencesData> {
        Arc::new(super::joint_influences_data::compute_joint_influences_data(
            &self.prim_source,
            &self.skeleton_prim_source,
        ))
    }

    /// Wrap value as retained data source (port of _ToDataSource).
    fn to_data_source<T>(value: T) -> HdDataSourceBaseHandle
    where
        T: Clone + Send + Sync + std::fmt::Debug + 'static,
        HdRetainedTypedSampledDataSource<T>: HdSampledDataSource,
    {
        HdRetainedTypedSampledDataSource::new(value) as HdDataSourceBaseHandle
    }

    /// Get skinning transforms (with joint mapper remapping if needed).
    pub fn get_skinning_transforms(
        &self,
    ) -> Option<
        Arc<
            dyn usd_hd::data_source::HdTypedSampledDataSource<Vec<usd_gf::matrix4::Matrix4f>>
                + Send
                + Sync,
        >,
    > {
        let skel_xforms = self.resolved_skeleton_schema.get_skinning_transforms()?;
        let joint_data = self.get_joint_influences_data();
        if joint_data.joint_mapper.is_null() || joint_data.joint_mapper.is_identity() {
            return Some(skel_xforms);
        }
        Some(
            super::resolved_points_based_sources::SkinngXformsDataSource::new(
                joint_data,
                skel_xforms,
            ),
        )
    }

    /// Get skinning scale transforms (3x3 from 4x4).
    pub fn get_skinning_scale_transforms(
        &self,
    ) -> Option<
        Arc<
            dyn usd_hd::data_source::HdTypedSampledDataSource<Vec<usd_gf::matrix3::Matrix3f>>
                + Send
                + Sync,
        >,
    > {
        self.get_skinning_transforms().map(|x| {
            super::resolved_points_based_sources::SkinngScaleXformsDataSource::new(x)
                as Arc<
                    dyn usd_hd::data_source::HdTypedSampledDataSource<
                            Vec<usd_gf::matrix3::Matrix3f>,
                        > + Send
                        + Sync,
                >
        })
    }

    /// Get skinning dual quaternions (for dual quat skinning).
    pub fn get_skinning_dual_quats(
        &self,
    ) -> Option<Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<Vec<Vec4f>> + Send + Sync>>
    {
        self.get_skinning_transforms().map(|x| {
            super::resolved_points_based_sources::SkinngDualQuatsDataSource::new(x)
                as Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<Vec<Vec4f>> + Send + Sync>
        })
    }

    /// Get blend shape weights (computed from blend shape data).
    pub fn get_blend_shape_weights(
        &self,
    ) -> Option<Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<Vec<f32>> + Send + Sync>>
    {
        let blend_data = self.get_blend_shape_data();
        let blend_shapes = self.resolved_skeleton_schema.get_blend_shapes()?;
        let blend_weights = self.resolved_skeleton_schema.get_blend_shape_weights();
        let blend_ranges = self.resolved_skeleton_schema.get_blend_shape_ranges()?;
        Some(
            super::resolved_points_based_sources::BlendShapeWeightsDataSource::new(
                blend_data,
                blend_shapes,
                blend_weights,
                blend_ranges,
            ),
        )
    }

    /// Get common space to prim local (inverse of prim local to common).
    pub fn get_common_space_to_prim_local(
        &self,
    ) -> Option<
        Arc<
            dyn usd_hd::data_source::HdTypedSampledDataSource<usd_gf::matrix4::Matrix4d>
                + Send
                + Sync,
        >,
    > {
        let prim_local = self.xform_resolver.get_prim_local_to_common_space()?;
        Some(super::resolved_points_based_sources::MatrixInverseDataSource::new(prim_local))
    }

    /// Get points from primvars.
    pub fn get_points(&self) -> Option<HdDataSourceBaseHandle> {
        let points_token = Token::new("points");
        let pv = self.primvars.get_primvar(&points_token)?;
        pv.get(&*super::data_source_utils::PRIMVAR_VALUE)
    }

    /// Get normals from primvars.
    pub fn get_normals(&self) -> Option<HdDataSourceBaseHandle> {
        let normals_token = Token::new("normals");
        let pv = self.primvars.get_primvar(&normals_token)?;
        pv.get(&*super::data_source_utils::PRIMVAR_VALUE)
    }

    /// Get geom bind transform (Matrix4f for vertex shader).
    pub fn get_geom_bind_transform(&self) -> HdDataSourceBaseHandle {
        let token = BindingSchema::get_geom_bind_transform_primvar_token();
        let pv = self.primvars.get_primvar(&token);
        let value = pv.and_then(|c| {
            c.get(&*super::data_source_utils::PRIMVAR_VALUE)
                .and_then(|ds| ds.as_sampled().map(|sampled| sampled.get_value(0.0)))
                .and_then(|value| usd_gf::matrix4::Matrix4d::extract(&value))
        });
        let m = value
            .map(|m4d| {
                usd_gf::matrix4::Matrix4f::new(
                    m4d[0][0] as f32,
                    m4d[0][1] as f32,
                    m4d[0][2] as f32,
                    m4d[0][3] as f32,
                    m4d[1][0] as f32,
                    m4d[1][1] as f32,
                    m4d[1][2] as f32,
                    m4d[1][3] as f32,
                    m4d[2][0] as f32,
                    m4d[2][1] as f32,
                    m4d[2][2] as f32,
                    m4d[2][3] as f32,
                    m4d[3][0] as f32,
                    m4d[3][1] as f32,
                    m4d[3][2] as f32,
                    m4d[3][3] as f32,
                )
            })
            .unwrap_or_else(usd_gf::matrix4::Matrix4f::identity);
        Self::to_data_source(m)
    }

    /// Get has constant influences.
    pub fn get_has_constant_influences(&self) -> HdDataSourceBaseHandle {
        Self::to_data_source(self.get_joint_influences_data().has_constant_influences)
    }

    /// Get num influences per component.
    pub fn get_num_influences_per_component(&self) -> HdDataSourceBaseHandle {
        Self::to_data_source(
            self.get_joint_influences_data()
                .num_influences_per_component,
        )
    }

    /// Get influences (interleaved joint indices and weights).
    pub fn get_influences(&self) -> HdDataSourceBaseHandle {
        Self::to_data_source(self.get_joint_influences_data().influences.clone())
    }

    /// Get blend shape offsets.
    pub fn get_blend_shape_offsets(&self) -> HdDataSourceBaseHandle {
        Self::to_data_source(self.get_blend_shape_data().blend_shape_offsets.clone())
    }

    /// Get blend shape offset ranges.
    pub fn get_blend_shape_offset_ranges(&self) -> HdDataSourceBaseHandle {
        Self::to_data_source(
            self.get_blend_shape_data()
                .blend_shape_offset_ranges
                .clone(),
        )
    }

    /// Get num blend shape offset ranges.
    pub fn get_num_blend_shape_offset_ranges(&self) -> HdDataSourceBaseHandle {
        Self::to_data_source(self.get_blend_shape_data().blend_shape_offset_ranges.len() as i32)
    }

    /// Get prim source (for overlay container).
    pub fn get_prim_source(&self) -> &HdContainerDataSourceHandle {
        &self.prim_source
    }

    /// Get skeleton prim source (for skinning primvars).
    pub fn get_skeleton_prim_source(&self) -> Option<HdContainerDataSourceHandle> {
        Some(self.skeleton_prim_source.clone())
    }

    /// Get face vertex indices from mesh topology.
    ///
    /// Port of GetFaceVertexIndices().
    /// Used for normals computation when mesh has face-varying normals.
    pub fn get_face_vertex_indices(&self) -> Option<HdDataSourceBaseHandle> {
        let mesh_schema = HdMeshSchema::get_from_parent(&self.prim_source);
        let topo = mesh_schema.get_topology()?;
        if !topo.is_defined() {
            return None;
        }
        topo.get_face_vertex_indices()
            .map(|ds| ds as HdDataSourceBaseHandle)
    }

    /// Get has face varying normals.
    pub fn get_has_face_varying_normals(&self) -> Option<HdDataSourceBaseHandle> {
        use super::data_source_utils::INTERPOLATION;

        let normals_token = Token::new("normals");
        let pv = self.primvars.get_primvar(&normals_token)?;
        let interp_token = get_typed_value_from_container_token(&pv, &*INTERPOLATION)?;
        let has_face_varying = interp_token == "faceVarying";
        Some(Self::to_data_source(has_face_varying))
    }
}
