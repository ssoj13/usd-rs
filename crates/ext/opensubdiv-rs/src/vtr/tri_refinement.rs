// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/triRefinement.h/.cpp

use crate::sdc::{Options, types::Split};
use super::level::Level;
use super::types::{Index, LocalIndex, index_is_valid};
use super::refinement::{Refinement, RefinementOptions};

/// Tri-splitting (Loop) refinement.
/// Mirrors C++ `Vtr::internal::TriRefinement`.
pub struct TriRefinement(pub Refinement);

impl TriRefinement {
    /// Create a tri refinement between `parent` and `child` levels.
    ///
    /// # Safety
    /// `parent` and `child` must outlive this struct.
    pub unsafe fn new(parent: *const Level, child: *mut Level, options: Options) -> Self {
        let mut r = unsafe { Refinement::new(parent, child, options, Split::ToTris) };
        // Wire all scheme-specific callbacks so that a bare Box<Refinement>
        // (as used in refine_adaptive) can call refine() correctly.
        r.allocate_fn    = Some(TriRefinement::allocate_parent_child_indices_impl);
        r.sparse_face_fn = Some(TriRefinement::mark_sparse_face_children_impl);
        r.populate_fv_fn = Some(TriRefinement::populate_face_vertex_relation_impl);
        r.populate_fe_fn = Some(TriRefinement::populate_face_edge_relation_impl);
        r.populate_ev_fn = Some(TriRefinement::populate_edge_vertex_relation_impl);
        r.populate_ef_fn = Some(TriRefinement::populate_edge_face_relation_impl);
        r.populate_vf_fn = Some(TriRefinement::populate_vertex_face_relation_impl);
        r.populate_ve_fn = Some(TriRefinement::populate_vertex_edge_relation_impl);
        Self(r)
    }

    pub fn refine(&mut self, opts: RefinementOptions) {
        // Wire up virtual-dispatch overrides before calling the base refine().
        self.0.refine_with_callbacks(opts,
            TriRefinement::allocate_parent_child_indices_impl,
            TriRefinement::mark_sparse_face_children_impl,
            TriRefinement::populate_face_vertex_relation_impl,
            TriRefinement::populate_face_edge_relation_impl,
            TriRefinement::populate_edge_vertex_relation_impl,
            TriRefinement::populate_edge_face_relation_impl,
            TriRefinement::populate_vertex_face_relation_impl,
            TriRefinement::populate_vertex_edge_relation_impl,
        );
    }

    pub fn inner(&self) -> &Refinement { &self.0 }
    pub fn inner_mut(&mut self) -> &mut Refinement { &mut self.0 }

    // =========================================================================
    // allocateParentChildIndices  (C++ TriRefinement::allocateParentChildIndices)
    //
    // For tri-split (Loop), every triangular face produces exactly 4 child faces.
    // Each parent edge produces 2 child edges.
    // No face-child vertices (until N-gon support is added).
    // =========================================================================

    fn allocate_parent_child_indices_impl(r: &mut Refinement) {
        let (num_faces, fe_indices_len, ev_indices_len, e_vert_count, v_vert_count) = unsafe {
            let p = &*r.parent;
            (
                p.get_num_faces(),
                p.face_edge_indices.len(),   // used for face-child-edge count
                p.edge_vert_indices.len(),   // used for edge-child-edge count
                p.get_num_edges(),
                p.get_num_vertices(),
            )
        };

        // face-child-faces: every tri face → 4 child faces (fixed count=4, offset=4*i)
        // We use a local vector, not the shared parent face-vert c/o.
        r.local_face_child_face_counts_offsets.resize(num_faces as usize * 2, 4);
        for i in 0..(num_faces as usize) {
            r.local_face_child_face_counts_offsets[i * 2 + 1] = (4 * i) as i32;
        }
        r.face_child_face_counts_offsets_shared = false; // we own local_face_child_face_counts_offsets

        // face-child-edges: share parent face-edge counts/offsets (one per face-vertex)
        r.face_child_edge_counts_offsets_shared = true;

        let face_child_face_count = num_faces as usize * 4;
        // face-child-edge count = parent face_edge_indices.len() (one child-edge per parent face-edge)
        let face_child_edge_count = fe_indices_len;
        // edge-child-edge count = parent edge_vert_indices.len() (2 per edge)
        let edge_child_edge_count = ev_indices_len;

        // No face-child-vertices for Loop (until N-gon support)
        let face_child_vert_count = 0usize;
        let edge_child_vert_count = unsafe { (&*r.parent).get_num_edges() as usize };
        let vert_child_vert_count = unsafe { (&*r.parent).get_num_vertices() as usize };

        r.face_child_face_indices.resize(face_child_face_count, 0);
        r.face_child_edge_indices.resize(face_child_edge_count, 0);
        r.edge_child_edge_indices.resize(edge_child_edge_count, 0);

        r.face_child_vert_index.resize(face_child_vert_count, 0);
        r.edge_child_vert_index.resize(edge_child_vert_count, 0);
        r.vert_child_vert_index.resize(vert_child_vert_count, 0);

        // suppress unused-variable warnings
        let _ = (e_vert_count, v_vert_count);
    }

    // =========================================================================
    // markSparseFaceChildren  (C++ TriRefinement::markSparseFaceChildren)
    //
    // For each parent face: if selected, mark all 4 child faces + 3 child edges.
    // Otherwise, check which corner vertices are selected and mark the corresponding
    // child faces and interior edges.  Include the middle child face when transitional.
    // =========================================================================

    fn mark_sparse_face_children_impl(r: &mut Refinement) {
        debug_assert!(!r.parent_face_tag.is_empty());

        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            // Collect local data before any mutable borrows
            let f_child_faces: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
            let f_child_edges: Vec<Index> = r.get_face_child_edges(p_face).to_vec();

            debug_assert_eq!(f_child_faces.len(), 4);
            debug_assert_eq!(f_child_edges.len(), 3);

            let f_verts: Vec<Index> = unsafe {
                (&*r.parent).get_face_vertices(p_face).as_slice().to_vec()
            };

            let is_selected = r.parent_face_tag[p_face as usize].selected;

            if is_selected {
                // Mark all 4 child faces, 3 child edges as selected
                for i in 0..4usize {
                    let cf = f_child_faces[i];
                    if index_is_valid(cf) {
                        r.face_child_face_indices[cf as usize] =
                            super::refinement::SPARSE_MASK_SELECTED;
                    }
                }
                for i in 0..3usize {
                    let ce = f_child_edges[i];
                    if index_is_valid(ce) {
                        r.face_child_edge_indices[ce as usize] =
                            super::refinement::SPARSE_MASK_SELECTED;
                    }
                }
                r.parent_face_tag[p_face as usize].transitional = 0;
            } else {
                // Count how many corner verts are selected
                let marked0 = r.parent_vertex_tag[f_verts[0] as usize].selected;
                let marked1 = r.parent_vertex_tag[f_verts[1] as usize].selected;
                let marked2 = r.parent_vertex_tag[f_verts[2] as usize].selected;
                let marked = marked0 || marked1 || marked2;

                if marked {
                    // Check transitional edges
                    let f_edges: Vec<Index> = unsafe {
                        (&*r.parent).get_face_edges(p_face).as_slice().to_vec()
                    };

                    let trans: u8 =
                        ((r.parent_edge_tag[f_edges[0] as usize].transitional as u8) << 0)
                      | ((r.parent_edge_tag[f_edges[1] as usize].transitional as u8) << 1)
                      | ((r.parent_edge_tag[f_edges[2] as usize].transitional as u8) << 2);

                    r.parent_face_tag[p_face as usize].transitional = trans;

                    // If any edge is transitional, mark the interior (middle) child face
                    // and all 3 interior edges as neighboring
                    if trans != 0 {
                        let cf3 = f_child_faces[3];
                        if index_is_valid(cf3) {
                            r.face_child_face_indices[cf3 as usize] =
                                super::refinement::SPARSE_MASK_NEIGHBORING;
                        }
                        for i in 0..3usize {
                            let ce = f_child_edges[i];
                            if index_is_valid(ce) {
                                r.face_child_edge_indices[ce as usize] =
                                    super::refinement::SPARSE_MASK_NEIGHBORING;
                            }
                        }
                    }

                    // Mark per-corner child face and associated interior edge
                    // child face 0 → vertex 0, interior edge 0
                    // child face 1 → vertex 1, interior edge 1
                    // child face 2 → vertex 2, interior edge 2
                    for corner in 0..3usize {
                        let vert_selected = r.parent_vertex_tag[f_verts[corner] as usize].selected;
                        if vert_selected {
                            let cf = f_child_faces[corner];
                            if index_is_valid(cf) {
                                r.face_child_face_indices[cf as usize] =
                                    super::refinement::SPARSE_MASK_NEIGHBORING;
                            }
                            let ce = f_child_edges[corner];
                            if index_is_valid(ce) {
                                r.face_child_edge_indices[ce as usize] =
                                    super::refinement::SPARSE_MASK_NEIGHBORING;
                            }
                        }
                    }
                }
            }
        }
    }

    // =========================================================================
    // populateFaceVertexRelation  (C++ TriRefinement::populateFaceVertexRelation)
    //
    // Each parent tri → 4 child tris (3 corners + 1 interior).
    // Child face vertices:
    //   child[0] = { vert[0], eMid[0], eMid[2] }
    //   child[1] = { eMid[0], vert[1], eMid[1] }
    //   child[2] = { eMid[2], eMid[1], vert[2] }
    //   child[3] = { eMid[1], eMid[2], eMid[0] }   (interior, reversed-winding for param)
    // =========================================================================

    fn populate_face_vertex_relation_impl(r: &mut Refinement) {
        unsafe {
            let child = &mut *r.child;
            if child.face_vert_counts_offsets.is_empty() {
                Self::populate_face_vertex_counts_and_offsets(r);
            }
            let nf = (&*r.child).get_num_faces();
            (&mut *r.child).face_vert_indices.resize((nf * 3) as usize, 0);
        }
        Self::populate_face_vertices_from_parent_faces(r);
    }

    fn populate_face_vertex_counts_and_offsets(r: &mut Refinement) {
        unsafe {
            let child = &mut *r.child;
            let nf = child.get_num_faces() as usize;
            // count=3, offset=i*3
            child.face_vert_counts_offsets.resize(nf * 2, 3);
            for i in 0..nf {
                child.face_vert_counts_offsets[i * 2 + 1] = (i * 3) as i32;
            }
        }
    }

    fn populate_face_vertices_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let (p_face_verts, p_face_edges, p_face_children) = unsafe {
                let p = &*r.parent;
                let fv: Vec<Index> = p.get_face_vertices(p_face).as_slice().to_vec();
                let fe: Vec<Index> = p.get_face_edges(p_face).as_slice().to_vec();
                let fc: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                (fv, fe, fc)
            };

            debug_assert_eq!(p_face_verts.len(), 3);
            debug_assert_eq!(p_face_children.len(), 4);

            // Child vertices from the three parent edges (edge midpoints)
            let c_verts_of_edges: [Index; 3] = [
                r.edge_child_vert_index[p_face_edges[0] as usize],
                r.edge_child_vert_index[p_face_edges[1] as usize],
                r.edge_child_vert_index[p_face_edges[2] as usize],
            ];

            // Child vertices from the three parent vertices
            let c_verts_of_verts: [Index; 3] = [
                r.vert_child_vert_index[p_face_verts[0] as usize],
                r.vert_child_vert_index[p_face_verts[1] as usize],
                r.vert_child_vert_index[p_face_verts[2] as usize],
            ];

            // child[0] — corner at vertex 0
            if index_is_valid(p_face_children[0]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut cv = child.get_face_vertices_mut(p_face_children[0]);
                    cv[0] = c_verts_of_verts[0];
                    cv[1] = c_verts_of_edges[0];
                    cv[2] = c_verts_of_edges[2];
                }
            }
            // child[1] — corner at vertex 1
            if index_is_valid(p_face_children[1]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut cv = child.get_face_vertices_mut(p_face_children[1]);
                    cv[0] = c_verts_of_edges[0];
                    cv[1] = c_verts_of_verts[1];
                    cv[2] = c_verts_of_edges[1];
                }
            }
            // child[2] — corner at vertex 2
            if index_is_valid(p_face_children[2]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut cv = child.get_face_vertices_mut(p_face_children[2]);
                    cv[0] = c_verts_of_edges[2];
                    cv[1] = c_verts_of_edges[1];
                    cv[2] = c_verts_of_verts[2];
                }
            }
            // child[3] — interior (reversed orientation to preserve parametric space)
            if index_is_valid(p_face_children[3]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut cv = child.get_face_vertices_mut(p_face_children[3]);
                    cv[0] = c_verts_of_edges[1];
                    cv[1] = c_verts_of_edges[2];
                    cv[2] = c_verts_of_edges[0];
                }
            }
        }
    }

    // =========================================================================
    // populateFaceEdgeRelation  (C++ TriRefinement::populateFaceEdgeRelation)
    //
    // Each parent tri → 4 child tris, each with 3 edges.
    // Interior child edges (face-child-edges) connect midpoints.
    // Boundary child edges come from splitting parent boundary edges.
    // =========================================================================

    fn populate_face_edge_relation_impl(r: &mut Refinement) {
        unsafe {
            let child = &mut *r.child;
            if child.face_vert_counts_offsets.is_empty() {
                Self::populate_face_vertex_counts_and_offsets(r);
            }
            let nf = (&*r.child).get_num_faces();
            (&mut *r.child).face_edge_indices.resize((nf * 3) as usize, 0);
        }
        Self::populate_face_edges_from_parent_faces(r);
    }

    fn populate_face_edges_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let (p_face_verts, p_face_edges, p_face_child_faces, p_face_child_edges) = unsafe {
                let p = &*r.parent;
                let fv: Vec<Index> = p.get_face_vertices(p_face).as_slice().to_vec();
                let fe: Vec<Index> = p.get_face_edges(p_face).as_slice().to_vec();
                let fc: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                let fce: Vec<Index> = r.get_face_child_edges(p_face).to_vec();
                (fv, fe, fc, fce)
            };

            debug_assert_eq!(p_face_child_faces.len(), 4);
            debug_assert_eq!(p_face_child_edges.len(), 3);

            // For each of the 3 boundary parent edges, compute which child edge
            // is "leading" (touches the corner vertex) and which is "trailing".
            // pEdgeChildEdges[i][0] = leading, pEdgeChildEdges[i][1] = trailing
            let mut p_edge_child_edges: [[Index; 2]; 3] = [[0; 2]; 3];
            for i in 0..3usize {
                let p_edge = p_face_edges[i];
                let c_edges = *r.get_edge_child_edges(p_edge);

                let (p_ev0, p_ev1) = unsafe {
                    let p = &*r.parent;
                    let ev = p.get_edge_vertices(p_edge);
                    (ev[0], ev[1])
                };

                // Degenerate edge → no reversal concern; treat as reversed
                let edge_reversed_wrt_face = (p_ev0 != p_ev1)
                    && (p_face_verts[i] != p_ev0);

                p_edge_child_edges[i][0] = c_edges[edge_reversed_wrt_face as usize];
                p_edge_child_edges[i][1] = c_edges[!edge_reversed_wrt_face as usize];
            }

            // child[0] = corner at vertex 0:
            //   edge[0] = p_edge_child_edges[0][0]  (leading half of edge 0)
            //   edge[1] = p_face_child_edges[0]      (interior edge connecting e0-mid to e2-mid … wait)
            //   edge[2] = p_edge_child_edges[2][1]  (trailing half of edge 2)
            //
            // Matches C++ exactly (see populateFaceEdgesFromParentFaces):
            //   cFaceEdges[0][0] = pEdgeChildEdges[0][0];
            //   cFaceEdges[0][1] = pFaceChildEdges[0];
            //   cFaceEdges[0][2] = pEdgeChildEdges[2][1];
            if index_is_valid(p_face_child_faces[0]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ce = child.get_face_edges_mut(p_face_child_faces[0]);
                    ce[0] = p_edge_child_edges[0][0];
                    ce[1] = p_face_child_edges[0];
                    ce[2] = p_edge_child_edges[2][1];
                }
            }
            // child[1] = corner at vertex 1:
            //   cFaceEdges[1][0] = pEdgeChildEdges[0][1];
            //   cFaceEdges[1][1] = pEdgeChildEdges[1][0];
            //   cFaceEdges[1][2] = pFaceChildEdges[1];
            if index_is_valid(p_face_child_faces[1]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ce = child.get_face_edges_mut(p_face_child_faces[1]);
                    ce[0] = p_edge_child_edges[0][1];
                    ce[1] = p_edge_child_edges[1][0];
                    ce[2] = p_face_child_edges[1];
                }
            }
            // child[2] = corner at vertex 2:
            //   cFaceEdges[2][0] = pFaceChildEdges[2];
            //   cFaceEdges[2][1] = pEdgeChildEdges[1][1];
            //   cFaceEdges[2][2] = pEdgeChildEdges[2][0];
            if index_is_valid(p_face_child_faces[2]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ce = child.get_face_edges_mut(p_face_child_faces[2]);
                    ce[0] = p_face_child_edges[2];
                    ce[1] = p_edge_child_edges[1][1];
                    ce[2] = p_edge_child_edges[2][0];
                }
            }
            // child[3] = interior:
            //   cFaceEdges[3][0] = pFaceChildEdges[2];
            //   cFaceEdges[3][1] = pFaceChildEdges[0];
            //   cFaceEdges[3][2] = pFaceChildEdges[1];
            if index_is_valid(p_face_child_faces[3]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ce = child.get_face_edges_mut(p_face_child_faces[3]);
                    ce[0] = p_face_child_edges[2];
                    ce[1] = p_face_child_edges[0];
                    ce[2] = p_face_child_edges[1];
                }
            }
        }
    }

    // =========================================================================
    // populateEdgeVertexRelation  (C++ TriRefinement::populateEdgeVertexRelation)
    //
    // Interior edges (face-child-edges) connect edge midpoints.
    // Boundary edges (edge-child-edges) connect midpoint to parent vertex.
    // =========================================================================

    fn populate_edge_vertex_relation_impl(r: &mut Refinement) {
        unsafe {
            let nce = (&*r.child).get_num_edges();
            (&mut *r.child).edge_vert_indices.resize((nce * 2) as usize, 0);
        }
        Self::populate_edge_vertices_from_parent_faces(r);
        Self::populate_edge_vertices_from_parent_edges(r);
    }

    fn populate_edge_vertices_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let (p_face_edges, p_face_child_edges) = unsafe {
                let p = &*r.parent;
                let fe: Vec<Index> = p.get_face_edges(p_face).as_slice().to_vec();
                let fce: Vec<Index> = r.get_face_child_edges(p_face).to_vec();
                (fe, fce)
            };

            debug_assert_eq!(p_face_edges.len(), 3);
            debug_assert_eq!(p_face_child_edges.len(), 3);

            // Midpoints of the three parent edges
            let p_edge_child_verts: [Index; 3] = [
                r.edge_child_vert_index[p_face_edges[0] as usize],
                r.edge_child_vert_index[p_face_edges[1] as usize],
                r.edge_child_vert_index[p_face_edges[2] as usize],
            ];

            // Interior child edges connect:
            //   face-child-edge[0]: eMid[0] → eMid[2]
            //   face-child-edge[1]: eMid[1] → eMid[0]
            //   face-child-edge[2]: eMid[2] → eMid[1]
            if index_is_valid(p_face_child_edges[0]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ev = child.get_edge_vertices_mut(p_face_child_edges[0]);
                    ev[0] = p_edge_child_verts[0];
                    ev[1] = p_edge_child_verts[2];
                }
            }
            if index_is_valid(p_face_child_edges[1]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ev = child.get_edge_vertices_mut(p_face_child_edges[1]);
                    ev[0] = p_edge_child_verts[1];
                    ev[1] = p_edge_child_verts[0];
                }
            }
            if index_is_valid(p_face_child_edges[2]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ev = child.get_edge_vertices_mut(p_face_child_edges[2]);
                    ev[0] = p_edge_child_verts[2];
                    ev[1] = p_edge_child_verts[1];
                }
            }
        }
    }

    fn populate_edge_vertices_from_parent_edges(r: &mut Refinement) {
        let num_edges = r.parent().get_num_edges();

        for p_edge in 0..num_edges {
            let (p_edge_verts, p_edge_children) = unsafe {
                let p = &*r.parent;
                let ev: [Index; 2] = [
                    p.get_edge_vertices(p_edge)[0],
                    p.get_edge_vertices(p_edge)[1],
                ];
                let ec: [Index; 2] = *r.get_edge_child_edges(p_edge);
                (ev, ec)
            };

            // edge-child-edge[j] goes from midpoint to parent vertex[j]
            // child[0]: eMid → vert[0]
            if index_is_valid(p_edge_children[0]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ev = child.get_edge_vertices_mut(p_edge_children[0]);
                    ev[0] = r.edge_child_vert_index[p_edge as usize];
                    ev[1] = r.vert_child_vert_index[p_edge_verts[0] as usize];
                }
            }
            // child[1]: eMid → vert[1]
            if index_is_valid(p_edge_children[1]) {
                unsafe {
                    let child = &mut *r.child;
                    let mut ev = child.get_edge_vertices_mut(p_edge_children[1]);
                    ev[0] = r.edge_child_vert_index[p_edge as usize];
                    ev[1] = r.vert_child_vert_index[p_edge_verts[1] as usize];
                }
            }
        }
    }

    // =========================================================================
    // populateEdgeFaceRelation  (C++ TriRefinement::populateEdgeFaceRelation)
    //
    // Interior edges: each has up to 2 incident child faces (the corner + interior).
    // Boundary edges: each has up to N incident faces (from parent edge's faces).
    // =========================================================================

    fn populate_edge_face_relation_impl(r: &mut Refinement) {
        let (estimate, max_ef) = unsafe {
            let p = &*r.parent;
            // face-child-edges × 2 + parent edge-face-indices × 2
            let est = r.face_child_edge_indices.len() * 2 + p.edge_face_indices.len() * 2;
            (est, p.max_edge_faces)
        };

        unsafe {
            let child = &mut *r.child;
            let nce = child.get_num_edges() as usize;
            child.edge_face_counts_offsets.resize(nce * 2, 0);
            child.edge_face_indices.resize(estimate, 0);
            child.edge_face_local_indices.resize(estimate, 0);
            child.max_edge_faces = max_ef;
        }

        Self::populate_edge_faces_from_parent_faces(r);
        Self::populate_edge_faces_from_parent_edges(r);

        // Trim to actual size
        unsafe {
            let child = &mut *r.child;
            let last = child.get_num_edges() - 1;
            let actual = child.get_num_edge_faces(last) + child.get_offset_of_edge_faces(last);
            child.edge_face_indices.resize(actual as usize, 0);
            child.edge_face_local_indices.resize(actual as usize, 0);
        }
    }

    fn populate_edge_faces_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let (p_face_child_faces, p_face_child_edges) = {
                let fc: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                let fce: Vec<Index> = r.get_face_child_edges(p_face).to_vec();
                (fc, fce)
            };

            debug_assert_eq!(p_face_child_faces.len(), 4);
            debug_assert_eq!(p_face_child_edges.len(), 3);

            let c_face_middle = p_face_child_faces[3];
            let is_middle_valid = index_is_valid(c_face_middle);

            // For each interior child edge j (connecting midpoints):
            //   incident to corner child face[j] (if valid), and middle child face (if valid).
            //   Local index in corner face = (j+1)%3, in middle face = (j+1)%3.
            for j in 0..3usize {
                let c_edge = p_face_child_edges[j];
                if !index_is_valid(c_edge) { continue; }

                unsafe {
                    let child = &mut *r.child;
                    child.resize_edge_faces(c_edge, 2);

                    let offset = child.edge_face_counts_offsets[c_edge as usize * 2 + 1] as usize;
                    let mut count = 0usize;

                    // Corner child face for this interior edge
                    if index_is_valid(p_face_child_faces[j]) {
                        child.edge_face_indices[offset + count] = p_face_child_faces[j];
                        child.edge_face_local_indices[offset + count] = ((j + 1) % 3) as LocalIndex;
                        count += 1;
                    }
                    // Middle child face
                    if is_middle_valid {
                        child.edge_face_indices[offset + count] = c_face_middle;
                        child.edge_face_local_indices[offset + count] = ((j + 1) % 3) as LocalIndex;
                        count += 1;
                    }
                    child.edge_face_counts_offsets[c_edge as usize * 2] = count as i32;
                }
            }
        }
    }

    fn populate_edge_faces_from_parent_edges(r: &mut Refinement) {
        let num_edges = r.parent().get_num_edges();

        for p_edge in 0..num_edges {
            let p_edge_children = *r.get_edge_child_edges(p_edge);
            if !index_is_valid(p_edge_children[0]) && !index_is_valid(p_edge_children[1]) {
                continue;
            }

            let (p_edge_faces, p_edge_in_face, p_edge_verts) = unsafe {
                let p = &*r.parent;
                let ef: Vec<Index> = p.get_edge_faces(p_edge).as_slice().to_vec();
                let einf: Vec<LocalIndex> = p.get_edge_face_local_indices(p_edge).as_slice().to_vec();
                let ev: [Index; 2] = [
                    p.get_edge_vertices(p_edge)[0],
                    p.get_edge_vertices(p_edge)[1],
                ];
                (ef, einf, ev)
            };

            for j in 0..2usize {
                let c_edge = p_edge_children[j];
                if !index_is_valid(c_edge) { continue; }

                unsafe {
                    let child = &mut *r.child;
                    child.resize_edge_faces(c_edge, p_edge_faces.len() as i32);

                    let offset = child.edge_face_counts_offsets[c_edge as usize * 2 + 1] as usize;
                    let mut count = 0usize;

                    for i in 0..p_edge_faces.len() {
                        let p_face       = p_edge_faces[i];
                        let edge_in_face = p_edge_in_face[i] as usize;

                        let p_face_verts: Vec<Index> = (&*r.parent).get_face_vertices(p_face)
                            .as_slice().to_vec();
                        let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();

                        // Identify which child face of this parent face is incident to c_edge.
                        // For a non-degenerate edge, the child face on side j corresponds
                        // to either this corner or the next, depending on edge orientation.
                        let child_of_edge = if p_edge_verts[0] == p_edge_verts[1] {
                            j
                        } else {
                            (p_face_verts[edge_in_face] != p_edge_verts[j]) as usize
                        };

                        let mut child_in_face = edge_in_face + child_of_edge;
                        if child_in_face == p_face_verts.len() { child_in_face = 0; }

                        if index_is_valid(p_face_children[child_in_face]) {
                            let child = &mut *r.child;
                            child.edge_face_indices[offset + count] = p_face_children[child_in_face];
                            child.edge_face_local_indices[offset + count] = edge_in_face as LocalIndex;
                            count += 1;
                        }
                    }
                    child.edge_face_counts_offsets[c_edge as usize * 2] = count as i32;
                }
            }
        }
    }

    // =========================================================================
    // populateVertexFaceRelation  (C++ TriRefinement::populateVertexFaceRelation)
    //
    // No vertices from parent faces for Loop (until N-gon support).
    // Vertices from edges: midpoints → up to 3 child faces per parent incident face.
    // Vertices from verts: corner → child corner face for each parent face.
    // =========================================================================

    fn populate_vertex_face_relation_impl(r: &mut Refinement) {
        let estimate = unsafe {
            let p = &*r.parent;
            // 3 child faces per parent edge-face for edge-child-verts
            // + 1 per parent vert-face for vert-child-verts
            p.edge_face_indices.len() * 3 + p.vert_face_indices.len()
        };

        unsafe {
            let child = &mut *r.child;
            let ncv = child.get_num_vertices() as usize;
            child.vert_face_counts_offsets.resize(ncv * 2, 0);
            child.vert_face_indices.resize(estimate, 0);
            child.vert_face_local_indices.resize(estimate, 0);
        }

        // No face-child vertices for Loop (faceChildVertCount=0), so order depends
        // on whether vertex-child-vertices start at index 0.
        if r.get_first_child_vertex_from_vertices() == 0 {
            Self::populate_vertex_faces_from_parent_vertices(r);
            Self::populate_vertex_faces_from_parent_edges(r);
        } else {
            Self::populate_vertex_faces_from_parent_edges(r);
            Self::populate_vertex_faces_from_parent_vertices(r);
        }

        // Trim to actual size
        unsafe {
            let child = &mut *r.child;
            let last = child.get_num_vertices() - 1;
            let actual = child.get_num_vertex_faces(last) + child.get_offset_of_vertex_faces(last);
            child.vert_face_indices.resize(actual as usize, 0);
            child.vert_face_local_indices.resize(actual as usize, 0);
        }
    }

    fn populate_vertex_faces_from_parent_edges(r: &mut Refinement) {
        let num_edges = r.parent().get_num_edges();

        for p_edge in 0..num_edges {
            let c_vert = r.edge_child_vert_index[p_edge as usize];
            if !index_is_valid(c_vert) { continue; }

            let (p_edge_faces, p_edge_in_face) = unsafe {
                let p = &*r.parent;
                let ef: Vec<Index> = p.get_edge_faces(p_edge).as_slice().to_vec();
                let einf: Vec<LocalIndex> = p.get_edge_face_local_indices(p_edge).as_slice().to_vec();
                (ef, einf)
            };

            unsafe {
                let child = &mut *r.child;
                // Up to 3 child faces per parent incident face (leading, middle, trailing)
                child.resize_vertex_faces(c_vert, (3 * p_edge_faces.len()) as i32);

                let offset = child.vert_face_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for i in 0..p_edge_faces.len() {
                    let p_face       = p_edge_faces[i];
                    let edge_in_face = p_edge_in_face[i] as usize;

                    // Three child faces of the parent tri that share this edge midpoint:
                    //   leadingFace  = (edgeInFace+1) % 3  (next corner)
                    //   middleFace   = 3                   (interior)
                    //   trailingFace = edgeInFace          (this corner)
                    let leading_face  = (edge_in_face + 1) % 3;
                    let middle_face   = 3usize;
                    let trailing_face = edge_in_face;

                    // Local indices of the child vertex within each child face
                    let leading_local  = edge_in_face as LocalIndex;
                    let middle_local   = ((edge_in_face + 2) % 3) as LocalIndex;
                    let trailing_local = ((edge_in_face + 1) % 3) as LocalIndex;

                    let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();

                    debug_assert_eq!(p_face_children.len(), 4);

                    let cf = p_face_children[leading_face];
                    if index_is_valid(cf) {
                        child.vert_face_indices[offset + count] = cf;
                        child.vert_face_local_indices[offset + count] = leading_local;
                        count += 1;
                    }
                    let cf = p_face_children[middle_face];
                    if index_is_valid(cf) {
                        child.vert_face_indices[offset + count] = cf;
                        child.vert_face_local_indices[offset + count] = middle_local;
                        count += 1;
                    }
                    let cf = p_face_children[trailing_face];
                    if index_is_valid(cf) {
                        child.vert_face_indices[offset + count] = cf;
                        child.vert_face_local_indices[offset + count] = trailing_local;
                        count += 1;
                    }
                }
                child.trim_vertex_faces(c_vert, count as i32);
            }
        }
    }

    fn populate_vertex_faces_from_parent_vertices(r: &mut Refinement) {
        let num_verts = r.parent().get_num_vertices();

        for p_vert in 0..num_verts {
            let c_vert = r.vert_child_vert_index[p_vert as usize];
            if !index_is_valid(c_vert) { continue; }

            let (p_vert_faces, p_vert_in_face) = unsafe {
                let p = &*r.parent;
                let vf: Vec<Index> = p.get_vertex_faces(p_vert).as_slice().to_vec();
                let vinf: Vec<LocalIndex> = p.get_vertex_face_local_indices(p_vert).as_slice().to_vec();
                (vf, vinf)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_faces(c_vert, p_vert_faces.len() as i32);

                let offset = child.vert_face_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for i in 0..p_vert_faces.len() {
                    let p_face       = p_vert_faces[i];
                    let vert_in_face = p_vert_in_face[i] as usize;   // which child of this face

                    let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();

                    // Child face for this corner = child[vert_in_face]; local index = vert_in_face
                    let c_face = p_face_children[vert_in_face];
                    if index_is_valid(c_face) {
                        child.vert_face_indices[offset + count] = c_face;
                        child.vert_face_local_indices[offset + count] = vert_in_face as LocalIndex;
                        count += 1;
                    }
                }
                child.trim_vertex_faces(c_vert, count as i32);
            }
        }
    }

    // =========================================================================
    // populateVertexEdgeRelation  (C++ TriRefinement::populateVertexEdgeRelation)
    //
    // Vertices from edges (midpoints): 2 + 2*N incident child edges, where N =
    //   number of parent edge-faces.  Ordering: leading child-edge-of-edge, then
    //   two interior face edges per face, then trailing child-edge-of-edge.
    // Vertices from verts: one child edge per parent incident edge.
    // =========================================================================

    fn populate_vertex_edge_relation_impl(r: &mut Refinement) {
        let estimate = unsafe {
            let p = &*r.parent;
            // 2 interior edges per parent edge-face + 2 child-edges-of-edge + parent vert-edges
            p.edge_face_indices.len() * 2 + p.get_num_edges() as usize * 2
                + p.vert_edge_indices.len()
        };

        unsafe {
            let child = &mut *r.child;
            let ncv = child.get_num_vertices() as usize;
            child.vert_edge_counts_offsets.resize(ncv * 2, 0);
            child.vert_edge_indices.resize(estimate, 0);
            child.vert_edge_local_indices.resize(estimate, 0);
        }

        if r.get_first_child_vertex_from_vertices() == 0 {
            Self::populate_vertex_edges_from_parent_vertices(r);
            Self::populate_vertex_edges_from_parent_edges(r);
        } else {
            Self::populate_vertex_edges_from_parent_edges(r);
            Self::populate_vertex_edges_from_parent_vertices(r);
        }

        // Trim to actual size
        unsafe {
            let child = &mut *r.child;
            let last = child.get_num_vertices() - 1;
            let actual = child.get_num_vertex_edges(last) + child.get_offset_of_vertex_edges(last);
            child.vert_edge_indices.resize(actual as usize, 0);
            child.vert_edge_local_indices.resize(actual as usize, 0);
        }
    }

    fn populate_vertex_edges_from_parent_edges(r: &mut Refinement) {
        let num_edges = r.parent().get_num_edges();

        for p_edge in 0..num_edges {
            let c_vert = r.edge_child_vert_index[p_edge as usize];
            if !index_is_valid(c_vert) { continue; }

            let (p_edge_faces, p_edge_in_face, p_edge_verts, p_edge_child_edges) = unsafe {
                let p = &*r.parent;
                let ef: Vec<Index> = p.get_edge_faces(p_edge).as_slice().to_vec();
                let einf: Vec<LocalIndex> = p.get_edge_face_local_indices(p_edge).as_slice().to_vec();
                let ev: [Index; 2] = [
                    p.get_edge_vertices(p_edge)[0],
                    p.get_edge_vertices(p_edge)[1],
                ];
                let ec: [Index; 2] = *r.get_edge_child_edges(p_edge);
                (ef, einf, ev, ec)
            };

            unsafe {
                let child = &mut *r.child;
                // Estimate: 2 child-edge-of-edge + 2 interior edges per incident face
                child.resize_vertex_edges(c_vert, (p_edge_faces.len() + 2) as i32);

                let offset = child.vert_edge_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                // Determine orientation of the parent edge wrt the first incident face.
                let mut p_edge_reversed = false;
                let mut c_edge_of_edge0 = super::types::INDEX_INVALID;
                let mut c_edge_of_edge1 = super::types::INDEX_INVALID;

                for i in 0..p_edge_faces.len() {
                    let p_face       = p_edge_faces[i];
                    let edge_in_face = p_edge_in_face[i] as usize;

                    let p_face_child_edges: Vec<Index> = r.get_face_child_edges(p_face).to_vec();

                    if i == 0 {
                        // Determine reversal from the first face
                        if p_edge_verts[0] != p_edge_verts[1] {
                            p_edge_reversed = {
                                let fv: Vec<Index> = (&*r.parent).get_face_vertices(p_face)
                                    .as_slice().to_vec();
                                fv[edge_in_face] != p_edge_verts[0]
                            };
                        }
                        c_edge_of_edge0 = p_edge_child_edges[(!p_edge_reversed) as usize];
                        c_edge_of_edge1 = p_edge_child_edges[p_edge_reversed as usize];
                    }

                    // Two interior face edges incident to this midpoint vertex:
                    //   cEdgeOfFace0 = pFaceChildEdges[(edgeInFace+1) % 3]
                    //   cEdgeOfFace1 = pFaceChildEdges[edgeInFace]
                    let c_edge_of_face0 = p_face_child_edges[(edge_in_face + 1) % 3];
                    let c_edge_of_face1 = p_face_child_edges[edge_in_face];

                    // On the first face: insert leading edge-of-edge, then two interior,
                    // then trailing edge-of-edge.
                    if i == 0 {
                        if index_is_valid(c_edge_of_edge0) {
                            child.vert_edge_indices[offset + count] = c_edge_of_edge0;
                            child.vert_edge_local_indices[offset + count] = 0;
                            count += 1;
                        }
                    }
                    if index_is_valid(c_edge_of_face0) {
                        child.vert_edge_indices[offset + count] = c_edge_of_face0;
                        child.vert_edge_local_indices[offset + count] = 1;
                        count += 1;
                    }
                    if index_is_valid(c_edge_of_face1) {
                        child.vert_edge_indices[offset + count] = c_edge_of_face1;
                        child.vert_edge_local_indices[offset + count] = 0;
                        count += 1;
                    }
                    if i == 0 {
                        if index_is_valid(c_edge_of_edge1) {
                            child.vert_edge_indices[offset + count] = c_edge_of_edge1;
                            child.vert_edge_local_indices[offset + count] = 0;
                            count += 1;
                        }
                    }
                }
                child.trim_vertex_edges(c_vert, count as i32);
            }
        }
    }

    fn populate_vertex_edges_from_parent_vertices(r: &mut Refinement) {
        let num_verts = r.parent().get_num_vertices();

        for p_vert in 0..num_verts {
            let c_vert = r.vert_child_vert_index[p_vert as usize];
            if !index_is_valid(c_vert) { continue; }

            let (p_vert_edges, p_vert_in_edge) = unsafe {
                let p = &*r.parent;
                let ve: Vec<Index> = p.get_vertex_edges(p_vert).as_slice().to_vec();
                let vine: Vec<LocalIndex> = p.get_vertex_edge_local_indices(p_vert).as_slice().to_vec();
                (ve, vine)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_edges(c_vert, p_vert_edges.len() as i32);

                let offset = child.vert_edge_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for i in 0..p_vert_edges.len() {
                    let p_edge_idx  = p_vert_edges[i];
                    let p_edge_vert = p_vert_in_edge[i] as usize;

                    // The child edge of the parent edge that touches this corner vertex
                    let c_edge = r.get_edge_child_edges(p_edge_idx)[p_edge_vert];
                    if index_is_valid(c_edge) {
                        child.vert_edge_indices[offset + count] = c_edge;
                        // Local index 1 = this vertex is the second endpoint of the child edge
                        child.vert_edge_local_indices[offset + count] = 1;
                        count += 1;
                    }
                }
                child.trim_vertex_edges(c_vert, count as i32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tri_refinement_smoke() {
        use super::*;
        use crate::sdc::Options;
        let _ = std::mem::size_of::<TriRefinement>();
        let _ = std::mem::size_of::<Options>();
    }
}
