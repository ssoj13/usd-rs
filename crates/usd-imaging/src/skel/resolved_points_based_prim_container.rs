//! ResolvedPointsBasedPrimContainer - HdContainerDataSource overlay for resolved points-based prim.
//!
//! Port of UsdSkelImagingDataSourceResolvedPointsBasedPrim::Get/GetNames.
//!
//! Wraps DataSourceResolvedPointsBasedPrim and provides overlay logic for primvars,
//! extComputationPrimvars when skinning is not deferred.

use super::data_source_primvar::{CONSTANT, DataSourcePrimvar, NORMAL, POINT, VERTEX};
use super::data_source_resolved_points_based_prim::DataSourceResolvedPointsBasedPrim;
use super::data_source_utils::get_typed_value_from_container_token;
use super::skeleton_schema::SkeletonSchema;
use std::sync::Arc;
use usd_hd::data_source::{
    HdBlockDataSource, HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase,
    HdDataSourceBaseHandle, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, HdSampledDataSource, cast_to_container,
};
use usd_hd::schema::HdPrimvarsSchema;
use usd_hd::skinning_settings::{
    self, blend_shape_offset_ranges_token, blend_shape_offsets_token, blend_shape_weights_token,
    common_space_to_prim_local_token, geom_bind_transform_token, has_constant_influences_token,
    influences_token, num_blend_shape_offset_ranges_token, num_blend_shape_weights_token,
    num_influences_per_component_token, num_joints_token, num_skinning_method_token,
    skel_local_to_common_space_token,
};
use usd_hd::types::HdTupleType;
use usd_sdf::Path;
use usd_tf::Token;

fn primvars_token() -> Token {
    Token::new("primvars")
}
fn ext_computation_primvars_token() -> Token {
    Token::new("extComputationPrimvars")
}
#[allow(dead_code)]
fn mesh_token() -> Token {
    Token::new("mesh")
}
fn points_token() -> Token {
    Token::new("points")
}
fn normals_token() -> Token {
    Token::new("normals")
}

/// Block points and normals primvars (replace with HdBlockDataSource).
///
/// Port of _BlockPointsAndNormalsPrimvars().
fn block_points_and_normals_primvars() -> HdContainerDataSourceHandle {
    let block = HdBlockDataSource::new() as HdDataSourceBaseHandle;
    HdRetainedContainerDataSource::from_entries(&[
        (points_token(), block.clone()),
        (normals_token(), block),
    ])
}

/// Build ext computation primvars for points and normals.
///
/// Port of _ExtComputationPrimvars().
fn ext_computation_primvars(
    prim_path: &Path,
    primvars: &HdPrimvarsSchema,
) -> HdContainerDataSourceHandle {
    use super::tokens::{EXT_COMPUTATION_NAME_TOKENS, EXT_COMPUTATION_OUTPUT_TOKENS};

    let normals_interpolation = primvars
        .get_container()
        .and_then(|c| c.get(&normals_token()))
        .and_then(|ds| cast_to_container(&ds))
        .and_then(|normals_cont| {
            get_typed_value_from_container_token(&normals_cont, &Token::new("interpolation"))
        })
        .unwrap_or_else(|| Token::new("vertex"));

    let points_comp_path = prim_path
        .append_child(EXT_COMPUTATION_NAME_TOKENS.points_computation.as_str())
        .unwrap_or_else(|| prim_path.clone());
    let normals_comp_path = prim_path
        .append_child(EXT_COMPUTATION_NAME_TOKENS.normals_computation.as_str())
        .unwrap_or_else(|| prim_path.clone());

    let points_ec = build_ext_computation_primvar(
        points_comp_path,
        EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points.clone(),
        VERTEX.clone(),
        POINT.clone(),
    );
    let normals_ec = build_ext_computation_primvar(
        normals_comp_path,
        EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals.clone(),
        normals_interpolation,
        NORMAL.clone(),
    );

    HdRetainedContainerDataSource::from_entries(&[
        (points_token(), points_ec as HdDataSourceBaseHandle),
        (normals_token(), normals_ec as HdDataSourceBaseHandle),
    ])
}

fn build_ext_computation_primvar(
    source_computation: Path,
    source_output_name: Token,
    interpolation: Token,
    role: Token,
) -> HdContainerDataSourceHandle {
    use usd_hd::types::HdType;

    let interp_ds = HdRetainedTypedSampledDataSource::new(interpolation) as HdDataSourceBaseHandle;
    let role_ds = HdRetainedTypedSampledDataSource::new(role) as HdDataSourceBaseHandle;
    let path_ds =
        HdRetainedTypedSampledDataSource::new(source_computation) as HdDataSourceBaseHandle;
    let output_name_ds =
        HdRetainedTypedSampledDataSource::new(source_output_name) as HdDataSourceBaseHandle;
    let value_type_ds =
        HdRetainedTypedSampledDataSource::new(HdTupleType::new(HdType::FloatVec3, 1))
            as HdDataSourceBaseHandle;

    HdRetainedContainerDataSource::from_entries(&[
        (Token::new("interpolation"), interp_ds),
        (Token::new("role"), role_ds),
        (Token::new("sourceComputation"), path_ds),
        (Token::new("sourceComputationOutputName"), output_name_ds),
        (Token::new("valueType"), value_type_ds),
    ])
}

/// Skinning primvars data source for deferred skinning.
///
/// Port of _SkinningPrimvarsDataSource.
#[derive(Debug)]
struct SkinningPrimvarsDataSource {
    resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
    skeleton_prim_source: HdContainerDataSourceHandle,
}

impl SkinningPrimvarsDataSource {
    fn new(
        resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
        skeleton_prim_source: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            resolved_prim,
            skeleton_prim_source,
        })
    }

    fn get_num_joints(&self) -> i32 {
        match SkeletonSchema::get_from_parent(&self.skeleton_prim_source) {
            Some(schema) => schema.get_joints().len() as i32,
            None => 0,
        }
    }

    fn is_dual_quat_skinning(&self) -> bool {
        self.resolved_prim.get_skinning_method() == "dualQuaternion"
    }
}

impl HdDataSourceBase for SkinningPrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            resolved_prim: self.resolved_prim.clone(),
            skeleton_prim_source: self.skeleton_prim_source.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            resolved_prim: self.resolved_prim.clone(),
            skeleton_prim_source: self.skeleton_prim_source.clone(),
        }))
    }
}

impl HdContainerDataSource for SkinningPrimvarsDataSource {
    fn get_names(&self) -> Vec<Token> {
        skinning_settings::get_skinning_input_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        fn to_ds<T>(v: T) -> HdDataSourceBaseHandle
        where
            T: Clone + Send + Sync + std::fmt::Debug + 'static,
            HdRetainedTypedSampledDataSource<T>: HdSampledDataSource,
        {
            HdRetainedTypedSampledDataSource::new(v) as HdDataSourceBaseHandle
        }

        // ExtComputationInputValues
        let empty_role = Token::new("");
        if *name == skinning_settings::skinning_xforms_token() {
            return self
                .resolved_prim
                .get_skinning_transforms()
                .map(|x| {
                    DataSourcePrimvar::new(
                        x as HdDataSourceBaseHandle,
                        CONSTANT.clone(),
                        empty_role.clone(),
                    )
                })
                .map(|c| c as HdDataSourceBaseHandle);
        }
        if *name == skinning_settings::skinning_scale_xforms_token() {
            return self
                .resolved_prim
                .get_skinning_scale_transforms()
                .map(|x| {
                    DataSourcePrimvar::new(
                        x as HdDataSourceBaseHandle,
                        CONSTANT.clone(),
                        empty_role.clone(),
                    )
                })
                .map(|c| c as HdDataSourceBaseHandle);
        }
        if *name == skinning_settings::skinning_dual_quats_token() {
            return self
                .resolved_prim
                .get_skinning_dual_quats()
                .map(|x| {
                    DataSourcePrimvar::new(
                        x as HdDataSourceBaseHandle,
                        CONSTANT.clone(),
                        empty_role.clone(),
                    )
                })
                .map(|c| c as HdDataSourceBaseHandle);
        }
        if *name == blend_shape_weights_token() {
            return self.resolved_prim.get_blend_shape_weights().map(|x| {
                DataSourcePrimvar::new(
                    x as HdDataSourceBaseHandle,
                    CONSTANT.clone(),
                    empty_role.clone(),
                ) as HdDataSourceBaseHandle
            });
        }
        if *name == skel_local_to_common_space_token() {
            return self
                .resolved_prim
                .get_resolved_skeleton_schema()
                .get_skel_local_to_common_space()
                .map(|x| {
                    DataSourcePrimvar::new(
                        x as HdDataSourceBaseHandle,
                        CONSTANT.clone(),
                        empty_role.clone(),
                    ) as HdDataSourceBaseHandle
                });
        }
        if *name == common_space_to_prim_local_token() {
            return self
                .resolved_prim
                .get_common_space_to_prim_local()
                .map(|x| {
                    DataSourcePrimvar::new(
                        x as HdDataSourceBaseHandle,
                        CONSTANT.clone(),
                        empty_role.clone(),
                    ) as HdDataSourceBaseHandle
                });
        }

        // ExtAggregatorComputationInputValues
        if *name == geom_bind_transform_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_geom_bind_transform(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == has_constant_influences_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_has_constant_influences(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == num_influences_per_component_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_num_influences_per_component(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == influences_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_influences(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == blend_shape_offsets_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_blend_shape_offsets(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == blend_shape_offset_ranges_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_blend_shape_offset_ranges(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == num_blend_shape_offset_ranges_token() {
            return Some(DataSourcePrimvar::new(
                self.resolved_prim.get_num_blend_shape_offset_ranges(),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == num_skinning_method_token() {
            let v = if self.is_dual_quat_skinning() {
                1i32
            } else {
                0i32
            };
            return Some(
                DataSourcePrimvar::new(to_ds(v), CONSTANT.clone(), empty_role.clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == num_joints_token() {
            return Some(DataSourcePrimvar::new(
                to_ds(self.get_num_joints()),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == num_blend_shape_weights_token() {
            let n = self.resolved_prim.get_blend_shape_data().num_sub_shapes;
            return Some(DataSourcePrimvar::new(
                to_ds(n as i32),
                CONSTANT.clone(),
                empty_role.clone(),
            ) as HdDataSourceBaseHandle);
        }

        // Fallback: resolved_prim.Get(name) - for any other names from GetSkinningInputNames
        self.resolved_prim.get_prim_source().get(name)
    }
}

/// Container overlay for resolved points-based prim.
///
/// Implements Get/GetNames per C++ DataSourceResolvedPointsBasedPrim.
#[derive(Debug)]
pub struct ResolvedPointsBasedPrimContainer {
    resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
}

impl ResolvedPointsBasedPrimContainer {
    /// Creates a new container overlay for the resolved points-based prim.
    pub fn new(resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>) -> Arc<Self> {
        Arc::new(Self { resolved_prim })
    }

    /// Build overlay container for prim data source.
    ///
    /// Returns container that overlays resolved data on prim_source.
    /// Used as overlay in HdOverlayContainerDataSource.
    pub fn build_overlay(
        resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
    ) -> HdContainerDataSourceHandle {
        let container = Self::new(resolved_prim);
        container as HdContainerDataSourceHandle
    }
}

impl HdDataSourceBase for ResolvedPointsBasedPrimContainer {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            resolved_prim: self.resolved_prim.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            resolved_prim: self.resolved_prim.clone(),
        }))
    }
}

impl HdContainerDataSource for ResolvedPointsBasedPrimContainer {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.resolved_prim.get_prim_source().get_names();
        if !self
            .resolved_prim
            .get_resolved_skeleton_schema()
            .is_defined()
        {
            return names;
        }
        if !skinning_settings::is_skinning_deferred() {
            add_if_necessary(&ext_computation_primvars_token(), &mut names);
        }
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let prim_source = self.resolved_prim.get_prim_source();

        // Deferred skinning: replace primvars with skinning primvars (except for skeleton)
        if skinning_settings::is_skinning_deferred()
            && *name == primvars_token()
            && self.resolved_prim.get_prim_path() != self.resolved_prim.get_skeleton_path()
        {
            if let Some(skel_src) = self.resolved_prim.get_skeleton_prim_source() {
                let skinning_ds =
                    SkinningPrimvarsDataSource::new(self.resolved_prim.clone(), skel_src);
                return Some(skinning_ds as HdDataSourceBaseHandle);
            }
        }

        let input_src = prim_source.get(name);

        if !self.resolved_prim.has_ext_computations() {
            return input_src;
        }

        if *name == ext_computation_primvars_token() {
            let ext_comp = ext_computation_primvars(
                self.resolved_prim.get_prim_path(),
                self.resolved_prim.get_primvars(),
            );
            let overlay = input_src
                .and_then(|input| cast_to_container(&input))
                .map(|input_cont| {
                    HdOverlayContainerDataSource::new_2(ext_comp.clone(), input_cont)
                        as HdDataSourceBaseHandle
                });
            return overlay.or_else(|| Some(ext_comp as HdDataSourceBaseHandle));
        }

        if *name == primvars_token() {
            return Some(block_points_and_normals_primvars() as HdDataSourceBaseHandle);
        }

        input_src
    }
}

fn add_if_necessary(name: &Token, names: &mut Vec<Token>) {
    if !names.iter().any(|n| n == name) {
        names.push(name.clone());
    }
}
