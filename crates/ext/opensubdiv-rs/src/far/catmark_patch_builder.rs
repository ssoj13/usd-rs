// Copyright 2018 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/catmarkPatchBuilder.h/.cpp
//
// Full port of CatmarkLimits, GregoryConverter, BSplineConverter and
// LinearConverter following the C++ implementation exactly.

#![allow(clippy::excessive_precision, clippy::too_many_arguments)]

use std::f64::consts::PI;

use super::patch_builder::{BasisType, PatchBuilderOptions, SourcePatch};
use super::patch_descriptor::PatchType;
use super::sparse_matrix::SparseMatrix;
use super::topology_refiner::TopologyRefiner;
use crate::vtr::types::Index;

// ---------------------------------------------------------------------------
// Weight trait — scalar operations needed on patch weights
// ---------------------------------------------------------------------------

/// Trait for types that can serve as patch conversion weights (f32 or f64).
pub trait Weight:
    Copy + Default
    + std::ops::Mul<Output = Self>
    + std::ops::Add<Output = Self>
    + std::ops::AddAssign
    + std::fmt::Debug
{
    fn from_f64(v: f64) -> Self;
    fn into_f64(self) -> f64;
    fn zero() -> Self;
    fn one() -> Self;
}

impl Weight for f32 {
    #[inline] fn from_f64(v: f64) -> Self { v as f32 }
    #[inline] fn into_f64(self) -> f64   { self as f64 }
    #[inline] fn zero() -> Self { 0.0 }
    #[inline] fn one()  -> Self { 1.0 }
}

impl Weight for f64 {
    #[inline] fn from_f64(v: f64) -> Self { v }
    #[inline] fn into_f64(self) -> f64   { self }
    #[inline] fn zero() -> Self { 0.0 }
    #[inline] fn one()  -> Self { 1.0 }
}

// ---------------------------------------------------------------------------
// CatmarkLimits — limit-position and limit-tangent weight computation
// ---------------------------------------------------------------------------

/// Pre-computed eigenvalue coefficient table for valences 0..29.
/// Entry 0,1,2 are unused (valence < 3 doesn't arise in valid Catmark meshes).
static EF_TABLE: [f64; 30] = [
    0.0,                    0.0,                    0.0,
    8.1281572906372312e-01, 0.5,                    3.6364406329142801e-01,
    2.8751379706077085e-01, 2.3868786685851678e-01, 2.0454364190756097e-01,
    1.7922903958061159e-01, 1.5965737079986253e-01, 1.4404233443011302e-01,
    1.3127568415883017e-01, 1.2063172212675841e-01, 1.1161437506676930e-01,
    1.0387245516114274e-01, 9.7150019090724835e-02, 9.1255917505950648e-02,
    8.6044378511602668e-02, 8.1402211336798411e-02, 7.7240129516184072e-02,
    7.3486719751997026e-02, 7.0084157479797987e-02, 6.6985104030725440e-02,
    6.4150420569810074e-02, 6.1547457638637268e-02, 5.9148757447233989e-02,
    5.6931056818776957e-02, 5.4874512279256417e-02, 5.2962091433796134e-02,
];

/// Compute the eigenvalue-derived scale coefficient for the given valence.
///
/// Uses the table for valence < 30, closed-form formula for valence >= 30.
#[inline]
fn compute_ef_coefficient(valence: i32) -> f64 {
    debug_assert!(valence > 0);
    if valence < 30 {
        return EF_TABLE[valence as usize];
    }
    let inv_val = 1.0 / valence as f64;
    let cos_t = (2.0 * PI * inv_val).cos();
    let divisor = (cos_t + 5.0) + ((cos_t + 9.0) * (cos_t + 1.0)).sqrt();
    16.0 * inv_val / divisor
}

/// Compute limit position and edge-point weights for an **interior** vertex.
///
/// `valence` — number of incident faces.
/// `face_in_ring` — which face in the ring the patch is attached to.
/// `p_weights`  — output weights for limit position P  (length 1 + 2*valence)
/// `ep_weights` — output weights for edge point Ep (same length, or None)
/// `em_weights` — output weights for edge point Em (same length, or None)
pub fn compute_interior_point_weights(
    valence: i32,
    face_in_ring: i32,
    p_weights: &mut [f64],
    ep_weights: Option<&mut [f64]>,
    em_weights: Option<&mut [f64]>,
) {
    let compute_edge = ep_weights.is_some() && em_weights.is_some();

    let f_val = valence as f64;
    let one_over_val = 1.0 / f_val;
    let one_over_val_plus5 = 1.0 / (f_val + 5.0);

    let p_coeff = one_over_val * one_over_val_plus5;
    let tan_coeff = compute_ef_coefficient(valence) * 0.5 * one_over_val_plus5;

    let face_angle = 2.0 * PI * one_over_val;

    let weight_width = (1 + 2 * valence) as usize;
    let mut tan_weights = vec![0.0f64; weight_width];

    p_weights[0] = f_val * one_over_val_plus5;

    for i in 0..valence as usize {
        p_weights[1 + 2 * i]     = p_coeff * 4.0;
        p_weights[1 + 2 * i + 1] = p_coeff;

        if compute_edge {
            let i_prev = (i + valence as usize - 1) % valence as usize;
            let i_next = (i + 1) % valence as usize;

            let cos_i_coeff = tan_coeff * (face_angle * i as f64).cos();

            tan_weights[1 + 2 * i_prev]     += cos_i_coeff * 2.0;
            tan_weights[1 + 2 * i_prev + 1] += cos_i_coeff;
            tan_weights[1 + 2 * i]          += cos_i_coeff * 4.0;
            tan_weights[1 + 2 * i + 1]      += cos_i_coeff;
            tan_weights[1 + 2 * i_next]     += cos_i_coeff * 2.0;
        }
    }

    if let (Some(ep), Some(em)) = (ep_weights, em_weights) {
        let val = valence as usize;
        let ep_offset = 2 * ((val - face_in_ring as usize) % val);
        let em_offset = 2 * ((val - face_in_ring as usize + val - 1) % val);

        ep[0] = p_weights[0];
        em[0] = p_weights[0];

        for i in 1..weight_width {
            let mut ip = i + ep_offset;
            if ip >= weight_width { ip -= weight_width - 1; }

            let mut im = i + em_offset;
            if im >= weight_width { im -= weight_width - 1; }

            ep[i] = p_weights[i] + tan_weights[ip];
            em[i] = p_weights[i] + tan_weights[im];
        }
    }
}

/// Compute limit position and edge-point weights for a **boundary** vertex.
///
/// `valence` — effective valence including the two boundary edges (numFaces+1).
pub fn compute_boundary_point_weights(
    valence: i32,
    face_in_ring: i32,
    p_weights: &mut [f64],
    ep_weights: Option<&mut [f64]>,
    em_weights: Option<&mut [f64]>,
) {
    let num_faces = valence - 1;
    let face_angle = PI / num_faces as f64;
    let weight_width = (2 * valence) as usize;
    let n = weight_width - 1; // index N

    // Position weights — only two non-zero entries at ends of boundary edges:
    for w in p_weights[..weight_width].iter_mut() { *w = 0.0; }
    p_weights[0] = 4.0 / 6.0;
    p_weights[1] = 1.0 / 6.0;
    p_weights[n] = 1.0 / 6.0;

    if ep_weights.is_none() && em_weights.is_none() {
        return;
    }

    // Interior tangent coefficients for the boundary case:
    let t_bnd_coeff_1 =  1.0 / 6.0;
    let t_bnd_coeff_n = -1.0 / 6.0;

    let mut tan_weights = vec![0.0f64; weight_width];
    {
        let k = num_faces as f64;
        let theta = face_angle;
        let c = theta.cos();
        let s = theta.sin();
        let div3 = 1.0 / 3.0;
        let div3kc = 1.0 / (3.0 * k + c);
        let gamma   = -4.0 * s * div3kc;
        let alpha_0k = -((1.0 + 2.0 * c) * (1.0 + c).sqrt()) * div3kc
                       / (1.0 - c).sqrt();
        let beta_0  =  s * div3kc;

        tan_weights[0] = gamma   * div3;
        tan_weights[1] = alpha_0k * div3;
        tan_weights[2] = beta_0   * div3;
        tan_weights[n] = alpha_0k * div3;

        for i in 1..(valence - 1) as usize {
            let sin_i   = (theta * i as f64).sin();
            let sin_ip1 = (theta * (i + 1) as f64).sin();

            let alpha = 4.0 * sin_i * div3kc;
            let beta  = (sin_i + sin_ip1) * div3kc;

            tan_weights[1 + 2 * i]     = alpha * div3;
            tan_weights[1 + 2 * i + 1] = beta  * div3;
        }
    }

    if let Some(ep) = ep_weights {
        if face_in_ring == 0 {
            // Ep lies on the leading boundary edge — only two weights:
            for w in ep[..weight_width].iter_mut() { *w = 0.0; }
            ep[0] = 2.0 / 3.0;
            ep[1] = 1.0 / 3.0;
        } else {
            let fa_next = face_angle * face_in_ring as f64;
            let cos_next = fa_next.cos();
            let sin_next = fa_next.sin();

            for i in 0..weight_width {
                ep[i] = tan_weights[i] * sin_next;
            }
            ep[0] += p_weights[0];
            ep[1] += p_weights[1] + t_bnd_coeff_1 * cos_next;
            ep[n] += p_weights[n] + t_bnd_coeff_n * cos_next;
        }
    }

    if let Some(em) = em_weights {
        if face_in_ring == num_faces - 1 {
            // Em lies on the trailing boundary edge — only two weights:
            for w in em[..weight_width].iter_mut() { *w = 0.0; }
            em[0] = 2.0 / 3.0;
            em[n] = 1.0 / 3.0;
        } else {
            let i_edge_prev = (face_in_ring + 1) % valence;
            let fa_prev = face_angle * i_edge_prev as f64;
            let cos_prev = fa_prev.cos();
            let sin_prev = fa_prev.sin();

            for i in 0..weight_width {
                em[i] = tan_weights[i] * sin_prev;
            }
            em[0] += p_weights[0];
            em[1] += p_weights[1] + t_bnd_coeff_1 * cos_prev;
            em[n] += p_weights[n] + t_bnd_coeff_n * cos_prev;
        }
    }
}

// ---------------------------------------------------------------------------
// SparseRow helper — copy one row to another in the same matrix
// ---------------------------------------------------------------------------

fn copy_matrix_row<R: Weight>(matrix: &mut SparseMatrix<R>, dst: i32, src: i32) {
    debug_assert_eq!(matrix.get_row_size(dst), matrix.get_row_size(src));
    let size = matrix.get_row_size(src) as usize;
    let src_cols: Vec<i32> = matrix.get_row_columns(src).to_vec();
    let src_elems: Vec<R>  = matrix.get_row_elements(src).to_vec();
    let (dc, de) = matrix.get_row_data_mut(dst);
    dc[..size].copy_from_slice(&src_cols[..size]);
    de[..size].copy_from_slice(&src_elems[..size]);
}

// ---------------------------------------------------------------------------
// Matrix utility helpers
// ---------------------------------------------------------------------------

fn init_full_matrix<R: Weight>(m: &mut SparseMatrix<R>, n_rows: i32, n_cols: i32) {
    m.resize(n_rows, n_cols, n_rows * n_cols);
    m.set_row_size(0, n_cols);
    {
        let cols = m.get_row_columns_mut(0);
        for i in 0..n_cols as usize { cols[i] = i as i32; }
    }
    let cols0: Vec<i32> = m.get_row_columns(0).to_vec();
    for row in 1..n_rows {
        m.set_row_size(row, n_cols);
        let dst = m.get_row_columns_mut(row);
        dst.copy_from_slice(&cols0);
    }
}

fn resize_matrix<R: Weight>(
    matrix: &mut SparseMatrix<R>,
    n_rows: i32, n_cols: i32, n_elements: i32,
    row_sizes: &[i32],
) {
    matrix.resize(n_rows, n_cols, n_elements);
    for i in 0..n_rows {
        matrix.set_row_size(i, row_sizes[i as usize]);
    }
    debug_assert_eq!(matrix.get_num_elements(), n_elements);
}

/// Remove duplicate column entries caused by valence-2 interior vertices.
fn remove_valence2_duplicates<R: Weight>(m: &mut SparseMatrix<R>) {
    let reg_face_size = 4usize;
    let n_rows  = m.get_num_rows();
    let n_cols  = m.get_num_columns();
    let n_elems = m.get_num_elements();

    let mut t: SparseMatrix<R> = SparseMatrix::new();
    t.resize(n_rows, n_cols, n_elems);

    for row in 0..n_rows {
        let src_size = m.get_row_size(row) as usize;
        let src_indices: Vec<i32> = m.get_row_columns(row).to_vec();
        let src_weights: Vec<R>   = m.get_row_elements(row).to_vec();

        let mut corner_used = [false; 4];
        let mut dup_count = 0usize;
        for i in 0..src_size {
            let idx = src_indices[i] as usize;
            if idx < reg_face_size {
                if corner_used[idx] { dup_count += 1; }
                corner_used[idx] = true;
            }
        }

        t.set_row_size(row, (src_size - dup_count) as i32);
        let (dst_indices, dst_weights) = t.get_row_data_mut(row);

        if dup_count > 0 {
            let mut corner_dst_pos: [Option<usize>; 4] = [None; 4];
            let mut write_pos = 0usize;

            for i in 0..src_size {
                let idx = src_indices[i] as usize;
                let w   = src_weights[i];

                if idx < reg_face_size {
                    if let Some(pos) = corner_dst_pos[idx] {
                        dst_weights[pos] += w;
                        continue;
                    } else {
                        corner_dst_pos[idx] = Some(write_pos);
                    }
                }
                dst_indices[write_pos] = idx as i32;
                dst_weights[write_pos] = w;
                write_pos += 1;
            }
        } else {
            dst_indices[..src_size].copy_from_slice(&src_indices);
            dst_weights[..src_size].copy_from_slice(&src_weights);
        }
    }
    m.swap(&mut t);
}

// ---------------------------------------------------------------------------
// GregoryConverter — 20-point Gregory patch from source patch neighborhood
// ---------------------------------------------------------------------------

/// Corner topology data used internally by GregoryConverter.
struct CornerTopology {
    is_boundary:    bool,
    is_sharp:       bool,
    #[allow(dead_code)]
    is_dart:        bool,
    is_regular:     bool,
    #[allow(dead_code)]
    is_val2_int:    bool,

    ep_on_boundary: bool,
    em_on_boundary: bool,

    fp_is_regular:  bool,
    fm_is_regular:  bool,
    fp_is_copied:   bool,
    fm_is_copied:   bool,

    valence:       i32,
    num_faces:     i32,
    face_in_ring:  i32,

    face_angle:     f64,
    cos_face_angle: f64,
    sin_face_angle: f64,

    ring_points: Vec<i32>,
}

impl CornerTopology {
    fn new() -> Self {
        Self {
            is_boundary:    false,
            is_sharp:       false,
            is_dart:        false,
            is_regular:     false,
            is_val2_int:    false,
            ep_on_boundary: false,
            em_on_boundary: false,
            fp_is_regular:  false,
            fm_is_regular:  false,
            fp_is_copied:   false,
            fm_is_copied:   false,
            valence:        0,
            num_faces:      0,
            face_in_ring:   0,
            face_angle:     0.0,
            cos_face_angle: 0.0,
            sin_face_angle: 0.0,
            ring_points:    Vec::new(),
        }
    }
}

struct GregoryConverter {
    num_source_points: i32,
    max_valence:       i32,

    is_isolated_interior: bool,
    has_val2_interior:    bool,
    isolated_corner:      i32,
    isolated_valence:     i32,

    corners: [CornerTopology; 4],
}

impl GregoryConverter {
    fn new(source_patch: &SourcePatch) -> Self {
        let corners = [
            CornerTopology::new(),
            CornerTopology::new(),
            CornerTopology::new(),
            CornerTopology::new(),
        ];
        let mut gc = Self {
            num_source_points: 0,
            max_valence:       0,
            is_isolated_interior: false,
            has_val2_interior:    false,
            isolated_corner:      -1,
            isolated_valence:     -1,
            corners,
        };
        gc.initialize(source_patch);
        gc
    }

    fn initialize(&mut self, source_patch: &SourcePatch) {
        self.num_source_points = source_patch.get_num_source_points();
        self.max_valence       = source_patch.max_valence;

        let mut boundary_count   = 0i32;
        let mut irregular_count  = 0i32;
        let mut irregular_corner = -1i32;
        let mut irregular_valence = -1i32;
        let mut sharp_count      = 0i32;
        let mut val2_int_count   = 0i32;

        for c_idx in 0..4usize {
            let src = &source_patch.corners[c_idx];
            let c = &mut self.corners[c_idx];

            c.is_boundary  = src.boundary;
            c.is_sharp     = src.sharp;
            c.is_dart      = src.dart;
            c.num_faces    = src.num_faces as i32;
            c.face_in_ring = src.patch_face as i32;
            c.is_val2_int  = src.val2_interior;
            c.valence      = c.num_faces + c.is_boundary as i32;

            c.is_regular = ((c.num_faces << (c.is_boundary as i32)) == 4)
                         && !c.is_sharp;

            if c.is_regular {
                c.face_angle    = std::f64::consts::FRAC_PI_2;
                c.cos_face_angle = 0.0;
                c.sin_face_angle = 1.0;
            } else {
                c.face_angle = (if c.is_boundary { PI } else { 2.0 * PI })
                               / c.num_faces as f64;
                c.cos_face_angle = c.face_angle.cos();
                c.sin_face_angle = c.face_angle.sin();
            }

            let ring_size = source_patch.get_corner_ring_size(c_idx);
            c.ring_points.resize(ring_size as usize, 0);
            source_patch.get_corner_ring_points(c_idx as i32, &mut c.ring_points);

            boundary_count  += c.is_boundary as i32;
            if !c.is_regular {
                irregular_count  += 1;
                irregular_corner  = c_idx as i32;
                irregular_valence = c.valence;
            }
            sharp_count    += c.is_sharp as i32;
            val2_int_count += c.is_val2_int as i32;
        }

        // Second pass: tags depending on adjacent corners.
        for c_idx in 0..4usize {
            let c_next = (c_idx + 1) & 0x3;
            let c_prev = (c_idx + 3) & 0x3;

            let cn_is_regular = self.corners[c_next].is_regular;
            let cp_is_regular = self.corners[c_prev].is_regular;

            let c = &mut self.corners[c_idx];

            c.ep_on_boundary = false;
            c.em_on_boundary = false;

            c.fp_is_regular = c.is_regular && cn_is_regular;
            c.fm_is_regular = c.is_regular && cp_is_regular;
            c.fp_is_copied  = false;
            c.fm_is_copied  = false;

            if c.is_boundary {
                c.ep_on_boundary = c.face_in_ring == 0;
                c.em_on_boundary = c.face_in_ring == (c.num_faces - 1);

                if c.num_faces > 1 {
                    if c.ep_on_boundary {
                        c.fp_is_regular = c.fm_is_regular;
                        c.fp_is_copied  = !c.fp_is_regular;
                    }
                    if c.em_on_boundary {
                        c.fm_is_regular = c.fp_is_regular;
                        c.fm_is_copied  = !c.fm_is_regular;
                    }
                } else {
                    // Single-face boundary — always regular:
                    c.fp_is_regular = true;
                    c.fm_is_regular = true;
                }
            }
        }

        self.is_isolated_interior = (irregular_count == 1)
            && (boundary_count == 0)
            && (irregular_valence > 2)
            && (sharp_count == 0);
        if self.is_isolated_interior {
            self.isolated_corner  = irregular_corner;
            self.isolated_valence = irregular_valence;
        }
        self.has_val2_interior = val2_int_count > 0;
    }

    fn convert<R: Weight>(&self, matrix: &mut SparseMatrix<R>) {
        if self.is_isolated_interior {
            self.resize_matrix_isolated_irregular(
                matrix, self.isolated_corner, self.isolated_valence);
        } else {
            self.resize_matrix_unisolated(matrix);
        }

        let max_ring_size = 1 + 2 * self.max_valence;
        let weight_buf_size = (3 * max_ring_size).max(2 * self.num_source_points) as usize;
        let mut weight_buf = vec![0.0f64; weight_buf_size];
        let mut index_buf  = vec![0i32;   weight_buf_size];

        // --- Edge points P, Ep, Em ---
        for c_idx in 0..4usize {
            if self.corners[c_idx].is_regular {
                self.assign_regular_edge_points(c_idx, matrix);
            } else {
                self.compute_irregular_edge_points(c_idx, matrix, &mut weight_buf);
            }
        }

        // --- Face points Fp, Fm ---
        for c_idx in 0..4usize {
            let (fp_reg, fm_reg) = {
                let c = &self.corners[c_idx];
                (c.fp_is_regular, c.fm_is_regular)
            };
            if fp_reg || fm_reg {
                self.assign_regular_face_points(c_idx, matrix);
            }
            if !fp_reg || !fm_reg {
                self.compute_irregular_face_points(
                    c_idx, matrix, &mut weight_buf, &mut index_buf);
            }
        }

        if self.has_val2_interior {
            remove_valence2_duplicates(matrix);
        }
    }

    // ---- Matrix sizing ----

    fn resize_matrix_isolated_irregular<R: Weight>(
        &self,
        matrix: &mut SparseMatrix<R>,
        corner_index: i32,
        corner_valence: i32,
    ) {
        let irr_ring = 1 + 2 * corner_valence;

        let irr_c    = corner_index as usize;
        let irr_plus = (corner_index as usize + 1) & 0x3;
        let irr_opp  = (corner_index as usize + 2) & 0x3;
        let irr_min  = (corner_index as usize + 3) & 0x3;

        let mut row_sizes = [0i32; 20];

        for i in 0..5usize {
            row_sizes[irr_c * 5 + i] = irr_ring;
        }
        row_sizes[irr_plus * 5 + 0] = 9;
        row_sizes[irr_plus * 5 + 1] = 6;
        row_sizes[irr_plus * 5 + 2] = 6;
        row_sizes[irr_plus * 5 + 3] = 4;
        row_sizes[irr_plus * 5 + 4] = 3 + irr_ring;

        row_sizes[irr_opp * 5 + 0] = 9;
        row_sizes[irr_opp * 5 + 1] = 6;
        row_sizes[irr_opp * 5 + 2] = 6;
        row_sizes[irr_opp * 5 + 3] = 4;
        row_sizes[irr_opp * 5 + 4] = 4;

        row_sizes[irr_min * 5 + 0] = 9;
        row_sizes[irr_min * 5 + 1] = 6;
        row_sizes[irr_min * 5 + 2] = 6;
        row_sizes[irr_min * 5 + 3] = 3 + irr_ring;
        row_sizes[irr_min * 5 + 4] = 4;

        let num_elements = 7 * irr_ring + 85;
        resize_matrix(matrix, 20, self.num_source_points, num_elements, &row_sizes);
    }

    fn resize_matrix_unisolated<R: Weight>(&self, matrix: &mut SparseMatrix<R>) {
        let mut row_sizes = [0i32; 20];
        let mut num_elements = 0i32;

        for c_idx in 0..4usize {
            let c = &self.corners[c_idx];
            let base = c_idx * 5;

            // P, Ep, Em sizes:
            if c.is_regular {
                if !c.is_boundary {
                    row_sizes[base + 0] = 9;
                    row_sizes[base + 1] = 6;
                    row_sizes[base + 2] = 6;
                } else {
                    row_sizes[base + 0] = 3;
                    row_sizes[base + 1] = if c.ep_on_boundary { 2 } else { 6 };
                    row_sizes[base + 2] = if c.em_on_boundary { 2 } else { 6 };
                }
            } else if c.is_sharp {
                row_sizes[base + 0] = 1;
                row_sizes[base + 1] = 2;
                row_sizes[base + 2] = 2;
            } else if !c.is_boundary {
                let ring = 1 + 2 * c.valence;
                row_sizes[base + 0] = ring;
                row_sizes[base + 1] = ring;
                row_sizes[base + 2] = ring;
            } else if c.num_faces > 1 {
                let ring = 1 + c.valence + c.num_faces;
                row_sizes[base + 0] = 3;
                row_sizes[base + 1] = if c.ep_on_boundary { 2 } else { ring };
                row_sizes[base + 2] = if c.em_on_boundary { 2 } else { ring };
            } else {
                row_sizes[base + 0] = 3;
                row_sizes[base + 1] = 2;
                row_sizes[base + 2] = 2;
            }
            num_elements += row_sizes[base] + row_sizes[base + 1] + row_sizes[base + 2];

            // Fp, Fm sizes:
            row_sizes[base + 3] = 4;
            row_sizes[base + 4] = 4;
            if !c.fp_is_regular || !c.fm_is_regular {
                let c_next = (c_idx + 1) & 0x3;
                let c_prev = (c_idx + 3) & 0x3;
                if !c.fp_is_regular {
                    let far = if c.fp_is_copied { c_prev } else { c_next };
                    row_sizes[base + 3] = self.get_irregular_face_point_size(c_idx, far);
                }
                if !c.fm_is_regular {
                    let far = if c.fm_is_copied { c_next } else { c_prev };
                    row_sizes[base + 4] = self.get_irregular_face_point_size(c_idx, far);
                }
            }
            num_elements += row_sizes[base + 3] + row_sizes[base + 4];
        }

        resize_matrix(matrix, 20, self.num_source_points, num_elements, &row_sizes);
    }

    // ---- Regular edge points ----

    fn assign_regular_edge_points<R: Weight>(&self, c_idx: usize, matrix: &mut SparseMatrix<R>) {
        let c = &self.corners[c_idx];
        let ring = &c.ring_points;

        let row_p  = (5 * c_idx) as i32;
        let row_ep = row_p + 1;
        let row_em = row_p + 2;

        if !c.is_boundary {
            // Interior regular — 9-point P stencil:
            {
                let (ci, cw) = matrix.get_row_data_mut(row_p);
                ci[0] = c_idx as i32; cw[0] = R::from_f64(4.0 / 9.0);
                ci[1] = ring[0];      cw[1] = R::from_f64(1.0 / 9.0);
                ci[2] = ring[2];      cw[2] = R::from_f64(1.0 / 9.0);
                ci[3] = ring[4];      cw[3] = R::from_f64(1.0 / 9.0);
                ci[4] = ring[6];      cw[4] = R::from_f64(1.0 / 9.0);
                ci[5] = ring[1];      cw[5] = R::from_f64(1.0 / 36.0);
                ci[6] = ring[3];      cw[6] = R::from_f64(1.0 / 36.0);
                ci[7] = ring[5];      cw[7] = R::from_f64(1.0 / 36.0);
                ci[8] = ring[7];      cw[8] = R::from_f64(1.0 / 36.0);
            }

            let i_ep = 2 *  c.face_in_ring as usize;
            let i_em = 2 * ((c.face_in_ring + 1) & 0x3) as usize;
            let i_op = 2 * ((c.face_in_ring + 2) & 0x3) as usize;
            let i_om = 2 * ((c.face_in_ring + 3) & 0x3) as usize;

            // 6-point Ep:
            {
                let (ci, cw) = matrix.get_row_data_mut(row_ep);
                ci[0] = c_idx as i32; cw[0] = R::from_f64(4.0 / 9.0);
                ci[1] = ring[i_ep];   cw[1] = R::from_f64(2.0 / 9.0);
                ci[2] = ring[i_em];   cw[2] = R::from_f64(1.0 / 9.0);
                ci[3] = ring[i_om];   cw[3] = R::from_f64(1.0 / 9.0);
                ci[4] = ring[i_ep+1]; cw[4] = R::from_f64(1.0 / 18.0);
                ci[5] = ring[i_om+1]; cw[5] = R::from_f64(1.0 / 18.0);
            }
            // 6-point Em:
            {
                let (ci, cw) = matrix.get_row_data_mut(row_em);
                ci[0] = c_idx as i32; cw[0] = R::from_f64(4.0 / 9.0);
                ci[1] = ring[i_em];   cw[1] = R::from_f64(2.0 / 9.0);
                ci[2] = ring[i_ep];   cw[2] = R::from_f64(1.0 / 9.0);
                ci[3] = ring[i_op];   cw[3] = R::from_f64(1.0 / 9.0);
                ci[4] = ring[i_ep+1]; cw[4] = R::from_f64(1.0 / 18.0);
                ci[5] = ring[i_em+1]; cw[5] = R::from_f64(1.0 / 18.0);
            }
        } else {
            // Boundary regular — 3-point P:
            {
                let (ci, cw) = matrix.get_row_data_mut(row_p);
                ci[0] = c_idx as i32; cw[0] = R::from_f64(2.0 / 3.0);
                ci[1] = ring[0];      cw[1] = R::from_f64(1.0 / 6.0);
                ci[2] = ring[4];      cw[2] = R::from_f64(1.0 / 6.0);
            }
            // Boundary edge vs interior edge:
            let (row_bnd, row_int, i_bnd) = if c.ep_on_boundary {
                (row_ep, row_em, 0usize)
            } else {
                (row_em, row_ep, 4usize)
            };
            // 2-point boundary edge:
            {
                let (ci, cw) = matrix.get_row_data_mut(row_bnd);
                ci[0] = c_idx as i32; cw[0] = R::from_f64(2.0 / 3.0);
                ci[1] = ring[i_bnd];  cw[1] = R::from_f64(1.0 / 3.0);
            }
            // 6-point interior edge:
            {
                let (ci, cw) = matrix.get_row_data_mut(row_int);
                ci[0] = c_idx as i32; cw[0] = R::from_f64(4.0 / 9.0);
                ci[1] = ring[2];      cw[1] = R::from_f64(2.0 / 9.0);
                ci[2] = ring[0];      cw[2] = R::from_f64(1.0 / 9.0);
                ci[3] = ring[4];      cw[3] = R::from_f64(1.0 / 9.0);
                ci[4] = ring[1];      cw[4] = R::from_f64(1.0 / 18.0);
                ci[5] = ring[3];      cw[5] = R::from_f64(1.0 / 18.0);
            }
        }
    }

    // ---- Irregular edge points ----

    fn compute_irregular_edge_points<R: Weight>(
        &self, c_idx: usize, matrix: &mut SparseMatrix<R>, weight_buf: &mut [f64],
    ) {
        let c = &self.corners[c_idx];
        let row_p  = (5 * c_idx) as i32;
        let row_ep = row_p + 1;
        let row_em = row_p + 2;

        if c.is_sharp {
            let (ci, cw) = matrix.get_row_data_mut(row_p);
            ci[0] = c_idx as i32; cw[0] = R::one();

            let (ci, cw) = matrix.get_row_data_mut(row_ep);
            ci[0] = c_idx as i32;          cw[0] = R::from_f64(2.0 / 3.0);
            ci[1] = ((c_idx+1)&0x3) as i32; cw[1] = R::from_f64(1.0 / 3.0);

            let (ci, cw) = matrix.get_row_data_mut(row_em);
            ci[0] = c_idx as i32;          cw[0] = R::from_f64(2.0 / 3.0);
            ci[1] = ((c_idx+3)&0x3) as i32; cw[1] = R::from_f64(1.0 / 3.0);

        } else if !c.is_boundary {
            self.compute_irregular_interior_edge_points(c_idx, matrix, weight_buf);
        } else if c.num_faces > 1 {
            self.compute_irregular_boundary_edge_points(c_idx, matrix, weight_buf);
        } else {
            // Smooth corner (numFaces==1, boundary):
            {
                let (ci, cw) = matrix.get_row_data_mut(row_p);
                ci[0] = c_idx as i32;          cw[0] = R::from_f64(4.0 / 6.0);
                ci[1] = ((c_idx+1)&0x3) as i32; cw[1] = R::from_f64(1.0 / 6.0);
                ci[2] = ((c_idx+3)&0x3) as i32; cw[2] = R::from_f64(1.0 / 6.0);
            }
            {
                let (ci, cw) = matrix.get_row_data_mut(row_ep);
                ci[0] = c_idx as i32;          cw[0] = R::from_f64(2.0 / 3.0);
                ci[1] = ((c_idx+1)&0x3) as i32; cw[1] = R::from_f64(1.0 / 3.0);
            }
            {
                let (ci, cw) = matrix.get_row_data_mut(row_em);
                ci[0] = c_idx as i32;          cw[0] = R::from_f64(2.0 / 3.0);
                ci[1] = ((c_idx+3)&0x3) as i32; cw[1] = R::from_f64(1.0 / 3.0);
            }
        }
    }

    fn compute_irregular_interior_edge_points<R: Weight>(
        &self, c_idx: usize, matrix: &mut SparseMatrix<R>, ring_weights: &mut [f64],
    ) {
        let c = &self.corners[c_idx];
        let valence = c.valence;
        let ww = (1 + 2 * valence) as usize;

        // Use non-overlapping slices from ring_weights:
        let (p_weights, rest) = ring_weights.split_at_mut(ww);
        let (ep_weights, em_weights) = rest.split_at_mut(ww);

        compute_interior_point_weights(
            valence, c.face_in_ring,
            p_weights, Some(ep_weights), Some(em_weights));

        let row_p  = (5 * c_idx) as i32;
        let row_ep = row_p + 1;
        let row_em = row_p + 2;

        {
            let (ci, cw) = matrix.get_row_data_mut(row_p);
            ci[0] = c_idx as i32; cw[0] = R::from_f64(p_weights[0]);
            for i in 1..ww {
                ci[i] = c.ring_points[i - 1];
                cw[i] = R::from_f64(p_weights[i]);
            }
        }
        {
            let (ci, cw) = matrix.get_row_data_mut(row_ep);
            ci[0] = c_idx as i32; cw[0] = R::from_f64(ep_weights[0]);
            for i in 1..ww {
                ci[i] = c.ring_points[i - 1];
                cw[i] = R::from_f64(ep_weights[i]);
            }
        }
        {
            let (ci, cw) = matrix.get_row_data_mut(row_em);
            ci[0] = c_idx as i32; cw[0] = R::from_f64(em_weights[0]);
            for i in 1..ww {
                ci[i] = c.ring_points[i - 1];
                cw[i] = R::from_f64(em_weights[i]);
            }
        }
    }

    fn compute_irregular_boundary_edge_points<R: Weight>(
        &self, c_idx: usize, matrix: &mut SparseMatrix<R>, ring_weights: &mut [f64],
    ) {
        let c = &self.corners[c_idx];
        let valence = c.valence;
        let ww = (1 + valence + c.num_faces) as usize;
        let nn = ww - 1;

        let (p_weights, rest) = ring_weights.split_at_mut(ww);
        let (ep_weights, em_weights) = rest.split_at_mut(ww);

        compute_boundary_point_weights(
            valence, c.face_in_ring,
            p_weights, Some(ep_weights), Some(em_weights));

        let row_p  = (5 * c_idx) as i32;
        let row_ep = row_p + 1;
        let row_em = row_p + 2;

        let p0 = c_idx as i32;
        let p1 = c.ring_points[0];
        let pn = c.ring_points[2 * (valence - 1) as usize];

        {
            let (ci, cw) = matrix.get_row_data_mut(row_p);
            ci[0] = p0; cw[0] = R::from_f64(p_weights[0]);
            ci[1] = p1; cw[1] = R::from_f64(p_weights[1]);
            ci[2] = pn; cw[2] = R::from_f64(p_weights[nn]);
        }
        {
            let (ci, cw) = matrix.get_row_data_mut(row_ep);
            ci[0] = p0; cw[0] = R::from_f64(ep_weights[0]);
            if c.ep_on_boundary {
                ci[1] = p1; cw[1] = R::from_f64(ep_weights[1]);
            } else {
                for i in 1..ww {
                    ci[i] = c.ring_points[i - 1];
                    cw[i] = R::from_f64(ep_weights[i]);
                }
            }
        }
        {
            let (ci, cw) = matrix.get_row_data_mut(row_em);
            ci[0] = p0; cw[0] = R::from_f64(em_weights[0]);
            if c.em_on_boundary {
                ci[1] = pn; cw[1] = R::from_f64(em_weights[nn]);
            } else {
                for i in 1..ww {
                    ci[i] = c.ring_points[i - 1];
                    cw[i] = R::from_f64(em_weights[i]);
                }
            }
        }
    }

    // ---- Face-point sizing ----

    fn get_irregular_face_point_size(&self, c_near: usize, c_far: usize) -> i32 {
        let cn = &self.corners[c_near];
        let cf = &self.corners[c_far];
        if cn.is_sharp && cf.is_sharp { return 2; }
        let this_size = if cn.is_sharp { 6 } else { 1 + cn.ring_points.len() as i32 };
        let adj_size  = if cf.is_regular || cf.is_sharp { 0 }
                        else { 1 + cf.ring_points.len() as i32 - 6 };
        this_size + adj_size
    }

    // ---- Regular face points ----

    fn assign_regular_face_points<R: Weight>(&self, c_idx: usize, matrix: &mut SparseMatrix<R>) {
        let c = &self.corners[c_idx];
        let c_next = (c_idx + 1) & 0x3;
        let c_opp  = (c_idx + 2) & 0x3;
        let c_prev = (c_idx + 3) & 0x3;

        let row_fp = (5 * c_idx + 3) as i32;
        let row_fm = (5 * c_idx + 4) as i32;

        if c.fp_is_regular {
            let (ci, cw) = matrix.get_row_data_mut(row_fp);
            ci[0] = c_idx  as i32; cw[0] = R::from_f64(4.0 / 9.0);
            ci[1] = c_prev as i32; cw[1] = R::from_f64(2.0 / 9.0);
            ci[2] = c_next as i32; cw[2] = R::from_f64(2.0 / 9.0);
            ci[3] = c_opp  as i32; cw[3] = R::from_f64(1.0 / 9.0);
        }
        if c.fm_is_regular {
            let (ci, cw) = matrix.get_row_data_mut(row_fm);
            ci[0] = c_idx  as i32; cw[0] = R::from_f64(4.0 / 9.0);
            ci[1] = c_prev as i32; cw[1] = R::from_f64(2.0 / 9.0);
            ci[2] = c_next as i32; cw[2] = R::from_f64(2.0 / 9.0);
            ci[3] = c_opp  as i32; cw[3] = R::from_f64(1.0 / 9.0);
        }
    }

    // ---- Irregular face-point helper ----

    /// Compute one irregular face point into `row_f_near`.
    ///
    /// Reads P, eNear, eFar rows from the matrix (already computed), combines
    /// them with cosine weights, adds the R-term ring corrections, and writes
    /// the result as sparse entries.
    fn compute_irregular_face_point<R: Weight>(
        &self,
        c_near: usize,
        edge_in_near_ring: i32,
        c_far: usize,
        row_p:      i32,
        row_e_near: i32,
        row_e_far:  i32,
        row_f_near: i32,
        sign: f64,
        row_weights: &mut [f64],
        col_mask: &mut [i32],
        matrix: &mut SparseMatrix<R>,
    ) {
        let cn = &self.corners[c_near];
        let cf = &self.corners[c_far];

        let cos_near = cn.cos_face_angle;
        let cos_far  = cf.cos_face_angle;

        let p_coeff      =                          cos_far  / 3.0;
        let e_near_coeff = (3.0 - 2.0 * cos_near - cos_far) / 3.0;
        let e_far_coeff  =        2.0 * cos_near            / 3.0;

        let full_size = self.num_source_points as usize;
        for v in row_weights[..full_size].iter_mut() { *v = 0.0; }
        for v in col_mask[..full_size].iter_mut()    { *v = 0; }

        // Accumulate P, eNear, eFar into the full row (as f64):
        for (ri, coeff) in &[
            (row_p,      p_coeff),
            (row_e_near, e_near_coeff),
            (row_e_far,  e_far_coeff),
        ] {
            let idx: Vec<i32> = matrix.get_row_columns(*ri).to_vec();
            let wgt: Vec<R>   = matrix.get_row_elements(*ri).to_vec();
            for (i, &col_i) in idx.iter().enumerate() {
                let col = col_i as usize;
                row_weights[col] += coeff * wgt[i].into_f64();
                col_mask[col] = 1 + col as i32;
            }
        }

        // R-term: ring points around the interior edge:
        let valence = cn.valence as usize;
        let ie  = edge_in_near_ring as usize;
        let ip  = (ie + valence - 1) % valence;
        let inx = (ie + 1) % valence;

        let rp = &cn.ring_points;
        let r0 = rp[2 * ip]     as usize;
        let r1 = rp[2 * ip + 1] as usize;
        let r2 = rp[2 * ie + 1] as usize;
        let r3 = rp[2 * inx]    as usize;

        row_weights[r0] += -sign /  9.0;
        row_weights[r1] += -sign / 18.0;
        row_weights[r2] +=  sign / 18.0;
        row_weights[r3] +=  sign /  9.0;

        for &col in &[r0, r1, r2, r3] {
            col_mask[col] = 1 + col as i32;
        }

        // Collect into the output row:
        let f_size = matrix.get_row_size(row_f_near) as usize;
        let mut nw = 0usize;
        {
            let (f_ci, f_cw) = matrix.get_row_data_mut(row_f_near);
            for i in 0..full_size {
                if col_mask[i] != 0 {
                    f_ci[nw] = col_mask[i] - 1;
                    f_cw[nw] = R::from_f64(row_weights[i]);
                    nw += 1;
                }
            }
            // Pad with zeros if val-2 duplicates reduced the real count:
            while nw < f_size {
                f_ci[nw] = c_near as i32;
                f_cw[nw] = R::zero();
                nw += 1;
            }
        }
    }

    fn compute_irregular_face_points<R: Weight>(
        &self,
        c_idx: usize,
        matrix: &mut SparseMatrix<R>,
        row_weights: &mut [f64],
        col_mask: &mut [i32],
    ) {
        let c_next = (c_idx + 1) & 0x3;
        let c_prev = (c_idx + 3) & 0x3;

        let row_ep_prev = (5 * c_prev  + 1) as i32;
        let row_em      = (5 * c_idx   + 2) as i32;
        let row_p       = (5 * c_idx   + 0) as i32;
        let row_ep      = (5 * c_idx   + 1) as i32;
        let row_em_next = (5 * c_next  + 2) as i32;
        let row_fp      = (5 * c_idx   + 3) as i32;
        let row_fm      = (5 * c_idx   + 4) as i32;

        let (fp_is_reg, fm_is_reg, fp_is_copied, fm_is_copied, face_in_ring, valence) = {
            let c = &self.corners[c_idx];
            (c.fp_is_regular, c.fm_is_regular, c.fp_is_copied, c.fm_is_copied,
             c.face_in_ring, c.valence)
        };

        if !fp_is_reg && !fp_is_copied {
            let ie = face_in_ring;
            self.compute_irregular_face_point(
                c_idx, ie, c_next,
                row_p, row_ep, row_em_next, row_fp,
                1.0, row_weights, col_mask, matrix);
        }
        if !fm_is_reg && !fm_is_copied {
            let ie = (face_in_ring + 1) % valence;
            self.compute_irregular_face_point(
                c_idx, ie, c_prev,
                row_p, row_em, row_ep_prev, row_fm,
                -1.0, row_weights, col_mask, matrix);
        }

        if fp_is_copied { copy_matrix_row(matrix, row_fp, row_fm); }
        if fm_is_copied { copy_matrix_row(matrix, row_fm, row_fp); }
    }
}

// ---------------------------------------------------------------------------
// BSplineConverter — 16-point B-spline from source patch
// ---------------------------------------------------------------------------

struct BSplineConverter {
    source_patch:      SourcePatch,
    gregory_converter: GregoryConverter,
}

impl BSplineConverter {
    fn new(source_patch: &SourcePatch) -> Self {
        let gc = GregoryConverter::new(source_patch);
        Self {
            source_patch:      source_patch.clone(),
            gregory_converter: gc,
        }
    }

    fn convert<R: Weight>(&self, matrix: &mut SparseMatrix<R>) {
        if self.gregory_converter.is_isolated_interior {
            self.convert_irregular_corner(
                self.gregory_converter.isolated_corner,
                matrix);
        } else {
            let mut greg: SparseMatrix<R> = SparseMatrix::new();
            self.gregory_converter.convert(&mut greg);
            self.convert_from_gregory(&greg, matrix);
        }
    }

    fn convert_from_gregory<R: Weight>(&self, g: &SparseMatrix<R>, b: &mut SparseMatrix<R>) {
        // Change-of-basis weight constants from C++ reference:
        let wc: [f64; 9] = [49.0,-42.0,-42.0, 36.0,-14.0,-14.0, 12.0, 12.0,  4.0];
        let wb: [f64; 6] = [-14.0, 12.0,  7.0, -6.0,  4.0, -2.0];
        let wi: [f64; 4] = [  4.0, -2.0, -2.0,  1.0];

        // Gregory → BSpline index tables (from C++ pIndices/epIndices/emIndices/fIndices):
        let p_idx: [[i32; 9]; 4] = [
            [  3,  1,  2,  0,  8, 18,  7, 16, 13 ],
            [  8,  6,  7,  5,  3, 13, 12,  1, 18 ],
            [ 13, 11, 12, 10, 18,  8, 17,  6,  3 ],
            [ 18, 16, 17, 15, 13,  3,  2, 11,  8 ],
        ];
        let ep_idx: [[i32; 6]; 4] = [
            [  3,  1,  8,  7, 18, 13 ],
            [  8,  6, 13, 12,  3, 18 ],
            [ 13, 11, 18, 17,  8,  3 ],
            [ 18, 16,  3,  2, 13,  8 ],
        ];
        let em_idx: [[i32; 6]; 4] = [
            [  3,  2, 18, 16,  8, 13 ],
            [  8,  7,  3,  1, 13, 18 ],
            [ 13, 12,  8,  6, 18,  3 ],
            [ 18, 17, 13, 11,  3,  8 ],
        ];
        let f_idx: [[i32; 4]; 4] = [
            [  3,  8, 18, 13 ],
            [  8, 13,  3, 18 ],
            [ 13, 18,  8,  3 ],
            [ 18,  3, 13,  8 ],
        ];

        init_full_matrix(b, 16, g.get_num_columns());

        Self::combine_rows(b,  0, g, &p_idx[0],  &wc);
        Self::combine_rows(b,  1, g, &ep_idx[0], &wb);
        Self::combine_rows(b,  2, g, &em_idx[1], &wb);
        Self::combine_rows(b,  3, g, &p_idx[1],  &wc);

        Self::combine_rows(b,  4, g, &em_idx[0], &wb);
        Self::combine_rows(b,  5, g, &f_idx[0],  &wi);
        Self::combine_rows(b,  6, g, &f_idx[1],  &wi);
        Self::combine_rows(b,  7, g, &ep_idx[1], &wb);

        Self::combine_rows(b,  8, g, &ep_idx[3], &wb);
        Self::combine_rows(b,  9, g, &f_idx[3],  &wi);
        Self::combine_rows(b, 10, g, &f_idx[2],  &wi);
        Self::combine_rows(b, 11, g, &em_idx[2], &wb);

        Self::combine_rows(b, 12, g, &p_idx[3],  &wc);
        Self::combine_rows(b, 13, g, &em_idx[3], &wb);
        Self::combine_rows(b, 14, g, &ep_idx[2], &wb);
        Self::combine_rows(b, 15, g, &p_idx[2],  &wc);
    }

    /// Combine `src_rows` of `g` (f64-weighted by `weights`) into full dst row.
    fn combine_rows<R: Weight>(
        b: &mut SparseMatrix<R>,
        dst_row: i32,
        g: &SparseMatrix<R>,
        src_rows: &[i32],
        weights: &[f64],
    ) {
        let n_cols = b.get_num_columns() as usize;
        {
            let dst = b.get_row_elements_mut(dst_row);
            for v in dst.iter_mut() { *v = R::zero(); }
        }
        for (k, &ri) in src_rows.iter().enumerate() {
            let s = weights[k];
            let src_ci: Vec<i32> = g.get_row_columns(ri).to_vec();
            let src_ew: Vec<R>   = g.get_row_elements(ri).to_vec();
            let dst = b.get_row_elements_mut(dst_row);
            for j in 0..src_ci.len() {
                let col = src_ci[j] as usize;
                if col < n_cols {
                    dst[col] += R::from_f64(s * src_ew[j].into_f64());
                }
            }
        }
    }

    fn build_irregular_corner_matrix<R: Weight>(
        valence: i32,
        num_source_points: i32,
        x_rows: &[i32],
        matrix: &mut SparseMatrix<R>,
    ) {
        let ring_plus_corner = 1 + 2 * valence;
        let num_elements = 7 * ring_plus_corner + 11;

        let mut row_sizes = [1i32; 16];
        row_sizes[x_rows[0] as usize] = ring_plus_corner;
        row_sizes[x_rows[1] as usize] = ring_plus_corner;
        row_sizes[x_rows[2] as usize] = ring_plus_corner;
        row_sizes[x_rows[3] as usize] = ring_plus_corner;
        row_sizes[x_rows[4] as usize] = ring_plus_corner;
        row_sizes[x_rows[5] as usize] = ring_plus_corner + 1;
        row_sizes[x_rows[6] as usize] = ring_plus_corner + 1;

        matrix.resize(16, num_source_points, num_elements);
        for i in 0..16i32 {
            matrix.set_row_size(i, row_sizes[i as usize]);
            let elems = matrix.get_row_elements_mut(i);
            if row_sizes[i as usize] == 1 {
                elems[0] = R::one();
            } else {
                for v in elems.iter_mut() { *v = R::zero(); }
            }
        }
    }

    fn convert_irregular_corner<R: Weight>(
        &self, irregular_corner: i32, matrix: &mut SparseMatrix<R>,
    ) {
        let sp = &self.source_patch;
        let corner = &sp.corners[irregular_corner as usize];
        let valence      = corner.num_faces as i32;
        let face_in_ring = corner.patch_face as i32;
        let rpc = (1 + 2 * valence) as usize; // ring_plus_corner

        // Compute limit point weights P, Ep, Em (as f64):
        let mut lw = vec![0.0f64; 3 * rpc];
        let (w_p, rest)  = lw.split_at_mut(rpc);
        let (w_ep, w_em) = rest.split_at_mut(rpc);
        compute_interior_point_weights(valence, face_in_ring, w_p, Some(w_ep), Some(w_em));

        // Row layout for the 7 X-points per corner orientation:
        let x_rows_all: [[i32; 7]; 4] = [
            [  0,  1,  4,  2,  8,  3, 12 ],
            [  3,  7,  2, 11,  1, 15,  0 ],
            [ 15, 14, 11, 13,  7, 12,  3 ],
            [ 12,  8, 13,  4, 14,  0, 15 ],
        ];
        let x_rows = &x_rows_all[irregular_corner as usize];

        let num_source_points = sp.get_num_source_points();
        Self::build_irregular_corner_matrix::<R>(valence, num_source_points, x_rows, matrix);

        // Ring indices for source points participating in X-formulae:
        let fp1 = (face_in_ring + 1) % valence;
        let fp2 = (face_in_ring + 2) % valence;
        let fm1 = (face_in_ring + valence - 1) % valence;

        let p0  = 0usize;
        let p1  = (1 + 2 * face_in_ring)  as usize;
        let p2  = (1 + 2 * face_in_ring + 1) as usize;
        let p3  = (1 + 2 * fp1)           as usize;
        let p15 = (1 + 2 * fp1 + 1)       as usize;
        let p4  = (1 + 2 * fp2)           as usize;
        let p6  = (1 + 2 * fm1)           as usize;
        let p7  = (1 + 2 * fm1 + 1)       as usize;
        // P8 and P14 indices used as sentinel (extra slot rpc):

        // Accumulate X[] coefficient arrays (as f64):
        let extra = rpc + 1; // one extra slot for X5/X6 trailing entry
        let mut wx: Vec<Vec<f64>> = (0..7).map(|_| vec![0.0f64; extra]).collect();

        // X1 = 1/3*(36Ep - (16P0+8P1+2P2+4P3+P6+2P7))
        wx[1][p0]=16.0; wx[1][p1]=8.0; wx[1][p2]=2.0;
        wx[1][p3]=4.0;  wx[1][p6]=1.0; wx[1][p7]=2.0;

        // X2 = 1/3*(36Em - (16P0+8P3+2P2+4P1+P4+2P15))
        wx[2][p0]=16.0; wx[2][p3]=8.0; wx[2][p2]=2.0;
        wx[2][p1]=4.0;  wx[2][p4]=1.0; wx[2][p15]=2.0;

        // X3 = 1/3*(-18Ep + (8P0+4P1+P2+2P3+2P6+4P7))
        wx[3][p0]=8.0; wx[3][p1]=4.0; wx[3][p2]=1.0;
        wx[3][p3]=2.0; wx[3][p6]=2.0; wx[3][p7]=4.0;

        // X4 = 1/3*(-18Em + (8P0+4P3+P2+2P1+2P4+4P15))
        wx[4][p0]=8.0; wx[4][p3]=4.0; wx[4][p2]=1.0;
        wx[4][p1]=2.0; wx[4][p4]=2.0; wx[4][p15]=4.0;

        // X5 extras: -P6 + P8  (P8 in extra slot rpc)
        wx[5][p6] = -1.0;
        wx[5][rpc] =  1.0;

        // X6 extras: -P4 + P14  (P14 in extra slot rpc)
        wx[6][p4]  = -1.0;
        wx[6][rpc] =  1.0;

        // X0 extras:
        wx[0][p0]=16.0; wx[0][p1]=4.0; wx[0][p2]=1.0; wx[0][p3]=4.0;

        // Combine ring weights:
        let one_third = 1.0 / 3.0;
        for i in 0..rpc {
            let x1 = (36.0 * w_ep[i] - wx[1][i]) * one_third;
            let x2 = (36.0 * w_em[i] - wx[2][i]) * one_third;
            let x3 = -w_ep[i] * 6.0 + wx[3][i] * one_third;
            let x4 = -w_em[i] * 6.0 + wx[4][i] * one_third;
            let x5 = wx[5][i] + x1;
            let x6 = wx[6][i] + x2;
            let x0 = w_p[i] * 36.0 - wx[0][i] - (x2 + x1) * 4.0 - (x3 + x4);
            wx[0][i] = x0; wx[1][i] = x1; wx[2][i] = x2;
            wx[3][i] = x3; wx[4][i] = x4; wx[5][i] = x5; wx[6][i] = x6;
        }

        // Gather ring point indices:
        let mut ring_points = vec![0i32; rpc];
        ring_points[0] = irregular_corner;
        sp.get_corner_ring_points(irregular_corner, &mut ring_points[1..]);

        // Identify P8..P15:
        let mut p_points = [0i32; 16];
        let p_next_start = ring_points[p7] + 1;
        for i in 8..16usize {
            let nx = p_next_start + (i as i32 - 8);
            p_points[i] = if nx < num_source_points { nx }
                          else { nx - num_source_points + 4 };
        }

        // Write weights and column indices for X[] rows:
        for xi in 0..7usize {
            let row = x_rows[xi];
            {
                let elems = matrix.get_row_elements_mut(row);
                for i in 0..rpc { elems[i] = R::from_f64(wx[xi][i]); }
                if xi >= 5 { elems[rpc] = R::from_f64(wx[xi][rpc]); }
            }
            {
                let cols = matrix.get_row_columns_mut(row);
                for i in 0..rpc { cols[i] = ring_points[i]; }
            }
        }
        // X5 and X6 trailing column indices:
        { let c = matrix.get_row_columns_mut(x_rows[5]); c[rpc] = p_points[8];  }
        { let c = matrix.get_row_columns_mut(x_rows[6]); c[rpc] = p_points[14]; }

        // Fixed identity rows for four interior source points:
        { let c = matrix.get_row_columns_mut(5);  c[0] = 0; }
        { let c = matrix.get_row_columns_mut(6);  c[0] = 1; }
        { let c = matrix.get_row_columns_mut(9);  c[0] = 3; }
        { let c = matrix.get_row_columns_mut(10); c[0] = 2; }

        // Exterior P9..P13 — rows from lookup table:
        let ext_rows_all: [[i32; 5]; 4] = [
            [  7, 11, 15, 14, 13 ],
            [ 14, 13, 12,  8,  4 ],
            [  8,  4,  0,  1,  2 ],
            [  1,  2,  3,  7, 11 ],
        ];
        let ext_rows = &ext_rows_all[irregular_corner as usize];
        for (k, &er) in ext_rows.iter().enumerate() {
            let c = matrix.get_row_columns_mut(er);
            c[0] = p_points[9 + k];
        }
    }
}

// ---------------------------------------------------------------------------
// LinearConverter — 4-point bilinear from source patch
// ---------------------------------------------------------------------------

struct LinearConverter {
    source_patch: SourcePatch,
}

impl LinearConverter {
    fn new(source_patch: &SourcePatch) -> Self {
        Self { source_patch: source_patch.clone() }
    }

    fn convert<R: Weight>(&self, matrix: &mut SparseMatrix<R>) {
        let sp = &self.source_patch;
        let max_ring = sp.max_ring_size as usize;
        let mut index_buf  = vec![0i32;   1 + max_ring];
        let mut weight_buf = vec![0.0f64; 1 + max_ring];

        let num_elements = 4 * (1 + max_ring as i32);
        matrix.resize(4, sp.get_num_source_points(), num_elements);

        let mut has_val2 = false;

        for c_idx in 0..4usize {
            let c = &sp.corners[c_idx];

            if c.sharp {
                matrix.set_row_size(c_idx as i32, 1);
                let (ci, cw) = matrix.get_row_data_mut(c_idx as i32);
                ci[0] = c_idx as i32; cw[0] = R::one();
                continue;
            }

            let ring_size = sp.get_corner_ring_size(c_idx) as usize;

            if c.boundary {
                matrix.set_row_size(c_idx as i32, 3);
            } else {
                matrix.set_row_size(c_idx as i32, (1 + ring_size) as i32);
            }

            index_buf[0] = c_idx as i32;
            sp.get_corner_ring_points(c_idx as i32, &mut index_buf[1..]);

            if c.boundary {
                compute_boundary_point_weights(
                    1 + c.num_faces as i32,
                    c.patch_face as i32,
                    &mut weight_buf,
                    None, None,
                );
                let (ci, cw) = matrix.get_row_data_mut(c_idx as i32);
                ci[0] = index_buf[0];         cw[0] = R::from_f64(weight_buf[0]);
                ci[1] = index_buf[1];         cw[1] = R::from_f64(weight_buf[1]);
                ci[2] = index_buf[ring_size]; cw[2] = R::from_f64(weight_buf[ring_size]);
            } else {
                compute_interior_point_weights(
                    c.num_faces as i32,
                    c.patch_face as i32,
                    &mut weight_buf,
                    None, None,
                );
                let n = 1 + ring_size;
                let (ci, cw) = matrix.get_row_data_mut(c_idx as i32);
                ci[..n].copy_from_slice(&index_buf[..n]);
                for i in 0..n { cw[i] = R::from_f64(weight_buf[i]); }
            }

            has_val2 |= c.val2_interior;
        }

        if has_val2 {
            remove_valence2_duplicates(matrix);
        }
    }
}

// ---------------------------------------------------------------------------
// CatmarkPatchBuilder — public API
// ---------------------------------------------------------------------------

/// Catmull-Clark patch builder.
pub struct CatmarkPatchBuilder<'r> {
    pub base: super::patch_builder::PatchBuilder<'r>,
}

impl<'r> CatmarkPatchBuilder<'r> {
    /// Construct a new Catmark patch builder.
    pub fn new(refiner: &'r TopologyRefiner, options: PatchBuilderOptions) -> Self {
        let mut base = super::patch_builder::PatchBuilder::create(refiner, options);

        let reg_type = match options.reg_basis {
            BasisType::Gregory => PatchType::GregoryBasis,
            BasisType::Linear  => PatchType::Quads,
            BasisType::Bezier  => PatchType::Regular,
            _                  => PatchType::Regular,
        };
        let irreg_type = if options.irreg_basis == BasisType::Unspecified {
            reg_type
        } else {
            match options.irreg_basis {
                BasisType::Linear  => PatchType::Quads,
                BasisType::Regular => PatchType::Regular,
                _                  => PatchType::GregoryBasis,
            }
        };

        base.reg_patch_type    = reg_type;
        base.irreg_patch_type  = irreg_type;
        base.native_patch_type = PatchType::Regular;
        base.linear_patch_type = PatchType::Quads;

        Self { base }
    }

    /// Map a basis type to the corresponding Catmark patch type.
    pub fn patch_type_from_basis(&self, basis: BasisType) -> PatchType {
        match basis {
            BasisType::Regular => PatchType::Regular,
            BasisType::Gregory => PatchType::GregoryBasis,
            BasisType::Linear  => PatchType::Quads,
            _                  => PatchType::Regular,
        }
    }

    /// Convert a source patch to f32 matrix of the requested type.
    pub fn convert_to_patch_type_f32(
        &self,
        source_patch: &SourcePatch,
        patch_type:   PatchType,
        matrix:       &mut SparseMatrix<f32>,
    ) -> i32 {
        convert_source_patch(source_patch, patch_type, matrix)
    }

    /// Convert a source patch to f64 matrix of the requested type.
    pub fn convert_to_patch_type_f64(
        &self,
        source_patch: &SourcePatch,
        patch_type:   PatchType,
        matrix:       &mut SparseMatrix<f64>,
    ) -> i32 {
        convert_source_patch(source_patch, patch_type, matrix)
    }

    // -- Delegated face queries --

    pub fn is_face_a_patch(&self, level: i32, face: Index) -> bool {
        self.base.is_face_a_patch(level, face)
    }
    pub fn is_face_a_leaf(&self, level: i32, face: Index) -> bool {
        self.base.is_face_a_leaf(level, face)
    }
    pub fn is_patch_regular(&self, level: i32, face: Index, fvc: i32) -> bool {
        self.base.is_patch_regular(level, face, fvc)
    }
    pub fn get_regular_patch_boundary_mask(&self, level: i32, face: Index, fvc: i32) -> i32 {
        self.base.get_regular_patch_boundary_mask(level, face, fvc)
    }
}

/// Generic dispatch to GregoryConverter, BSplineConverter or LinearConverter.
fn convert_source_patch<R: Weight>(
    source_patch: &SourcePatch,
    patch_type:   PatchType,
    matrix:       &mut SparseMatrix<R>,
) -> i32 {
    match patch_type {
        PatchType::GregoryBasis => {
            GregoryConverter::new(source_patch).convert(matrix);
        }
        PatchType::Regular => {
            BSplineConverter::new(source_patch).convert(matrix);
        }
        PatchType::Quads => {
            LinearConverter::new(source_patch).convert(matrix);
        }
        _ => {}
    }
    matrix.get_num_rows()
}

/// Standalone entry point called from external patch builders.
pub fn convert_catmark(
    source: &SourcePatch, patch_type: PatchType, matrix: &mut SparseMatrix<f32>,
) -> i32 {
    convert_source_patch(source, patch_type, matrix)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::{Options, types::SchemeType};
    use super::super::topology_refiner::TopologyRefiner;
    use super::super::patch_builder::{PatchBuilderOptions, BasisType};

    /// Build a fully-regular all-interior source patch (valence-4 everywhere).
    fn make_regular_source_patch() -> SourcePatch {
        let mut sp = SourcePatch::new();
        for i in 0..4usize {
            sp.corners[i].num_faces  = 4;
            sp.corners[i].patch_face = i as crate::vtr::types::LocalIndex;
            sp.corners[i].boundary   = false;
            sp.corners[i].sharp      = false;
            sp.corners[i].dart       = false;
        }
        sp.finalize(4);
        sp
    }

    #[test]
    fn ef_table_valence3() {
        let coeff = compute_ef_coefficient(3);
        assert!((coeff - 8.1281572906372312e-01).abs() < 1e-12);
    }

    #[test]
    fn ef_formula_valence30() {
        let coeff = compute_ef_coefficient(30);
        assert!(coeff > 0.0 && coeff < 1.0, "coeff = {}", coeff);
    }

    #[test]
    fn interior_weights_sum_to_one() {
        let valence = 4;
        let ww = (1 + 2 * valence) as usize;
        let mut pw = vec![0.0f64; ww];
        compute_interior_point_weights(valence, 0, &mut pw, None, None);
        let sum: f64 = pw.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12, "sum = {}", sum);
    }

    #[test]
    fn interior_weights_irregular_valence() {
        // Valence 6 — sum should still be 1.0:
        let valence = 6;
        let ww = (1 + 2 * valence) as usize;
        let mut pw = vec![0.0f64; ww];
        compute_interior_point_weights(valence, 2, &mut pw, None, None);
        let sum: f64 = pw.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12, "sum = {}", sum);
    }

    #[test]
    fn boundary_weights_sum_to_one() {
        // valence=3 → numFaces=2, boundary:
        let valence = 3;
        let ww = (2 * valence) as usize;
        let mut pw = vec![0.0f64; ww];
        compute_boundary_point_weights(valence, 0, &mut pw, None, None);
        let sum: f64 = pw.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12, "sum = {}", sum);
    }

    #[test]
    fn catmark_patch_types_default() {
        let refiner = TopologyRefiner::new(SchemeType::Catmark, Options::default());
        let opts = PatchBuilderOptions::default();
        let pb = CatmarkPatchBuilder::new(&refiner, opts);
        assert_eq!(pb.base.get_regular_patch_type(),  PatchType::Regular);
        assert_eq!(pb.base.get_irreg_patch_type(),    PatchType::Regular);
    }

    #[test]
    fn catmark_patch_types_gregory() {
        let refiner = TopologyRefiner::new(SchemeType::Catmark, Options::default());
        let mut opts = PatchBuilderOptions::default();
        opts.reg_basis   = BasisType::Gregory;
        opts.irreg_basis = BasisType::Gregory;
        let pb = CatmarkPatchBuilder::new(&refiner, opts);
        assert_eq!(pb.base.get_regular_patch_type(),  PatchType::GregoryBasis);
        assert_eq!(pb.base.get_irreg_patch_type(),    PatchType::GregoryBasis);
    }

    #[test]
    fn gregory_converter_regular_patch() {
        let sp = make_regular_source_patch();
        let gc = GregoryConverter::new(&sp);
        let mut mat: SparseMatrix<f32> = SparseMatrix::new();
        gc.convert(&mut mat);
        assert_eq!(mat.get_num_rows(), 20);
        assert!(mat.get_num_elements() > 0);
    }

    #[test]
    fn linear_converter_regular_patch_row_sums() {
        let sp = make_regular_source_patch();
        let lc = LinearConverter::new(&sp);
        let mut mat: SparseMatrix<f32> = SparseMatrix::new();
        lc.convert(&mut mat);
        assert_eq!(mat.get_num_rows(), 4);
        for row in 0..4i32 {
            let w = mat.get_row_elements(row);
            let s: f32 = w.iter().sum();
            assert!((s - 1.0).abs() < 1e-6, "row {} sum = {}", row, s);
        }
    }

    #[test]
    fn bspline_converter_regular_patch() {
        let sp = make_regular_source_patch();
        let bc = BSplineConverter::new(&sp);
        let mut mat: SparseMatrix<f64> = SparseMatrix::new();
        bc.convert(&mut mat);
        assert_eq!(mat.get_num_rows(), 16);
        assert!(mat.get_num_elements() > 0);
    }

    #[test]
    fn convert_catmark_gregory_f32() {
        let sp = make_regular_source_patch();
        let mut mat: SparseMatrix<f32> = SparseMatrix::new();
        let rows = convert_source_patch(&sp, PatchType::GregoryBasis, &mut mat);
        assert_eq!(rows, 20);
    }

    #[test]
    fn convert_catmark_quads_f32() {
        let sp = make_regular_source_patch();
        let mut mat: SparseMatrix<f32> = SparseMatrix::new();
        let rows = convert_source_patch(&sp, PatchType::Quads, &mut mat);
        assert_eq!(rows, 4);
    }

    #[test]
    fn convert_catmark_regular_f64() {
        let sp = make_regular_source_patch();
        let mut mat: SparseMatrix<f64> = SparseMatrix::new();
        let rows = convert_source_patch(&sp, PatchType::Regular, &mut mat);
        assert_eq!(rows, 16);
    }
}
