
//! Smooth normals computation for meshes.
//!
//! Computes per-vertex normals by averaging cross products of incident face edges.
//! Requires HdVertexAdjacency built from mesh topology.
//! See pxr/imaging/hd/smoothNormals.h for C++ reference.

use super::types::HdVec4_2_10_10_10_Rev;
use super::vertex_adjacency::HdVertexAdjacency;
use usd_gf::{Vec3d, Vec3f};

/// Compute smooth normals for f32 points.
///
/// Returns one normal per vertex. Uses vertex adjacency to iterate incident
/// faces and average cross products of (next-curr) x (prev-curr).
pub fn compute_smooth_normals(
    adjacency: &HdVertexAdjacency,
    num_points: usize,
    points: &[[f32; 3]],
) -> Vec<[f32; 3]> {
    let table = adjacency.adjacency_table();
    let num_adj_points = adjacency.num_points();
    let num_points = num_points.min(num_adj_points);

    let mut normals = vec![[0.0f32; 3]; num_points];

    for i in 0..num_points {
        let offset_idx = i * 2;
        let offset = table[offset_idx] as usize;
        let valence = table[offset_idx + 1] as usize;

        let curr = if i < points.len() {
            Vec3f::new(points[i][0], points[i][1], points[i][2])
        } else {
            Vec3f::zero()
        };

        let mut normal = Vec3f::zero();
        let mut e = offset;
        for _ in 0..valence {
            let prev_idx = table[e] as usize;
            let next_idx = table[e + 1] as usize;
            e += 2;

            let prev = if prev_idx < points.len() {
                Vec3f::new(
                    points[prev_idx][0],
                    points[prev_idx][1],
                    points[prev_idx][2],
                )
            } else {
                Vec3f::zero()
            };
            let next = if next_idx < points.len() {
                Vec3f::new(
                    points[next_idx][0],
                    points[next_idx][1],
                    points[next_idx][2],
                )
            } else {
                Vec3f::zero()
            };

            // normal += Cross(next-curr, prev-curr)
            normal += (next - curr).cross(&(prev - curr));
        }

        let n = normal.normalized();
        normals[i] = [n.x, n.y, n.z];
    }

    normals
}

/// Compute smooth normals packed into HdVec4f_2_10_10_10_REV format (f64 points).
///
/// Matches C++ Hd_SmoothNormals::ComputeSmoothNormalsPacked for GfVec3d.
pub fn compute_smooth_normals_packed_f64(
    adjacency: &HdVertexAdjacency,
    num_points: usize,
    points: &[[f64; 3]],
) -> Vec<HdVec4_2_10_10_10_Rev> {
    let table = adjacency.adjacency_table();
    let num_adj_points = adjacency.num_points();
    let num_points = num_points.min(num_adj_points);

    let mut normals = vec![HdVec4_2_10_10_10_Rev::from_i32(0); num_points];

    for i in 0..num_points {
        let offset_idx = i * 2;
        let offset = table[offset_idx] as usize;
        let valence = table[offset_idx + 1] as usize;

        let curr = if i < points.len() {
            Vec3d::new(points[i][0], points[i][1], points[i][2])
        } else {
            Vec3d::zero()
        };

        let mut normal = Vec3d::zero();
        let mut e = offset;
        for _ in 0..valence {
            let prev_idx = table[e] as usize;
            let next_idx = table[e + 1] as usize;
            e += 2;

            let prev = if prev_idx < points.len() {
                Vec3d::new(
                    points[prev_idx][0],
                    points[prev_idx][1],
                    points[prev_idx][2],
                )
            } else {
                Vec3d::zero()
            };
            let next = if next_idx < points.len() {
                Vec3d::new(
                    points[next_idx][0],
                    points[next_idx][1],
                    points[next_idx][2],
                )
            } else {
                Vec3d::zero()
            };

            normal += (next - curr).cross(&(prev - curr));
        }

        let n = normal.normalized();
        normals[i] = HdVec4_2_10_10_10_Rev::from_vec3(n.x as f32, n.y as f32, n.z as f32);
    }

    normals
}

/// Compute smooth normals packed into HdVec4f_2_10_10_10_REV format (f32 points).
///
/// Matches C++ Hd_SmoothNormals::ComputeSmoothNormalsPacked.
pub fn compute_smooth_normals_packed(
    adjacency: &HdVertexAdjacency,
    num_points: usize,
    points: &[[f32; 3]],
) -> Vec<HdVec4_2_10_10_10_Rev> {
    let table = adjacency.adjacency_table();
    let num_adj_points = adjacency.num_points();
    let num_points = num_points.min(num_adj_points);

    let mut normals = vec![HdVec4_2_10_10_10_Rev::from_i32(0); num_points];

    for i in 0..num_points {
        let offset_idx = i * 2;
        let offset = table[offset_idx] as usize;
        let valence = table[offset_idx + 1] as usize;

        let curr = if i < points.len() {
            Vec3f::new(points[i][0], points[i][1], points[i][2])
        } else {
            Vec3f::zero()
        };

        let mut normal = Vec3f::zero();
        let mut e = offset;
        for _ in 0..valence {
            let prev_idx = table[e] as usize;
            let next_idx = table[e + 1] as usize;
            e += 2;

            let prev = if prev_idx < points.len() {
                Vec3f::new(
                    points[prev_idx][0],
                    points[prev_idx][1],
                    points[prev_idx][2],
                )
            } else {
                Vec3f::zero()
            };
            let next = if next_idx < points.len() {
                Vec3f::new(
                    points[next_idx][0],
                    points[next_idx][1],
                    points[next_idx][2],
                )
            } else {
                Vec3f::zero()
            };

            normal += (next - curr).cross(&(prev - curr));
        }

        let n = normal.normalized();
        normals[i] = HdVec4_2_10_10_10_Rev::from_vec3(n.x, n.y, n.z);
    }

    normals
}

/// Compute smooth normals for f64 points.
pub fn compute_smooth_normals_f64(
    adjacency: &HdVertexAdjacency,
    num_points: usize,
    points: &[[f64; 3]],
) -> Vec<[f64; 3]> {
    let table = adjacency.adjacency_table();
    let num_adj_points = adjacency.num_points();
    let num_points = num_points.min(num_adj_points);

    let mut normals = vec![[0.0f64; 3]; num_points];

    for i in 0..num_points {
        let offset_idx = i * 2;
        let offset = table[offset_idx] as usize;
        let valence = table[offset_idx + 1] as usize;

        let curr = if i < points.len() {
            Vec3d::new(points[i][0], points[i][1], points[i][2])
        } else {
            Vec3d::zero()
        };

        let mut normal = Vec3d::zero();
        let mut e = offset;
        for _ in 0..valence {
            let prev_idx = table[e] as usize;
            let next_idx = table[e + 1] as usize;
            e += 2;

            let prev = if prev_idx < points.len() {
                Vec3d::new(
                    points[prev_idx][0],
                    points[prev_idx][1],
                    points[prev_idx][2],
                )
            } else {
                Vec3d::zero()
            };
            let next = if next_idx < points.len() {
                Vec3d::new(
                    points[next_idx][0],
                    points[next_idx][1],
                    points[next_idx][2],
                )
            } else {
                Vec3d::zero()
            };

            normal += (next - curr).cross(&(prev - curr));
        }

        let n = normal.normalized();
        normals[i] = [n.x, n.y, n.z];
    }

    normals
}
