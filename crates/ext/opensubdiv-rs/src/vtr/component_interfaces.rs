// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/componentInterfaces.h

//! Component interface adapters for Scheme mask queries.
//!
//! Provides lightweight wrappers around Level topology that satisfy the generic
//! `FACE`, `EDGE`, `VERTEX` type requirements of Sdc::Scheme mask computation.

use crate::sdc::crease::Crease;
use super::level::Level;
use super::types::Index;

// ---------------------------------------------------------------------------
// CountOffset — utility struct for variable-arity topology relations
// ---------------------------------------------------------------------------

/// A (count, offset) pair used for variable-arity topology relations.
#[derive(Clone, Copy, Default)]
pub struct CountOffset {
    pub count:  i32,
    pub offset: i32,
}

impl CountOffset {
    pub fn new(count: i32, offset: i32) -> Self { Self { count, offset } }
}

// ---------------------------------------------------------------------------
// FaceInterface — neighborhood info around a face
// ---------------------------------------------------------------------------

/// Lightweight face-topology adapter for Scheme mask queries.
/// Mirrors C++ `Vtr::internal::FaceInterface`.
#[derive(Clone, Copy)]
pub struct FaceInterface {
    vert_count: i32,
}

impl FaceInterface {
    pub fn new(vert_count: i32) -> Self {
        Self { vert_count }
    }

    /// Number of vertices in this face (generic interface for `<typename FACE>`).
    #[inline]
    pub fn get_num_vertices(&self) -> i32 {
        self.vert_count
    }
}

impl Default for FaceInterface {
    fn default() -> Self { Self { vert_count: 0 } }
}

// ---------------------------------------------------------------------------
// EdgeInterface — neighborhood info around an edge
// ---------------------------------------------------------------------------

/// Lightweight edge-topology adapter for Scheme mask queries.
/// Mirrors C++ `Vtr::internal::EdgeInterface`.
///
/// Provides the generic `<typename EDGE>` interface expected by Sdc::Scheme.
pub struct EdgeInterface<'a> {
    level: &'a Level,
    e_index: Index,
}

impl<'a> EdgeInterface<'a> {
    pub fn new(level: &'a Level) -> Self {
        Self { level, e_index: 0 }
    }

    /// Set the edge index for subsequent queries.
    pub fn set_index(&mut self, edge_index: Index) {
        self.e_index = edge_index;
    }

    // -- Generic interface expected of <typename EDGE> --

    /// Number of faces incident to this edge.
    #[inline]
    pub fn get_num_faces(&self) -> i32 {
        self.level.get_edge_faces(self.e_index).size()
    }

    /// Sharpness of this edge.
    #[inline]
    pub fn get_sharpness(&self) -> f32 {
        self.level.get_edge_sharpness(self.e_index)
    }

    /// Compute child sharpnesses for the two sub-edges after subdivision.
    ///
    /// P2 approximation: uniform `sharpness - 1.0` for both child edges.
    /// C++ carries the same stub with comment "Need to use the Refinement here
    /// to identify the two child edges" (componentInterfaces.h).  Full
    /// implementation would call `Crease::SubdivideEdgeSharpnessAtVertex` with
    /// the surrounding edge sharpnesses gathered from the Refinement, which
    /// requires access to topology not available at this interface level.
    pub fn get_child_sharpnesses(&self, _crease: &Crease, s: &mut [f32; 2]) {
        let sharp = self.get_sharpness() - 1.0;
        s[0] = sharp;
        s[1] = sharp;
    }

    /// Fill `verts_per_face` with vertex counts for each face incident to this edge.
    pub fn get_num_vertices_per_face(&self, verts_per_face: &mut [i32]) {
        let e_faces = self.level.get_edge_faces(self.e_index);
        for i in 0..e_faces.size() as usize {
            verts_per_face[i] = self.level.get_face_vertices(e_faces[i]).size();
        }
    }
}

// ---------------------------------------------------------------------------
// VertexInterface — neighborhood info around a vertex
// ---------------------------------------------------------------------------

/// Lightweight vertex-topology adapter for Scheme mask queries.
/// Mirrors C++ `Vtr::internal::VertexInterface`.
///
/// Provides the generic `<typename VERT>` interface expected by Sdc::Scheme.
pub struct VertexInterface<'a> {
    parent: &'a Level,
    child:  &'a Level,
    p_index: Index,
    c_index: Index,
    e_count: i32,
    f_count: i32,
}

impl<'a> VertexInterface<'a> {
    pub fn new(parent: &'a Level, child: &'a Level) -> Self {
        Self {
            parent, child,
            p_index: 0, c_index: 0,
            e_count: 0, f_count: 0,
        }
    }

    /// Set the parent and child vertex indices, computing edge/face counts.
    pub fn set_index(&mut self, parent_index: Index, child_index: Index) {
        self.p_index = parent_index;
        self.c_index = child_index;
        self.e_count = self.parent.get_vertex_edges(self.p_index).size();
        self.f_count = self.parent.get_vertex_faces(self.p_index).size();
    }

    // -- Generic interface expected of <typename VERT> --

    /// Number of edges incident to this vertex.
    #[inline]
    pub fn get_num_edges(&self) -> i32 { self.e_count }

    /// Number of faces incident to this vertex.
    #[inline]
    pub fn get_num_faces(&self) -> i32 { self.f_count }

    /// Sharpness of this vertex in the parent level.
    #[inline]
    pub fn get_sharpness(&self) -> f32 {
        self.parent.get_vertex_sharpness(self.p_index)
    }

    /// Fill `p_sharpness` with sharpness of each edge incident to this vertex.
    /// Returns the slice passed in (for chaining).
    pub fn get_sharpness_per_edge<'b>(&self, p_sharpness: &'b mut [f32]) -> &'b [f32] {
        let p_edges = self.parent.get_vertex_edges(self.p_index);
        for i in 0..self.e_count as usize {
            p_sharpness[i] = self.parent.get_edge_sharpness(p_edges[i]);
        }
        p_sharpness
    }

    /// Child vertex sharpness after one subdivision step.
    #[inline]
    pub fn get_child_sharpness(&self, _crease: &Crease) -> f32 {
        self.child.get_vertex_sharpness(self.c_index)
    }

    /// Compute child edge sharpnesses around this vertex using the Crease rules.
    pub fn get_child_sharpness_per_edge<'b>(
        &self,
        crease: &Crease,
        c_sharpness: &'b mut [f32],
    ) -> &'b [f32] {
        let mut p_sharpness = vec![0.0f32; self.e_count as usize];
        self.get_sharpness_per_edge(&mut p_sharpness);
        crease.subdivide_edge_sharpnesses_around_vertex(
            &p_sharpness,
            c_sharpness,
        );
        c_sharpness
    }

    /// Fill `verts_per_face` with vertex counts for each face around this vertex.
    pub fn get_num_vertices_per_face(&self, verts_per_face: &mut [i32]) {
        let p_faces = self.parent.get_vertex_faces(self.p_index);
        for i in 0..self.f_count as usize {
            verts_per_face[i] = self.parent.get_face_vertices(p_faces[i]).size();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_offset_basics() {
        let co = CountOffset::new(4, 12);
        assert_eq!(co.count, 4);
        assert_eq!(co.offset, 12);

        let co2 = CountOffset::default();
        assert_eq!(co2.count, 0);
        assert_eq!(co2.offset, 0);
    }

    #[test]
    fn face_interface_basics() {
        let f = FaceInterface::new(4);
        assert_eq!(f.get_num_vertices(), 4);

        let f_tri = FaceInterface::new(3);
        assert_eq!(f_tri.get_num_vertices(), 3);

        let f_default = FaceInterface::default();
        assert_eq!(f_default.get_num_vertices(), 0);
    }

    #[test]
    fn edge_interface_on_simple_level() {
        // Build a minimal Level with 1 quad face (4 verts, 4 edges)
        let mut level = Level::new();
        level.vert_count = 4;
        level.edge_count = 4;
        level.face_count = 1;

        // face 0 has 4 verts
        level.face_vert_counts_offsets = vec![4, 0];
        level.face_vert_indices = vec![0, 1, 2, 3];

        // edge 0: verts 0-1, incident to face 0
        level.edge_vert_indices = vec![0,1, 1,2, 2,3, 3,0];
        level.edge_sharpness = vec![0.0, 2.5, 0.0, 0.0];
        level.edge_face_counts_offsets = vec![
            1, 0,  // edge 0
            1, 1,  // edge 1
            1, 2,  // edge 2
            1, 3,  // edge 3
        ];
        level.edge_face_indices = vec![0, 0, 0, 0];
        level.edge_face_local_indices = vec![0, 1, 2, 3];

        let mut ei = EdgeInterface::new(&level);

        ei.set_index(1);
        assert_eq!(ei.get_num_faces(), 1);
        assert_eq!(ei.get_sharpness(), 2.5);

        let mut child_sharp = [0.0f32; 2];
        let crease = Crease::with_options(Default::default());
        ei.get_child_sharpnesses(&crease, &mut child_sharp);
        assert!((child_sharp[0] - 1.5).abs() < 1e-6);

        let mut vpf = [0i32; 1];
        ei.get_num_vertices_per_face(&mut vpf);
        assert_eq!(vpf[0], 4);
    }
}
