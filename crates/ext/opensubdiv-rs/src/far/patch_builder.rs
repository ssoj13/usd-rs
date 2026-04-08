// Copyright 2018 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/patchBuilder.h + far/patchBuilder.cpp
//
// Core PatchBuilder that assembles regular and irregular patches from topology.
// Uses enum dispatch instead of C++ virtual methods.

use super::patch_descriptor::PatchType;
use super::patch_param::PatchParam;
use super::ptex_indices::PtexIndices;
use super::sparse_matrix::SparseMatrix;
use super::topology_refiner::TopologyRefiner;
use super::types::{INDEX_INVALID, Index, LocalIndex};
use crate::sdc::crease::Rule;
use crate::sdc::types::{SchemeType, SchemeTypeTraits};
use crate::vtr::level::{ETag, Level, VSpan, VTag};

// ---------------------------------------------------------------------------
// Fast modulus helpers
// ---------------------------------------------------------------------------

#[inline]
fn fast_mod4(x: i32) -> i32 {
    x & 0x3
}
#[inline]
fn fast_mod3(x: i32) -> i32 {
    const TABLE: [i32; 6] = [0, 1, 2, 0, 1, 2];
    TABLE[x as usize]
}
#[inline]
fn fast_mod_n(x: i32, n: i32) -> i32 {
    if x < n { x } else { x - n }
}

// ---------------------------------------------------------------------------
// Triangular boundary mask encoding/decoding
// ---------------------------------------------------------------------------

#[inline]
fn unpack_tri_boundary_lower(mask: i32) -> i32 {
    mask & 0x7
}
#[inline]
fn unpack_tri_boundary_upper(mask: i32) -> i32 {
    (mask >> 3) & 0x3
}
#[inline]
fn pack_tri_boundary(upper: i32, lower: i32) -> i32 {
    (upper << 3) | lower
}

fn encode_tri_boundary_mask(e_bits: i32, v_bits: i32) -> i32 {
    let mut upper = 0;
    let mut lower = e_bits;
    if v_bits != 0 {
        if e_bits == 0 {
            upper = 1;
            lower = v_bits;
        } else if v_bits == 7 && (e_bits == 1 || e_bits == 2 || e_bits == 4) {
            upper = 2;
            lower = e_bits;
        }
    }
    pack_tri_boundary(upper, lower)
}

fn decode_tri_boundary_mask(mask: i32) -> (i32, i32) {
    const E_TO_V: [i32; 8] = [0, 3, 6, 7, 5, 7, 7, 7];
    let lower = unpack_tri_boundary_lower(mask);
    let upper = unpack_tri_boundary_upper(mask);
    match upper {
        0 => (lower, E_TO_V[lower as usize]),
        1 => (0, lower),
        2 => (lower, 0x7),
        _ => (0, 0),
    }
}

// ---------------------------------------------------------------------------
// Edge singularity helpers
// ---------------------------------------------------------------------------

fn get_singular_edge_mask(include_inf_sharp: bool) -> ETag {
    let mut m = ETag::default();
    m.set_boundary(true);
    m.set_non_manifold(true);
    m.set_inf_sharp(include_inf_sharp);
    m
}

fn is_edge_singular(level: &Level, e: Index, mask: ETag) -> bool {
    let etag = level.get_edge_tag(e);
    (etag.0 & mask.0) > 0
}

// ---------------------------------------------------------------------------
// Corner span identification
// ---------------------------------------------------------------------------

fn identify_manifold_corner_span(
    level: &Level,
    f: Index,
    f_corner: i32,
    mask: ETag,
    span: &mut VSpan,
    _fvc: i32,
) {
    let f_verts = level.get_face_vertices(f);
    let f_edges = level.get_face_edges(f);

    let v_edges = level.get_vertex_edges(f_verts[f_corner]);
    let n_edges = v_edges.size();

    let i_leading_start = v_edges.find_index(f_edges[f_corner]);
    let i_trailing_start = fast_mod_n(i_leading_start + 1, n_edges);

    span.clear();
    span.num_faces = 1;
    span.corner_in_span = 0;

    let mut i_leading = i_leading_start;
    while !is_edge_singular(level, v_edges[i_leading], mask) {
        span.num_faces += 1;
        span.corner_in_span += 1;
        i_leading = fast_mod_n(i_leading + n_edges - 1, n_edges);
        if i_leading == i_trailing_start {
            break;
        }
    }

    let mut i_trailing = i_trailing_start;
    if i_trailing != i_leading {
        while !is_edge_singular(level, v_edges[i_trailing], mask) {
            span.num_faces += 1;
            i_trailing = fast_mod_n(i_trailing + 1, n_edges);
            if i_trailing == i_leading_start {
                break;
            }
        }
    }
    span.start_face = i_leading as LocalIndex;
}

fn count_manifold_corner_span(level: &Level, f: Index, f_corner: i32, mask: ETag, fvc: i32) -> i32 {
    let mut span = VSpan::default();
    identify_manifold_corner_span(level, f, f_corner, mask, &mut span, fvc);
    span.num_faces as i32
}

fn identify_non_manifold_corner_span(
    level: &Level,
    f_index: Index,
    f_corner: i32,
    mask: ETag,
    span: &mut VSpan,
    _fvc: i32,
) {
    let f_edges = level.get_face_edges(f_index);
    let nfe = f_edges.size();

    let e_leading_start = f_edges[f_corner];
    let e_trailing_start = f_edges[((f_corner + nfe - 1) % nfe) as i32];

    span.clear();
    span.num_faces = 1;
    span.corner_in_span = 0;

    let mut start_face = f_index;
    let mut start_corner = f_corner;

    // Traverse clockwise to find leading edge
    let mut f_leading = f_index;
    let mut e_leading = e_leading_start;
    while !is_edge_singular(level, e_leading, mask) {
        span.num_faces += 1;
        span.corner_in_span += 1;

        let e_faces = level.get_edge_faces(e_leading);
        debug_assert!(e_faces.size() == 2);
        f_leading = if e_faces[0] == f_leading {
            e_faces[1]
        } else {
            e_faces[0]
        };
        let next_f_edges = level.get_face_edges(f_leading);

        start_face = f_leading;
        start_corner = (next_f_edges.find_index(e_leading) + 1) % next_f_edges.size();

        e_leading = next_f_edges[start_corner as i32];
        if e_leading == e_trailing_start {
            span.periodic = !is_edge_singular(level, e_leading, mask);
            break;
        }
    }

    // Traverse counter-clockwise to find trailing edge
    let mut f_trailing = f_index;
    let mut e_trailing = e_trailing_start;
    if e_trailing != e_leading {
        while !is_edge_singular(level, e_trailing, mask) {
            span.num_faces += 1;

            let e_faces = level.get_edge_faces(e_trailing);
            debug_assert!(e_faces.size() == 2);
            f_trailing = if e_faces[0] == f_trailing {
                e_faces[1]
            } else {
                e_faces[0]
            };
            let next_f_edges = level.get_face_edges(f_trailing);

            e_trailing = next_f_edges[((next_f_edges.find_index(e_trailing) + next_f_edges.size()
                - 1)
                % next_f_edges.size()) as i32];
            if e_trailing == e_leading_start {
                span.periodic = !is_edge_singular(level, e_trailing, mask);
                break;
            }
        }
    }

    // Identify start_face in vertex's incident faces
    let v_index = level.get_face_vertices(f_index)[f_corner];
    let v_faces = level.get_vertex_faces(v_index);
    let v_in_faces = level.get_vertex_face_local_indices(v_index);

    span.start_face = v_faces.size() as LocalIndex;
    for i in 0..v_faces.size() {
        if v_faces[i] == start_face && v_in_faces[i] as i32 == start_corner {
            span.start_face = i as LocalIndex;
            break;
        }
    }
}

fn count_non_manifold_corner_span(
    level: &Level,
    f: Index,
    f_corner: i32,
    mask: ETag,
    fvc: i32,
) -> i32 {
    let mut span = VSpan::default();
    identify_non_manifold_corner_span(level, f, f_corner, mask, &mut span, fvc);
    span.num_faces as i32
}

// ---------------------------------------------------------------------------
// Ring-gathering helpers
// ---------------------------------------------------------------------------

fn get_face_points(level: &Level, f: Index, fvc: i32) -> Vec<Index> {
    if fvc < 0 {
        let a = level.get_face_vertices(f);
        (0..a.size()).map(|i| a[i]).collect()
    } else {
        let a = level.get_face_fvar_values(f, fvc);
        (0..a.size()).map(|i| a[i]).collect()
    }
}

fn gather_tri_regular_ring(level: &Level, v: Index, ring: &mut [i32], fvc: i32) -> i32 {
    let v_edges = level.get_vertex_edges(v);
    let v_faces = level.get_vertex_faces(v);
    let v_in_faces = level.get_vertex_face_local_indices(v);
    let is_boundary = v_edges.size() > v_faces.size();

    let mut idx = 0usize;
    for i in 0..v_faces.size() {
        let fp = get_face_points(level, v_faces[i], fvc);
        let vin = v_in_faces[i] as i32;
        ring[idx] = fp[fast_mod3(vin + 1) as usize];
        idx += 1;
        if is_boundary && i == v_faces.size() - 1 {
            ring[idx] = fp[fast_mod3(vin + 2) as usize];
            idx += 1;
        }
    }
    idx as i32
}

fn gather_regular_partial_ring(
    level: &Level,
    v: Index,
    span: &VSpan,
    ring: &mut [i32],
    fvc: i32,
) -> i32 {
    let is_manifold = !level.is_vertex_non_manifold(v);
    let v_faces = level.get_vertex_faces(v);
    let v_in_faces = level.get_vertex_face_local_indices(v);
    let n_faces = span.num_faces as i32;
    let start = span.start_face as i32;

    let mut next_face = v_faces[start];
    let mut v_in_next = v_in_faces[start] as i32;
    let mut idx = 0usize;

    for i in 0..n_faces {
        let this_face = next_face;
        let v_in_this = v_in_next;
        let fp = get_face_points(level, this_face, fvc);
        let is_quad = fp.len() == 4;

        if is_quad {
            ring[idx] = fp[fast_mod4(v_in_this + 1) as usize];
            idx += 1;
            ring[idx] = fp[fast_mod4(v_in_this + 2) as usize];
            idx += 1;
        } else {
            ring[idx] = fp[fast_mod3(v_in_this + 1) as usize];
            idx += 1;
        }

        if i == n_faces - 1 {
            if !span.periodic {
                if is_quad {
                    ring[idx] = fp[fast_mod4(v_in_this + 3) as usize];
                    idx += 1;
                } else {
                    ring[idx] = fp[fast_mod3(v_in_this + 2) as usize];
                    idx += 1;
                }
            }
        } else if is_manifold {
            let i_next = fast_mod_n(start + i + 1, v_faces.size());
            next_face = v_faces[i_next];
            v_in_next = v_in_faces[i_next] as i32;
        } else {
            let n_fp = fp.len() as i32;
            let next_in_this = fast_mod_n(v_in_this + n_fp - 1, n_fp);
            let next_edge = level.get_face_edges(this_face)[next_in_this];
            let e_faces = level.get_edge_faces(next_edge);
            next_face = if e_faces[0] == this_face {
                e_faces[1]
            } else {
                e_faces[0]
            };
            v_in_next = level.get_face_edges(next_face).find_index(next_edge);
        }
    }
    idx as i32
}

fn get_next_face_in_vert_faces(
    level: &Level,
    this_idx: i32,
    v_faces: &[Index],
    v_in_faces: &[LocalIndex],
    manifold: bool,
    v_in_next: &mut i32,
) -> Index {
    if manifold {
        let next_idx = fast_mod_n(this_idx + 1, v_faces.len() as i32);
        *v_in_next = v_in_faces[next_idx as usize] as i32;
        v_faces[next_idx as usize]
    } else {
        let this_face = v_faces[this_idx as usize];
        let vin = v_in_faces[this_idx as usize] as i32;
        let f_edges = level.get_face_edges(this_face);
        let n = f_edges.size();
        let next_edge = f_edges[fast_mod_n(vin + n - 1, n)];
        let e_faces = level.get_edge_faces(next_edge);
        debug_assert!(e_faces.size() == 2);
        let nf = if e_faces[0] == this_face {
            e_faces[1]
        } else {
            e_faces[0]
        };
        *v_in_next = level.get_face_edges(nf).find_index(next_edge);
        nf
    }
}

fn get_prev_face_in_vert_faces(
    level: &Level,
    this_idx: i32,
    v_faces: &[Index],
    v_in_faces: &[LocalIndex],
    manifold: bool,
    v_in_prev: &mut i32,
) -> Index {
    if manifold {
        let prev_idx = if this_idx > 0 {
            this_idx - 1
        } else {
            v_faces.len() as i32 - 1
        };
        *v_in_prev = v_in_faces[prev_idx as usize] as i32;
        v_faces[prev_idx as usize]
    } else {
        let this_face = v_faces[this_idx as usize];
        let vin = v_in_faces[this_idx as usize] as i32;
        let f_edges = level.get_face_edges(this_face);
        let prev_edge = f_edges[vin];
        let e_faces = level.get_edge_faces(prev_edge);
        debug_assert!(e_faces.size() == 2);
        let pf = if e_faces[0] == this_face {
            e_faces[1]
        } else {
            e_faces[0]
        };
        let edge_in_prev = level.get_face_edges(pf).find_index(prev_edge);
        *v_in_prev = fast_mod_n(edge_in_prev + 1, f_edges.size());
        pf
    }
}

// ---------------------------------------------------------------------------
// SourcePatch
// ---------------------------------------------------------------------------

/// Topology of a single corner in the irregular patch neighborhood.
#[derive(Clone, Copy, Default)]
pub struct Corner {
    pub num_faces: LocalIndex,
    pub patch_face: LocalIndex,
    pub boundary: bool,
    pub sharp: bool,
    pub dart: bool,
    pub shares_with_prev: bool,
    pub shares_with_next: bool,
    pub val2_interior: bool,
    pub val2_adjacent: bool,
}

/// Full topological specification of an irregular patch neighborhood.
#[derive(Clone, Default)]
pub struct SourcePatch {
    pub corners: [Corner; 4],
    pub num_corners: i32,
    pub num_source_points: i32,
    pub max_valence: i32,
    pub max_ring_size: i32,
    pub ring_sizes: [i32; 4],
    pub local_ring_sizes: [i32; 4],
    pub local_ring_offsets: [i32; 4],
}

impl SourcePatch {
    /// Create a new default SourcePatch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of source points after finalization.
    pub fn get_num_source_points(&self) -> i32 {
        self.num_source_points
    }

    /// Ring size for the given corner index.
    pub fn get_corner_ring_size(&self, corner: usize) -> i32 {
        self.ring_sizes[corner]
    }

    /// Finalize after all corners have been set. Computes ring sizes and
    /// total source point count.
    pub fn finalize(&mut self, face_size: i32) {
        let is_quad = face_size == 4;
        let is_quad_i = is_quad as i32;
        self.num_corners = face_size;
        self.max_valence = 0;
        self.max_ring_size = 0;
        self.num_source_points = self.num_corners;

        for c_idx in 0..self.num_corners as usize {
            let c_prev = fast_mod_n(c_idx as i32 + 2 + is_quad_i, self.num_corners) as usize;
            let c_next = fast_mod_n(c_idx as i32 + 1, self.num_corners) as usize;

            let prev_val2 = self.corners[c_prev].num_faces == 2 && !self.corners[c_prev].boundary;
            let this_val2 = self.corners[c_idx].num_faces == 2 && !self.corners[c_idx].boundary;
            let next_val2 = self.corners[c_next].num_faces == 2 && !self.corners[c_next].boundary;

            self.corners[c_idx].val2_interior = this_val2;
            self.corners[c_idx].val2_adjacent = prev_val2 || next_val2;

            let corner = self.corners[c_idx];
            let nf = corner.num_faces as i32;

            if (nf + corner.boundary as i32) > 2 {
                if corner.boundary {
                    self.corners[c_idx].shares_with_prev =
                        is_quad && corner.patch_face != corner.num_faces - 1;
                    self.corners[c_idx].shares_with_next = corner.patch_face != 0;
                } else if corner.dart {
                    let cp = &self.corners[c_prev];
                    let cn = &self.corners[c_next];
                    let prev_on_dart = cp.boundary && cp.patch_face == 0;
                    let next_on_dart = cn.boundary && cn.patch_face == cn.num_faces - 1;
                    self.corners[c_idx].shares_with_prev = is_quad && !prev_on_dart;
                    self.corners[c_idx].shares_with_next = !next_on_dart;
                } else {
                    self.corners[c_idx].shares_with_prev = is_quad;
                    self.corners[c_idx].shares_with_next = true;
                }

                self.ring_sizes[c_idx] = nf * (1 + is_quad_i) + corner.boundary as i32;
                self.local_ring_sizes[c_idx] = self.ring_sizes[c_idx]
                    - (self.num_corners - 1)
                    - self.corners[c_idx].shares_with_prev as i32
                    - self.corners[c_idx].shares_with_next as i32;

                if self.corners[c_idx].val2_adjacent && !corner.boundary {
                    self.local_ring_sizes[c_idx] -= prev_val2 as i32;
                    self.local_ring_sizes[c_idx] -= (next_val2 && is_quad) as i32;
                }
            } else {
                self.corners[c_idx].shares_with_prev = false;
                self.corners[c_idx].shares_with_next = false;

                if nf == 1 {
                    self.ring_sizes[c_idx] = self.num_corners - 1;
                    self.local_ring_sizes[c_idx] = 0;
                } else {
                    self.ring_sizes[c_idx] = 2 * (1 + is_quad_i);
                    self.local_ring_sizes[c_idx] = is_quad_i;
                }
            }
            self.local_ring_offsets[c_idx] = self.num_source_points;
            self.max_valence = self.max_valence.max(nf + corner.boundary as i32);
            self.max_ring_size = self.max_ring_size.max(self.ring_sizes[c_idx]);
            self.num_source_points += self.local_ring_sizes[c_idx];
        }
    }

    /// Compute ring point indices for `corner` into `ring_points`.
    /// Returns the ring size.
    pub fn get_corner_ring_points(&self, corner: i32, ring_points: &mut [i32]) -> i32 {
        let is_quad = self.num_corners == 4;
        let is_quad_i = is_quad as i32;
        let c = corner as usize;

        let c_next = fast_mod_n(corner + 1, self.num_corners) as usize;
        let c_opp = fast_mod_n(corner + 1 + is_quad_i, self.num_corners) as usize;
        let c_prev = fast_mod_n(corner + 2 + is_quad_i, self.num_corners) as usize;

        let mut rs = 0usize;

        // Adjacent corner points
        ring_points[rs] = c_next as i32;
        rs += 1;
        if is_quad {
            ring_points[rs] = c_opp as i32;
            rs += 1;
        }
        ring_points[rs] = c_prev as i32;
        rs += 1;

        // Shared points preceding local ring
        if self.corners[c_prev].val2_interior && !self.corners[c].boundary {
            ring_points[rs] = if is_quad { c_opp as i32 } else { c_next as i32 };
            rs += 1;
        }
        if self.corners[c].shares_with_prev {
            ring_points[rs] = self.local_ring_offsets[c_prev] + self.local_ring_sizes[c_prev] - 1;
            rs += 1;
        }

        // Local ring points
        for i in 0..self.local_ring_sizes[c] {
            ring_points[rs] = self.local_ring_offsets[c] + i;
            rs += 1;
        }

        // Shared points following local ring
        if is_quad {
            if self.corners[c].shares_with_next {
                ring_points[rs] = self.local_ring_offsets[c_next];
                rs += 1;
            }
            if self.corners[c_next].val2_interior && !self.corners[c].boundary {
                ring_points[rs] = c_opp as i32;
                rs += 1;
            }
        } else if self.corners[c].shares_with_next {
            if self.corners[c_next].val2_interior && !self.corners[c].boundary {
                ring_points[rs] = c_prev as i32;
                rs += 1;
            } else if self.local_ring_sizes[c_next] == 0 {
                ring_points[rs] = self.local_ring_offsets[c_prev];
                rs += 1;
            } else {
                ring_points[rs] = self.local_ring_offsets[c_next];
                rs += 1;
            }
        }

        debug_assert_eq!(rs as i32, self.ring_sizes[c]);

        // Rotate if patch face is not first
        if self.corners[c].patch_face > 0 {
            let rot = rs as i32 - (1 + is_quad_i) * self.corners[c].patch_face as i32;
            let rot = rot as usize;
            ring_points[..rs].rotate_left(rot);
        }
        rs as i32
    }
}

// ---------------------------------------------------------------------------
// BasisType + PatchBuilderOptions
// ---------------------------------------------------------------------------

/// Basis type for patch construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BasisType {
    Unspecified = 0,
    Regular = 1,
    Gregory = 2,
    Linear = 3,
    Bezier = 4,
}

/// Options for PatchBuilder construction.
#[derive(Debug, Clone, Copy)]
pub struct PatchBuilderOptions {
    pub reg_basis: BasisType,
    pub irreg_basis: BasisType,
    pub fill_missing_boundary_points: bool,
    pub approx_inf_sharp_with_smooth: bool,
    pub approx_smooth_corner_with_sharp: bool,
}

impl Default for PatchBuilderOptions {
    fn default() -> Self {
        Self {
            reg_basis: BasisType::Unspecified,
            irreg_basis: BasisType::Unspecified,
            fill_missing_boundary_points: false,
            approx_inf_sharp_with_smooth: false,
            approx_smooth_corner_with_sharp: false,
        }
    }
}

/// Single-crease patch information.
#[derive(Debug, Clone, Copy, Default)]
pub struct SingleCreaseInfo {
    pub crease_edge_in_face: i32,
    pub crease_sharpness: f32,
}

// ---------------------------------------------------------------------------
// PatchBuilder
// ---------------------------------------------------------------------------

/// Assists identification and assembly of limit surface patches from a
/// refined topology. Uses enum dispatch for scheme-specific behavior.
pub struct PatchBuilder<'a> {
    refiner: &'a TopologyRefiner,
    options: PatchBuilderOptions,

    scheme_type: SchemeType,
    scheme_reg_face_size: i32,
    scheme_is_linear: bool,

    pub(crate) reg_patch_type: PatchType,
    pub(crate) irreg_patch_type: PatchType,
    pub(crate) native_patch_type: PatchType,
    pub(crate) linear_patch_type: PatchType,
}

impl<'a> PatchBuilder<'a> {
    /// Create a PatchBuilder for the given refiner and options.
    /// Dispatches to the correct scheme-specific configuration.
    pub fn create(refiner: &'a TopologyRefiner, options: PatchBuilderOptions) -> Self {
        let scheme = refiner.get_scheme_type();
        let reg_face = SchemeTypeTraits::get_regular_face_size(scheme);
        let is_linear = SchemeTypeTraits::get_local_neighborhood_size(scheme) == 0;

        let (reg_pt, irreg_pt, native_pt, linear_pt) = match scheme {
            SchemeType::Bilinear => {
                let r = bilinear_patch_type(options.reg_basis);
                let ir = if options.irreg_basis == BasisType::Unspecified {
                    r
                } else {
                    bilinear_patch_type(options.irreg_basis)
                };
                (r, ir, PatchType::Quads, PatchType::Quads)
            }
            SchemeType::Catmark => {
                let r = catmark_patch_type(options.reg_basis);
                let ir = if options.irreg_basis == BasisType::Unspecified {
                    r
                } else {
                    catmark_patch_type(options.irreg_basis)
                };
                (r, ir, PatchType::Regular, PatchType::Quads)
            }
            SchemeType::Loop => {
                let r = loop_patch_type(options.reg_basis);
                let ir = if options.irreg_basis == BasisType::Unspecified {
                    r
                } else {
                    loop_patch_type(options.irreg_basis)
                };
                (r, ir, PatchType::Loop, PatchType::Triangles)
            }
        };

        Self {
            refiner,
            options,
            scheme_type: scheme,
            scheme_reg_face_size: reg_face,
            scheme_is_linear: is_linear,
            reg_patch_type: reg_pt,
            irreg_patch_type: irreg_pt,
            native_patch_type: native_pt,
            linear_patch_type: linear_pt,
        }
    }

    // -- Accessors --

    pub fn get_regular_face_size(&self) -> i32 {
        self.scheme_reg_face_size
    }
    pub fn get_reg_basis(&self) -> BasisType {
        self.options.reg_basis
    }
    pub fn get_irreg_basis(&self) -> BasisType {
        self.options.irreg_basis
    }
    pub fn get_regular_patch_type(&self) -> PatchType {
        self.reg_patch_type
    }
    pub fn get_irregular_patch_type(&self) -> PatchType {
        self.irreg_patch_type
    }
    pub fn get_irreg_patch_type(&self) -> PatchType {
        self.irreg_patch_type
    }
    pub fn get_native_patch_type(&self) -> PatchType {
        self.native_patch_type
    }
    pub fn get_linear_patch_type(&self) -> PatchType {
        self.linear_patch_type
    }

    // -- Face-level queries --

    /// True if face is a valid patch (not a hole, not incomplete).
    pub fn is_face_a_patch(&self, level_idx: i32, face: Index) -> bool {
        let level = self.refiner.get_level_internal(level_idx);

        if self.refiner.has_holes() && level.is_face_hole(face) {
            return false;
        }

        if level_idx == 0 {
            if self.scheme_is_linear {
                return level.get_face_vertices(face).size() == self.scheme_reg_face_size;
            } else {
                return !level.get_face_composite_vtag(face).incid_irreg_face();
            }
        }

        if self.scheme_reg_face_size == 4 {
            !level.get_face_composite_vtag(face).incomplete()
        } else {
            let ref_ = self.refiner.get_refinement_internal(level_idx - 1);
            !ref_.get_child_face_tag(face).incomplete
        }
    }

    /// True if face is a leaf (not selected for further refinement).
    pub fn is_face_a_leaf(&self, level_idx: i32, face: Index) -> bool {
        if level_idx < self.refiner.get_max_level() {
            let ref_ = self.refiner.get_refinement_internal(level_idx);
            if ref_.get_parent_face_sparse_tag(face).selected {
                return false;
            }
        }
        true
    }

    /// True if the patch is regular (no irregular features).
    pub fn is_patch_regular(&self, level_idx: i32, face: Index, fvc: i32) -> bool {
        if self.scheme_is_linear {
            return true;
        }

        let level = self.refiner.get_level_internal(level_idx);

        let mut v_tags = [VTag::default(); 4];
        level.get_face_vtags(face, &mut v_tags, fvc);
        let comp = VTag::bitwise_or(&v_tags[..self.scheme_reg_face_size as usize]);

        if !comp.inf_sharp() && !comp.inf_sharp_edges() {
            return !comp.xordinary();
        }

        let test_inf = !self.options.approx_inf_sharp_with_smooth;

        let mut irreg_tag = VTag::default();
        irreg_tag.set_non_manifold(true);
        irreg_tag.set_xordinary(true);
        irreg_tag.set_inf_irregular(test_inf);
        let irreg_mask = irreg_tag.get_bits();

        if (comp.get_bits() & irreg_mask) == 0 {
            return true;
        }

        let may_have_irreg = self.refiner.has_irreg_faces_flag();
        let needs_extra = (comp.xordinary() && may_have_irreg) as i32;
        let isolated = level_idx > needs_extra;

        if isolated {
            let needs_inspect = comp.non_manifold()
                || (self.options.approx_smooth_corner_with_sharp
                    && comp.xordinary()
                    && comp.boundary())
                || (test_inf && comp.inf_irregular() && comp.inf_sharp_edges());

            if !needs_inspect {
                return if test_inf {
                    !comp.inf_irregular()
                } else {
                    !comp.xordinary()
                };
            }
        }

        let n_reg_bnd = if self.scheme_reg_face_size == 4 { 2 } else { 3 };

        for i in 0..self.scheme_reg_face_size {
            let vt = v_tags[i as usize];
            if (vt.get_bits() & irreg_mask) == 0 {
                continue;
            }

            if vt.non_manifold() {
                let n = count_non_manifold_corner_span(
                    level,
                    face,
                    i,
                    get_singular_edge_mask(test_inf),
                    fvc,
                );
                if vt.inf_sharp() {
                    if n != 1 {
                        return false;
                    }
                } else if n != n_reg_bnd {
                    return false;
                }
                continue;
            }

            if vt.xordinary() {
                if !vt.inf_sharp_edges() {
                    return false;
                }
                if self.options.approx_smooth_corner_with_sharp && vt.boundary() && !vt.inf_sharp()
                {
                    let mut e_tags = [ETag::default(); 4];
                    level.get_face_etags(face, &mut e_tags, fvc);
                    let i_prev = if i > 0 {
                        i - 1
                    } else {
                        self.scheme_reg_face_size - 1
                    };
                    if e_tags[i as usize].boundary() && e_tags[i_prev as usize].boundary() {
                        continue;
                    }
                }
                if !test_inf {
                    return false;
                }
            }

            if vt.inf_irregular() {
                if !vt.inf_sharp_edges() {
                    return false;
                }
                if vt.inf_sharp_crease() && vt.boundary() {
                    return false;
                }
                let n =
                    count_manifold_corner_span(level, face, i, get_singular_edge_mask(true), fvc);
                if vt.inf_sharp_crease() {
                    if n != n_reg_bnd {
                        return false;
                    }
                } else if n != 1 {
                    return false;
                }
            }
        }
        true
    }

    /// Compute boundary mask for a regular patch.
    pub fn get_regular_patch_boundary_mask(&self, level_idx: i32, face: Index, fvc: i32) -> i32 {
        if self.scheme_is_linear {
            return 0;
        }

        let level = self.refiner.get_level_internal(level_idx);
        let mut v_tags = [VTag::default(); 4];
        level.get_face_vtags(face, &mut v_tags, fvc);
        let f_tag = VTag::bitwise_or(&v_tags[..self.scheme_reg_face_size as usize]);

        if !f_tag.inf_sharp_edges() {
            return 0;
        }

        let mut e_tags = [ETag::default(); 4];
        level.get_face_etags(face, &mut e_tags, fvc);

        let test_inf = !self.options.approx_inf_sharp_with_smooth;

        let mut e_feature = ETag::default();
        e_feature.set_boundary(true);
        e_feature.set_inf_sharp(test_inf);
        e_feature.set_non_manifold(true);
        let ef_mask = e_feature.0;

        let mut e_bits = 0i32;
        for i in 0..self.scheme_reg_face_size.min(4) {
            if (e_tags[i as usize].0 & ef_mask) != 0 {
                e_bits |= 1 << i;
            }
        }

        if self.scheme_reg_face_size == 4 {
            return e_bits;
        }

        // Triangles: also check vertex boundary features
        let mut v_feature = VTag::default();
        v_feature.set_boundary(true);
        v_feature.set_inf_sharp_edges(test_inf);
        v_feature.set_non_manifold(true);
        let vf_mask = v_feature.get_bits();

        let mut v_bits = 0i32;
        for i in 0..3 {
            if (v_tags[i as usize].get_bits() & vf_mask) != 0 {
                v_bits |= 1 << i;
            }
        }

        if e_bits != 0 || v_bits != 0 {
            encode_tri_boundary_mask(e_bits, v_bits)
        } else {
            0
        }
    }

    /// Identify corner spans for an irregular patch.
    pub fn get_irregular_patch_corner_spans(
        &self,
        level_idx: i32,
        face: Index,
        corner_spans: &mut [VSpan; 4],
        fvc: i32,
    ) {
        let level = self.refiner.get_level_internal(level_idx);

        let mut v_tags = [VTag::default(); 4];
        level.get_face_vtags(face, &mut v_tags, fvc);

        let test_inf = !self.options.approx_inf_sharp_with_smooth;
        let sing_mask = get_singular_edge_mask(test_inf);

        for i in 0..self.scheme_reg_face_size as usize {
            let vt = v_tags[i];
            let is_nm = vt.non_manifold();

            let test_edges =
                test_inf && vt.inf_sharp_edges() && (vt.rule() != Rule::Dart.bits() as u16);

            if test_edges || is_nm {
                if is_nm {
                    identify_non_manifold_corner_span(
                        level,
                        face,
                        i as i32,
                        sing_mask,
                        &mut corner_spans[i],
                        fvc,
                    );
                } else {
                    identify_manifold_corner_span(
                        level,
                        face,
                        i as i32,
                        sing_mask,
                        &mut corner_spans[i],
                        fvc,
                    );
                }
            } else {
                corner_spans[i].clear();
            }

            // Sharpen if corner or inf-sharp
            if vt.corner() {
                corner_spans[i].sharp = true;
            } else if is_nm {
                corner_spans[i].sharp = vt.inf_sharp();
            } else if test_inf {
                corner_spans[i].sharp = if test_edges {
                    !vt.inf_sharp_crease()
                } else {
                    vt.inf_sharp()
                };
            }

            // Legacy: reinterpret smooth corner as sharp
            if !corner_spans[i].sharp
                && self.options.approx_smooth_corner_with_sharp
                && vt.xordinary()
                && vt.boundary()
                && !vt.inf_sharp()
                && !vt.non_manifold()
            {
                let n = if corner_spans[i].is_assigned() {
                    corner_spans[i].num_faces as i32
                } else {
                    let fv = level.get_face_vertices(face);
                    level.get_vertex_faces(fv[i as i32]).size()
                };
                corner_spans[i].sharp = n == 1;
            }
        }
    }

    /// Check if face-varying topology matches at face.
    pub fn does_fvar_patch_match(&self, level_idx: i32, face: Index, fvc: i32) -> bool {
        self.refiner
            .get_level_internal(level_idx)
            .does_face_fvar_topology_match(face, fvc)
    }

    // -- Regular patch point retrieval --

    /// Get control point indices for a regular patch.
    pub fn get_regular_patch_points(
        &self,
        level_idx: i32,
        face: Index,
        reg_boundary_mask: i32,
        points: &mut [Index],
        fvc: i32,
    ) -> i32 {
        if self.scheme_is_linear {
            return self.get_regular_face_points(level_idx, face, points, fvc);
        }
        if self.scheme_reg_face_size == 4 {
            self.get_quad_regular_patch_points(level_idx, face, reg_boundary_mask, points, fvc)
        } else {
            self.get_tri_regular_patch_points(level_idx, face, reg_boundary_mask, points, fvc)
        }
    }

    fn get_regular_face_points(
        &self,
        level_idx: i32,
        face: Index,
        points: &mut [Index],
        fvc: i32,
    ) -> i32 {
        let fp = get_face_points(self.refiner.get_level_internal(level_idx), face, fvc);
        for (i, &v) in fp.iter().enumerate() {
            points[i] = v;
        }
        fp.len() as i32
    }

    fn get_quad_regular_patch_points(
        &self,
        level_idx: i32,
        face: Index,
        mut bnd_mask: i32,
        points: &mut [Index],
        fvc: i32,
    ) -> i32 {
        if bnd_mask < 0 {
            bnd_mask = self.get_regular_patch_boundary_mask(level_idx, face, fvc);
        }
        let interior = bnd_mask == 0;

        const PP: [[usize; 4]; 4] = [[5, 4, 0, 1], [6, 2, 3, 7], [10, 11, 15, 14], [9, 13, 12, 8]];

        let level = self.refiner.get_level_internal(level_idx);
        let f_verts = level.get_face_vertices(face);
        let f_points = get_face_points(level, face, fvc);

        let bnd_pt = if !interior && self.options.fill_missing_boundary_points {
            f_points[0]
        } else {
            INDEX_INVALID
        };

        for i in 0..4i32 {
            let v = f_verts[i];
            let ci = &PP[i as usize];

            let v_faces_arr: Vec<Index> = {
                let a = level.get_vertex_faces(v);
                (0..a.size()).map(|j| a[j]).collect()
            };
            let v_in_arr: Vec<LocalIndex> = {
                let a = level.get_vertex_face_local_indices(v);
                (0..a.size()).map(|j| a[j]).collect()
            };
            let f_in_v = v_faces_arr.iter().position(|&x| x == face).unwrap_or(0) as i32;

            let interior_corner =
                interior || ((bnd_mask & (1 << i)) | (bnd_mask & (1 << fast_mod4(i + 3)))) == 0;

            if interior_corner {
                let opp_idx = fast_mod4(f_in_v + 2);
                let f_opp = v_faces_arr[opp_idx as usize];
                let v_in_opp = v_in_arr[opp_idx as usize] as i32;
                let opp_pts = get_face_points(level, f_opp, fvc);
                points[ci[1]] = opp_pts[fast_mod4(v_in_opp + 1) as usize];
                points[ci[2]] = opp_pts[fast_mod4(v_in_opp + 2) as usize];
                points[ci[3]] = opp_pts[fast_mod4(v_in_opp + 3) as usize];
            } else if (bnd_mask & (1 << i)) != 0 && (bnd_mask & (1 << fast_mod4(i + 3))) != 0 {
                points[ci[1]] = bnd_pt;
                points[ci[2]] = bnd_pt;
                points[ci[3]] = bnd_pt;
            } else if (bnd_mask & (1 << i)) != 0 {
                let manifold = !level.get_vertex_tag(v).non_manifold();
                let mut v_in_next = 0i32;
                let f_next = get_next_face_in_vert_faces(
                    level,
                    f_in_v,
                    &v_faces_arr,
                    &v_in_arr,
                    manifold,
                    &mut v_in_next,
                );
                let np = get_face_points(level, f_next, fvc);
                points[ci[1]] = np[fast_mod4(v_in_next + 3) as usize];
                points[ci[2]] = bnd_pt;
                points[ci[3]] = bnd_pt;
            } else {
                let manifold = !level.get_vertex_tag(v).non_manifold();
                let mut v_in_prev = 0i32;
                let f_prev = get_prev_face_in_vert_faces(
                    level,
                    f_in_v,
                    &v_faces_arr,
                    &v_in_arr,
                    manifold,
                    &mut v_in_prev,
                );
                let pp = get_face_points(level, f_prev, fvc);
                points[ci[1]] = bnd_pt;
                points[ci[2]] = bnd_pt;
                points[ci[3]] = pp[fast_mod4(v_in_prev + 1) as usize];
            }
            points[ci[0]] = f_points[i as usize];
        }
        16
    }

    fn get_tri_regular_patch_points(
        &self,
        level_idx: i32,
        face: Index,
        mut bnd_mask: i32,
        points: &mut [Index],
        fvc: i32,
    ) -> i32 {
        if bnd_mask < 0 {
            bnd_mask = self.get_regular_patch_boundary_mask(level_idx, face, fvc);
        }
        let interior = bnd_mask == 0;

        const PP: [[usize; 4]; 3] = [[4, 7, 3, 0], [5, 1, 2, 6], [8, 9, 11, 10]];

        let (mut e_mask, mut v_mask) = (0, 0);
        if !interior {
            let (e, v) = decode_tri_boundary_mask(bnd_mask);
            e_mask = e;
            v_mask = v;
        }

        let level = self.refiner.get_level_internal(level_idx);
        let f_verts = level.get_face_vertices(face);
        let f_points = get_face_points(level, face, fvc);

        let bnd_pt = if !interior && self.options.fill_missing_boundary_points {
            f_points[0]
        } else {
            INDEX_INVALID
        };

        for i in 0..3i32 {
            let v = f_verts[i];
            let ci = &PP[i as usize];

            let v_faces_arr: Vec<Index> = {
                let a = level.get_vertex_faces(v);
                (0..a.size()).map(|j| a[j]).collect()
            };
            let v_in_arr: Vec<LocalIndex> = {
                let a = level.get_vertex_face_local_indices(v);
                (0..a.size()).map(|j| a[j]).collect()
            };
            let f_in_v = v_faces_arr.iter().position(|&x| x == face).unwrap_or(0) as i32;

            let interior_corner = interior || (v_mask & (1 << i)) == 0;

            if interior_corner {
                let f2i = fast_mod_n(f_in_v + 2, 6);
                let f3i = fast_mod_n(f_in_v + 3, 6);
                let f2 = v_faces_arr[f2i as usize];
                let f3 = v_faces_arr[f3i as usize];
                let vin2 = v_in_arr[f2i as usize] as i32;
                let vin3 = v_in_arr[f3i as usize] as i32;
                let p2 = get_face_points(level, f2, fvc);
                let p3 = get_face_points(level, f3, fvc);
                points[ci[1]] = p2[fast_mod3(vin2 + 1) as usize];
                points[ci[2]] = p3[fast_mod3(vin3 + 1) as usize];
                points[ci[3]] = p3[fast_mod3(vin3 + 2) as usize];
            } else if (e_mask & (1 << i)) != 0 && (e_mask & (1 << fast_mod3(i + 2))) != 0 {
                points[ci[1]] = bnd_pt;
                points[ci[2]] = bnd_pt;
                points[ci[3]] = bnd_pt;
            } else if (e_mask & (1 << i)) != 0 {
                let n_vf = v_faces_arr.len() as i32;
                let f2i = fast_mod_n(f_in_v + 2, n_vf);
                let f2 = v_faces_arr[f2i as usize];
                let vin2 = v_in_arr[f2i as usize] as i32;
                let p2 = get_face_points(level, f2, fvc);
                points[ci[1]] = p2[fast_mod3(vin2 + 1) as usize];
                points[ci[2]] = p2[fast_mod3(vin2 + 2) as usize];
                points[ci[3]] = bnd_pt;
            } else if (e_mask & (1 << fast_mod3(i + 2))) != 0 {
                let n_vf = v_faces_arr.len() as i32;
                let f0i = fast_mod_n(f_in_v + n_vf - 2, n_vf);
                let f0 = v_faces_arr[f0i as usize];
                let vin0 = v_in_arr[f0i as usize] as i32;
                let p0 = get_face_points(level, f0, fvc);
                points[ci[1]] = bnd_pt;
                points[ci[2]] = bnd_pt;
                points[ci[3]] = p0[fast_mod3(vin0 + 1) as usize];
            } else {
                // Boundary vertex on edge
                let manifold = !level.get_vertex_tag(v).non_manifold();
                let mut vin2 = 0i32;
                let f2 = get_next_face_in_vert_faces(
                    level,
                    f_in_v,
                    &v_faces_arr,
                    &v_in_arr,
                    manifold,
                    &mut vin2,
                );
                let p2 = get_face_points(level, f2, fvc);
                points[ci[1]] = p2[fast_mod3(vin2 + 2) as usize];
                points[ci[2]] = bnd_pt;
                points[ci[3]] = bnd_pt;
            }
            points[ci[0]] = f_points[i as usize];
        }
        12
    }

    // -- Irregular patch methods --

    /// Assemble the SourcePatch from the topology around `face`.
    pub fn assemble_irregular_source_patch(
        &self,
        level_idx: i32,
        face: Index,
        corner_spans: &[VSpan; 4],
        source: &mut SourcePatch,
    ) -> i32 {
        let level = self.refiner.get_level_internal(level_idx);
        let f_verts = level.get_face_vertices(face);

        for c in 0..f_verts.size() as usize {
            let vt = level.get_vertex_tag(f_verts[c as i32]);
            let pc = &mut source.corners[c];

            if corner_spans[c].is_assigned() {
                pc.num_faces = corner_spans[c].num_faces;
                pc.patch_face = corner_spans[c].corner_in_span;
                pc.boundary = !corner_spans[c].periodic;
            } else {
                let vf = level.get_vertex_faces(f_verts[c as i32]);
                pc.num_faces = vf.size() as LocalIndex;
                pc.patch_face = vf.find_index(face) as LocalIndex;
                pc.boundary = vt.boundary();
            }
            pc.sharp = corner_spans[c].sharp;
            pc.dart = (vt.rule() == Rule::Dart.bits() as u16) && vt.inf_sharp_edges();
        }
        source.finalize(f_verts.size());
        source.num_source_points
    }

    /// Gather source point indices for an irregular patch.
    pub fn gather_irregular_source_points(
        &self,
        level_idx: i32,
        face: Index,
        corner_spans: &[VSpan; 4],
        source: &mut SourcePatch,
        patch_verts: &mut [Index],
        fvc: i32,
    ) -> i32 {
        let level = self.refiner.get_level_internal(level_idx);
        let f_verts = level.get_face_vertices(face);

        let mut src_ring = vec![0i32; source.max_ring_size as usize];
        let mut patch_ring = vec![0i32; source.max_ring_size as usize];

        for c in 0..source.num_corners as usize {
            let cv = f_verts[c as i32];

            let src_count = if corner_spans[c].is_assigned() {
                gather_regular_partial_ring(level, cv, &corner_spans[c], &mut src_ring, fvc)
            } else if source.num_corners == 4 {
                level.gather_quad_regular_ring_around_vertex(cv, &mut src_ring, fvc)
            } else {
                gather_tri_regular_ring(level, cv, &mut src_ring, fvc)
            };

            let patch_count = source.get_corner_ring_points(c as i32, &mut patch_ring);
            debug_assert_eq!(patch_count, src_count);

            for i in 0..patch_count as usize {
                debug_assert!((patch_ring[i] as i32) < source.num_source_points);
                patch_verts[patch_ring[i] as usize] = src_ring[i];
            }
        }
        source.num_source_points
    }

    /// Get irregular patch source points (combined assemble + gather).
    pub fn get_irregular_patch_source_points(
        &self,
        level_idx: i32,
        face: Index,
        corner_spans: &[VSpan; 4],
        source_points: &mut [Index],
        fvc: i32,
    ) -> i32 {
        let mut sp = SourcePatch::default();
        self.assemble_irregular_source_patch(level_idx, face, corner_spans, &mut sp);
        self.gather_irregular_source_points(
            level_idx,
            face,
            corner_spans,
            &mut sp,
            source_points,
            fvc,
        )
    }

    /// Get the conversion matrix for an irregular patch.
    pub fn get_irregular_patch_conversion_matrix(
        &self,
        level_idx: i32,
        face: Index,
        corner_spans: &[VSpan; 4],
        matrix: &mut SparseMatrix<f32>,
    ) -> i32 {
        let mut sp = SourcePatch::default();
        self.assemble_irregular_source_patch(level_idx, face, corner_spans, &mut sp);
        self.convert_to_patch_type(&sp, self.irreg_patch_type, matrix)
    }

    /// Scheme-specific patch conversion (dispatches by scheme type).
    pub fn convert_to_patch_type(
        &self,
        source: &SourcePatch,
        patch_type: PatchType,
        matrix: &mut SparseMatrix<f32>,
    ) -> i32 {
        match self.scheme_type {
            SchemeType::Bilinear => {
                // Bilinear conversion not yet supported
                let _ = (source, patch_type, matrix);
                -1
            }
            SchemeType::Catmark => {
                super::catmark_patch_builder::convert_catmark(source, patch_type, matrix)
            }
            SchemeType::Loop => super::loop_patch_builder::convert_loop(source, patch_type, matrix),
        }
    }

    // -- Single crease --

    pub fn is_regular_single_crease_patch(
        &self,
        level_idx: i32,
        face: Index,
        info: &mut SingleCreaseInfo,
    ) -> bool {
        if self.scheme_reg_face_size != 4 {
            return false;
        }
        let level = self.refiner.get_level_internal(level_idx);
        level.is_single_crease_patch_full(
            face,
            &mut info.crease_sharpness,
            &mut info.crease_edge_in_face,
        )
    }

    // -- PatchParam computation --

    /// Compute PatchParam for a given face in the hierarchy.
    pub fn compute_patch_param(
        &self,
        level_idx: i32,
        face: Index,
        ptex: &PtexIndices,
        is_regular: bool,
        boundary_mask: i32,
        compute_transition: bool,
    ) -> PatchParam {
        let depth = level_idx;
        let mut child_idx_in_parent = 0i32;
        let mut u = 0i32;
        let mut v = 0i32;
        let mut ofs = 1i32;

        let reg_size = self.scheme_reg_face_size;

        let tl = self.refiner.get_level(depth);
        let mut irreg_base = tl.get_face_vertices(face).size() != reg_size;

        let mut rotated = false;
        let mut child_face = face;

        for i in (1..=depth).rev() {
            let ref_ = self.refiner.get_refinement_internal(i - 1);
            let parent_level = self.refiner.get_level_internal(i - 1);
            let parent_face = ref_.get_child_face_parent_face(child_face);

            irreg_base = parent_level.get_face_vertices(parent_face).size() != reg_size;

            if reg_size == 3 {
                child_idx_in_parent = ref_.get_child_face_in_parent_face(child_face);
                if rotated {
                    match child_idx_in_parent {
                        0 => {}
                        1 => {
                            u -= ofs;
                        }
                        2 => {
                            v -= ofs;
                        }
                        3 => {
                            u += ofs;
                            v += ofs;
                            rotated = false;
                        }
                        _ => {}
                    }
                } else {
                    match child_idx_in_parent {
                        0 => {}
                        1 => {
                            u += ofs;
                        }
                        2 => {
                            v += ofs;
                        }
                        3 => {
                            u -= ofs;
                            v -= ofs;
                            rotated = true;
                        }
                        _ => {}
                    }
                }
                ofs <<= 1;
            } else if !irreg_base {
                child_idx_in_parent = ref_.get_child_face_in_parent_face(child_face);
                match child_idx_in_parent {
                    0 => {}
                    1 => {
                        u += ofs;
                    }
                    2 => {
                        u += ofs;
                        v += ofs;
                    }
                    3 => {
                        v += ofs;
                    }
                    _ => {}
                }
                ofs <<= 1;
            } else {
                let children = ref_.get_face_child_faces(parent_face);
                for j in 0..children.len() {
                    if children[j] == child_face {
                        child_idx_in_parent = j as i32;
                        break;
                    }
                }
            }
            child_face = parent_face;
        }

        if rotated {
            u += ofs;
            v += ofs;
        }

        let base_face = child_face;
        let mut ptex_index = ptex.get_face_id(base_face);
        debug_assert!(ptex_index != -1);
        if irreg_base {
            ptex_index += child_idx_in_parent;
        }

        let trans_mask = if compute_transition && level_idx < self.refiner.get_max_level() {
            let ref_ = self.refiner.get_refinement_internal(level_idx);
            ref_.get_parent_face_sparse_tag(face).transitional as i32
        } else {
            0
        };

        let mut param = PatchParam::default();
        param.set(
            ptex_index,
            u as i16,
            v as i16,
            depth as u16,
            irreg_base,
            boundary_mask as u16,
            trans_mask as u16,
            is_regular,
        );
        param
    }
}

// ---------------------------------------------------------------------------
// Scheme-specific patch type mapping (non-virtual dispatch)
// ---------------------------------------------------------------------------

fn bilinear_patch_type(basis: BasisType) -> PatchType {
    match basis {
        BasisType::Regular => PatchType::Quads,
        BasisType::Gregory => PatchType::GregoryBasis,
        BasisType::Linear => PatchType::Quads,
        _ => PatchType::NonPatch,
    }
}

fn catmark_patch_type(basis: BasisType) -> PatchType {
    match basis {
        BasisType::Regular => PatchType::Regular,
        BasisType::Gregory => PatchType::GregoryBasis,
        BasisType::Linear => PatchType::Quads,
        _ => PatchType::NonPatch,
    }
}

fn loop_patch_type(basis: BasisType) -> PatchType {
    match basis {
        BasisType::Regular => PatchType::Loop,
        BasisType::Gregory => PatchType::GregoryTriangle,
        BasisType::Linear => PatchType::Triangles,
        _ => PatchType::NonPatch,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_patch_quad_interior() {
        // 4 regular interior corners (valence 4, not boundary)
        let mut sp = SourcePatch::default();
        for c in 0..4 {
            sp.corners[c].num_faces = 4;
            sp.corners[c].patch_face = 0;
            sp.corners[c].boundary = false;
        }
        sp.finalize(4);
        assert_eq!(sp.num_corners, 4);
        // Regular interior quad: each corner has ring_size = 4*2 + 0 = 8
        for c in 0..4 {
            assert_eq!(sp.ring_sizes[c], 8);
        }
        // Total source points = 4 corners + local ring points
        assert!(sp.num_source_points > 4);
    }

    #[test]
    fn source_patch_tri_interior() {
        let mut sp = SourcePatch::default();
        for c in 0..3 {
            sp.corners[c].num_faces = 6;
            sp.corners[c].patch_face = 0;
            sp.corners[c].boundary = false;
        }
        sp.finalize(3);
        assert_eq!(sp.num_corners, 3);
        for c in 0..3 {
            assert_eq!(sp.ring_sizes[c], 6); // 6*1 + 0
        }
    }

    #[test]
    fn corner_ring_points_quad() {
        let mut sp = SourcePatch::default();
        for c in 0..4 {
            sp.corners[c].num_faces = 4;
            sp.corners[c].patch_face = 0;
            sp.corners[c].boundary = false;
        }
        sp.finalize(4);

        let mut ring = [0i32; 32];
        let n = sp.get_corner_ring_points(0, &mut ring);
        assert_eq!(n, sp.ring_sizes[0]);
    }

    #[test]
    fn basis_type_mapping() {
        assert_eq!(catmark_patch_type(BasisType::Regular), PatchType::Regular);
        assert_eq!(
            catmark_patch_type(BasisType::Gregory),
            PatchType::GregoryBasis
        );
        assert_eq!(catmark_patch_type(BasisType::Linear), PatchType::Quads);
        assert_eq!(loop_patch_type(BasisType::Regular), PatchType::Loop);
        assert_eq!(
            loop_patch_type(BasisType::Gregory),
            PatchType::GregoryTriangle
        );
        assert_eq!(bilinear_patch_type(BasisType::Regular), PatchType::Quads);
    }

    #[test]
    fn tri_boundary_encode_decode() {
        // No boundary
        let m = encode_tri_boundary_mask(0, 0);
        assert_eq!(m, 0);

        // One boundary edge
        let m = encode_tri_boundary_mask(1, 0);
        let (e, _v) = decode_tri_boundary_mask(m);
        assert_eq!(e, 1);

        // Boundary vertex only
        let m = encode_tri_boundary_mask(0, 2);
        let (e, v) = decode_tri_boundary_mask(m);
        assert_eq!(e, 0);
        assert_eq!(v, 2);
    }
}
