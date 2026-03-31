
//! Triangle index builder and face-varying triangulation computations.
//!
//! Converts arbitrary polygon meshes to triangles using fan triangulation.
//! Generates:
//! - Triangle index buffer (3 indices per triangle)
//! - Primitive param buffer (maps triangle -> coarse face index)
//! - Edge index buffer (identifies polygon edges for wireframe)
//! - Face-varying triangulation (expands per-face-vertex data to triangles)
//!
//! See pxr/imaging/hdSt/triangulate.h for C++ reference.

use crate::mesh_topology::HdStMeshTopology;

// ---------------------------------------------------------------------------
// Triangle index builder
// ---------------------------------------------------------------------------

/// Result of triangle index building.
#[derive(Debug, Clone, Default)]
pub struct TriangleIndexResult {
    /// Triangle vertex indices (3 per triangle)
    pub indices: Vec<u32>,
    /// Primitive parameter: coarse face index per triangle
    pub primitive_params: Vec<i32>,
    /// Triangle edge indices: bitmask per triangle indicating real polygon edges
    pub edge_indices: Vec<u8>,
    /// Number of triangles
    pub num_triangles: usize,
}

/// Build triangle indices from mesh topology.
///
/// Performs fan triangulation: for each polygon with N vertices,
/// generates N-2 triangles sharing vertex 0 as the fan hub.
///
/// Index layout after triangulation:
/// ```text
/// ----+--------+--------+------
/// ... |i0 i1 i2|i3 i4 i5| ...   index buffer (3 per triangle)
/// ----+--------+--------+------
/// ... |   m0   |   m1   | ...   primitive param (coarse face index)
/// ----+--------+--------+------
/// ```
pub fn build_triangle_indices(topology: &HdStMeshTopology) -> TriangleIndexResult {
    let (indices, primitive_params) = topology.compute_triangle_indices();
    let edge_indices = topology.compute_triangle_edge_indices();
    let num_triangles = indices.len() / 3;

    TriangleIndexResult {
        indices,
        primitive_params,
        edge_indices,
        num_triangles,
    }
}

/// Compute total number of triangles from face vertex counts (excluding holes).
pub fn compute_triangle_count(
    face_vertex_counts: &[i32],
    hole_indices: &[i32],
) -> usize {
    let mut count = 0usize;
    for (fi, &nv) in face_vertex_counts.iter().enumerate() {
        if hole_indices.contains(&(fi as i32)) {
            continue;
        }
        if nv >= 3 {
            count += (nv - 2) as usize;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Face-varying triangulation
// ---------------------------------------------------------------------------

/// Triangulate face-varying data.
///
/// Face-varying data has one value per face-vertex. After triangulation,
/// each triangle needs 3 face-varying values indexed by position in the
/// original per-face-vertex array.
///
/// Input: `fvar_data` with one element per face-vertex (sum of face_vertex_counts).
/// Output: reindexed data with one element per triangle vertex.
pub fn triangulate_face_varying<T: Clone>(
    topology: &HdStMeshTopology,
    fvar_data: &[T],
) -> Vec<T> {
    let mut result = Vec::new();
    let mut offset = 0usize;

    for (face_idx, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as usize;

        if topology.hole_indices.contains(&(face_idx as i32)) {
            offset += count;
            continue;
        }

        if count < 3 {
            offset += count;
            continue;
        }

        // Fan triangulation: use fvar values at face-vertex positions
        for i in 1..count - 1 {
            if offset < fvar_data.len()
                && offset + i < fvar_data.len()
                && offset + i + 1 < fvar_data.len()
            {
                result.push(fvar_data[offset].clone());
                result.push(fvar_data[offset + i].clone());
                result.push(fvar_data[offset + i + 1].clone());
            }
        }

        offset += count;
    }

    result
}

/// Build face-varying index mapping: maps triangulated face-vertex index
/// to original face-varying index.
///
/// Returns a vec where result[tri_fv_idx] = original_fv_idx.
pub fn build_fvar_triangle_index_map(topology: &HdStMeshTopology) -> Vec<u32> {
    let mut mapping = Vec::new();
    let mut offset = 0u32;

    for (face_idx, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as u32;

        if topology.hole_indices.contains(&(face_idx as i32)) {
            offset += count;
            continue;
        }

        if count < 3 {
            offset += count;
            continue;
        }

        for i in 1..count - 1 {
            mapping.push(offset); // v0 of fan hub
            mapping.push(offset + i);
            mapping.push(offset + i + 1);
        }

        offset += count;
    }

    mapping
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_triangle_indices_quad() {
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        let result = build_triangle_indices(&topo);

        assert_eq!(result.num_triangles, 2);
        assert_eq!(result.indices.len(), 6);
        assert_eq!(result.primitive_params, vec![0, 0]);
    }

    #[test]
    fn test_build_triangle_indices_mixed() {
        // Quad + triangle
        let topo = HdStMeshTopology::from_faces(
            vec![4, 3],
            vec![0, 1, 2, 3, 4, 5, 6],
        );
        let result = build_triangle_indices(&topo);

        assert_eq!(result.num_triangles, 3); // 2 from quad + 1 from triangle
        assert_eq!(result.primitive_params, vec![0, 0, 1]);
    }

    #[test]
    fn test_compute_triangle_count() {
        assert_eq!(compute_triangle_count(&[4, 3, 5], &[]), 6); // 2+1+3
        assert_eq!(compute_triangle_count(&[4, 3, 5], &[1]), 5); // skip tri
        assert_eq!(compute_triangle_count(&[2], &[]), 0); // degenerate
    }

    #[test]
    fn test_triangulate_face_varying_quad() {
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);

        // UV data: one per face-vertex
        let uvs: Vec<[f32; 2]> = vec![
            [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
        ];

        let tri_uvs = triangulate_face_varying(&topo, &uvs);
        assert_eq!(tri_uvs.len(), 6); // 2 triangles * 3 verts
        assert_eq!(tri_uvs[0], [0.0, 0.0]); // fan hub
    }

    #[test]
    fn test_fvar_index_map() {
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        let map = build_fvar_triangle_index_map(&topo);

        assert_eq!(map.len(), 6); // 2 triangles * 3
        // First triangle: fv[0], fv[1], fv[2]
        assert_eq!(map[0], 0);
        assert_eq!(map[1], 1);
        assert_eq!(map[2], 2);
        // Second triangle: fv[0], fv[2], fv[3]
        assert_eq!(map[3], 0);
        assert_eq!(map[4], 2);
        assert_eq!(map[5], 3);
    }

    #[test]
    fn test_triangulate_with_holes() {
        let mut topo = HdStMeshTopology::from_faces(
            vec![3, 3],
            vec![0, 1, 2, 3, 4, 5],
        );
        topo.hole_indices = vec![0]; // First face is a hole

        let data: Vec<f32> = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
        let result = triangulate_face_varying(&topo, &data);
        // Only second triangle survives
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 40.0);
    }

    #[test]
    fn test_edge_indices() {
        // Pentagon -> 3 triangles
        let topo = HdStMeshTopology::from_faces(vec![5], vec![0, 1, 2, 3, 4]);
        let result = build_triangle_indices(&topo);

        assert_eq!(result.edge_indices.len(), 3);
        // First tri: edges 0,1 are real (bit 0 + bit 1 = 3)
        assert_eq!(result.edge_indices[0] & 0b011, 0b011);
        // Last tri: edges 1,2 are real (bit 1 + bit 2 = 6)
        assert_eq!(result.edge_indices[2] & 0b110, 0b110);
    }
}
