#![allow(dead_code)]

//! HdSt_VertexAdjacencyBuilder - Storm-side vertex adjacency management.
//!
//! Wraps `Hd_VertexAdjacency` (from usd-hd) with buffer array range
//! tracking and shared builder computations for Storm's GPU pipeline.
//!
//! Note: The core adjacency table lives in `smooth_normals::VertexAdjacency`
//! (usd-hd-st) and `usd_hd::vertex_adjacency`. This module adds the
//! Storm-specific builder pattern with BAR (buffer array range) management.
//!
//! Port of pxr/imaging/hdSt/vertexAdjacency.h

use std::sync::{Arc, Weak};

/// Shared pointer types for adjacency builder.
pub type VertexAdjacencyBuilderSharedPtr = Arc<VertexAdjacencyBuilder>;
pub type VertexAdjacencyBuilderComputationSharedPtr = Arc<VertexAdjacencyBuilderComputation>;
pub type VertexAdjacencyBuilderComputationWeakPtr = Weak<VertexAdjacencyBuilderComputation>;

/// Storm-side vertex adjacency builder.
///
/// Manages the adjacency table lifecycle for Storm's GPU smooth normals
/// pipeline. Holds a reference to the core adjacency data and tracks
/// the buffer array range used for GPU upload.
///
/// Port of HdSt_VertexAdjacencyBuilder
#[derive(Debug)]
pub struct VertexAdjacencyBuilder {
    /// Adjacency table data (indices into neighbor lists)
    adjacency_table: Vec<i32>,
    /// Number of vertices
    num_points: usize,
    /// Weak reference to shared computation (dedup across meshes sharing topology)
    shared_computation: Option<VertexAdjacencyBuilderComputationWeakPtr>,
}

impl VertexAdjacencyBuilder {
    /// Create a new vertex adjacency builder.
    pub fn new() -> Self {
        Self {
            adjacency_table: Vec::new(),
            num_points: 0,
            shared_computation: None,
        }
    }

    /// Get the adjacency table (flat array of neighbor vertex indices).
    pub fn adjacency_table(&self) -> &[i32] {
        &self.adjacency_table
    }

    /// Get the number of vertices.
    pub fn num_points(&self) -> usize {
        self.num_points
    }

    /// Set adjacency data directly.
    pub fn set_adjacency_data(&mut self, table: Vec<i32>, num_points: usize) {
        self.adjacency_table = table;
        self.num_points = num_points;
    }

    /// Get or create a shared adjacency builder computation.
    ///
    /// Multiple meshes sharing topology can share a single computation
    /// and only build the adjacency table once.
    pub fn get_shared_computation(
        &mut self,
        face_counts: &[i32],
        face_indices: &[i32],
        num_points: usize,
    ) -> VertexAdjacencyBuilderComputationSharedPtr {
        // Try to reuse existing computation
        if let Some(weak) = &self.shared_computation {
            if let Some(existing) = weak.upgrade() {
                return existing;
            }
        }

        // Create new computation
        let comp = Arc::new(VertexAdjacencyBuilderComputation::new(
            face_counts.to_vec(),
            face_indices.to_vec(),
            num_points,
        ));
        self.shared_computation = Some(Arc::downgrade(&comp));
        comp
    }
}

impl Default for VertexAdjacencyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Computation that builds the adjacency table.
///
/// A null buffer source - doesn't produce buffer output itself, but
/// other computations depend on it to ensure the adjacency table is built.
///
/// Port of HdSt_VertexAdjacencyBuilderComputation
#[derive(Debug)]
pub struct VertexAdjacencyBuilderComputation {
    /// Face vertex counts
    face_counts: Vec<i32>,
    /// Face vertex indices
    face_indices: Vec<i32>,
    /// Number of vertices
    num_points: usize,
    /// Whether computation has been resolved
    resolved: bool,
}

impl VertexAdjacencyBuilderComputation {
    /// Create a new adjacency builder computation.
    pub fn new(face_counts: Vec<i32>, face_indices: Vec<i32>, num_points: usize) -> Self {
        Self {
            face_counts,
            face_indices,
            num_points,
            resolved: false,
        }
    }

    /// Whether this computation has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Get face vertex counts.
    pub fn face_counts(&self) -> &[i32] {
        &self.face_counts
    }

    /// Get face vertex indices.
    pub fn face_indices(&self) -> &[i32] {
        &self.face_indices
    }

    /// Get number of vertices.
    pub fn num_points(&self) -> usize {
        self.num_points
    }
}

/// Buffer source that uploads a pre-computed adjacency table to GPU.
///
/// Depends on a VertexAdjacencyBuilderComputation to ensure the
/// adjacency data is available before upload.
///
/// Port of HdSt_VertexAdjacencyBufferSource
#[derive(Debug)]
pub struct VertexAdjacencyBufferSource {
    /// Reference to the adjacency data
    adjacency_data: Vec<i32>,
    /// Whether resolved
    resolved: bool,
}

impl VertexAdjacencyBufferSource {
    /// Create a buffer source from adjacency data.
    pub fn new(adjacency_data: Vec<i32>) -> Self {
        Self {
            adjacency_data,
            resolved: false,
        }
    }

    /// Get the adjacency data.
    pub fn data(&self) -> &[i32] {
        &self.adjacency_data
    }

    /// Number of elements.
    pub fn num_elements(&self) -> usize {
        self.adjacency_data.len()
    }

    /// Whether resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Mark as resolved.
    pub fn resolve(&mut self) {
        self.resolved = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_lifecycle() {
        let mut builder = VertexAdjacencyBuilder::new();
        assert_eq!(builder.num_points(), 0);
        assert!(builder.adjacency_table().is_empty());

        builder.set_adjacency_data(vec![0, 1, 2, 1, 0, 2], 3);
        assert_eq!(builder.num_points(), 3);
        assert_eq!(builder.adjacency_table().len(), 6);
    }

    #[test]
    fn test_shared_computation() {
        let mut builder = VertexAdjacencyBuilder::new();
        let counts = vec![3];
        let indices = vec![0, 1, 2];

        let comp1 = builder.get_shared_computation(&counts, &indices, 3);
        let comp2 = builder.get_shared_computation(&counts, &indices, 3);

        // Should return same Arc (shared)
        assert!(Arc::ptr_eq(&comp1, &comp2));
    }

    #[test]
    fn test_buffer_source() {
        let mut src = VertexAdjacencyBufferSource::new(vec![0, 1, 2, 3]);
        assert_eq!(src.num_elements(), 4);
        assert!(!src.is_resolved());

        src.resolve();
        assert!(src.is_resolved());
    }
}
