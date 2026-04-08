// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/level.h/.cpp
//
// This is the central internal topology representation.  A Level stores the
// six topological relations (face-vert, face-edge, edge-vert, edge-face,
// vert-face, vert-edge) as SOA (Structure-of-Arrays) with an interleaved
// [count, offset] pair per component for variable-arity relations.

use super::fvar_level::FVarLevel;
use super::types::{
    ConstIndexArray, ConstLocalIndexArray, INDEX_INVALID, Index, IndexArray, LocalIndex,
    LocalIndexArray, index_is_valid,
};
use crate::sdc::{Options, crease::Rule};

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

/// Per-vertex topology tag (bitpacked u16).
/// Mirrors C++ `Vtr::internal::Level::VTag`.
#[derive(Clone, Copy, Default)]
pub struct VTag(pub u16);

impl VTag {
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    pub fn get_bits(self) -> u16 {
        self.0
    }

    // ---- bit accessors ----
    #[inline]
    pub fn non_manifold(self) -> bool {
        self.0 & 0x0001 != 0
    }
    #[inline]
    pub fn xordinary(self) -> bool {
        self.0 & 0x0002 != 0
    }
    #[inline]
    pub fn boundary(self) -> bool {
        self.0 & 0x0004 != 0
    }
    #[inline]
    pub fn corner(self) -> bool {
        self.0 & 0x0008 != 0
    }
    #[inline]
    pub fn inf_sharp(self) -> bool {
        self.0 & 0x0010 != 0
    }
    #[inline]
    pub fn semi_sharp(self) -> bool {
        self.0 & 0x0020 != 0
    }
    #[inline]
    pub fn semi_sharp_edges(self) -> bool {
        self.0 & 0x0040 != 0
    }
    /// 4-bit rule field (bits [10:7]).
    #[inline]
    pub fn rule(self) -> u16 {
        (self.0 >> 7) & 0x000F
    }
    #[inline]
    pub fn incomplete(self) -> bool {
        self.0 & 0x0800 != 0
    }
    #[inline]
    pub fn incid_irreg_face(self) -> bool {
        self.0 & 0x1000 != 0
    }
    #[inline]
    pub fn inf_sharp_edges(self) -> bool {
        self.0 & 0x2000 != 0
    }
    #[inline]
    pub fn inf_sharp_crease(self) -> bool {
        self.0 & 0x4000 != 0
    }
    #[inline]
    pub fn inf_irregular(self) -> bool {
        self.0 & 0x8000 != 0
    }

    // ---- setters ----
    #[inline]
    pub fn set_non_manifold(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0001, v);
    }
    #[inline]
    pub fn set_xordinary(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0002, v);
    }
    #[inline]
    pub fn set_boundary(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0004, v);
    }
    #[inline]
    pub fn set_corner(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0008, v);
    }
    #[inline]
    pub fn set_inf_sharp(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0010, v);
    }
    #[inline]
    pub fn set_semi_sharp(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0020, v);
    }
    #[inline]
    pub fn set_semi_sharp_edges(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0040, v);
    }
    #[inline]
    pub fn set_rule(&mut self, r: u16) {
        self.0 = (self.0 & !0x0780) | ((r & 0xF) << 7);
    }
    #[inline]
    pub fn set_incomplete(&mut self, v: bool) {
        set_bit(&mut self.0, 0x0800, v);
    }
    #[inline]
    pub fn set_incid_irreg_face(&mut self, v: bool) {
        set_bit(&mut self.0, 0x1000, v);
    }
    #[inline]
    pub fn set_inf_sharp_edges(&mut self, v: bool) {
        set_bit(&mut self.0, 0x2000, v);
    }
    #[inline]
    pub fn set_inf_sharp_crease(&mut self, v: bool) {
        set_bit(&mut self.0, 0x4000, v);
    }
    #[inline]
    pub fn set_inf_irregular(&mut self, v: bool) {
        set_bit(&mut self.0, 0x8000, v);
    }

    /// Bitwise-OR of up to `size` tags — produces a "composite" tag.
    pub fn bitwise_or(tags: &[VTag]) -> VTag {
        VTag(tags.iter().fold(0u16, |acc, t| acc | t.0))
    }
}

/// Per-edge topology tag (bitpacked u8).
/// Mirrors C++ `Vtr::internal::Level::ETag`.
#[derive(Clone, Copy, Default)]
pub struct ETag(pub u8);

impl ETag {
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    #[inline]
    pub fn non_manifold(self) -> bool {
        self.0 & 0x01 != 0
    }
    #[inline]
    pub fn boundary(self) -> bool {
        self.0 & 0x02 != 0
    }
    #[inline]
    pub fn inf_sharp(self) -> bool {
        self.0 & 0x04 != 0
    }
    #[inline]
    pub fn semi_sharp(self) -> bool {
        self.0 & 0x08 != 0
    }

    #[inline]
    pub fn set_non_manifold(&mut self, v: bool) {
        set_bit_u8(&mut self.0, 0x01, v);
    }
    #[inline]
    pub fn set_boundary(&mut self, v: bool) {
        set_bit_u8(&mut self.0, 0x02, v);
    }
    #[inline]
    pub fn set_inf_sharp(&mut self, v: bool) {
        set_bit_u8(&mut self.0, 0x04, v);
    }
    #[inline]
    pub fn set_semi_sharp(&mut self, v: bool) {
        set_bit_u8(&mut self.0, 0x08, v);
    }

    pub fn bitwise_or(tags: &[ETag]) -> ETag {
        ETag(tags.iter().fold(0u8, |acc, t| acc | t.0))
    }
}

/// Per-face topology tag (bitpacked u8).
/// Mirrors C++ `Vtr::internal::Level::FTag`.
#[derive(Clone, Copy, Default)]
pub struct FTag(pub u8);

impl FTag {
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    #[inline]
    pub fn hole(self) -> bool {
        self.0 & 0x01 != 0
    }
    #[inline]
    pub fn set_hole(&mut self, v: bool) {
        set_bit_u8(&mut self.0, 0x01, v);
    }
}

/// A span around a vertex (subset of incident faces).
/// Mirrors C++ `Vtr::internal::Level::VSpan`.
#[derive(Clone, Copy, Default)]
pub struct VSpan {
    pub num_faces: LocalIndex,
    pub start_face: LocalIndex,
    pub corner_in_span: LocalIndex,
    pub periodic: bool,
    pub sharp: bool,
}

impl VSpan {
    pub fn clear(&mut self) {
        *self = VSpan::default();
    }
    pub fn is_assigned(&self) -> bool {
        self.num_faces > 0
    }
}

// ---------------------------------------------------------------------------
// TopologyError
// ---------------------------------------------------------------------------

/// Topology validation error codes.
/// Mirrors C++ `Vtr::internal::Level::TopologyError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TopologyError {
    MissingEdgeFaces = 0,
    MissingEdgeVerts,
    MissingFaceEdges,
    MissingFaceVerts,
    MissingVertFaces,
    MissingVertEdges,
    FailedCorrelationEdgeFace,
    FailedCorrelationFaceVert,
    FailedCorrelationFaceEdge,
    FailedOrientationIncidentEdge,
    FailedOrientationIncidentFace,
    FailedOrientationIncidentFacesEdges,
    DegenerateEdge,
    NonManifoldEdge,
    InvalidCreaseEdge,
    InvalidCreaseVert,
}

impl TopologyError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingEdgeFaces => "Missing edge-face relation",
            Self::MissingEdgeVerts => "Missing edge-vert relation",
            Self::MissingFaceEdges => "Missing face-edge relation",
            Self::MissingFaceVerts => "Missing face-vert relation",
            Self::MissingVertFaces => "Missing vert-face relation",
            Self::MissingVertEdges => "Missing vert-edge relation",
            Self::FailedCorrelationEdgeFace => "Correlation failure: edge-face",
            Self::FailedCorrelationFaceVert => "Correlation failure: face-vert",
            Self::FailedCorrelationFaceEdge => "Correlation failure: face-edge",
            Self::FailedOrientationIncidentEdge => "Orientation failure: incident edge",
            Self::FailedOrientationIncidentFace => "Orientation failure: incident face",
            Self::FailedOrientationIncidentFacesEdges => {
                "Orientation failure: incident faces+edges"
            }
            Self::DegenerateEdge => "Degenerate edge",
            Self::NonManifoldEdge => "Non-manifold edge",
            Self::InvalidCreaseEdge => "Invalid crease edge",
            Self::InvalidCreaseVert => "Invalid crease vertex",
        }
    }
}

/// Validation error callback function type.
pub type ValidationCallback = fn(TopologyError, &str);

// ---------------------------------------------------------------------------
// Level
// ---------------------------------------------------------------------------

/// A single level of the topology hierarchy.
///
/// Stores all six topological relations in SOA layout.  Variable-arity
/// relations (face-vert, vert-face, etc.) use an interleaved `[count, offset]`
/// pairs vector that is indexed by component index.
///
/// Mirrors C++ `Vtr::internal::Level`.
#[derive(Clone)]
pub struct Level {
    // Component counts
    pub(crate) face_count: i32,
    pub(crate) edge_count: i32,
    pub(crate) vert_count: i32,

    /// Depth within the refinement hierarchy (0 = base level).
    pub(crate) depth: i32,

    pub(crate) max_edge_faces: i32,
    pub(crate) max_valence: i32,

    // --- Face-vertex relation (face → [vert]) ---
    /// Interleaved [count, offset] per face (2*face_count entries).
    pub(crate) face_vert_counts_offsets: Vec<i32>,
    pub(crate) face_vert_indices: Vec<Index>,

    // --- Face-edge relation (face → [edge]) ---
    pub(crate) face_edge_indices: Vec<Index>,

    // --- Edge-vertex relation (edge → [v0, v1]) ---
    pub(crate) edge_vert_indices: Vec<Index>, // always 2 per edge

    // --- Edge-face relation (edge → [face]) ---
    pub(crate) edge_face_counts_offsets: Vec<i32>,
    pub(crate) edge_face_indices: Vec<Index>,
    pub(crate) edge_face_local_indices: Vec<LocalIndex>,

    // --- Vert-face relation (vert → [face]) ---
    pub(crate) vert_face_counts_offsets: Vec<i32>,
    pub(crate) vert_face_indices: Vec<Index>,
    pub(crate) vert_face_local_indices: Vec<LocalIndex>,

    // --- Vert-edge relation (vert → [edge]) ---
    pub(crate) vert_edge_counts_offsets: Vec<i32>,
    pub(crate) vert_edge_indices: Vec<Index>,
    pub(crate) vert_edge_local_indices: Vec<LocalIndex>,

    // --- Component tags ---
    pub(crate) vert_tags: Vec<VTag>,
    pub(crate) edge_tags: Vec<ETag>,
    pub(crate) face_tags: Vec<FTag>,

    // --- Sharpness ---
    pub(crate) edge_sharpness: Vec<f32>,
    pub(crate) vert_sharpness: Vec<f32>,

    // --- Face-varying channels ---
    pub(crate) fvar_channels: Vec<Box<FVarLevel>>,
}

// ---- internal bit helpers ----
#[inline]
fn set_bit(x: &mut u16, mask: u16, v: bool) {
    if v {
        *x |= mask;
    } else {
        *x &= !mask;
    }
}
#[inline]
fn set_bit_u8(x: &mut u8, mask: u8, v: bool) {
    if v {
        *x |= mask;
    } else {
        *x &= !mask;
    }
}

impl Level {
    /// Create an empty base level.
    pub fn new() -> Self {
        Self {
            face_count: 0,
            edge_count: 0,
            vert_count: 0,
            depth: 0,
            max_edge_faces: 0,
            max_valence: 0,
            face_vert_counts_offsets: Vec::new(),
            face_vert_indices: Vec::new(),
            face_edge_indices: Vec::new(),
            edge_vert_indices: Vec::new(),
            edge_face_counts_offsets: Vec::new(),
            edge_face_indices: Vec::new(),
            edge_face_local_indices: Vec::new(),
            vert_face_counts_offsets: Vec::new(),
            vert_face_indices: Vec::new(),
            vert_face_local_indices: Vec::new(),
            vert_edge_counts_offsets: Vec::new(),
            vert_edge_indices: Vec::new(),
            vert_edge_local_indices: Vec::new(),
            vert_tags: Vec::new(),
            edge_tags: Vec::new(),
            face_tags: Vec::new(),
            edge_sharpness: Vec::new(),
            vert_sharpness: Vec::new(),
            fvar_channels: Vec::new(),
        }
    }

    // ---- simple accessors ----

    #[inline]
    pub fn get_depth(&self) -> i32 {
        self.depth
    }
    #[inline]
    pub fn get_num_vertices(&self) -> i32 {
        self.vert_count
    }
    #[inline]
    pub fn get_num_faces(&self) -> i32 {
        self.face_count
    }
    #[inline]
    pub fn get_num_edges(&self) -> i32 {
        self.edge_count
    }
    #[inline]
    pub fn get_num_face_vertices_total(&self) -> i32 {
        self.face_vert_indices.len() as i32
    }
    #[inline]
    pub fn get_num_face_edges_total(&self) -> i32 {
        self.face_edge_indices.len() as i32
    }
    #[inline]
    pub fn get_num_edge_vertices_total(&self) -> i32 {
        self.edge_vert_indices.len() as i32
    }
    #[inline]
    pub fn get_num_edge_faces_total(&self) -> i32 {
        self.edge_face_indices.len() as i32
    }
    #[inline]
    pub fn get_num_vertex_faces_total(&self) -> i32 {
        self.vert_face_indices.len() as i32
    }
    #[inline]
    pub fn get_num_vertex_edges_total(&self) -> i32 {
        self.vert_edge_indices.len() as i32
    }
    #[inline]
    pub fn get_max_valence(&self) -> i32 {
        self.max_valence
    }
    #[inline]
    pub fn get_max_edge_faces(&self) -> i32 {
        self.max_edge_faces
    }

    // ---- per-component counts and offsets ----

    #[inline]
    pub fn get_num_face_vertices(&self, f: Index) -> i32 {
        self.face_vert_counts_offsets[2 * f as usize]
    }
    #[inline]
    pub fn get_offset_of_face_vertices(&self, f: Index) -> i32 {
        self.face_vert_counts_offsets[2 * f as usize + 1]
    }
    #[inline]
    pub fn get_num_face_edges(&self, f: Index) -> i32 {
        self.get_num_face_vertices(f)
    }
    #[inline]
    pub fn get_offset_of_face_edges(&self, f: Index) -> i32 {
        self.get_offset_of_face_vertices(f)
    }
    #[inline]
    pub fn get_num_edge_vertices(&self, _e: Index) -> i32 {
        2
    }
    #[inline]
    pub fn get_offset_of_edge_vertices(&self, e: Index) -> i32 {
        2 * e
    }
    #[inline]
    pub fn get_num_edge_faces(&self, e: Index) -> i32 {
        self.edge_face_counts_offsets[2 * e as usize]
    }
    #[inline]
    pub fn get_offset_of_edge_faces(&self, e: Index) -> i32 {
        self.edge_face_counts_offsets[2 * e as usize + 1]
    }
    #[inline]
    pub fn get_num_vertex_faces(&self, v: Index) -> i32 {
        self.vert_face_counts_offsets[2 * v as usize]
    }
    #[inline]
    pub fn get_offset_of_vertex_faces(&self, v: Index) -> i32 {
        self.vert_face_counts_offsets[2 * v as usize + 1]
    }
    #[inline]
    pub fn get_num_vertex_edges(&self, v: Index) -> i32 {
        self.vert_edge_counts_offsets[2 * v as usize]
    }
    #[inline]
    pub fn get_offset_of_vertex_edges(&self, v: Index) -> i32 {
        self.vert_edge_counts_offsets[2 * v as usize + 1]
    }

    // ---- relation accessors (immutable) ----

    pub fn get_face_vertices(&self, f: Index) -> ConstIndexArray<'_> {
        let count = self.get_num_face_vertices(f) as usize;
        let offset = self.get_offset_of_face_vertices(f) as usize;
        ConstIndexArray::new(&self.face_vert_indices[offset..offset + count])
    }
    pub fn get_face_edges(&self, f: Index) -> ConstIndexArray<'_> {
        let count = self.get_num_face_edges(f) as usize;
        let offset = self.get_offset_of_face_edges(f) as usize;
        ConstIndexArray::new(&self.face_edge_indices[offset..offset + count])
    }
    pub fn get_edge_vertices(&self, e: Index) -> ConstIndexArray<'_> {
        let offset = self.get_offset_of_edge_vertices(e) as usize;
        ConstIndexArray::new(&self.edge_vert_indices[offset..offset + 2])
    }
    pub fn get_edge_faces(&self, e: Index) -> ConstIndexArray<'_> {
        let count = self.get_num_edge_faces(e) as usize;
        let offset = self.get_offset_of_edge_faces(e) as usize;
        ConstIndexArray::new(&self.edge_face_indices[offset..offset + count])
    }
    pub fn get_vertex_faces(&self, v: Index) -> ConstIndexArray<'_> {
        let count = self.get_num_vertex_faces(v) as usize;
        let offset = self.get_offset_of_vertex_faces(v) as usize;
        ConstIndexArray::new(&self.vert_face_indices[offset..offset + count])
    }
    pub fn get_vertex_edges(&self, v: Index) -> ConstIndexArray<'_> {
        let count = self.get_num_vertex_edges(v) as usize;
        let offset = self.get_offset_of_vertex_edges(v) as usize;
        ConstIndexArray::new(&self.vert_edge_indices[offset..offset + count])
    }

    pub fn get_edge_face_local_indices(&self, e: Index) -> ConstLocalIndexArray<'_> {
        let count = self.get_num_edge_faces(e) as usize;
        let offset = self.get_offset_of_edge_faces(e) as usize;
        ConstLocalIndexArray::new(&self.edge_face_local_indices[offset..offset + count])
    }
    pub fn get_vertex_face_local_indices(&self, v: Index) -> ConstLocalIndexArray<'_> {
        let count = self.get_num_vertex_faces(v) as usize;
        let offset = self.get_offset_of_vertex_faces(v) as usize;
        ConstLocalIndexArray::new(&self.vert_face_local_indices[offset..offset + count])
    }
    pub fn get_vertex_edge_local_indices(&self, v: Index) -> ConstLocalIndexArray<'_> {
        let count = self.get_num_vertex_edges(v) as usize;
        let offset = self.get_offset_of_vertex_edges(v) as usize;
        ConstLocalIndexArray::new(&self.vert_edge_local_indices[offset..offset + count])
    }

    // ---- relation accessors (mutable) ----

    pub fn get_face_vertices_mut(&mut self, f: Index) -> IndexArray<'_> {
        let count = self.face_vert_counts_offsets[2 * f as usize] as usize;
        let offset = self.face_vert_counts_offsets[2 * f as usize + 1] as usize;
        IndexArray::new(&mut self.face_vert_indices[offset..offset + count])
    }
    pub fn get_face_edges_mut(&mut self, f: Index) -> IndexArray<'_> {
        let count = self.face_vert_counts_offsets[2 * f as usize] as usize;
        let offset = self.face_vert_counts_offsets[2 * f as usize + 1] as usize;
        IndexArray::new(&mut self.face_edge_indices[offset..offset + count])
    }
    pub fn get_edge_vertices_mut(&mut self, e: Index) -> IndexArray<'_> {
        let offset = (2 * e) as usize;
        IndexArray::new(&mut self.edge_vert_indices[offset..offset + 2])
    }
    pub fn get_edge_faces_mut(&mut self, e: Index) -> IndexArray<'_> {
        let count = self.edge_face_counts_offsets[2 * e as usize] as usize;
        let offset = self.edge_face_counts_offsets[2 * e as usize + 1] as usize;
        IndexArray::new(&mut self.edge_face_indices[offset..offset + count])
    }
    pub fn get_vertex_faces_mut(&mut self, v: Index) -> IndexArray<'_> {
        let count = self.vert_face_counts_offsets[2 * v as usize] as usize;
        let offset = self.vert_face_counts_offsets[2 * v as usize + 1] as usize;
        IndexArray::new(&mut self.vert_face_indices[offset..offset + count])
    }
    pub fn get_vertex_edges_mut(&mut self, v: Index) -> IndexArray<'_> {
        let count = self.vert_edge_counts_offsets[2 * v as usize] as usize;
        let offset = self.vert_edge_counts_offsets[2 * v as usize + 1] as usize;
        IndexArray::new(&mut self.vert_edge_indices[offset..offset + count])
    }

    pub fn get_edge_face_local_indices_mut(&mut self, e: Index) -> LocalIndexArray<'_> {
        let count = self.edge_face_counts_offsets[2 * e as usize] as usize;
        let offset = self.edge_face_counts_offsets[2 * e as usize + 1] as usize;
        LocalIndexArray::new(&mut self.edge_face_local_indices[offset..offset + count])
    }
    pub fn get_vertex_face_local_indices_mut(&mut self, v: Index) -> LocalIndexArray<'_> {
        let count = self.vert_face_counts_offsets[2 * v as usize] as usize;
        let offset = self.vert_face_counts_offsets[2 * v as usize + 1] as usize;
        LocalIndexArray::new(&mut self.vert_face_local_indices[offset..offset + count])
    }
    pub fn get_vertex_edge_local_indices_mut(&mut self, v: Index) -> LocalIndexArray<'_> {
        let count = self.vert_edge_counts_offsets[2 * v as usize] as usize;
        let offset = self.vert_edge_counts_offsets[2 * v as usize + 1] as usize;
        LocalIndexArray::new(&mut self.vert_edge_local_indices[offset..offset + count])
    }

    // ---- all face vertices (flat array) ----
    pub fn get_all_face_vertices(&self) -> ConstIndexArray<'_> {
        ConstIndexArray::new(&self.face_vert_indices)
    }

    // ---- sharpness ----
    #[inline]
    pub fn get_edge_sharpness(&self, e: Index) -> f32 {
        self.edge_sharpness[e as usize]
    }
    #[inline]
    pub fn get_edge_sharpness_mut(&mut self, e: Index) -> &mut f32 {
        &mut self.edge_sharpness[e as usize]
    }
    #[inline]
    pub fn get_vertex_sharpness(&self, v: Index) -> f32 {
        self.vert_sharpness[v as usize]
    }
    #[inline]
    pub fn get_vertex_sharpness_mut(&mut self, v: Index) -> &mut f32 {
        &mut self.vert_sharpness[v as usize]
    }
    #[inline]
    pub fn get_vertex_rule(&self, v: Index) -> Rule {
        // Rule is stored in VTag bits [10:7]; extract and convert to Rule enum.
        Rule::from_bits(self.vert_tags[v as usize].rule() as u8)
    }

    // ---- edge lookup ----

    /// Find the edge index connecting vertices `v0` and `v1`, or INDEX_INVALID.
    pub fn find_edge(&self, v0: Index, v1: Index) -> Index {
        let v0_edges = self.get_vertex_edges(v0);
        self.find_edge_with_edges(v0, v1, v0_edges)
    }

    pub fn find_edge_with_edges(&self, v0: Index, v1: Index, v0_edges: ConstIndexArray) -> Index {
        for i in 0..v0_edges.size() {
            let e = v0_edges[i];
            let ev = self.get_edge_vertices(e);
            if (ev[0] == v0 && ev[1] == v1) || (ev[0] == v1 && ev[1] == v0) {
                return e;
            }
        }
        INDEX_INVALID
    }

    // ---- tags ----
    #[inline]
    pub fn get_vertex_tag(&self, v: Index) -> VTag {
        self.vert_tags[v as usize]
    }
    #[inline]
    pub fn get_edge_tag(&self, e: Index) -> ETag {
        self.edge_tags[e as usize]
    }
    #[inline]
    pub fn get_face_tag(&self, f: Index) -> FTag {
        self.face_tags[f as usize]
    }
    #[inline]
    pub fn get_vertex_tag_mut(&mut self, v: Index) -> &mut VTag {
        &mut self.vert_tags[v as usize]
    }
    #[inline]
    pub fn get_edge_tag_mut(&mut self, e: Index) -> &mut ETag {
        &mut self.edge_tags[e as usize]
    }
    #[inline]
    pub fn get_face_tag_mut(&mut self, f: Index) -> &mut FTag {
        &mut self.face_tags[f as usize]
    }

    // ---- holes ----
    pub fn set_face_hole(&mut self, f: Index, b: bool) {
        self.face_tags[f as usize].set_hole(b);
    }
    pub fn is_face_hole(&self, f: Index) -> bool {
        self.face_tags[f as usize].hole()
    }

    // ---- non-manifold ----
    pub fn set_edge_non_manifold(&mut self, e: Index, b: bool) {
        self.edge_tags[e as usize].set_non_manifold(b);
    }
    pub fn is_edge_non_manifold(&self, e: Index) -> bool {
        self.edge_tags[e as usize].non_manifold()
    }
    pub fn set_vertex_non_manifold(&mut self, v: Index, b: bool) {
        self.vert_tags[v as usize].set_non_manifold(b);
    }
    pub fn is_vertex_non_manifold(&self, v: Index) -> bool {
        self.vert_tags[v as usize].non_manifold()
    }

    /// Return true if the non-manifold vertex lies on an interior non-manifold
    /// crease (two non-manifold edges with equal face counts, forming a pair of
    /// manifold subsets each bounded by both crease edges).
    ///
    /// Mirrors C++ `Level::testVertexNonManifoldCrease()`.
    pub fn test_vertex_non_manifold_crease(&self, v: Index) -> bool {
        // Find exactly two non-manifold edges around this vertex.
        let v_edges = self.get_vertex_edges(v);
        let mut non_man_edges: [Index; 2] = [INDEX_INVALID, INDEX_INVALID];
        for i in 0..v_edges.size() {
            let e = v_edges[i];
            if self.is_edge_non_manifold(e) {
                if non_man_edges[1] != INDEX_INVALID {
                    return false; // more than 2 non-manifold edges
                }
                if non_man_edges[0] == INDEX_INVALID {
                    non_man_edges[0] = e;
                } else {
                    non_man_edges[1] = e;
                }
            } else if self.get_num_edge_faces(e) != 2 {
                return false; // manifold edge with wrong face count
            }
        }
        if non_man_edges[0] == INDEX_INVALID || non_man_edges[1] == INDEX_INVALID {
            return false;
        }
        // Both non-manifold edges must have the same number of incident faces.
        if self.get_num_edge_faces(non_man_edges[0]) != self.get_num_edge_faces(non_man_edges[1]) {
            return false;
        }

        // For each of the two non-manifold edges, verify that the manifold
        // subset of faces connected to it is bounded by both crease edges.
        let mut num_traversed = 0usize;
        for i in 0..2usize {
            let e_index = non_man_edges[i];
            let e_start = non_man_edges[if i != 0 { 0 } else { 1 }];
            let e_end = non_man_edges[if i == 0 { 0 } else { 1 }];

            let e_faces = self.get_edge_faces(e_index);
            let e_in_face = self.get_edge_face_local_indices(e_index);

            for j in 0..e_faces.size() {
                let f_start = e_faces[j];
                let f_verts = self.get_face_vertices(f_start);
                // Skip faces where edge is reversed (belongs to the other subset)
                if f_verts[e_in_face[j] as usize] != v {
                    continue;
                }

                let mut e_next = e_start;
                let mut f_next = f_start;
                loop {
                    if e_next != e_start {
                        let f_pair = self.get_edge_faces(e_next);
                        let next_idx = if f_pair[0] == f_next { 1 } else { 0 };
                        f_next = f_pair[next_idx];
                    }
                    num_traversed += 1;

                    let f_edges = self.get_face_edges(f_next);
                    let i_next = f_edges.find_index(e_next) as usize;
                    let prev_i = if i_next > 0 {
                        i_next - 1
                    } else {
                        (f_edges.size() - 1) as usize
                    };
                    e_next = f_edges[prev_i];
                    if e_next == e_end {
                        return false;
                    }
                    if e_next == e_start {
                        break;
                    }
                }
            }
        }
        num_traversed == self.get_num_vertex_faces(v) as usize
    }

    // ---- face-varying ----
    #[inline]
    pub fn get_num_fvar_channels(&self) -> i32 {
        self.fvar_channels.len() as i32
    }
    pub fn get_num_fvar_values(&self, channel: i32) -> i32 {
        self.fvar_channels[channel as usize].get_num_values()
    }
    pub fn get_face_fvar_values(&self, f: Index, channel: i32) -> ConstIndexArray<'_> {
        self.fvar_channels[channel as usize].get_face_values(f)
    }
    pub fn get_face_fvar_values_mut(&mut self, f: Index, channel: i32) -> IndexArray<'_> {
        self.fvar_channels[channel as usize].get_face_values_mut(f)
    }
    pub fn get_fvar_level(&self, channel: i32) -> &FVarLevel {
        &self.fvar_channels[channel as usize]
    }
    pub fn get_fvar_level_mut(&mut self, channel: i32) -> &mut FVarLevel {
        &mut self.fvar_channels[channel as usize]
    }
    pub fn get_fvar_options(&self, channel: i32) -> Options {
        self.fvar_channels[channel as usize].get_options()
    }

    /// Create a new FVar channel with `num_values` distinct values.
    /// Returns the channel index.
    pub fn create_fvar_channel(&mut self, num_values: i32, options: Options) -> i32 {
        let channel = self.fvar_channels.len() as i32;
        let mut fvar = Box::new(FVarLevel::new(self as *const Level));
        fvar.set_options(options);
        fvar.resize(
            num_values,
            self.face_count,
            self.face_vert_indices.len() as i32,
        );
        self.fvar_channels.push(fvar);
        channel
    }

    /// Destroy a FVar channel by index.
    pub fn destroy_fvar_channel(&mut self, channel: i32) {
        self.fvar_channels.remove(channel as usize);
    }

    // ---- fvar topology queries ----
    /// Check if vertex FVar topology matches for the given channel.
    /// Delegates to FVarLevel::value_topology_matches for the vertex's first value.
    pub fn does_vertex_fvar_topology_match(&self, v: Index, channel: i32) -> bool {
        let fvar = self.get_fvar_level(channel);
        let offset = fvar.get_vertex_value_offset(v, 0);
        fvar.value_topology_matches(offset)
    }

    /// Check if face FVar topology matches for the given channel.
    /// Returns true if none of the face's FVar value tags indicate a mismatch.
    pub fn does_face_fvar_topology_match(&self, f: Index, channel: i32) -> bool {
        !self
            .get_fvar_level(channel)
            .get_face_composite_value_tag(f)
            .is_mismatch()
    }

    /// Check if edge FVar topology matches for the given channel.
    /// Delegates to FVarLevel::edge_topology_matches.
    pub fn does_edge_fvar_topology_match(&self, e: Index, channel: i32) -> bool {
        self.get_fvar_level(channel).edge_topology_matches(e)
    }

    /// Collect VTags for each vertex of face `f`.
    /// If `fvar_channel >= 0`, returns FVar-combined tags per C++ getFaceVTags.
    pub fn get_face_vtags(&self, f: Index, vtags: &mut [VTag], fvar_channel: i32) {
        let fv = self.get_face_vertices(f);
        if fvar_channel < 0 {
            for i in 0..fv.size() as usize {
                vtags[i] = self.get_vertex_tag(fv[i]);
            }
        } else {
            let fvar = self.get_fvar_level(fvar_channel);
            let f_values = fvar.get_face_values(f);
            for i in 0..fv.size() as usize {
                let value_index = fvar.find_vertex_value_index(fv[i], f_values[i]);
                let value_tag = fvar.get_value_tag(value_index);
                vtags[i] = value_tag.combine_with_level_vtag(self.get_vertex_tag(fv[i]));
            }
        }
    }

    /// Composite VTag for the face (bitwise OR over corners).
    /// With `fvar_channel >= 0`, combines FVar value tags per C++ getFaceCompositeVTag.
    pub fn get_face_composite_vtag(&self, f: Index) -> VTag {
        let fv = self.get_face_vertices(f);
        let mut bits = 0u16;
        for i in 0..fv.size() as usize {
            bits |= self.get_vertex_tag(fv[i]).get_bits();
        }
        VTag(bits)
    }

    /// Composite VTag for the face, optionally combining FVar value tags.
    pub fn get_face_composite_vtag_with_fvar(&self, f: Index, fvar_channel: i32) -> VTag {
        let fv = self.get_face_vertices(f);
        if fvar_channel < 0 {
            return self.get_face_composite_vtag(f);
        }
        let fvar = self.get_fvar_level(fvar_channel);
        // Build per-vertex ValueTags for this face, then combine with level VTags
        let mut bits = 0u16;
        let f_values = fvar.get_face_values(f);
        for i in 0..fv.size() as usize {
            let value_index = fvar.find_vertex_value_index(fv[i], f_values[i]);
            let value_tag = fvar.get_value_tag(value_index);
            let combined = value_tag.combine_with_level_vtag(self.get_vertex_tag(fv[i]));
            bits |= combined.get_bits();
        }
        VTag(bits)
    }

    /// Composite VTag for a vertex across all its FVar values.
    /// Mirrors C++ `Level::getVertexCompositeFVarVTag`.
    pub fn get_vertex_composite_fvar_vtag(&self, v: Index, channel: i32) -> VTag {
        let fvar = self.get_fvar_level(channel);
        let fv_tags = fvar.get_vertex_value_tags(v);
        let v_tag = self.get_vertex_tag(v);
        // If the first value is a mismatch, combine all value tags
        if fv_tags[0usize].is_mismatch() {
            let mut bits = fv_tags[0usize].combine_with_level_vtag(v_tag).get_bits();
            for i in 1..fv_tags.size() as usize {
                bits |= fv_tags[i].combine_with_level_vtag(v_tag).get_bits();
            }
            VTag(bits)
        } else {
            v_tag
        }
    }

    /// Collect ETags for each edge of face `f`.
    /// If `fvar_channel >= 0`, merges FVar edge tags per C++ getFaceETags.
    pub fn get_face_etags(&self, f: Index, etags: &mut [ETag], fvar_channel: i32) {
        let fe = self.get_face_edges(f);
        if fvar_channel < 0 {
            for i in 0..fe.size() as usize {
                etags[i] = self.get_edge_tag(fe[i as i32]);
            }
        } else {
            let fvar = self.get_fvar_level(fvar_channel);
            for i in 0..fe.size() as usize {
                let fvar_etag = fvar.get_edge_tag(fe[i as i32]);
                etags[i] = fvar_etag.combine_with_level_etag(self.get_edge_tag(fe[i as i32]));
            }
        }
    }

    /// Tests whether a quad face forms a single-crease patch.
    /// Matches C++ `Level::isSingleCreasePatch` exactly:
    /// 1) Composite VTag guard (reject corner/dart/boundary/xordinary/nonManifold)
    /// 2) Build 4-bit crease-vertex mask, look up sharpEdgeFromCreaseMask[16]
    /// 3) Verify sharpness symmetry on all edges of both Crease vertices
    pub fn is_single_crease_patch_full(
        &self,
        f: Index,
        sharpness: &mut f32,
        edge_in_face: &mut i32,
    ) -> bool {
        let fv = self.get_face_vertices(f);
        if fv.size() != 4 {
            return false;
        }

        // Composite VTag for all face corners — safe to use for scalar flags only.
        // Do NOT use all_tag.rule() for rule classification: the composite OR's all
        // corner rule bits together, making the 4-bit field meaningless as a single rule.
        let all_tag = self.get_face_composite_vtag(f);

        // Cheap early-reject: any boundary/xordinary/non-manifold vertex disqualifies.
        if all_tag.boundary() || all_tag.xordinary() || all_tag.non_manifold() {
            return false;
        }

        // Build 4-bit mask: bit i set if vertex i individually has Rule::Crease.
        // This matches C++ which checks getVertexRule(fVerts[i]) == RULE_CREASE per vertex.
        let crease_mask = ((self.get_vertex_tag(fv[0]).rule() == Rule::Crease as u16) as usize)
            | (((self.get_vertex_tag(fv[1]).rule() == Rule::Crease as u16) as usize) << 1)
            | (((self.get_vertex_tag(fv[2]).rule() == Rule::Crease as u16) as usize) << 2)
            | (((self.get_vertex_tag(fv[3]).rule() == Rule::Crease as u16) as usize) << 3);

        // Early-out: no crease vertices at all — can't be a single-crease patch.
        if crease_mask == 0 {
            return false;
        }

        // Lookup table: exactly 2 adjacent crease vertices → edge index between them
        const SHARP_EDGE_FROM_CREASE_MASK: [i32; 16] =
            [-1, -1, -1, 0, -1, -1, 1, -1, -1, 3, -1, -1, 2, -1, -1, -1];

        let sharp_edge = SHARP_EDGE_FROM_CREASE_MASK[crease_mask];
        if sharp_edge < 0 {
            return false;
        }

        // Verify sharpness symmetry: opposing edges of each Crease vertex must match
        let va_edges = self.get_vertex_edges(fv[sharp_edge as usize]);
        let vb_edges = self.get_vertex_edges(fv[((sharp_edge + 1) & 3) as usize]);

        if va_edges.size() != 4 || vb_edges.size() != 4 {
            return false;
        }

        if self.get_edge_sharpness(va_edges[0]) != self.get_edge_sharpness(va_edges[2])
            || self.get_edge_sharpness(va_edges[1]) != self.get_edge_sharpness(va_edges[3])
            || self.get_edge_sharpness(vb_edges[0]) != self.get_edge_sharpness(vb_edges[2])
            || self.get_edge_sharpness(vb_edges[1]) != self.get_edge_sharpness(vb_edges[3])
        {
            return false;
        }

        *sharpness = self.get_edge_sharpness(self.get_face_edges(f)[sharp_edge]);
        *edge_in_face = sharp_edge;
        true
    }

    /// Simple bool version for backward compat.
    pub fn is_single_crease_patch(&self, f: Index) -> bool {
        let mut s = 0.0f32;
        let mut e = 0i32;
        self.is_single_crease_patch_full(f, &mut s, &mut e)
    }

    // ---- sizing methods (called by factory during construction) ----

    /// Allocate storage for `n` faces (just the count+offset arrays).
    pub fn resize_faces(&mut self, n: i32) {
        self.face_count = n;
        self.face_vert_counts_offsets.resize((2 * n) as usize, 0);
        self.face_tags.resize(n as usize, FTag::default());
    }

    /// Allocate storage for all face-vertex indices.
    /// Mirrors C++ `Level::resizeFaceVertices(int)` — only resizes `_faceVertIndices`.
    pub fn resize_face_vertices_total(&mut self, total: i32) {
        self.face_vert_indices.resize(total as usize, INDEX_INVALID);
    }

    /// Allocate storage for all face-edge indices.
    /// Mirrors C++ `Level::resizeFaceEdges(int)` — only resizes `_faceEdgeIndices`.
    pub fn resize_face_edges_total(&mut self, total: i32) {
        self.face_edge_indices.resize(total as usize, INDEX_INVALID);
    }

    /// Convenience: resize both face-vert and face-edge index arrays to `total`.
    /// Used internally when both are known to have the same total (uniform meshes).
    pub fn resize_face_vertices_and_edges_total(&mut self, total: i32) {
        self.face_vert_indices.resize(total as usize, INDEX_INVALID);
        self.face_edge_indices.resize(total as usize, INDEX_INVALID);
    }

    /// Set the vertex count for a single face (called in order, builds offsets).
    pub fn resize_face_vertices(&mut self, f: Index, count: i32) {
        let fi = f as usize;
        let offset = if fi == 0 {
            0
        } else {
            let prev = self.face_vert_counts_offsets[2 * (fi - 1)] as usize;
            let off = self.face_vert_counts_offsets[2 * (fi - 1) + 1] as usize;
            prev + off
        };
        self.face_vert_counts_offsets[2 * fi] = count;
        self.face_vert_counts_offsets[2 * fi + 1] = offset as i32;
        // Track maximum valence (mirrors C++ _maxValence update in resizeFaceVertices(Index,int))
        self.max_valence = self.max_valence.max(count);
    }

    /// Allocate storage for `n` edges — count/offset arrays, tags, sharpness.
    /// Does NOT allocate edge-vert indices; call `resize_edge_vertices()` separately.
    /// Mirrors C++ `Level::resizeEdges(int)` exactly.
    pub fn resize_edges(&mut self, n: i32) {
        self.edge_count = n;
        self.edge_face_counts_offsets.resize((2 * n) as usize, 0);
        self.edge_tags.resize(n as usize, ETag::default());
        self.edge_sharpness.resize(n as usize, 0.0);
    }

    /// Allocate edge-vertex index storage for `2 * edge_count` entries.
    /// Must be called after `resize_edges()`. Mirrors C++ `Level::resizeEdgeVertices()`.
    pub fn resize_edge_vertices(&mut self) {
        self.edge_vert_indices
            .resize((2 * self.edge_count) as usize, INDEX_INVALID);
    }

    pub fn resize_edge_faces(&mut self, e: Index, count: i32) {
        let ei = e as usize;
        let offset = if ei == 0 {
            0
        } else {
            let prev = self.edge_face_counts_offsets[2 * (ei - 1)] as usize;
            let off = self.edge_face_counts_offsets[2 * (ei - 1) + 1] as usize;
            prev + off
        };
        self.edge_face_counts_offsets[2 * ei] = count;
        self.edge_face_counts_offsets[2 * ei + 1] = offset as i32;
        // Track maximum edge-face count (mirrors C++ _maxEdgeFaces update)
        self.max_edge_faces = self.max_edge_faces.max(count);
    }

    pub fn trim_edge_faces(&mut self, e: Index, count: i32) {
        self.edge_face_counts_offsets[2 * e as usize] = count;
    }

    /// Allocate storage for `n` vertices.
    pub fn resize_vertices(&mut self, n: i32) {
        self.vert_count = n;
        self.vert_face_counts_offsets.resize((2 * n) as usize, 0);
        self.vert_edge_counts_offsets.resize((2 * n) as usize, 0);
        self.vert_tags.resize(n as usize, VTag::default());
        self.vert_sharpness.resize(n as usize, 0.0);
    }

    pub fn resize_vertex_faces(&mut self, v: Index, count: i32) {
        let vi = v as usize;
        let offset = if vi == 0 {
            0
        } else {
            let prev = self.vert_face_counts_offsets[2 * (vi - 1)] as usize;
            let off = self.vert_face_counts_offsets[2 * (vi - 1) + 1] as usize;
            prev + off
        };
        self.vert_face_counts_offsets[2 * vi] = count;
        self.vert_face_counts_offsets[2 * vi + 1] = offset as i32;
    }

    pub fn trim_vertex_faces(&mut self, v: Index, count: i32) {
        self.vert_face_counts_offsets[2 * v as usize] = count;
    }

    pub fn resize_vertex_edges(&mut self, v: Index, count: i32) {
        let vi = v as usize;
        let offset = if vi == 0 {
            0
        } else {
            let prev = self.vert_edge_counts_offsets[2 * (vi - 1)] as usize;
            let off = self.vert_edge_counts_offsets[2 * (vi - 1) + 1] as usize;
            prev + off
        };
        self.vert_edge_counts_offsets[2 * vi] = count;
        self.vert_edge_counts_offsets[2 * vi + 1] = offset as i32;
        // Track maximum valence (mirrors C++ _maxValence update in resizeVertexEdges)
        self.max_valence = self.max_valence.max(count);
    }

    pub fn trim_vertex_edges(&mut self, v: Index, count: i32) {
        self.vert_edge_counts_offsets[2 * v as usize] = count;
    }

    pub fn set_max_valence(&mut self, v: i32) {
        self.max_valence = v;
    }

    // ---- local index population ----

    /// Populate local indices (vertex-in-face, vertex-in-edge, edge-in-face)
    /// for a manifold mesh.  Pre-allocates all arrays upfront to avoid O(n^2)
    /// element-at-a-time resizes.
    /// Mirrors C++ `Level::populateLocalIndices()`.
    pub fn populate_local_indices(&mut self) {
        let total_vf = self.vert_face_indices.len();
        let total_ve = self.vert_edge_indices.len();
        let total_ef = self.edge_face_indices.len();

        // Pre-allocate upfront (C++ uses resizeVertexFaces / resizeVertexEdges which
        // pre-size the local-index arrays alongside the main index arrays).
        self.vert_face_local_indices.resize(total_vf, 0);
        self.vert_edge_local_indices.resize(total_ve, 0);
        self.edge_face_local_indices.resize(total_ef, 0);

        // For each vertex: record local index of vertex in each incident face.
        // Track vFaceLast to handle duplicate faces in non-manifold topology
        // (C++ level.cpp:1819-1829): when the same face appears twice consecutively,
        // start searching from the position after the previous match.
        for v in 0..self.vert_count {
            let vf_count = self.get_num_vertex_faces(v) as usize;
            let vf_offset = self.get_offset_of_vertex_faces(v) as usize;
            let mut v_face_last: Index = INDEX_INVALID;
            for fi in 0..vf_count {
                let f = self.vert_face_indices[vf_offset + fi];
                let fverts = self.get_face_vertices(f);
                // If the same face appears again, start search after the previous local index.
                let v_start = if f == v_face_last {
                    self.vert_face_local_indices[vf_offset + fi - 1] as usize + 1
                } else {
                    0
                };
                let fv = fverts.as_slice();
                let local = fv[v_start..]
                    .iter()
                    .position(|&x| x == v)
                    .map(|p| (v_start + p) as LocalIndex)
                    .unwrap_or(0);
                self.vert_face_local_indices[vf_offset + fi] = local;
                v_face_last = f;
            }

            // For each vertex: record local index (0=first endpoint, 1=second) in edge.
            // Handle degenerate edges (both endpoints equal) per C++ level.cpp:1843-1847:
            // the first occurrence gets local index 0, the second gets 1.
            let ve_count = self.get_num_vertex_edges(v) as usize;
            let ve_offset = self.get_offset_of_vertex_edges(v) as usize;
            for ei in 0..ve_count {
                let e = self.vert_edge_indices[ve_offset + ei];
                let ev = self.get_edge_vertices(e);
                self.vert_edge_local_indices[ve_offset + ei] = if ev[0] != ev[1] {
                    // Normal edge: local index is which endpoint matches v.
                    (ev[0] != v) as LocalIndex
                } else {
                    // Degenerate edge (ev[0] == ev[1]): first occurrence = 0, second = 1.
                    (ei > 0 && self.vert_edge_indices[ve_offset + ei - 1] == e) as LocalIndex
                };
            }
        }

        // For each edge: record local index of edge in each incident face.
        // Track eFaceLast to handle duplicate faces in non-manifold topology
        // (C++ level.cpp:1862-1872): same as the vFaceLast pattern above.
        for e in 0..self.edge_count {
            let ef_count = self.get_num_edge_faces(e) as usize;
            let ef_offset = self.get_offset_of_edge_faces(e) as usize;
            let mut e_face_last: Index = INDEX_INVALID;
            for fi in 0..ef_count {
                let f = self.edge_face_indices[ef_offset + fi];
                let (fcount, foffset) = {
                    let co = &self.face_vert_counts_offsets;
                    (co[2 * f as usize] as usize, co[2 * f as usize + 1] as usize)
                };
                let fedges = &self.face_edge_indices[foffset..foffset + fcount];
                // If the same face appears again, start search after the previous local index.
                let e_start = if f == e_face_last {
                    self.edge_face_local_indices[ef_offset + fi - 1] as usize + 1
                } else {
                    0
                };
                let local = fedges[e_start..]
                    .iter()
                    .position(|&x| x == e)
                    .map(|p| (e_start + p) as LocalIndex)
                    .unwrap_or(0);
                self.edge_face_local_indices[ef_offset + fi] = local;
                e_face_last = f;
            }
        }
    }

    // ---- topology completion ----

    /// Orient vert-faces and vert-edges in CCW order for each manifold vertex.
    /// Non-manifold vertices (already tagged) are skipped; vertices that fail
    /// orientation are tagged non-manifold. Matches C++ `orientIncidentComponents()`.
    pub fn orient_incident_components(&mut self) {
        for v in 0..self.vert_count {
            if !self.vert_tags[v as usize].non_manifold() {
                if !self.order_vertex_faces_and_edges(v) {
                    self.vert_tags[v as usize].set_non_manifold(true);
                }
            }
        }
    }

    /// Order vertex incident faces and edges in CCW winding.
    /// Returns false if vertex is non-manifold (cannot be ordered).
    /// Matches C++ `Level::orderVertexFacesAndEdges(Index vIndex)`.
    pub fn order_vertex_faces_and_edges(&mut self, v: Index) -> bool {
        let vf_count = self.get_num_vertex_faces(v) as usize;
        let vf_offset = self.get_offset_of_vertex_faces(v) as usize;
        let ve_count = self.get_num_vertex_edges(v) as usize;
        let ve_offset = self.get_offset_of_vertex_edges(v) as usize;

        // Snapshot current unordered lists
        let v_faces: Vec<Index> = self.vert_face_indices[vf_offset..vf_offset + vf_count].to_vec();
        let v_edges: Vec<Index> = self.vert_edge_indices[ve_offset..ve_offset + ve_count].to_vec();

        let f_count = v_faces.len();
        let e_count = v_edges.len();

        if f_count == 0 || e_count < 2 || (e_count as isize - f_count as isize) > 1 {
            return false;
        }

        // Helper: find index of `val` in a slice
        fn find_in(arr: &[Index], val: Index) -> usize {
            arr.iter().position(|&x| x == val).unwrap_or(arr.len())
        }

        let mut f_start: Index;
        let mut fv_start: usize;
        let mut e_start: Index;

        if e_count == f_count {
            // Interior: start with first face
            f_start = v_faces[0];
            let fverts = self.get_face_vertices(f_start);
            let fv_sl: Vec<Index> = (0..fverts.size()).map(|i| fverts[i]).collect();
            fv_start = find_in(&fv_sl, v);
            let fedges = self.get_face_edges(f_start);
            e_start = fedges[fv_start as i32];
        } else {
            // Boundary: find the leading boundary edge
            f_start = INDEX_INVALID;
            fv_start = 0;
            e_start = INDEX_INVALID;
            for &ei in &v_edges {
                let ef_count = self.get_num_edge_faces(ei);
                if ef_count == 1 {
                    let ef = self.get_edge_faces(ei);
                    let candidate_f = ef[0];
                    let fverts = self.get_face_vertices(candidate_f);
                    let fv_sl: Vec<Index> = (0..fverts.size()).map(|i| fverts[i]).collect();
                    let fv_pos = find_in(&fv_sl, v);
                    let fedges = self.get_face_edges(candidate_f);
                    // Leading boundary: edge is the "forward" edge from v in this face
                    if ei == fedges[fv_pos as i32] {
                        e_start = ei;
                        f_start = candidate_f;
                        fv_start = fv_pos;
                        break;
                    }
                }
            }
            if !index_is_valid(e_start) {
                return false;
            }
        }

        let mut ordered_faces = Vec::with_capacity(f_count);
        let mut ordered_edges = Vec::with_capacity(e_count);

        ordered_faces.push(f_start);
        ordered_edges.push(e_start);

        let e_first = e_start;

        while ordered_edges.len() < e_count {
            // Find the next edge CCW: the one before the current fv_start in the face
            let fverts = self.get_face_vertices(f_start);
            let fedges = self.get_face_edges(f_start);
            let n = fverts.size() as usize;

            let fe_next = if fv_start > 0 { fv_start - 1 } else { n - 1 };
            let e_next = fedges[fe_next as i32];

            // Detect non-manifold: repeated edge or premature return to start
            if e_next == e_start || e_next == e_first {
                return false;
            }

            ordered_edges.push(e_next);

            if ordered_faces.len() < f_count {
                // Cross to the opposite face of e_next
                let ef = self.get_edge_faces(e_next);
                if ef.size() == 0 {
                    return false;
                }
                if ef.size() == 1 && ef[0] == f_start {
                    return false;
                }

                // Pick the face that is NOT f_start
                f_start = if ef[0] == f_start { ef[1] } else { ef[0] };

                // Find e_next in the new face's edges to get the fv_start
                let new_fedges = self.get_face_edges(f_start);
                let new_sl: Vec<Index> = (0..new_fedges.size()).map(|i| new_fedges[i]).collect();
                fv_start = find_in(&new_sl, e_next);

                ordered_faces.push(f_start);
            }

            e_start = e_next;
        }

        if ordered_edges.len() != e_count || ordered_faces.len() != f_count {
            return false;
        }

        // Write back ordered lists
        self.vert_face_indices[vf_offset..vf_offset + f_count].copy_from_slice(&ordered_faces);
        self.vert_edge_indices[ve_offset..ve_offset + e_count].copy_from_slice(&ordered_edges);

        true
    }

    /// Read-only variant of ordering used by validate_topology().
    /// Fills `out_faces` and `out_edges` with the expected CCW-ordered indices.
    /// Returns false if the vertex neighbourhood is non-manifold / cannot be oriented.
    /// Mirrors C++ `Level::orderVertexFacesAndEdges(Index, Index*, Index*)`.
    fn order_vertex_faces_and_edges_into(
        &self,
        v: Index,
        out_faces: &mut [Index],
        out_edges: &mut [Index],
    ) -> bool {
        let vf_count = self.get_num_vertex_faces(v) as usize;
        let ve_count = self.get_num_vertex_edges(v) as usize;

        let v_faces: &[Index] = &self.vert_face_indices[self.get_offset_of_vertex_faces(v) as usize
            ..self.get_offset_of_vertex_faces(v) as usize + vf_count];
        let v_edges: &[Index] = &self.vert_edge_indices[self.get_offset_of_vertex_edges(v) as usize
            ..self.get_offset_of_vertex_edges(v) as usize + ve_count];

        let f_count = v_faces.len();
        let e_count = v_edges.len();

        if f_count == 0 || e_count < 2 || (e_count as isize - f_count as isize) > 1 {
            return false;
        }

        fn find_in(arr: &[Index], val: Index) -> usize {
            arr.iter().position(|&x| x == val).unwrap_or(arr.len())
        }

        let mut f_start: Index;
        let mut fv_start: usize;
        let mut e_start: Index;

        if e_count == f_count {
            // Interior vertex: start from the first face
            f_start = v_faces[0];
            let fverts = self.get_face_vertices(f_start);
            let fv_sl: Vec<Index> = (0..fverts.size()).map(|i| fverts[i]).collect();
            fv_start = find_in(&fv_sl, v);
            let fedges = self.get_face_edges(f_start);
            e_start = fedges[fv_start as i32];
        } else {
            // Boundary vertex: find the leading boundary edge
            f_start = INDEX_INVALID;
            fv_start = 0;
            e_start = INDEX_INVALID;
            for &ei in v_edges {
                if self.get_num_edge_faces(ei) == 1 {
                    let ef = self.get_edge_faces(ei);
                    let candidate_f = ef[0];
                    let fverts = self.get_face_vertices(candidate_f);
                    let fv_sl: Vec<Index> = (0..fverts.size()).map(|i| fverts[i]).collect();
                    let fv_pos = find_in(&fv_sl, v);
                    let fedges = self.get_face_edges(candidate_f);
                    if ei == fedges[fv_pos as i32] {
                        e_start = ei;
                        f_start = candidate_f;
                        fv_start = fv_pos;
                        break;
                    }
                }
            }
            if !index_is_valid(e_start) {
                return false;
            }
        }

        let mut ordered_faces = Vec::with_capacity(f_count);
        let mut ordered_edges = Vec::with_capacity(e_count);

        ordered_faces.push(f_start);
        ordered_edges.push(e_start);
        let e_first = e_start;

        while ordered_edges.len() < e_count {
            let fverts = self.get_face_vertices(f_start);
            let fedges = self.get_face_edges(f_start);
            let n = fverts.size() as usize;
            let fe_next = if fv_start > 0 { fv_start - 1 } else { n - 1 };
            let e_next = fedges[fe_next as i32];

            if e_next == e_start || e_next == e_first {
                return false;
            }
            ordered_edges.push(e_next);

            if ordered_faces.len() < f_count {
                let ef = self.get_edge_faces(e_next);
                if ef.size() == 0 {
                    return false;
                }
                if ef.size() == 1 && ef[0] == f_start {
                    return false;
                }
                f_start = if ef[0] == f_start { ef[1] } else { ef[0] };
                let new_fedges = self.get_face_edges(f_start);
                let new_sl: Vec<Index> = (0..new_fedges.size()).map(|i| new_fedges[i]).collect();
                fv_start = find_in(&new_sl, e_next);
                ordered_faces.push(f_start);
            }
            e_start = e_next;
        }

        if ordered_faces.len() != f_count || ordered_edges.len() != e_count {
            return false;
        }

        out_faces[..f_count].copy_from_slice(&ordered_faces);
        out_edges[..e_count].copy_from_slice(&ordered_edges);
        true
    }

    /// Build edges and remaining relations from face-vertex data only.
    /// Handles degenerate edges (v0==v1) and non-manifold edges (>2 faces).
    /// Calls `orient_incident_components()` after building topology for CCW order.
    /// Matches C++ `Level::completeTopologyFromFaceVertices()`.
    pub fn complete_topology_from_face_vertices(&mut self) -> bool {
        let face_count = self.face_count as usize;
        let vert_count = self.vert_count as usize;

        assert!(vert_count > 0 && face_count > 0 && self.edge_count == 0);

        // Ensure vert/face storage is allocated
        self.vert_tags.resize(vert_count, VTag::default());
        self.vert_sharpness.resize(vert_count, 0.0);
        self.vert_face_counts_offsets.resize(2 * vert_count, 0);
        self.vert_edge_counts_offsets.resize(2 * vert_count, 0);
        self.face_edge_indices
            .resize(self.face_vert_indices.len(), INDEX_INVALID);

        // Dynamic per-component lists
        let mut vert_face_list: Vec<Vec<Index>> = vec![Vec::new(); vert_count];
        let mut vert_edge_list: Vec<Vec<Index>> = vec![Vec::new(); vert_count];
        let mut edge_face_list: Vec<Vec<Index>> = Vec::new();

        // Track non-manifold edges for later tagging
        let mut non_manifold_edges: Vec<Index> = Vec::new();

        // Iterate faces, build edges
        for f in 0..face_count {
            let fv_count = self.face_vert_counts_offsets[2 * f] as usize;
            let fv_offset = self.face_vert_counts_offsets[2 * f + 1] as usize;

            for vi in 0..fv_count {
                let v0 = self.face_vert_indices[fv_offset + vi];
                let v1 = self.face_vert_indices[fv_offset + (vi + 1) % fv_count];

                // Search for existing edge in v0's incident edges
                let mut e_index: Index = INDEX_INVALID;

                if v0 != v1 {
                    // Non-degenerate: look for edge [v0,v1] or [v1,v0]
                    for &ei in &vert_edge_list[v0 as usize] {
                        let ev0 = self.edge_vert_indices[2 * ei as usize];
                        let ev1 = self.edge_vert_indices[2 * ei as usize + 1];
                        if (ev0 == v0 && ev1 == v1) || (ev0 == v1 && ev1 == v0) {
                            e_index = ei;
                            break;
                        }
                    }
                } else {
                    // Degenerate edge (v0==v1): always create new, mark non-manifold
                    non_manifold_edges.push(self.edge_count);
                }

                // Check for non-manifold conditions on existing edge
                if index_is_valid(e_index) {
                    let ef = &edge_face_list[e_index as usize];
                    if ef.last() == Some(&(f as Index)) {
                        // Edge already in this face — create new instance
                        non_manifold_edges.push(e_index);
                        non_manifold_edges.push(self.edge_count);
                        e_index = INDEX_INVALID;
                    } else if ef.len() > 1 {
                        // More than 2 faces sharing edge
                        non_manifold_edges.push(e_index);
                    } else if v0 == self.edge_vert_indices[2 * e_index as usize] {
                        // Same winding as first face — non-manifold
                        non_manifold_edges.push(e_index);
                    }
                }

                // Create new edge if needed
                if !index_is_valid(e_index) {
                    e_index = self.edge_count;
                    self.edge_count += 1;
                    self.edge_vert_indices.push(v0);
                    self.edge_vert_indices.push(v1);
                    edge_face_list.push(Vec::new());

                    vert_edge_list[v0 as usize].push(e_index);
                    vert_edge_list[v1 as usize].push(e_index);
                }

                edge_face_list[e_index as usize].push(f as Index);
                vert_face_list[v0 as usize].push(f as Index);
                self.face_edge_indices[fv_offset + vi] = e_index;
            }
        }

        // Build flat vert-face relation
        let total_vf: usize = vert_face_list.iter().map(|v| v.len()).sum();
        self.vert_face_indices.clear();
        self.vert_face_indices.reserve(total_vf);
        self.vert_face_local_indices.resize(total_vf, 0);
        let mut vf_offset = 0usize;
        let mut max_vert_faces = 0;
        for v in 0..vert_count {
            let count = vert_face_list[v].len();
            self.vert_face_counts_offsets[2 * v] = count as i32;
            self.vert_face_counts_offsets[2 * v + 1] = vf_offset as i32;
            self.vert_face_indices.extend_from_slice(&vert_face_list[v]);
            max_vert_faces = max_vert_faces.max(count as i32);
            vf_offset += count;
        }

        // Build flat vert-edge relation
        let total_ve: usize = vert_edge_list.iter().map(|v| v.len()).sum();
        self.vert_edge_indices.clear();
        self.vert_edge_indices.reserve(total_ve);
        self.vert_edge_local_indices.resize(total_ve, 0);
        let mut ve_offset = 0usize;
        let mut max_vert_edges = 0;
        for v in 0..vert_count {
            let count = vert_edge_list[v].len();
            self.vert_edge_counts_offsets[2 * v] = count as i32;
            self.vert_edge_counts_offsets[2 * v + 1] = ve_offset as i32;
            self.vert_edge_indices.extend_from_slice(&vert_edge_list[v]);
            max_vert_edges = max_vert_edges.max(count as i32);
            ve_offset += count;
        }

        // Build flat edge-face relation
        let edge_count = self.edge_count as usize;
        let total_ef: usize = edge_face_list.iter().map(|v| v.len()).sum();
        self.edge_face_indices.clear();
        self.edge_face_indices.reserve(total_ef);
        self.edge_face_local_indices.resize(total_ef, 0);
        self.edge_face_counts_offsets.resize(2 * edge_count, 0);
        self.edge_tags.resize(edge_count, ETag::default());
        self.edge_sharpness.resize(edge_count, 0.0);
        let mut ef_offset = 0usize;
        let mut max_edge_faces = 0i32;
        for e in 0..edge_count {
            let count = edge_face_list[e].len();
            self.edge_face_counts_offsets[2 * e] = count as i32;
            self.edge_face_counts_offsets[2 * e + 1] = ef_offset as i32;
            self.edge_face_indices.extend_from_slice(&edge_face_list[e]);
            max_edge_faces = max_edge_faces.max(count as i32);
            ef_offset += count;
        }

        // Update maxima
        self.max_edge_faces = max_edge_faces;
        self.max_valence = self.max_valence.max(max_vert_faces).max(max_vert_edges);

        const VALENCE_LIMIT: i32 = 0xFFFF; // LocalIndex::MAX
        if self.max_valence > VALENCE_LIMIT {
            return false;
        }

        // Tag non-manifold edges and their incident vertices
        for &ei in &non_manifold_edges {
            if (ei as usize) < edge_count {
                self.edge_tags[ei as usize].set_non_manifold(true);
                let ev0 = self.edge_vert_indices[2 * ei as usize];
                let ev1 = self.edge_vert_indices[2 * ei as usize + 1];
                if (ev0 as usize) < vert_count {
                    self.vert_tags[ev0 as usize].set_non_manifold(true);
                }
                if (ev1 as usize) < vert_count {
                    self.vert_tags[ev1 as usize].set_non_manifold(true);
                }
            }
        }

        // Orient incident components in CCW order (marks non-manifold on failure)
        self.orient_incident_components();

        // Populate local indices for all relations
        self.populate_local_indices();

        // Tag boundary edges and vertices
        for e in 0..edge_count {
            if self.edge_face_counts_offsets[2 * e] == 1 {
                self.edge_tags[e].set_boundary(true);
            }
        }
        for v in 0..self.vert_count {
            let ve_cnt = self.get_num_vertex_edges(v) as usize;
            let ve_off = self.get_offset_of_vertex_edges(v) as usize;
            let is_boundary = (0..ve_cnt).any(|i| {
                let ei = self.vert_edge_indices[ve_off + i];
                self.edge_tags[ei as usize].boundary()
            });
            self.vert_tags[v as usize].set_boundary(is_boundary);
        }

        true
    }

    // ---- FVar channel completion ----
    /// Complete FVar channel topology by delegating to the full FVarLevel implementation.
    /// Mirrors C++ `Level::completeFVarChannelTopology()` which calls
    /// `FVarLevel::completeTopologyFromFaceValues(regBoundaryValence)`.
    pub fn complete_fvar_channel_topology(&mut self, channel: i32, reg_boundary_valence: i32) {
        // Split borrow: take the channel out, run it, put it back.
        // SAFETY: we're not touching any other field of self during the call.
        self.fvar_channels[channel as usize]
            .complete_topology_from_face_values(reg_boundary_valence);
    }

    // ---- validation ----

    /// Full topology consistency validation.
    /// Mirrors C++ `Level::validateTopology(ValidationCallback callback, void* clientData)`.
    ///
    /// `callback` is an optional function called on each error with
    /// (error_code, description_string).  Pass `None` for silent validation.
    ///
    /// Checks:
    ///   - face-vert <-> vert-face correlation
    ///   - face-edge <-> edge-face correlation
    ///   - edge-vert <-> vert-edge correlation
    ///   - vertex ordering (vert-faces and vert-edges are in CCW order)
    ///   - non-manifold edge tagging (degenerate / wrong face count)
    pub fn validate_topology(&self, callback: Option<ValidationCallback>) -> bool {
        let report = |err: TopologyError| {
            if let Some(cb) = callback {
                cb(err, err.as_str());
            }
        };

        // Abort early when essential relation tables are empty
        if self.get_num_face_vertices_total() == 0 || self.get_num_vertex_faces_total() == 0 {
            report(TopologyError::MissingFaceVerts);
            return false;
        }

        let mut ok = true;

        // ---- face-vert <-> vert-face correlation ----
        'face_vert: for f_index in 0..self.get_num_faces() {
            let f_verts = self.get_face_vertices(f_index);
            for i in 0..f_verts.size() as usize {
                let v_index = f_verts[i];
                let v_faces = self.get_vertex_faces(v_index);
                let v_in_face = self.get_vertex_face_local_indices(v_index);
                let found = (0..v_faces.size() as usize)
                    .any(|j| v_faces[j] == f_index && v_in_face[j] as usize == i);
                if !found {
                    report(TopologyError::FailedCorrelationFaceVert);
                    ok = false;
                    break 'face_vert;
                }
            }
        }

        // ---- face-edge <-> edge-face correlation ----
        if self.get_num_edge_faces_total() == 0 || self.get_num_face_edges_total() == 0 {
            report(TopologyError::MissingFaceEdges);
            return false;
        }
        'face_edge: for f_index in 0..self.get_num_faces() {
            let f_edges = self.get_face_edges(f_index);
            for i in 0..f_edges.size() as usize {
                let e_index = f_edges[i];
                let e_faces = self.get_edge_faces(e_index);
                let e_in_face = self.get_edge_face_local_indices(e_index);
                let found = (0..e_faces.size() as usize)
                    .any(|j| e_faces[j] == f_index && e_in_face[j] as usize == i);
                if !found {
                    report(TopologyError::FailedCorrelationEdgeFace);
                    ok = false;
                    break 'face_edge;
                }
            }
        }

        // ---- edge-vert <-> vert-edge correlation ----
        if self.get_num_edge_vertices_total() == 0 || self.get_num_vertex_edges_total() == 0 {
            report(TopologyError::MissingEdgeVerts);
            return false;
        }
        'edge_vert: for e_index in 0..self.get_num_edges() {
            let e_verts = self.get_edge_vertices(e_index);
            for i in 0..2usize {
                let v_index = e_verts[i];
                let v_edges = self.get_vertex_edges(v_index);
                let v_in_edge = self.get_vertex_edge_local_indices(v_index);
                let found = (0..v_edges.size() as usize)
                    .any(|j| v_edges[j] == e_index && v_in_edge[j] as usize == i);
                if !found {
                    // Intentionally matches C++ level.cpp:264 which also reports
                    // TOPOLOGY_FAILED_CORRELATION_FACE_VERT for edge-vert failures.
                    // This is a pre-existing quirk in the C++ reference, preserved
                    // for exact parity with the upstream error code.
                    report(TopologyError::FailedCorrelationFaceVert);
                    ok = false;
                    break 'edge_vert;
                }
            }
        }

        // ---- vertex face/edge orientation check ----
        let max_val = (self.max_valence as usize).max(4);
        let mut ordered_faces = vec![INDEX_INVALID; max_val];
        let mut ordered_edges = vec![INDEX_INVALID; max_val];

        for v_index in 0..self.get_num_vertices() {
            let vt = self.get_vertex_tag(v_index);
            if vt.incomplete() || vt.non_manifold() {
                continue;
            }

            let v_faces = self.get_vertex_faces(v_index);
            let v_edges = self.get_vertex_edges(v_index);
            let nf = v_faces.size() as usize;
            let ne = v_edges.size() as usize;

            if !self.order_vertex_faces_and_edges_into(
                v_index,
                &mut ordered_faces[..nf],
                &mut ordered_edges[..ne],
            ) {
                report(TopologyError::FailedOrientationIncidentFacesEdges);
                ok = false;
                continue;
            }

            let faces_ok = (0..nf).all(|i| v_faces[i] == ordered_faces[i]);
            let edges_ok = (0..ne).all(|i| v_edges[i] == ordered_edges[i]);
            if !faces_ok {
                report(TopologyError::FailedOrientationIncidentFace);
                ok = false;
            }
            if !edges_ok {
                report(TopologyError::FailedOrientationIncidentEdge);
                ok = false;
            }
        }

        // ---- non-manifold edge tag check ----
        for e_index in 0..self.get_num_edges() {
            let e_tag = self.get_edge_tag(e_index);
            if e_tag.non_manifold() {
                continue;
            }

            let e_verts = self.get_edge_vertices(e_index);
            if e_verts[0] == e_verts[1] {
                report(TopologyError::DegenerateEdge);
                ok = false;
                continue;
            }
            let e_faces = self.get_edge_faces(e_index);
            if e_faces.size() < 1 || e_faces.size() > 2 {
                report(TopologyError::NonManifoldEdge);
                ok = false;
            }
        }

        ok
    }

    /// Debug print (writes to stderr).
    /// Print full topology diagnostic output.
    /// Mirrors C++ `Level::print()` — outputs all six relations, sharpness, and tags.
    pub fn print(&self, refinement: Option<&super::refinement::Refinement>) {
        eprintln!("Level ({:p}):", self);
        eprintln!("  Depth = {}", self.depth);
        eprintln!("  Primary component counts:");
        eprintln!("    faces = {}", self.face_count);
        eprintln!("    edges = {}", self.edge_count);
        eprintln!("    verts = {}", self.vert_count);

        eprintln!("  Topology relation sizes:");

        // ---- Face relations ----
        eprintln!("    Face relations:");
        eprintln!(
            "      face-vert counts/offset = {}",
            self.face_vert_counts_offsets.len()
        );
        eprintln!("      face-vert indices = {}", self.face_vert_indices.len());
        for i in 0..self.face_count {
            let fv = self.get_face_vertices(i);
            eprint!("        face {:4} verts: ", i);
            for k in 0..fv.size() {
                eprint!(" {}", fv[k]);
            }
            eprintln!();
        }
        eprintln!("      face-edge indices = {}", self.face_edge_indices.len());
        for i in 0..self.face_count {
            let fe = self.get_face_edges(i);
            eprint!("        face {:4} edges: ", i);
            for k in 0..fe.size() {
                eprint!(" {}", fe[k]);
            }
            eprintln!();
        }
        eprintln!("      face tags = {}", self.face_tags.len());
        for i in 0..self.face_count {
            eprintln!(
                "        face {:4}:  hole = {}",
                i,
                self.face_tags[i as usize].hole() as i32
            );
        }
        if let Some(r) = refinement {
            eprintln!("      face child-verts = {}", r.face_child_vert_index.len());
        }

        // ---- Edge relations ----
        eprintln!("    Edge relations:");
        eprintln!("      edge-vert indices = {}", self.edge_vert_indices.len());
        for i in 0..self.edge_count {
            let ev = self.get_edge_vertices(i);
            eprintln!("        edge {:4} verts:  {} {}", i, ev[0], ev[1]);
        }
        eprintln!(
            "      edge-face counts/offset = {}",
            self.edge_face_counts_offsets.len()
        );
        eprintln!(
            "      edge-face indices       = {}",
            self.edge_face_indices.len()
        );
        eprintln!(
            "      edge-face local-indices = {}",
            self.edge_face_local_indices.len()
        );
        for i in 0..self.edge_count {
            let ef = self.get_edge_faces(i);
            let efl = self.get_edge_face_local_indices(i);
            eprint!("        edge {:4} faces: ", i);
            for k in 0..ef.size() {
                eprint!(" {}", ef[k]);
            }
            eprintln!();
            eprint!("             face-edges: ");
            for k in 0..efl.size() {
                eprint!(" {}", efl[k]);
            }
            eprintln!();
        }
        if let Some(r) = refinement {
            eprintln!("      edge child-verts = {}", r.edge_child_vert_index.len());
            for i in 0..self.edge_count {
                eprintln!(
                    "        edge {:4} child vert:  {}",
                    i, r.edge_child_vert_index[i as usize]
                );
            }
        }
        eprintln!("      edge sharpness = {}", self.edge_sharpness.len());
        for i in 0..self.edge_count {
            eprintln!(
                "        edge {:4} sharpness:  {:.6}",
                i, self.edge_sharpness[i as usize]
            );
        }
        eprintln!("      edge tags = {}", self.edge_tags.len());
        for i in 0..self.edge_count {
            let t = self.edge_tags[i as usize];
            eprintln!(
                "        edge {:4}:  boundary = {}  nonManifold = {}  semiSharp = {}  infSharp = {}",
                i,
                t.boundary() as i32,
                t.non_manifold() as i32,
                t.semi_sharp() as i32,
                t.inf_sharp() as i32
            );
        }

        // ---- Vert relations ----
        eprintln!("    Vert relations:");
        eprintln!(
            "      vert-face counts/offset = {}",
            self.vert_face_counts_offsets.len()
        );
        eprintln!(
            "      vert-face indices       = {}",
            self.vert_face_indices.len()
        );
        eprintln!(
            "      vert-face local-indices = {}",
            self.vert_face_local_indices.len()
        );
        for i in 0..self.vert_count {
            let vf = self.get_vertex_faces(i);
            let vfl = self.get_vertex_face_local_indices(i);
            eprint!("        vert {:4} faces: ", i);
            for k in 0..vf.size() {
                eprint!(" {}", vf[k]);
            }
            eprintln!();
            eprint!("             face-verts: ");
            for k in 0..vfl.size() {
                eprint!(" {}", vfl[k]);
            }
            eprintln!();
        }
        eprintln!(
            "      vert-edge counts/offset = {}",
            self.vert_edge_counts_offsets.len()
        );
        eprintln!(
            "      vert-edge indices       = {}",
            self.vert_edge_indices.len()
        );
        eprintln!(
            "      vert-edge local-indices = {}",
            self.vert_edge_local_indices.len()
        );
        for i in 0..self.vert_count {
            let ve = self.get_vertex_edges(i);
            let vel = self.get_vertex_edge_local_indices(i);
            eprint!("        vert {:4} edges: ", i);
            for k in 0..ve.size() {
                eprint!(" {}", ve[k]);
            }
            eprintln!();
            eprint!("             edge-verts: ");
            for k in 0..vel.size() {
                eprint!(" {}", vel[k]);
            }
            eprintln!();
        }
        if let Some(r) = refinement {
            eprintln!("      vert child-verts = {}", r.vert_child_vert_index.len());
        }
        eprintln!("      vert sharpness = {}", self.vert_sharpness.len());
        for i in 0..self.vert_count {
            eprintln!(
                "        vert {:4} sharpness:  {:.6}",
                i, self.vert_sharpness[i as usize]
            );
        }
        eprintln!("      vert tags = {}", self.vert_tags.len());
        for i in 0..self.vert_count {
            let t = self.vert_tags[i as usize];
            eprintln!(
                "        vert {:4}:  rule = {:?}  boundary = {}  corner = {}  xordinary = {}  nonManifold = {}  infSharp = {}  infSharpEdges = {}  infSharpCrease = {}  infIrregular = {}  semiSharp = {}  semiSharpEdges = {}",
                i,
                Rule::from_bits(t.rule() as u8),
                t.boundary() as i32,
                t.corner() as i32,
                t.xordinary() as i32,
                t.non_manifold() as i32,
                t.inf_sharp() as i32,
                t.inf_sharp_edges() as i32,
                t.inf_sharp_crease() as i32,
                t.inf_irregular() as i32,
                t.semi_sharp() as i32,
                t.semi_sharp_edges() as i32
            );
        }
    }

    // ---- patch point gathering ----

    /// `i % 4` via bitmask — only valid for non-negative i (mirrors C++ `fastMod4`).
    #[inline(always)]
    fn fast_mod4(i: usize) -> usize {
        i & 3
    }

    /// Return the face point-index array (vertex or fvar) depending on `fvar_channel`.
    #[inline]
    fn face_points(&self, face: Index, fvar_channel: i32) -> ConstIndexArray<'_> {
        if fvar_channel < 0 {
            self.get_face_vertices(face)
        } else {
            self.get_face_fvar_values(face, fvar_channel)
        }
    }

    /// Gather the 16 control points for a quad B-spline interior patch.
    ///
    /// Layout (C++ `level.cpp` diagram):
    /// ```text
    ///  ---5-----4-----15----14---
    ///     |     |     |     |
    ///  ---6-----0-----3-----13---
    ///     |     |x   x|     |
    ///  ---7-----1-----2-----12---
    ///     |     |     |     |
    ///  ---8-----9-----10----11---
    /// ```
    /// Mirrors C++ `Level::gatherQuadRegularInteriorPatchPoints`.
    pub fn gather_quad_regular_interior_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        rotation: i32,
        fvar_channel: i32,
    ) -> i32 {
        debug_assert!((0..4).contains(&rotation));
        static ROT: [usize; 7] = [0, 1, 2, 3, 0, 1, 2];
        let rot = &ROT[rotation as usize..];

        let face_verts = self.get_face_vertices(f);
        let face_pts = self.face_points(f, fvar_channel);

        // 4 face vertices → points[0..4]
        points[0] = face_pts[rot[0]];
        points[1] = face_pts[rot[1]];
        points[2] = face_pts[rot[2]];
        points[3] = face_pts[rot[3]];

        // For each rotated corner: walk to diagonally-opposite face, collect 3 pts.
        let mut pt = 4usize;
        for i in 0..4usize {
            let v = face_verts[rot[i]];
            let v_faces = self.get_vertex_faces(v);
            let v_in_fcs = self.get_vertex_face_local_indices(v);

            let this_in_v = v_faces.find_index_in_4_tuple(f) as usize;
            let int_in_v = Self::fast_mod4(this_in_v + 2);

            let int_face = v_faces[int_in_v as i32];
            let v_in_int = v_in_fcs[int_in_v as i32] as usize;
            let int_pts = self.face_points(int_face, fvar_channel);

            points[pt] = int_pts[Self::fast_mod4(v_in_int + 1)];
            points[pt + 1] = int_pts[Self::fast_mod4(v_in_int + 2)];
            points[pt + 2] = int_pts[Self::fast_mod4(v_in_int + 3)];
            pt += 3;
        }
        debug_assert_eq!(pt, 16);
        16
    }

    /// Gather the 12 control points for a quad B-spline boundary patch.
    ///
    /// `boundary_edge_in_face` is the local index (0-3) of the boundary edge.
    /// Layout (C++ diagram):
    /// ```text
    ///  ---4-----0-----3-----11---
    ///           |x   x|
    ///  ---5-----1-----2-----10---
    ///           |v0 v1|
    ///  ---6-----7-----8-----9----
    /// ```
    /// Mirrors C++ `Level::gatherQuadRegularBoundaryPatchPoints`.
    pub fn gather_quad_regular_boundary_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        boundary_edge_in_face: i32,
        fvar_channel: i32,
    ) -> i32 {
        let bei = boundary_edge_in_face as usize;
        let int_edge = Self::fast_mod4(bei + 2);

        // v0 and v1: the two interior vertices (opposite the boundary edge)
        let int_v0 = int_edge;
        let int_v1 = Self::fast_mod4(int_edge + 1);

        let face_verts = self.get_face_vertices(f);
        let v0 = face_verts[int_v0];
        let v1 = face_verts[int_v1];

        let v0_faces = self.get_vertex_faces(v0);
        let v1_faces = self.get_vertex_faces(v1);
        let v0_in_fcs = self.get_vertex_face_local_indices(v0);
        let v1_in_fcs = self.get_vertex_face_local_indices(v1);

        let bnd_v0 = v0_faces.find_index_in_4_tuple(f) as usize;
        let bnd_v1 = v1_faces.find_index_in_4_tuple(f) as usize;

        let prev_v0 = Self::fast_mod4(bnd_v0 + 1);
        let int_v0f = Self::fast_mod4(bnd_v0 + 2);
        let int_v1f = Self::fast_mod4(bnd_v1 + 2);
        let next_v1 = Self::fast_mod4(bnd_v1 + 3);

        let prev_face = v0_faces[prev_v0 as i32];
        let int_v0face = v0_faces[int_v0f as i32];
        let int_v1face = v1_faces[int_v1f as i32];
        let next_face = v1_faces[next_v1 as i32];

        let v0_in_prev = v0_in_fcs[prev_v0 as i32] as usize;
        let v0_in_int = v0_in_fcs[int_v0f as i32] as usize;
        let v1_in_int = v1_in_fcs[int_v1f as i32] as usize;
        let v1_in_next = v1_in_fcs[next_v1 as i32] as usize;

        let this_pts = self.face_points(f, fvar_channel);
        let prev_pts = self.face_points(prev_face, fvar_channel);
        let iv0_pts = self.face_points(int_v0face, fvar_channel);
        let iv1_pts = self.face_points(int_v1face, fvar_channel);
        let next_pts = self.face_points(next_face, fvar_channel);

        points[0] = this_pts[Self::fast_mod4(bei + 1)];
        points[1] = this_pts[Self::fast_mod4(bei + 2)];
        points[2] = this_pts[Self::fast_mod4(bei + 3)];
        points[3] = this_pts[bei];

        points[4] = prev_pts[Self::fast_mod4(v0_in_prev + 2)];

        points[5] = iv0_pts[Self::fast_mod4(v0_in_int + 1)];
        points[6] = iv0_pts[Self::fast_mod4(v0_in_int + 2)];
        points[7] = iv0_pts[Self::fast_mod4(v0_in_int + 3)];

        points[8] = iv1_pts[Self::fast_mod4(v1_in_int + 1)];
        points[9] = iv1_pts[Self::fast_mod4(v1_in_int + 2)];
        points[10] = iv1_pts[Self::fast_mod4(v1_in_int + 3)];

        points[11] = next_pts[Self::fast_mod4(v1_in_next + 2)];

        12
    }

    /// Gather the 9 control points for a quad B-spline corner patch.
    ///
    /// `corner_vert_in_face` is the local index (0-3) of the corner vertex.
    /// Layout (C++ diagram):
    /// ```text
    ///  0-----3-----8---
    ///  |x   x|     |
    ///  1-----2-----7---
    ///  |     |     |
    ///  4-----5-----6---
    /// ```
    /// Mirrors C++ `Level::gatherQuadRegularCornerPatchPoints`.
    pub fn gather_quad_regular_corner_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        corner_vert_in_face: i32,
        fvar_channel: i32,
    ) -> i32 {
        let cvi = corner_vert_in_face as usize;
        let int_fv = Self::fast_mod4(cvi + 2);

        let face_verts = self.get_face_vertices(f);
        let int_vert = face_verts[int_fv];

        let iv_faces = self.get_vertex_faces(int_vert);
        let iv_in_fcs = self.get_vertex_face_local_indices(int_vert);

        // Find 'f' in int_vert's incident face list
        let mut corner_in_iv = 0usize;
        for i in 0..iv_faces.size() as usize {
            if iv_faces[i] == f {
                corner_in_iv = i;
                break;
            }
        }

        let prev_iv = Self::fast_mod4(corner_in_iv + 1);
        let int_iv = Self::fast_mod4(corner_in_iv + 2);
        let next_iv = Self::fast_mod4(corner_in_iv + 3);

        let prev_face = iv_faces[prev_iv as i32];
        let int_face = iv_faces[int_iv as i32];
        let next_face = iv_faces[next_iv as i32];

        let iv_in_prev = iv_in_fcs[prev_iv as i32] as usize;
        let iv_in_int = iv_in_fcs[int_iv as i32] as usize;
        let iv_in_next = iv_in_fcs[next_iv as i32] as usize;

        let this_pts = self.face_points(f, fvar_channel);
        let prev_pts = self.face_points(prev_face, fvar_channel);
        let int_pts = self.face_points(int_face, fvar_channel);
        let next_pts = self.face_points(next_face, fvar_channel);

        points[0] = this_pts[cvi];
        points[1] = this_pts[Self::fast_mod4(cvi + 1)];
        points[2] = this_pts[Self::fast_mod4(cvi + 2)];
        points[3] = this_pts[Self::fast_mod4(cvi + 3)];

        points[4] = prev_pts[Self::fast_mod4(iv_in_prev + 2)];

        points[5] = int_pts[Self::fast_mod4(iv_in_int + 1)];
        points[6] = int_pts[Self::fast_mod4(iv_in_int + 2)];
        points[7] = int_pts[Self::fast_mod4(iv_in_int + 3)];

        points[8] = next_pts[Self::fast_mod4(iv_in_next + 2)];

        9
    }

    /// Gather bilinear patch points (4 face vertices, possibly rotated).
    ///
    /// Mirrors C++ `Level::gatherQuadLinearPatchPoints`.
    pub fn gather_quad_linear_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        rotation: i32,
        fvar_channel: i32,
    ) -> i32 {
        debug_assert!((0..4).contains(&rotation));
        static ROT: [usize; 7] = [0, 1, 2, 3, 0, 1, 2];
        let rot = &ROT[rotation as usize..];
        let face_pts = self.face_points(f, fvar_channel);

        points[0] = face_pts[rot[0]];
        points[1] = face_pts[rot[1]];
        points[2] = face_pts[rot[2]];
        points[3] = face_pts[rot[3]];
        4
    }

    /// Returns the face-only vertex/fvar points for this face (used for tri patches).
    ///
    /// Distinct from `face_points` in C++ usage context: called from
    /// `gatherTriRegularInteriorPatchPoints` to get face-center points only.
    /// Kept for future tri-patch support (currently unused pending tri-patch impl).
    #[allow(dead_code)]
    #[inline]
    fn face_verts_only(&self, face: Index, fvar_channel: i32) -> ConstIndexArray<'_> {
        self.face_points(face, fvar_channel)
    }

    /// `otherOfTwo(arr, v)` — given a 2-element slice, return the element that is NOT `v`.
    /// Mirrors the anonymous C++ helper `otherOfTwo`.
    #[inline(always)]
    fn other_of_two(pair: ConstIndexArray<'_>, v: Index) -> Index {
        pair[if v == pair[0] { 1 } else { 0 }]
    }

    /// Gather the 12 control points for a tri-regular interior patch.
    ///
    /// Layout (C++ comment):
    /// ```text
    ///              3           11
    ///              X - - - - - X
    ///            /   \       /   \
    ///          /       \ 0 /       \
    ///     4  X - - - - - X - - - - - X 10
    ///      /   \       / * \       /   \
    ///    /       \   / * * * \   /       \
    ///  5  X------X - - - - - X - - - - - X  9
    ///      \       / 1 \       / 2 \       /
    ///        \   /       \   /       \   /
    ///          X - - - - - X - - - - - X
    ///          6           7           8
    /// ```
    /// Mirrors C++ `Level::gatherTriRegularInteriorPatchPoints`.
    pub fn gather_tri_regular_interior_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        rotation: i32,
    ) -> i32 {
        let f_verts = self.get_face_vertices(f);
        let f_edges = self.get_face_edges(f);

        let (i0, i1, i2) = if rotation == 0 {
            (0usize, 1, 2)
        } else {
            let r = rotation as usize % 3;
            (r, (r + 1) % 3, (r + 2) % 3)
        };

        let v0 = f_verts[i0];
        let v1 = f_verts[i1];
        let v2 = f_verts[i2];

        let v0_edges = self.get_vertex_edges(v0);
        let v1_edges = self.get_vertex_edges(v1);
        let v2_edges = self.get_vertex_edges(v2);

        let e0 = f_edges[i0];
        let e1 = f_edges[i1];
        let e2 = f_edges[i2];

        let e0_in_v0 = v0_edges.find_index(e0) as usize;
        let e1_in_v1 = v1_edges.find_index(e1) as usize;
        let e2_in_v2 = v2_edges.find_index(e2) as usize;

        let n0 = v0_edges.size() as usize; // should be 6 for regular interior
        let n1 = v1_edges.size() as usize;
        let n2 = v2_edges.size() as usize;

        points[0] = v0;
        points[1] = v1;
        points[2] = v2;

        points[11] = Self::other_of_two(self.get_edge_vertices(v0_edges[(e0_in_v0 + 3) % n0]), v0);
        points[3] = Self::other_of_two(self.get_edge_vertices(v0_edges[(e0_in_v0 + 4) % n0]), v0);
        points[4] = Self::other_of_two(self.get_edge_vertices(v0_edges[(e0_in_v0 + 5) % n0]), v0);

        points[5] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 3) % n1]), v1);
        points[6] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 4) % n1]), v1);
        points[7] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 5) % n1]), v1);

        points[8] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e2_in_v2 + 3) % n2]), v2);
        points[9] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e2_in_v2 + 4) % n2]), v2);
        points[10] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e2_in_v2 + 5) % n2]), v2);

        12
    }

    /// Gather the 9 control points for a tri-regular boundary-edge patch.
    ///
    /// `boundary_edge_in_face` is the local index (0-2) of the boundary edge.
    /// Mirrors C++ `Level::gatherTriRegularBoundaryEdgePatchPoints`.
    pub fn gather_tri_regular_boundary_edge_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        boundary_face_edge: i32,
    ) -> i32 {
        let f_verts = self.get_face_vertices(f);
        let be = boundary_face_edge as usize;

        let v0 = f_verts[be];
        let v1 = f_verts[(be + 1) % 3];
        let v2 = f_verts[(be + 2) % 3];

        let v0_edges = self.get_vertex_edges(v0);
        let v1_edges = self.get_vertex_edges(v1);
        let v2_edges = self.get_vertex_edges(v2);

        let n2 = v2_edges.size() as usize;

        // e1InV2Edges: find v1Edges[2] in v2's edges
        let e1_key = v1_edges[2];
        let e1_in_v2 = v2_edges.find_index(e1_key) as usize;

        points[0] = v0;
        points[1] = v1;
        points[2] = v2;

        points[3] = Self::other_of_two(self.get_edge_vertices(v1_edges[0]), v1);

        points[4] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e1_in_v2 + 1) % n2]), v2);
        points[5] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e1_in_v2 + 2) % n2]), v2);
        points[6] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e1_in_v2 + 3) % n2]), v2);
        points[7] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e1_in_v2 + 4) % n2]), v2);

        points[8] = Self::other_of_two(self.get_edge_vertices(v0_edges[3]), v0);

        9
    }

    /// Gather the 10 control points for a tri-regular boundary-vertex patch.
    ///
    /// `boundary_face_vert` is the local index (0-2) of the boundary vertex.
    /// Mirrors C++ `Level::gatherTriRegularBoundaryVertexPatchPoints`.
    pub fn gather_tri_regular_boundary_vertex_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        boundary_face_vert: i32,
    ) -> i32 {
        let f_verts = self.get_face_vertices(f);
        let f_edges = self.get_face_edges(f);
        let bv = boundary_face_vert as usize;

        let v0 = f_verts[bv];
        let v1 = f_verts[(bv + 1) % 3];
        let v2 = f_verts[(bv + 2) % 3];

        let e1 = f_edges[bv]; // edge between v0 and v1
        let e2 = f_edges[(bv + 2) % 3]; // edge between v2 and v0

        let v1_edges = self.get_vertex_edges(v1);
        let v2_edges = self.get_vertex_edges(v2);

        let n1 = v1_edges.size() as usize;
        let n2 = v2_edges.size() as usize;

        let e1_in_v1 = v1_edges.find_index(e1) as usize;
        let e2_in_v2 = v2_edges.find_index(e2) as usize;

        points[0] = v0;
        points[1] = v1;
        points[2] = v2;

        points[3] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 1) % n1]), v1);
        points[4] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 2) % n1]), v1);
        points[5] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 3) % n1]), v1);
        points[6] = Self::other_of_two(self.get_edge_vertices(v1_edges[(e1_in_v1 + 4) % n1]), v1);

        points[7] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e2_in_v2 + 3) % n2]), v2);
        points[8] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e2_in_v2 + 4) % n2]), v2);
        points[9] = Self::other_of_two(self.get_edge_vertices(v2_edges[(e2_in_v2 + 5) % n2]), v2);

        10
    }

    /// Gather the 6 control points for a tri-regular corner-vertex patch.
    ///
    /// `corner_face_vert` is the local index (0-2) of the corner vertex.
    /// Mirrors C++ `Level::gatherTriRegularCornerVertexPatchPoints`.
    pub fn gather_tri_regular_corner_vertex_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        corner_face_vert: i32,
    ) -> i32 {
        let f_verts = self.get_face_vertices(f);
        let cv = corner_face_vert as usize;

        let v0 = f_verts[cv];
        let v1 = f_verts[(cv + 1) % 3];
        let v2 = f_verts[(cv + 2) % 3];

        let v1_edges = self.get_vertex_edges(v1);
        let v2_edges = self.get_vertex_edges(v2);

        points[0] = v0;
        points[1] = v1;
        points[2] = v2;

        points[3] = Self::other_of_two(self.get_edge_vertices(v1_edges[0]), v1);
        points[4] = Self::other_of_two(self.get_edge_vertices(v1_edges[1]), v1);
        points[5] = Self::other_of_two(self.get_edge_vertices(v2_edges[3]), v2);

        6
    }

    /// Gather the 8 control points for a tri-regular corner-edge patch.
    ///
    /// `corner_face_edge` is the local index (0-2) of the shared corner edge.
    /// Mirrors C++ `Level::gatherTriRegularCornerEdgePatchPoints`.
    pub fn gather_tri_regular_corner_edge_patch_points(
        &self,
        f: Index,
        points: &mut [Index],
        corner_face_edge: i32,
    ) -> i32 {
        let f_verts = self.get_face_vertices(f);
        let ce = corner_face_edge as usize;

        let v0 = f_verts[ce];
        let v1 = f_verts[(ce + 1) % 3];
        let v2 = f_verts[(ce + 2) % 3];

        let v0_edges = self.get_vertex_edges(v0);
        let v1_edges = self.get_vertex_edges(v1);

        points[0] = v0;
        points[1] = v1;
        points[2] = v2;

        points[3] = Self::other_of_two(self.get_edge_vertices(v1_edges[3]), v1);
        points[4] = Self::other_of_two(self.get_edge_vertices(v1_edges[0]), v1);
        points[7] = Self::other_of_two(self.get_edge_vertices(v0_edges[3]), v0);

        // points[4] is v4, points[7] is v7; get their edges to find points[5] and [6]
        let v4 = points[4];
        let v7 = points[7];
        let v4_edges = self.get_vertex_edges(v4);
        let v7_edges = self.get_vertex_edges(v7);

        // C++: points[5] = otherOfTwo(v4Edges[v4Edges.size()-3], v1)  (NOT v4!)
        //      points[6] = otherOfTwo(v7Edges[2], v1)
        let n4 = v4_edges.size() as usize;
        points[5] = Self::other_of_two(self.get_edge_vertices(v4_edges[n4 - 3]), v1);
        points[6] = Self::other_of_two(self.get_edge_vertices(v7_edges[2]), v1);

        8
    }

    /// Gather the ring of points around a quad vertex.
    ///
    /// Returns the count of ring points written.  Interior vertex with N faces →
    /// 2*N points; boundary vertex → 2*N+1 points.
    /// Mirrors C++ `Level::gatherQuadRegularRingAroundVertex`.
    pub fn gather_quad_regular_ring_around_vertex(
        &self,
        v: Index,
        ring: &mut [Index],
        fvar_channel: i32,
    ) -> i32 {
        let v_edges = self.get_vertex_edges(v);
        let v_faces = self.get_vertex_faces(v);
        let v_in_fcs = self.get_vertex_face_local_indices(v);

        let is_boundary = v_edges.size() > v_faces.size();
        let nf = v_faces.size() as usize;
        let mut idx = 0usize;

        for i in 0..nf {
            let f_pts = self.face_points(v_faces[i], fvar_channel);
            let vif = v_in_fcs[i] as usize;

            ring[idx] = f_pts[Self::fast_mod4(vif + 1)];
            ring[idx + 1] = f_pts[Self::fast_mod4(vif + 2)];
            idx += 2;

            // Extra trailing point for the last face of a boundary vertex
            if is_boundary && i == nf - 1 {
                ring[idx] = f_pts[Self::fast_mod4(vif + 3)];
                idx += 1;
            }
        }
        idx as i32
    }

    /// Gather a partial ring of points defined by `span` around a quad vertex.
    ///
    /// Returns the count of ring points written.
    /// Mirrors C++ `Level::gatherQuadRegularPartialRingAroundVertex`.
    pub fn gather_quad_regular_partial_ring_around_vertex(
        &self,
        v: Index,
        span: &VSpan,
        ring: &mut [Index],
        fvar_channel: i32,
    ) -> i32 {
        debug_assert!(!self.get_vertex_tag(v).non_manifold());

        let v_faces = self.get_vertex_faces(v);
        let v_in_fcs = self.get_vertex_face_local_indices(v);
        let nf_total = v_faces.size() as usize;

        let n_faces = span.num_faces as usize;
        let start = span.start_face as usize;
        let mut idx = 0usize;

        for i in 0..n_faces {
            let f_local = (start + i) % nf_total;
            let f_pts = self.face_points(v_faces[f_local], fvar_channel);
            let vif = v_in_fcs[f_local] as usize;

            ring[idx] = f_pts[Self::fast_mod4(vif + 1)];
            ring[idx + 1] = f_pts[Self::fast_mod4(vif + 2)];
            idx += 2;

            // Extra trailing point at the end of a non-periodic span
            if i == n_faces - 1 && !span.periodic {
                ring[idx] = f_pts[Self::fast_mod4(vif + 3)];
                idx += 1;
            }
        }
        idx as i32
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::new()
    }
}
