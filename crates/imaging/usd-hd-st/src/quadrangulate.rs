
//! Quadrangulation computations for Catmull-Clark subdivision.
//!
//! Converts arbitrary polygon meshes to quads by inserting face center points.
//! Provides both CPU and GPU quadrangulation paths:
//!
//! **CPU path:**
//! ```text
//! QuadInfoBuilder -> QuadIndexBuilder -> QuadrangulateComputation
//! ```
//!
//! **GPU path:**
//! ```text
//! QuadInfoBuilder -> QuadrangulateTableComputation -> QuadrangulateComputationGPU
//! ```
//!
//! See pxr/imaging/hdSt/quadrangulate.h for C++ reference.

use crate::mesh_topology::HdStMeshTopology;

// ---------------------------------------------------------------------------
// Quad index builder
// ---------------------------------------------------------------------------

/// Result of quad index building.
#[derive(Debug, Clone, Default)]
pub struct QuadIndexResult {
    /// Quad vertex indices (4 per quad)
    pub indices: Vec<u32>,
    /// Primitive parameter: coarse face index per quad
    pub primitive_params: Vec<i32>,
    /// Edge indices per quad (bitmask of real polygon edges)
    pub edge_indices: Vec<u8>,
    /// Number of quads
    pub num_quads: usize,
}

/// Build quad indices from mesh topology.
///
/// Non-quad faces are split by inserting a center point:
/// - Triangles -> 3 quads (center + edge midpoints)
/// - Pentagons -> 5 quads, etc.
/// - Existing quads pass through unchanged.
///
/// Index layout:
/// ```text
/// ----+-----------+-----------+------
/// ... |i0 i1 i2 i3|i4 i5 i6 i7| ...    index buffer (4 per quad)
/// ----+-----------+-----------+------
/// ... |     m0    |     m1    | ...    primitive param (coarse face index)
/// ----+-----------+-----------+------
/// ```
pub fn build_quad_indices(topology: &HdStMeshTopology) -> QuadIndexResult {
    let (indices, primitive_params) = topology.compute_quad_indices();
    let num_quads = indices.len() / 4;

    // Compute edge flags per quad
    let edge_indices = compute_quad_edge_indices(topology);

    QuadIndexResult {
        indices,
        primitive_params,
        edge_indices,
        num_quads,
    }
}

/// Compute edge flags per quad after quadrangulation.
fn compute_quad_edge_indices(topology: &HdStMeshTopology) -> Vec<u8> {
    let mut edge_flags = Vec::new();

    for (face_idx, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as usize;

        if topology.hole_indices.contains(&(face_idx as i32)) {
            continue;
        }

        if count == 4 {
            // Original quad: all 4 edges are real
            edge_flags.push(0b1111);
        } else if count >= 3 {
            // Subdivided face: only the original polygon edges are "real"
            for _ in 0..count {
                // Each sub-quad has one real edge (the original polygon edge)
                // and three internal edges
                edge_flags.push(1 << 1); // edge 1 is the real one
            }
        }
    }

    edge_flags
}

// ---------------------------------------------------------------------------
// Quadrangulation table (for GPU quadrangulation)
// ---------------------------------------------------------------------------

/// Quadrangulation table entry for GPU computation.
///
/// Each entry describes how to compute a quadrangulated primvar value
/// from the original primvar array.
#[derive(Debug, Clone)]
pub struct QuadrangulateTableEntry {
    /// Index into the output buffer
    pub dst_index: u32,
    /// Indices into the source primvar buffer to average
    pub src_indices: Vec<u32>,
}

/// Build the quadrangulation table for GPU primvar quadrangulation.
///
/// For each non-quad face, the center point value is the average of all
/// face vertex values. The table encodes these averaging operations.
pub fn build_quadrangulate_table(topology: &HdStMeshTopology) -> Vec<QuadrangulateTableEntry> {
    let mut table = Vec::new();
    let num_orig_points = topology.get_num_points() as u32;
    let mut center_dst = num_orig_points;
    let mut offset = 0usize;

    for (face_idx, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as usize;

        if topology.hole_indices.contains(&(face_idx as i32)) {
            offset += count;
            continue;
        }

        if count != 4 {
            // Non-quad face: compute center point as average
            let src_indices: Vec<u32> = (0..count)
                .map(|i| topology.face_vertex_indices[offset + i] as u32)
                .collect();

            table.push(QuadrangulateTableEntry {
                dst_index: center_dst,
                src_indices,
            });
            center_dst += 1;
        }

        offset += count;
    }

    table
}

// ---------------------------------------------------------------------------
// CPU quadrangulation of primvar data
// ---------------------------------------------------------------------------

/// Quadrangulate primvar data on CPU.
///
/// Takes original primvar data (one value per vertex) and produces
/// quadrangulated data (original values + new center point values).
///
/// `stride` is the number of float components per primvar element.
pub fn quadrangulate_primvar(
    topology: &HdStMeshTopology,
    primvar: &[f32],
    stride: usize,
) -> Vec<f32> {
    let quad_info = match topology.get_quad_info() {
        Some(qi) => qi,
        None => return primvar.to_vec(),
    };

    let num_orig = topology.get_num_points();
    let total_points = num_orig + quad_info.num_points;
    let mut result = vec![0.0f32; total_points * stride];

    // Copy original primvar data
    let copy_len = (num_orig * stride).min(primvar.len());
    result[..copy_len].copy_from_slice(&primvar[..copy_len]);

    // Compute center point values
    let mut center_idx = num_orig;
    let mut offset = 0usize;

    for (fi, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as usize;

        if topology.hole_indices.contains(&(fi as i32)) {
            offset += count;
            continue;
        }

        if count != 4 && count >= 3 {
            // Average all face vertices for center point
            let dst_offset = center_idx * stride;
            let inv_n = 1.0 / count as f32;

            for i in 0..count {
                let vi = topology.face_vertex_indices[offset + i] as usize;
                let src_offset = vi * stride;
                for c in 0..stride {
                    if src_offset + c < primvar.len() {
                        result[dst_offset + c] += primvar[src_offset + c] * inv_n;
                    }
                }
            }

            center_idx += 1;
        }

        offset += count;
    }

    result
}

// ---------------------------------------------------------------------------
// Face-varying quadrangulation
// ---------------------------------------------------------------------------

/// Quadrangulate face-varying data.
///
/// Face-varying data has one value per face-vertex. After quadrangulation,
/// each quad needs 4 face-varying values. Non-quad faces get their center
/// point computed as the average of face-varying values.
pub fn quadrangulate_face_varying<T: Clone + Default>(
    topology: &HdStMeshTopology,
    fvar_data: &[T],
    _avg_fn: impl Fn(&[&T]) -> T,
) -> Vec<T> {
    let mut result = Vec::new();
    let mut offset = 0usize;

    for (face_idx, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as usize;

        if topology.hole_indices.contains(&(face_idx as i32)) {
            offset += count;
            continue;
        }

        if count == 4 {
            // Quad: pass through 4 fvar values
            for i in 0..4 {
                if offset + i < fvar_data.len() {
                    result.push(fvar_data[offset + i].clone());
                }
            }
        } else if count >= 3 {
            // Non-quad: produce count quads, each with 4 fvar values
            let face_vals: Vec<&T> = (0..count)
                .filter_map(|i| fvar_data.get(offset + i))
                .collect();

            if face_vals.len() == count {
                let center = _avg_fn(&face_vals);

                for i in 0..count {
                    let prev = (i + count - 1) % count;
                    let next = (i + 1) % count;
                    // Quad: prev_vertex, current_vertex, next_vertex, center
                    result.push(face_vals[prev].clone());
                    result.push(face_vals[i].clone());
                    result.push(face_vals[next].clone());
                    result.push(center.clone());
                }
            }
        }

        offset += count;
    }

    result
}

// ---------------------------------------------------------------------------
// Utility: compute total quad count
// ---------------------------------------------------------------------------

/// Compute total number of quads after quadrangulation.
pub fn compute_quad_count(face_vertex_counts: &[i32], hole_indices: &[i32]) -> usize {
    let mut count = 0usize;
    for (fi, &nv) in face_vertex_counts.iter().enumerate() {
        if hole_indices.contains(&(fi as i32)) {
            continue;
        }
        if nv == 4 {
            count += 1;
        } else if nv >= 3 {
            count += nv as usize; // Each face becomes N quads
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quad_count() {
        assert_eq!(compute_quad_count(&[4], &[]), 1);
        assert_eq!(compute_quad_count(&[3], &[]), 3); // tri -> 3 quads
        assert_eq!(compute_quad_count(&[5], &[]), 5); // pent -> 5 quads
        assert_eq!(compute_quad_count(&[4, 3], &[1]), 1); // tri is hole
    }

    #[test]
    fn test_build_quad_indices_passthrough() {
        // Pure quad mesh: should pass through
        let mut topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        topo.compute_quad_info();

        let result = build_quad_indices(&topo);
        assert_eq!(result.num_quads, 1);
        assert_eq!(result.indices.len(), 4);
        assert_eq!(result.primitive_params, vec![0]);
    }

    #[test]
    fn test_build_quadrangulate_table() {
        let mut topo = HdStMeshTopology::from_faces(
            vec![4, 3],
            vec![0, 1, 2, 3, 4, 5, 6],
        );
        topo.compute_quad_info();

        let table = build_quadrangulate_table(&topo);
        // Only the triangle needs a center point
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].src_indices.len(), 3); // 3 verts averaged
    }

    #[test]
    fn test_quadrangulate_primvar() {
        let mut topo = HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]);
        topo.compute_quad_info();

        // 3 vertices, stride=1 (scalar primvar)
        let primvar = vec![0.0, 3.0, 6.0];
        let result = quadrangulate_primvar(&topo, &primvar, 1);

        // 3 original + 1 center point = 4
        assert_eq!(result.len(), 4);
        // Center should be average: (0+3+6)/3 = 3.0
        assert!((result[3] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_quadrangulate_primvar_vec3() {
        let mut topo = HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]);
        topo.compute_quad_info();

        // 3 vertices, stride=3 (position)
        let primvar = vec![
            0.0, 0.0, 0.0,
            3.0, 0.0, 0.0,
            0.0, 3.0, 0.0,
        ];
        let result = quadrangulate_primvar(&topo, &primvar, 3);

        assert_eq!(result.len(), 12); // 4 verts * 3
        // Center xyz = (1.0, 1.0, 0.0)
        assert!((result[9] - 1.0).abs() < 1e-6);
        assert!((result[10] - 1.0).abs() < 1e-6);
        assert!((result[11] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_quad_edge_indices() {
        // Pure quad: all edges real
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        let edges = compute_quad_edge_indices(&topo);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0], 0b1111);
    }

    #[test]
    fn test_face_varying_quad_passthrough() {
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);

        let fvar: Vec<f32> = vec![0.0, 1.0, 2.0, 3.0];
        let result = quadrangulate_face_varying(&topo, &fvar, |vals| {
            let sum: f32 = vals.iter().map(|&&v| v).sum();
            sum / vals.len() as f32
        });

        assert_eq!(result.len(), 4);
        assert_eq!(result, vec![0.0, 1.0, 2.0, 3.0]);
    }
}
