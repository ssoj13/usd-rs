// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/fvarLevel.h/.cpp

//! Face-varying topology channel associated with a Level.
//!
//! Stores per-value, per-edge, and per-vertex tags that classify FVar topology
//! relative to the vertex topology of the parent Level.

use crate::sdc::crease::Rule;
use crate::sdc::options::{FVarLinearInterpolation, VtxBoundaryInterpolation};
use crate::sdc::Options;
use super::types::{Index, LocalIndex, ConstIndexArray, IndexArray,
                   ConstLocalIndexArray, LocalIndexArray};
use super::array::{ConstArray, Array};
use super::level::{Level, VTag, ETag as LevelETag};

// ---------------------------------------------------------------------------
// ETag — per-edge tag for FVar discontinuity
// ---------------------------------------------------------------------------

/// Per-edge face-varying tag (bitpacked u8).
/// Mirrors C++ `FVarLevel::ETag`.
#[derive(Clone, Copy, Default)]
pub struct ETag {
    pub mismatch: bool,
    pub discts_v0: bool,
    pub discts_v1: bool,
    pub linear: bool,
}

impl ETag {
    pub fn clear(&mut self) {
        *self = ETag::default();
    }

    /// Combine this FVar edge tag with the Level's edge tag.
    /// If FVar topology mismatches, the edge becomes a boundary with inf-sharp.
    pub fn combine_with_level_etag(&self, mut level_tag: LevelETag) -> LevelETag {
        if self.mismatch {
            level_tag.set_boundary(true);
            level_tag.set_inf_sharp(true);
        }
        level_tag
    }
}

// ---------------------------------------------------------------------------
// ValueTag — per-value tag for FVar classification
// ---------------------------------------------------------------------------

/// Per-value face-varying tag (bitpacked u8).
/// Mirrors C++ `FVarLevel::ValueTag`.
#[derive(Clone, Copy, Default)]
pub struct ValueTag {
    pub mismatch:       bool,
    pub xordinary:      bool,
    pub non_manifold:   bool,
    pub crease:         bool,
    pub semi_sharp:     bool,
    pub dep_sharp:      bool,
    pub inf_sharp_edges: bool,
    pub inf_irregular:  bool,
}

impl ValueTag {
    pub fn clear(&mut self) {
        *self = ValueTag::default();
    }

    #[inline] pub fn is_mismatch(&self) -> bool { self.mismatch }
    #[inline] pub fn is_crease(&self) -> bool { self.crease }
    #[inline] pub fn is_corner(&self) -> bool { !self.crease }
    #[inline] pub fn is_semi_sharp(&self) -> bool { self.semi_sharp }
    #[inline] pub fn is_inf_sharp(&self) -> bool { !self.semi_sharp && !self.crease }
    #[inline] pub fn is_dep_sharp(&self) -> bool { self.dep_sharp }
    #[inline] pub fn has_crease_ends(&self) -> bool { self.crease || self.semi_sharp }
    #[inline] pub fn has_inf_sharp_edges(&self) -> bool { self.inf_sharp_edges }
    #[inline] pub fn has_inf_irregularity(&self) -> bool { self.inf_irregular }

    /// Pack into a single byte (for bitwise OR composite).
    pub fn get_bits(&self) -> u8 {
        (self.mismatch as u8)
            | ((self.xordinary as u8) << 1)
            | ((self.non_manifold as u8) << 2)
            | ((self.crease as u8) << 3)
            | ((self.semi_sharp as u8) << 4)
            | ((self.dep_sharp as u8) << 5)
            | ((self.inf_sharp_edges as u8) << 6)
            | ((self.inf_irregular as u8) << 7)
    }

    /// Unpack from a single byte.
    pub fn from_bits(bits: u8) -> Self {
        Self {
            mismatch:       bits & 0x01 != 0,
            xordinary:      bits & 0x02 != 0,
            non_manifold:   bits & 0x04 != 0,
            crease:         bits & 0x08 != 0,
            semi_sharp:     bits & 0x10 != 0,
            dep_sharp:      bits & 0x20 != 0,
            inf_sharp_edges: bits & 0x40 != 0,
            inf_irregular:  bits & 0x80 != 0,
        }
    }

    /// Combine this FVar value tag with the Level's vertex tag.
    /// Mirrors C++ `ValueTag::combineWithLevelVTag()`.
    pub fn combine_with_level_vtag(&self, mut level_tag: VTag) -> VTag {
        if self.mismatch {
            // Semi-sharp FVar values are treated as corners until sharpness decays
            if self.is_corner() {
                level_tag.set_rule(Rule::Corner as u16);
            } else {
                level_tag.set_rule(Rule::Crease as u16);
            }
            if self.is_crease() || self.is_semi_sharp() {
                level_tag.set_inf_sharp(false);
                level_tag.set_inf_sharp_crease(true);
                level_tag.set_corner(false);
            } else {
                level_tag.set_inf_sharp(true);
                level_tag.set_inf_sharp_crease(false);
                level_tag.set_corner(!self.inf_irregular && !self.inf_sharp_edges);
            }
            level_tag.set_inf_sharp_edges(true);
            level_tag.set_inf_irregular(self.inf_irregular);
            level_tag.set_boundary(true);
            level_tag.set_xordinary(self.xordinary);
            if self.non_manifold {
                level_tag.set_non_manifold(true);
            }
        }
        level_tag
    }
}

// Type aliases for value tag arrays
pub type ConstValueTagArray<'a> = ConstArray<'a, ValueTag>;
pub type ValueTagArray<'a> = Array<'a, ValueTag>;

// ---------------------------------------------------------------------------
// CreaseEndPair — identifies the face span endpoints for a crease value
// ---------------------------------------------------------------------------

/// The two "end faces" of a crease value's span.
/// Mirrors C++ `FVarLevel::CreaseEndPair`.
#[derive(Clone, Copy, Default)]
pub struct CreaseEndPair {
    pub start_face: LocalIndex,
    pub end_face:   LocalIndex,
}

pub type ConstCreaseEndPairArray<'a> = ConstArray<'a, CreaseEndPair>;
pub type CreaseEndPairArray<'a> = Array<'a, CreaseEndPair>;

// Sibling = LocalIndex
pub type Sibling = LocalIndex;
pub type ConstSiblingArray<'a> = ConstLocalIndexArray<'a>;
pub type SiblingArray<'a> = LocalIndexArray<'a>;

// ---------------------------------------------------------------------------
// ValueSpan — transient analysis data for a single value's face span
// ---------------------------------------------------------------------------

/// Information about the "span" of faces sharing a single FVar value
/// around a vertex. Used only during base-level topology analysis.
#[derive(Clone, Copy, Default)]
pub(crate) struct ValueSpan {
    pub size:                LocalIndex,
    pub start:               LocalIndex,
    pub discts_edge_count:   LocalIndex,
    pub semi_sharp_edge_count: LocalIndex,
    pub inf_sharp_edge_count: LocalIndex,
}

// ---------------------------------------------------------------------------
// FVarLevel
// ---------------------------------------------------------------------------

/// Per-face-varying-channel topology level.
/// Mirrors C++ `Vtr::internal::FVarLevel`.
///
/// Stores face-varying values, per-edge/per-vertex/per-value tags, and the
/// vertex-to-value mapping (sibling structure) for one FVar channel.
///
/// Uses a raw `*const Level` back-reference because Level owns FVarLevel.
pub struct FVarLevel {
    // Back-reference to the owning Level (same pattern as C++)
    level: *const Level,

    // Linear interpolation options
    pub(crate) options: Options,
    pub(crate) is_linear: bool,
    pub(crate) has_linear_boundaries: bool,
    pub(crate) has_dependent_sharpness: bool,
    pub(crate) value_count: i32,

    // Per-face: face-varying values (same layout as face-vert indices)
    pub(crate) face_vert_values: Vec<Index>,

    // Per-edge tags
    pub(crate) edge_tags: Vec<ETag>,

    // Per-vertex: sibling structure
    pub(crate) vert_sibling_counts:  Vec<Sibling>,
    pub(crate) vert_sibling_offsets: Vec<i32>,
    pub(crate) vert_face_siblings:   Vec<Sibling>,

    // Per-value (indexed by vertex-value offset)
    pub(crate) vert_value_indices:     Vec<Index>,
    pub(crate) vert_value_tags:        Vec<ValueTag>,
    pub(crate) vert_value_crease_ends: Vec<CreaseEndPair>,
}

// Allow Clone for FVarLevel (Level pointer is rebindable)
impl Clone for FVarLevel {
    fn clone(&self) -> Self {
        Self {
            level: self.level,
            options: self.options,
            is_linear: self.is_linear,
            has_linear_boundaries: self.has_linear_boundaries,
            has_dependent_sharpness: self.has_dependent_sharpness,
            value_count: self.value_count,
            face_vert_values: self.face_vert_values.clone(),
            edge_tags: self.edge_tags.clone(),
            vert_sibling_counts: self.vert_sibling_counts.clone(),
            vert_sibling_offsets: self.vert_sibling_offsets.clone(),
            vert_face_siblings: self.vert_face_siblings.clone(),
            vert_value_indices: self.vert_value_indices.clone(),
            vert_value_tags: self.vert_value_tags.clone(),
            vert_value_crease_ends: self.vert_value_crease_ends.clone(),
        }
    }
}

impl FVarLevel {
    /// Create a new FVarLevel associated with the given Level.
    ///
    /// # Safety
    /// The Level pointer must remain valid for the lifetime of this FVarLevel.
    pub fn new(level: *const Level) -> Self {
        Self {
            level,
            options: Options::default(),
            is_linear: false,
            has_linear_boundaries: false,
            has_dependent_sharpness: false,
            value_count: 0,
            face_vert_values: Vec::new(),
            edge_tags: Vec::new(),
            vert_sibling_counts: Vec::new(),
            vert_sibling_offsets: Vec::new(),
            vert_face_siblings: Vec::new(),
            vert_value_indices: Vec::new(),
            vert_value_tags: Vec::new(),
            vert_value_crease_ends: Vec::new(),
        }
    }

    // -- Level access --

    #[inline]
    fn level(&self) -> &Level {
        unsafe { &*self.level }
    }

    #[inline]
    pub fn get_level(&self) -> &Level {
        self.level()
    }

    // -- Channel-wide queries --

    #[inline] pub fn get_num_values(&self) -> i32 { self.value_count }
    #[inline] pub fn get_num_face_values_total(&self) -> i32 { self.face_vert_values.len() as i32 }
    #[inline] pub fn is_linear(&self) -> bool { self.is_linear }
    #[inline] pub fn has_linear_boundaries(&self) -> bool { self.has_linear_boundaries }
    #[inline] pub fn has_smooth_boundaries(&self) -> bool { !self.has_linear_boundaries }
    #[inline] pub fn has_crease_ends(&self) -> bool { self.has_smooth_boundaries() }
    #[inline] pub fn get_options(&self) -> Options { self.options }

    // -- Per-face access --

    /// Immutable FVar values for a face (indexed like face-vertices in the Level).
    pub fn get_face_values(&self, f_index: Index) -> ConstIndexArray<'_> {
        let lev = self.level();
        let v_count = lev.get_num_face_vertices(f_index) as usize;
        let v_offset = lev.get_offset_of_face_vertices(f_index) as usize;
        ConstIndexArray::new(&self.face_vert_values[v_offset..v_offset + v_count])
    }

    /// Mutable FVar values for a face.
    pub fn get_face_values_mut(&mut self, f_index: Index) -> IndexArray<'_> {
        let lev = self.level();
        let v_count = lev.get_num_face_vertices(f_index) as usize;
        let v_offset = lev.get_offset_of_face_vertices(f_index) as usize;
        IndexArray::new(&mut self.face_vert_values[v_offset..v_offset + v_count])
    }

    // -- Per-edge access --

    #[inline]
    pub fn get_edge_tag(&self, e_index: Index) -> ETag {
        self.edge_tags[e_index as usize]
    }

    #[inline]
    pub fn edge_topology_matches(&self, e_index: Index) -> bool {
        !self.get_edge_tag(e_index).mismatch
    }

    // -- Per-vertex access --

    #[inline]
    pub fn get_num_vertex_values(&self, v: Index) -> i32 {
        self.vert_sibling_counts[v as usize] as i32
    }

    #[inline]
    pub fn get_vertex_value_offset(&self, v: Index, sibling: Sibling) -> Index {
        self.vert_sibling_offsets[v as usize] + sibling as i32
    }

    #[inline]
    pub fn get_vertex_value(&self, v: Index, sibling: Sibling) -> Index {
        self.vert_value_indices[self.get_vertex_value_offset(v, sibling) as usize]
    }

    /// Find the index into vert_value_indices for a given (vertex, value) pair.
    pub fn find_vertex_value_index(&self, vertex_index: Index, value_index: Index) -> Index {
        if self.level().get_depth() > 0 {
            return value_index;
        }
        let mut vv_index = self.get_vertex_value_offset(vertex_index, 0);
        while self.vert_value_indices[vv_index as usize] != value_index {
            vv_index += 1;
        }
        vv_index
    }

    // -- Vertex values array access --

    pub fn get_vertex_values(&self, v_index: Index) -> ConstIndexArray<'_> {
        let v_count = self.get_num_vertex_values(v_index) as usize;
        let v_offset = self.get_vertex_value_offset(v_index, 0) as usize;
        ConstIndexArray::new(&self.vert_value_indices[v_offset..v_offset + v_count])
    }

    pub fn get_vertex_values_mut(&mut self, v_index: Index) -> IndexArray<'_> {
        let v_count = self.get_num_vertex_values(v_index) as usize;
        let v_offset = self.get_vertex_value_offset(v_index, 0) as usize;
        IndexArray::new(&mut self.vert_value_indices[v_offset..v_offset + v_count])
    }

    // -- Vertex value tags array access --

    pub fn get_vertex_value_tags(&self, v_index: Index) -> ConstValueTagArray<'_> {
        let v_count = self.get_num_vertex_values(v_index) as usize;
        let v_offset = self.get_vertex_value_offset(v_index, 0) as usize;
        ConstValueTagArray::new(&self.vert_value_tags[v_offset..v_offset + v_count])
    }

    pub fn get_vertex_value_tags_mut(&mut self, v_index: Index) -> ValueTagArray<'_> {
        let v_count = self.get_num_vertex_values(v_index) as usize;
        let v_offset = self.get_vertex_value_offset(v_index, 0) as usize;
        ValueTagArray::new(&mut self.vert_value_tags[v_offset..v_offset + v_count])
    }

    // -- Vertex value crease ends access --

    pub fn get_vertex_value_crease_ends(&self, v_index: Index) -> ConstCreaseEndPairArray<'_> {
        let v_count = self.get_num_vertex_values(v_index) as usize;
        let v_offset = self.get_vertex_value_offset(v_index, 0) as usize;
        ConstCreaseEndPairArray::new(&self.vert_value_crease_ends[v_offset..v_offset + v_count])
    }

    pub fn get_vertex_value_crease_ends_mut(&mut self, v_index: Index) -> CreaseEndPairArray<'_> {
        let v_count = self.get_num_vertex_values(v_index) as usize;
        let v_offset = self.get_vertex_value_offset(v_index, 0) as usize;
        CreaseEndPairArray::new(&mut self.vert_value_crease_ends[v_offset..v_offset + v_count])
    }

    // -- Vertex-face siblings access --

    pub fn get_vertex_face_siblings(&self, v_index: Index) -> ConstSiblingArray<'_> {
        let lev = self.level();
        let v_count = lev.get_num_vertex_faces(v_index) as usize;
        let v_offset = lev.get_offset_of_vertex_faces(v_index) as usize;
        ConstSiblingArray::new(&self.vert_face_siblings[v_offset..v_offset + v_count])
    }

    pub fn get_vertex_face_siblings_mut(&mut self, v_index: Index) -> SiblingArray<'_> {
        let lev = self.level();
        let v_count = lev.get_num_vertex_faces(v_index) as usize;
        let v_offset = lev.get_offset_of_vertex_faces(v_index) as usize;
        SiblingArray::new(&mut self.vert_face_siblings[v_offset..v_offset + v_count])
    }

    // -- Per-value queries --

    #[inline]
    pub fn get_value_tag(&self, value_index: Index) -> ValueTag {
        self.vert_value_tags[value_index as usize]
    }

    #[inline]
    pub fn value_topology_matches(&self, value_index: Index) -> bool {
        !self.get_value_tag(value_index).mismatch
    }

    #[inline]
    pub fn get_value_crease_end_pair(&self, value_index: Index) -> CreaseEndPair {
        self.vert_value_crease_ends[value_index as usize]
    }

    // -- Face value tag queries --

    /// Fill `value_tags` with the value tag for each vertex of the face.
    pub fn get_face_value_tags(&self, face_index: Index, value_tags: &mut [ValueTag]) {
        let face_values = self.get_face_values(face_index);
        let face_verts = self.level().get_face_vertices(face_index);
        for i in 0..face_values.size() as usize {
            let src_value_index = self.find_vertex_value_index(face_verts[i], face_values[i]);
            value_tags[i] = self.vert_value_tags[src_value_index as usize];
        }
    }

    /// Compute the bitwise-OR composite of all value tags for a face.
    pub fn get_face_composite_value_tag(&self, face_index: Index) -> ValueTag {
        let face_values = self.get_face_values(face_index);
        let face_verts = self.level().get_face_vertices(face_index);
        let mut comp: u8 = 0;
        for i in 0..face_values.size() as usize {
            let src_idx = self.find_vertex_value_index(face_verts[i], face_values[i]);
            comp |= self.vert_value_tags[src_idx as usize].get_bits();
        }
        ValueTag::from_bits(comp)
    }

    // =====================================================================
    // Higher-level topological queries
    // =====================================================================

    /// For a given edge and incident-face index, return the FVar values at
    /// each end vertex within that face.
    pub fn get_edge_face_values(
        &self, e_index: Index, f_inc_to_edge: i32, values_per_vert: &mut [Index; 2],
    ) {
        let lev = self.level();
        let e_verts = lev.get_edge_vertices(e_index);

        if (self.get_num_vertex_values(e_verts[0]) + self.get_num_vertex_values(e_verts[1])) > 2 {
            let e_face = lev.get_edge_faces(e_index)[f_inc_to_edge as usize];
            let e_in_face = lev.get_edge_face_local_indices(e_index)[f_inc_to_edge as usize] as usize;

            let f_values = self.get_face_values(e_face);

            values_per_vert[0] = f_values[e_in_face];
            let next = if (e_in_face + 1) < f_values.size() as usize { e_in_face + 1 } else { 0 };
            values_per_vert[1] = f_values[next];

            // Ensure value pair matches vertex pair
            if e_verts[0] != lev.get_face_vertices(e_face)[e_in_face] {
                values_per_vert.swap(0, 1);
            }
        } else {
            // Simple case: one value per vertex
            if lev.get_depth() > 0 {
                values_per_vert[0] = self.get_vertex_value_offset(e_verts[0], 0);
                values_per_vert[1] = self.get_vertex_value_offset(e_verts[1], 0);
            } else {
                values_per_vert[0] = self.get_vertex_value(e_verts[0], 0);
                values_per_vert[1] = self.get_vertex_value(e_verts[1], 0);
            }
        }
    }

    /// For a vertex, return the FVar value at the far end of each incident edge.
    pub fn get_vertex_edge_values(&self, v_index: Index, values_per_edge: &mut [Index]) {
        let lev = self.level();
        let v_edges = lev.get_vertex_edges(v_index);
        let v_in_edge = lev.get_vertex_edge_local_indices(v_index);
        let v_faces = lev.get_vertex_faces(v_index);
        let v_in_face = lev.get_vertex_face_local_indices(v_index);

        let v_is_boundary = lev.get_vertex_tag(v_index).boundary();
        let v_is_manifold = !lev.get_vertex_tag(v_index).non_manifold();
        let is_base_level = lev.get_depth() == 0;

        for i in 0..v_edges.size() as usize {
            let e_index = v_edges[i];
            let e_verts = lev.get_edge_vertices(e_index);

            debug_assert!(self.edge_topology_matches(e_index));

            let v_other = e_verts[if v_in_edge[i] != 0 { 0 } else { 1 }];
            if self.get_num_vertex_values(v_other) == 1 {
                values_per_edge[i] = if is_base_level {
                    self.get_vertex_value(v_other, 0)
                } else {
                    self.get_vertex_value_offset(v_other, 0)
                };
            } else if v_is_manifold {
                if v_is_boundary && (i == (v_edges.size() as usize - 1)) {
                    let f_values = self.get_face_values(v_faces[i as i32 - 1]);
                    let vif = v_in_face[i as i32 - 1] as usize;
                    let prev_in_face = if vif > 0 { vif - 1 } else { f_values.size() as usize - 1 };
                    values_per_edge[i] = f_values[prev_in_face];
                } else {
                    let f_values = self.get_face_values(v_faces[i as i32]);
                    let vif = v_in_face[i as i32] as usize;
                    let next_in_face = if vif == (f_values.size() as usize - 1) { 0 } else { vif + 1 };
                    values_per_edge[i] = f_values[next_in_face];
                }
            } else {
                // Non-manifold: look up via edge's first face
                let e_face0 = lev.get_edge_faces(e_index)[0usize];
                let e_in_face0 = lev.get_edge_face_local_indices(e_index)[0usize] as usize;

                let f_verts = lev.get_face_vertices(e_face0);
                let f_values = self.get_face_values(e_face0);
                if v_other == f_verts[e_in_face0] {
                    values_per_edge[i] = f_values[e_in_face0];
                } else {
                    let value_in_face = if e_in_face0 == (f_values.size() as usize - 1) { 0 } else { e_in_face0 + 1 };
                    values_per_edge[i] = f_values[value_in_face];
                }
            }
        }
    }

    /// Get the crease end values for a vertex-value sibling.
    pub fn get_vertex_crease_end_values(
        &self, v_index: Index, v_sibling: Sibling, end_values: &mut [Index; 2],
    ) {
        let lev = self.level();
        let v_value_crease_ends = self.get_vertex_value_crease_ends(v_index);

        let v_faces = lev.get_vertex_faces(v_index);
        let v_in_face = lev.get_vertex_face_local_indices(v_index);

        let vert_face0 = v_value_crease_ends[v_sibling as usize].start_face;
        let vert_face1 = v_value_crease_ends[v_sibling as usize].end_face;

        let face0_values = self.get_face_values(v_faces[vert_face0 as i32]);
        let face1_values = self.get_face_values(v_faces[vert_face1 as i32]);

        let end_in_face0 = v_in_face[vert_face0 as i32] as usize;
        let end_in_face1 = v_in_face[vert_face1 as i32] as usize;

        let end_in_face0 = if end_in_face0 == (face0_values.size() as usize - 1) { 0 } else { end_in_face0 + 1 };
        let end_in_face1 = if end_in_face1 > 0 { end_in_face1 - 1 } else { face1_values.size() as usize - 1 };

        end_values[0] = face0_values[end_in_face0];
        end_values[1] = face1_values[end_in_face1];
    }

    // =====================================================================
    // Initialization and sizing
    // =====================================================================

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    /// Resize per-component arrays to match the associated Level.
    pub fn resize_components(&mut self) {
        let lev = self.level();
        let num_fv_total = lev.get_num_face_vertices_total() as usize;
        let num_edges = lev.get_num_edges() as usize;
        let num_verts = lev.get_num_vertices() as usize;
        let num_vf_total = lev.get_num_vertex_faces_total() as usize;

        self.face_vert_values.resize(num_fv_total, 0);

        let etag_match = ETag::default();
        self.edge_tags.resize(num_edges, etag_match);

        self.vert_sibling_counts.resize(num_verts, 0);
        self.vert_sibling_offsets.resize(num_verts, 0);
        self.vert_face_siblings.resize(num_vf_total, 0);
    }

    /// Resize per-vertex-value arrays.
    pub fn resize_vertex_values(&mut self, vertex_value_count: i32) {
        let n = vertex_value_count as usize;
        self.vert_value_indices.resize(n, 0);

        let tag_match = ValueTag::default();
        self.vert_value_tags.resize(n, tag_match);

        if self.has_crease_ends() {
            self.vert_value_crease_ends.resize(n, CreaseEndPair::default());
        }
    }

    /// Set the total number of distinct values.
    pub fn resize_values(&mut self, value_count: i32) {
        self.value_count = value_count;
    }

    // =====================================================================
    // Topology analysis — completeTopologyFromFaceValues
    // =====================================================================

    /// Analyze face-varying topology and populate all tags from the face values.
    /// Called once after base-level face values are assigned.
    /// `regular_boundary_valence` is the expected valence for smooth boundary verts.
    pub fn complete_topology_from_face_values(&mut self, regular_boundary_valence: i32) {
        // Bypass borrow checker: level is a raw ptr, safe to deref independently
        let lev: &Level = unsafe { &*self.level };

        let geom_options = self.options.get_vtx_boundary_interpolation();
        let fvar_options = self.options.get_fvar_linear_interpolation();

        self.is_linear = fvar_options == FVarLinearInterpolation::All;
        self.has_linear_boundaries =
            fvar_options == FVarLinearInterpolation::All
            || fvar_options == FVarLinearInterpolation::Boundaries;
        self.has_dependent_sharpness =
            fvar_options == FVarLinearInterpolation::CornersPlus1
            || fvar_options == FVarLinearInterpolation::CornersPlus2;

        let geom_corners_are_smooth = geom_options != VtxBoundaryInterpolation::EdgeAndCorner;
        let fvar_corners_are_sharp = fvar_options != FVarLinearInterpolation::None;
        let make_smooth_corners_sharp = geom_corners_are_smooth && fvar_corners_are_sharp;
        let sharpen_both_if_one_corner = fvar_options == FVarLinearInterpolation::CornersPlus2;
        let sharpen_darts = sharpen_both_if_one_corner || self.has_linear_boundaries;

        let num_verts = lev.get_num_vertices();
        let mut vertex_mismatch = vec![false; num_verts as usize];

        let max_valence = lev.get_max_valence() as usize;

        // Buffers for per-vertex processing
        let mut index_buffer = vec![0 as Index; max_valence];
        let mut unique_values = vec![0i32; max_valence];
        let mut sibling_buffer: Vec<Sibling> = vec![0; max_valence];
        let mut span_buffer = vec![ValueSpan::default(); max_valence];

        let mut total_value_count: i32 = 0;

        // ---- Pass 1: identify discts edges, count unique values per vertex ----
        for v_index in 0..num_verts {
            let v_faces = lev.get_vertex_faces(v_index);
            let v_in_face = lev.get_vertex_face_local_indices(v_index);
            let nf = v_faces.size() as usize;

            // Collect FVar value at this vertex in each incident face
            let v_values = &mut index_buffer[..nf];
            for i in 0..nf {
                v_values[i] = self.face_vert_values[
                    lev.get_offset_of_face_vertices(v_faces[i]) as usize + v_in_face[i] as usize
                ];
            }

            let v_edges = lev.get_vertex_edges(v_index);
            let v_in_edge = lev.get_vertex_edge_local_indices(v_index);

            let v_is_manifold = !lev.get_vertex_tag(v_index).non_manifold();
            let v_is_boundary = lev.get_vertex_tag(v_index).boundary();

            if v_is_manifold {
                let start = if v_is_boundary { 1 } else { 0 };
                for i in start..nf {
                    let v_face_next = i;
                    let v_face_prev = if i > 0 { i - 1 } else { nf - 1 };

                    if v_values[v_face_next] != v_values[v_face_prev] {
                        let e_index = v_edges[i as i32];
                        let e_verts = lev.get_edge_vertices(e_index);
                        vertex_mismatch[e_verts[0usize] as usize] = true;
                        vertex_mismatch[e_verts[1usize] as usize] = true;

                        let e_tag = &mut self.edge_tags[e_index as usize];
                        e_tag.discts_v0 = e_verts[0usize] == v_index;
                        e_tag.discts_v1 = e_verts[1usize] == v_index;
                        e_tag.mismatch = true;
                        e_tag.linear = self.has_linear_boundaries;
                    }
                }
            } else if nf > 0 {
                // Non-manifold: check each edge for continuity between its faces
                for i in 0..v_edges.size() as usize {
                    let e_index = v_edges[i as i32];
                    let e_faces = lev.get_edge_faces(e_index);
                    if e_faces.size() < 2 { continue; }

                    let e_in_face = lev.get_edge_face_local_indices(e_index);
                    let e_verts = lev.get_edge_vertices(e_index);
                    let vert_in_edge = v_in_edge[i as i32];

                    let mut mark_edge_discts = false;
                    let mut value_index_in_face0: Index = 0;
                    for j in 0..e_faces.size() as usize {
                        if mark_edge_discts { break; }
                        let f_index = e_faces[j as i32];
                        let f_verts = lev.get_face_vertices(f_index);
                        let f_values = self.get_face_values(f_index);

                        let edge_in_face = e_in_face[j as i32] as usize;
                        let edge_reversed = if e_verts[0usize] != f_verts[edge_in_face as i32] { 1 } else { 0 };
                        let mut vert_in_face = edge_in_face + (if vert_in_edge as usize != edge_reversed { 1 } else { 0 });
                        if vert_in_face == f_verts.size() as usize { vert_in_face = 0; }

                        if j == 0 {
                            value_index_in_face0 = f_values[vert_in_face];
                        } else {
                            mark_edge_discts = f_values[vert_in_face] != value_index_in_face0;
                        }
                    }
                    if mark_edge_discts {
                        let e_verts = lev.get_edge_vertices(e_index);
                        vertex_mismatch[e_verts[0usize] as usize] = true;
                        vertex_mismatch[e_verts[1usize] as usize] = true;

                        let e_tag = &mut self.edge_tags[e_index as usize];
                        e_tag.discts_v0 = e_verts[0usize] == v_index;
                        e_tag.discts_v1 = e_verts[1usize] == v_index;
                        e_tag.mismatch = true;
                        e_tag.linear = self.has_linear_boundaries;
                    }
                }
            }

            // Handle boundary vertices that may need mismatch due to linear boundaries
            if v_is_boundary && !vertex_mismatch[v_index as usize] {
                if self.has_linear_boundaries && nf > 0 {
                    vertex_mismatch[v_index as usize] = true;
                    if v_is_manifold {
                        self.edge_tags[v_edges[0i32] as usize].linear = true;
                        self.edge_tags[v_edges[v_edges.size() - 1] as usize].linear = true;
                    } else {
                        for i in 0..v_edges.size() as usize {
                            if lev.get_edge_tag(v_edges[i as i32]).boundary() {
                                self.edge_tags[v_edges[i as i32] as usize].linear = true;
                            }
                        }
                    }
                } else if nf == 1 && make_smooth_corners_sharp {
                    vertex_mismatch[v_index as usize] = true;
                }
            }

            // Count unique values around vertex
            let mut unique_value_count = 1usize;
            unique_values[0] = v_values[0];
            sibling_buffer[0] = 0;

            for i in 1..nf {
                if v_values[i] == v_values[i - 1] {
                    sibling_buffer[i] = sibling_buffer[i - 1];
                } else {
                    sibling_buffer[i] = unique_value_count as Sibling;

                    // Check if this value already exists in the unique set
                    let mut found = false;
                    for k in 0..unique_value_count {
                        if unique_values[k] == v_values[i] {
                            sibling_buffer[i] = k as Sibling;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        unique_values[unique_value_count] = v_values[i];
                        unique_value_count += 1;
                    }
                }
            }

            // Non-manifold with multiple values but no discts edges
            if !v_is_manifold && !vertex_mismatch[v_index as usize] {
                vertex_mismatch[v_index as usize] = unique_value_count > 1;
            }

            self.vert_sibling_counts[v_index as usize] = unique_value_count as Sibling;
            self.vert_sibling_offsets[v_index as usize] = total_value_count;
            total_value_count += unique_value_count as i32;

            // Update vert-face siblings
            if unique_value_count > 1 {
                let offset = lev.get_offset_of_vertex_faces(v_index) as usize;
                for i in 0..nf {
                    self.vert_face_siblings[offset + i] = sibling_buffer[i];
                }
            }
        }

        // ---- Pass 2: assign vertex values and tag mismatched topology ----
        self.resize_vertex_values(total_value_count);

        for v_index in 0..num_verts {
            let v_faces = lev.get_vertex_faces(v_index);
            let v_in_face = lev.get_vertex_face_local_indices(v_index);
            let nf = v_faces.size() as usize;

            // Assign vertex values from face values
            let vv_offset = self.vert_sibling_offsets[v_index as usize] as usize;
            let vv_count = self.vert_sibling_counts[v_index as usize] as usize;

            if nf > 0 {
                self.vert_value_indices[vv_offset] = self.face_vert_values[
                    lev.get_offset_of_face_vertices(v_faces[0i32]) as usize + v_in_face[0i32] as usize
                ];
            } else {
                self.vert_value_indices[vv_offset] = 0;
            }

            if !vertex_mismatch[v_index as usize] {
                continue;
            }

            // Assign remaining sibling values
            if vv_count > 1 {
                let siblings = {
                    let offset = lev.get_offset_of_vertex_faces(v_index) as usize;
                    // Copy siblings to a temp buffer to avoid borrow issues
                    let mut tmp = vec![0 as Sibling; nf];
                    tmp.copy_from_slice(&self.vert_face_siblings[offset..offset + nf]);
                    tmp
                };

                let mut next_sibling = 1usize;
                for i in 1..nf {
                    if siblings[i] as usize == next_sibling {
                        self.vert_value_indices[vv_offset + next_sibling] = self.face_vert_values[
                            lev.get_offset_of_face_vertices(v_faces[i as i32]) as usize
                                + v_in_face[i as i32] as usize
                        ];
                        next_sibling += 1;
                    }
                }
            }

            // Tag values
            let v_tag = lev.get_vertex_tag(v_index);

            let all_corners_are_sharp_base =
                self.has_linear_boundaries || v_tag.inf_sharp() || v_tag.non_manifold()
                || (self.has_dependent_sharpness && (vv_count > 2))
                || (sharpen_darts && (vv_count == 1) && !v_tag.boundary());

            // Gather value spans
            let value_spans = &mut span_buffer[..vv_count];
            for s in value_spans.iter_mut() { *s = ValueSpan::default(); }
            self.gather_value_spans(v_index, value_spans);

            // Determine if all corners should be sharp (dependency analysis)
            let mut all_corners_are_sharp = all_corners_are_sharp_base;
            let mut has_dependent_values_to_sharpen = false;

            if !all_corners_are_sharp && self.has_dependent_sharpness && vv_count == 2 {
                all_corners_are_sharp =
                    value_spans[0].inf_sharp_edge_count > 0 || value_spans[1].inf_sharp_edge_count > 0
                    || value_spans[0].discts_edge_count > 0 || value_spans[1].discts_edge_count > 0;

                if sharpen_both_if_one_corner {
                    all_corners_are_sharp |= (value_spans[0].size == 1) || (value_spans[1].size == 1);
                }

                has_dependent_values_to_sharpen =
                    (value_spans[0].semi_sharp_edge_count > 0) != (value_spans[1].semi_sharp_edge_count > 0);
            }

            // Tag each value
            for i in 0..vv_count {
                let value_tag = &mut self.vert_value_tags[vv_offset + i];
                value_tag.clear();
                value_tag.mismatch = true;

                let v_span = &value_spans[i];
                if v_span.discts_edge_count > 0 {
                    value_tag.non_manifold = true;
                    continue;
                }
                debug_assert!(v_span.size != 0);

                let is_inf_sharp = all_corners_are_sharp
                    || v_span.inf_sharp_edge_count > 0
                    || ((v_span.size == 1) && fvar_corners_are_sharp);

                if v_span.size == 1 {
                    value_tag.xordinary = !is_inf_sharp;
                } else {
                    value_tag.xordinary = v_span.size as i32 != regular_boundary_valence;
                }

                value_tag.inf_sharp_edges = v_span.inf_sharp_edge_count > 0;
                value_tag.inf_irregular = if v_span.inf_sharp_edge_count > 0 {
                    (v_span.size as i32 - v_span.inf_sharp_edge_count as i32) > 1
                } else if is_inf_sharp {
                    v_span.size > 1
                } else {
                    value_tag.xordinary
                };

                if !is_inf_sharp {
                    if v_span.semi_sharp_edge_count > 0 || v_tag.semi_sharp() {
                        value_tag.semi_sharp = true;
                    } else if has_dependent_values_to_sharpen {
                        value_tag.semi_sharp = true;
                        value_tag.dep_sharp = true;
                    } else {
                        value_tag.crease = true;
                    }

                    if self.has_crease_ends() {
                        let crease_end = &mut self.vert_value_crease_ends[vv_offset + i];
                        crease_end.start_face = v_span.start;
                        if i == 0 && v_span.start != 0 {
                            crease_end.end_face =
                                (v_span.start as i32 + v_span.size as i32 - 1 - v_faces.size()) as LocalIndex;
                        } else {
                            crease_end.end_face =
                                (v_span.start as i32 + v_span.size as i32 - 1) as LocalIndex;
                        }
                    }
                }
            }
        }
    }

    // =====================================================================
    // Gather value spans
    // =====================================================================

    /// Gather span information for each value of a vertex.
    pub(crate) fn gather_value_spans(&self, v_index: Index, v_value_spans: &mut [ValueSpan]) {
        let lev = self.level();
        let v_edges = lev.get_vertex_edges(v_index);
        let v_faces = lev.get_vertex_faces(v_index);
        let nf = v_faces.size() as usize;
        let ne = v_edges.size() as usize;

        let v_face_siblings = self.get_vertex_face_siblings(v_index);

        let v_has_single_value = self.get_num_vertex_values(v_index) == 1;
        let v_is_boundary = ne > nf;
        let v_is_non_manifold = lev.get_vertex_tag(v_index).non_manifold();

        if v_is_non_manifold {
            // Mark all spans with a discts edge to trigger non-manifold handling
            let v_values = self.get_vertex_values(v_index);
            for i in 0..v_values.size() as usize {
                v_value_spans[i].size = 0;
                v_value_spans[i].discts_edge_count = 1;
            }
        } else if v_has_single_value && !v_is_boundary {
            // Interior dart: check for discts edges
            v_value_spans[0].size = 0;
            v_value_spans[0].start = 0;
            for i in 0..ne {
                if self.edge_tags[v_edges[i as i32] as usize].mismatch {
                    if v_value_spans[0].size > 0 {
                        v_value_spans[0].discts_edge_count = 1;
                        break;
                    } else {
                        v_value_spans[0].size = nf as LocalIndex;
                        v_value_spans[0].start = i as LocalIndex;
                    }
                } else if lev.get_edge_tag(v_edges[i as i32]).inf_sharp() {
                    v_value_spans[0].inf_sharp_edge_count += 1;
                } else if lev.get_edge_tag(v_edges[i as i32]).semi_sharp() {
                    v_value_spans[0].semi_sharp_edge_count += 1;
                }
            }
            v_value_spans[0].size = nf as LocalIndex;
        } else {
            // Walk around the vertex and accumulate span info for each value
            v_value_spans[0].size = 1;
            v_value_spans[0].start = 0;

            if !v_is_boundary && (v_face_siblings[nf as i32 - 1] == 0) {
                if self.edge_tags[v_edges[0i32] as usize].mismatch {
                    v_value_spans[0].discts_edge_count += 1;
                } else if lev.get_edge_tag(v_edges[0i32]).inf_sharp() {
                    v_value_spans[0].inf_sharp_edge_count += 1;
                } else if lev.get_edge_tag(v_edges[0i32]).semi_sharp() {
                    v_value_spans[0].semi_sharp_edge_count += 1;
                }
            }

            for i in 1..nf {
                let sib = v_face_siblings[i as i32] as usize;
                let prev_sib = v_face_siblings[i as i32 - 1] as usize;

                if sib == prev_sib {
                    if self.edge_tags[v_edges[i as i32] as usize].mismatch {
                        v_value_spans[sib].discts_edge_count += 1;
                    } else if lev.get_edge_tag(v_edges[i as i32]).inf_sharp() {
                        v_value_spans[sib].inf_sharp_edge_count += 1;
                    } else if lev.get_edge_tag(v_edges[i as i32]).semi_sharp() {
                        v_value_spans[sib].semi_sharp_edge_count += 1;
                    }
                } else {
                    // Different sibling: starting a new span for this value
                    if v_value_spans[sib].size > 0 {
                        v_value_spans[sib].discts_edge_count += 1;
                    }
                    v_value_spans[sib].start = i as LocalIndex;
                }
                v_value_spans[sib].size += 1;
            }

            // If span for value 0 wrapped around, adjust disjoint count
            if (v_face_siblings[nf as i32 - 1] == 0) && !v_is_boundary {
                if v_value_spans[0].discts_edge_count > 0 {
                    v_value_spans[0].discts_edge_count -= 1;
                }
            }
        }
    }

    // =====================================================================
    // Face value initialization
    // =====================================================================

    /// Initialize face values as a copy of the Level's face-vertex indices.
    pub fn initialize_face_values_from_face_vertices(&mut self) {
        let lev: &Level = unsafe { &*self.level };
        let src = lev.get_all_face_vertices();
        self.face_vert_values.resize(src.size() as usize, 0);
        for i in 0..src.size() as usize {
            self.face_vert_values[i] = src[i];
        }
    }

    /// Initialize face values from the vertex-face sibling offsets.
    pub fn initialize_face_values_from_vertex_face_siblings(&mut self) {
        let lev: &Level = unsafe { &*self.level };
        let fv_indices = lev.get_all_face_vertices();

        // First pass: initialize each face-value with the first value offset
        for i in 0..fv_indices.size() as usize {
            self.face_vert_values[i] = self.get_vertex_value_offset(fv_indices[i], 0);
        }

        // Second pass: adjust for siblings > 0
        for v_index in 0..lev.get_num_vertices() {
            if self.get_num_vertex_values(v_index) > 1 {
                let v_faces = lev.get_vertex_faces(v_index);
                let v_in_face = lev.get_vertex_face_local_indices(v_index);
                // Collect siblings to avoid borrow conflict with face_vert_values
                let siblings: Vec<Sibling> = {
                    let s = self.get_vertex_face_siblings(v_index);
                    (0..v_faces.size()).map(|j| s[j]).collect()
                };

                for j in 0..v_faces.size() as usize {
                    if siblings[j] != 0 {
                        let fv_offset = lev.get_offset_of_face_vertices(v_faces[j as i32]) as usize;
                        self.face_vert_values[fv_offset + v_in_face[j as i32] as usize]
                            += siblings[j] as i32;
                    }
                }
            }
        }
    }

    /// Build face-vertex siblings from vertex-face siblings (for validation).
    pub fn build_face_vertex_siblings_from_vertex_face_siblings(
        &self, fv_siblings: &mut Vec<Sibling>,
    ) {
        let lev = self.level();
        let total = lev.get_num_face_vertices_total() as usize;
        fv_siblings.resize(total, 0);
        fv_siblings.fill(0);

        for v_index in 0..lev.get_num_vertices() {
            if self.get_num_vertex_values(v_index) > 1 {
                let v_faces = lev.get_vertex_faces(v_index);
                let v_in_face = lev.get_vertex_face_local_indices(v_index);
                let v_siblings = self.get_vertex_face_siblings(v_index);

                for j in 0..v_faces.size() as usize {
                    if v_siblings[j as i32] > 0 {
                        fv_siblings[
                            lev.get_offset_of_face_vertices(v_faces[j as i32]) as usize
                            + v_in_face[j as i32] as usize
                        ] = v_siblings[j as i32];
                    }
                }
            }
        }
    }

    // =====================================================================
    // Validation
    // =====================================================================

    /// Validate internal consistency of the FVar topology.
    pub fn validate(&self) -> bool {
        let lev = self.level();

        if self.vert_sibling_counts.len() as i32 != lev.get_num_vertices() {
            return false;
        }
        if self.edge_tags.len() as i32 != lev.get_num_edges() {
            return false;
        }
        if self.face_vert_values.len() as i32 != lev.get_num_face_vertices_total() {
            return false;
        }
        if lev.get_depth() > 0 && self.value_count != self.vert_value_indices.len() as i32 {
            return false;
        }

        // Verify face-verts and siblings yield expected face-values
        let mut fv_siblings = Vec::new();
        self.build_face_vertex_siblings_from_vertex_face_siblings(&mut fv_siblings);

        for f_index in 0..lev.get_num_faces() {
            let f_verts = lev.get_face_vertices(f_index);
            let f_values = self.get_face_values(f_index);
            let fv_off = lev.get_offset_of_face_vertices(f_index) as usize;

            for fv_idx in 0..f_verts.size() as usize {
                let v_index = f_verts[fv_idx];
                let fv_value = f_values[fv_idx];
                let fv_sibling = fv_siblings[fv_off + fv_idx];

                if fv_sibling as i32 >= self.get_num_vertex_values(v_index) {
                    return false;
                }
                let test_value = self.get_vertex_value(v_index, fv_sibling);
                if test_value != fv_value {
                    return false;
                }
            }
        }

        // Verify vert-face siblings yield expected values
        for v_index in 0..lev.get_num_vertices() {
            let v_faces = lev.get_vertex_faces(v_index);
            let v_in_face = lev.get_vertex_face_local_indices(v_index);
            let v_siblings = self.get_vertex_face_siblings(v_index);

            for j in 0..v_faces.size() as usize {
                let v_sibling = v_siblings[j as i32];
                if v_sibling as i32 >= self.get_num_vertex_values(v_index) {
                    return false;
                }

                let f_index = v_faces[j as i32];
                let fv_index = v_in_face[j as i32] as usize;
                let fv_value = self.get_face_values(f_index)[fv_index];
                let v_value = self.get_vertex_value(v_index, v_sibling);
                if v_value != fv_value {
                    return false;
                }
            }
        }
        true
    }

    // =====================================================================
    // Legacy resize interface (backward compat with existing Level code)
    // =====================================================================

    /// Legacy resize for backward compatibility with Level::create_fvar_channel.
    pub fn resize(&mut self, num_values: i32, num_faces: i32, num_face_values_total: i32) {
        self.value_count = num_values;
        self.face_vert_values.resize(num_face_values_total as usize, super::types::INDEX_INVALID);
        // Legacy: vert_sibling_counts etc. are populated by complete_topology_from_face_values
        let _ = num_faces; // no longer needed (delegated to Level)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etag_default_is_matching() {
        let e = ETag::default();
        assert!(!e.mismatch);
        assert!(!e.discts_v0);
        assert!(!e.discts_v1);
        assert!(!e.linear);
    }

    #[test]
    fn etag_combine_with_level_etag() {
        let mut fvar_etag = ETag::default();
        fvar_etag.mismatch = true;

        let level_etag = super::super::level::ETag::default();
        let combined = fvar_etag.combine_with_level_etag(level_etag);

        assert!(combined.boundary());
        assert!(combined.inf_sharp());
    }

    #[test]
    fn value_tag_bits_roundtrip() {
        let tag = ValueTag {
            mismatch: true,
            xordinary: false,
            non_manifold: true,
            crease: false,
            semi_sharp: true,
            dep_sharp: false,
            inf_sharp_edges: true,
            inf_irregular: false,
        };
        let bits = tag.get_bits();
        let recovered = ValueTag::from_bits(bits);
        assert_eq!(recovered.mismatch, true);
        assert_eq!(recovered.non_manifold, true);
        assert_eq!(recovered.semi_sharp, true);
        assert_eq!(recovered.inf_sharp_edges, true);
        assert_eq!(recovered.xordinary, false);
        assert_eq!(recovered.crease, false);
        assert_eq!(recovered.dep_sharp, false);
        assert_eq!(recovered.inf_irregular, false);
    }

    #[test]
    fn value_tag_classification() {
        let mut tag = ValueTag::default();
        // Default: not crease, not semi_sharp => is_corner=true, is_inf_sharp=true
        assert!(tag.is_corner());
        assert!(tag.is_inf_sharp());
        assert!(!tag.is_crease());
        assert!(!tag.is_semi_sharp());

        tag.crease = true;
        assert!(tag.is_crease());
        assert!(!tag.is_corner());
        assert!(!tag.is_inf_sharp());

        tag.crease = false;
        tag.semi_sharp = true;
        assert!(tag.is_semi_sharp());
        assert!(tag.is_corner()); // semi_sharp but not crease => corner
        assert!(!tag.is_inf_sharp());
    }

    #[test]
    fn crease_end_pair_default() {
        let cep = CreaseEndPair::default();
        assert_eq!(cep.start_face, 0);
        assert_eq!(cep.end_face, 0);
    }

    #[test]
    fn fvar_level_basic_construction() {
        let level = Level::new();
        let fvar = FVarLevel::new(&level as *const Level);
        assert_eq!(fvar.get_num_values(), 0);
        assert!(!fvar.is_linear());
        assert!(!fvar.has_linear_boundaries());
        assert!(fvar.has_smooth_boundaries());
    }

    #[test]
    fn fvar_level_resize_components() {
        let mut level = Level::new();
        level.face_count = 2;
        level.edge_count = 5;
        level.vert_count = 4;
        // 2 faces, face 0 has 3 verts, face 1 has 4 verts = 7 total
        level.face_vert_counts_offsets = vec![3, 0, 4, 3];
        level.face_vert_indices = vec![0, 1, 2, 0, 2, 3, 1]; // 7 entries
        // vert-face c/o
        level.vert_face_counts_offsets = vec![
            2, 0,  // v0: 2 faces
            2, 2,  // v1: 2 faces
            2, 4,  // v2: 2 faces
            1, 6,  // v3: 1 face
        ];
        level.vert_face_indices = vec![0, 1, 0, 1, 0, 1, 1]; // 7 entries
        level.vert_face_local_indices = vec![0, 0, 1, 3, 2, 1, 2]; // 7 entries

        let mut fvar = FVarLevel::new(&level as *const Level);
        fvar.resize_components();

        assert_eq!(fvar.face_vert_values.len(), 7);
        assert_eq!(fvar.edge_tags.len(), 5);
        assert_eq!(fvar.vert_sibling_counts.len(), 4);
        assert_eq!(fvar.vert_sibling_offsets.len(), 4);
        assert_eq!(fvar.vert_face_siblings.len(), 7);
    }

    #[test]
    fn value_tag_combine_with_level_vtag_corner() {
        let mut vtag = ValueTag::default();
        vtag.mismatch = true;
        // Not crease, not semi_sharp => corner

        let level_vtag = VTag::default();
        let combined = vtag.combine_with_level_vtag(level_vtag);

        assert!(combined.boundary());
        assert!(combined.inf_sharp());
        assert!(combined.inf_sharp_edges());
        assert_eq!(combined.rule(), Rule::Corner as u16);
    }

    #[test]
    fn value_tag_combine_with_level_vtag_crease() {
        let mut vtag = ValueTag::default();
        vtag.mismatch = true;
        vtag.crease = true;

        let level_vtag = VTag::default();
        let combined = vtag.combine_with_level_vtag(level_vtag);

        assert!(combined.boundary());
        assert!(!combined.inf_sharp());
        assert!(combined.inf_sharp_crease());
        assert!(!combined.corner());
        assert_eq!(combined.rule(), Rule::Crease as u16);
    }
}
