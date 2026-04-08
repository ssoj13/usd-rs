// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/quadRefinement.h/.cpp

use super::level::Level;
use super::refinement::{Refinement, RefinementOptions};
use super::types::{Index, LocalIndex, index_is_valid};
use crate::sdc::{Options, types::Split};

/// Quad-splitting (Catmark / Bilinear) refinement.
/// Mirrors C++ `Vtr::internal::QuadRefinement`.
pub struct QuadRefinement(pub Refinement);

impl QuadRefinement {
    /// Create a quad refinement between `parent` and `child` levels.
    ///
    /// # Safety
    /// `parent` and `child` must outlive this struct.
    pub unsafe fn new(parent: *const Level, child: *mut Level, options: Options) -> Self {
        let mut r = unsafe { Refinement::new(parent, child, options, Split::ToQuads) };
        // Wire all scheme-specific callbacks so that a bare Box<Refinement>
        // (as used in refine_adaptive) can call refine() correctly.
        r.allocate_fn = Some(QuadRefinement::allocate_parent_child_indices_impl);
        r.sparse_face_fn = Some(QuadRefinement::mark_sparse_face_children_impl);
        r.populate_fv_fn = Some(QuadRefinement::populate_face_vertex_relation_impl);
        r.populate_fe_fn = Some(QuadRefinement::populate_face_edge_relation_impl);
        r.populate_ev_fn = Some(QuadRefinement::populate_edge_vertex_relation_impl);
        r.populate_ef_fn = Some(QuadRefinement::populate_edge_face_relation_impl);
        r.populate_vf_fn = Some(QuadRefinement::populate_vertex_face_relation_impl);
        r.populate_ve_fn = Some(QuadRefinement::populate_vertex_edge_relation_impl);
        Self(r)
    }

    pub fn refine(&mut self, opts: RefinementOptions) {
        // Wire up virtual-dispatch overrides before calling the base refine().
        // In C++ these are virtual methods; here we set them manually.
        self.0.refine_with_callbacks(
            opts,
            QuadRefinement::allocate_parent_child_indices_impl,
            QuadRefinement::mark_sparse_face_children_impl,
            QuadRefinement::populate_face_vertex_relation_impl,
            QuadRefinement::populate_face_edge_relation_impl,
            QuadRefinement::populate_edge_vertex_relation_impl,
            QuadRefinement::populate_edge_face_relation_impl,
            QuadRefinement::populate_vertex_face_relation_impl,
            QuadRefinement::populate_vertex_edge_relation_impl,
        );
    }

    pub fn inner(&self) -> &Refinement {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut Refinement {
        &mut self.0
    }

    // =========================================================================
    // allocateParentChildIndices  (C++ QuadRefinement::allocateParentChildIndices)
    // =========================================================================

    fn allocate_parent_child_indices_impl(r: &mut Refinement) {
        let (
            fv_indices_len,
            fe_indices_len,
            ev_indices_len,
            f_vert_count,
            e_vert_count,
            v_vert_count,
        ) = unsafe {
            let p = &*r.parent;
            (
                p.face_vert_indices.len(),
                p.face_edge_indices.len(),
                p.edge_vert_indices.len(),
                p.get_num_faces(),
                p.get_num_edges(),
                p.get_num_vertices(),
            )
        };

        // face-child-faces and face-child-edges share the parent face-vert c/o array
        r.face_child_face_counts_offsets_shared = true;
        r.face_child_edge_counts_offsets_shared = true;

        r.face_child_face_indices.resize(fv_indices_len, 0);
        r.face_child_edge_indices.resize(fe_indices_len, 0);
        r.edge_child_edge_indices.resize(ev_indices_len, 0);

        r.face_child_vert_index.resize(f_vert_count as usize, 0);
        r.edge_child_vert_index.resize(e_vert_count as usize, 0);
        r.vert_child_vert_index.resize(v_vert_count as usize, 0);
    }

    // =========================================================================
    // markSparseFaceChildren  (C++ QuadRefinement::markSparseFaceChildren)
    // =========================================================================

    fn mark_sparse_face_children_impl(r: &mut Refinement) {
        debug_assert!(!r.parent_face_tag.is_empty());

        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let f_verts: Vec<Index> =
                unsafe { (&*r.parent).get_face_vertices(p_face).as_slice().to_vec() };

            let p_face_tag = &r.parent_face_tag[p_face as usize];

            if p_face_tag.selected {
                // Mark all child faces, child edges, and the face-child vertex as selected.
                // Use count/offset from the shared parent face-vert c/o array.
                let pf_co = r.face_child_face_co_pub(p_face);
                for i in 0..pf_co.0 {
                    let idx = pf_co.1 + i;
                    r.face_child_face_indices[idx] = super::refinement::SPARSE_MASK_SELECTED;
                    r.face_child_edge_indices[idx] = super::refinement::SPARSE_MASK_SELECTED;
                }
                r.face_child_vert_index[p_face as usize] = super::refinement::SPARSE_MASK_SELECTED;

                r.parent_face_tag[p_face as usize].transitional = 0;
            } else {
                let mut marked = false;

                for i in 0..f_verts.len() {
                    if r.parent_vertex_tag[f_verts[i] as usize].selected {
                        let i_prev = if i > 0 { i - 1 } else { f_verts.len() - 1 };
                        // Mark neighboring child face and edges
                        let pf_co = r.face_child_face_co_pub(p_face);
                        r.face_child_face_indices[pf_co.1 + i] =
                            super::refinement::SPARSE_MASK_NEIGHBORING;
                        r.face_child_edge_indices[pf_co.1 + i] =
                            super::refinement::SPARSE_MASK_NEIGHBORING;
                        r.face_child_edge_indices[pf_co.1 + i_prev] =
                            super::refinement::SPARSE_MASK_NEIGHBORING;
                        marked = true;
                    }
                }

                if marked {
                    r.face_child_vert_index[p_face as usize] =
                        super::refinement::SPARSE_MASK_NEIGHBORING;

                    // Assign transitional bits from incident edge tags
                    let f_edges: Vec<Index> =
                        unsafe { (&*r.parent).get_face_edges(p_face).as_slice().to_vec() };
                    let n = f_edges.len();
                    let mut trans: u8 = 0;
                    if n == 4 {
                        trans = ((r.parent_edge_tag[f_edges[0] as usize].transitional as u8) << 0)
                            | ((r.parent_edge_tag[f_edges[1] as usize].transitional as u8) << 1)
                            | ((r.parent_edge_tag[f_edges[2] as usize].transitional as u8) << 2)
                            | ((r.parent_edge_tag[f_edges[3] as usize].transitional as u8) << 3);
                    } else if n == 3 {
                        trans = ((r.parent_edge_tag[f_edges[0] as usize].transitional as u8) << 0)
                            | ((r.parent_edge_tag[f_edges[1] as usize].transitional as u8) << 1)
                            | ((r.parent_edge_tag[f_edges[2] as usize].transitional as u8) << 2);
                    } else {
                        for fe in &f_edges {
                            trans |= r.parent_edge_tag[*fe as usize].transitional;
                        }
                    }
                    r.parent_face_tag[p_face as usize].transitional = trans;
                }
            }
        }
    }

    // =========================================================================
    // populateFaceVertexRelation  (C++ QuadRefinement::populateFaceVertexRelation)
    // =========================================================================

    fn populate_face_vertex_relation_impl(r: &mut Refinement) {
        // Ensure face-vert counts/offsets are set in child
        unsafe {
            let child = &mut *r.child;
            if child.face_vert_counts_offsets.is_empty() {
                Self::populate_face_vertex_counts_and_offsets(r);
            }
            let nf = (&*r.child).get_num_faces();
            (&mut *r.child)
                .face_vert_indices
                .resize((nf * 4) as usize, 0);
        }
        Self::populate_face_vertices_from_parent_faces(r);
    }

    fn populate_face_vertex_counts_and_offsets(r: &mut Refinement) {
        unsafe {
            let child = &mut *r.child;
            let nf = child.get_num_faces() as usize;
            child.face_vert_counts_offsets.resize(nf * 2, 0);
            for i in 0..nf {
                child.face_vert_counts_offsets[i * 2] = 4;
                child.face_vert_counts_offsets[i * 2 + 1] = (i << 2) as i32;
            }
        }
    }

    fn populate_face_vertices_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            // Collect parent face info
            let (p_face_verts, p_face_edges, p_face_children) = unsafe {
                let p = &*r.parent;
                let fv: Vec<Index> = p.get_face_vertices(p_face).as_slice().to_vec();
                let fe: Vec<Index> = p.get_face_edges(p_face).as_slice().to_vec();
                let fc: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                (fv, fe, fc)
            };

            let pf_size = p_face_verts.len();

            for j in 0..pf_size {
                let c_face = p_face_children[j];
                if !index_is_valid(c_face) {
                    continue;
                }

                let j_prev = if j > 0 { j - 1 } else { pf_size - 1 };

                let c_vert_of_face = r.face_child_vert_index[p_face as usize];
                let c_vert_of_eprev = r.edge_child_vert_index[p_face_edges[j_prev] as usize];
                let c_vert_of_vert = r.vert_child_vert_index[p_face_verts[j] as usize];
                let c_vert_of_enext = r.edge_child_vert_index[p_face_edges[j] as usize];

                unsafe {
                    let child = &mut *r.child;
                    let mut c_face_verts = child.get_face_vertices_mut(c_face); // mut slice

                    if pf_size == 4 {
                        // Quad: standard orientation
                        let j_opp = if j_prev > 0 { j_prev - 1 } else { 3 };
                        let j_next = if j_opp > 0 { j_opp - 1 } else { 3 };

                        c_face_verts[j] = c_vert_of_vert;
                        c_face_verts[j_next] = c_vert_of_enext;
                        c_face_verts[j_opp] = c_vert_of_face;
                        c_face_verts[j_prev] = c_vert_of_eprev;
                    } else {
                        // Non-quad: fixed orientation
                        c_face_verts[0] = c_vert_of_vert;
                        c_face_verts[1] = c_vert_of_enext;
                        c_face_verts[2] = c_vert_of_face;
                        c_face_verts[3] = c_vert_of_eprev;
                    }
                }
            }
        }
    }

    // =========================================================================
    // populateFaceEdgeRelation  (C++ QuadRefinement::populateFaceEdgeRelation)
    // =========================================================================

    fn populate_face_edge_relation_impl(r: &mut Refinement) {
        unsafe {
            let child = &mut *r.child;
            if child.face_vert_counts_offsets.is_empty() {
                Self::populate_face_vertex_counts_and_offsets(r);
            }
            let nf = (&*r.child).get_num_faces();
            (&mut *r.child)
                .face_edge_indices
                .resize((nf * 4) as usize, 0);
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

            let pf_size = p_face_verts.len();

            for j in 0..pf_size {
                let c_face = p_face_child_faces[j];
                if !index_is_valid(c_face) {
                    continue;
                }

                let j_prev = if j > 0 { j - 1 } else { pf_size - 1 };

                let p_prev_edge = p_face_edges[j_prev];
                let p_next_edge = p_face_edges[j];

                let (p_prev_ev, p_next_ev) = unsafe {
                    let p = &*r.parent;
                    let prev_ev: [Index; 2] = [
                        p.get_edge_vertices(p_prev_edge)[0],
                        p.get_edge_vertices(p_prev_edge)[1],
                    ];
                    let next_ev: [Index; 2] = [
                        p.get_edge_vertices(p_next_edge)[0],
                        p.get_edge_vertices(p_next_edge)[1],
                    ];
                    (prev_ev, next_ev)
                };

                let p_corner_vert = p_face_verts[j];

                // Determine which half of each edge touches the corner vertex
                let corner_in_prev_edge = if p_prev_ev[0] != p_prev_ev[1] {
                    (p_prev_ev[0] != p_corner_vert) as usize
                } else {
                    1
                };
                let corner_in_next_edge = if p_next_ev[0] != p_next_ev[1] {
                    (p_next_ev[0] != p_corner_vert) as usize
                } else {
                    0
                };

                let c_edge_of_prev = r.get_edge_child_edges(p_prev_edge)[corner_in_prev_edge];
                let c_edge_of_next = r.get_edge_child_edges(p_next_edge)[corner_in_next_edge];
                let c_edge_perp_prev = p_face_child_edges[j_prev];
                let c_edge_perp_next = p_face_child_edges[j];

                unsafe {
                    let child = &mut *r.child;
                    let mut c_face_edges = child.get_face_edges_mut(c_face); // mut slice

                    if pf_size == 4 {
                        let j_opp = if j_prev > 0 { j_prev - 1 } else { 3 };
                        let j_next = if j_opp > 0 { j_opp - 1 } else { 3 };

                        c_face_edges[j] = c_edge_of_next;
                        c_face_edges[j_next] = c_edge_perp_next;
                        c_face_edges[j_opp] = c_edge_perp_prev;
                        c_face_edges[j_prev] = c_edge_of_prev;
                    } else {
                        c_face_edges[0] = c_edge_of_next;
                        c_face_edges[1] = c_edge_perp_next;
                        c_face_edges[2] = c_edge_perp_prev;
                        c_face_edges[3] = c_edge_of_prev;
                    }
                }
            }
        }
    }

    // =========================================================================
    // populateEdgeVertexRelation  (C++ QuadRefinement::populateEdgeVertexRelation)
    // =========================================================================

    fn populate_edge_vertex_relation_impl(r: &mut Refinement) {
        unsafe {
            let nce = (&*r.child).get_num_edges();
            (&mut *r.child)
                .edge_vert_indices
                .resize((nce * 2) as usize, 0);
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

            for j in 0..p_face_edges.len() {
                let c_edge = p_face_child_edges[j];
                if !index_is_valid(c_edge) {
                    continue;
                }

                unsafe {
                    let child = &mut *r.child;
                    let mut c_edge_verts = child.get_edge_vertices_mut(c_edge); // mut slice
                    c_edge_verts[0] = r.face_child_vert_index[p_face as usize];
                    c_edge_verts[1] = r.edge_child_vert_index[p_face_edges[j] as usize];
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

            for j in 0..2usize {
                let c_edge = p_edge_children[j];
                if !index_is_valid(c_edge) {
                    continue;
                }

                unsafe {
                    let child = &mut *r.child;
                    let mut c_edge_verts = child.get_edge_vertices_mut(c_edge); // mut slice
                    c_edge_verts[0] = r.edge_child_vert_index[p_edge as usize];
                    c_edge_verts[1] = r.vert_child_vert_index[p_edge_verts[j] as usize];
                }
            }
        }
    }

    // =========================================================================
    // populateEdgeFaceRelation  (C++ QuadRefinement::populateEdgeFaceRelation)
    // =========================================================================

    fn populate_edge_face_relation_impl(r: &mut Refinement) {
        let (estimate, max_ef) = unsafe {
            let p = &*r.parent;
            let est = p.face_vert_indices.len() * 2 + p.edge_face_indices.len() * 2;
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

            let pf_size = p_face_child_faces.len();

            for j in 0..pf_size {
                let c_edge = p_face_child_edges[j];
                if !index_is_valid(c_edge) {
                    continue;
                }

                let j_next = if (j + 1) < pf_size { j + 1 } else { 0 };

                unsafe {
                    let child = &mut *r.child;
                    child.resize_edge_faces(c_edge, 2);

                    // Read offset before taking mutable borrow of counts array.
                    let offset = child.edge_face_counts_offsets[c_edge as usize * 2 + 1] as usize;
                    let mut count = 0usize;

                    if index_is_valid(p_face_child_faces[j]) {
                        child.edge_face_indices[offset + count] = p_face_child_faces[j];
                        child.edge_face_local_indices[offset + count] = if pf_size == 4 {
                            j_next as LocalIndex
                        } else {
                            1
                        };
                        count += 1;
                    }
                    if index_is_valid(p_face_child_faces[j_next]) {
                        child.edge_face_indices[offset + count] = p_face_child_faces[j_next];
                        child.edge_face_local_indices[offset + count] = if pf_size == 4 {
                            ((j_next + 2) & 3) as LocalIndex
                        } else {
                            2
                        };
                        count += 1;
                    }
                    // Trim to actual count.
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
                let einf: Vec<LocalIndex> =
                    p.get_edge_face_local_indices(p_edge).as_slice().to_vec();
                let ev: [Index; 2] = [
                    p.get_edge_vertices(p_edge)[0],
                    p.get_edge_vertices(p_edge)[1],
                ];
                (ef, einf, ev)
            };

            for j in 0..2usize {
                let c_edge = p_edge_children[j];
                if !index_is_valid(c_edge) {
                    continue;
                }

                unsafe {
                    let child = &mut *r.child;
                    child.resize_edge_faces(c_edge, p_edge_faces.len() as i32);

                    let offset = child.edge_face_counts_offsets[c_edge as usize * 2 + 1] as usize;
                    let mut c_edge_face_count = 0usize;

                    for i in 0..p_edge_faces.len() {
                        let p_face = p_edge_faces[i];
                        let edge_in_face = p_edge_in_face[i] as usize;

                        let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                        let p_face_size = p_face_children.len();
                        let p_face_verts: Vec<Index> =
                            (&*r.parent).get_face_vertices(p_face).as_slice().to_vec();

                        let child_of_edge = if p_edge_verts[0] == p_edge_verts[1] {
                            j
                        } else {
                            (p_face_verts[edge_in_face] != p_edge_verts[j]) as usize
                        };

                        let mut child_in_face = edge_in_face + child_of_edge;
                        if child_in_face == p_face_children.len() {
                            child_in_face = 0;
                        }

                        if index_is_valid(p_face_children[child_in_face]) {
                            let child = &mut *r.child;
                            child.edge_face_indices[offset + c_edge_face_count] =
                                p_face_children[child_in_face];
                            child.edge_face_local_indices[offset + c_edge_face_count] =
                                if p_face_size == 4 {
                                    edge_in_face as LocalIndex
                                } else {
                                    if child_of_edge != 0 { 3 } else { 0 }
                                };
                            c_edge_face_count += 1;
                        }
                    }
                    child.edge_face_counts_offsets[c_edge as usize * 2] = c_edge_face_count as i32;
                }
            }
        }
    }

    // =========================================================================
    // populateVertexFaceRelation  (C++ QuadRefinement::populateVertexFaceRelation)
    // =========================================================================

    fn populate_vertex_face_relation_impl(r: &mut Refinement) {
        let estimate = unsafe {
            let p = &*r.parent;
            p.face_vert_indices.len() + p.edge_face_indices.len() * 2 + p.vert_face_indices.len()
        };

        unsafe {
            let child = &mut *r.child;
            let ncv = child.get_num_vertices() as usize;
            child.vert_face_counts_offsets.resize(ncv * 2, 0);
            child.vert_face_indices.resize(estimate, 0);
            child.vert_face_local_indices.resize(estimate, 0);
        }

        if r.get_first_child_vertex_from_vertices() == 0 {
            Self::populate_vertex_faces_from_parent_vertices(r);
            Self::populate_vertex_faces_from_parent_faces(r);
            Self::populate_vertex_faces_from_parent_edges(r);
        } else {
            Self::populate_vertex_faces_from_parent_faces(r);
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

    fn populate_vertex_faces_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let c_vert = r.face_child_vert_index[p_face as usize];
            if !index_is_valid(c_vert) {
                continue;
            }

            let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
            let pf_size = p_face_children.len();

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_faces(c_vert, pf_size as i32);

                let offset = child.vert_face_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for j in 0..pf_size {
                    if index_is_valid(p_face_children[j]) {
                        child.vert_face_indices[offset + count] = p_face_children[j];
                        child.vert_face_local_indices[offset + count] = if pf_size == 4 {
                            ((j + 2) & 3) as LocalIndex
                        } else {
                            2
                        };
                        count += 1;
                    }
                }
                child.trim_vertex_faces(c_vert, count as i32);
            }
        }
    }

    fn populate_vertex_faces_from_parent_edges(r: &mut Refinement) {
        let num_edges = r.parent().get_num_edges();

        for p_edge in 0..num_edges {
            let c_vert = r.edge_child_vert_index[p_edge as usize];
            if !index_is_valid(c_vert) {
                continue;
            }

            let (p_edge_faces, p_edge_in_face) = unsafe {
                let p = &*r.parent;
                let ef: Vec<Index> = p.get_edge_faces(p_edge).as_slice().to_vec();
                let einf: Vec<LocalIndex> =
                    p.get_edge_face_local_indices(p_edge).as_slice().to_vec();
                (ef, einf)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_faces(c_vert, (2 * p_edge_faces.len()) as i32);

                let offset = child.vert_face_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for i in 0..p_edge_faces.len() {
                    let p_face = p_edge_faces[i];
                    let edge_in_face = p_edge_in_face[i] as usize;

                    let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                    let pf_size = p_face_children.len();

                    let fc0 = edge_in_face;
                    let fc1 = if (edge_in_face + 1) < pf_size {
                        edge_in_face + 1
                    } else {
                        0
                    };

                    // Second child first (for CC-wise ordering)
                    if index_is_valid(p_face_children[fc1]) {
                        child.vert_face_indices[offset + count] = p_face_children[fc1];
                        child.vert_face_local_indices[offset + count] =
                            if pf_size == 4 { fc0 as LocalIndex } else { 3 };
                        count += 1;
                    }
                    if index_is_valid(p_face_children[fc0]) {
                        child.vert_face_indices[offset + count] = p_face_children[fc0];
                        child.vert_face_local_indices[offset + count] =
                            if pf_size == 4 { fc1 as LocalIndex } else { 1 };
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
            if !index_is_valid(c_vert) {
                continue;
            }

            let (p_vert_faces, p_vert_in_face) = unsafe {
                let p = &*r.parent;
                let vf: Vec<Index> = p.get_vertex_faces(p_vert).as_slice().to_vec();
                let vinf: Vec<LocalIndex> =
                    p.get_vertex_face_local_indices(p_vert).as_slice().to_vec();
                (vf, vinf)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_faces(c_vert, p_vert_faces.len() as i32);

                let offset = child.vert_face_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for i in 0..p_vert_faces.len() {
                    let p_face = p_vert_faces[i];
                    let vert_in_face = p_vert_in_face[i] as usize;

                    let p_face_children: Vec<Index> = r.get_face_child_faces(p_face).to_vec();
                    let pf_size = p_face_children.len();

                    if index_is_valid(p_face_children[vert_in_face]) {
                        child.vert_face_indices[offset + count] = p_face_children[vert_in_face];
                        child.vert_face_local_indices[offset + count] = if pf_size == 4 {
                            vert_in_face as LocalIndex
                        } else {
                            0
                        };
                        count += 1;
                    }
                }
                child.trim_vertex_faces(c_vert, count as i32);
            }
        }
    }

    // =========================================================================
    // populateVertexEdgeRelation  (C++ QuadRefinement::populateVertexEdgeRelation)
    // =========================================================================

    fn populate_vertex_edge_relation_impl(r: &mut Refinement) {
        let estimate = unsafe {
            let p = &*r.parent;
            p.face_vert_indices.len()
                + p.edge_face_indices.len()
                + p.get_num_edges() as usize * 2
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
            Self::populate_vertex_edges_from_parent_faces(r);
            Self::populate_vertex_edges_from_parent_edges(r);
        } else {
            Self::populate_vertex_edges_from_parent_faces(r);
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

    fn populate_vertex_edges_from_parent_faces(r: &mut Refinement) {
        let num_faces = r.parent().get_num_faces();

        for p_face in 0..num_faces {
            let c_vert = r.face_child_vert_index[p_face as usize];
            if !index_is_valid(c_vert) {
                continue;
            }

            let (p_face_verts_len, p_face_child_edges) = unsafe {
                let p = &*r.parent;
                let fv_len = p.get_face_vertices(p_face).size() as usize;
                let fce: Vec<Index> = r.get_face_child_edges(p_face).to_vec();
                (fv_len, fce)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_edges(c_vert, p_face_verts_len as i32);

                let offset = child.vert_edge_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for j in 0..p_face_verts_len {
                    // Leading edge = j-1 (for j=0 it wraps to last)
                    let j_leading = if j > 0 { j - 1 } else { p_face_verts_len - 1 };
                    if index_is_valid(p_face_child_edges[j_leading]) {
                        child.vert_edge_indices[offset + count] = p_face_child_edges[j_leading];
                        child.vert_edge_local_indices[offset + count] = 0;
                        count += 1;
                    }
                }
                child.trim_vertex_edges(c_vert, count as i32);
            }
        }
    }

    fn populate_vertex_edges_from_parent_edges(r: &mut Refinement) {
        let num_edges = r.parent().get_num_edges();

        for p_edge in 0..num_edges {
            let c_vert = r.edge_child_vert_index[p_edge as usize];
            if !index_is_valid(c_vert) {
                continue;
            }

            let (p_edge_faces, p_edge_in_face, p_edge_verts, p_edge_child_edges) = unsafe {
                let p = &*r.parent;
                let ef: Vec<Index> = p.get_edge_faces(p_edge).as_slice().to_vec();
                let einf: Vec<LocalIndex> =
                    p.get_edge_face_local_indices(p_edge).as_slice().to_vec();
                let ev: [Index; 2] = [
                    p.get_edge_vertices(p_edge)[0],
                    p.get_edge_vertices(p_edge)[1],
                ];
                let ec: [Index; 2] = *r.get_edge_child_edges(p_edge);
                (ef, einf, ev, ec)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_edges(c_vert, (p_edge_faces.len() + 2) as i32);

                let offset = child.vert_edge_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                // First two: child edges of parent edge
                if index_is_valid(p_edge_child_edges[0]) {
                    child.vert_edge_indices[offset + count] = p_edge_child_edges[0];
                    child.vert_edge_local_indices[offset + count] = 0;
                    count += 1;
                }
                if index_is_valid(p_edge_child_edges[1]) {
                    child.vert_edge_indices[offset + count] = p_edge_child_edges[1];
                    child.vert_edge_local_indices[offset + count] = 0;
                    count += 1;
                }

                // Then append interior face edges with swap for correct ordering
                for i in 0..p_edge_faces.len() {
                    let p_face = p_edge_faces[i];
                    let edge_in_face = p_edge_in_face[i] as usize;

                    let c_edge_of_face = r.get_face_child_edges(p_face)[edge_in_face];

                    if index_is_valid(c_edge_of_face) {
                        let child = &mut *r.child;
                        child.vert_edge_indices[offset + count] = c_edge_of_face;
                        child.vert_edge_local_indices[offset + count] = 1;
                        count += 1;

                        // Swap for CC-wise order on first face
                        if i == 0 && count == 3 {
                            let p_face_verts: Vec<Index> =
                                (&*r.parent).get_face_vertices(p_face).as_slice().to_vec();
                            let child = &mut *r.child;
                            if (p_edge_verts[0] != p_edge_verts[1])
                                && (p_face_verts[edge_in_face] == p_edge_verts[0])
                            {
                                child.vert_edge_indices.swap(offset, offset + 1);
                                child.vert_edge_local_indices.swap(offset, offset + 1);
                            }
                            child.vert_edge_indices.swap(offset + 1, offset + 2);
                            child.vert_edge_local_indices.swap(offset + 1, offset + 2);
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
            if !index_is_valid(c_vert) {
                continue;
            }

            let (p_vert_edges, p_vert_in_edge) = unsafe {
                let p = &*r.parent;
                let ve: Vec<Index> = p.get_vertex_edges(p_vert).as_slice().to_vec();
                let vine: Vec<LocalIndex> =
                    p.get_vertex_edge_local_indices(p_vert).as_slice().to_vec();
                (ve, vine)
            };

            unsafe {
                let child = &mut *r.child;
                child.resize_vertex_edges(c_vert, p_vert_edges.len() as i32);

                let offset = child.vert_edge_counts_offsets[c_vert as usize * 2 + 1] as usize;
                let mut count = 0usize;

                for i in 0..p_vert_edges.len() {
                    let p_edge_idx = p_vert_edges[i];
                    let p_edge_vert = p_vert_in_edge[i] as usize;

                    let p_edge_child = r.get_edge_child_edges(p_edge_idx)[p_edge_vert];
                    if index_is_valid(p_edge_child) {
                        let child = &mut *r.child;
                        child.vert_edge_indices[offset + count] = p_edge_child;
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
    // QuadRefinement tests require a fully wired Level — integration-tested in far/topology_refiner.

    #[test]
    fn quad_refinement_smoke() {
        // Just verify it compiles with the right types in scope.
        use super::*;
        use crate::sdc::Options;
        let _ = std::mem::size_of::<QuadRefinement>();
        let _ = std::mem::size_of::<Options>();
    }
}
