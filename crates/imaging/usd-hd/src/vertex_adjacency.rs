//! Mesh vertex adjacency for smooth normal computation.
//!
//! Encapsulates mesh adjacency information used for smooth normal computation.
//! The adjacency table provides prev/next vertex indices for each face that uses
//! each vertex. See pxr/imaging/hd/vertexAdjacency.h for C++ reference.

use super::flat_normals::MeshTopologyView;

const RIGHT_HANDED: &str = "rightHanded";

/// Vertex adjacency for a mesh.
///
/// The adjacency table format:
/// - First `num_points * 2` entries: for each vertex i, [offset, valence]
///   where offset points to prev/next pairs in the second part
/// - Remaining entries: prev/next pairs (2 ints per pair) per vertex per face
#[derive(Debug, Clone, Default)]
pub struct HdVertexAdjacency {
    num_points: usize,
    adjacency_table: Vec<i32>,
}

impl HdVertexAdjacency {
    /// Create empty adjacency.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build adjacency table from mesh topology.
    pub fn build_adjacency_table<T: MeshTopologyView>(&mut self, topology: &T) {
        let face_counts = topology.face_vertex_counts();
        let face_indices = topology.face_vertex_indices();
        let num_faces = face_counts.len();
        let flip = topology.orientation() != RIGHT_HANDED;

        if num_faces == 0 || face_indices.is_empty() {
            self.num_points = 0;
            self.adjacency_table.clear();
            return;
        }

        // Compute num_points from max index in face vertex indices
        self.num_points = Self::compute_num_points(face_indices);

        if self.num_points == 0 {
            self.adjacency_table.clear();
            return;
        }

        // Compute valence per vertex
        let mut vertex_valence: Vec<usize> = vec![0; self.num_points];
        let mut vert_index = 0usize;

        for i in 0..num_faces {
            let nv = face_counts[i] as usize;
            for _ in 0..nv {
                let idx_i32 = face_indices[vert_index];
                vert_index += 1;
                if idx_i32 < 0 {
                    self.num_points = 0;
                    self.adjacency_table.clear();
                    return;
                }
                let idx = idx_i32 as usize;
                if idx >= self.num_points {
                    self.num_points = 0;
                    self.adjacency_table.clear();
                    return;
                }
                vertex_valence[idx] += 1;
            }
        }

        // Total entries: 2 per point (offset, valence) + 2 per vertex per face
        let num_entries = self.num_points * 2 + vertex_valence.iter().sum::<usize>() * 2;

        self.adjacency_table.clear();
        self.adjacency_table.resize(num_entries, 0);

        // Fill offsets (first part)
        let mut current_offset = (self.num_points * 2) as i32;
        for point_num in 0..self.num_points {
            self.adjacency_table[point_num * 2] = current_offset;
            current_offset += 2 * vertex_valence[point_num] as i32;
        }

        // Fill prev/next pairs
        let mut vertex_count: Vec<usize> = vec![0; self.num_points];
        vert_index = 0;

        for i in 0..num_faces {
            let nv = face_counts[i] as usize;
            for j in 0..nv {
                let j_prev = (j + nv - 1) % nv;
                let j_next = (j + 1) % nv;
                let mut prev = face_indices[vert_index + j_prev] as usize;
                let curr = face_indices[vert_index + j] as usize;
                let mut next = face_indices[vert_index + j_next] as usize;
                if flip {
                    std::mem::swap(&mut prev, &mut next);
                }

                let entry_offset = self.adjacency_table[curr * 2] as usize;
                let count = &mut vertex_count[curr];
                let pair_offset = entry_offset + *count * 2;
                *count += 1;

                self.adjacency_table[pair_offset] = prev as i32;
                self.adjacency_table[pair_offset + 1] = next as i32;
            }
            vert_index += nv;
        }

        // Store valence in second slot of each vertex's header
        for point_num in 0..self.num_points {
            self.adjacency_table[point_num * 2 + 1] = vertex_valence[point_num] as i32;
        }
    }

    /// Number of points in the adjacency table.
    pub fn num_points(&self) -> usize {
        self.num_points
    }

    /// The adjacency table (see struct doc for format).
    pub fn adjacency_table(&self) -> &[i32] {
        &self.adjacency_table
    }

    /// Compute number of points from vertex indices (max index + 1).
    pub fn compute_num_points(face_vertex_indices: &[i32]) -> usize {
        let mut max_idx: i32 = -1;
        for &idx in face_vertex_indices {
            if idx > max_idx {
                max_idx = idx;
            }
        }
        (max_idx + 1) as usize
    }
}
