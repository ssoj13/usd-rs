#![allow(dangerous_implicit_autorefs)]
// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/fvarRefinement.h/.cpp

//! Face-varying refinement data for a single channel.
//!
//! Maintains the mapping between parent and child FVarLevel values during
//! refinement, analogous to how `Refinement` maps Level components.

use crate::sdc::crease::Crease;
use super::types::{Index, LocalIndex};
use super::level::Level;
use super::refinement::Refinement;
use super::fvar_level::{FVarLevel, ValueTag, CreaseEndPair};

/// Face-varying refinement between a parent and child FVarLevel.
///
/// Mirrors C++ `Vtr::internal::FVarRefinement`.
/// Uses raw pointers to Refinement, parent/child Levels and FVarLevels
/// to avoid borrow-checker conflicts during simultaneous read/write.
pub struct FVarRefinement {
    pub channel: i32,

    refinement:   *const Refinement,
    parent_level: *const Level,
    parent_fvar:  *const FVarLevel,
    child_level:  *const Level,
    child_fvar:   *mut   FVarLevel,

    /// Maps each child vertex-value to the sibling index within the parent.
    /// UnsafeCell allows interior mutability through &self methods (matches
    /// the raw-pointer ownership model used for all other fields).
    child_value_parent_source: std::cell::UnsafeCell<Vec<LocalIndex>>,
}

impl FVarRefinement {
    /// Placeholder constructor (channel index only, pointers unset).
    /// Used by `Refinement::subdivide_fvar_channels()`.
    pub fn new(channel: i32) -> Self {
        Self {
            channel,
            refinement: std::ptr::null(),
            parent_level: std::ptr::null(),
            parent_fvar: std::ptr::null(),
            child_level: std::ptr::null(),
            child_fvar: std::ptr::null_mut(),
            child_value_parent_source: std::cell::UnsafeCell::new(Vec::new()),
        }
    }

    /// Full constructor linking parent and child FVarLevels through a Refinement.
    pub fn with_refs(
        refinement: &Refinement,
        parent_fvar: &FVarLevel,
        child_fvar: &mut FVarLevel,
        channel: i32,
    ) -> Self {
        Self {
            channel,
            refinement: refinement as *const Refinement,
            parent_level: refinement.parent() as *const Level,
            parent_fvar: parent_fvar as *const FVarLevel,
            child_level: refinement.child() as *const Level,
            child_fvar: child_fvar as *mut FVarLevel,
            child_value_parent_source: std::cell::UnsafeCell::new(Vec::new()),
        }
    }

    /// Bind the raw pointers after construction.
    pub fn bind(
        &mut self,
        refinement: &Refinement,
        parent_fvar: &FVarLevel,
        child_fvar: &mut FVarLevel,
    ) {
        self.refinement = refinement as *const Refinement;
        self.parent_level = refinement.parent() as *const Level;
        self.parent_fvar = parent_fvar as *const FVarLevel;
        self.child_level = refinement.child() as *const Level;
        self.child_fvar = child_fvar as *mut FVarLevel;
    }

    // -- Raw-pointer accessors (bypass borrow checker) --

    #[inline]
    fn r(&self) -> &Refinement { unsafe { &*self.refinement } }
    #[inline]
    fn pl(&self) -> &Level { unsafe { &*self.parent_level } }
    #[inline]
    fn pf(&self) -> &FVarLevel { unsafe { &*self.parent_fvar } }
    #[inline]
    fn cl(&self) -> &Level { unsafe { &*self.child_level } }
    #[inline]
    fn cf(&self) -> &FVarLevel { unsafe { &*self.child_fvar } }
    #[inline]
    fn cf_mut(&self) -> &mut FVarLevel { unsafe { &mut *self.child_fvar } }

    // -----------------------------------------------------------------------
    // Public queries
    // -----------------------------------------------------------------------

    /// Get the parent source (sibling index) for a child vertex value.
    pub fn get_child_value_parent_source(&self, v_index: Index, sibling: i32) -> i32 {
        let offset = self.cf().get_vertex_value_offset(v_index, sibling as LocalIndex);
        unsafe { (*self.child_value_parent_source.get())[offset as usize] as i32 }
    }

    /// Compute the fractional weight for blending a semi-sharp value.
    pub fn get_fractional_weight(
        &self,
        p_vert: Index, p_sibling: LocalIndex,
        c_vert: Index, _c_sibling: LocalIndex,
    ) -> f32 {
        let pl = self.pl();
        let cl = self.cl();
        let pf = self.pf();
        let refinement = self.r();

        let p_vert_edges = pl.get_vertex_edges(p_vert);

        // Build child vert-edges (full topology may be absent)
        let c_vert_edges: Vec<Index> = if cl.get_num_vertex_edges_total() > 0 {
            cl.get_vertex_edges(c_vert).as_slice().to_vec()
        } else {
            let p_in_edge = pl.get_vertex_edge_local_indices(p_vert);
            (0..p_vert_edges.size()).map(|i| {
                let ee = refinement.get_edge_child_edges(p_vert_edges[i as usize]);
                ee[p_in_edge[i as usize] as usize]
            }).collect()
        };

        // Gather edge sharpness within the value's crease span
        let p_crease_ends = pf.get_vertex_value_crease_ends(p_vert);
        let p_start = p_crease_ends[p_sibling as usize].start_face as i32;
        let p_end   = p_crease_ends[p_sibling as usize].end_face as i32;
        let n_edges = p_vert_edges.size();

        let mut p_sharp = Vec::new();
        let mut c_sharp = Vec::new();

        if p_end > p_start {
            for i in (p_start + 1)..=p_end {
                p_sharp.push(pl.get_edge_sharpness(p_vert_edges[i as usize]));
                c_sharp.push(cl.get_edge_sharpness(c_vert_edges[i as usize]));
            }
        } else if p_start > p_end {
            for i in (p_start + 1)..n_edges {
                p_sharp.push(pl.get_edge_sharpness(p_vert_edges[i as usize]));
                c_sharp.push(cl.get_edge_sharpness(c_vert_edges[i as usize]));
            }
            for i in 0..=p_end {
                p_sharp.push(pl.get_edge_sharpness(p_vert_edges[i as usize]));
                c_sharp.push(cl.get_edge_sharpness(c_vert_edges[i as usize]));
            }
        }

        Crease::with_options(refinement.get_options()).compute_fractional_weight_at_vertex(
            pl.get_vertex_sharpness(p_vert),
            cl.get_vertex_sharpness(c_vert),
            &p_sharp,
            Some(&c_sharp),
        )
    }

    // -----------------------------------------------------------------------
    // Main entry point
    // -----------------------------------------------------------------------

    /// Apply the full face-varying refinement: allocate child values,
    /// populate, propagate tags, and finalize.
    pub fn apply_refinement(&mut self) {
        // Transfer basic properties from parent to child
        let cf = self.cf_mut();
        let pf = self.pf();
        cf.options                = pf.options;
        cf.is_linear              = pf.is_linear;
        cf.has_linear_boundaries  = pf.has_linear_boundaries;
        cf.has_dependent_sharpness = pf.has_dependent_sharpness;

        self.estimate_and_alloc_child_values();
        self.populate_child_values();
        self.trim_and_finalize_child_values();

        self.propagate_edge_tags();
        self.propagate_value_tags();

        if self.cf().has_smooth_boundaries() {
            self.propagate_value_creases();
            self.reclassify_semisharp_values();
        }

        // Build redundant face-values as post-process
        if self.cf().get_num_values() > self.cl().get_num_vertices() {
            self.cf_mut().initialize_face_values_from_vertex_face_siblings();
        } else {
            self.cf_mut().initialize_face_values_from_face_vertices();
        }
    }

    // -----------------------------------------------------------------------
    // Allocation
    // -----------------------------------------------------------------------

    /// Estimate maximum child values and pre-allocate.
    fn estimate_and_alloc_child_values(&mut self) {
        let r = self.r();
        let pl = self.pl();
        let pf = self.pf();

        let mut max_count = r.get_num_child_vertices_from_faces();

        // Edge-vertices
        let ev_start = r.get_first_child_vertex_from_edges();
        let ev_end   = ev_start + r.get_num_child_vertices_from_edges();
        for cv in ev_start..ev_end {
            let pe = r.get_child_vertex_parent_index(cv);
            max_count += if pf.edge_topology_matches(pe) {
                1
            } else {
                pl.get_edge_faces(pe).size()
            };
        }

        // Vertex-vertices
        let vv_start = r.get_first_child_vertex_from_vertices();
        let vv_end   = vv_start + r.get_num_child_vertices_from_vertices();
        for cv in vv_start..vv_end {
            debug_assert!(r.is_child_vertex_complete(cv));
            let pv = r.get_child_vertex_parent_index(cv);
            max_count += pf.get_num_vertex_values(pv);
        }

        // Resize child to match child Level
        self.cf_mut().resize_components();

        // Allocate value tags and parent-source to estimated max
        self.cf_mut().vert_value_tags.resize(max_count as usize, ValueTag::default());
        self.child_value_parent_source.get_mut().resize(max_count as usize, 0);
    }

    /// Trim over-allocated arrays to the actual value count and fill indices.
    fn trim_and_finalize_child_values(&mut self) {
        let vc = self.cf().value_count;
        let has_smooth = self.cf().has_smooth_boundaries();

        let cf = self.cf_mut();
        cf.vert_value_tags.resize(vc as usize, ValueTag::default());
        if has_smooth {
            cf.vert_value_crease_ends.resize(vc as usize, CreaseEndPair::default());
        }

        self.child_value_parent_source.get_mut().resize(vc as usize, 0);

        let cf = self.cf_mut();
        cf.vert_value_indices.resize(vc as usize, 0);
        for i in 0..vc {
            cf.vert_value_indices[i as usize] = i;
        }
    }

    // -----------------------------------------------------------------------
    // Populate child values
    // -----------------------------------------------------------------------

    /// Populate all child values in the same vertex ordering as Refinement.
    fn populate_child_values(&mut self) {
        self.cf_mut().value_count = 0;

        if self.r().has_face_vertices_first() {
            self.populate_from_face_verts();
            self.populate_from_edge_verts();
            self.populate_from_vert_verts();
        } else {
            self.populate_from_vert_verts();
            self.populate_from_face_verts();
            self.populate_from_edge_verts();
        }
    }

    /// Face-vertices: each produces exactly one child value.
    fn populate_from_face_verts(&self) {
        let r = self.r();
        let start = r.get_first_child_vertex_from_faces();
        let end   = start + r.get_num_child_vertices_from_faces();
        let cf = self.cf_mut();

        for cv in start..end {
            cf.vert_sibling_offsets[cv as usize] = cf.value_count;
            cf.vert_sibling_counts[cv as usize] = 1;
            cf.value_count += 1;
        }
    }

    /// Edge-vertices: 1 value if edge matches, else split per incident face.
    fn populate_from_edge_verts(&self) {
        let r = self.r();
        let pf = self.pf();
        let start = r.get_first_child_vertex_from_edges();
        let end   = start + r.get_num_child_vertices_from_edges();

        for cv in start..end {
            let pe = r.get_child_vertex_parent_index(cv);
            let cf = self.cf_mut();
            cf.vert_sibling_offsets[cv as usize] = cf.value_count;

            if pf.edge_topology_matches(pe) {
                cf.vert_sibling_counts[cv as usize] = 1;
                cf.value_count += 1;
            } else {
                let count = self.populate_for_edge_vertex(cv, pe);
                let cf = self.cf_mut();
                cf.vert_sibling_counts[cv as usize] = count as LocalIndex;
                cf.value_count += count;
            }
        }
    }

    /// Vertex-vertices: 1 value if topology matches, else propagate siblings.
    fn populate_from_vert_verts(&self) {
        let r = self.r();
        let pf = self.pf();
        let start = r.get_first_child_vertex_from_vertices();
        let end   = start + r.get_num_child_vertices_from_vertices();

        for cv in start..end {
            let pv = r.get_child_vertex_parent_index(cv);
            let cf = self.cf_mut();
            cf.vert_sibling_offsets[cv as usize] = cf.value_count;

            let pv_offset = pf.get_vertex_value_offset(pv, 0);
            if pf.value_topology_matches(pv_offset) {
                cf.vert_sibling_counts[cv as usize] = 1;
                cf.value_count += 1;
            } else {
                let count = self.populate_for_vertex_vertex(cv, pv);
                let cf = self.cf_mut();
                cf.vert_sibling_counts[cv as usize] = count as LocalIndex;
                cf.value_count += count;
            }
        }
    }

    /// Populate child values for a discontinuous edge-vertex.
    /// Returns the number of child values created.
    fn populate_for_edge_vertex(&self, c_vert: Index, p_edge: Index) -> i32 {
        let pl = self.pl();
        let p_edge_faces = pl.get_edge_faces(p_edge);
        let n_faces = p_edge_faces.size();

        if n_faces == 1 {
            return 1;
        }

        // Snapshot parent face indices
        let pef: Vec<Index> = p_edge_faces.as_slice().to_vec();

        let c_val_offset = self.cf().get_vertex_value_offset(c_vert, 0);

        // Update parent-source for all child values
        for i in 0..n_faces {
            self.child_value_parent_source_mut()[(c_val_offset + i) as usize] = i as LocalIndex;
        }

        // Update vertex-face siblings in the child
        let r = self.r();
        let cl = self.cl();
        let c_vert_faces: Vec<Index> = cl.get_vertex_faces(c_vert).as_slice().to_vec();
        let c_sib_offset = vf_offset(cl, c_vert);

        let cf = self.cf_mut();
        for i in 0..c_vert_faces.len() {
            let p_face = r.get_child_face_parent_face(c_vert_faces[i]);
            if n_faces == 2 {
                // Two parent faces: sibling 0 is default, set 1 for second face
                if p_face == pef[1] {
                    cf.vert_face_siblings[c_sib_offset + i] = 1;
                }
            } else {
                // Non-manifold: match child face to parent face
                for j in 0..pef.len() {
                    if p_face == pef[j] {
                        cf.vert_face_siblings[c_sib_offset + i] = j as LocalIndex;
                    }
                }
            }
        }

        n_faces
    }

    /// Populate child values for a vertex-vertex with multiple values.
    /// Returns the number of child values created.
    fn populate_for_vertex_vertex(&self, c_vert: Index, p_vert: Index) -> i32 {
        debug_assert!(self.r().is_child_vertex_complete(c_vert));

        let pf = self.pf();
        let c_value_count = pf.get_num_vertex_values(p_vert);

        if c_value_count > 1 {
            let c_val_idx = self.cf().get_vertex_value_offset(c_vert, 0);

            // Set parent source for non-primary values
            for j in 1..c_value_count {
                self.child_value_parent_source_mut()[(c_val_idx + j) as usize] = j as LocalIndex;
            }

            // Copy vertex-face siblings from parent to child
            let p_sibs: Vec<LocalIndex> = pf.get_vertex_face_siblings(p_vert).as_slice().to_vec();
            let cl = self.cl();
            let c_sib_offset = vf_offset(cl, c_vert);
            let c_sib_count = cl.get_num_vertex_faces(c_vert) as usize;

            let cf = self.cf_mut();
            for j in 0..c_sib_count.min(p_sibs.len()) {
                cf.vert_face_siblings[c_sib_offset + j] = p_sibs[j];
            }
        }

        c_value_count
    }

    /// Mutable access to child_value_parent_source via UnsafeCell.
    #[inline]
    fn child_value_parent_source_mut(&self) -> &mut Vec<LocalIndex> {
        unsafe { &mut *self.child_value_parent_source.get() }
    }

    // -----------------------------------------------------------------------
    // Tag propagation
    // -----------------------------------------------------------------------

    /// Propagate per-edge FVar tags from parent to child edges.
    pub fn propagate_edge_tags(&self) {
        let r = self.r();
        let n_from_faces = r.get_num_child_edges_from_faces();
        let n_child_edges = self.cl().get_num_edges();

        let e_tag_match = super::fvar_level::ETag::default();
        let cf = self.cf_mut();

        // Face-edges: all continuous (matching)
        for e in 0..n_from_faces {
            cf.edge_tags[e as usize] = e_tag_match;
        }

        // Edge-edges: inherit from parent
        let pf = self.pf();
        for e in n_from_faces..n_child_edges {
            let pe = r.get_child_edge_parent_index(e);
            cf.edge_tags[e as usize] = pf.edge_tags[pe as usize];
        }
    }

    /// Propagate per-value tags from parent to child vertex-values.
    pub fn propagate_value_tags(&self) {
        let r = self.r();
        let pf = self.pf();
        let cf = self.cf_mut();
        let val_match = ValueTag::default();

        // -- Face-vertices: all matching, sequential --
        let fv_start = r.get_first_child_vertex_from_faces();
        let fv_end   = fv_start + r.get_num_child_vertices_from_faces();
        let mut c_val = cf.vert_sibling_offsets[fv_start as usize];
        for _ in fv_start..fv_end {
            cf.vert_value_tags[c_val as usize] = val_match;
            c_val += 1;
        }

        // -- Edge-vertices: mismatch edges get split tags --
        let mut val_mismatch = val_match;
        val_mismatch.mismatch = true;

        let mut val_crease = val_mismatch;
        val_crease.crease = true;

        let val_split = if pf.has_smooth_boundaries() { val_crease } else { val_mismatch };

        let ev_start = r.get_first_child_vertex_from_edges();
        let ev_end   = ev_start + r.get_num_child_vertices_from_edges();

        for cv in ev_start..ev_end {
            let pe = r.get_child_vertex_parent_index(cv);
            let pe_tag = pf.edge_tags[pe as usize];

            let v_off = cf.vert_sibling_offsets[cv as usize] as usize;
            let v_cnt = cf.vert_sibling_counts[cv as usize] as usize;

            let tag = if pe_tag.mismatch || pe_tag.linear { val_split } else { val_match };
            for i in 0..v_cnt {
                cf.vert_value_tags[v_off + i] = tag;
            }
        }

        // -- Vertex-vertices: inherit parent tags (complete only) --
        let vv_start = r.get_first_child_vertex_from_vertices();
        let vv_end   = vv_start + r.get_num_child_vertices_from_vertices();

        for cv in vv_start..vv_end {
            let pv = r.get_child_vertex_parent_index(cv);
            debug_assert!(r.is_child_vertex_complete(cv));

            let p_off = pf.vert_sibling_offsets[pv as usize] as usize;
            let p_cnt = pf.vert_sibling_counts[pv as usize] as usize;
            let c_off = cf.vert_sibling_offsets[cv as usize] as usize;

            cf.vert_value_tags[c_off..c_off + p_cnt]
                .copy_from_slice(&pf.vert_value_tags[p_off..p_off + p_cnt]);
        }
    }

    /// Propagate crease-end pairs for smooth-boundary values.
    pub fn propagate_value_creases(&self) {
        debug_assert!(self.cf().has_smooth_boundaries());

        let r = self.r();
        let pf = self.pf();
        let cf = self.cf_mut();

        // Number of child faces per edge depends on split (quad=2, tri=3)
        let inc_faces = if r.get_regular_face_size() == 4 { 2i32 } else { 3 };

        // -- Edge-vertices: assign sequential crease-end spans --
        let ev_start = r.get_first_child_vertex_from_edges();
        let ev_end   = ev_start + r.get_num_child_vertices_from_edges();

        for cv in ev_start..ev_end {
            let v_off = cf.vert_sibling_offsets[cv as usize] as usize;
            let v_cnt = cf.vert_sibling_counts[cv as usize] as usize;

            if !cf.vert_value_tags[v_off].is_mismatch() { continue; }
            if !r.is_child_vertex_complete(cv) { continue; }

            let mut cs = 0i32;
            let mut ce = cs + inc_faces - 1;
            for i in 0..v_cnt {
                if !cf.vert_value_tags[v_off + i].is_inf_sharp() {
                    cf.vert_value_crease_ends[v_off + i].start_face = cs as LocalIndex;
                    cf.vert_value_crease_ends[v_off + i].end_face   = ce as LocalIndex;
                }
                cs += inc_faces;
                ce += inc_faces;
            }
        }

        // -- Vertex-vertices: inherit crease-end pairs from parent --
        let vv_start = r.get_first_child_vertex_from_vertices();
        let vv_end   = vv_start + r.get_num_child_vertices_from_vertices();

        for cv in vv_start..vv_end {
            let v_off = cf.vert_sibling_offsets[cv as usize] as usize;
            let v_cnt = cf.vert_sibling_counts[cv as usize] as usize;

            if !cf.vert_value_tags[v_off].is_mismatch() { continue; }
            if !r.is_child_vertex_complete(cv) { continue; }

            let pv = r.get_child_vertex_parent_index(cv);
            let p_off = pf.vert_sibling_offsets[pv as usize] as usize;

            for j in 0..v_cnt {
                if !cf.vert_value_tags[v_off + j].is_inf_sharp() {
                    cf.vert_value_crease_ends[v_off + j] = pf.vert_value_crease_ends[p_off + j];
                }
            }
        }
    }

    /// Reclassify semi-sharp values that have decayed to smooth as creases.
    pub fn reclassify_semisharp_values(&self) {
        let r = self.r();
        let pf = self.pf();
        let pl = self.pl();
        let cl = self.cl();
        let cf = self.cf_mut();
        let has_dep = pf.has_dependent_sharpness;

        let vv_start = r.get_first_child_vertex_from_vertices();
        let vv_end   = vv_start + r.get_num_child_vertices_from_vertices();

        for cv in vv_start..vv_end {
            let v_off = cf.vert_sibling_offsets[cv as usize] as usize;
            let v_cnt = cf.vert_sibling_counts[cv as usize] as usize;

            if !cf.vert_value_tags[v_off].is_mismatch() { continue; }
            if !r.is_child_vertex_complete(cv) { continue; }

            let pv = r.get_child_vertex_parent_index(cv);
            let p_vtag = pl.get_vertex_tag(pv);
            if !p_vtag.semi_sharp() && !p_vtag.semi_sharp_edges() { continue; }

            let c_vtag = cl.get_vertex_tag(cv);
            if c_vtag.semi_sharp() || c_vtag.inf_sharp() { continue; }

            // No longer semi-sharp at all: clear all semi-sharp -> crease
            if !c_vtag.semi_sharp() && !c_vtag.semi_sharp_edges() {
                for j in 0..v_cnt {
                    if cf.vert_value_tags[v_off + j].semi_sharp {
                        cf.vert_value_tags[v_off + j].semi_sharp = false;
                        cf.vert_value_tags[v_off + j].dep_sharp  = false;
                        cf.vert_value_tags[v_off + j].crease     = true;
                    }
                }
                // Handle dependent sharpness for 2-value case before continuing
                if v_cnt == 2 && has_dep {
                    dep_sharp_check(cf, v_off);
                }
                continue;
            }

            // Some semi-sharp edges remain — inspect each value's crease span.
            // Build child vert-edges (full topology may be absent)
            let c_vert_edges: Vec<Index> = if cl.get_num_vertex_edges_total() > 0 {
                cl.get_vertex_edges(cv).as_slice().to_vec()
            } else {
                let p_vert_edges = pl.get_vertex_edges(pv);
                let p_in_edge = pl.get_vertex_edge_local_indices(pv);
                (0..p_vert_edges.size()).map(|i| {
                    let ee = r.get_edge_child_edges(p_vert_edges[i as usize]);
                    ee[p_in_edge[i as usize] as usize]
                }).collect()
            };
            let n_edges = c_vert_edges.len() as i32;

            // Snapshot crease ends before mutating tags
            let crease_ends: Vec<CreaseEndPair> =
                cf.vert_value_crease_ends[v_off..v_off + v_cnt].to_vec();

            for j in 0..v_cnt {
                if cf.vert_value_tags[v_off + j].semi_sharp
                    && !cf.vert_value_tags[v_off + j].dep_sharp
                {
                    let vs = crease_ends[j].start_face as i32;
                    let ve = crease_ends[j].end_face as i32;

                    let mut still_semi = false;
                    if ve > vs {
                        for k in (vs + 1)..=ve {
                            if cl.get_edge_tag(c_vert_edges[k as usize]).semi_sharp() {
                                still_semi = true;
                                break;
                            }
                        }
                    } else if vs > ve {
                        for k in (vs + 1)..n_edges {
                            if cl.get_edge_tag(c_vert_edges[k as usize]).semi_sharp() {
                                still_semi = true;
                                break;
                            }
                        }
                        if !still_semi {
                            for k in 0..=ve {
                                if cl.get_edge_tag(c_vert_edges[k as usize]).semi_sharp() {
                                    still_semi = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !still_semi {
                        cf.vert_value_tags[v_off + j].semi_sharp = false;
                        cf.vert_value_tags[v_off + j].dep_sharp  = false;
                        cf.vert_value_tags[v_off + j].crease     = true;
                    }
                }
            }

            // Dependent sharpness (2-value case only)
            if v_cnt == 2 && has_dep {
                dep_sharp_check(cf, v_off);
            }
        }
    }
}

/// Clear dependent-sharpness tag if the other value is no longer semi-sharp.
fn dep_sharp_check(cf: &mut FVarLevel, off: usize) {
    if cf.vert_value_tags[off].dep_sharp && !cf.vert_value_tags[off + 1].semi_sharp {
        cf.vert_value_tags[off].dep_sharp = false;
    } else if cf.vert_value_tags[off + 1].dep_sharp && !cf.vert_value_tags[off].semi_sharp {
        cf.vert_value_tags[off + 1].dep_sharp = false;
    }
}

/// Compute the global offset into `vert_face_siblings` for a given vertex.
/// Uses Level's internal counts/offsets array (same layout).
#[inline]
fn vf_offset(level: &Level, v: Index) -> usize {
    level.vert_face_counts_offsets[v as usize * 2 + 1] as usize
}
