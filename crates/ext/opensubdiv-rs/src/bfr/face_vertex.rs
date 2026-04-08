//! FaceVertex — internal topological corner descriptor.
//!
//! Wraps a VertexDescriptor and extends it with ring-location context,
//! face connectivity for unordered neighbourhoods, and subset-finding logic.
//!
//! Ported from OpenSubdiv bfr/faceVertex.h/.cpp.

use super::face_vertex_subset::FaceVertexSubset;
use super::vertex_descriptor::VertexDescriptor;
use super::vertex_tag::VertexTag;
use crate::sdc::crease::{
    SHARPNESS_INFINITE, is_infinite as is_inf_sharp, is_semi_sharp, is_sharp,
};

pub type Index = i32;

/// Topological description of one corner of a face — wraps VertexDescriptor
/// with ring-position context and unordered-face connectivity.
pub struct FaceVertex {
    pub(crate) v_desc: VertexDescriptor,
    pub(crate) tag: VertexTag,

    pub(crate) face_in_ring: i16,
    pub(crate) common_face_size: i16, // 0 = heterogeneous

    pub(crate) reg_face_size: u8,
    pub(crate) is_exp_inf_sharp: bool,
    pub(crate) is_exp_semi_sharp: bool,
    pub(crate) is_imp_inf_sharp: bool,
    pub(crate) is_imp_semi_sharp: bool,

    pub(crate) num_face_verts: i32,

    /// For each incident face, [2*f] = prev-face neighbour, [2*f+1] = next-face
    /// neighbour (-1 = none / boundary / non-manifold).
    pub(crate) face_edge_neighbors: Vec<i16>,
}

impl Default for FaceVertex {
    fn default() -> Self {
        Self {
            v_desc: VertexDescriptor::default(),
            tag: VertexTag::default(),
            face_in_ring: 0,
            common_face_size: 0,
            reg_face_size: 0,
            is_exp_inf_sharp: false,
            is_exp_semi_sharp: false,
            is_imp_inf_sharp: false,
            is_imp_semi_sharp: false,
            num_face_verts: 0,
            face_edge_neighbors: Vec::new(),
        }
    }
}

impl FaceVertex {
    pub fn new() -> Self {
        Self::default()
    }

    // ------------------------------------------------------------------
    //  Initialize / Finalize
    // ------------------------------------------------------------------

    /// Begin specification; called before VertexDescriptor population.
    pub fn initialize(&mut self, face_size: i32, reg_face_size: i32) {
        self.common_face_size = face_size as i16;
        self.reg_face_size = reg_face_size as u8;
        self.num_face_verts = 0;
        self.is_exp_inf_sharp = false;
        self.is_exp_semi_sharp = false;
        self.is_imp_inf_sharp = false;
        self.is_imp_semi_sharp = false;
        self.v_desc.is_valid = false;
        self.v_desc.is_initialized = false;
    }

    /// Finalize after VertexDescriptor has been fully populated.
    pub fn finalize(&mut self, face_in_vertex: i32) {
        debug_assert!(self.v_desc.is_finalized);

        self.face_in_ring = face_in_vertex as i16;

        if !self.v_desc.has_incident_face_sizes() {
            self.num_face_verts = self.v_desc.num_faces as i32 * self.common_face_size as i32;
        } else {
            self.common_face_size = 0;
            self.num_face_verts = *self.v_desc.face_size_offsets.last().unwrap_or(&0);
        }

        self.is_exp_inf_sharp = is_inf_sharp(self.v_desc.vert_sharpness);
        self.is_exp_semi_sharp = is_semi_sharp(self.v_desc.vert_sharpness);

        self.tag.bits_mut().clear();

        let has_face_sizes = self.v_desc.has_incident_face_sizes();
        let common_size = self.common_face_size;
        let reg_size = self.reg_face_size;
        let exp_inf = self.is_exp_inf_sharp;
        let exp_semi = self.is_exp_semi_sharp;
        let is_manifold = self.v_desc.is_manifold;

        self.tag.bits_mut().set_un_common_face_sizes(has_face_sizes);
        self.tag
            .bits_mut()
            .set_irregular_face_sizes(common_size != 0 && common_size as u8 != reg_size);
        self.tag.bits_mut().set_inf_sharp_verts(exp_inf);
        self.tag.bits_mut().set_semi_sharp_verts(exp_semi);
        self.tag.bits_mut().set_un_ordered_faces(!is_manifold);

        if is_manifold {
            self.finalize_ordered_tags();
        }
    }

    /// Access the embedded VertexDescriptor for filling by SurfaceFactory subclass.
    pub fn get_vertex_descriptor_mut(&mut self) -> &mut VertexDescriptor {
        &mut self.v_desc
    }

    // ------------------------------------------------------------------
    //  Simple property queries
    // ------------------------------------------------------------------

    pub fn get_tag(&self) -> VertexTag {
        self.tag
    }

    /// Index of this face in the ring of incident faces around the vertex.
    pub fn get_face(&self) -> i32 {
        self.face_in_ring as i32
    }

    pub fn get_num_faces(&self) -> i32 {
        self.v_desc.num_faces as i32
    }
    pub fn get_num_face_vertices(&self) -> i32 {
        self.num_face_verts
    }

    pub fn has_common_face_size(&self) -> bool {
        self.common_face_size > 0
    }
    pub fn get_common_face_size(&self) -> i32 {
        self.common_face_size as i32
    }

    // ------------------------------------------------------------------
    //  Incident-face size / traversal
    // ------------------------------------------------------------------

    pub fn get_face_size(&self, face: i32) -> i32 {
        if self.common_face_size != 0 {
            self.common_face_size as i32
        } else {
            let off = &self.v_desc.face_size_offsets;
            (off[face as usize + 1] - off[face as usize]) as i32
        }
    }

    fn get_connected_face_next(&self, face: i32) -> i32 {
        self.face_edge_neighbors[2 * face as usize + 1] as i32
    }
    fn get_connected_face_prev(&self, face: i32) -> i32 {
        self.face_edge_neighbors[2 * face as usize] as i32
    }

    pub fn get_face_next(&self, face: i32) -> i32 {
        if self.is_un_ordered() {
            self.get_connected_face_next(face)
        } else if face < self.v_desc.num_faces as i32 - 1 {
            face + 1
        } else if self.is_boundary() {
            -1
        } else {
            0
        }
    }

    pub fn get_face_previous(&self, face: i32) -> i32 {
        if self.is_un_ordered() {
            self.get_connected_face_prev(face)
        } else if face > 0 {
            face - 1
        } else if self.is_boundary() {
            -1
        } else {
            self.v_desc.num_faces as i32 - 1
        }
    }

    pub fn get_face_after(&self, step: i32) -> i32 {
        debug_assert!(step >= 0);
        if self.is_ordered() {
            (self.face_in_ring as i32 + step) % self.v_desc.num_faces as i32
        } else if step == 1 {
            self.get_connected_face_next(self.face_in_ring as i32)
        } else if step == 2 {
            let f = self.get_connected_face_next(self.face_in_ring as i32);
            self.get_connected_face_next(f)
        } else {
            let mut f = self.face_in_ring as i32;
            for _ in 0..step {
                f = self.get_connected_face_next(f);
            }
            f
        }
    }

    pub fn get_face_before(&self, step: i32) -> i32 {
        debug_assert!(step >= 0);
        if self.is_ordered() {
            (self.face_in_ring as i32 - step + self.v_desc.num_faces as i32)
                % self.v_desc.num_faces as i32
        } else if step == 1 {
            self.get_connected_face_prev(self.face_in_ring as i32)
        } else if step == 2 {
            let f = self.get_connected_face_prev(self.face_in_ring as i32);
            self.get_connected_face_prev(f)
        } else {
            let mut f = self.face_in_ring as i32;
            for _ in 0..step {
                f = self.get_connected_face_prev(f);
            }
            f
        }
    }

    pub fn get_face_first(&self, subset: &FaceVertexSubset) -> i32 {
        self.get_face_before(subset.num_faces_before as i32)
    }
    pub fn get_face_last(&self, subset: &FaceVertexSubset) -> i32 {
        self.get_face_after(subset.num_faces_after as i32)
    }

    // ------------------------------------------------------------------
    //  Index accessors
    // ------------------------------------------------------------------

    pub fn get_face_index_offset(&self, face: i32) -> i32 {
        if self.common_face_size != 0 {
            face * self.common_face_size as i32
        } else {
            self.v_desc.face_size_offsets[face as usize] as i32
        }
    }

    /// Index of this face's corner vertex in `indices`.
    pub fn get_face_index_at_corner_self(&self, indices: &[Index]) -> Index {
        indices[self.get_face_index_offset(self.face_in_ring as i32) as usize]
    }

    pub fn get_face_index_at_corner(&self, face: i32, indices: &[Index]) -> Index {
        indices[self.get_face_index_offset(face) as usize]
    }

    /// Index of the leading vertex (position after corner) in incident face.
    pub fn get_face_index_leading(&self, face: i32, indices: &[Index]) -> Index {
        indices[self.get_face_index_offset(face) as usize + 1]
    }

    /// Index of the trailing vertex (position before corner) in incident face.
    pub fn get_face_index_trailing(&self, face: i32, indices: &[Index]) -> Index {
        // Safe: use face+1 offset -1 (wraps correctly even for last face)
        indices[self.get_face_index_offset(face + 1) as usize - 1]
    }

    pub fn face_indices_match_at_corner(&self, f1: i32, f2: i32, idx: &[Index]) -> bool {
        self.get_face_index_at_corner(f1, idx) == self.get_face_index_at_corner(f2, idx)
    }
    pub fn face_indices_match_at_edge_end(&self, f1: i32, f2: i32, idx: &[Index]) -> bool {
        self.get_face_index_trailing(f1, idx) == self.get_face_index_leading(f2, idx)
    }
    pub fn face_indices_match_across_edge(&self, f1: i32, f2: i32, idx: &[Index]) -> bool {
        self.face_indices_match_at_corner(f1, f2, idx)
            && self.face_indices_match_at_edge_end(f1, f2, idx)
    }

    // ------------------------------------------------------------------
    //  Sharpness
    // ------------------------------------------------------------------

    pub fn get_vertex_sharpness(&self) -> f32 {
        self.v_desc.vert_sharpness
    }

    /// Sharpness of face-edge by flat index (2*face + trailing).
    pub fn get_face_edge_sharpness_by_idx(&self, face_edge: i32) -> f32 {
        self.v_desc.face_edge_sharpness[face_edge as usize]
    }
    pub fn get_face_edge_sharpness(&self, face: i32, trailing: bool) -> f32 {
        self.v_desc.face_edge_sharpness[face as usize * 2 + trailing as usize]
    }

    pub fn is_face_edge_sharp(&self, face: i32, trailing: bool) -> bool {
        is_sharp(self.get_face_edge_sharpness(face, trailing))
    }
    pub fn is_face_edge_inf_sharp(&self, face: i32, trailing: bool) -> bool {
        is_inf_sharp(self.get_face_edge_sharpness(face, trailing))
    }
    pub fn is_face_edge_semi_sharp(&self, face: i32, trailing: bool) -> bool {
        is_semi_sharp(self.get_face_edge_sharpness(face, trailing))
    }

    pub fn has_implicit_vertex_sharpness(&self) -> bool {
        self.is_imp_inf_sharp || self.is_imp_semi_sharp
    }

    pub fn get_implicit_vertex_sharpness(&self) -> f32 {
        if self.is_imp_inf_sharp {
            return SHARPNESS_INFINITE;
        }
        debug_assert!(self.is_imp_semi_sharp);
        let mut sharpness = self.get_vertex_sharpness();
        for i in 0..self.get_num_faces() {
            if self.get_face_previous(i) >= 0 {
                let s = self.get_face_edge_sharpness_by_idx(2 * i);
                if s > sharpness {
                    sharpness = s;
                }
            }
        }
        sharpness
    }

    // ------------------------------------------------------------------
    //  Subset methods
    // ------------------------------------------------------------------

    pub fn get_vertex_subset(&self, subset: &mut FaceVertexSubset) -> i32 {
        if self.is_manifold() {
            self.init_complete_subset(subset);
        } else {
            self.find_connected_subset_extent(subset);
            self.adjust_subset_tags(subset, None);
            if !subset.is_sharp() && self.has_implicit_vertex_sharpness() {
                self.sharpen_subset_with(subset, self.get_implicit_vertex_sharpness());
            }
        }
        subset.num_faces_total as i32
    }

    pub fn find_face_varying_subset(
        &self,
        fvar_subset: &mut FaceVertexSubset,
        fvar_indices: &[Index],
        vtx_subset: &FaceVertexSubset,
    ) -> i32 {
        self.find_fvar_subset_extent(vtx_subset, fvar_subset, fvar_indices);

        let matches_vertex = fvar_subset.extent_matches_superset(vtx_subset);
        if !matches_vertex {
            if fvar_subset.is_sharp() {
                self.unsharpen_subset(fvar_subset);
            }
            self.adjust_subset_tags(fvar_subset, Some(vtx_subset));
        }

        // Sharpen if vertex is non-manifold
        if !fvar_subset.is_sharp() && !self.is_manifold() {
            self.sharpen_subset(fvar_subset);
        }

        // Sharpen if fvar is non-manifold (duplicate corner indices outside subset)
        if !fvar_subset.is_sharp() && (fvar_subset.get_num_faces() < vtx_subset.get_num_faces()) {
            let fvar_match = self.get_face_index_at_corner_self(fvar_indices);
            let mut num_matches = 0i32;
            for i in 0..self.get_num_faces() {
                if self.get_face_index_at_corner(i, fvar_indices) == fvar_match {
                    num_matches += 1;
                    if num_matches > fvar_subset.get_num_faces() {
                        self.sharpen_subset(fvar_subset);
                        break;
                    }
                }
            }
        }
        fvar_subset.get_num_faces()
    }

    pub fn sharpen_subset(&self, subset: &mut FaceVertexSubset) {
        subset.tag.bits_mut().set_inf_sharp_verts(true);
        subset.tag.bits_mut().set_semi_sharp_verts(false);
    }
    pub fn unsharpen_subset(&self, subset: &mut FaceVertexSubset) {
        subset
            .tag
            .bits_mut()
            .set_inf_sharp_verts(self.is_exp_inf_sharp);
        subset
            .tag
            .bits_mut()
            .set_semi_sharp_verts(self.is_exp_semi_sharp);
    }
    pub fn sharpen_subset_with(&self, subset: &mut FaceVertexSubset, sharpness: f32) {
        if sharpness > subset.local_sharpness {
            subset.local_sharpness = sharpness;
            subset
                .tag
                .bits_mut()
                .set_inf_sharp_verts(is_inf_sharp(sharpness));
            subset
                .tag
                .bits_mut()
                .set_semi_sharp_verts(is_semi_sharp(sharpness));
        }
    }

    // ------------------------------------------------------------------
    //  ConnectUnOrderedFaces
    // ------------------------------------------------------------------

    /// Connect unordered incident faces using their face-vertex indices.
    pub fn connect_un_ordered_faces(&mut self, fv_indices: &[Index]) {
        let num_face_edges = self.get_num_faces() * 2;
        self.face_edge_neighbors
            .resize(num_face_edges as usize, 0i16);

        // Build edge table
        let max_edges = num_face_edges as usize;
        let mut edges: Vec<Edge> = Vec::with_capacity(max_edges);
        let mut fe_edges: Vec<i16> = vec![0i16; num_face_edges as usize];

        let num_edges = self.create_un_ordered_edges(&mut edges, &mut fe_edges, fv_indices);
        self.mark_duplicate_edges(&mut edges, &fe_edges, fv_indices);
        self.assign_un_ordered_face_neighbors(&edges, &fe_edges);
        self.finalize_un_ordered_tags(&edges, num_edges, fv_indices);
    }

    // ------------------------------------------------------------------
    //  Private helpers
    // ------------------------------------------------------------------

    fn is_ordered(&self) -> bool {
        !self.tag.bits().un_ordered_faces()
    }
    fn is_un_ordered(&self) -> bool {
        self.tag.bits().un_ordered_faces()
    }
    fn is_boundary(&self) -> bool {
        self.tag.bits().boundary_verts()
    }
    fn is_interior(&self) -> bool {
        !self.tag.bits().boundary_verts()
    }
    fn is_manifold(&self) -> bool {
        !self.tag.bits().non_manifold_verts()
    }

    fn init_complete_subset(&self, subset: &mut FaceVertexSubset) -> i32 {
        let num_faces = self.get_num_faces();
        subset.initialize(self.tag);
        subset.num_faces_total = num_faces as i16;
        if self.is_interior() {
            subset.num_faces_before = 0;
            subset.num_faces_after = (num_faces - 1) as i16;
        } else if self.is_ordered() {
            subset.num_faces_before = self.face_in_ring;
            subset.num_faces_after = (num_faces - 1 - self.face_in_ring as i32) as i16;
        } else {
            // Unordered boundary: count forward
            let mut count_after: i16 = 0;
            let mut f = self.get_face_next(self.face_in_ring as i32);
            while f >= 0 {
                count_after += 1;
                f = self.get_face_next(f);
            }
            subset.num_faces_after = count_after;
            subset.num_faces_before = (num_faces - 1 - count_after as i32) as i16;
        }
        subset.num_faces_total as i32
    }

    fn find_connected_subset_extent(&self, subset: &mut FaceVertexSubset) -> i32 {
        subset.initialize(self.tag);
        subset.tag.bits_mut().set_non_manifold_verts(false);

        let f_start = self.face_in_ring as i32;

        let mut f = self.get_face_next(f_start);
        while f >= 0 {
            if f == f_start {
                // Periodic
                subset.set_boundary(false);
                return subset.num_faces_total as i32;
            }
            subset.num_faces_after += 1;
            subset.num_faces_total += 1;
            f = self.get_face_next(f);
        }
        let mut f = self.get_face_previous(f_start);
        while f >= 0 {
            subset.num_faces_before += 1;
            subset.num_faces_total += 1;
            f = self.get_face_previous(f);
        }
        subset.set_boundary(true);
        subset.num_faces_total as i32
    }

    fn find_fvar_subset_extent(
        &self,
        vtx_sub: &FaceVertexSubset,
        fvar_sub: &mut FaceVertexSubset,
        fvar_indices: &[Index],
    ) -> i32 {
        fvar_sub.initialize(vtx_sub.tag);
        fvar_sub.set_boundary(true);

        if vtx_sub.num_faces_total == 1 {
            return 1;
        }

        let corner_face = self.face_in_ring as i32;

        // Gather faces "after"
        let num_after = vtx_sub.num_faces_after as i32;
        if num_after > 0 {
            let mut this_face = corner_face;
            let mut next_face = self.get_face_next(this_face);
            for _ in 0..num_after {
                if !self.face_indices_match_across_edge(this_face, next_face, fvar_indices) {
                    break;
                }
                fvar_sub.num_faces_after += 1;
                fvar_sub.num_faces_total += 1;
                this_face = next_face;
                next_face = self.get_face_next(this_face);
            }
            if next_face == corner_face {
                debug_assert_eq!(vtx_sub.num_faces_before, 0);
                if self.face_indices_match_at_edge_end(this_face, corner_face, fvar_indices) {
                    fvar_sub.set_boundary(false);
                }
                return fvar_sub.num_faces_total as i32;
            }
        }

        // Gather faces "before"
        let mut num_before = vtx_sub.num_faces_before as i32;
        if !vtx_sub.is_boundary() {
            num_before += vtx_sub.num_faces_after as i32 - fvar_sub.num_faces_after as i32;
        }
        if num_before > 0 {
            let mut this_face = corner_face;
            let mut prev_face = self.get_face_previous(this_face);
            for _ in 0..num_before {
                if !self.face_indices_match_across_edge(prev_face, this_face, fvar_indices) {
                    break;
                }
                fvar_sub.num_faces_before += 1;
                fvar_sub.num_faces_total += 1;
                this_face = prev_face;
                prev_face = self.get_face_previous(this_face);
            }
        }
        fvar_sub.num_faces_total as i32
    }

    fn adjust_subset_tags(
        &self,
        subset: &mut FaceVertexSubset,
        superset: Option<&FaceVertexSubset>,
    ) {
        {
            let bits = subset.tag.bits_mut();
            if bits.boundary_verts() {
                bits.set_inf_sharp_darts(false);
            }
            if bits.inf_sharp_verts() {
                bits.set_semi_sharp_verts(false);
            }
        }

        let (num_super_faces, super_boundary) = if let Some(sup) = superset {
            (sup.get_num_faces(), sup.is_boundary())
        } else {
            (self.get_num_faces(), self.is_boundary())
        };

        if subset.get_num_faces() < num_super_faces || subset.is_boundary() != super_boundary {
            let irr_flag = subset.tag.bits().irregular_face_sizes();
            let ise_flag = subset.tag.bits().inf_sharp_edges();
            let sse_flag = subset.tag.bits().semi_sharp_edges();

            if irr_flag {
                let irr = self.subset_has_irregular_faces(subset);
                subset.tag.bits_mut().set_irregular_face_sizes(irr);
            }
            if ise_flag {
                let ise = self.subset_has_inf_sharp_edges(subset);
                subset.tag.bits_mut().set_inf_sharp_edges(ise);
                if ise && subset.is_boundary() {
                    self.sharpen_subset(subset);
                }
            }
            if sse_flag {
                let sse = self.subset_has_semi_sharp_edges(subset);
                subset.tag.bits_mut().set_semi_sharp_edges(sse);
            }
        }
    }

    fn subset_has_irregular_faces(&self, subset: &FaceVertexSubset) -> bool {
        if !self.tag.bits().un_common_face_sizes() {
            return true;
        }
        let mut f = self.get_face_first(subset);
        for _ in 0..subset.get_num_faces() {
            if self.get_face_size(f) != self.reg_face_size as i32 {
                return true;
            }
            f = self.get_face_next(f);
        }
        false
    }

    fn subset_has_inf_sharp_edges(&self, subset: &FaceVertexSubset) -> bool {
        let n = subset.get_num_faces();
        if n > 1 {
            let mut f = self.get_face_first(subset);
            let start = if subset.is_boundary() { 1 } else { 0 };
            for _ in start..n {
                if self.is_face_edge_inf_sharp(f, true) {
                    return true;
                }
                f = self.get_face_next(f);
            }
        }
        false
    }

    fn subset_has_semi_sharp_edges(&self, subset: &FaceVertexSubset) -> bool {
        let n = subset.get_num_faces();
        if n > 1 {
            let mut f = self.get_face_first(subset);
            let start = if subset.is_boundary() { 1 } else { 0 };
            for _ in start..n {
                if self.is_face_edge_semi_sharp(f, true) {
                    return true;
                }
                f = self.get_face_next(f);
            }
        }
        false
    }

    fn finalize_ordered_tags(&mut self) {
        self.tag.bits_mut().set_un_ordered_faces(false);
        self.tag.bits_mut().set_non_manifold_verts(false);
        self.tag
            .bits_mut()
            .set_boundary_verts(self.v_desc.is_boundary);
        self.tag
            .bits_mut()
            .set_boundary_non_sharp(self.v_desc.is_boundary);

        if self.v_desc.has_edge_sharpness() {
            let is_boundary = self.v_desc.is_boundary;
            let num_faces = self.v_desc.num_faces as usize;

            if is_boundary {
                let last = 2 * num_faces - 1;
                let s0 = self.v_desc.face_edge_sharpness[0];
                let sl = self.v_desc.face_edge_sharpness[last];
                let non_sharp = !is_inf_sharp(s0) || !is_inf_sharp(sl);
                self.tag.bits_mut().set_boundary_non_sharp(non_sharp);
            }

            let mut num_inf = 0i32;
            let mut num_semi = 0i32;
            let start = if is_boundary { 1 } else { 0 };
            for i in start..num_faces {
                let s = self.v_desc.face_edge_sharpness[2 * i];
                if is_inf_sharp(s) {
                    num_inf += 1;
                } else if is_sharp(s) {
                    num_semi += 1;
                }
            }

            self.tag.bits_mut().set_inf_sharp_edges(num_inf > 0);
            self.tag.bits_mut().set_semi_sharp_edges(num_semi > 0);
            self.tag
                .bits_mut()
                .set_inf_sharp_darts(num_inf == 1 && !is_boundary);

            let num_inf_total = num_inf + if is_boundary { 2 } else { 0 };
            if num_inf_total > 2 {
                self.is_imp_inf_sharp = true;
            } else if num_inf_total + num_semi > 2 {
                self.is_imp_semi_sharp = true;
            }

            if !self.is_exp_inf_sharp && self.is_imp_inf_sharp {
                self.tag.bits_mut().set_inf_sharp_verts(true);
                self.tag.bits_mut().set_semi_sharp_verts(false);
            }
        }
    }

    // ------------------------------------------------------------------
    //  UnOrdered face connectivity internals
    // ------------------------------------------------------------------

    fn create_un_ordered_edges(
        &self,
        edges: &mut Vec<Edge>,
        fe_edges: &mut [i16],
        fv_indices: &[Index],
    ) -> usize {
        let num_faces = self.get_num_faces() as usize;
        let num_face_edges = num_faces * 2;
        let v_corner = fv_indices[0];
        let has_sharpness = self.v_desc.has_edge_sharpness();

        for fe_index in 0..num_face_edges {
            let face = fe_index / 2;
            let trailing = (fe_index & 1) != 0;
            let v_index = if trailing {
                self.get_face_index_trailing(face as i32, fv_indices)
            } else {
                self.get_face_index_leading(face as i32, fv_indices)
            };

            let e_index = if v_index != v_corner {
                // Find existing edge or create new
                let found = edges.iter().position(|e| e.end_vertex == v_index);
                if let Some(idx) = found {
                    edges[idx].add_face(face as i32, trailing);
                    idx
                } else {
                    let idx = edges.len();
                    let mut e = Edge::new(v_index);
                    e.set_boundary();
                    e.set_face(face as i32, trailing);
                    if has_sharpness {
                        e.set_sharpness(self.get_face_edge_sharpness_by_idx(fe_index as i32));
                    }
                    edges.push(e);
                    idx
                }
            } else {
                // Degenerate self-edge
                let idx = edges.len();
                let mut e = Edge::new(v_index);
                e.set_degenerate();
                edges.push(e);
                idx
            };

            fe_edges[fe_index] = e_index as i16;
        }
        edges.len()
    }

    fn mark_duplicate_edges(&self, edges: &mut Vec<Edge>, fe_edges: &[i16], fv_indices: &[Index]) {
        if self.common_face_size == 3 {
            return;
        }

        let v_corner = fv_indices[0];
        let num_faces = self.get_num_faces() as usize;

        if self.common_face_size == 4 {
            let mut fv_opp = 2usize; // offset to opposite vertex
            for face in 0..num_faces {
                if fv_indices[fv_opp] == v_corner {
                    edges[fe_edges[2 * face] as usize].set_duplicate();
                    edges[fe_edges[2 * face + 1] as usize].set_duplicate();
                }
                fv_opp += 4;
            }
        } else {
            let mut fv_offset = 0usize;
            for face in 0..num_faces {
                let face_size = self.get_face_size(face as i32) as usize;
                let fv = &fv_indices[fv_offset..fv_offset + face_size];
                if face_size == 4 {
                    if fv[2] == v_corner {
                        edges[fe_edges[2 * face] as usize].set_duplicate();
                        edges[fe_edges[2 * face + 1] as usize].set_duplicate();
                    }
                } else {
                    for j in 2..face_size.saturating_sub(2) {
                        if fv[j] == v_corner {
                            if j >= 1 && fv[j - 1] == fv[1] {
                                edges[fe_edges[2 * face] as usize].set_duplicate();
                            }
                            if j + 1 < face_size && fv[j + 1] == fv[face_size - 1] {
                                edges[fe_edges[2 * face + 1] as usize].set_duplicate();
                            }
                        }
                    }
                }
                fv_offset += face_size;
            }
        }
    }

    fn assign_un_ordered_face_neighbors(&mut self, edges: &[Edge], fe_edges: &[i16]) {
        let num_face_edges = fe_edges.len();
        for i in 0..num_face_edges {
            let e = &edges[fe_edges[i] as usize];
            if e.non_manifold || e.boundary {
                self.face_edge_neighbors[i] = -1;
            } else {
                let trailing = (i & 1) != 0;
                self.face_edge_neighbors[i] = if trailing { e.next_face } else { e.prev_face };
            }
        }
    }

    fn finalize_un_ordered_tags(&mut self, edges: &[Edge], num_edges: usize, fv_indices: &[Index]) {
        let mut num_non_manifold = 0i32;
        let mut num_inf_sharp = 0i32;
        let mut num_semi_sharp = 0i32;
        let mut num_singular = 0i32;
        let mut has_boundary = false;
        let mut has_boundary_not_sharp = false;
        let mut has_degenerate = false;
        let mut has_duplicate = false;

        for e in &edges[..num_edges] {
            if e.interior {
                num_inf_sharp += e.inf_sharp as i32;
                num_semi_sharp += e.semi_sharp as i32;
            } else if e.boundary {
                has_boundary = true;
                has_boundary_not_sharp |= !e.inf_sharp;
            } else {
                num_non_manifold += 1;
                has_degenerate |= e.degenerate;
                has_duplicate |= e.duplicate;
            }
            num_singular += (e.non_manifold || e.boundary || e.inf_sharp) as i32;
        }

        let mut is_non_manifold;
        let mut is_non_manifold_crease = false;

        if num_non_manifold > 0 {
            is_non_manifold = true;
            if !has_degenerate && !has_duplicate && !has_boundary {
                is_non_manifold_crease = !self.is_exp_inf_sharp
                    && num_non_manifold == 2
                    && self.get_num_faces() > num_edges as i32
                    && self.test_non_manifold_crease(edges, num_edges, fv_indices);
            }
        } else {
            is_non_manifold = (num_edges as i32 - self.get_num_faces()) != has_boundary as i32;
            if !is_non_manifold {
                let mut tmp = FaceVertexSubset::default();
                let n = self.find_connected_subset_extent(&mut tmp);
                if n < self.get_num_faces() {
                    is_non_manifold = true;
                }
            }
        }

        let bits = self.tag.bits_mut();
        bits.set_non_manifold_verts(is_non_manifold);
        bits.set_boundary_verts(has_boundary);
        bits.set_boundary_non_sharp(has_boundary_not_sharp);
        bits.set_inf_sharp_edges(num_inf_sharp > 0);
        bits.set_semi_sharp_edges(num_semi_sharp > 0);
        bits.set_inf_sharp_darts(num_inf_sharp == 1 && !has_boundary);

        if (num_singular > 2) || (is_non_manifold && !is_non_manifold_crease) {
            self.is_imp_inf_sharp = true;
        } else if num_singular + num_semi_sharp > 2 {
            self.is_imp_semi_sharp = true;
        }

        if !self.is_exp_inf_sharp && self.is_imp_inf_sharp {
            bits.set_inf_sharp_verts(true);
            bits.set_semi_sharp_verts(false);
        }
    }

    fn test_non_manifold_crease(
        &self,
        edges: &[Edge],
        num_edges: usize,
        fv_indices: &[Index],
    ) -> bool {
        // Collect the two non-manifold edge end vertices
        let mut crease_end = [-1i32; 2];
        let mut ce_count = 0;
        for e in &edges[..num_edges] {
            if e.non_manifold {
                if ce_count < 2 {
                    crease_end[ce_count] = e.end_vertex;
                    ce_count += 1;
                }
            }
        }
        if crease_end[0] < 0 || crease_end[1] < 0 {
            return false;
        }

        // Build face-corner leading/trailing vertex arrays
        let num_faces = self.get_num_faces() as usize;
        let mut leading = vec![-1i32; num_faces];
        let mut trailing = vec![-1i32; num_faces];
        let mut size = num_faces;

        for i in 0..num_faces {
            leading[i] = self.get_face_index_leading(i as i32, fv_indices);
            trailing[i] = self.get_face_index_trailing(i as i32, fv_indices);
        }

        // Remove manifold subsets in each direction
        let remove_subset =
            |lv: &mut Vec<i32>, tv: &mut Vec<i32>, sz: &mut usize, start: i32, end: i32| -> i32 {
                if start == end {
                    return -1;
                }
                let mut next = start;
                loop {
                    let pos = lv[..*sz].iter().position(|&x| x == next);
                    if let Some(p) = pos {
                        next = tv[p];
                        lv.swap(p, *sz - 1);
                        tv.swap(p, *sz - 1);
                        *sz -= 1;
                        if next == end {
                            return 1;
                        }
                        if next == start {
                            return -1;
                        }
                    } else {
                        return if next == start { 0 } else { -1 };
                    }
                }
            };

        loop {
            let r = remove_subset(
                &mut leading,
                &mut trailing,
                &mut size,
                crease_end[0],
                crease_end[1],
            );
            if r < 0 {
                return false;
            }
            if r == 0 {
                break;
            }
        }
        if size == 0 {
            return true;
        }
        loop {
            let r = remove_subset(
                &mut leading,
                &mut trailing,
                &mut size,
                crease_end[1],
                crease_end[0],
            );
            if r < 0 {
                return false;
            }
            if r == 0 {
                break;
            }
        }
        size == 0
    }
}

// ---------------------------------------------------------------------------
//  Internal Edge struct for unordered face connectivity
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Edge {
    end_vertex: i32,
    boundary: bool,
    interior: bool,
    non_manifold: bool,
    trailing: bool,
    degenerate: bool,
    duplicate: bool,
    inf_sharp: bool,
    semi_sharp: bool,
    prev_face: i16,
    next_face: i16,
}

impl Edge {
    fn new(end_vertex: i32) -> Self {
        Self {
            end_vertex,
            ..Default::default()
        }
    }
    fn set_boundary(&mut self) {
        self.boundary = true;
    }
    fn set_interior(&mut self) {
        self.boundary = false;
        self.interior = true;
    }
    fn set_non_manifold(&mut self) {
        self.boundary = false;
        self.interior = false;
        self.non_manifold = true;
    }
    fn set_degenerate(&mut self) {
        self.set_non_manifold();
        self.degenerate = true;
    }
    fn set_duplicate(&mut self) {
        self.set_non_manifold();
        self.duplicate = true;
    }

    fn set_sharpness(&mut self, s: f32) {
        if is_inf_sharp(s) {
            self.inf_sharp = true;
        } else if is_sharp(s) {
            self.semi_sharp = true;
        }
    }

    fn set_face(&mut self, face: i32, new_trailing: bool) {
        self.trailing = new_trailing;
        if new_trailing {
            self.prev_face = face as i16;
        } else {
            self.next_face = face as i16;
        }
    }

    fn add_face(&mut self, face: i32, new_trailing: bool) {
        if self.boundary {
            if new_trailing == self.trailing
                || (face
                    == if self.trailing {
                        self.prev_face
                    } else {
                        self.next_face
                    } as i32)
            {
                self.set_non_manifold();
            } else {
                self.set_interior();
                self.set_face(face, new_trailing);
            }
        } else if self.interior {
            self.set_non_manifold();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_vertex_initialize() {
        let mut fv = FaceVertex::new();
        fv.initialize(4, 4);
        assert_eq!(fv.common_face_size, 4);
        assert_eq!(fv.reg_face_size, 4);
    }

    #[test]
    fn edge_state_transitions() {
        let mut e = Edge::new(42);
        e.set_boundary();
        assert!(e.boundary);
        e.set_interior();
        assert!(e.interior && !e.boundary);
        e.set_non_manifold();
        assert!(e.non_manifold && !e.interior && !e.boundary);
    }
}
