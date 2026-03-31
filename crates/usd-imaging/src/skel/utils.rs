//! UsdSkelImaging utils - bone mesh utilities.
//!
//! Port of pxr/usdImaging/usdSkelImaging/utils.h/cpp
//!
//! Collection of utility methods for imaging skeletons as bone meshes.

use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3f;
use usd_hd::prim::mesh::HdMeshTopology;
use usd_skel::Topology;

// Bone mesh constants (from C++ _boneVerts, _boneNumVerts, etc.)
const BONE_VERTS: [i32; 12] = [0, 2, 1, 0, 3, 2, 0, 4, 3, 0, 1, 4];
const BONE_NUM_VERTS: i32 = 12;
const BONE_NUM_VERTS_PER_FACE: i32 = 3;
const BONE_NUM_FACES: i32 = 4;
const BONE_NUM_POINTS: i32 = 5;

/// Compute number of bones (joints with a valid parent) in topology.
fn compute_bone_count(topology: &Topology) -> usize {
    let num_joints = topology.num_joints() as i32;
    let mut num_bones = 0usize;
    for i in 0..topology.num_joints() {
        let parent = topology.get_parent(i);
        if parent >= 0 && parent < num_joints {
            num_bones += 1;
        }
    }
    num_bones
}

/// Compute mesh topology for imaging skeleton as bones.
///
/// Bones are constructed from child to parent as a pyramid-shaped object
/// with square base at the parent and tip at the child.
///
/// Returns (mesh_topology, num_points) or None on error.
pub fn compute_bone_topology(skel_topology: &Topology) -> Option<(HdMeshTopology, usize)> {
    let num_bones = compute_bone_count(skel_topology);
    let num_points = num_bones * (BONE_NUM_POINTS as usize);

    let face_vertex_counts = vec![BONE_NUM_VERTS_PER_FACE; num_bones * (BONE_NUM_FACES as usize)];
    let mut face_vertex_indices = vec![0i32; num_bones * (BONE_NUM_VERTS as usize)];

    for i in 0..num_bones {
        let base = i * (BONE_NUM_POINTS as usize);
        for (j, &v) in BONE_VERTS.iter().enumerate() {
            face_vertex_indices[i * (BONE_NUM_VERTS as usize) + j] = v + base as i32;
        }
    }

    let mesh_topology = HdMeshTopology::from_data(face_vertex_counts, face_vertex_indices);
    Some((mesh_topology, num_points))
}

/// Compute mesh points for imaging a skeleton.
///
/// Given the topology and joint skel-space transforms, compute
/// point positions for the bone mesh.
pub fn compute_bone_points(
    topology: &Topology,
    joint_skel_xforms: &[Matrix4d],
    num_points: usize,
    points: &mut [Vec3f],
) -> bool {
    if joint_skel_xforms.len() != topology.num_joints() {
        return false;
    }
    if points.len() < num_points {
        return false;
    }

    let num_bones = compute_bone_count(topology);
    let expected_points = num_bones * (BONE_NUM_POINTS as usize);
    if num_points != expected_points {
        return false;
    }

    let mut bone_idx = 0usize;
    for i in 0..topology.num_joints() {
        let parent = topology.get_parent(i);
        let num_joints = topology.num_joints() as i32;
        if parent < 0 || parent >= num_joints {
            continue;
        }

        let xform = &joint_skel_xforms[i];
        let parent_xform = &joint_skel_xforms[parent as usize];
        let base = bone_idx * (BONE_NUM_POINTS as usize);
        compute_points_for_single_bone(
            xform,
            parent_xform,
            &mut points[base..base + BONE_NUM_POINTS as usize],
        );
        bone_idx += 1;
    }
    true
}

/// Find the index of the parent transform basis best aligned with bone direction.
///
/// Port of C++ `_FindBestAlignedBasis`. For an orthogonal matrix, the best
/// aligned basis has an absolute dot product with dir > PI/4.
/// Returns axis index (0, 1, or 2).
fn find_best_aligned_basis(parent_xform: &Matrix4d, bone_dir: &Vec3f) -> usize {
    let pi_4 = std::f64::consts::FRAC_PI_4;
    let dir = usd_gf::vec3::Vec3d::new(bone_dir.x as f64, bone_dir.y as f64, bone_dir.z as f64);

    for i in 0..2 {
        let row = parent_xform.row3(i);
        if row.dot(&dir).abs() > pi_4 {
            return i;
        }
    }
    // Default to last basis
    2
}

/// Compute mesh points for imaging a single bone.
///
/// Port of C++ `UsdSkelImagingComputePointsForSingleBone`.
/// A bone is a pyramid with tip at child joint and square base at parent.
/// Uses parent transform basis vectors best aligned with bone direction
/// (via `find_best_aligned_basis`) for the base displacement vectors.
pub fn compute_points_for_single_bone(
    xform: &Matrix4d,
    parent_xform: &Matrix4d,
    points: &mut [Vec3f],
) {
    if points.len() < BONE_NUM_POINTS as usize {
        return;
    }

    // Child joint position (tip of bone)
    let child_pos = xform.extract_translation();
    let end = Vec3f::new(child_pos.x as f32, child_pos.y as f32, child_pos.z as f32);

    // Parent joint position (center of base)
    let parent_pos = parent_xform.extract_translation();
    let start = Vec3f::new(
        parent_pos.x as f32,
        parent_pos.y as f32,
        parent_pos.z as f32,
    );

    // Vector from parent to child
    let bone_dir = end - start;

    // Lookup tables matching C++: iAxis[] = {1,0,0}, jAxis[] = {2,2,1}
    static I_AXIS: [usize; 3] = [1, 0, 0];
    static J_AXIS: [usize; 3] = [2, 2, 1];

    // Find which parent transform basis is best aligned with bone direction
    let norm_dir = bone_dir.normalized();
    let principal_axis = find_best_aligned_basis(parent_xform, &norm_dir);

    // Pick i,j basis vectors from parent transform rows
    let row_i = parent_xform.row3(I_AXIS[principal_axis]);
    let i = Vec3f::new(row_i.x as f32, row_i.y as f32, row_i.z as f32).normalized();

    let row_j = parent_xform.row3(J_AXIS[principal_axis]);
    let j = Vec3f::new(row_j.x as f32, row_j.y as f32, row_j.z as f32).normalized();

    // Bone thickness proportional to length (matches C++ 0.1 scalar)
    let size = bone_dir.length() * 0.1;
    let i_scaled = i * size;
    let j_scaled = j * size;

    // Point 0: child (tip)
    points[0] = end;
    // Points 1-4: square base at parent
    points[1] = start + i_scaled + j_scaled;
    points[2] = start + i_scaled - j_scaled;
    points[3] = start - i_scaled - j_scaled;
    points[4] = start - i_scaled + j_scaled;
}

/// Compute joint indices corresponding to each point in a bone mesh.
///
/// Can be used to animate a bone mesh using normal skinning.
/// Does not compute joint weights (they would all be 1).
pub fn compute_bone_joint_indices(
    topology: &Topology,
    joint_indices: &mut [i32],
    num_points: usize,
) -> bool {
    let num_bones = compute_bone_count(topology);
    let expected_points = num_bones * (BONE_NUM_POINTS as usize);
    if num_points != expected_points || joint_indices.len() < num_points {
        return false;
    }

    let mut bone_idx = 0usize;
    for i in 0..topology.num_joints() {
        let parent = topology.get_parent(i);
        let num_joints = topology.num_joints() as i32;
        if parent < 0 || parent >= num_joints {
            continue;
        }

        let base = bone_idx * (BONE_NUM_POINTS as usize);
        // Point 0 (tip) -> child joint
        joint_indices[base] = i as i32;
        // Points 1-4 (base) -> parent joint
        for j in 1..BONE_NUM_POINTS as usize {
            joint_indices[base + j] = parent;
        }
        bone_idx += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_bone_count() {
        // Linear chain: 0 -> 1 -> 2
        let topology = Topology::from_parent_indices(vec![-1, 0, 1]);
        assert_eq!(compute_bone_count(&topology), 2);
    }

    #[test]
    fn test_compute_bone_topology() {
        let topology = Topology::from_parent_indices(vec![-1, 0, 1]);
        let (mesh_topo, num_pts) = compute_bone_topology(&topology).unwrap();
        assert_eq!(num_pts, 2 * 5);
        assert_eq!(mesh_topo.num_faces(), 8);
    }

    #[test]
    fn test_compute_bone_joint_indices() {
        let topology = Topology::from_parent_indices(vec![-1, 0, 1]);
        let (_, num_pts) = compute_bone_topology(&topology).unwrap();
        let mut joint_indices = vec![0i32; num_pts];
        assert!(compute_bone_joint_indices(
            &topology,
            &mut joint_indices,
            num_pts
        ));
    }

    #[test]
    fn test_find_best_aligned_basis_x_aligned() {
        // Identity parent, bone along X -> principal axis 0, so i=row[1], j=row[2]
        let parent = Matrix4d::identity();
        let dir = Vec3f::new(1.0, 0.0, 0.0);
        assert_eq!(find_best_aligned_basis(&parent, &dir), 0);
    }

    #[test]
    fn test_find_best_aligned_basis_y_aligned() {
        // Identity parent, bone along Y -> principal axis 1, so i=row[0], j=row[2]
        let parent = Matrix4d::identity();
        let dir = Vec3f::new(0.0, 1.0, 0.0);
        assert_eq!(find_best_aligned_basis(&parent, &dir), 1);
    }

    #[test]
    fn test_find_best_aligned_basis_z_aligned() {
        // Identity parent, bone along Z -> neither 0 nor 1 match, falls to 2
        let parent = Matrix4d::identity();
        let dir = Vec3f::new(0.0, 0.0, 1.0);
        assert_eq!(find_best_aligned_basis(&parent, &dir), 2);
    }

    #[test]
    fn test_single_bone_points_identity() {
        // Parent at origin, child at (1,0,0)
        let parent_xform = Matrix4d::identity();
        let mut child_xform = Matrix4d::identity();
        child_xform.set_translate(&usd_gf::vec3::Vec3d::new(1.0, 0.0, 0.0));

        let mut points = vec![Vec3f::default(); 5];
        compute_points_for_single_bone(&child_xform, &parent_xform, &mut points);

        // Tip at child
        assert!((points[0].x - 1.0).abs() < 1e-5);
        assert!(points[0].y.abs() < 1e-5);
        assert!(points[0].z.abs() < 1e-5);

        // Base points around parent origin with thickness 0.1
        let size = 0.1f32; // length=1.0 * 0.1
        for p in &points[1..] {
            // All base points should be at x ~= 0 (parent origin)
            assert!(p.x.abs() < 1e-5, "base x={}", p.x);
            // y,z should be +/- size
            assert!((p.y.abs() - size).abs() < 1e-4 || p.y.abs() < 1e-5);
        }
    }
}
