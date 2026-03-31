//! SkelGuideData - Data for computing skeleton guide mesh.
//!
//! Port of pxr/usdImaging/usdSkelImaging/skelGuideData (inline in skelData usage)
//!
//! Used to compute topology and geometry for drawing skeleton as mesh.

use super::skel_data::SkelData;
use super::utils::{compute_bone_joint_indices, compute_bone_points, compute_bone_topology};
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3f;
use usd_sdf::Path;

/// Data to compute the skeleton guide as mesh.
///
/// Mesh depicts posed skeleton by rendering each joint with a parent
/// as a pyramid-shaped bone.
#[derive(Debug, Clone)]
pub struct SkelGuideData {
    /// Path of skeleton prim (for warnings/errors).
    pub prim_path: Path,

    /// Number of joints in topology.
    pub num_joints: usize,

    /// Indices into joints - one per mesh point.
    pub bone_joint_indices: Vec<i32>,

    /// Mesh points before applying skinning transforms.
    pub bone_mesh_points: Vec<Vec3f>,
}

impl SkelGuideData {
    /// Create new empty guide data.
    pub fn new(prim_path: Path, num_joints: usize) -> Self {
        Self {
            prim_path,
            num_joints,
            bone_joint_indices: Vec::new(),
            bone_mesh_points: Vec::new(),
        }
    }
}

/// Compute SkelGuideData from SkelData.
///
/// Builds bone joint indices and mesh points from the skeleton's
/// topology and bind transforms. If bind transforms are empty,
/// returns guide data with empty geometry.
pub fn compute_skel_guide_data(skel_data: &SkelData) -> SkelGuideData {
    let num_joints = skel_data.topology.num_joints();
    let mut guide = SkelGuideData::new(skel_data.prim_path.clone(), num_joints);

    // Need bind transforms to compute bone mesh points
    if skel_data.bind_transforms.is_empty() {
        return guide;
    }

    // Compute bone topology to get num_points
    let (_topo, num_points) = match compute_bone_topology(&skel_data.topology) {
        Some(result) => result,
        None => return guide,
    };

    if num_points == 0 {
        return guide;
    }

    // Compute bone joint indices
    guide.bone_joint_indices = vec![0i32; num_points];
    if !compute_bone_joint_indices(
        &skel_data.topology,
        &mut guide.bone_joint_indices,
        num_points,
    ) {
        log::warn!(
            "Failed to compute bone joint indices for {}",
            skel_data.prim_path
        );
        guide.bone_joint_indices.clear();
        return guide;
    }

    // Convert bind transforms f32 -> f64 for compute_bone_points
    let bind_xforms_d: Vec<Matrix4d> = skel_data
        .bind_transforms
        .iter()
        .map(|m| Matrix4d::from(*m))
        .collect();

    // Compute bone mesh points from bind transforms
    guide.bone_mesh_points = vec![Vec3f::default(); num_points];
    if !compute_bone_points(
        &skel_data.topology,
        &bind_xforms_d,
        num_points,
        &mut guide.bone_mesh_points,
    ) {
        log::warn!(
            "Failed to compute bone mesh points for {}",
            skel_data.prim_path
        );
        guide.bone_mesh_points.clear();
        guide.bone_joint_indices.clear();
    }

    guide
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::matrix4::Matrix4f;
    use usd_skel::Topology;

    #[test]
    fn test_empty_skel_data() {
        let path = Path::from_string("/Skel").unwrap();
        let skel_data = SkelData::new(
            path,
            crate::skel::skeleton_schema::SkeletonSchema::new(None),
        );
        let guide = compute_skel_guide_data(&skel_data);
        assert!(guide.bone_joint_indices.is_empty());
        assert!(guide.bone_mesh_points.is_empty());
    }

    #[test]
    fn test_guide_with_bind_transforms() {
        let path = Path::from_string("/Skel").unwrap();
        let schema = crate::skel::skeleton_schema::SkeletonSchema::new(None);
        let topology = Topology::from_parent_indices(vec![-1, 0, 1]);

        // 3 joints: root at origin, joint1 at (1,0,0), joint2 at (2,0,0)
        let xf0 = Matrix4f::identity();
        let mut xf1 = Matrix4f::identity();
        xf1[3][0] = 1.0;
        let mut xf2 = Matrix4f::identity();
        xf2[3][0] = 2.0;

        let skel_data = SkelData {
            prim_path: path,
            skeleton_schema: schema,
            topology,
            bind_transforms: vec![xf0, xf1, xf2],
            inverse_bind_transforms: vec![
                Matrix4f::identity(),
                Matrix4f::identity(),
                Matrix4f::identity(),
            ],
        };

        let guide = compute_skel_guide_data(&skel_data);
        // 2 bones * 5 points = 10
        assert_eq!(guide.bone_joint_indices.len(), 10);
        assert_eq!(guide.bone_mesh_points.len(), 10);
        assert_eq!(guide.num_joints, 3);
    }
}
