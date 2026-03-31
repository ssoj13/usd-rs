//! JointInfluencesData - Data for skinning joint influences.
//!
//! Port of pxr/usdImaging/usdSkelImaging/jointInfluencesData.h/cpp
//!
//! Data feeding into ext computations - which points are influenced by which transforms.

use super::binding_schema::BindingSchema;
use super::data_source_utils::{
    ELEMENT_SIZE, INTERPOLATION, PRIMVAR_VALUE, get_typed_value_from_container_i32,
    get_typed_value_from_container_token, get_typed_value_from_container_vec_f32,
    get_typed_value_from_container_vec_i32,
};
use super::skeleton_schema::SkeletonSchema;
use usd_gf::vec2::Vec2f;
use usd_hd::HdContainerDataSourceHandle;
use usd_hd::schema::HdPrimvarsSchema;
use usd_skel::{AnimMapper, utils::interleave_influences};
use usd_tf::Token;

/// Data for skinning joint influences.
///
/// From SkelBindingAPI primvars - which points influenced by which transforms.
#[derive(Debug, Clone)]
pub struct JointInfluencesData {
    /// Each Vec2f is (joint index, weight).
    /// If has_constant_influences: num_influences_per_component elements total.
    /// Else: num_influences_per_component per point.
    pub influences: Vec<Vec2f>,

    /// True if all points have same influence pattern.
    pub has_constant_influences: bool,

    /// Number of (joint, weight) pairs per point (or total if constant).
    pub num_influences_per_component: i32,

    /// Remapping of joints in skeleton to skinning joints.
    pub joint_mapper: AnimMapper,
}

impl JointInfluencesData {
    /// Create new empty joint influences data.
    pub fn new() -> Self {
        Self {
            influences: Vec::new(),
            has_constant_influences: false,
            num_influences_per_component: 0,
            joint_mapper: AnimMapper::new(),
        }
    }
}

impl Default for JointInfluencesData {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute JointInfluencesData from SkelBindingAPI and Skeleton prim data sources.
pub fn compute_joint_influences_data(
    prim_source: &HdContainerDataSourceHandle,
    skeleton_prim_source: &HdContainerDataSourceHandle,
) -> JointInfluencesData {
    let mut data = JointInfluencesData::new();

    let primvars = HdPrimvarsSchema::get_from_parent(prim_source);
    if !primvars.is_defined() {
        return data;
    }

    let joint_indices_primvar =
        primvars.get_primvar(&BindingSchema::get_joint_indices_primvar_token());
    let joint_indices_container = match &joint_indices_primvar {
        Some(c) => c,
        None => return data,
    };

    // Interpolation - constant means hasConstantInfluences
    let interpolation =
        get_typed_value_from_container_token(joint_indices_container, &*INTERPOLATION);
    data.has_constant_influences = interpolation
        .as_ref()
        .map(|t: &Token| t == "constant")
        .unwrap_or(false);

    let joint_indices =
        get_typed_value_from_container_vec_i32(joint_indices_container, &*PRIMVAR_VALUE);
    let joint_indices = match joint_indices {
        Some(indices) => indices,
        None => return data,
    };
    if joint_indices.is_empty() {
        return data;
    }

    let joint_weights_primvar =
        primvars.get_primvar(&BindingSchema::get_joint_weights_primvar_token());
    let joint_weights_container = match &joint_weights_primvar {
        Some(c) => c,
        None => return data,
    };

    let joint_weights =
        get_typed_value_from_container_vec_f32(joint_weights_container, &*PRIMVAR_VALUE);
    let joint_weights = match joint_weights {
        Some(weights) => weights,
        None => return data,
    };
    if joint_weights.is_empty() {
        return data;
    }

    if let Some(elem_size) =
        get_typed_value_from_container_i32(joint_weights_container, &*ELEMENT_SIZE)
    {
        data.num_influences_per_component = elem_size;
    }
    if data.num_influences_per_component <= 0 {
        data.num_influences_per_component = 1;
    }

    data.influences
        .resize(joint_indices.len(), Vec2f::default());
    if !interleave_influences(&joint_indices, &joint_weights, &mut data.influences) {
        return data;
    }

    // Joint mapper: map from skeleton order to binding order.
    let binding_schema = BindingSchema::get_from_parent(prim_source);
    let skeleton_schema = SkeletonSchema::get_from_parent(skeleton_prim_source)
        .unwrap_or_else(|| SkeletonSchema::new(None));
    let binding_joints = binding_schema.get_joints();
    let skeleton_joints = skeleton_schema.get_joints();
    if !binding_joints.is_empty() && !skeleton_joints.is_empty() {
        data.joint_mapper = AnimMapper::from_orders(&skeleton_joints, &binding_joints);
    }

    data
}
