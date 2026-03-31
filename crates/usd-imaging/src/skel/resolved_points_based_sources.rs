//! Data sources for DataSourceResolvedPointsBasedPrim.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceResolvedPointsBasedPrim.cpp
//! inner classes: _MatrixInverseDataSource, _SkinningXformsDataSource,
//! _BlendShapeWeightsDataSource, _SkinningScaleXformsDataSource,
//! _SkinningDualQuatsDataSource.

use super::blend_shape_data::{BlendShapeData, compute_blend_shape_weights};
use super::joint_influences_data::JointInfluencesData;
use std::sync::Arc;
use usd_gf::matrix3::Matrix3f;
use usd_gf::matrix4::Matrix4f;
use usd_gf::quat::Quatf;
use usd_gf::vec2::Vec2i;
use usd_gf::vec4::Vec4f;
use usd_hd::HdSampledDataSource;
use usd_hd::data_source::{
    HdDataSourceBase, HdDataSourceBaseHandle, HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use usd_tf::Token;

type HdMatrix4fArrayDataSourceHandle =
    Arc<dyn HdTypedSampledDataSource<Vec<Matrix4f>> + Send + Sync>;
type HdMatrixDataSourceHandle =
    Arc<dyn HdTypedSampledDataSource<usd_gf::matrix4::Matrix4d> + Send + Sync>;

/// Matrix inverse data source - returns inverse of input matrix at each sample.
#[derive(Debug)]
pub struct MatrixInverseDataSource {
    input_src: HdMatrixDataSourceHandle,
    value_at_zero: usd_gf::matrix4::Matrix4d,
}

impl MatrixInverseDataSource {
    /// Creates a new matrix inverse data source from the input matrix source.
    pub fn new(input_src: HdMatrixDataSourceHandle) -> Arc<Self> {
        let value_at_zero = input_src
            .get_typed_value(0.0)
            .inverse()
            .unwrap_or_else(|| usd_gf::matrix4::Matrix4d::identity());
        Arc::new(Self {
            input_src,
            value_at_zero,
        })
    }
}

impl HdTypedSampledDataSource<usd_gf::matrix4::Matrix4d> for MatrixInverseDataSource {
    fn get_typed_value(&self, shutter_offset: f32) -> usd_gf::matrix4::Matrix4d {
        if (shutter_offset - 0.0).abs() < 1e-9 {
            return self.value_at_zero;
        }
        let m = self.input_src.get_typed_value(shutter_offset);
        m.inverse().unwrap_or(self.value_at_zero)
    }
}

impl HdSampledDataSource for MatrixInverseDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        let input = Arc::clone(&self.input_src) as Arc<dyn HdSampledDataSource>;
        input.get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdDataSourceBase for MatrixInverseDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_src: Arc::clone(&self.input_src),
            value_at_zero: self.value_at_zero,
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.value_at_zero))
    }
}

/// Skinng transforms data source - remaps via joint mapper.
#[derive(Debug)]
pub struct SkinngXformsDataSource {
    joint_influences_data: Arc<JointInfluencesData>,
    skel_skinning_xforms: HdMatrix4fArrayDataSourceHandle,
}

impl SkinngXformsDataSource {
    /// Creates a new skinning transforms data source.
    pub fn new(
        joint_influences_data: Arc<JointInfluencesData>,
        skel_skinning_xforms: HdMatrix4fArrayDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            joint_influences_data,
            skel_skinning_xforms,
        })
    }
}

impl HdTypedSampledDataSource<Vec<Matrix4f>> for SkinngXformsDataSource {
    fn get_typed_value(&self, shutter_offset: f32) -> Vec<Matrix4f> {
        let source = self.skel_skinning_xforms.get_typed_value(shutter_offset);
        let mut result = Vec::new();
        if self
            .joint_influences_data
            .joint_mapper
            .remap_transforms_4f(&source, &mut result, 1)
        {
            result
        } else {
            source
        }
    }
}

impl HdSampledDataSource for SkinngXformsDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        let input = Arc::clone(&self.skel_skinning_xforms) as Arc<dyn HdSampledDataSource>;
        input.get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdDataSourceBase for SkinngXformsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            joint_influences_data: Arc::clone(&self.joint_influences_data),
            skel_skinning_xforms: Arc::clone(&self.skel_skinning_xforms),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get_typed_value(0.0)))
    }
}

fn compute_skinning_scale_xform(skinning_xform: &Matrix4f) -> Matrix3f {
    // Factor: M = r * diag(s) * u * translate(t).
    // Extract scale+shear by removing rotation and translation.
    if let Some((_r, _s, mut factored_rot, translation, _p)) = skinning_xform.factor() {
        factored_rot.orthonormalize();
        let mut trans_mat = Matrix4f::identity();
        trans_mat.set_translate(&translation);
        let tmp_non_scale = factored_rot * trans_mat;
        if let Some(inv) = tmp_non_scale.inverse() {
            let product = *skinning_xform * inv;
            return product.extract_rotation_matrix();
        }
    }
    Matrix3f::identity()
}

fn compute_skinning_scale_xforms(skinning_xforms: &[Matrix4f]) -> Vec<Matrix3f> {
    skinning_xforms
        .iter()
        .map(compute_skinning_scale_xform)
        .collect()
}

/// Skinng scale transforms - extract 3x3 scale+shear from 4x4.
#[derive(Debug)]
pub struct SkinngScaleXformsDataSource {
    skinning_xforms: HdMatrix4fArrayDataSourceHandle,
}

impl SkinngScaleXformsDataSource {
    /// Creates a new skinning scale transforms data source.
    pub fn new(
        skinning_xforms: Arc<dyn HdTypedSampledDataSource<Vec<Matrix4f>> + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            skinning_xforms: skinning_xforms as HdMatrix4fArrayDataSourceHandle,
        })
    }
}

impl HdTypedSampledDataSource<Vec<Matrix3f>> for SkinngScaleXformsDataSource {
    fn get_typed_value(&self, shutter_offset: f32) -> Vec<Matrix3f> {
        let xforms = self.skinning_xforms.get_typed_value(shutter_offset);
        compute_skinning_scale_xforms(&xforms)
    }
}

impl HdSampledDataSource for SkinngScaleXformsDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        let input = Arc::clone(&self.skinning_xforms) as Arc<dyn HdSampledDataSource>;
        input.get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdDataSourceBase for SkinngScaleXformsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            skinning_xforms: Arc::clone(&self.skinning_xforms),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get_typed_value(0.0)))
    }
}

fn to_vec4f_from_quat(q: &Quatf) -> Vec4f {
    let img = q.imaginary();
    Vec4f::new(img.x, img.y, img.z, q.real())
}

fn compute_skinning_dual_quat(skinning_xform: &Matrix4f) -> usd_gf::DualQuatf {
    use usd_gf::DualQuatf;
    if let Some((_r, _s, mut factored_rot, translation, _p)) = skinning_xform.factor() {
        factored_rot.orthonormalize();
        let rotation_q = factored_rot.extract_rotation_quat();
        let rotation = Quatf::new(rotation_q.real(), *rotation_q.imaginary());
        DualQuatf::from_rotation_translation(&rotation, &translation)
    } else {
        DualQuatf::zero()
    }
}

fn compute_skinning_dual_quats(skinning_xforms: &[Matrix4f]) -> Vec<Vec4f> {
    let mut result = Vec::with_capacity(skinning_xforms.len() * 2);
    for xform in skinning_xforms {
        let dq = compute_skinning_dual_quat(xform);
        result.push(to_vec4f_from_quat(dq.real()));
        result.push(to_vec4f_from_quat(dq.dual()));
    }
    result
}

/// Skinng dual quats - convert 4x4 to dual quat pairs (2 Vec4f per joint).
#[derive(Debug)]
pub struct SkinngDualQuatsDataSource {
    skinning_xforms: HdMatrix4fArrayDataSourceHandle,
}

impl SkinngDualQuatsDataSource {
    /// Creates a new skinning dual quaternions data source.
    pub fn new(
        skinning_xforms: Arc<dyn HdTypedSampledDataSource<Vec<Matrix4f>> + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            skinning_xforms: skinning_xforms as HdMatrix4fArrayDataSourceHandle,
        })
    }
}

impl HdTypedSampledDataSource<Vec<Vec4f>> for SkinngDualQuatsDataSource {
    fn get_typed_value(&self, shutter_offset: f32) -> Vec<Vec4f> {
        let xforms = self.skinning_xforms.get_typed_value(shutter_offset);
        compute_skinning_dual_quats(&xforms)
    }
}

impl HdSampledDataSource for SkinngDualQuatsDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        let input = Arc::clone(&self.skinning_xforms) as Arc<dyn HdSampledDataSource>;
        input.get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdDataSourceBase for SkinngDualQuatsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            skinning_xforms: Arc::clone(&self.skinning_xforms),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get_typed_value(0.0)))
    }
}

/// Blend shape weights data source - computes weights per range from blend shape data.
#[derive(Debug)]
pub struct BlendShapeWeightsDataSource {
    blend_shape_data: Arc<BlendShapeData>,
    blend_shapes: Arc<dyn HdTypedSampledDataSource<Vec<Token>> + Send + Sync>,
    blend_shape_weights: Option<Arc<dyn HdTypedSampledDataSource<Vec<f32>> + Send + Sync>>,
    blend_shape_ranges: Arc<dyn HdTypedSampledDataSource<Vec<Vec2i>> + Send + Sync>,
}

impl BlendShapeWeightsDataSource {
    /// Creates a new blend shape weights data source.
    pub fn new(
        blend_shape_data: Arc<BlendShapeData>,
        blend_shapes: Arc<dyn HdTypedSampledDataSource<Vec<Token>> + Send + Sync>,
        blend_shape_weights: Option<Arc<dyn HdTypedSampledDataSource<Vec<f32>> + Send + Sync>>,
        blend_shape_ranges: Arc<dyn HdTypedSampledDataSource<Vec<Vec2i>> + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            blend_shape_data,
            blend_shapes,
            blend_shape_weights,
            blend_shape_ranges,
        })
    }
}

impl HdTypedSampledDataSource<Vec<f32>> for BlendShapeWeightsDataSource {
    fn get_typed_value(&self, shutter_offset: f32) -> Vec<f32> {
        let blend_shapes = self.blend_shapes.get_typed_value(shutter_offset);
        let blend_shape_weights = self
            .blend_shape_weights
            .as_ref()
            .map(|ds| ds.get_typed_value(shutter_offset))
            .unwrap_or_default();
        let blend_shape_ranges = self.blend_shape_ranges.get_typed_value(shutter_offset);

        let num_sub_shapes = self.blend_shape_data.num_sub_shapes;
        let mut result = vec![0.0f32; blend_shape_ranges.len() * num_sub_shapes];

        for (i, range) in blend_shape_ranges.iter().enumerate() {
            let start = range[0] as usize;
            let len = range[1] as usize;
            if start >= blend_shapes.len() || start + len > blend_shapes.len() {
                continue;
            }
            let shapes = &blend_shapes[start..start + len];
            let weights = if start + len <= blend_shape_weights.len() {
                &blend_shape_weights[start..start + len]
            } else {
                &[]
            };
            let out_weights = compute_blend_shape_weights(&self.blend_shape_data, shapes, weights);
            for (j, w) in out_weights.iter().enumerate() {
                if j < num_sub_shapes {
                    result[i * num_sub_shapes + j] = *w;
                }
            }
        }
        result
    }
}

impl HdSampledDataSource for BlendShapeWeightsDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        if let Some(ref ds) = self.blend_shape_weights {
            let input = Arc::clone(ds) as Arc<dyn HdSampledDataSource>;
            return input.get_contributing_sample_times(start_time, end_time, out_sample_times);
        }
        false
    }
}

impl HdDataSourceBase for BlendShapeWeightsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            blend_shape_data: Arc::clone(&self.blend_shape_data),
            blend_shapes: Arc::clone(&self.blend_shapes),
            blend_shape_weights: self.blend_shape_weights.clone(),
            blend_shape_ranges: Arc::clone(&self.blend_shape_ranges),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get_typed_value(0.0)))
    }
}
