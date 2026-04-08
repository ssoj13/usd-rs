//! DataSourceResolvedSkeletonPrim - Resolved skeleton prim data source.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceResolvedSkeletonPrim.h/cpp

use super::animation_schema::AnimationSchema;
use super::binding_schema::BindingSchema;
use super::data_source_primvar::{CONSTANT, DataSourcePrimvar, POINT, VERTEX};
use super::resolved_skeleton_schema::{
    HdFloatArrayDataSourceHandle, HdMatrix4fArrayDataSourceHandle, HdTokenArrayDataSourceHandle,
    HdVec2iArrayDataSourceHandle, ResolvedSkeletonSchema,
};
use super::skel_data::SkelData;
use super::skel_guide_data::SkelGuideData;
use super::skeleton_schema::SkeletonSchema;
use super::xform_resolver::DataSourceXformResolver;
use std::sync::Arc;
use usd_gf::matrix4::{Matrix4d, Matrix4f};
use usd_gf::{Quatf, Vec2i, Vec3f, Vec3h};
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, HdSampledDataSource, HdSampledDataSourceTime,
    HdTypedSampledDataSource,
};
use usd_hd::scene_index::HdSceneIndexHandle;
use usd_hd::scene_index::observer::DirtiedPrimEntry;
use usd_hd::schema::{HdMeshSchema, HdMeshTopologySchema, HdPrimvarsSchema};
use usd_sdf::Path;
use usd_skel::{AnimMapper, utils::concat_joint_transforms_f};
use usd_tf::Token;
use usd_vt::{Array, Value};

fn mesh_token() -> Token {
    Token::new("mesh")
}

fn primvars_token() -> Token {
    Token::new("primvars")
}

fn points_token() -> Token {
    Token::new("points")
}

fn topology_locator() -> HdDataSourceLocator {
    HdMeshSchema::get_topology_locator()
}

fn points_primvar_value_locator() -> HdDataSourceLocator {
    HdPrimvarsSchema::get_default_locator()
        .append(&points_token())
        .append(&Token::new("primvarValue"))
}

fn to_ds<T>(value: T) -> HdDataSourceBaseHandle
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    HdRetainedTypedSampledDataSource<T>: HdSampledDataSource,
{
    HdRetainedTypedSampledDataSource::new(value) as HdDataSourceBaseHandle
}

fn quat_to_matrix4f(rotate: &Quatf, translate: &Vec3f, scale: &Vec3h) -> Matrix4f {
    let sx = f32::from(scale.x);
    let sy = f32::from(scale.y);
    let sz = f32::from(scale.z);

    let w = rotate.real();
    let x = rotate.imaginary().x;
    let y = rotate.imaginary().y;
    let z = rotate.imaginary().z;

    let r00 = 1.0 - 2.0 * (y * y + z * z);
    let r01 = 2.0 * (x * y - z * w);
    let r02 = 2.0 * (x * z + y * w);
    let r10 = 2.0 * (x * y + z * w);
    let r11 = 1.0 - 2.0 * (x * x + z * z);
    let r12 = 2.0 * (y * z - x * w);
    let r20 = 2.0 * (x * z - y * w);
    let r21 = 2.0 * (y * z + x * w);
    let r22 = 1.0 - 2.0 * (x * x + y * y);

    Matrix4f::new(
        r00 * sx,
        r01 * sx,
        r02 * sx,
        0.0,
        r10 * sy,
        r11 * sy,
        r12 * sy,
        0.0,
        r20 * sz,
        r21 * sz,
        r22 * sz,
        0.0,
        translate.x,
        translate.y,
        translate.z,
        1.0,
    )
}

fn compute_blend_shape_ranges(
    schemas: &[AnimationSchema],
    shutter_offset: HdSampledDataSourceTime,
) -> Vec<Vec2i> {
    let mut offset = 0i32;
    let mut result = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let count = schema
            .get_blend_shape_weights_data_source()
            .map(|ds| ds.get_typed_value(shutter_offset).len() as i32)
            .unwrap_or(0);
        result.push(Vec2i::new(offset, count));
        offset += count;
    }
    result
}

#[derive(Debug, Clone)]
struct SkinningTransformsDataSource {
    skel_data: Arc<SkelData>,
    rest_transforms: Option<Arc<dyn HdTypedSampledDataSource<Vec<Matrix4d>> + Send + Sync>>,
    animation_schemas: Vec<AnimationSchema>,
}

impl SkinningTransformsDataSource {
    fn new(
        skel_data: Arc<SkelData>,
        rest_transforms: Option<Arc<dyn HdTypedSampledDataSource<Vec<Matrix4d>> + Send + Sync>>,
        animation_schemas: Vec<AnimationSchema>,
    ) -> Arc<Self> {
        Arc::new(Self {
            skel_data,
            rest_transforms,
            animation_schemas,
        })
    }

    fn compute_local_rest(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Matrix4f> {
        if let Some(rest_ds) = &self.rest_transforms {
            return rest_ds
                .get_typed_value(shutter_offset)
                .into_iter()
                .map(|m| {
                    Matrix4f::new(
                        m[0][0] as f32,
                        m[0][1] as f32,
                        m[0][2] as f32,
                        m[0][3] as f32,
                        m[1][0] as f32,
                        m[1][1] as f32,
                        m[1][2] as f32,
                        m[1][3] as f32,
                        m[2][0] as f32,
                        m[2][1] as f32,
                        m[2][2] as f32,
                        m[2][3] as f32,
                        m[3][0] as f32,
                        m[3][1] as f32,
                        m[3][2] as f32,
                        m[3][3] as f32,
                    )
                })
                .collect();
        }
        self.skel_data.bind_transforms.clone()
    }

    fn overlay_animation(
        &self,
        local_xforms: &mut Vec<Matrix4f>,
        schema: &AnimationSchema,
        shutter_offset: HdSampledDataSourceTime,
    ) {
        let skeleton_joints = self.skel_data.skeleton_schema.get_joints();
        let animation_joints = schema
            .get_joints_data_source()
            .map(|ds| ds.get_typed_value(shutter_offset))
            .unwrap_or_default();

        let translations = match schema.get_translations_data_source() {
            Some(ds) => ds.get_typed_value(shutter_offset),
            None => return,
        };
        let rotations = match schema.get_rotations_data_source() {
            Some(ds) => ds.get_typed_value(shutter_offset),
            None => return,
        };
        let scales = match schema.get_scales_data_source() {
            Some(ds) => ds.get_typed_value(shutter_offset),
            None => return,
        };

        if translations.len() != rotations.len() || rotations.len() != scales.len() {
            return;
        }

        let anim_local: Vec<Matrix4f> = translations
            .iter()
            .zip(rotations.iter())
            .zip(scales.iter())
            .map(|((t, r), s)| quat_to_matrix4f(r, t, s))
            .collect();

        if anim_local.is_empty() {
            return;
        }

        let mapper = if animation_joints.is_empty() || skeleton_joints.is_empty() {
            AnimMapper::new()
        } else {
            AnimMapper::from_orders(&animation_joints, &skeleton_joints)
        };

        if mapper.is_null() || mapper.is_identity() {
            let n = local_xforms.len().min(anim_local.len());
            local_xforms[..n].clone_from_slice(&anim_local[..n]);
            return;
        }

        let _ = mapper.remap_transforms_4f(&anim_local, local_xforms, 1);
    }

    fn compute(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Matrix4f> {
        let num_joints = self.skel_data.topology.num_joints();
        if num_joints == 0 {
            return Vec::new();
        }

        let mut local_xforms = self.compute_local_rest(shutter_offset);
        if local_xforms.len() != num_joints {
            local_xforms.resize(num_joints, Matrix4f::identity());
        }

        if let Some(schema) = self.animation_schemas.first() {
            self.overlay_animation(&mut local_xforms, schema, shutter_offset);
        }

        let mut skel_xforms = vec![Matrix4f::identity(); num_joints];
        if !concat_joint_transforms_f(
            &self.skel_data.topology,
            &local_xforms,
            &mut skel_xforms,
            None,
        ) {
            return Vec::new();
        }

        skel_xforms
            .iter()
            .zip(self.skel_data.inverse_bind_transforms.iter())
            .map(|(skel, inv_bind)| *inv_bind * *skel)
            .collect()
    }
}

impl HdDataSourceBase for SkinningTransformsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from_no_hash(self.compute(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for SkinningTransformsDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from_no_hash(self.compute(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        out_sample_times.clear();
        let mut varying = false;

        if let Some(rest_ds) = &self.rest_transforms {
            varying |=
                rest_ds.get_contributing_sample_times(start_time, end_time, out_sample_times);
        }

        for schema in &self.animation_schemas {
            if let Some(ds) = schema.get_translations_data_source() {
                let mut tmp = Vec::new();
                if ds.get_contributing_sample_times(start_time, end_time, &mut tmp) {
                    varying = true;
                    out_sample_times.extend(tmp);
                }
            }
            if let Some(ds) = schema.get_rotations_data_source() {
                let mut tmp = Vec::new();
                if ds.get_contributing_sample_times(start_time, end_time, &mut tmp) {
                    varying = true;
                    out_sample_times.extend(tmp);
                }
            }
            if let Some(ds) = schema.get_scales_data_source() {
                let mut tmp = Vec::new();
                if ds.get_contributing_sample_times(start_time, end_time, &mut tmp) {
                    varying = true;
                    out_sample_times.extend(tmp);
                }
            }
        }

        if !varying {
            return false;
        }

        out_sample_times.push(start_time);
        out_sample_times.push(0.0);
        out_sample_times.push(end_time);
        out_sample_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        out_sample_times.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
        true
    }
}

impl HdTypedSampledDataSource<Vec<Matrix4f>> for SkinningTransformsDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Matrix4f> {
        self.compute(shutter_offset)
    }
}

#[derive(Debug, Clone)]
struct BlendShapeRangesDataSource {
    animation_schemas: Vec<AnimationSchema>,
}

impl BlendShapeRangesDataSource {
    fn new(animation_schemas: Vec<AnimationSchema>) -> Arc<Self> {
        Arc::new(Self { animation_schemas })
    }

    fn compute(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Vec2i> {
        compute_blend_shape_ranges(&self.animation_schemas, shutter_offset)
    }
}

impl HdDataSourceBase for BlendShapeRangesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from_no_hash(self.compute(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for BlendShapeRangesDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from_no_hash(self.compute(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        out_sample_times.clear();
        let mut varying = false;
        for schema in &self.animation_schemas {
            if let Some(ds) = schema.get_blend_shape_weights_data_source() {
                let mut tmp = Vec::new();
                if ds.get_contributing_sample_times(start_time, end_time, &mut tmp) {
                    varying = true;
                    out_sample_times.extend(tmp);
                }
            }
        }
        if !varying {
            return false;
        }
        out_sample_times.push(start_time);
        out_sample_times.push(0.0);
        out_sample_times.push(end_time);
        out_sample_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        out_sample_times.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
        true
    }
}

impl HdTypedSampledDataSource<Vec<Vec2i>> for BlendShapeRangesDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Vec2i> {
        self.compute(shutter_offset)
    }
}

#[derive(Debug, Clone)]
struct BlendShapesDataSource {
    animation_schemas: Vec<AnimationSchema>,
    ranges: Arc<BlendShapeRangesDataSource>,
}

impl BlendShapesDataSource {
    fn new(
        animation_schemas: Vec<AnimationSchema>,
        ranges: Arc<BlendShapeRangesDataSource>,
    ) -> Arc<Self> {
        Arc::new(Self {
            animation_schemas,
            ranges,
        })
    }

    fn compute(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Token> {
        let ranges = self.ranges.compute(shutter_offset);
        let total = ranges
            .last()
            .map(|r| (r.x + r.y).max(0) as usize)
            .unwrap_or(0);
        let mut result = vec![Token::empty(); total];

        for (i, schema) in self.animation_schemas.iter().enumerate() {
            let Some(range) = ranges.get(i) else {
                continue;
            };
            let Some(ds) = schema.get_blend_shapes_data_source() else {
                continue;
            };
            let values = ds.get_typed_value(shutter_offset);
            for j in 0..(range.y.max(0) as usize).min(values.len()) {
                result[range.x as usize + j] = values[j].clone();
            }
        }

        result
    }
}

impl HdDataSourceBase for BlendShapesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from_no_hash(self.compute(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for BlendShapesDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from_no_hash(self.compute(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.ranges
            .get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdTypedSampledDataSource<Vec<Token>> for BlendShapesDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Token> {
        self.compute(shutter_offset)
    }
}

#[derive(Debug, Clone)]
struct BlendShapeWeightsDataSource {
    animation_schemas: Vec<AnimationSchema>,
    ranges: Arc<BlendShapeRangesDataSource>,
}

impl BlendShapeWeightsDataSource {
    fn new(
        animation_schemas: Vec<AnimationSchema>,
        ranges: Arc<BlendShapeRangesDataSource>,
    ) -> Arc<Self> {
        Arc::new(Self {
            animation_schemas,
            ranges,
        })
    }

    fn compute(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<f32> {
        let ranges = self.ranges.compute(shutter_offset);
        let total = ranges
            .last()
            .map(|r| (r.x + r.y).max(0) as usize)
            .unwrap_or(0);
        let mut result = vec![0.0f32; total];

        for (i, schema) in self.animation_schemas.iter().enumerate() {
            let Some(range) = ranges.get(i) else {
                continue;
            };
            let Some(ds) = schema.get_blend_shape_weights_data_source() else {
                continue;
            };
            let values = ds.get_typed_value(shutter_offset);
            for j in 0..(range.y.max(0) as usize).min(values.len()) {
                result[range.x as usize + j] = values[j];
            }
        }

        result
    }
}

impl HdDataSourceBase for BlendShapeWeightsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from_no_hash(self.compute(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for BlendShapeWeightsDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from_no_hash(self.compute(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.ranges
            .get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdTypedSampledDataSource<Vec<f32>> for BlendShapeWeightsDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<f32> {
        self.compute(shutter_offset)
    }
}

#[derive(Debug, Clone)]
struct PointsPrimvarValueDataSource {
    guide_data: Arc<SkelGuideData>,
    skinning_transforms: HdMatrix4fArrayDataSourceHandle,
}

impl PointsPrimvarValueDataSource {
    fn new(
        guide_data: Arc<SkelGuideData>,
        skinning_transforms: HdMatrix4fArrayDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            guide_data,
            skinning_transforms,
        })
    }

    fn compute(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let xforms = self.skinning_transforms.get_typed_value(shutter_offset);
        if xforms.is_empty() || self.guide_data.bone_mesh_points.is_empty() {
            return self.guide_data.bone_mesh_points.clone();
        }

        self.guide_data
            .bone_mesh_points
            .iter()
            .zip(self.guide_data.bone_joint_indices.iter())
            .map(|(point, joint_idx)| {
                let joint = (*joint_idx).max(0) as usize;
                xforms
                    .get(joint)
                    .map(|m| m.transform_point(point))
                    .unwrap_or(*point)
            })
            .collect()
    }
}

impl HdDataSourceBase for PointsPrimvarValueDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from_no_hash(self.compute(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for PointsPrimvarValueDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from_no_hash(self.compute(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.skinning_transforms.get_contributing_sample_times(
            start_time,
            end_time,
            out_sample_times,
        )
    }
}

impl HdTypedSampledDataSource<Vec<Vec3f>> for PointsPrimvarValueDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Vec3f> {
        self.compute(shutter_offset)
    }
}

#[derive(Debug, Clone)]
struct SkelGuideSkinningPrimvarsDataSource {
    guide_data: Arc<SkelGuideData>,
    skinning_transforms: HdMatrix4fArrayDataSourceHandle,
}

impl SkelGuideSkinningPrimvarsDataSource {
    fn new(
        guide_data: Arc<SkelGuideData>,
        skinning_transforms: HdMatrix4fArrayDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            guide_data,
            skinning_transforms,
        })
    }
}

impl HdDataSourceBase for SkelGuideSkinningPrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for SkelGuideSkinningPrimvarsDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            points_token(),
            usd_hd::skinning_settings::skinning_xforms_token(),
            usd_hd::skinning_settings::skel_local_to_common_space_token(),
            usd_hd::skinning_settings::common_space_to_prim_local_token(),
            usd_hd::skinning_settings::geom_bind_transform_token(),
            usd_hd::skinning_settings::has_constant_influences_token(),
            usd_hd::skinning_settings::num_influences_per_component_token(),
            usd_hd::skinning_settings::influences_token(),
            usd_hd::skinning_settings::num_skinning_method_token(),
            usd_hd::skinning_settings::num_joints_token(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == points_token() {
            return Some(DataSourcePrimvar::new(
                to_ds(self.guide_data.bone_mesh_points.clone()),
                VERTEX.clone(),
                POINT.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == usd_hd::skinning_settings::skinning_xforms_token() {
            return Some(DataSourcePrimvar::new(
                self.skinning_transforms.clone() as HdDataSourceBaseHandle,
                CONSTANT.clone(),
                Token::empty(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == usd_hd::skinning_settings::skel_local_to_common_space_token()
            || *name == usd_hd::skinning_settings::common_space_to_prim_local_token()
            || *name == usd_hd::skinning_settings::geom_bind_transform_token()
        {
            return Some(DataSourcePrimvar::new_default(to_ds(Matrix4f::identity()))
                as HdDataSourceBaseHandle);
        }
        if *name == usd_hd::skinning_settings::has_constant_influences_token() {
            return Some(DataSourcePrimvar::new_default(to_ds(false)) as HdDataSourceBaseHandle);
        }
        if *name == usd_hd::skinning_settings::num_influences_per_component_token() {
            return Some(DataSourcePrimvar::new_default(to_ds(1i32)) as HdDataSourceBaseHandle);
        }
        if *name == usd_hd::skinning_settings::influences_token() {
            let influences: Vec<usd_gf::Vec2f> = self
                .guide_data
                .bone_joint_indices
                .iter()
                .map(|joint| usd_gf::Vec2f::new(*joint as f32, 1.0))
                .collect();
            return Some(
                DataSourcePrimvar::new_default(to_ds(influences)) as HdDataSourceBaseHandle
            );
        }
        if *name == usd_hd::skinning_settings::num_skinning_method_token() {
            return Some(DataSourcePrimvar::new_default(to_ds(0i32)) as HdDataSourceBaseHandle);
        }
        if *name == usd_hd::skinning_settings::num_joints_token() {
            return Some(
                DataSourcePrimvar::new_default(to_ds(self.guide_data.num_joints as i32))
                    as HdDataSourceBaseHandle,
            );
        }
        None
    }
}

#[derive(Debug, Clone)]
struct ResolvedSkeletonSchemaDataSource {
    source: Arc<DataSourceResolvedSkeletonPrim>,
}

impl ResolvedSkeletonSchemaDataSource {
    fn new(source: Arc<DataSourceResolvedSkeletonPrim>) -> Arc<Self> {
        Arc::new(Self { source })
    }
}

impl HdDataSourceBase for ResolvedSkeletonSchemaDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ResolvedSkeletonSchemaDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new("skelLocalToCommonSpace"),
            Token::new("skinningTransforms"),
            Token::new("blendShapes"),
            Token::new("blendShapeWeights"),
            Token::new("blendShapeRanges"),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == Token::new("skelLocalToCommonSpace") {
            return self
                .source
                .get_skel_local_to_common_space()
                .map(|ds| ds as HdDataSourceBaseHandle);
        }
        if *name == Token::new("skinningTransforms") {
            return Some(self.source.get_skinning_transforms() as HdDataSourceBaseHandle);
        }
        if *name == Token::new("blendShapes") {
            return Some(self.source.get_blend_shapes() as HdDataSourceBaseHandle);
        }
        if *name == Token::new("blendShapeWeights") {
            return Some(self.source.get_blend_shape_weights() as HdDataSourceBaseHandle);
        }
        if *name == Token::new("blendShapeRanges") {
            return Some(self.source.get_blend_shape_ranges() as HdDataSourceBaseHandle);
        }
        None
    }
}

pub struct DataSourceResolvedSkeletonPrim {
    prim_path: Path,
    animation_source: Path,
    prim_source: HdContainerDataSourceHandle,
    scene_index: HdSceneIndexHandle,
    instancer_paths: Vec<Path>,
    animation_schema: Option<AnimationSchema>,
    instance_animation_sources: Vec<Path>,
    instance_animation_schemas: Vec<AnimationSchema>,
    skel_data_cache: super::data_source_utils::SharedPtrThunk<SkelData>,
    skel_guide_data_cache: super::data_source_utils::SharedPtrThunk<SkelGuideData>,
}

impl std::fmt::Debug for DataSourceResolvedSkeletonPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceResolvedSkeletonPrim")
            .field("prim_path", &self.prim_path)
            .field("animation_source", &self.animation_source)
            .finish()
    }
}

impl DataSourceResolvedSkeletonPrim {
    fn clone_handle(&self) -> Arc<Self> {
        Arc::new(Self {
            prim_path: self.prim_path.clone(),
            animation_source: self.animation_source.clone(),
            prim_source: self.prim_source.clone(),
            scene_index: self.scene_index.clone(),
            instancer_paths: self.instancer_paths.clone(),
            animation_schema: self.animation_schema.clone(),
            instance_animation_sources: self.instance_animation_sources.clone(),
            instance_animation_schemas: self.instance_animation_schemas.clone(),
            skel_data_cache: Default::default(),
            skel_guide_data_cache: Default::default(),
        })
    }

    pub fn new(
        scene_index: HdSceneIndexHandle,
        prim_path: Path,
        prim_source: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        let binding = BindingSchema::get_from_parent(&prim_source);
        let animation_source = binding.get_animation_source().unwrap_or_default();
        let animation_schema = if animation_source.is_empty() {
            None
        } else {
            scene_index
                .read()
                .get_prim(&animation_source)
                .data_source
                .and_then(|ds| AnimationSchema::get_from_parent(&ds))
                .filter(|schema| schema.is_defined())
        };

        let xform_resolver = DataSourceXformResolver::new(scene_index.clone(), prim_source.clone());
        let instancer_paths = xform_resolver.instancer_paths().to_vec();
        let instance_animation_sources = xform_resolver.get_instance_animation_source();
        let instance_animation_schemas = instance_animation_sources
            .iter()
            .filter_map(|path| scene_index.read().get_prim(path).data_source)
            .filter_map(|ds| AnimationSchema::get_from_parent(&ds))
            .filter(|schema| schema.is_defined())
            .collect();

        Arc::new(Self {
            prim_path,
            animation_source,
            prim_source,
            scene_index,
            instancer_paths,
            animation_schema,
            instance_animation_sources,
            instance_animation_schemas,
            skel_data_cache: Default::default(),
            skel_guide_data_cache: Default::default(),
        })
    }

    pub fn get_animation_source(&self) -> &Path {
        &self.animation_source
    }

    pub fn get_resolved_animation_sources(&self) -> Vec<Path> {
        if self.should_resolve_instance_animation() {
            return self.instance_animation_sources.clone();
        }
        if self.animation_source.is_empty() {
            return Vec::new();
        }
        vec![self.animation_source.clone()]
    }

    pub fn get_instancer_paths(&self) -> &[Path] {
        &self.instancer_paths
    }

    pub fn get_skel_data(&self) -> Arc<SkelData> {
        if std::env::var_os("USD_PROFILE_SKEL_DS").is_some() {
            eprintln!(
                "[ResolvedSkeletonPrim] path={} get_skel_data:start",
                self.prim_path
            );
        }
        let result = self.skel_data_cache.get(|| {
            Arc::new(super::skel_data::compute_skel_data_from_source(
                self.prim_path.clone(),
                &self.prim_source,
            ))
        });
        if std::env::var_os("USD_PROFILE_SKEL_DS").is_some() {
            eprintln!(
                "[ResolvedSkeletonPrim] path={} get_skel_data:done",
                self.prim_path
            );
        }
        result
    }

    pub fn get_skel_guide_data(&self) -> Arc<SkelGuideData> {
        let skel_data = self.get_skel_data();
        self.skel_guide_data_cache
            .get(|| Arc::new(super::skel_guide_data::compute_skel_guide_data(&skel_data)))
    }

    fn should_resolve_instance_animation(&self) -> bool {
        usd_hd::skinning_settings::is_skinning_deferred() && self.animation_schema.is_none()
    }

    fn resolved_animation_schemas(&self) -> Vec<AnimationSchema> {
        if self.should_resolve_instance_animation() {
            return self.instance_animation_schemas.clone();
        }
        self.animation_schema.clone().into_iter().collect()
    }

    pub fn get_skel_local_to_common_space(
        &self,
    ) -> Option<usd_hd::schema::HdMatrixDataSourceHandle> {
        let resolver =
            DataSourceXformResolver::new(self.scene_index.clone(), self.prim_source.clone());
        resolver.get_prim_local_to_common_space()
    }

    pub fn get_skinning_transforms(&self) -> HdMatrix4fArrayDataSourceHandle {
        if std::env::var_os("USD_PROFILE_SKEL_DS").is_some() {
            eprintln!(
                "[ResolvedSkeletonPrim] path={} get_skinning_transforms:start",
                self.prim_path
            );
        }
        let skel_data = self.get_skel_data();
        let rest_transforms = skel_data.skeleton_schema.get_rest_transforms_data_source();
        let result = SkinningTransformsDataSource::new(
            skel_data,
            rest_transforms,
            self.resolved_animation_schemas(),
        ) as HdMatrix4fArrayDataSourceHandle;
        if std::env::var_os("USD_PROFILE_SKEL_DS").is_some() {
            eprintln!(
                "[ResolvedSkeletonPrim] path={} get_skinning_transforms:done",
                self.prim_path
            );
        }
        result
    }

    pub fn get_blend_shape_ranges(&self) -> HdVec2iArrayDataSourceHandle {
        BlendShapeRangesDataSource::new(self.resolved_animation_schemas())
            as HdVec2iArrayDataSourceHandle
    }

    pub fn get_blend_shapes(&self) -> HdTokenArrayDataSourceHandle {
        let schemas = self.resolved_animation_schemas();
        let ranges = BlendShapeRangesDataSource::new(schemas.clone());
        BlendShapesDataSource::new(schemas, ranges) as HdTokenArrayDataSourceHandle
    }

    pub fn get_blend_shape_weights(&self) -> HdFloatArrayDataSourceHandle {
        let schemas = self.resolved_animation_schemas();
        let ranges = BlendShapeRangesDataSource::new(schemas.clone());
        BlendShapeWeightsDataSource::new(schemas, ranges) as HdFloatArrayDataSourceHandle
    }

    pub fn get_dependendend_on_data_source_locators() -> HdDataSourceLocatorSet {
        let mut result = HdDataSourceLocatorSet::new();
        result.insert(SkeletonSchema::get_default_locator());
        result.insert(BindingSchema::get_animation_source_locator());
        result.insert(DataSourceXformResolver::get_xform_locator());
        result
    }

    pub fn process_dirty_locators(
        &self,
        dirtied_prim_type: &Token,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: Option<&mut Vec<DirtiedPrimEntry>>,
    ) -> bool {
        let mut new_dirty = HdDataSourceLocatorSet::new();
        let mut resync = false;

        if *dirtied_prim_type == Token::new("skeleton") {
            if dirty_locators.contains(&SkeletonSchema::get_default_locator())
                || dirty_locators.contains(&BindingSchema::get_animation_source_locator())
                || dirty_locators.contains(&DataSourceXformResolver::get_instanced_by_locator())
            {
                resync = true;
            }

            let mut skel_data_locators = HdDataSourceLocatorSet::new();
            skel_data_locators.insert(SkeletonSchema::get_joints_locator());
            skel_data_locators.insert(SkeletonSchema::get_bind_transforms_locator());
            if dirty_locators.intersects(&skel_data_locators) {
                self.skel_data_cache.invalidate();
                self.skel_guide_data_cache.invalidate();
                new_dirty.insert(ResolvedSkeletonSchema::get_skinning_transforms_locator());
                new_dirty.insert(topology_locator());
                if usd_hd::skinning_settings::is_skinning_deferred() {
                    new_dirty.insert(HdPrimvarsSchema::get_default_locator());
                } else {
                    new_dirty.insert(points_primvar_value_locator());
                }
            }

            if dirty_locators.contains(&SkeletonSchema::get_rest_transforms_locator()) {
                new_dirty.insert(ResolvedSkeletonSchema::get_skinning_transforms_locator());
                if usd_hd::skinning_settings::is_skinning_deferred() {
                    new_dirty.insert(HdPrimvarsSchema::get_default_locator());
                } else {
                    new_dirty.insert(points_primvar_value_locator());
                }
            }

            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                DataSourceXformResolver::get_xform_locator(),
            )) {
                new_dirty.insert(ResolvedSkeletonSchema::get_skel_local_to_common_space_locator());
            }
        } else if *dirtied_prim_type == Token::new("skelAnimation") {
            if dirty_locators.contains(&AnimationSchema::get_default_locator())
                || dirty_locators.contains(&DataSourceXformResolver::get_instanced_by_locator())
            {
                resync = true;
            }

            if dirty_locators.contains(&AnimationSchema::get_joints_locator()) {
                self.skel_data_cache.invalidate();
                self.skel_guide_data_cache.invalidate();
                new_dirty.insert(ResolvedSkeletonSchema::get_skinning_transforms_locator());
                if usd_hd::skinning_settings::is_skinning_deferred() {
                    new_dirty.insert(HdPrimvarsSchema::get_default_locator());
                } else {
                    new_dirty.insert(points_primvar_value_locator());
                }
            }

            let mut transforms = HdDataSourceLocatorSet::new();
            transforms.insert(AnimationSchema::get_translations_locator());
            transforms.insert(AnimationSchema::get_rotations_locator());
            transforms.insert(AnimationSchema::get_scales_locator());
            if dirty_locators.intersects(&transforms) {
                new_dirty.insert(ResolvedSkeletonSchema::get_skinning_transforms_locator());
                if usd_hd::skinning_settings::is_skinning_deferred() {
                    new_dirty.insert(HdPrimvarsSchema::get_default_locator());
                } else {
                    new_dirty.insert(points_primvar_value_locator());
                }
            }

            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                AnimationSchema::get_blend_shapes_locator(),
            )) {
                new_dirty.insert(ResolvedSkeletonSchema::get_blend_shapes_locator());
                new_dirty.insert(ResolvedSkeletonSchema::get_blend_shape_ranges_locator());
            }
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                AnimationSchema::get_blend_shape_weights_locator(),
            )) {
                new_dirty.insert(ResolvedSkeletonSchema::get_blend_shape_weights_locator());
                new_dirty.insert(ResolvedSkeletonSchema::get_blend_shape_ranges_locator());
            }
        } else if *dirtied_prim_type == Token::new("instancer") {
            if dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                DataSourceXformResolver::get_instanced_by_locator(),
            )) || (self.should_resolve_instance_animation()
                && dirty_locators.intersects(&HdDataSourceLocatorSet::from_locator(
                    DataSourceXformResolver::get_instance_animation_source_locator(),
                )))
            {
                resync = true;
            }

            let mut instancer_locators = HdDataSourceLocatorSet::new();
            instancer_locators.insert(DataSourceXformResolver::get_xform_locator());
            instancer_locators.insert(DataSourceXformResolver::get_instance_xform_locator());
            if dirty_locators.intersects(&instancer_locators) {
                new_dirty.insert(ResolvedSkeletonSchema::get_skel_local_to_common_space_locator());
            }
        }

        if let Some(entries) = entries {
            if !new_dirty.is_empty() {
                entries.push(DirtiedPrimEntry::new(self.prim_path.clone(), new_dirty));
            }
        }

        resync
    }
}

impl HdDataSourceBase for DataSourceResolvedSkeletonPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        self.clone_handle()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(self.clone_handle())
    }
}

impl HdContainerDataSource for DataSourceResolvedSkeletonPrim {
    fn get_names(&self) -> Vec<Token> {
        vec![
            ResolvedSkeletonSchema::get_schema_token(),
            mesh_token(),
            primvars_token(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == ResolvedSkeletonSchema::get_schema_token() {
            return Some(ResolvedSkeletonSchemaDataSource::new(self.clone_handle())
                as HdDataSourceBaseHandle);
        }

        if *name == mesh_token() {
            let skel_data = self.get_skel_data();
            let (topology, _) = super::utils::compute_bone_topology(&skel_data.topology)?;
            let topology_container = HdMeshTopologySchema::build_retained(
                Some(HdRetainedTypedSampledDataSource::new(Array::from(
                    topology.face_vertex_counts,
                ))),
                Some(HdRetainedTypedSampledDataSource::new(Array::from(
                    topology.face_vertex_indices,
                ))),
                None,
                Some(HdRetainedTypedSampledDataSource::new(Token::new(
                    "rightHanded",
                ))),
            );
            return Some(HdMeshSchema::build_retained(
                Some(topology_container),
                Some(HdRetainedTypedSampledDataSource::new(Token::new("none"))),
                None,
                Some(HdRetainedTypedSampledDataSource::new(true)),
            ) as HdDataSourceBaseHandle);
        }

        if *name == primvars_token() {
            if std::env::var_os("USD_PROFILE_SKEL_DS").is_some() {
                eprintln!(
                    "[ResolvedSkeletonPrim] path={} get:primvars",
                    self.prim_path
                );
            }
            let skinning_xforms = self.get_skinning_transforms();
            if usd_hd::skinning_settings::is_skinning_deferred() {
                return Some(SkelGuideSkinningPrimvarsDataSource::new(
                    self.get_skel_guide_data(),
                    skinning_xforms,
                ) as HdDataSourceBaseHandle);
            }
            return Some(HdRetainedContainerDataSource::from_entries(&[(
                points_token(),
                DataSourcePrimvar::new(
                    PointsPrimvarValueDataSource::new(self.get_skel_guide_data(), skinning_xforms)
                        as HdDataSourceBaseHandle,
                    VERTEX.clone(),
                    POINT.clone(),
                ) as HdDataSourceBaseHandle,
            )]) as HdDataSourceBaseHandle);
        }

        None
    }
}
