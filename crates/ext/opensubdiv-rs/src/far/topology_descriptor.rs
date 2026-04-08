// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/topologyDescriptor.h + topologyDescriptor.cpp

use super::types::Index;

// ---------------------------------------------------------------------------
// FVarChannel
// ---------------------------------------------------------------------------

/// Per-channel face-varying data.
/// Mirrors C++ `Far::TopologyDescriptor::FVarChannel`.
#[derive(Debug, Default, Clone)]
pub struct FVarChannel {
    /// Number of distinct face-varying values in this channel.
    pub num_values: i32,
    /// Per-face-corner value indices (one per face vertex, same order as face-verts).
    pub value_indices: Vec<Index>,
}

// ---------------------------------------------------------------------------
// TopologyDescriptor
// ---------------------------------------------------------------------------

/// A simple reference to raw topology data for use with `TopologyRefinerFactory`.
///
/// This is a convenience struct when mesh topology is not available in an
/// existing data structure.  Mirrors C++ `Far::TopologyDescriptor`.
#[derive(Debug, Default, Clone)]
pub struct TopologyDescriptor {
    /// Total number of vertices.
    pub num_vertices: i32,
    /// Total number of faces.
    pub num_faces: i32,

    /// Number of vertices per face (one entry per face).
    pub num_verts_per_face: Vec<i32>,
    /// Flat list of per-face vertex indices in face order.
    pub vert_indices_per_face: Vec<Index>,

    // ---- crease data ----
    /// Number of creased edges.
    pub num_creases: i32,
    /// Pairs of vertex indices (2 * num_creases entries): each pair defines an edge.
    pub crease_vertex_index_pairs: Vec<Index>,
    /// Per-crease sharpness values (num_creases entries).
    pub crease_weights: Vec<f32>,

    // ---- corner data ----
    /// Number of sharp corner vertices.
    pub num_corners: i32,
    /// Vertex indices of corners (num_corners entries).
    pub corner_vertex_indices: Vec<Index>,
    /// Per-corner sharpness values (num_corners entries).
    pub corner_weights: Vec<f32>,

    // ---- holes ----
    /// Indices of faces tagged as holes.
    pub hole_indices: Vec<Index>,

    // ---- winding ----
    /// If true, face vertex lists are in left-handed (CW) winding order;
    /// the factory will reverse them to the expected CCW convention.
    pub is_left_handed: bool,

    // ---- face-varying channels ----
    /// One entry per face-varying channel.
    pub fvar_channels: Vec<FVarChannel>,
}

impl TopologyDescriptor {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_zeroed() {
        let d = TopologyDescriptor::default();
        assert_eq!(d.num_vertices, 0);
        assert_eq!(d.num_faces, 0);
        assert!(d.num_verts_per_face.is_empty());
        assert!(!d.is_left_handed);
    }

    #[test]
    fn test_fvar_channel_default() {
        let c = FVarChannel::default();
        assert_eq!(c.num_values, 0);
        assert!(c.value_indices.is_empty());
    }
}
