//! UsdSkel utility functions.
//!
//! Port of pxr/usd/usdSkel/utils.h/cpp
//!
//! Collection of utility methods for skeletal animation computations including:
//! - Joint transform computations (local space, skeleton space)
//! - Transform decomposition and composition (TRS)
//! - Joint influence manipulation (normalization, sorting, resizing)
//! - Skinning implementations (Linear Blend Skinning, Dual Quaternion Skinning)
//! - Blend shape application
//!
//! # Skinning Methods
//!
//! Two skinning methods are supported:
//! - **Linear Blend Skinning (LBS)** - Also known as "classicLinear"
//! - **Dual Quaternion Skinning (DQS)** - Also known as "dualQuaternion"
//!
//! LBS is faster but can cause volume loss at joints. DQS preserves volume
//! better but is more computationally expensive.

use super::topology::Topology;
use usd_core::Prim;
use usd_gf::{
    DualQuatd, Matrix3d, Matrix4d, Matrix4f, Quatd, Quatf, Range3f, Vec2f, Vec3d, Vec3f, Vec3h,
    half::Half, quat::dot as quat_dot,
};
use usd_tf::Token;

/// Convert Matrix4d to Matrix4f (lossy f64->f32 conversion).
fn mat4d_to_f(m: &Matrix4d) -> Matrix4f {
    Matrix4f::from_array([
        [
            m[0][0] as f32,
            m[0][1] as f32,
            m[0][2] as f32,
            m[0][3] as f32,
        ],
        [
            m[1][0] as f32,
            m[1][1] as f32,
            m[1][2] as f32,
            m[1][3] as f32,
        ],
        [
            m[2][0] as f32,
            m[2][1] as f32,
            m[2][2] as f32,
            m[2][3] as f32,
        ],
        [
            m[3][0] as f32,
            m[3][1] as f32,
            m[3][2] as f32,
            m[3][3] as f32,
        ],
    ])
}

/// Skinning method token for classic linear blend skinning.
pub const SKINNING_METHOD_LBS: &str = "classicLinear";
/// Skinning method token for dual quaternion skinning.
pub const SKINNING_METHOD_DQS: &str = "dualQuaternion";

// ============================================================================
// Prim Type Checks
// ============================================================================

/// Returns true if `prim` is a valid skel animation source.
///
/// Matches C++ `UsdSkelIsSkelAnimationPrim()`.
pub fn is_skel_animation_prim(prim: &Prim) -> bool {
    if !prim.is_valid() {
        return false;
    }
    let type_name = prim.type_name();
    type_name == "SkelAnimation"
}

/// Returns true if `prim` is considered to be a skinnable primitive.
///
/// Whether or not the prim is actually skinned additionally depends on
/// whether or not the prim has a bound skeleton and proper joint influences.
///
/// Matches C++ `UsdSkelIsSkinnablePrim()`.
pub fn is_skinnable_prim(prim: &Prim) -> bool {
    let type_name = prim.type_name();
    let type_str = type_name.as_str();

    // Check if it's a boundable geometry type.
    // C++ uses IsA<UsdGeomBoundable>(); we match exact type names to avoid
    // false positives (e.g. "MeshLight" would incorrectly match starts_with).
    let is_boundable = matches!(
        type_str,
        "Mesh"
            | "Points"
            | "BasisCurves"
            | "NurbsCurves"
            | "NurbsPatch"
            | "Sphere"
            | "Cube"
            | "Cylinder"
            | "Cone"
            | "Capsule"
            | "PointInstancer"
    );

    // Not a skeleton or skel root
    let is_skel = type_str == "Skeleton" || type_str == "SkelRoot";

    is_boundable && !is_skel
}

// ============================================================================
// Joint Transform Utilities
// ============================================================================

/// Concatenate joint transforms from local space to skeleton space.
///
/// This concatenates transforms from `joint_local_xforms`, providing joint
/// transforms in joint-local space. The resulting transforms are written to
/// `xforms`, which must be the same size as `topology`.
///
/// If `root_xform` is not None, the resulting joint transforms include
/// that additional root transformation.
///
/// Matches C++ `UsdSkelConcatJointTransforms()`.
pub fn concat_joint_transforms(
    topology: &Topology,
    joint_local_xforms: &[Matrix4d],
    xforms: &mut [Matrix4d],
    root_xform: Option<&Matrix4d>,
) -> bool {
    let num_joints = topology.num_joints();

    if joint_local_xforms.len() != num_joints {
        eprintln!(
            "Size of joint_local_xforms [{}] != number of joints [{}]",
            joint_local_xforms.len(),
            num_joints
        );
        return false;
    }
    if xforms.len() != num_joints {
        eprintln!(
            "Size of xforms [{}] != number of joints [{}]",
            xforms.len(),
            num_joints
        );
        return false;
    }

    for i in 0..num_joints {
        let parent = topology.get_parent(i);
        if parent >= 0 {
            let parent_idx = parent as usize;
            if parent_idx < i {
                // Child transform = local * parent_skel
                xforms[i] = joint_local_xforms[i] * xforms[parent_idx];
            } else {
                if parent_idx == i {
                    eprintln!("Joint {} has itself as its parent.", i);
                } else {
                    eprintln!(
                        "Joint {} has mis-ordered parent {}. Joints are expected \
                        to be ordered with parent joints always coming before children.",
                        i, parent
                    );
                }
                return false;
            }
        } else {
            // Root joint
            xforms[i] = joint_local_xforms[i];
            if let Some(root) = root_xform {
                xforms[i] *= *root;
            }
        }
    }
    true
}

/// Single-precision overload of concat_joint_transforms.
pub fn concat_joint_transforms_f(
    topology: &Topology,
    joint_local_xforms: &[Matrix4f],
    xforms: &mut [Matrix4f],
    root_xform: Option<&Matrix4f>,
) -> bool {
    let num_joints = topology.num_joints();

    if joint_local_xforms.len() != num_joints || xforms.len() != num_joints {
        return false;
    }

    for i in 0..num_joints {
        let parent = topology.get_parent(i);
        if parent >= 0 {
            let parent_idx = parent as usize;
            if parent_idx < i {
                xforms[i] = joint_local_xforms[i] * xforms[parent_idx];
            } else {
                return false;
            }
        } else {
            xforms[i] = joint_local_xforms[i];
            if let Some(root) = root_xform {
                xforms[i] *= *root;
            }
        }
    }
    true
}

/// Compute joint transforms in joint-local space.
///
/// Transforms are computed from `xforms`, holding concatenated joint transforms,
/// and `inverse_xforms`, providing the inverse of each of those transforms.
/// The resulting local space transforms are written to `joint_local_xforms`,
/// which must be the same size as `topology`.
///
/// If `root_inverse_xform` is provided, it is applied to root joints.
///
/// Matches C++ `UsdSkelComputeJointLocalTransforms()`.
pub fn compute_joint_local_transforms(
    topology: &Topology,
    xforms: &[Matrix4d],
    inverse_xforms: &[Matrix4d],
    joint_local_xforms: &mut [Matrix4d],
    root_inverse_xform: Option<&Matrix4d>,
) -> bool {
    let num_joints = topology.num_joints();

    if xforms.len() != num_joints {
        eprintln!(
            "Size of xforms [{}] != number of joints [{}]",
            xforms.len(),
            num_joints
        );
        return false;
    }
    if inverse_xforms.len() != num_joints {
        eprintln!(
            "Size of inverse_xforms [{}] != number of joints [{}]",
            inverse_xforms.len(),
            num_joints
        );
        return false;
    }
    if joint_local_xforms.len() != num_joints {
        eprintln!(
            "Size of joint_local_xforms [{}] != number of joints [{}]",
            joint_local_xforms.len(),
            num_joints
        );
        return false;
    }

    // Skel-space transforms are computed as:
    //     skelXform = jointLocalXform * parentSkelXform
    // So we want:
    //     jointLocalXform = skelXform * inv(parentSkelXform)

    for i in 0..num_joints {
        let parent = topology.get_parent(i);
        if parent >= 0 {
            let parent_idx = parent as usize;
            if parent_idx < i {
                joint_local_xforms[i] = xforms[i] * inverse_xforms[parent_idx];
            } else {
                if parent_idx == i {
                    eprintln!("Joint {} has itself as its parent.", i);
                } else {
                    eprintln!(
                        "Joint {} has mis-ordered parent {}. Joints are expected \
                        to be ordered with parent joints always coming before children.",
                        i, parent
                    );
                }
                return false;
            }
        } else {
            // Root joint
            joint_local_xforms[i] = xforms[i];
            if let Some(root_inv) = root_inverse_xform {
                joint_local_xforms[i] *= *root_inv;
            }
        }
    }
    true
}

/// Single-precision overload of compute_joint_local_transforms.
///
/// Matches C++ `UsdSkelComputeJointLocalTransforms()` with GfMatrix4f.
pub fn compute_joint_local_transforms_f(
    topology: &Topology,
    xforms: &[Matrix4f],
    inverse_xforms: &[Matrix4f],
    joint_local_xforms: &mut [Matrix4f],
    root_inverse_xform: Option<&Matrix4f>,
) -> bool {
    let num_joints = topology.num_joints();
    if xforms.len() != num_joints
        || inverse_xforms.len() != num_joints
        || joint_local_xforms.len() != num_joints
    {
        return false;
    }

    for i in 0..num_joints {
        let parent = topology.get_parent(i);
        if parent >= 0 {
            let parent_idx = parent as usize;
            if parent_idx < i {
                joint_local_xforms[i] = xforms[i] * inverse_xforms[parent_idx];
            } else {
                return false;
            }
        } else {
            joint_local_xforms[i] = xforms[i];
            if let Some(root_inv) = root_inverse_xform {
                joint_local_xforms[i] *= *root_inv;
            }
        }
    }
    true
}

/// Single-precision convenience overload that computes inverse transforms internally.
pub fn compute_joint_local_transforms_auto_inverse_f(
    topology: &Topology,
    xforms: &[Matrix4f],
    joint_local_xforms: &mut [Matrix4f],
    root_inverse_xform: Option<&Matrix4f>,
) -> bool {
    let inverse_xforms: Vec<Matrix4f> = xforms
        .iter()
        .map(|m| {
            // Convert to f64, invert, convert back
            let md = Matrix4d::from(*m);
            mat4d_to_f(&md.inverse().unwrap_or_else(Matrix4d::identity))
        })
        .collect();
    compute_joint_local_transforms_f(
        topology,
        xforms,
        &inverse_xforms,
        joint_local_xforms,
        root_inverse_xform,
    )
}

/// Convenience overload that computes inverse transforms internally.
pub fn compute_joint_local_transforms_auto_inverse(
    topology: &Topology,
    xforms: &[Matrix4d],
    joint_local_xforms: &mut [Matrix4d],
    root_inverse_xform: Option<&Matrix4d>,
) -> bool {
    let inverse_xforms: Vec<Matrix4d> = xforms
        .iter()
        .map(|m| m.inverse().unwrap_or_else(Matrix4d::identity))
        .collect();
    compute_joint_local_transforms(
        topology,
        xforms,
        &inverse_xforms,
        joint_local_xforms,
        root_inverse_xform,
    )
}

/// Compute an extent from a set of skel-space joint transforms.
///
/// The `root_xform` may also be set to provide an additional root
/// transformation on top of all joints, which is useful for computing
/// extent relative to a different space.
///
/// Matches C++ `UsdSkelComputeJointsExtent()`.
pub fn compute_joints_extent(
    xforms: &[Matrix4d],
    extent: &mut Range3f,
    pad: f32,
    root_xform: Option<&Matrix4d>,
) -> bool {
    *extent = Range3f::default();

    for xform in xforms {
        let trans = xform.extract_translation();
        let pivot = Vec3f::new(trans.x as f32, trans.y as f32, trans.z as f32);

        let transformed = if let Some(root) = root_xform {
            let t =
                root.transform_point(&Vec3d::new(pivot.x as f64, pivot.y as f64, pivot.z as f64));
            Vec3f::new(t.x as f32, t.y as f32, t.z as f32)
        } else {
            pivot
        };

        extent.union_with_point(&transformed);
    }

    // Apply padding
    let min = *extent.min();
    let max = *extent.max();
    extent.set_min(Vec3f::new(min.x - pad, min.y - pad, min.z - pad));
    extent.set_max(Vec3f::new(max.x + pad, max.y + pad, max.z + pad));

    true
}

/// Compute an extent from a set of skel-space joint transforms (Matrix4f variant).
///
/// Matches C++ `UsdSkelComputeJointsExtent()` template instantiation for Matrix4f.
pub fn compute_joints_extent_f(
    xforms: &[Matrix4f],
    extent: &mut Range3f,
    pad: f32,
    root_xform: Option<&Matrix4f>,
) -> bool {
    *extent = Range3f::default();

    for xform in xforms {
        let trans = xform.extract_translation();
        let pivot = Vec3f::new(trans.x as f32, trans.y as f32, trans.z as f32);

        let transformed = if let Some(root) = root_xform {
            let p = Vec3d::new(pivot.x as f64, pivot.y as f64, pivot.z as f64);
            // Convert to f64 for transform, then back
            let root_d = Matrix4d::from(*root);
            let t = root_d.transform_point(&p);
            Vec3f::new(t.x as f32, t.y as f32, t.z as f32)
        } else {
            pivot
        };

        extent.union_with_point(&transformed);
    }

    // Apply padding
    let min_v = *extent.min();
    let max_v = *extent.max();
    extent.set_min(Vec3f::new(min_v.x - pad, min_v.y - pad, min_v.z - pad));
    extent.set_max(Vec3f::new(max_v.x + pad, max_v.y + pad, max_v.z + pad));

    true
}

// ============================================================================
// Transform Composition Utilities
// ============================================================================

/// Single-precision overload of decompose_transforms.
///
/// Matches C++ `UsdSkelDecomposeTransforms()` with GfMatrix4f.
pub fn decompose_transforms_f(
    xforms: &[Matrix4f],
    translations: &mut [Vec3f],
    rotations: &mut [Quatf],
    scales: &mut [Vec3h],
) -> bool {
    if translations.len() != xforms.len()
        || rotations.len() != xforms.len()
        || scales.len() != xforms.len()
    {
        return false;
    }
    for i in 0..xforms.len() {
        let xd = Matrix4d::from(xforms[i]);
        if !decompose_transform(&xd, &mut translations[i], &mut rotations[i], &mut scales[i]) {
            return false;
        }
    }
    true
}

/// Single-precision overload of make_transforms.
///
/// Matches C++ `UsdSkelMakeTransforms()` with GfMatrix4f.
pub fn make_transforms_f(
    translations: &[Vec3f],
    rotations: &[Quatf],
    scales: &[Vec3h],
    xforms: &mut [Matrix4f],
) -> bool {
    if translations.len() != xforms.len()
        || rotations.len() != xforms.len()
        || scales.len() != xforms.len()
    {
        return false;
    }
    for i in 0..xforms.len() {
        let mut xd = Matrix4d::identity();
        make_transform(&translations[i], &rotations[i], &scales[i], &mut xd);
        xforms[i] = mat4d_to_f(&xd);
    }
    true
}

/// Decompose a transform into translate/rotate/scale components.
///
/// The transform order for decomposition is scale, rotate, translate.
///
/// Returns true if decomposition succeeded.
///
/// Matches C++ `UsdSkelDecomposeTransform()`.
pub fn decompose_transform(
    xform: &Matrix4d,
    translate: &mut Vec3f,
    rotate: &mut Quatf,
    scale: &mut Vec3h,
) -> bool {
    // Use Matrix4::factor() to extract components
    if let Some((_scale_orient, factored_scale, mut factored_rot, factored_translate, _p)) =
        xform.factor()
    {
        // Orthonormalize the rotation matrix
        if factored_rot.orthonormalize() {
            *scale = Vec3h::new(
                Half::from_f32(factored_scale.x as f32),
                Half::from_f32(factored_scale.y as f32),
                Half::from_f32(factored_scale.z as f32),
            );
            *translate = Vec3f::new(
                factored_translate.x as f32,
                factored_translate.y as f32,
                factored_translate.z as f32,
            );
            let quat = factored_rot.extract_rotation_quat();
            *rotate = Quatf::from_components(
                quat.real() as f32,
                quat.imaginary().x as f32,
                quat.imaginary().y as f32,
                quat.imaginary().z as f32,
            );
            return true;
        }
    }
    false
}

/// Decompose an array of transforms into translate/rotate/scale components.
///
/// All slices must be the same size.
///
/// Matches C++ `UsdSkelDecomposeTransforms()`.
pub fn decompose_transforms(
    xforms: &[Matrix4d],
    translations: &mut [Vec3f],
    rotations: &mut [Quatf],
    scales: &mut [Vec3h],
) -> bool {
    if translations.len() != xforms.len() {
        eprintln!(
            "Size of translations [{}] != size of xforms [{}]",
            translations.len(),
            xforms.len()
        );
        return false;
    }
    if rotations.len() != xforms.len() {
        eprintln!(
            "Size of rotations [{}] != size of xforms [{}]",
            rotations.len(),
            xforms.len()
        );
        return false;
    }
    if scales.len() != xforms.len() {
        eprintln!(
            "Size of scales [{}] != size of xforms [{}]",
            scales.len(),
            xforms.len()
        );
        return false;
    }

    for i in 0..xforms.len() {
        if !decompose_transform(
            &xforms[i],
            &mut translations[i],
            &mut rotations[i],
            &mut scales[i],
        ) {
            eprintln!(
                "Failed decomposing transform {}. The source transform may be singular.",
                i
            );
            return false;
        }
    }

    true
}

/// Convert a quaternion to a 3x3 rotation matrix.
fn quat_to_matrix3(q: &Quatf) -> Matrix3d {
    let w = q.real() as f64;
    let x = q.imaginary().x as f64;
    let y = q.imaginary().y as f64;
    let z = q.imaginary().z as f64;

    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;

    Matrix3d::new(
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy - wz),
        2.0 * (xz + wy),
        2.0 * (xy + wz),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz - wx),
        2.0 * (xz - wy),
        2.0 * (yz + wx),
        1.0 - 2.0 * (xx + yy),
    )
}

/// Create a transform from translate/rotate/scale components.
///
/// This performs the inverse of decompose_transform.
/// Order is scale * rotate * translate.
///
/// Matches C++ `UsdSkelMakeTransform()`.
pub fn make_transform(translate: &Vec3f, rotate: &Quatf, scale: &Vec3h, xform: &mut Matrix4d) {
    // Convert quaternion to rotation matrix
    let rot_mat = quat_to_matrix3(rotate);

    // Order is scale*rotate*translate
    // Build the matrix: each row is the rotated basis scaled
    let sx = scale.x.to_f64();
    let sy = scale.y.to_f64();
    let sz = scale.z.to_f64();
    *xform = Matrix4d::new(
        rot_mat[0][0] * sx,
        rot_mat[0][1] * sx,
        rot_mat[0][2] * sx,
        0.0,
        rot_mat[1][0] * sy,
        rot_mat[1][1] * sy,
        rot_mat[1][2] * sy,
        0.0,
        rot_mat[2][0] * sz,
        rot_mat[2][1] * sz,
        rot_mat[2][2] * sz,
        0.0,
        translate.x as f64,
        translate.y as f64,
        translate.z as f64,
        1.0,
    );
}

/// Create transforms from arrays of components.
///
/// All slices must be the same size.
///
/// Matches C++ `UsdSkelMakeTransforms()`.
pub fn make_transforms(
    translations: &[Vec3f],
    rotations: &[Quatf],
    scales: &[Vec3h],
    xforms: &mut [Matrix4d],
) -> bool {
    if translations.len() != xforms.len() {
        eprintln!(
            "Size of translations [{}] != size of xforms [{}]",
            translations.len(),
            xforms.len()
        );
        return false;
    }
    if rotations.len() != xforms.len() {
        eprintln!(
            "Size of rotations [{}] != size of xforms [{}]",
            rotations.len(),
            xforms.len()
        );
        return false;
    }
    if scales.len() != xforms.len() {
        eprintln!(
            "Size of scales [{}] != size of xforms [{}]",
            scales.len(),
            xforms.len()
        );
        return false;
    }

    for i in 0..xforms.len() {
        make_transform(&translations[i], &rotations[i], &scales[i], &mut xforms[i]);
    }

    true
}

// ============================================================================
// Joint Influence Utilities
// ============================================================================

/// Normalize weight values across each consecutive run of
/// `num_influences_per_component` elements.
///
/// If the total weight for a run of elements is smaller than `eps`,
/// the elements' weights are set to zero.
///
/// Matches C++ `UsdSkelNormalizeWeights()`.
pub fn normalize_weights(
    weights: &mut [f32],
    num_influences_per_component: usize,
    eps: f32,
) -> bool {
    if num_influences_per_component == 0 {
        eprintln!("num_influences_per_component must be > 0");
        return false;
    }
    if weights.len() % num_influences_per_component != 0 {
        eprintln!(
            "Size of weights [{}] is not divisible by num_influences_per_component [{}]",
            weights.len(),
            num_influences_per_component
        );
        return false;
    }

    let num_components = weights.len() / num_influences_per_component;

    for i in 0..num_components {
        let start = i * num_influences_per_component;
        let end = start + num_influences_per_component;
        let weight_set = &mut weights[start..end];

        let sum: f32 = weight_set.iter().sum();

        if sum.abs() > eps {
            for w in weight_set.iter_mut() {
                *w /= sum;
            }
        } else {
            for w in weight_set.iter_mut() {
                *w = 0.0;
            }
        }
    }
    true
}

/// Sort joint influences such that highest weight values come first.
///
/// Matches C++ `UsdSkelSortInfluences()`.
pub fn sort_influences(
    indices: &mut [i32],
    weights: &mut [f32],
    num_influences_per_component: usize,
) -> bool {
    if indices.len() != weights.len() {
        eprintln!(
            "Size of indices [{}] != size of weights [{}]",
            indices.len(),
            weights.len()
        );
        return false;
    }
    if num_influences_per_component == 0 {
        eprintln!("num_influences_per_component must be > 0");
        return false;
    }
    if indices.len() % num_influences_per_component != 0 {
        eprintln!(
            "Size of indices [{}] is not divisible by num_influences_per_component [{}]",
            indices.len(),
            num_influences_per_component
        );
        return false;
    }
    if num_influences_per_component < 2 {
        // Nothing to sort
        return true;
    }

    let num_components = indices.len() / num_influences_per_component;

    for i in 0..num_components {
        let start = i * num_influences_per_component;
        let end = start + num_influences_per_component;

        // Collect (weight, index) pairs
        let mut influences: Vec<(f32, i32)> = weights[start..end]
            .iter()
            .zip(indices[start..end].iter())
            .map(|(&w, &idx)| (w, idx))
            .collect();

        // Sort by weight descending
        influences.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Write back
        for (j, (w, idx)) in influences.into_iter().enumerate() {
            weights[start + j] = w;
            indices[start + j] = idx;
        }
    }
    true
}

/// Convert an array of constant influences to varying influences.
///
/// The `size` should match the size required for 'vertex' interpolation
/// on the geometry primitive (typically the number of points).
///
/// Matches C++ `UsdSkelExpandConstantInfluencesToVarying()`.
pub fn expand_constant_influences_to_varying<T: Clone>(array: &mut Vec<T>, size: usize) -> bool {
    if size == 0 {
        array.clear();
        return true;
    }

    let num_influences_per_component = array.len();
    if num_influences_per_component == 0 {
        return true;
    }

    let original: Vec<T> = array.clone();
    array.clear();
    array.reserve(num_influences_per_component * size);

    for _ in 0..size {
        array.extend(original.iter().cloned());
    }
    true
}

/// Combine arrays of joint indices and weights into interleaved
/// (index, weight) vectors.
///
/// Matches C++ `UsdSkelInterleaveInfluences()`.
pub fn interleave_influences(indices: &[i32], weights: &[f32], interleaved: &mut [Vec2f]) -> bool {
    if indices.len() != weights.len() {
        eprintln!(
            "Size of indices [{}] != size of weights [{}]",
            indices.len(),
            weights.len()
        );
        return false;
    }
    if interleaved.len() != indices.len() {
        eprintln!(
            "Size of interleaved [{}] != size of indices [{}]",
            interleaved.len(),
            indices.len()
        );
        return false;
    }

    for i in 0..indices.len() {
        interleaved[i] = Vec2f::new(indices[i] as f32, weights[i]);
    }
    true
}

/// Resize the number of influences per component.
///
/// If the size decreases, influences are additionally re-normalized.
///
/// Matches C++ `UsdSkelResizeInfluences()`.
pub fn resize_influences(
    indices: &mut Vec<i32>,
    weights: &mut Vec<f32>,
    src_num_influences: usize,
    new_num_influences: usize,
) -> bool {
    if src_num_influences == 0 || new_num_influences == 0 {
        return false;
    }
    if indices.len() != weights.len() {
        return false;
    }
    if indices.len() % src_num_influences != 0 {
        return false;
    }

    let num_components = indices.len() / src_num_influences;

    if new_num_influences < src_num_influences {
        // Shrinking - need to truncate and renormalize
        let mut new_indices = Vec::with_capacity(num_components * new_num_influences);
        let mut new_weights = Vec::with_capacity(num_components * new_num_influences);

        for i in 0..num_components {
            let start = i * src_num_influences;
            for j in 0..new_num_influences {
                new_indices.push(indices[start + j]);
                new_weights.push(weights[start + j]);
            }
        }

        *indices = new_indices;
        *weights = new_weights;

        // Re-normalize
        normalize_weights(weights, new_num_influences, f32::EPSILON)
    } else if new_num_influences > src_num_influences {
        // Growing - pad with zeros
        let mut new_indices = Vec::with_capacity(num_components * new_num_influences);
        let mut new_weights = Vec::with_capacity(num_components * new_num_influences);

        for i in 0..num_components {
            let start = i * src_num_influences;
            for j in 0..src_num_influences {
                new_indices.push(indices[start + j]);
                new_weights.push(weights[start + j]);
            }
            // Pad with zeros
            for _ in src_num_influences..new_num_influences {
                new_indices.push(0);
                new_weights.push(0.0);
            }
        }

        *indices = new_indices;
        *weights = new_weights;
        true
    } else {
        // Same size, nothing to do
        true
    }
}

// ============================================================================
// Skinning Implementations - Linear Blend Skinning (LBS)
// ============================================================================

/// Skin points using linear blend skinning (LBS).
///
/// The `joint_xforms` are skinning transforms given in skeleton space,
/// while the `geom_bind_transform` provides the transform that places
/// the initial points into that same skeleton space.
///
/// Matches C++ `UsdSkelSkinPointsLBS()`.
pub fn skin_points_lbs(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        eprintln!(
            "Size of joint_indices [{}] != size of joint_weights [{}]",
            joint_indices.len(),
            joint_weights.len()
        );
        return false;
    }

    if joint_indices.len() != points.len() * num_influences_per_point {
        eprintln!(
            "Size of joint_indices [{}] != (points.len() [{}] * num_influences_per_point [{}]).",
            joint_indices.len(),
            points.len(),
            num_influences_per_point
        );
        return false;
    }

    for pi in 0..points.len() {
        // Transform point to skeleton space
        let initial_p = geom_bind_transform.transform_point(&Vec3d::new(
            points[pi].x as f64,
            points[pi].y as f64,
            points[pi].z as f64,
        ));

        let mut p = Vec3d::new(0.0, 0.0, 0.0);

        for wi in 0..num_influences_per_point {
            let influence_idx = pi * num_influences_per_point + wi;
            let joint_idx = joint_indices[influence_idx];

            if joint_idx >= 0 && (joint_idx as usize) < joint_xforms.len() {
                let w = joint_weights[influence_idx];
                if w != 0.0 {
                    let transformed = joint_xforms[joint_idx as usize].transform_point(&initial_p);
                    p += transformed * w as f64;
                }
            } else {
                eprintln!(
                    "Out of range joint index {} at index {} (num joints = {}).",
                    joint_idx,
                    influence_idx,
                    joint_xforms.len()
                );
                return false;
            }
        }

        points[pi] = Vec3f::new(p.x as f32, p.y as f32, p.z as f32);
    }

    true
}

/// Single-precision overload of skin_points_lbs.
///
/// Matches C++ `UsdSkelSkinPointsLBS()` with GfMatrix4f.
pub fn skin_points_lbs_f(
    geom_bind_transform: &Matrix4f,
    joint_xforms: &[Matrix4f],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    // Convert to f64 and delegate
    let geom_bind_d = Matrix4d::from(*geom_bind_transform);
    let joint_xforms_d: Vec<Matrix4d> = joint_xforms.iter().map(|m| Matrix4d::from(*m)).collect();
    skin_points_lbs(
        &geom_bind_d,
        &joint_xforms_d,
        joint_indices,
        joint_weights,
        num_influences_per_point,
        points,
    )
}

/// Single-precision overload of skin_points_dqs.
///
/// Matches C++ `UsdSkelSkinPointsDQS()` with GfMatrix4f.
pub fn skin_points_dqs_f(
    geom_bind_transform: &Matrix4f,
    joint_xforms: &[Matrix4f],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    let geom_bind_d = Matrix4d::from(*geom_bind_transform);
    let joint_xforms_d: Vec<Matrix4d> = joint_xforms.iter().map(|m| Matrix4d::from(*m)).collect();
    skin_points_dqs(
        &geom_bind_d,
        &joint_xforms_d,
        joint_indices,
        joint_weights,
        num_influences_per_point,
        points,
    )
}

/// Single-precision dispatch for skinning points.
///
/// Matches C++ `UsdSkelSkinPoints()` with GfMatrix4f.
pub fn skin_points_f(
    skinning_method: &Token,
    geom_bind_transform: &Matrix4f,
    joint_xforms: &[Matrix4f],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_points_lbs_f(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            points,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_points_dqs_f(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            points,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Single-precision dispatch for skinning a transform.
///
/// Matches C++ `UsdSkelSkinTransform()` with GfMatrix4f.
pub fn skin_transform_f(
    skinning_method: &Token,
    geom_bind_transform: &Matrix4f,
    joint_xforms: &[Matrix4f],
    joint_indices: &[i32],
    joint_weights: &[f32],
    xform: &mut Matrix4f,
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_transform_lbs_f(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            xform,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        // Convert f32 -> f64, apply DQS, convert back
        let geom_bind_d = Matrix4d::from(*geom_bind_transform);
        let joint_xforms_d: Vec<Matrix4d> =
            joint_xforms.iter().map(|m| Matrix4d::from(*m)).collect();
        let mut xform_d = Matrix4d::identity();
        let result = skin_transform_dqs(
            &geom_bind_d,
            &joint_xforms_d,
            joint_indices,
            joint_weights,
            &mut xform_d,
        );
        if result {
            *xform = mat4d_to_f(&xform_d);
        }
        result
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Skin points using the specified skinning method.
///
/// Supports both LBS ("classicLinear") and DQS ("dualQuaternion").
///
/// Matches C++ `UsdSkelSkinPoints()`.
pub fn skin_points(
    skinning_method: &Token,
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_points_lbs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            points,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_points_dqs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            points,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Skin points using dual quaternion skinning (DQS).
///
/// DQS provides better volume preservation than LBS, especially around
/// joints with large rotations.
pub fn skin_points_dqs(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }
    if joint_indices.len() != points.len() * num_influences_per_point {
        return false;
    }

    // Convert joint transforms to dual quaternions
    let mut joint_dual_quats: Vec<DualQuatd> = Vec::with_capacity(joint_xforms.len());
    let mut joint_scales: Vec<Matrix3d> = Vec::with_capacity(joint_xforms.len());
    let mut has_joint_scale = false;

    for xform in joint_xforms {
        if let Some((_, _scale, mut rot, translation, _p)) = xform.factor() {
            rot.orthonormalize();
            let rotation_q = rot.extract_rotation_quat();
            let dq = DualQuatd::from_rotation_translation(&rotation_q, &translation);
            joint_dual_quats.push(dq);

            // Calculate scale matrix
            let non_scale_xform = rot * *Matrix4d::identity().set_translate(&translation);
            if let Some(inv) = non_scale_xform.inverse() {
                let scale_mat = (*xform * inv).extract_rotation_matrix();
                // Check if scale is not identity
                if !is_close_matrix3(&scale_mat, &Matrix3d::identity(), 1e-6) {
                    has_joint_scale = true;
                }
                joint_scales.push(scale_mat);
            } else {
                joint_scales.push(Matrix3d::identity());
            }
        } else {
            joint_dual_quats.push(DualQuatd::zero());
            joint_scales.push(Matrix3d::identity());
        }
    }

    for pi in 0..points.len() {
        let initial_p = geom_bind_transform.transform_point(&Vec3d::new(
            points[pi].x as f64,
            points[pi].y as f64,
            points[pi].z as f64,
        ));

        let mut scaled_p = Vec3d::new(0.0, 0.0, 0.0);

        // Find pivot joint (with max weight)
        let pivot_idx = get_pivot_joint_index(
            pi,
            joint_dual_quats.len(),
            joint_indices,
            joint_weights,
            num_influences_per_point,
        );
        let pivot_quat = if pivot_idx >= 0 {
            *joint_dual_quats[pivot_idx as usize].real()
        } else {
            Quatd::zero()
        };

        let mut weighted_sum_dq = DualQuatd::zero();

        for wi in 0..num_influences_per_point {
            let influence_idx = pi * num_influences_per_point + wi;
            let joint_idx = joint_indices[influence_idx];

            if joint_idx >= 0 && (joint_idx as usize) < joint_dual_quats.len() {
                let mut w = joint_weights[influence_idx];
                if w != 0.0 {
                    // Apply scale using LBS if any joint has scale.
                    // Row-vector convention: initialP * scaleMatrix (v * M).
                    if has_joint_scale {
                        let s = initial_p * joint_scales[joint_idx as usize];
                        scaled_p += s * w as f64;
                    }

                    // Apply rotation & translation using DQS
                    let joint_dq = &joint_dual_quats[joint_idx as usize];
                    // Flip if on opposite hemisphere
                    if quat_dot(joint_dq.real(), &pivot_quat) < 0.0 {
                        w = -w;
                    }
                    weighted_sum_dq += *joint_dq * w as f64;
                }
            } else {
                eprintln!(
                    "Out of range joint index {} at index {} (num joints = {}).",
                    joint_idx,
                    influence_idx,
                    joint_dual_quats.len()
                );
                return false;
            }
        }

        if !has_joint_scale {
            scaled_p = initial_p;
        }

        weighted_sum_dq = weighted_sum_dq.normalized();
        let result = weighted_sum_dq.transform(&scaled_p);
        points[pi] = Vec3f::new(result.x as f32, result.y as f32, result.z as f32);
    }

    true
}

/// Helper to find pivot joint index (joint with max weight).
fn get_pivot_joint_index(
    point_idx: usize,
    joint_array_size: usize,
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
) -> i32 {
    let mut pivot_idx = -1i32;
    let mut max_w = -1.0f32;

    for wi in 0..num_influences_per_point {
        let influence_idx = point_idx * num_influences_per_point + wi;
        let joint_idx = joint_indices[influence_idx];

        if joint_idx >= 0 && (joint_idx as usize) < joint_array_size {
            let w = joint_weights[influence_idx];
            if pivot_idx < 0 || max_w < w {
                max_w = w;
                pivot_idx = joint_idx;
            }
        }
    }

    pivot_idx
}

/// Helper to check if two Matrix3d are close.
fn is_close_matrix3(a: &Matrix3d, b: &Matrix3d, eps: f64) -> bool {
    for i in 0..3 {
        for j in 0..3 {
            if (a[i][j] - b[i][j]).abs() > eps {
                return false;
            }
        }
    }
    true
}

/// Skin points using LBS with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinPointsLBS()` with interleaved influences.
pub fn skin_points_lbs_interleaved(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if influences.len() != points.len() * num_influences_per_point {
        eprintln!(
            "Size of influences [{}] != (points.len() [{}] * num_influences_per_point [{}]).",
            influences.len(),
            points.len(),
            num_influences_per_point
        );
        return false;
    }

    // Unpack interleaved influences
    let mut joint_indices = Vec::with_capacity(influences.len());
    let mut joint_weights = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }

    skin_points_lbs(
        geom_bind_transform,
        joint_xforms,
        &joint_indices,
        &joint_weights,
        num_influences_per_point,
        points,
    )
}

/// Skin points using DQS with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
pub fn skin_points_dqs_interleaved(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if influences.len() != points.len() * num_influences_per_point {
        return false;
    }

    let mut joint_indices = Vec::with_capacity(influences.len());
    let mut joint_weights = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }

    skin_points_dqs(
        geom_bind_transform,
        joint_xforms,
        &joint_indices,
        &joint_weights,
        num_influences_per_point,
        points,
    )
}

/// Skin points using the specified skinning method with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinPoints()` with interleaved influences.
pub fn skin_points_interleaved(
    skinning_method: &Token,
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    points: &mut [Vec3f],
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_points_lbs_interleaved(
            geom_bind_transform,
            joint_xforms,
            influences,
            num_influences_per_point,
            points,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_points_dqs_interleaved(
            geom_bind_transform,
            joint_xforms,
            influences,
            num_influences_per_point,
            points,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

// ============================================================================
// Normal Skinning
// ============================================================================

/// Skin normals using linear blend skinning (LBS).
///
/// The `joint_xforms` are the inverse transposes of the 3x3 component
/// of the skinning transforms. The `geom_bind_transform` is the inverse
/// transpose of the matrix that transforms points from bind pose.
///
/// Matches C++ `UsdSkelSkinNormalsLBS()`.
pub fn skin_normals_lbs(
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    normals: &mut [Vec3f],
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }
    if joint_indices.len() != normals.len() * num_influences_per_point {
        return false;
    }

    for ni in 0..normals.len() {
        // Transform normal by geomBindTransform.
        // C++ uses row-vector convention: initialN = normals[ni] * geomBindTransform
        // so we must use Vec3 * Matrix3 (row-vector), not Matrix3 * Vec3 (column-vector).
        let v = Vec3d::new(
            normals[ni].x as f64,
            normals[ni].y as f64,
            normals[ni].z as f64,
        );
        // row-vector: v * M  matches C++ `normals[ni]*geomBindTransform`
        let initial_n = v * *geom_bind_transform;

        let mut n = Vec3d::new(0.0, 0.0, 0.0);

        for wi in 0..num_influences_per_point {
            let influence_idx = ni * num_influences_per_point + wi;
            let joint_idx = joint_indices[influence_idx];

            if joint_idx >= 0 && (joint_idx as usize) < joint_xforms.len() {
                let w = joint_weights[influence_idx];
                if w != 0.0 {
                    // row-vector: initialN * jointXform  matches C++ `initialN*jointXforms[jointIdx]`
                    let transformed = initial_n * joint_xforms[joint_idx as usize];
                    n += transformed * w as f64;
                }
            } else {
                eprintln!(
                    "Out of range joint index {} at index {} (num joints = {}).",
                    joint_idx,
                    influence_idx,
                    joint_xforms.len()
                );
                return false;
            }
        }

        // Normalize the result
        let normalized = n.normalized();
        normals[ni] = Vec3f::new(
            normalized.x as f32,
            normalized.y as f32,
            normalized.z as f32,
        );
    }

    true
}

/// Skin normals using dual quaternion skinning (DQS).
///
/// Matches C++ `_SkinNormalsDQS()`. Converts joint rotation matrices to
/// quaternions, then does hemisphere-consistent quaternion blending per normal.
/// Scale is handled separately with LBS if any joint has non-identity scale.
pub fn skin_normals_dqs(
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    normals: &mut [Vec3f],
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }

    let n_joints = joint_xforms.len();

    // Convert joint 3x3 rotation matrices to quaternions + scale matrices.
    // Matches C++ _ConvertToQuaternions().
    let mut joint_quats: Vec<Quatd> = Vec::with_capacity(n_joints);
    let mut joint_scales: Vec<Matrix3d> = Vec::with_capacity(n_joints);
    let mut has_joint_scale = false;
    for xform in joint_xforms {
        let rot_mat = xform.orthonormalized();
        let q_leg = rot_mat.extract_rotation_quaternion();
        let quat = Quatd::new(
            q_leg.real(),
            Vec3d::new(
                q_leg.imaginary().x,
                q_leg.imaginary().y,
                q_leg.imaginary().z,
            ),
        );
        joint_quats.push(quat);
        // Scale = xform * rot_inv
        if let Some(rot_inv) = rot_mat.inverse() {
            let scale_mat = *xform * rot_inv;
            if !is_close_matrix3(&scale_mat, &Matrix3d::identity(), 1e-6) {
                has_joint_scale = true;
            }
            joint_scales.push(scale_mat);
        } else {
            joint_scales.push(Matrix3d::identity());
        }
    }

    for ni in 0..normals.len() {
        let pi = ni; // identity point index (non-facevarying path)

        // Transform normal by geom bind transform.
        // Row-vector convention: initialN = normals[ni] * geomBindTransform (v * M).
        let v = Vec3d::new(
            normals[ni].x as f64,
            normals[ni].y as f64,
            normals[ni].z as f64,
        );
        // row-vector: v * M  matches C++ `normals[ni]*geomBindTransform`
        let initial_n = v * *geom_bind_transform;
        let initial_nf = Vec3f::new(initial_n.x as f32, initial_n.y as f32, initial_n.z as f32);

        // Find pivot quaternion (joint with max weight)
        let pivot_quat = {
            let mut best_w = f32::NEG_INFINITY;
            let mut best_q = Quatd::new(1.0, Vec3d::new(0.0, 0.0, 0.0));
            for wi in 0..num_influences_per_point {
                let influence_idx = pi * num_influences_per_point + wi;
                if influence_idx >= joint_indices.len() {
                    break;
                }
                let joint_idx = joint_indices[influence_idx];
                let w = joint_weights[influence_idx];
                if w > best_w && joint_idx >= 0 && (joint_idx as usize) < n_joints {
                    best_w = w;
                    best_q = joint_quats[joint_idx as usize];
                }
            }
            best_q
        };

        let mut scaled_n = Vec3f::new(0.0, 0.0, 0.0);
        let mut weighted_sum_quat = Quatd::new(0.0, Vec3d::new(0.0, 0.0, 0.0));

        for wi in 0..num_influences_per_point {
            let influence_idx = pi * num_influences_per_point + wi;
            if influence_idx >= joint_indices.len() {
                break;
            }
            let joint_idx = joint_indices[influence_idx];
            if joint_idx >= 0 && (joint_idx as usize) < n_joints {
                let mut w = joint_weights[influence_idx];
                if w != 0.0 {
                    // Apply scale with LBS if needed.
                    // C++ uses row-vector: initialN * jointScales[i], so v * M.
                    if has_joint_scale {
                        let scale_n = initial_n * joint_scales[joint_idx as usize];
                        scaled_n.x += scale_n.x as f32 * w;
                        scaled_n.y += scale_n.y as f32 * w;
                        scaled_n.z += scale_n.z as f32 * w;
                    }
                    // Apply rotation with DQS - hemisphere flip
                    let joint_q = joint_quats[joint_idx as usize];
                    if quat_dot(&joint_q, &pivot_quat) < 0.0 {
                        w = -w;
                    }
                    weighted_sum_quat = Quatd::new(
                        weighted_sum_quat.real() + joint_q.real() * w as f64,
                        Vec3d::new(
                            weighted_sum_quat.imaginary().x + joint_q.imaginary().x * w as f64,
                            weighted_sum_quat.imaginary().y + joint_q.imaginary().y * w as f64,
                            weighted_sum_quat.imaginary().z + joint_q.imaginary().z * w as f64,
                        ),
                    );
                }
            } else {
                eprintln!(
                    "Out of range joint index {} at index {} (num joints = {}).",
                    joint_idx, influence_idx, n_joints
                );
                return false;
            }
        }

        if !has_joint_scale {
            scaled_n = initial_nf;
        }

        // Normalize and apply weighted quaternion rotation
        weighted_sum_quat = weighted_sum_quat.normalized();
        // Transform scaled_n by the quaternion: q * v * q^-1
        let sv = Vec3d::new(scaled_n.x as f64, scaled_n.y as f64, scaled_n.z as f64);
        let rotated = weighted_sum_quat.transform(&sv);
        let result = rotated.normalized();
        normals[ni] = Vec3f::new(result.x as f32, result.y as f32, result.z as f32);
    }

    true
}

/// Skin normals using the specified skinning method.
///
/// Matches C++ `UsdSkelSkinNormals()`.
pub fn skin_normals(
    skinning_method: &Token,
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    normals: &mut [Vec3f],
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_normals_lbs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            normals,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_normals_dqs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            normals,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Skin face-varying normals using linear blend skinning.
///
/// Uses `face_vertex_indices` to map normal indices to point indices.
///
/// Matches C++ `UsdSkelSkinFaceVaryingNormalsLBS()`.
pub fn skin_face_varying_normals_lbs(
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    face_vertex_indices: &[i32],
    normals: &mut [Vec3f],
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }
    if face_vertex_indices.len() != normals.len() {
        return false;
    }

    let num_points = joint_indices.len() / num_influences_per_point;

    for ni in 0..normals.len() {
        let point_idx = face_vertex_indices[ni];
        if point_idx < 0 || point_idx as usize >= num_points {
            eprintln!(
                "faceVertexIndices is out of range [{}] at index [{}]",
                point_idx, ni
            );
            return false;
        }
        let pi = point_idx as usize;

        // row-vector convention: v * M, matching C++ `normals[ni]*geomBindTransform`
        let v = Vec3d::new(
            normals[ni].x as f64,
            normals[ni].y as f64,
            normals[ni].z as f64,
        );
        let initial_n = v * *geom_bind_transform;

        let mut n = Vec3d::new(0.0, 0.0, 0.0);

        for wi in 0..num_influences_per_point {
            let influence_idx = pi * num_influences_per_point + wi;
            let joint_idx = joint_indices[influence_idx];

            if joint_idx >= 0 && (joint_idx as usize) < joint_xforms.len() {
                let w = joint_weights[influence_idx];
                if w != 0.0 {
                    // row-vector: initialN * jointXform, matching C++ `initialN*jointXforms[i]`
                    let transformed = initial_n * joint_xforms[joint_idx as usize];
                    n += transformed * w as f64;
                }
            } else {
                return false;
            }
        }

        let normalized = n.normalized();
        normals[ni] = Vec3f::new(
            normalized.x as f32,
            normalized.y as f32,
            normalized.z as f32,
        );
    }

    true
}

/// Skin face-varying normals using LBS with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight). Uses `face_vertex_indices`
/// to map normal indices to point indices.
///
/// Matches C++ `UsdSkelSkinFaceVaryingNormalsLBS()` with interleaved influences.
pub fn skin_face_varying_normals_lbs_interleaved(
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    face_vertex_indices: &[i32],
    normals: &mut [Vec3f],
) -> bool {
    // Extract indices and weights from interleaved data
    let mut joint_indices = Vec::with_capacity(influences.len());
    let mut joint_weights = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }

    skin_face_varying_normals_lbs(
        geom_bind_transform,
        joint_xforms,
        &joint_indices,
        &joint_weights,
        num_influences_per_point,
        face_vertex_indices,
        normals,
    )
}

/// Skin normals using LBS with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinNormalsLBS()` with interleaved influences.
pub fn skin_normals_lbs_interleaved(
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    normals: &mut [Vec3f],
) -> bool {
    if influences.len() != normals.len() * num_influences_per_point {
        return false;
    }

    let mut joint_indices = Vec::with_capacity(influences.len());
    let mut joint_weights = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }

    skin_normals_lbs(
        geom_bind_transform,
        joint_xforms,
        &joint_indices,
        &joint_weights,
        num_influences_per_point,
        normals,
    )
}

/// Skin normals using the specified skinning method with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinNormals()` with interleaved influences.
pub fn skin_normals_interleaved(
    skinning_method: &Token,
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    normals: &mut [Vec3f],
) -> bool {
    // Unpack interleaved influences
    let mut joint_indices: Vec<i32> = Vec::with_capacity(influences.len());
    let mut joint_weights: Vec<f32> = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }
    if skinning_method == SKINNING_METHOD_LBS {
        skin_normals_lbs(
            geom_bind_transform,
            joint_xforms,
            &joint_indices,
            &joint_weights,
            num_influences_per_point,
            normals,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_normals_dqs(
            geom_bind_transform,
            joint_xforms,
            &joint_indices,
            &joint_weights,
            num_influences_per_point,
            normals,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Skin face-varying normals using dual quaternion skinning (DQS).
///
/// Like `skin_normals_dqs` but uses `face_vertex_indices` to map normals to points.
/// Matches C++ `_SkinNormalsDQS()` with `_FaceVaryingPointIndexFn`.
pub fn skin_face_varying_normals_dqs(
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    face_vertex_indices: &[i32],
    normals: &mut [Vec3f],
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }
    if face_vertex_indices.len() != normals.len() {
        return false;
    }

    let n_joints = joint_xforms.len();
    let num_points = joint_indices.len() / num_influences_per_point;

    // Convert joint 3x3 rotation matrices to quaternions + scale matrices.
    let mut joint_quats: Vec<Quatd> = Vec::with_capacity(n_joints);
    let mut joint_scales: Vec<Matrix3d> = Vec::with_capacity(n_joints);
    let mut has_joint_scale = false;
    for xform in joint_xforms {
        let rot_mat = xform.orthonormalized();
        let q_leg = rot_mat.extract_rotation_quaternion();
        let quat = Quatd::new(
            q_leg.real(),
            Vec3d::new(
                q_leg.imaginary().x,
                q_leg.imaginary().y,
                q_leg.imaginary().z,
            ),
        );
        joint_quats.push(quat);
        if let Some(rot_inv) = rot_mat.inverse() {
            let scale_mat = *xform * rot_inv;
            if !is_close_matrix3(&scale_mat, &Matrix3d::identity(), 1e-6) {
                has_joint_scale = true;
            }
            joint_scales.push(scale_mat);
        } else {
            joint_scales.push(Matrix3d::identity());
        }
    }

    for ni in 0..normals.len() {
        let point_idx = face_vertex_indices[ni];
        if point_idx < 0 || point_idx as usize >= num_points {
            eprintln!(
                "faceVertexIndices is out of range [{}] at index [{}]",
                point_idx, ni
            );
            return false;
        }
        let pi = point_idx as usize;

        let v = Vec3d::new(
            normals[ni].x as f64,
            normals[ni].y as f64,
            normals[ni].z as f64,
        );
        // Row-vector convention: initialN = normal * geomBindTransform (v * M).
        let initial_n = v * *geom_bind_transform;
        let initial_nf = Vec3f::new(initial_n.x as f32, initial_n.y as f32, initial_n.z as f32);

        // Find pivot quaternion (joint with max weight)
        let pivot_quat = {
            let mut best_w = f32::NEG_INFINITY;
            let mut best_q = Quatd::new(1.0, Vec3d::new(0.0, 0.0, 0.0));
            for wi in 0..num_influences_per_point {
                let influence_idx = pi * num_influences_per_point + wi;
                if influence_idx >= joint_indices.len() {
                    break;
                }
                let joint_idx = joint_indices[influence_idx];
                let w = joint_weights[influence_idx];
                if w > best_w && joint_idx >= 0 && (joint_idx as usize) < n_joints {
                    best_w = w;
                    best_q = joint_quats[joint_idx as usize];
                }
            }
            best_q
        };

        let mut scaled_n = Vec3f::new(0.0, 0.0, 0.0);
        let mut weighted_sum_quat = Quatd::new(0.0, Vec3d::new(0.0, 0.0, 0.0));

        for wi in 0..num_influences_per_point {
            let influence_idx = pi * num_influences_per_point + wi;
            if influence_idx >= joint_indices.len() {
                break;
            }
            let joint_idx = joint_indices[influence_idx];
            if joint_idx >= 0 && (joint_idx as usize) < n_joints {
                let mut w = joint_weights[influence_idx];
                if w != 0.0 {
                    if has_joint_scale {
                        // Row-vector convention: initialN * scaleMatrix (v * M).
                        let scale_n = initial_n * joint_scales[joint_idx as usize];
                        scaled_n.x += scale_n.x as f32 * w;
                        scaled_n.y += scale_n.y as f32 * w;
                        scaled_n.z += scale_n.z as f32 * w;
                    }
                    let joint_q = joint_quats[joint_idx as usize];
                    if quat_dot(&joint_q, &pivot_quat) < 0.0 {
                        w = -w;
                    }
                    weighted_sum_quat = Quatd::new(
                        weighted_sum_quat.real() + joint_q.real() * w as f64,
                        Vec3d::new(
                            weighted_sum_quat.imaginary().x + joint_q.imaginary().x * w as f64,
                            weighted_sum_quat.imaginary().y + joint_q.imaginary().y * w as f64,
                            weighted_sum_quat.imaginary().z + joint_q.imaginary().z * w as f64,
                        ),
                    );
                }
            } else {
                eprintln!(
                    "Out of range joint index {} at influence index {} (num joints = {}).",
                    joint_idx,
                    ni * num_influences_per_point + wi,
                    n_joints
                );
                return false;
            }
        }

        if !has_joint_scale {
            scaled_n = initial_nf;
        }
        weighted_sum_quat = weighted_sum_quat.normalized();
        let sv = Vec3d::new(scaled_n.x as f64, scaled_n.y as f64, scaled_n.z as f64);
        let rotated = weighted_sum_quat.transform(&sv);
        let result = rotated.normalized();
        normals[ni] = Vec3f::new(result.x as f32, result.y as f32, result.z as f32);
    }

    true
}

/// Skin face-varying normals using the specified skinning method.
///
/// Uses `face_vertex_indices` to map normal indices to point indices.
///
/// Matches C++ `UsdSkelSkinFaceVaryingNormals()` (non-LBS variant).
pub fn skin_face_varying_normals(
    skinning_method: &Token,
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    num_influences_per_point: usize,
    face_vertex_indices: &[i32],
    normals: &mut [Vec3f],
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_face_varying_normals_lbs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            face_vertex_indices,
            normals,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_face_varying_normals_dqs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            num_influences_per_point,
            face_vertex_indices,
            normals,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Skin face-varying normals with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinFaceVaryingNormals()` with interleaved influences.
pub fn skin_face_varying_normals_interleaved(
    skinning_method: &Token,
    geom_bind_transform: &Matrix3d,
    joint_xforms: &[Matrix3d],
    influences: &[Vec2f],
    num_influences_per_point: usize,
    face_vertex_indices: &[i32],
    normals: &mut [Vec3f],
) -> bool {
    let num_points = influences.len() / num_influences_per_point;
    let mut joint_indices = Vec::with_capacity(influences.len());
    let mut joint_weights = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }

    if joint_indices.len() != num_points * num_influences_per_point {
        return false;
    }
    if skinning_method == SKINNING_METHOD_LBS {
        skin_face_varying_normals_lbs(
            geom_bind_transform,
            joint_xforms,
            &joint_indices,
            &joint_weights,
            num_influences_per_point,
            face_vertex_indices,
            normals,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_face_varying_normals_dqs(
            geom_bind_transform,
            joint_xforms,
            &joint_indices,
            &joint_weights,
            num_influences_per_point,
            face_vertex_indices,
            normals,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

// ============================================================================
// Transform Skinning
// ============================================================================

/// Skin a transform using linear blend skinning (LBS).
///
/// The `joint_xforms` are skinning transforms given in skeleton space,
/// while the `geom_bind_transform` provides the transform that initially
/// places a primitive in that same skeleton space.
///
/// Matches C++ `UsdSkelSkinTransformLBS()`.
pub fn skin_transform_lbs(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    xform: &mut Matrix4d,
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }

    // Early-out for the common case where an object is rigidly bound to a single joint
    if joint_indices.len() == 1 && (joint_weights[0] - 1.0).abs() < 1e-6 {
        let joint_idx = joint_indices[0];
        if joint_idx >= 0 && (joint_idx as usize) < joint_xforms.len() {
            *xform = *geom_bind_transform * joint_xforms[joint_idx as usize];
            return true;
        } else {
            eprintln!(
                "Out of range joint index {} at index 0 (num joints = {}).",
                joint_idx,
                joint_xforms.len()
            );
            return false;
        }
    }

    // Compute a 4-point frame to describe the transform
    let pivot = geom_bind_transform.extract_translation();
    let pivot_f = Vec3f::new(pivot.x as f32, pivot.y as f32, pivot.z as f32);

    let row0 = geom_bind_transform.row3(0);
    let row1 = geom_bind_transform.row3(1);
    let row2 = geom_bind_transform.row3(2);

    let mut frame_points = [
        Vec3f::new(
            pivot_f.x + row0.x as f32,
            pivot_f.y + row0.y as f32,
            pivot_f.z + row0.z as f32,
        ),
        Vec3f::new(
            pivot_f.x + row1.x as f32,
            pivot_f.y + row1.y as f32,
            pivot_f.z + row1.z as f32,
        ),
        Vec3f::new(
            pivot_f.x + row2.x as f32,
            pivot_f.y + row2.y as f32,
            pivot_f.z + row2.z as f32,
        ),
        pivot_f,
    ];

    // Skin each frame point
    for pi in 0..4 {
        let initial_p = Vec3d::new(
            frame_points[pi].x as f64,
            frame_points[pi].y as f64,
            frame_points[pi].z as f64,
        );

        let mut p = Vec3d::new(0.0, 0.0, 0.0);

        for wi in 0..joint_indices.len() {
            let joint_idx = joint_indices[wi];
            if joint_idx >= 0 && (joint_idx as usize) < joint_xforms.len() {
                let w = joint_weights[wi];
                if w != 0.0 {
                    let transformed = joint_xforms[joint_idx as usize].transform_point(&initial_p);
                    p += transformed * w as f64;
                }
            } else {
                eprintln!(
                    "Out of range joint index {} at index {} (num joints = {}).",
                    joint_idx,
                    wi,
                    joint_xforms.len()
                );
                return false;
            }
        }

        frame_points[pi] = Vec3f::new(p.x as f32, p.y as f32, p.z as f32);
    }

    // Reconstruct matrix from frame points
    let skinned_pivot = &frame_points[3];
    xform.set_translate(&Vec3d::new(
        skinned_pivot.x as f64,
        skinned_pivot.y as f64,
        skinned_pivot.z as f64,
    ));

    for i in 0..3 {
        let basis = Vec3d::new(
            (frame_points[i].x - skinned_pivot.x) as f64,
            (frame_points[i].y - skinned_pivot.y) as f64,
            (frame_points[i].z - skinned_pivot.z) as f64,
        );
        xform.set_row3(i, &basis);
    }

    true
}

/// Single-precision overload of skin_transform_lbs.
///
/// Matches C++ `UsdSkelSkinTransformLBS()` with GfMatrix4f.
pub fn skin_transform_lbs_f(
    geom_bind_transform: &Matrix4f,
    joint_xforms: &[Matrix4f],
    joint_indices: &[i32],
    joint_weights: &[f32],
    xform: &mut Matrix4f,
) -> bool {
    let geom_bind_d = Matrix4d::from(*geom_bind_transform);
    let joint_xforms_d: Vec<Matrix4d> = joint_xforms.iter().map(|m| Matrix4d::from(*m)).collect();
    let mut xform_d = Matrix4d::identity();
    let result = skin_transform_lbs(
        &geom_bind_d,
        &joint_xforms_d,
        joint_indices,
        joint_weights,
        &mut xform_d,
    );
    if result {
        *xform = mat4d_to_f(&xform_d);
    }
    result
}

/// Skin a transform using dual quaternion skinning (DQS).
///
/// Matches C++ `UsdSkel_SkinTransformDQS()`. Converts joint matrices to
/// dual quaternions, then does hemisphere-consistent DQ blending on a 4-point
/// frame representing the transform. Scale is handled with LBS if needed.
pub fn skin_transform_dqs(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    xform: &mut Matrix4d,
) -> bool {
    if joint_indices.len() != joint_weights.len() {
        return false;
    }

    // Early-out for a single joint with weight ~1.0
    if joint_indices.len() == 1 && (joint_weights[0] - 1.0).abs() < 1e-6 {
        let joint_idx = joint_indices[0];
        if joint_idx >= 0 && (joint_idx as usize) < joint_xforms.len() {
            *xform = *geom_bind_transform * joint_xforms[joint_idx as usize];
            return true;
        } else {
            eprintln!(
                "Out of range joint index {} at index 0 (num joints = {}).",
                joint_idx,
                joint_xforms.len()
            );
            return false;
        }
    }

    let n_joints = joint_xforms.len();

    // Convert joint 4x4 matrices to dual quaternions + scale matrices.
    // Matches C++ _ConvertToDualQuaternions().
    let mut joint_dqs: Vec<DualQuatd> = Vec::with_capacity(n_joints);
    let mut joint_scales: Vec<Matrix3d> = Vec::with_capacity(n_joints);
    let mut has_joint_scale = false;
    for jxform in joint_xforms {
        // factor() decomposes: M = scaleOrient * scale * rot * translate
        if let Some((rot_mat, translation)) = decompose_rotation_translation(jxform) {
            let q_leg = rot_mat.extract_rotation_quaternion();
            let rot_q = Quatd::new(
                q_leg.real(),
                Vec3d::new(
                    q_leg.imaginary().x,
                    q_leg.imaginary().y,
                    q_leg.imaginary().z,
                ),
            );
            joint_dqs.push(DualQuatd::from_rotation_translation(&rot_q, &translation));

            // Scale = jxform * inv(rot_translate)
            // Approximate: extract upper-left 3x3, remove rotation
            let rot_3 = rot_mat; // already orthonormal
            if let Some(rot_inv) = rot_3.inverse() {
                let m3 = jxform.extract_rotation_matrix();
                let scale_mat = m3 * rot_inv;
                if !is_close_matrix3(&scale_mat, &Matrix3d::identity(), 1e-6) {
                    has_joint_scale = true;
                }
                joint_scales.push(scale_mat);
            } else {
                joint_scales.push(Matrix3d::identity());
            }
        } else {
            // Degenerate matrix - use zero DQ
            joint_dqs.push(DualQuatd::zero());
            joint_scales.push(Matrix3d::identity());
        }
    }

    // Build 4-point frame from geomBindTransform:
    // [row0 basis + pivot, row1 basis + pivot, row2 basis + pivot, pivot]
    let pivot = geom_bind_transform.extract_translation();
    let pivot_f = Vec3f::new(pivot.x as f32, pivot.y as f32, pivot.z as f32);
    let row0 = geom_bind_transform.row3(0);
    let row1 = geom_bind_transform.row3(1);
    let row2 = geom_bind_transform.row3(2);

    let mut frame_points = [
        Vec3f::new(
            pivot_f.x + row0.x as f32,
            pivot_f.y + row0.y as f32,
            pivot_f.z + row0.z as f32,
        ),
        Vec3f::new(
            pivot_f.x + row1.x as f32,
            pivot_f.y + row1.y as f32,
            pivot_f.z + row1.z as f32,
        ),
        Vec3f::new(
            pivot_f.x + row2.x as f32,
            pivot_f.y + row2.y as f32,
            pivot_f.z + row2.z as f32,
        ),
        pivot_f,
    ];

    // Find pivot DQ (joint with max weight)
    let pivot_quat = {
        let mut best_w = f32::NEG_INFINITY;
        let mut best_q = Quatd::new(1.0, Vec3d::new(0.0, 0.0, 0.0));
        for wi in 0..joint_indices.len() {
            let joint_idx = joint_indices[wi];
            let w = joint_weights[wi];
            if w > best_w && joint_idx >= 0 && (joint_idx as usize) < n_joints {
                best_w = w;
                best_q = *joint_dqs[joint_idx as usize].real();
            }
        }
        best_q
    };

    let mut scaled_points = [Vec3f::new(0.0, 0.0, 0.0); 4];
    let mut weighted_sum_dq = DualQuatd::zero();

    for wi in 0..joint_indices.len() {
        let joint_idx = joint_indices[wi];
        if joint_idx >= 0 && (joint_idx as usize) < n_joints {
            let mut w = joint_weights[wi];
            if w != 0.0 {
                // Scale with LBS if any joint has non-identity scale.
                // Row-vector convention: framePoint * scaleMatrix (v * M).
                if has_joint_scale {
                    for pi in 0..4 {
                        let fp = frame_points[pi];
                        let fp_d = Vec3d::new(fp.x as f64, fp.y as f64, fp.z as f64);
                        let scaled = fp_d * joint_scales[joint_idx as usize];
                        scaled_points[pi].x += scaled.x as f32 * w;
                        scaled_points[pi].y += scaled.y as f32 * w;
                        scaled_points[pi].z += scaled.z as f32 * w;
                    }
                }

                // DQS rotation/translation - hemisphere flip
                let joint_dq = &joint_dqs[joint_idx as usize];
                if quat_dot(joint_dq.real(), &pivot_quat) < 0.0 {
                    w = -w;
                }
                weighted_sum_dq += *joint_dq * w as f64;
            }
        } else {
            eprintln!(
                "Out of range joint index {} at index {} (num joints = {}).",
                joint_idx, wi, n_joints
            );
            return false;
        }
    }

    weighted_sum_dq = weighted_sum_dq.normalized();

    // Apply DQ to each frame point
    for pi in 0..4 {
        let src = if has_joint_scale {
            scaled_points[pi]
        } else {
            frame_points[pi]
        };
        let src_d = Vec3d::new(src.x as f64, src.y as f64, src.z as f64);
        let deformed = weighted_sum_dq.transform(&src_d);
        frame_points[pi] = Vec3f::new(deformed.x as f32, deformed.y as f32, deformed.z as f32);
    }

    // Reconstruct matrix from deformed frame points
    let skinned_pivot = frame_points[3];
    xform.set_translate(&Vec3d::new(
        skinned_pivot.x as f64,
        skinned_pivot.y as f64,
        skinned_pivot.z as f64,
    ));
    for i in 0..3 {
        let basis = Vec3d::new(
            (frame_points[i].x - skinned_pivot.x) as f64,
            (frame_points[i].y - skinned_pivot.y) as f64,
            (frame_points[i].z - skinned_pivot.z) as f64,
        );
        xform.set_row3(i, &basis);
    }

    true
}

/// Decompose a 4x4 matrix into (rotation_3x3, translation).
/// Returns None if the matrix is degenerate.
fn decompose_rotation_translation(m: &Matrix4d) -> Option<(Matrix3d, Vec3d)> {
    let mut rot = m.extract_rotation_matrix();
    rot.orthonormalize();
    let t = m.extract_translation();
    Some((rot, t))
}

/// Skin a transform using the specified skinning method.
///
/// Matches C++ `UsdSkelSkinTransform()`.
pub fn skin_transform(
    skinning_method: &Token,
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    joint_indices: &[i32],
    joint_weights: &[f32],
    xform: &mut Matrix4d,
) -> bool {
    if skinning_method == SKINNING_METHOD_LBS {
        skin_transform_lbs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            xform,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_transform_dqs(
            geom_bind_transform,
            joint_xforms,
            joint_indices,
            joint_weights,
            xform,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

/// Skin a transform using LBS with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinTransformLBS()` with interleaved influences.
pub fn skin_transform_lbs_interleaved(
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    influences: &[Vec2f],
    xform: &mut Matrix4d,
) -> bool {
    let mut joint_indices = Vec::with_capacity(influences.len());
    let mut joint_weights = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }

    skin_transform_lbs(
        geom_bind_transform,
        joint_xforms,
        &joint_indices,
        &joint_weights,
        xform,
    )
}

/// Skin a transform using the specified skinning method with interleaved influences.
///
/// Each Vec2f contains (joint_index, weight).
///
/// Matches C++ `UsdSkelSkinTransform()` with interleaved influences.
pub fn skin_transform_interleaved(
    skinning_method: &Token,
    geom_bind_transform: &Matrix4d,
    joint_xforms: &[Matrix4d],
    influences: &[Vec2f],
    xform: &mut Matrix4d,
) -> bool {
    // Unpack interleaved influences
    let mut joint_indices: Vec<i32> = Vec::with_capacity(influences.len());
    let mut joint_weights: Vec<f32> = Vec::with_capacity(influences.len());
    for inf in influences {
        joint_indices.push(inf.x as i32);
        joint_weights.push(inf.y);
    }
    if skinning_method == SKINNING_METHOD_LBS {
        skin_transform_lbs(
            geom_bind_transform,
            joint_xforms,
            &joint_indices,
            &joint_weights,
            xform,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        skin_transform_dqs(
            geom_bind_transform,
            joint_xforms,
            &joint_indices,
            &joint_weights,
            xform,
        )
    } else {
        eprintln!("Unknown skinning method: '{}'", skinning_method.as_str());
        false
    }
}

// ============================================================================
// Blend Shape Application
// ============================================================================

/// Apply a single blend shape to points.
///
/// The shape is given as a slice of offsets. If the `indices` slice is not
/// empty, it provides the index into the `points` slice at which each offset
/// should be mapped. Otherwise, the `offsets` slice must be the same size as
/// the `points` slice.
///
/// Matches C++ `UsdSkelApplyBlendShape()`.
pub fn apply_blend_shape(
    weight: f32,
    offsets: &[Vec3f],
    indices: &[i32],
    points: &mut [Vec3f],
) -> bool {
    // Early out if weight is zero
    if weight.abs() < 1e-6 {
        return true;
    }

    if indices.is_empty() {
        // Non-indexed blend shape
        if offsets.len() != points.len() {
            eprintln!(
                "Size of non-indexed offsets [{}] != size of points [{}]",
                offsets.len(),
                points.len()
            );
            return false;
        }

        for i in 0..points.len() {
            points[i] += offsets[i] * weight;
        }
    } else {
        // Indexed blend shape
        if offsets.len() != indices.len() {
            eprintln!(
                "Size of indexed offsets [{}] != size of indices [{}]",
                offsets.len(),
                indices.len()
            );
            return false;
        }

        for i in 0..offsets.len() {
            let index = indices[i];
            if index >= 0 && (index as usize) < points.len() {
                points[index as usize] += offsets[i] * weight;
            } else {
                eprintln!(
                    "Out of range point index {} (num points = {}).",
                    index,
                    points.len()
                );
                return false;
            }
        }
    }

    true
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_weights() {
        let mut weights = vec![1.0, 2.0, 3.0, 4.0]; // 2 components, 2 influences each
        assert!(normalize_weights(&mut weights, 2, f32::EPSILON));

        // First component: 1+2=3, normalized: 1/3, 2/3
        assert!((weights[0] - 1.0 / 3.0).abs() < 0.001);
        assert!((weights[1] - 2.0 / 3.0).abs() < 0.001);
        // Second component: 3+4=7, normalized: 3/7, 4/7
        assert!((weights[2] - 3.0 / 7.0).abs() < 0.001);
        assert!((weights[3] - 4.0 / 7.0).abs() < 0.001);
    }

    #[test]
    fn test_sort_influences() {
        let mut indices = vec![0, 1, 2, 3];
        let mut weights = vec![0.1, 0.4, 0.3, 0.2]; // 2 components, 2 influences each

        assert!(sort_influences(&mut indices, &mut weights, 2));

        // First component: (0.1, 0) and (0.4, 1) -> sorted: (0.4, 1), (0.1, 0)
        assert_eq!(weights[0], 0.4);
        assert_eq!(indices[0], 1);
        assert_eq!(weights[1], 0.1);
        assert_eq!(indices[1], 0);

        // Second component: (0.3, 2) and (0.2, 3) -> sorted: (0.3, 2), (0.2, 3)
        assert_eq!(weights[2], 0.3);
        assert_eq!(indices[2], 2);
        assert_eq!(weights[3], 0.2);
        assert_eq!(indices[3], 3);
    }

    #[test]
    fn test_concat_joint_transforms() {
        // Simple chain: root -> child
        let topology = Topology::from_parent_indices(vec![-1, 0]);

        let local_xforms = vec![Matrix4d::identity(), Matrix4d::identity()];
        let mut xforms = vec![Matrix4d::default(); 2];

        assert!(concat_joint_transforms(
            &topology,
            &local_xforms,
            &mut xforms,
            None
        ));

        // Both should be identity
        assert_eq!(xforms[0], Matrix4d::identity());
        assert_eq!(xforms[1], Matrix4d::identity());
    }

    #[test]
    fn test_expand_constant_influences() {
        let mut weights = vec![0.5, 0.5];
        assert!(expand_constant_influences_to_varying(&mut weights, 3));

        assert_eq!(weights.len(), 6);
        assert_eq!(weights, vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_apply_blend_shape_non_indexed() {
        let mut points = vec![
            Vec3f::new(0.0, 0.0, 0.0),
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
        ];
        let offsets = vec![
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
            Vec3f::new(0.0, 0.0, 1.0),
        ];

        assert!(apply_blend_shape(0.5, &offsets, &[], &mut points));

        assert!((points[0].x - 0.5).abs() < 0.001);
        assert!((points[1].y - 0.5).abs() < 0.001);
        assert!((points[2].z - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_apply_blend_shape_indexed() {
        let mut points = vec![
            Vec3f::new(0.0, 0.0, 0.0),
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
        ];
        let offsets = vec![Vec3f::new(1.0, 0.0, 0.0)];
        let indices = vec![1]; // Apply only to point 1

        assert!(apply_blend_shape(1.0, &offsets, &indices, &mut points));

        assert_eq!(points[0], Vec3f::new(0.0, 0.0, 0.0)); // unchanged
        assert_eq!(points[1], Vec3f::new(2.0, 0.0, 0.0)); // offset applied
        assert_eq!(points[2], Vec3f::new(0.0, 1.0, 0.0)); // unchanged
    }

    #[test]
    fn test_make_transform_and_decompose() {
        let translate = Vec3f::new(1.0, 2.0, 3.0);
        let rotate = Quatf::identity();
        let scale = Vec3h::new(
            Half::from_f32(1.0),
            Half::from_f32(1.0),
            Half::from_f32(1.0),
        );

        let mut xform = Matrix4d::identity();
        make_transform(&translate, &rotate, &scale, &mut xform);

        let mut out_translate = Vec3f::default();
        let mut out_rotate = Quatf::default();
        let mut out_scale = Vec3h::default();

        assert!(decompose_transform(
            &xform,
            &mut out_translate,
            &mut out_rotate,
            &mut out_scale
        ));

        assert!((out_translate.x - 1.0).abs() < 0.001);
        assert!((out_translate.y - 2.0).abs() < 0.001);
        assert!((out_translate.z - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_skin_points_lbs_identity() {
        let geom_bind = Matrix4d::identity();
        let joint_xforms = vec![Matrix4d::identity()];
        let joint_indices = vec![0, 0]; // 2 points, 1 influence each
        let joint_weights = vec![1.0, 1.0];
        let mut points = vec![Vec3f::new(1.0, 0.0, 0.0), Vec3f::new(0.0, 1.0, 0.0)];

        assert!(skin_points_lbs(
            &geom_bind,
            &joint_xforms,
            &joint_indices,
            &joint_weights,
            1,
            &mut points
        ));

        // Points should be unchanged with identity transforms
        assert!((points[0].x - 1.0).abs() < 0.001);
        assert!((points[1].y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_resize_influences_shrink() {
        let mut indices = vec![0, 1, 2, 3, 4, 5]; // 2 components, 3 influences each
        let mut weights = vec![0.5, 0.3, 0.2, 0.6, 0.3, 0.1];

        assert!(resize_influences(&mut indices, &mut weights, 3, 2));

        assert_eq!(indices.len(), 4);
        assert_eq!(weights.len(), 4);
        // Should be truncated and renormalized
    }

    #[test]
    fn test_resize_influences_grow() {
        let mut indices = vec![0, 1, 2, 3]; // 2 components, 2 influences each
        let mut weights = vec![0.6, 0.4, 0.7, 0.3];

        assert!(resize_influences(&mut indices, &mut weights, 2, 3));

        assert_eq!(indices.len(), 6);
        assert_eq!(weights.len(), 6);
        // Check padding
        assert_eq!(indices[2], 0);
        assert_eq!(weights[2], 0.0);
        assert_eq!(indices[5], 0);
        assert_eq!(weights[5], 0.0);
    }

    #[test]
    fn test_normalize_weights_zero_sum() {
        // All-zero weight set must be zeroed out (not NaN) when sum < eps
        let mut weights = vec![0.0, 0.0, 1.0, 2.0];
        assert!(normalize_weights(&mut weights, 2, f32::EPSILON));
        // First component: sum = 0.0 -> all zeroed
        assert_eq!(weights[0], 0.0);
        assert_eq!(weights[1], 0.0);
        // Second component: sum = 3.0 -> normalized
        assert!((weights[2] - 1.0 / 3.0).abs() < 1e-6);
        assert!((weights[3] - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_weights_already_normalized() {
        let mut weights = vec![0.3, 0.7];
        assert!(normalize_weights(&mut weights, 2, f32::EPSILON));
        assert!((weights[0] - 0.3).abs() < 1e-6);
        assert!((weights[1] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_weights_invalid_divisor() {
        // Zero num_influences_per_component must return false
        let mut weights = vec![1.0, 2.0];
        assert!(!normalize_weights(&mut weights, 0, f32::EPSILON));
    }

    #[test]
    fn test_normalize_weights_indivisible_size() {
        // 3 weights with 2 per component is not divisible -> false
        let mut weights = vec![1.0, 2.0, 3.0];
        assert!(!normalize_weights(&mut weights, 2, f32::EPSILON));
    }

    #[test]
    fn test_apply_blend_shape_zero_weight() {
        // weight == 0 -> points unchanged (early-out path)
        let original = vec![Vec3f::new(1.0, 2.0, 3.0)];
        let mut points = original.clone();
        let offsets = vec![Vec3f::new(10.0, 10.0, 10.0)];
        assert!(apply_blend_shape(0.0, &offsets, &[], &mut points));
        assert_eq!(points, original);
    }

    #[test]
    fn test_apply_blend_shape_full_weight() {
        // weight == 1.0 -> full offset applied
        let mut points = vec![Vec3f::new(0.0, 0.0, 0.0)];
        let offsets = vec![Vec3f::new(5.0, -3.0, 1.0)];
        assert!(apply_blend_shape(1.0, &offsets, &[], &mut points));
        assert!((points[0].x - 5.0).abs() < 1e-6);
        assert!((points[0].y + 3.0).abs() < 1e-6);
        assert!((points[0].z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_apply_blend_shape_indexed_out_of_range() {
        let mut points = vec![Vec3f::new(0.0, 0.0, 0.0)];
        let offsets = vec![Vec3f::new(1.0, 0.0, 0.0)];
        let indices = vec![5]; // out of range
        // Must return false on out-of-range index
        assert!(!apply_blend_shape(1.0, &offsets, &indices, &mut points));
    }

    #[test]
    fn test_blend_shape_pipeline_multi_shape() {
        // Simulate multiple blend shapes applied to same point set:
        // shape A: move point 0 in X, shape B: move point 1 in Y
        let mut points = vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        // Shape A: offset point 0 by (2, 0, 0)
        let offsets_a = vec![Vec3f::new(2.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        // Shape B: offset point 1 by (0, 3, 0)
        let offsets_b = vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 3.0, 0.0)];

        assert!(apply_blend_shape(0.5, &offsets_a, &[], &mut points));
        assert!(apply_blend_shape(1.0, &offsets_b, &[], &mut points));

        // point[0] moved by 0.5 * 2 = 1.0 in X
        assert!((points[0].x - 1.0).abs() < 1e-6);
        // point[1] moved by 1.0 * 3 = 3.0 in Y
        assert!((points[1].y - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_interleave_influences() {
        let indices = vec![0, 1, 2, 3];
        let weights = vec![0.6, 0.4, 0.8, 0.2];
        let mut interleaved = vec![Vec2f::default(); 4];

        assert!(interleave_influences(&indices, &weights, &mut interleaved));

        // Interleaved layout: Vec2f(index as f32, weight) — x = index, y = weight
        assert_eq!(interleaved[0][0] as i32, 0); // index 0
        assert!((interleaved[0][1] - 0.6).abs() < 1e-6); // weight 0.6
        assert_eq!(interleaved[1][0] as i32, 1); // index 1
        assert!((interleaved[1][1] - 0.4).abs() < 1e-6); // weight 0.4
    }

    #[test]
    fn test_sort_influences_single_component() {
        // Single component with 3 influences, already sorted desc -> unchanged
        let mut indices = vec![5, 2, 8];
        let mut weights = vec![0.7, 0.2, 0.1];
        assert!(sort_influences(&mut indices, &mut weights, 3));
        assert_eq!(weights[0], 0.7);
        assert_eq!(indices[0], 5);
    }

    #[test]
    fn test_skin_points_lbs_translation() {
        // Single joint with a translation: all points shift by (1, 0, 0)
        let mut translate = Matrix4d::identity();
        translate[3][0] = 1.0; // row-major: row 3, col 0 = tx
        let joint_xforms = vec![translate];
        let geom_bind = Matrix4d::identity();
        // 2 points, 1 influence each (indices=[0,0], weights=[1,1])
        let joint_indices = vec![0, 0];
        let joint_weights = vec![1.0f32, 1.0f32];
        let mut points = vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(2.0, 0.0, 0.0)];
        assert!(skin_points_lbs(
            &geom_bind,
            &joint_xforms,
            &joint_indices,
            &joint_weights,
            1,
            &mut points
        ));
        // Both points shifted by (1, 0, 0)
        assert!(
            (points[0].x - 1.0).abs() < 1e-5,
            "point[0].x = {}",
            points[0].x
        );
        assert!(
            (points[1].x - 3.0).abs() < 1e-5,
            "point[1].x = {}",
            points[1].x
        );
    }

    #[test]
    fn test_expand_constant_influences_zero_size() {
        // Expanding to size 0 should clear the array
        let mut arr = vec![0.5f32, 0.5f32];
        assert!(expand_constant_influences_to_varying(&mut arr, 0));
        assert!(arr.is_empty());
    }

    /// Regression test: row-vector vs column-vector convention.
    ///
    /// Uses an asymmetric geom_bind_transform so that v*M != M*v.
    /// The non-uniform scale matrix S = [[2,0,0],[0,3,0],[0,0,1]] applied in
    /// row-vector order (v * S) doubles X and triples Y; column-vector order
    /// (S * v) gives the same result for a pure-scale diagonal matrix, so we
    /// use an off-diagonal (shear) matrix to distinguish the two.
    ///
    /// Shear matrix K = [[1,1,0],[0,1,0],[0,0,1]] in row-vector convention:
    ///   v * K:  (x+y*0, x+y, z) -- wait, let's be explicit.
    ///   v.x' = v.x*K[0][0] + v.y*K[1][0] + v.z*K[2][0] = v.x*1 + v.y*0 + 0 = v.x
    ///   v.y' = v.x*K[0][1] + v.y*K[1][1] + v.z*K[2][1] = v.x*1 + v.y*1 + 0
    ///   => (1,0,0) * K = (1,1,0)  [row-vector, correct for USD]
    ///   K * (1,0,0) column-wise = (1,0,0)  [column-vector would give different result]
    ///
    /// So for normal (1,0,0):
    ///   row-vector: (1,0,0)*K = (1,1,0), normalized = (1/sqrt2, 1/sqrt2, 0)
    ///   col-vector: K*(1,0,0) = (1,0,0), normalized = (1,0,0)
    #[test]
    fn test_row_vector_convention_regression() {
        // Shear matrix: K[0][1] = 1 makes (1,0,0)*K = (1,1,0)
        // Row-major storage: K = [[1,1,0],[0,1,0],[0,0,1]]
        let shear = Matrix3d::new(1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);

        // skin_normals_lbs: geom_bind = shear, identity joint
        // input normal (1,0,0), expected row-vector result (1,1,0) -> normalized (0.707, 0.707, 0)
        let mut normals_lbs = vec![Vec3f::new(1.0, 0.0, 0.0)];
        assert!(skin_normals_lbs(
            &shear,
            &[shear], // joint xform = same shear
            &[0],
            &[1.0f32],
            1,
            &mut normals_lbs,
        ));
        // v=(1,0,0), after geomBind shear -> (1,1,0); after joint shear -> (1,2,0);
        // normalized = (1/sqrt5, 2/sqrt5, 0)
        let sqrt5 = 5.0_f32.sqrt();
        assert!(
            (normals_lbs[0].x - 1.0 / sqrt5).abs() < 1e-5,
            "LBS normal.x expected {} got {}",
            1.0 / sqrt5,
            normals_lbs[0].x
        );
        assert!(
            (normals_lbs[0].y - 2.0 / sqrt5).abs() < 1e-5,
            "LBS normal.y expected {} got {}",
            2.0 / sqrt5,
            normals_lbs[0].y
        );
        // column-vector M*v would give (1,0,0) -> (1,1,0) normalized, completely different from (1,2,0)

        // skin_points_lbs with asymmetric geom_bind and joint.
        // geom_bind has tx=0, joint has tx=5 to check translation direction.
        // Use asymmetric rotation: 90 degrees around Z in row-vector Imath convention.
        // Row-vector 90 deg Z rot matrix: [[0,-1,0,0],[1,0,0,0],[0,0,1,0],[0,0,0,1]]
        // v=(1,0,0) * M = (0,-1,0)  [row-vector]
        // M * v=(1,0,0) = (-1,0,0)  [column-vector - different!]
        let mut rot90z = Matrix4d::identity();
        rot90z[0][0] = 0.0;
        rot90z[0][1] = -1.0;
        rot90z[1][0] = 1.0;
        rot90z[1][1] = 0.0;
        let mut points_lbs = vec![Vec3f::new(1.0, 0.0, 0.0)];
        assert!(skin_points_lbs(
            &Matrix4d::identity(),
            &[rot90z],
            &[0],
            &[1.0f32],
            1,
            &mut points_lbs,
        ));
        // row-vector: (1,0,0) * rot90z = (0,-1,0)
        assert!(
            (points_lbs[0].x).abs() < 1e-5,
            "LBS point.x: expected ~0, got {}",
            points_lbs[0].x
        );
        assert!(
            (points_lbs[0].y + 1.0).abs() < 1e-5,
            "LBS point.y: expected -1, got {}",
            points_lbs[0].y
        );
        // column-vector would give rot90z*(1,0,0) = (-1,0,0), which is wrong
    }
}
