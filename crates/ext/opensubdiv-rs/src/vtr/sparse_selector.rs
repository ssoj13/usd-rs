// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/sparseSelector.h/.cpp

use super::types::Index;
use super::refinement::Refinement;

/// Manages component selection for sparse (feature-adaptive) refinement.
///
/// Wraps a `Refinement` and marks parent components as selected.
/// Mirrors C++ `Vtr::internal::SparseSelector`.
pub struct SparseSelector<'r> {
    refinement: &'r mut Refinement,
    selected:   bool,
}

impl<'r> SparseSelector<'r> {
    pub fn new(refinement: &'r mut Refinement) -> Self {
        Self { refinement, selected: false }
    }

    /// Rebind to a new Refinement, resetting selection state.
    /// Mirrors C++ `SparseSelector::setRefinement(Refinement& refine)`.
    pub fn set_refinement(&mut self, refinement: &'r mut Refinement) {
        self.refinement = refinement;
        self.selected   = false;
    }

    pub fn get_refinement(&self) -> &Refinement { self.refinement }
    pub fn get_refinement_mut(&mut self) -> &mut Refinement { self.refinement }

    /// Returns `true` if no components have been selected yet.
    pub fn is_selection_empty(&self) -> bool { !self.selected }

    // ---- selection queries ----
    pub fn was_vertex_selected(&self, v: Index) -> bool {
        self.refinement.get_parent_vertex_sparse_tag(v).selected
    }
    pub fn was_edge_selected(&self, e: Index) -> bool {
        self.refinement.get_parent_edge_sparse_tag(e).selected
    }
    pub fn was_face_selected(&self, f: Index) -> bool {
        self.refinement.get_parent_face_sparse_tag(f).selected
    }

    /// Allocate sparse selection tags on first use (C++ initializeSelection).
    fn initialize_selection(&mut self) {
        if !self.selected {
            self.refinement.initialize_sparse_selection_tags();
            self.selected = true;
        }
    }

    // ---- mark helpers (no cascading, no guards) ----

    fn mark_vertex_selected(&mut self, v: Index) {
        self.refinement.get_parent_vertex_sparse_tag_mut(v).selected = true;
    }
    fn mark_edge_selected(&mut self, e: Index) {
        self.refinement.get_parent_edge_sparse_tag_mut(e).selected = true;
    }
    fn mark_face_selected(&mut self, f: Index) {
        self.refinement.get_parent_face_sparse_tag_mut(f).selected = true;
    }

    // ---- selection methods (match C++ exactly) ----

    /// Select a vertex — just mark it, no cascade (P0-1 fix).
    pub fn select_vertex(&mut self, v: Index) {
        self.initialize_selection();
        // C++: "Don't bother to test-and-set here, just set"
        self.mark_vertex_selected(v);
    }

    /// Select an edge + its two endpoint vertices (P0-2 fix).
    pub fn select_edge(&mut self, e: Index) {
        self.initialize_selection();
        if !self.was_edge_selected(e) {
            self.mark_edge_selected(e);
            // Mark endpoint vertices
            let verts: Vec<Index> = self.refinement.parent()
                .get_edge_vertices(e).as_slice().to_vec();
            self.mark_vertex_selected(verts[0]);
            self.mark_vertex_selected(verts[1]);
        }
    }

    /// Select a face + all incident edges and vertices (P0-3 fix).
    pub fn select_face(&mut self, f: Index) {
        self.initialize_selection();
        if !self.was_face_selected(f) {
            self.mark_face_selected(f);
            // Direct marking of incident edges and vertices — no recursion
            let face_edges: Vec<Index> = self.refinement.parent()
                .get_face_edges(f).as_slice().to_vec();
            let face_verts: Vec<Index> = self.refinement.parent()
                .get_face_vertices(f).as_slice().to_vec();
            for i in 0..face_verts.len() {
                self.mark_edge_selected(face_edges[i]);
                self.mark_vertex_selected(face_verts[i]);
            }
        }
    }
}
