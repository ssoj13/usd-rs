
//! Flat normals computation for meshes.
//!
//! Computes per-face normals using triangle fan from vertex 0.
//! See pxr/imaging/hd/flatNormals.h for C++ reference.

use super::types::HdVec4_2_10_10_10_Rev;
use usd_gf::Vec3f;
use usd_tf::Token;

/// Token for right-handed orientation (no flip).
const RIGHT_HANDED: &str = "rightHanded";

/// Mesh topology view for normal computation.
///
/// Provides face vertex counts, indices, and orientation.
pub trait MeshTopologyView {
    /// Face vertex counts (vertices per face).
    fn face_vertex_counts(&self) -> &[i32];

    /// Face vertex indices (flat array).
    fn face_vertex_indices(&self) -> &[i32];

    /// Orientation token (rightHanded, leftHanded).
    fn orientation(&self) -> &Token;
}

impl MeshTopologyView for usd_px_osd::MeshTopology {
    fn face_vertex_counts(&self) -> &[i32] {
        self.face_vertex_counts()
    }

    fn face_vertex_indices(&self) -> &[i32] {
        self.face_vertex_indices()
    }

    fn orientation(&self) -> &Token {
        self.orientation()
    }
}

/// Compute flat normals for a mesh (f32 points).
///
/// Returns one normal per face. Uses triangle fan from vertex 0,
/// averages triangle normals per face.
pub fn compute_flat_normals<T: MeshTopologyView>(
    topology: &T,
    points: &[[f32; 3]],
) -> Vec<[f32; 3]> {
    let face_counts = topology.face_vertex_counts();
    let face_indices = topology.face_vertex_indices();
    let flip = topology.orientation() != RIGHT_HANDED;
    let flip_sign = if flip { -1.0 } else { 1.0 };

    let num_faces = face_counts.len();
    let mut normals = vec![[0.0f32; 3]; num_faces];
    let mut offset = 0usize;

    for i in 0..num_faces {
        let count = face_counts[i] as usize;
        if count < 3 || offset + count > face_indices.len() {
            offset += count;
            continue;
        }

        let v0_idx = face_indices[offset] as usize;
        let v0 = if v0_idx < points.len() {
            Vec3f::new(points[v0_idx][0], points[v0_idx][1], points[v0_idx][2])
        } else {
            Vec3f::new(0.0, 0.0, 0.0)
        };

        let mut normal = Vec3f::new(0.0, 0.0, 0.0);
        for j in 2..count {
            let v1_idx = face_indices[offset + j - 1] as usize;
            let v2_idx = face_indices[offset + j] as usize;
            let v1 = if v1_idx < points.len() {
                Vec3f::new(points[v1_idx][0], points[v1_idx][1], points[v1_idx][2])
            } else {
                Vec3f::new(0.0, 0.0, 0.0)
            };
            let v2 = if v2_idx < points.len() {
                Vec3f::new(points[v2_idx][0], points[v2_idx][1], points[v2_idx][2])
            } else {
                Vec3f::new(0.0, 0.0, 0.0)
            };
            let cross = (v1 - v0).cross(&(v2 - v0)) * flip_sign;
            normal += cross;
        }

        let n = normal.normalized();
        normals[i] = [n.x, n.y, n.z];

        offset += count;
    }

    normals
}

/// Compute flat normals packed into HdVec4f_2_10_10_10_REV format (f32 points).
///
/// Matches C++ Hd_FlatNormals::ComputeFlatNormalsPacked.
pub fn compute_flat_normals_packed<T: MeshTopologyView>(
    topology: &T,
    points: &[[f32; 3]],
) -> Vec<HdVec4_2_10_10_10_Rev> {
    let face_counts = topology.face_vertex_counts();
    let face_indices = topology.face_vertex_indices();
    let flip = topology.orientation() != RIGHT_HANDED;
    let flip_sign = if flip { -1.0f32 } else { 1.0f32 };

    let num_faces = face_counts.len();
    let mut normals = vec![HdVec4_2_10_10_10_Rev::from_i32(0); num_faces];
    let mut offset = 0usize;

    for i in 0..num_faces {
        let count = face_counts[i] as usize;
        if count < 3 || offset + count > face_indices.len() {
            offset += count;
            continue;
        }

        let v0_idx = face_indices[offset] as usize;
        let v0 = if v0_idx < points.len() {
            Vec3f::new(points[v0_idx][0], points[v0_idx][1], points[v0_idx][2])
        } else {
            Vec3f::new(0.0, 0.0, 0.0)
        };

        let mut normal = Vec3f::new(0.0, 0.0, 0.0);
        for j in 2..count {
            let v1_idx = face_indices[offset + j - 1] as usize;
            let v2_idx = face_indices[offset + j] as usize;
            let v1 = if v1_idx < points.len() {
                Vec3f::new(points[v1_idx][0], points[v1_idx][1], points[v1_idx][2])
            } else {
                Vec3f::new(0.0, 0.0, 0.0)
            };
            let v2 = if v2_idx < points.len() {
                Vec3f::new(points[v2_idx][0], points[v2_idx][1], points[v2_idx][2])
            } else {
                Vec3f::new(0.0, 0.0, 0.0)
            };
            let cross = (v1 - v0).cross(&(v2 - v0)) * flip_sign;
            normal += cross;
        }

        let n = normal.normalized();
        normals[i] = HdVec4_2_10_10_10_Rev::from_vec3(n.x, n.y, n.z);
        offset += count;
    }

    normals
}

/// Compute flat normals packed into HdVec4f_2_10_10_10_REV format (f64 points).
///
/// Matches C++ Hd_FlatNormals::ComputeFlatNormalsPacked for GfVec3d.
pub fn compute_flat_normals_packed_f64<T: MeshTopologyView>(
    topology: &T,
    points: &[[f64; 3]],
) -> Vec<HdVec4_2_10_10_10_Rev> {
    let face_counts = topology.face_vertex_counts();
    let face_indices = topology.face_vertex_indices();
    let flip = topology.orientation() != RIGHT_HANDED;
    let flip_sign = if flip { -1.0f64 } else { 1.0f64 };

    let num_faces = face_counts.len();
    let mut normals = vec![HdVec4_2_10_10_10_Rev::from_i32(0); num_faces];
    let mut offset = 0usize;

    for i in 0..num_faces {
        let count = face_counts[i] as usize;
        if count < 3 || offset + count > face_indices.len() {
            offset += count;
            continue;
        }

        let v0_idx = face_indices[offset] as usize;
        let v0 = if v0_idx < points.len() {
            usd_gf::Vec3d::new(points[v0_idx][0], points[v0_idx][1], points[v0_idx][2])
        } else {
            usd_gf::Vec3d::new(0.0, 0.0, 0.0)
        };

        let mut normal = usd_gf::Vec3d::new(0.0, 0.0, 0.0);
        for j in 2..count {
            let v1_idx = face_indices[offset + j - 1] as usize;
            let v2_idx = face_indices[offset + j] as usize;
            let v1 = if v1_idx < points.len() {
                usd_gf::Vec3d::new(points[v1_idx][0], points[v1_idx][1], points[v1_idx][2])
            } else {
                usd_gf::Vec3d::new(0.0, 0.0, 0.0)
            };
            let v2 = if v2_idx < points.len() {
                usd_gf::Vec3d::new(points[v2_idx][0], points[v2_idx][1], points[v2_idx][2])
            } else {
                usd_gf::Vec3d::new(0.0, 0.0, 0.0)
            };
            let cross = (v1 - v0).cross(&(v2 - v0)) * flip_sign;
            normal += cross;
        }

        let n = normal.normalized();
        normals[i] = HdVec4_2_10_10_10_Rev::from_vec3(n.x as f32, n.y as f32, n.z as f32);
        offset += count;
    }

    normals
}

/// Compute flat normals for f64 points.
pub fn compute_flat_normals_f64<T: MeshTopologyView>(
    topology: &T,
    points: &[[f64; 3]],
) -> Vec<[f64; 3]> {
    let face_counts = topology.face_vertex_counts();
    let face_indices = topology.face_vertex_indices();
    let flip = topology.orientation() != RIGHT_HANDED;
    let flip_sign = if flip { -1.0 } else { 1.0 };

    let num_faces = face_counts.len();
    let mut normals = vec![[0.0f64; 3]; num_faces];
    let mut offset = 0usize;

    for i in 0..num_faces {
        let count = face_counts[i] as usize;
        if count < 3 || offset + count > face_indices.len() {
            offset += count;
            continue;
        }

        let v0_idx = face_indices[offset] as usize;
        let v0 = if v0_idx < points.len() {
            usd_gf::Vec3d::new(points[v0_idx][0], points[v0_idx][1], points[v0_idx][2])
        } else {
            usd_gf::Vec3d::new(0.0, 0.0, 0.0)
        };

        let mut normal = usd_gf::Vec3d::new(0.0, 0.0, 0.0);
        for j in 2..count {
            let v1_idx = face_indices[offset + j - 1] as usize;
            let v2_idx = face_indices[offset + j] as usize;
            let v1 = if v1_idx < points.len() {
                usd_gf::Vec3d::new(points[v1_idx][0], points[v1_idx][1], points[v1_idx][2])
            } else {
                usd_gf::Vec3d::new(0.0, 0.0, 0.0)
            };
            let v2 = if v2_idx < points.len() {
                usd_gf::Vec3d::new(points[v2_idx][0], points[v2_idx][1], points[v2_idx][2])
            } else {
                usd_gf::Vec3d::new(0.0, 0.0, 0.0)
            };
            let cross = (v1 - v0).cross(&(v2 - v0)) * flip_sign;
            normal += cross;
        }

        let n = normal.normalized();
        normals[i] = [n.x, n.y, n.z];

        offset += count;
    }

    normals
}
