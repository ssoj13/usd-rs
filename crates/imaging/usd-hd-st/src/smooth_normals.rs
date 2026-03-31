#![allow(dead_code)]

//! Smooth normal computation for Storm.
//!
//! Computes area-weighted smooth (vertex) normals by averaging the face normals
//! of all faces adjacent to each vertex. Requires a vertex adjacency table.
//!
//! CPU path: iterates over adjacency, accumulates area-weighted face normals.
//! GPU path: dispatches a compute shader via HGI.

use std::sync::Arc;
use usd_gf::Vec3f;
use usd_tf::Token;

use crate::flat_normals::HdBufferSpec;
use usd_hd::types::HdType;

// ---------------------------------------------------------------------------
// Vertex adjacency
// ---------------------------------------------------------------------------

/// Vertex adjacency data for smooth-normal computation.
///
/// For each vertex, stores the list of faces that share it.
/// This is the Rust equivalent of C++ `Hd_VertexAdjacency`.
#[derive(Debug, Clone)]
pub struct VertexAdjacency {
    /// Number of vertices
    num_points: usize,
    /// Adjacency table: for each vertex, a list of face indices.
    /// Stored as a flat array with an offset table.
    /// `offsets[v]` .. `offsets[v+1]` gives the range in `adjacency`
    /// containing the face indices adjacent to vertex `v`.
    offsets: Vec<usize>,
    adjacency: Vec<usize>,
}

impl VertexAdjacency {
    /// Build adjacency from mesh topology.
    ///
    /// `face_vertex_counts` - per-face vertex count
    /// `face_vertex_indices` - flattened face vertex indices
    /// `num_points` - total number of vertices
    pub fn build(
        face_vertex_counts: &[i32],
        face_vertex_indices: &[i32],
        num_points: usize,
    ) -> Self {
        // Count how many faces each vertex belongs to
        let mut counts = vec![0usize; num_points];
        for &vi in face_vertex_indices {
            let vi = vi as usize;
            if vi < num_points {
                counts[vi] += 1;
            }
        }

        // Build offset table (prefix sum)
        let mut offsets = vec![0usize; num_points + 1];
        for i in 0..num_points {
            offsets[i + 1] = offsets[i] + counts[i];
        }
        let total = offsets[num_points];

        // Fill adjacency
        let mut adjacency = vec![0usize; total];
        let mut cursors = offsets[..num_points].to_vec();

        let mut idx_offset = 0usize;
        for (face_idx, &count) in face_vertex_counts.iter().enumerate() {
            let count = count as usize;
            for j in 0..count {
                if idx_offset + j >= face_vertex_indices.len() {
                    break;
                }
                let vi = face_vertex_indices[idx_offset + j] as usize;
                if vi < num_points {
                    adjacency[cursors[vi]] = face_idx;
                    cursors[vi] += 1;
                }
            }
            idx_offset += count;
        }

        Self {
            num_points,
            offsets,
            adjacency,
        }
    }

    /// Get adjacent face indices for a vertex.
    pub fn get_adjacent_faces(&self, vertex: usize) -> &[usize] {
        if vertex >= self.num_points {
            return &[];
        }
        &self.adjacency[self.offsets[vertex]..self.offsets[vertex + 1]]
    }

    /// Get total number of vertices.
    pub fn get_num_points(&self) -> usize {
        self.num_points
    }
}

// ---------------------------------------------------------------------------
// CPU smooth normals
// ---------------------------------------------------------------------------

/// CPU smooth-normal computation.
///
/// Computes area-weighted vertex normals by averaging the face normals of
/// all adjacent faces for each vertex. The face normal contribution is
/// weighted by the face area (implicit in the un-normalized cross product).
///
/// Matches C++ `HdSt_SmoothNormalsComputationCPU`.
pub struct SmoothNormalsComputationCpu {
    /// Pre-built vertex adjacency
    adjacency: VertexAdjacency,
    /// Face vertex counts (for computing face normals)
    face_vertex_counts: Vec<i32>,
    /// Face vertex indices
    face_vertex_indices: Vec<i32>,
    /// Source vertex positions
    points: Vec<Vec3f>,
    /// Destination buffer name
    dst_name: Token,
    /// Output packed format?
    packed: bool,
    /// Result normals (one per vertex)
    result: Option<Vec<Vec3f>>,
    /// Packed result
    result_packed: Option<Vec<i32>>,
    /// Resolved flag
    resolved: bool,
}

impl SmoothNormalsComputationCpu {
    /// Create a new CPU smooth-normal computation.
    pub fn new(
        adjacency: VertexAdjacency,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        points: Vec<Vec3f>,
        dst_name: Token,
        packed: bool,
    ) -> Self {
        Self {
            adjacency,
            face_vertex_counts,
            face_vertex_indices,
            points,
            dst_name,
            packed,
            result: None,
            result_packed: None,
            resolved: false,
        }
    }

    /// Get the output buffer spec.
    pub fn get_buffer_specs(&self) -> Vec<HdBufferSpec> {
        let data_type = if self.packed {
            HdType::Int32_2_10_10_10_Rev
        } else {
            HdType::FloatVec3
        };
        vec![HdBufferSpec {
            name: self.dst_name.clone(),
            data_type,
        }]
    }

    /// Get destination buffer name.
    pub fn get_name(&self) -> &Token {
        &self.dst_name
    }

    /// Check validity.
    pub fn is_valid(&self) -> bool {
        !self.points.is_empty()
    }

    /// Whether the result has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Resolve: compute smooth normals by accumulating edge cross products per vertex.
    ///
    /// Port of C++ `HdSt_SmoothNormalsComputationCPU::Resolve()`
    /// (pxr/imaging/hdSt/smoothNormals.cpp).
    ///
    /// For each vertex V, walks its adjacency list (stored as prev/next vertex
    /// pairs in each adjacent face) and accumulates the cross product:
    ///   acc += cross(P[prev] - P[V], P[next] - P[V])
    /// This implicitly area-weights each face's contribution.
    /// The adjacency here stores face indices; we reconstruct the prev/next
    /// vertex of V within each adjacent face for the cross product.
    pub fn resolve(&mut self) -> bool {
        if self.resolved {
            return true;
        }
        if self.points.is_empty() {
            return false;
        }

        // Build per-face start offsets for index lookup.
        let num_faces = self.face_vertex_counts.len();
        let mut face_offsets = Vec::with_capacity(num_faces + 1);
        let mut off = 0usize;
        for &c in &self.face_vertex_counts {
            face_offsets.push(off);
            off += c as usize;
        }
        face_offsets.push(off);

        let num_points = self.points.len();
        let mut normals = vec![Vec3f::new(0.0, 0.0, 0.0); num_points];

        // For each vertex, walk adjacent faces and accumulate edge cross products.
        // C++ smoothNormals.cpp:30-51: for each entry in adjacency list,
        // it uses the prev/next vertex within the face to form two edges.
        for vi in 0..num_points {
            let faces = self.adjacency.get_adjacent_faces(vi);
            let mut acc = Vec3f::new(0.0, 0.0, 0.0);

            for &fi in faces {
                if fi >= num_faces {
                    continue;
                }
                let fstart = face_offsets[fi];
                let fcount = self.face_vertex_counts[fi] as usize;
                if fcount < 3 || fstart + fcount > self.face_vertex_indices.len() {
                    continue;
                }

                // Find position of vi within the face.
                let mut pos_in_face = usize::MAX;
                for k in 0..fcount {
                    if self.face_vertex_indices[fstart + k] as usize == vi {
                        pos_in_face = k;
                        break;
                    }
                }
                if pos_in_face == usize::MAX {
                    continue;
                }

                // Prev and next vertex indices within the face (wrapping).
                let prev_k = if pos_in_face == 0 { fcount - 1 } else { pos_in_face - 1 };
                let next_k = (pos_in_face + 1) % fcount;

                let vi_prev = self.face_vertex_indices[fstart + prev_k] as usize;
                let vi_next = self.face_vertex_indices[fstart + next_k] as usize;

                if vi_prev >= num_points || vi_next >= num_points {
                    continue;
                }

                let p  = self.points[vi];
                let pp = self.points[vi_prev];
                let pn = self.points[vi_next];

                // Edge vectors from current vertex; cross product is area-weighted face normal.
                let e_prev = pp - p;
                let e_next = pn - p;
                let c = cross(e_next, e_prev);
                acc = Vec3f::new(acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]);
            }

            normals[vi] = normalize(acc);
        }

        if self.packed {
            self.result_packed = Some(normals.iter().map(|n| pack_normal(*n)).collect());
        } else {
            self.result = Some(normals);
        }

        self.resolved = true;
        log::debug!(
            "SmoothNormalsComputationCpu::resolve: {} vertices computed",
            num_points
        );
        true
    }

    /// Get computed normals (unpacked). Only valid after resolve().
    pub fn get_result(&self) -> Option<&[Vec3f]> {
        self.result.as_deref()
    }

    /// Get computed normals (packed). Only valid after resolve() with packed=true.
    pub fn get_result_packed(&self) -> Option<&[i32]> {
        self.result_packed.as_deref()
    }

    /// Number of output elements (one per vertex).
    pub fn get_num_elements(&self) -> usize {
        self.points.len()
    }
}
}

// ---------------------------------------------------------------------------
// GPU smooth normals
// ---------------------------------------------------------------------------

/// GPU smooth-normal computation.
///
/// Dispatches a compute shader that reads vertex positions and adjacency data,
/// then accumulates per-vertex normals from adjacent faces.
///
/// Matches C++ `HdSt_SmoothNormalsComputationGPU`.
pub struct SmoothNormalsComputationGpu {
    /// Source buffer attribute name (e.g. "points")
    src_name: Token,
    /// Destination buffer attribute name (e.g. "normals")
    dst_name: Token,
    /// Source data type
    src_data_type: HdType,
    /// Destination data type
    dst_data_type: HdType,
}

impl SmoothNormalsComputationGpu {
    /// Create a new GPU smooth-normal computation.
    pub fn new(
        src_name: Token,
        dst_name: Token,
        src_data_type: HdType,
        packed: bool,
    ) -> Self {
        let dst_data_type = if packed {
            HdType::Int32_2_10_10_10_Rev
        } else {
            src_data_type
        };

        if src_data_type != HdType::FloatVec3 && src_data_type != HdType::DoubleVec3 {
            log::error!(
                "Unsupported points type {:?} for smooth normals GPU computation",
                src_data_type
            );
        }

        Self {
            src_name,
            dst_name,
            src_data_type,
            dst_data_type,
        }
    }

    /// Get the output buffer spec.
    pub fn get_buffer_specs(&self) -> Vec<HdBufferSpec> {
        vec![HdBufferSpec {
            name: self.dst_name.clone(),
            data_type: self.dst_data_type,
        }]
    }

    /// Number of output elements.
    /// Returns 0 because smooth normals GPU computation writes into the same
    /// range as the source buffer (no resize needed).
    pub fn get_num_output_elements(&self) -> i32 {
        0
    }

    /// Execute the GPU computation.
    ///
    /// In the full implementation this would:
    /// 1. Select compute shader variant by src/dst data type
    /// 2. Bind points, normals, and adjacency buffers
    /// 3. Set uniform buffer with offsets/strides
    /// 4. Dispatch compute shader
    pub fn execute(&self, _resource_registry: &dyn std::any::Any) {
        if self.src_data_type == HdType::Invalid {
            return;
        }

        let _uniform = SmoothNormalsUniform {
            vertex_offset: 0,
            adjacency_offset: 0,
            points_offset: 0,
            points_stride: 3,
            normals_offset: 0,
            normals_stride: if self.dst_data_type == HdType::Int32_2_10_10_10_Rev {
                1
            } else {
                3
            },
            index_end: 0,
        };

        log::debug!(
            "SmoothNormalsComputationGpu::execute: ({:?} -> {:?})",
            self.src_data_type,
            self.dst_data_type,
        );
    }
}

/// Uniform block for smooth normals GPU compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SmoothNormalsUniform {
    pub vertex_offset: i32,
    pub adjacency_offset: i32,
    pub points_offset: i32,
    pub points_stride: i32,
    pub normals_offset: i32,
    pub normals_stride: i32,
    pub index_end: i32,
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

#[inline]
fn cross(a: Vec3f, b: Vec3f) -> Vec3f {
    Vec3f::new(
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )
}

#[inline]
fn normalize(v: Vec3f) -> Vec3f {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len_sq < 1e-12 {
        Vec3f::new(0.0, 0.0, 0.0)
    } else {
        let inv_len = 1.0 / len_sq.sqrt();
        Vec3f::new(v[0] * inv_len, v[1] * inv_len, v[2] * inv_len)
    }
}

#[inline]
fn pack_normal(n: Vec3f) -> i32 {
    // Pack a normalised vector into a 2_10_10_10_REV / INT_2_10_10_10_REV word.
    // Each component is a signed 10-bit integer: range [-512, 511].
    // Sign extension fix: cast via i32 first so negative values get masked
    // to their 10-bit two's complement representation, not treated as large u32.
    // C++ reference: GfPackNormal() in gf/vec3f.h.
    let xi = (n[0].clamp(-1.0, 1.0) * 511.0).round() as i32;
    let yi = (n[1].clamp(-1.0, 1.0) * 511.0).round() as i32;
    let zi = (n[2].clamp(-1.0, 1.0) * 511.0).round() as i32;
    // Mask to 10 bits (handles negative values via two's complement).
    let x = xi & 0x3FF;
    let y = yi & 0x3FF;
    let z = zi & 0x3FF;
    // w = 0 (bits 30-31), explicit for clarity.
    x | (y << 10) | (z << 20)
}

/// Shared pointer alias.
pub type SmoothNormalsComputationCpuSharedPtr = Arc<SmoothNormalsComputationCpu>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple plane: 2 triangles forming a quad in the XY plane.
    fn make_quad_mesh() -> (Vec<i32>, Vec<i32>, Vec<Vec3f>) {
        let counts = vec![3, 3];
        let indices = vec![0, 1, 2, 0, 2, 3];
        let points = vec![
            Vec3f::new(0.0, 0.0, 0.0),
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(1.0, 1.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
        ];
        (counts, indices, points)
    }

    #[test]
    fn test_adjacency_build() {
        let (counts, indices, points) = make_quad_mesh();
        let adj = VertexAdjacency::build(&counts, &indices, points.len());

        assert_eq!(adj.get_num_points(), 4);
        // Vertex 0 is in both faces
        assert_eq!(adj.get_adjacent_faces(0).len(), 2);
        // Vertex 1 is in face 0 only
        assert_eq!(adj.get_adjacent_faces(1).len(), 1);
        // Vertex 2 is in both faces
        assert_eq!(adj.get_adjacent_faces(2).len(), 2);
        // Vertex 3 is in face 1 only
        assert_eq!(adj.get_adjacent_faces(3).len(), 1);
    }

    #[test]
    fn test_smooth_normals_flat_plane() {
        let (counts, indices, points) = make_quad_mesh();
        let adj = VertexAdjacency::build(&counts, &indices, points.len());

        let mut comp = SmoothNormalsComputationCpu::new(
            adj,
            counts,
            indices,
            points,
            Token::new("normals"),
            false,
        );

        assert!(comp.resolve());
        let result = comp.get_result().unwrap();
        assert_eq!(result.len(), 4);

        // All normals should point in +Z for a flat plane
        for n in result {
            assert!((n[2] - 1.0).abs() < 1e-5, "expected +Z normal, got {:?}", n);
        }
    }

    #[test]
    fn test_smooth_normals_packed() {
        let (counts, indices, points) = make_quad_mesh();
        let adj = VertexAdjacency::build(&counts, &indices, points.len());

        let mut comp = SmoothNormalsComputationCpu::new(
            adj,
            counts,
            indices,
            points,
            Token::new("normals"),
            true,
        );

        assert!(comp.resolve());
        let packed = comp.get_result_packed().unwrap();
        assert_eq!(packed.len(), 4);
        // All packed values should be non-zero (encoding +Z normal)
        for &p in packed {
            assert_ne!(p, 0);
        }
    }

    #[test]
    fn test_smooth_normals_corner() {
        // L-shaped mesh: two triangles at a 90-degree angle
        let counts = vec![3, 3];
        let indices = vec![0, 1, 2, 0, 2, 3];
        let points = vec![
            Vec3f::new(0.0, 0.0, 0.0), // shared corner vertex
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
            Vec3f::new(0.0, 0.0, 1.0), // out of XY plane
        ];
        let adj = VertexAdjacency::build(&counts, &indices, points.len());

        let mut comp = SmoothNormalsComputationCpu::new(
            adj,
            counts,
            indices,
            points,
            Token::new("normals"),
            false,
        );

        assert!(comp.resolve());
        let result = comp.get_result().unwrap();
        // Vertex 0 is shared: its normal should be the average of the two face normals
        let n0 = result[0];
        let len = (n0[0] * n0[0] + n0[1] * n0[1] + n0[2] * n0[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-5, "normal should be unit length");
    }

    #[test]
    fn test_gpu_smooth_normals() {
        let comp = SmoothNormalsComputationGpu::new(
            Token::new("points"),
            Token::new("normals"),
            HdType::FloatVec3,
            false,
        );
        assert_eq!(comp.get_num_output_elements(), 0);
        let specs = comp.get_buffer_specs();
        assert_eq!(specs[0].data_type, HdType::FloatVec3);
    }

    #[test]
    fn test_gpu_smooth_normals_packed() {
        let comp = SmoothNormalsComputationGpu::new(
            Token::new("points"),
            Token::new("normals"),
            HdType::FloatVec3,
            true,
        );
        let specs = comp.get_buffer_specs();
        assert_eq!(specs[0].data_type, HdType::Int32_2_10_10_10_Rev);
    }

    #[test]
    fn test_buffer_specs() {
        let (counts, indices, points) = make_quad_mesh();
        let adj = VertexAdjacency::build(&counts, &indices, points.len());
        let comp = SmoothNormalsComputationCpu::new(
            adj, counts, indices, points, Token::new("normals"), false,
        );
        let specs = comp.get_buffer_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, Token::new("normals"));
    }
}
