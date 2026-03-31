//! UsdSkelSkinningQuery - object for querying resolved skinning bindings.
//!
//! Port of pxr/usd/usdSkel/skinningQuery.h/cpp

use super::anim_mapper::AnimMapper;
use super::binding_api::BindingAPI;
use super::tokens::tokens;
use super::utils::compute_joints_extent;
use std::sync::Arc;
use usd_core::{Attribute, Prim, Relationship};
use usd_geom::boundable::Boundable;
use usd_geom::primvar::Primvar;
use usd_gf::{Matrix3d, Matrix4d, Range3f, Vec3f};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Skinning query flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct SkinningFlags(u32);

impl SkinningFlags {
    const HAS_JOINT_INFLUENCES: u32 = 0x1;
    const HAS_BLEND_SHAPES: u32 = 0x2;

    fn has_joint_influences(&self) -> bool {
        (self.0 & Self::HAS_JOINT_INFLUENCES) != 0
    }

    fn has_blend_shapes(&self) -> bool {
        (self.0 & Self::HAS_BLEND_SHAPES) != 0
    }
}

/// Object used for querying resolved bindings for skinning.
///
/// Matches C++ `UsdSkelSkinningQuery`.
#[derive(Clone)]
pub struct SkinningQuery {
    /// The skinned prim.
    prim: Option<Prim>,
    /// Path to the bound skeleton.
    skeleton_path: Option<Path>,
    /// Number of influences per component.
    num_influences_per_component: usize,
    /// Flags.
    flags: SkinningFlags,
    /// Interpolation mode.
    interpolation: Token,
    /// Joint indices attribute.
    joint_indices_attr: Option<Attribute>,
    /// Joint weights attribute.
    joint_weights_attr: Option<Attribute>,
    /// Skinning method attribute.
    skinning_method_attr: Option<Attribute>,
    /// Geometry bind transform attribute.
    geom_bind_transform_attr: Option<Attribute>,
    /// Blend shapes attribute.
    blend_shapes_attr: Option<Attribute>,
    /// Blend shape targets relationship.
    blend_shape_targets_rel: Option<Relationship>,
    /// Joint mapper (from skeleton order to local order).
    joint_mapper: Option<Arc<AnimMapper>>,
    /// Blend shape mapper.
    blend_shape_mapper: Option<Arc<AnimMapper>>,
    /// Local joint order.
    joint_order: Option<Vec<Token>>,
    /// Local blend shape order.
    blend_shape_order: Option<Vec<Token>>,
}

impl Default for SkinningQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl SkinningQuery {
    /// Construct an invalid skinning query.
    pub fn new() -> Self {
        Self {
            prim: None,
            skeleton_path: None,
            num_influences_per_component: 1,
            flags: SkinningFlags::default(),
            interpolation: Token::empty(),
            joint_indices_attr: None,
            joint_weights_attr: None,
            skinning_method_attr: None,
            geom_bind_transform_attr: None,
            blend_shapes_attr: None,
            blend_shape_targets_rel: None,
            joint_mapper: None,
            blend_shape_mapper: None,
            joint_order: None,
            blend_shape_order: None,
        }
    }

    /// Construct a skinning query from a binding API.
    pub fn from_binding(binding_api: &BindingAPI) -> Self {
        let prim = binding_api.prim();
        if !prim.is_valid() {
            return Self::new();
        }

        let mut query = Self::new();
        query.prim = Some(prim.clone());

        // Get skeleton path from binding
        if let Some(skel_rel) = binding_api.get_skeleton_rel() {
            if let Some(path) = skel_rel.get_targets().first() {
                query.skeleton_path = Some(path.clone());
            }
        }

        // Get joint indices and weights
        let joint_indices = binding_api.get_joint_indices_attr();
        let joint_weights = binding_api.get_joint_weights_attr();
        let skinning_method = binding_api.get_skinning_method_attr();
        let geom_bind_transform = binding_api.get_geom_bind_transform_attr();
        let joints = binding_api.get_joints_attr();
        let blend_shapes = binding_api.get_blend_shapes_attr();
        let blend_shape_targets = binding_api.get_blend_shape_targets_rel();

        // Initialize joint influence bindings
        query.init_joint_influence_bindings(joint_indices, joint_weights, &[], joints);

        // Initialize blend shape bindings
        query.init_blend_shape_bindings(blend_shapes, blend_shape_targets, &[]);

        // Store other attributes
        query.skinning_method_attr = skinning_method;
        query.geom_bind_transform_attr = geom_bind_transform;

        query
    }

    /// Construct a skinning query for resolved properties set through UsdSkelBindingAPI.
    #[allow(clippy::too_many_arguments)]
    pub fn from_bindings(
        prim: Prim,
        skel_joint_order: &[Token],
        blend_shape_order: &[Token],
        joint_indices: Option<Attribute>,
        joint_weights: Option<Attribute>,
        skinning_method: Option<Attribute>,
        geom_bind_transform: Option<Attribute>,
        joints: Option<Attribute>,
        blend_shapes: Option<Attribute>,
        blend_shape_targets: Option<Relationship>,
    ) -> Self {
        let mut query = Self::new();
        query.prim = Some(prim);
        query.skinning_method_attr = skinning_method;
        query.geom_bind_transform_attr = geom_bind_transform;

        // Initialize joint influence bindings
        query.init_joint_influence_bindings(joint_indices, joint_weights, skel_joint_order, joints);

        // Initialize blend shape bindings
        query.init_blend_shape_bindings(blend_shapes, blend_shape_targets, blend_shape_order);

        query
    }

    fn init_joint_influence_bindings(
        &mut self,
        joint_indices: Option<Attribute>,
        joint_weights: Option<Attribute>,
        skel_joint_order: &[Token],
        joints_attr: Option<Attribute>,
    ) {
        let (indices, weights) = match (joint_indices, joint_weights) {
            (Some(i), Some(w)) => (i, w),
            _ => return,
        };

        // Store attributes
        self.joint_indices_attr = Some(indices.clone());
        self.joint_weights_attr = Some(weights.clone());

        // Read interpolation and element size from primvar metadata
        // (matches C++ which stores UsdGeomPrimvar and reads GetElementSize/GetInterpolation)
        let indices_primvar = Primvar::new(indices);
        let weights_primvar = Primvar::new(weights);

        let indices_element_size = indices_primvar.get_element_size();
        let weights_element_size = weights_primvar.get_element_size();

        if indices_element_size != weights_element_size {
            eprintln!(
                "jointIndices element size ({}) != jointWeights element size ({}).",
                indices_element_size, weights_element_size
            );
            return;
        }

        if indices_element_size <= 0 {
            eprintln!(
                "Invalid element size [{}]: element size must be greater than zero.",
                indices_element_size
            );
            return;
        }

        let indices_interp = indices_primvar.get_interpolation();
        let weights_interp = weights_primvar.get_interpolation();

        if indices_interp != weights_interp {
            eprintln!(
                "jointIndices interpolation ({}) != jointWeights interpolation ({}).",
                indices_interp.as_str(),
                weights_interp.as_str()
            );
            return;
        }

        if indices_interp != "constant" && indices_interp != "vertex" {
            eprintln!(
                "Invalid interpolation ({}) for joint influences: \
                 interpolation must be either 'constant' or 'vertex'.",
                indices_interp.as_str()
            );
            return;
        }

        // Valid joint influences
        self.num_influences_per_component = indices_element_size as usize;
        self.interpolation = indices_interp;

        self.flags.0 |= SkinningFlags::HAS_JOINT_INFLUENCES;

        // Get local joint order and create mapper
        if let Some(joints) = joints_attr {
            if let Some(local_order) = joints.get_typed_vec::<Token>(TimeCode::default()) {
                self.joint_order = Some(local_order.clone());
                // Create mapper from skeleton order to local order
                self.joint_mapper = Some(Arc::new(AnimMapper::from_orders(
                    skel_joint_order,
                    &local_order,
                )));
            }
        }
    }

    fn init_blend_shape_bindings(
        &mut self,
        blend_shapes: Option<Attribute>,
        blend_shape_targets: Option<Relationship>,
        skel_blend_shape_order: &[Token],
    ) {
        let Some(bs_attr) = blend_shapes else {
            return;
        };

        self.blend_shapes_attr = Some(bs_attr.clone());
        self.blend_shape_targets_rel = blend_shape_targets;

        // Get local blend shape order
        if let Some(local_order) = bs_attr.get_typed_vec::<Token>(TimeCode::default()) {
            if !local_order.is_empty() {
                self.blend_shape_order = Some(local_order.clone());
                self.flags.0 |= SkinningFlags::HAS_BLEND_SHAPES;

                // Create mapper from animation blend shape order to local order
                if !skel_blend_shape_order.is_empty() {
                    self.blend_shape_mapper = Some(Arc::new(AnimMapper::from_orders(
                        skel_blend_shape_order,
                        &local_order,
                    )));
                }
            }
        }
    }

    /// Returns true if this query is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_some()
    }

    /// Get the skinned prim.
    pub fn get_prim(&self) -> Option<&Prim> {
        self.prim.as_ref()
    }

    /// Returns true if there are blend shapes associated with this prim.
    pub fn has_blend_shapes(&self) -> bool {
        self.flags.has_blend_shapes()
    }

    /// Returns true if joint influence data is associated with this prim.
    pub fn has_joint_influences(&self) -> bool {
        self.flags.has_joint_influences()
    }

    /// Returns the number of influences encoded for each component.
    pub fn get_num_influences_per_component(&self) -> usize {
        self.num_influences_per_component
    }

    /// Get the interpolation mode.
    pub fn get_interpolation(&self) -> &Token {
        &self.interpolation
    }

    /// Returns true if the prim has the same joint influences across all points.
    /// Matches C++ which checks `_interpolation == UsdGeomTokens->constant`.
    pub fn is_rigidly_deformed(&self) -> bool {
        self.interpolation == "constant"
    }

    /// Get the skinning method attribute.
    pub fn get_skinning_method_attr(&self) -> Option<&Attribute> {
        self.skinning_method_attr.as_ref()
    }

    /// Get the geometry bind transform attribute.
    pub fn get_geom_bind_transform_attr(&self) -> Option<&Attribute> {
        self.geom_bind_transform_attr.as_ref()
    }

    /// Get the joint indices attribute.
    pub fn get_joint_indices_attr(&self) -> Option<&Attribute> {
        self.joint_indices_attr.as_ref()
    }

    /// Get the joint weights attribute.
    pub fn get_joint_weights_attr(&self) -> Option<&Attribute> {
        self.joint_weights_attr.as_ref()
    }

    /// Get the blend shapes attribute.
    pub fn get_blend_shapes_attr(&self) -> Option<&Attribute> {
        self.blend_shapes_attr.as_ref()
    }

    /// Get the blend shape targets relationship.
    pub fn get_blend_shape_targets_rel(&self) -> Option<&Relationship> {
        self.blend_shape_targets_rel.as_ref()
    }

    /// Return the mapper for remapping from skeleton joint order to local joint order.
    pub fn get_joint_mapper(&self) -> Option<&AnimMapper> {
        self.joint_mapper.as_ref().map(|m| m.as_ref())
    }

    /// Deprecated: Use get_joint_mapper.
    pub fn get_mapper(&self) -> Option<&AnimMapper> {
        self.get_joint_mapper()
    }

    /// Return the mapper for remapping blend shapes.
    pub fn get_blend_shape_mapper(&self) -> Option<&AnimMapper> {
        self.blend_shape_mapper.as_ref().map(|m| m.as_ref())
    }

    /// Get the custom joint order for this skinning site.
    pub fn get_joint_order(&self) -> Option<&[Token]> {
        self.joint_order.as_deref()
    }

    /// Get the blend shapes for this skinning site.
    pub fn get_blend_shape_order(&self) -> Option<&[Token]> {
        self.blend_shape_order.as_deref()
    }

    /// Get time samples for all properties that affect skinning.
    pub fn get_time_samples(&self, times: &mut Vec<f64>) -> bool {
        self.get_time_samples_in_interval(f64::NEG_INFINITY, f64::INFINITY, times)
    }

    /// Get time samples in interval for all properties that affect skinning.
    pub fn get_time_samples_in_interval(&self, start: f64, end: f64, times: &mut Vec<f64>) -> bool {
        times.clear();

        // Collect time samples from relevant attributes
        let attrs: Vec<Option<&Attribute>> = vec![
            self.joint_indices_attr.as_ref(),
            self.joint_weights_attr.as_ref(),
            self.geom_bind_transform_attr.as_ref(),
        ];

        for attr_opt in attrs.into_iter().flatten() {
            for t in attr_opt.get_time_samples_in_interval(start, end) {
                if !times.contains(&t) {
                    times.push(t);
                }
            }
        }

        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        true
    }

    /// Compute joint influences (indices and weights).
    pub fn compute_joint_influences(
        &self,
        indices: &mut Vec<i32>,
        weights: &mut Vec<f32>,
        time: &TimeCode,
    ) -> bool {
        let Some(ref indices_attr) = self.joint_indices_attr else {
            return false;
        };
        let Some(ref weights_attr) = self.joint_weights_attr else {
            return false;
        };

        let Some(idx_data) = indices_attr.get_typed::<Vec<i32>>(*time) else {
            return false;
        };
        let Some(wgt_data) = weights_attr.get_typed::<Vec<f32>>(*time) else {
            return false;
        };

        // Validate sizes match
        if idx_data.len() != wgt_data.len() {
            return false;
        }

        *indices = idx_data;
        *weights = wgt_data;
        true
    }

    /// Compute varying joint influences (expanded for constant influences).
    pub fn compute_varying_joint_influences(
        &self,
        num_points: usize,
        indices: &mut Vec<i32>,
        weights: &mut Vec<f32>,
        time: &TimeCode,
    ) -> bool {
        if !self.compute_joint_influences(indices, weights, time) {
            return false;
        }

        if self.is_rigidly_deformed() && num_points > 0 {
            // Expand constant influences to per-point
            let num_influences = self.num_influences_per_component;
            let original_indices = indices.clone();
            let original_weights = weights.clone();

            indices.clear();
            weights.clear();
            indices.reserve(num_points * num_influences);
            weights.reserve(num_points * num_influences);

            for _ in 0..num_points {
                indices.extend_from_slice(&original_indices);
                weights.extend_from_slice(&original_weights);
            }
        }

        true
    }

    /// Get the skinning method token.
    pub fn get_skinning_method(&self) -> Token {
        if let Some(ref attr) = self.skinning_method_attr {
            if let Some(method) = attr.get_typed::<Token>(TimeCode::default()) {
                return method;
            }
        }
        // Default to linear blend skinning
        tokens().classic_linear.clone()
    }

    /// Get the geometry bind transform.
    pub fn get_geom_bind_transform(&self, time: &TimeCode) -> Matrix4d {
        if let Some(ref attr) = self.geom_bind_transform_attr {
            if let Some(xform) = attr.get_typed::<Matrix4d>(*time) {
                return xform;
            }
        }
        Matrix4d::identity()
    }

    /// Get the path to the bound skeleton.
    pub fn get_skeleton_path(&self) -> Option<Path> {
        self.skeleton_path.clone()
    }

    /// Compute an approximate padding for use in extents computations.
    ///
    /// The padding is computed as the difference between the pivots of the
    /// `skel_rest_xforms` (skeleton-space joint transforms at rest) and the
    /// extents of the skinned primitive.
    ///
    /// Matches C++ `ComputeExtentsPadding()`.
    pub fn compute_extents_padding(
        &self,
        skel_rest_xforms: &[Matrix4d],
        boundable: &Boundable,
        time: &TimeCode,
    ) -> f32 {
        // Get boundable extent
        let extent_attr = boundable.get_extent_attr();
        if !extent_attr.is_valid() {
            return 0.0;
        }
        let extent = match extent_attr.get_typed::<Vec<Vec3f>>(*time) {
            Some(e) if e.len() == 2 => e,
            _ => return 0.0,
        };

        // Compute joints extent from rest transforms
        let mut joints_range = Range3f::default();
        if !compute_joints_extent(skel_rest_xforms, &mut joints_range, 0.0, None) {
            return 0.0;
        }

        // Get the aligned range of the gprim in its bind pose
        let geom_bind = self.get_geom_bind_transform(time);
        // Transform extent corners by geom bind transform
        let min_pt = geom_bind.transform_point(&extent[0].into());
        let max_pt = geom_bind.transform_point(&extent[1].into());
        let gprim_min = Vec3f::new(min_pt.x as f32, min_pt.y as f32, min_pt.z as f32);
        let gprim_max = Vec3f::new(max_pt.x as f32, max_pt.y as f32, max_pt.z as f32);

        // Compute padding as max difference between joint range and gprim range
        let joints_min = *joints_range.min();
        let joints_max = *joints_range.max();
        let min_diff = joints_min.clone() - gprim_min;
        let max_diff = gprim_max - joints_max.clone();

        let mut padding = 0.0f32;
        for i in 0..3 {
            padding = padding.max(min_diff[i]);
            padding = padding.max(max_diff[i]);
        }

        padding
    }

    /// Get a description string.
    pub fn get_description(&self) -> String {
        if let Some(ref prim) = self.prim {
            format!("SkinningQuery for {}", prim.path().get_string())
        } else {
            "Invalid SkinningQuery".to_string()
        }
    }

    /// Compute skinned points.
    ///
    /// Both `xforms` and `points` are given in skeleton space.
    /// Joint influences and binding transform are computed at `time`.
    /// If a joint_mapper exists, remaps xforms from skel order to binding order first.
    pub fn compute_skinned_points(
        &self,
        xforms: &[Matrix4d],
        points: &mut Vec<Vec3f>,
        time: &TimeCode,
    ) -> bool {
        if !self.has_joint_influences() {
            return false;
        }
        // Empty points is a no-op success (matches C++ which returns true for 0 points).
        if points.is_empty() {
            return true;
        }

        let mut indices = Vec::new();
        let mut weights = Vec::new();

        if !self.compute_varying_joint_influences(points.len(), &mut indices, &mut weights, time) {
            return false;
        }

        // Remap joint transforms from skeleton order to binding order if needed.
        // C++: VtArray<Matrix4> orderedXforms(xforms) then _jointMapper->RemapTransforms(xforms, &orderedXforms).
        // Pre-copy xforms so that unmapped target positions keep the xform at the same index
        // (matching C++ behavior where _ResizeContainer preserves existing array elements).
        let ordered_xforms_storage: Vec<Matrix4d>;
        let ordered_xforms: &[Matrix4d] = if let Some(mapper) = &self.joint_mapper {
            // Pre-initialize to a copy of xforms (matching C++ orderedXforms(xforms) initialization)
            let mut remapped = xforms.to_vec();
            if !mapper.remap_transforms_4d(xforms, &mut remapped, 1) {
                return false;
            }
            ordered_xforms_storage = remapped;
            &ordered_xforms_storage
        } else {
            xforms
        };

        let geom_bind_transform = self.get_geom_bind_transform(time);
        let skinning_method = self.get_skinning_method();

        // Delegate to utils::skin_points which handles LBS/DQS dispatch.
        super::utils::skin_points(
            &skinning_method,
            &geom_bind_transform,
            ordered_xforms,
            &indices,
            &weights,
            self.num_influences_per_component,
            points,
        )
    }

    /// Compute skinned normals.
    ///
    /// Remaps xforms (skel->binding order), builds inv-transpose 3x3s, then
    /// delegates to `utils::skin_normals` (LBS or DQS per skinning method).
    /// Matches C++ `UsdSkelSkinningQuery::ComputeSkinnedNormals()`.
    pub fn compute_skinned_normals(
        &self,
        xforms: &[Matrix4d],
        normals: &mut Vec<Vec3f>,
        time: &TimeCode,
    ) -> bool {
        if !self.has_joint_influences() {
            return false;
        }
        // Empty normals is a no-op success (matches C++ which returns true for 0 normals).
        if normals.is_empty() {
            return true;
        }

        let mut indices = Vec::new();
        let mut weights = Vec::new();

        if !self.compute_varying_joint_influences(normals.len(), &mut indices, &mut weights, time) {
            return false;
        }

        // Remap joint transforms from skeleton order to binding order if needed.
        // Pre-copy xforms so unmapped target positions keep the xform at the same index.
        let ordered_xforms_storage: Vec<Matrix4d>;
        let ordered_xforms: &[Matrix4d] = if let Some(mapper) = &self.joint_mapper {
            let mut remapped = xforms.to_vec(); // pre-copy matches C++ orderedXforms(xforms)
            if !mapper.remap_transforms_4d(xforms, &mut remapped, 1) {
                return false;
            }
            ordered_xforms_storage = remapped;
            &ordered_xforms_storage
        } else {
            xforms
        };

        // Build inv-transpose 3x3 from each ordered joint xform.
        // Matches C++: orderedXforms[i].ExtractRotationMatrix().GetInverse().GetTranspose()
        let inv_transpose_xforms: Vec<Matrix3d> = ordered_xforms
            .iter()
            .map(|m| {
                let rot = m.extract_rotation_matrix();
                rot.inverse().unwrap_or_else(Matrix3d::identity).transpose()
            })
            .collect();

        // Inv-transpose of geom bind 3x3
        let geom_bind = self.get_geom_bind_transform(time);
        let geom_bind_inv_transpose = geom_bind
            .extract_rotation_matrix()
            .inverse()
            .unwrap_or_else(Matrix3d::identity)
            .transpose();

        let skinning_method = self.get_skinning_method();

        // Delegate to utils::skin_normals (handles LBS and DQS dispatch).
        super::utils::skin_normals(
            &skinning_method,
            &geom_bind_inv_transpose,
            &inv_transpose_xforms,
            &indices,
            &weights,
            self.num_influences_per_component,
            normals,
        )
    }

    /// Compute skinned normals with face-varying interpolation.
    ///
    /// Uses `face_vertex_indices` to map per-face-vertex normals to per-point
    /// joint influences. Matches C++ `UsdSkelSkinFaceVaryingNormals()`.
    pub fn compute_skinned_facevarying_normals(
        &self,
        xforms: &[Matrix4d],
        normals: &mut Vec<Vec3f>,
        face_vertex_indices: &[i32],
        time: &TimeCode,
    ) -> bool {
        if !self.has_joint_influences() || normals.is_empty() {
            return false;
        }

        if face_vertex_indices.len() != normals.len() {
            eprintln!(
                "Size of faceVertexIndices [{}] != size of normals [{}].",
                face_vertex_indices.len(),
                normals.len()
            );
            return false;
        }

        // Determine num_points from the max face vertex index + 1
        let num_points = face_vertex_indices
            .iter()
            .filter(|&&i| i >= 0)
            .map(|&i| i as usize + 1)
            .max()
            .unwrap_or(0);

        let mut indices = Vec::new();
        let mut weights = Vec::new();

        if !self.compute_varying_joint_influences(num_points, &mut indices, &mut weights, time) {
            return false;
        }

        // Remap joint transforms from skeleton order to binding order if needed.
        // Pre-copy xforms so unmapped target positions keep the xform at the same index.
        let ordered_xforms_storage: Vec<Matrix4d>;
        let ordered_xforms: &[Matrix4d] = if let Some(mapper) = &self.joint_mapper {
            let mut remapped = xforms.to_vec(); // pre-copy matches C++ orderedXforms(xforms)
            if !mapper.remap_transforms_4d(xforms, &mut remapped, 1) {
                return false;
            }
            ordered_xforms_storage = remapped;
            &ordered_xforms_storage
        } else {
            xforms
        };

        // Build inv-transpose 3x3 from each ordered joint xform.
        let inv_transpose_xforms: Vec<Matrix3d> = ordered_xforms
            .iter()
            .map(|m| {
                let rot = m.extract_rotation_matrix();
                rot.inverse().unwrap_or_else(Matrix3d::identity).transpose()
            })
            .collect();

        // Inv-transpose of geom bind 3x3
        let geom_bind = self.get_geom_bind_transform(time);
        let geom_bind_inv_transpose = geom_bind
            .extract_rotation_matrix()
            .inverse()
            .unwrap_or_else(Matrix3d::identity)
            .transpose();

        let skinning_method = self.get_skinning_method();
        let num_influences = self.num_influences_per_component;

        // Delegate to face-varying skin_normals from utils (handles LBS/DQS dispatch).
        super::utils::skin_face_varying_normals(
            &skinning_method,
            &geom_bind_inv_transpose,
            &inv_transpose_xforms,
            &indices,
            &weights,
            num_influences,
            face_vertex_indices,
            normals,
        )
    }

    /// Compute a skinned transform (for rigid deformation only).
    /// Remaps xforms (skel->binding order) then delegates to utils::skin_transform.
    /// Matches C++ `UsdSkelSkinningQuery::ComputeSkinnedTransform()`.
    pub fn compute_skinned_transform(
        &self,
        xforms: &[Matrix4d],
        xform: &mut Matrix4d,
        time: &TimeCode,
    ) -> bool {
        if !self.is_rigidly_deformed() {
            return false;
        }

        let mut indices = Vec::new();
        let mut weights = Vec::new();

        if !self.compute_joint_influences(&mut indices, &mut weights, time) {
            return false;
        }

        // Remap joint transforms from skeleton order to binding order if needed.
        // Pre-copy xforms so unmapped target positions keep the xform at the same index.
        let ordered_xforms_storage: Vec<Matrix4d>;
        let ordered_xforms: &[Matrix4d] = if let Some(mapper) = &self.joint_mapper {
            let mut remapped = xforms.to_vec(); // pre-copy matches C++ orderedXforms(xforms)
            if !mapper.remap_transforms_4d(xforms, &mut remapped, 1) {
                return false;
            }
            ordered_xforms_storage = remapped;
            &ordered_xforms_storage
        } else {
            xforms
        };

        let geom_bind_transform = self.get_geom_bind_transform(time);
        let skinning_method = self.get_skinning_method();

        // Delegate to utils::skin_transform (handles LBS and DQS dispatch).
        super::utils::skin_transform(
            &skinning_method,
            &geom_bind_transform,
            ordered_xforms,
            &indices,
            &weights,
            xform,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_query() {
        let query = SkinningQuery::new();
        assert!(!query.is_valid());
        assert!(!query.has_blend_shapes());
        assert!(!query.has_joint_influences());
    }
}
