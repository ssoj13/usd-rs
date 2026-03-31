// Copyright 2018 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/loopPatchBuilder.h/.cpp

//! Loop subdivision patch builder.
//!
//! Builds regular Box-spline triangle patches and Gregory triangle patches
//! for Loop subdivision.  Mirrors C++ `Far::LoopPatchBuilder`.

use std::f64::consts::PI;

use super::patch_builder::{BasisType, PatchBuilder, PatchBuilderOptions, SourcePatch};
use super::patch_descriptor::PatchType;
use super::sparse_matrix::SparseMatrix;
use super::topology_refiner::TopologyRefiner;
use crate::vtr::types::Index;

// ---------------------------------------------------------------------------
// BasisType → PatchType mapping for Loop (mirrors patchTypeFromBasisArray[])
// ---------------------------------------------------------------------------

fn loop_patch_type_from_basis(basis: BasisType) -> PatchType {
    match basis {
        BasisType::Regular | BasisType::Unspecified => PatchType::Loop,
        BasisType::Gregory => PatchType::GregoryTriangle,
        BasisType::Linear  => PatchType::Triangles,
        _                  => PatchType::NonPatch,
    }
}

// ---------------------------------------------------------------------------
// Thin wrapper: delegate to SparseMatrix::assign_row to avoid the E0499
// double-mutable-borrow that arises from calling get_row_columns_mut and
// get_row_elements_mut in the same scope.
// ---------------------------------------------------------------------------

#[inline]
fn assign_row<R: Copy + Default>(
    m: &mut SparseMatrix<R>, row: i32, idx: &[i32], w: &[R],
) {
    m.assign_row(row, idx, w);
}

// ---------------------------------------------------------------------------
// LoopPatchBuilder
// ---------------------------------------------------------------------------

/// Loop subdivision patch builder.
///
/// Wraps `PatchBuilder` with Loop-specific patch type assignments and
/// change-of-basis conversion matrices for the three supported target types:
///   - `Loop` (12-point quartic Box-spline triangle)
///   - `GregoryTriangle` (18-point quartic Gregory triangle)
///   - `Triangles` (3-point linear triangle)
pub struct LoopPatchBuilder<'r> {
    pub base: PatchBuilder<'r>,
}

impl<'r> LoopPatchBuilder<'r> {
    /// Construct a new Loop patch builder.
    pub fn new(refiner: &'r TopologyRefiner, options: PatchBuilderOptions) -> Self {
        let mut base = PatchBuilder::create(refiner, options);

        let reg_type = loop_patch_type_from_basis(options.reg_basis);
        let irreg_type = if options.irreg_basis == BasisType::Unspecified {
            reg_type
        } else {
            loop_patch_type_from_basis(options.irreg_basis)
        };

        base.reg_patch_type    = reg_type;
        base.irreg_patch_type  = irreg_type;
        base.native_patch_type = PatchType::Loop;
        base.linear_patch_type = PatchType::Triangles;

        Self { base }
    }

    pub fn patch_type_from_basis(&self, basis: BasisType) -> PatchType {
        loop_patch_type_from_basis(basis)
    }

    pub fn convert_to_patch_type_f32(
        &self,
        source_patch: &SourcePatch,
        patch_type:   PatchType,
        matrix:       &mut SparseMatrix<f32>,
    ) -> i32 {
        self.convert_source_patch_f32(source_patch, patch_type, matrix)
    }

    pub fn convert_to_patch_type_f64(
        &self,
        source_patch: &SourcePatch,
        patch_type:   PatchType,
        matrix:       &mut SparseMatrix<f64>,
    ) -> i32 {
        self.convert_source_patch_f64(source_patch, patch_type, matrix)
    }

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

    fn convert_source_patch_f32(
        &self, sp: &SourcePatch, pt: PatchType, m: &mut SparseMatrix<f32>,
    ) -> i32 {
        match pt {
            PatchType::Loop            => { convert_to_loop_f32(sp, m);    m.get_num_rows() }
            PatchType::Triangles       => { convert_to_linear_f32(sp, m);  m.get_num_rows() }
            PatchType::GregoryTriangle => { convert_to_gregory_f32(sp, m); m.get_num_rows() }
            _ => 0,
        }
    }

    fn convert_source_patch_f64(
        &self, sp: &SourcePatch, pt: PatchType, m: &mut SparseMatrix<f64>,
    ) -> i32 {
        match pt {
            PatchType::Loop            => { convert_to_loop_f64(sp, m);    m.get_num_rows() }
            PatchType::Triangles       => { convert_to_linear_f64(sp, m);  m.get_num_rows() }
            PatchType::GregoryTriangle => { convert_to_gregory_f64(sp, m); m.get_num_rows() }
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// LoopLimits — interior and boundary limit-point weights
//
// F2 FIX: Interior limit mask uses the correct formula from
// Sdc::Scheme<SCHEME_LOOP>::assignSmoothLimitMask (loopScheme.h):
//
//   For n == 6 (regular): v_w = 1/2, e_w = 1/12
//   For n != 6 (irregular):
//     beta  = 0.25 * cos(2*PI/n) + 0.375
//     gamma = (0.625 - beta^2) / n
//     e_w   = 1.0 / (n + 3.0 / (8.0 * gamma))
//     v_w   = 1.0 - e_w * n
//
// The old incorrect code used beta = 3/(8n) which is wrong.
// ---------------------------------------------------------------------------

/// Compute interior Loop limit point weights (and optionally tangent points).
/// Mirrors C++ LoopLimits<REAL>::ComputeInteriorPointWeights.
fn loop_limits_interior<R: Copy + Default + num_traits::Float>(
    valence:  usize,
    face_in:  usize,
    pw:       &mut [R],
    ep:       Option<&mut [R]>,
    em:       Option<&mut [R]>,
) {
    let n   = valence as f64;
    let rs  = valence + 1;

    // Correct limit mask (F2 fix)
    let (vw, ew) = if valence == 6 {
        (0.5_f64, 1.0_f64 / 12.0_f64)
    } else {
        let ct    = (2.0 * PI / n).cos();
        let beta  = 0.25 * ct + 0.375;
        let gamma = (0.625 - beta * beta) / n;
        let ew    = 1.0 / (n + 3.0 / (8.0 * gamma));
        (1.0 - ew * n, ew)
    };

    pw[0] = R::from(vw).unwrap_or_default();
    for i in 1..rs { pw[i] = R::from(ew).unwrap_or_default(); }

    if let (Some(epw), Some(emw)) = (ep, em) {
        // tanScale = (3 + 2*cos(2*PI/n)) / (6*n)  from C++ loopPatchBuilder.cpp
        let theta = 2.0 * PI / n;
        let ts    = (3.0 + 2.0 * theta.cos()) / (6.0 * n);

        // t2[i] = pw[i] + t1[i]*ts  where t1[i] = cos((i-1)*theta)
        let mut t2 = vec![0.0_f64; rs];
        t2[0] = vw;
        for i in 1..rs { t2[i] = ew + ((i - 1) as f64 * theta).cos() * ts; }

        // ep = t2 rotated by face_in
        let n1e = face_in;
        let n2e = valence - n1e;
        epw[0] = R::from(t2[0]).unwrap_or_default();
        for i in 0..n1e { epw[1+i] = R::from(t2[1+n2e+i]).unwrap_or_default(); }
        for i in 0..n2e { epw[1+n1e+i] = R::from(t2[1+i]).unwrap_or_default(); }

        // em = t2 rotated by (face_in+1)%n
        let n1m = (face_in + 1) % valence;
        let n2m = valence - n1m;
        emw[0] = R::from(t2[0]).unwrap_or_default();
        for i in 0..n1m { emw[1+i] = R::from(t2[1+n2m+i]).unwrap_or_default(); }
        for i in 0..n2m { emw[1+n1m+i] = R::from(t2[1+i]).unwrap_or_default(); }
    }
}

/// Compute boundary Loop limit point weights.
/// Mirrors C++ LoopLimits<REAL>::ComputeBoundaryPointWeights.
fn loop_limits_boundary<R: Copy + Default + num_traits::Float>(
    valence: usize,
    face_in: usize,
    pw:      &mut [R],
    ep:      Option<&mut [R]>,
    em:      Option<&mut [R]>,
) {
    let rs  = valence + 1;

    // Crease limit: v=2/3, first and last boundary edge endpoints=1/6
    for v in pw.iter_mut() { *v = R::zero(); }
    pw[0]       = R::from(2.0/3.0).unwrap_or_default();
    pw[1]       = R::from(1.0/6.0).unwrap_or_default();
    pw[valence] = R::from(1.0/6.0).unwrap_or_default();

    if let (Some(epw), Some(emw)) = (ep, em) {
        let n  = valence as f64;
        let fa = PI / (n - 1.0);  // face_angle

        // Interior tangent weights (per-edge sine)
        let mut t2w = vec![0.0_f64; rs];
        for i in 1..valence { t2w[i] = (fa * i as f64).sin() / (n - 1.0); }

        // ep
        if face_in == 0 {
            for v in epw.iter_mut() { *v = R::zero(); }
            epw[0] = R::from(2.0/3.0).unwrap_or_default();
            epw[1] = R::from(1.0/3.0).unwrap_or_default();
        } else {
            let a = fa * face_in as f64;
            let (ca, sa) = (a.cos(), a.sin());
            for i in 0..rs { epw[i] = R::from(t2w[i] * sa / 24.0).unwrap_or_default(); }
            epw[0]       = R::from(epw[0].to_f64().unwrap_or(0.0) + 2.0/3.0).unwrap_or_default();
            epw[1]       = R::from(epw[1].to_f64().unwrap_or(0.0) + 1.0/6.0 +  ca/6.0).unwrap_or_default();
            epw[valence] = R::from(epw[valence].to_f64().unwrap_or(0.0) + 1.0/6.0 - ca/6.0).unwrap_or_default();
        }

        // em
        if face_in == valence - 1 {
            for v in emw.iter_mut() { *v = R::zero(); }
            emw[0]       = R::from(2.0/3.0).unwrap_or_default();
            emw[valence] = R::from(1.0/3.0).unwrap_or_default();
        } else {
            let ip = (face_in + 1) % valence;
            let a  = fa * ip as f64;
            let (ca, sa) = (a.cos(), a.sin());
            for i in 0..rs { emw[i] = R::from(t2w[i] * sa / 24.0).unwrap_or_default(); }
            emw[0]       = R::from(emw[0].to_f64().unwrap_or(0.0) + 2.0/3.0).unwrap_or_default();
            emw[1]       = R::from(emw[1].to_f64().unwrap_or(0.0) + 1.0/6.0 +  ca/6.0).unwrap_or_default();
            emw[valence] = R::from(emw[valence].to_f64().unwrap_or(0.0) + 1.0/6.0 - ca/6.0).unwrap_or_default();
        }
    }
}

// ---------------------------------------------------------------------------
// SparseMatrix helpers
// ---------------------------------------------------------------------------

fn init_full_matrix<R: Copy + Default>(m: &mut SparseMatrix<R>, nr: i32, nc: i32) {
    m.resize(nr, nc, nr * nc);
    for row in 0..nr {
        m.set_row_size(row, nc);
        let cols = m.get_row_columns_mut(row);
        for i in 0..nc as usize { cols[i] = i as i32; }
    }
}

fn resize_matrix<R: Copy + Default>(
    m: &mut SparseMatrix<R>, nr: i32, nc: i32, ne: i32, sizes: &[i32],
) {
    m.resize(nr, nc, ne);
    for i in 0..nr { m.set_row_size(i, sizes[i as usize]); }
}

fn add_row_to_full<R>(full: &mut [R], mat: &SparseMatrix<R>, row: i32, s: R)
where R: Copy + Default + std::ops::AddAssign + std::ops::Mul<Output = R>,
{
    let idx = mat.get_row_columns(row);
    let wgt = mat.get_row_elements(row);
    for (&i, &w) in idx.iter().zip(wgt.iter()) { full[i as usize] += s * w; }
}

/// Combine multiple sparse rows into a dense destination row.
fn combine_rows_in_full<R>(
    dst: &mut SparseMatrix<R>, dst_row: i32,
    src: &SparseMatrix<R>, src_rows: &[i32], src_w: &[R],
) where R: Copy + Default + std::ops::AddAssign + std::ops::Mul<Output = R> + PartialEq,
{
    let nc = dst.get_num_columns() as usize;
    let mut full = vec![R::default(); nc];
    let zero = R::default();
    for (&ri, &w) in src_rows.iter().zip(src_w.iter()) {
        if w != zero { add_row_to_full(&mut full, src, ri, w); }
    }
    dst.get_row_elements_mut(dst_row).copy_from_slice(&full);
}

/// Combine two rows into local buffers; return (indices, weights).
fn combine_two_rows<R>(
    mat: &SparseMatrix<R>,
    ra: i32, wa: R, rb: i32, wb_coeff: R,
    n: usize, rbuf: &mut Vec<R>, mbuf: &mut Vec<i32>,
) -> (Vec<i32>, Vec<R>)
where R: Copy + Default + num_traits::Float + std::ops::AddAssign,
{
    rbuf[..n].iter_mut().for_each(|v| *v = R::zero());
    mbuf[..n].iter_mut().for_each(|v| *v = 0);

    for (rows, coeff) in [(ra, wa), (rb, wb_coeff)] {
        let idx = mat.get_row_columns(rows).to_vec();
        let wgt = mat.get_row_elements(rows).to_vec();
        for (&i, &w) in idx.iter().zip(wgt.iter()) {
            rbuf[i as usize] += coeff * w;
            mbuf[i as usize]  = 1 + i;
        }
    }

    let mut oi = Vec::new(); let mut ow = Vec::new();
    for i in 0..n {
        if mbuf[i] != 0 { oi.push(mbuf[i]-1); ow.push(rbuf[i]); }
    }
    (oi, ow)
}

// ---------------------------------------------------------------------------
// Gregory-to-Loop 12×15 conversion matrix
// From C++ gregoryToLoopMatrix[12][15] in loopPatchBuilder.cpp.
// Columns indexed by G_ROW_INDICES.
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const GREGORY_TO_LOOP: [[f32; 15]; 12] = [
    [  8.214411,  7.571190, -7.690082,  2.237840, -1.118922,-16.428828,  0.666666,  0.666666,
                  2.237835,  6.309870,  0.666666, -1.690100, -0.428812, -0.428805,  0.214407 ],
    [ -0.304687,  0.609374,  6.752593,  0.609374, -0.304687,  0.609378, -3.333333, -3.333333,
                  0.609378, -1.247389, -3.333333, -1.247389,  3.276037,  3.276037, -1.638020 ],
    [ -1.118922,  2.237840, -7.690082,  7.571190,  8.214411,  2.237835,  0.666666,  0.666666,
                -16.428828, -1.690100,  0.666666,  6.309870, -0.428805, -0.428812,  0.214407 ],
    [  8.214411,-16.428828,  6.309870, -0.428812,  0.214407,  7.571190,  0.666666,  0.666666,
                 -0.428805, -7.690082,  0.666666, -1.690100,  2.237840,  2.237835, -1.118922 ],
    [ -0.813368,  1.626735, -0.773435, -1.039929,  0.519965,  1.626735,  0.666666,  0.666666,
                 -1.039930, -0.773435,  0.666666,  1.226558, -1.039929, -1.039930,  0.519965 ],
    [  0.519965, -1.039929, -0.773435,  1.626735, -0.813368, -1.039930,  0.666666,  0.666666,
                  1.626735,  1.226558,  0.666666, -0.773435, -1.039930, -1.039929,  0.519965 ],
    [  0.214407, -0.428812,  6.309870,-16.428828,  8.214411, -0.428805,  0.666666,  0.666666,
                  7.571190, -1.690100,  0.666666, -7.690082,  2.237835,  2.237840, -1.118922 ],
    [ -0.304687,  0.609378, -1.247389,  3.276037, -1.638020,  0.609374, -3.333333, -3.333333,
                  3.276037,  6.752593, -3.333333, -1.247389,  0.609374,  0.609378, -0.304687 ],
    [  0.519965, -1.039930,  1.226558, -1.039930,  0.519965, -1.039929,  0.666666,  0.666666,
                 -1.039929, -0.773435,  0.666666, -0.773435,  1.626735,  1.626735, -0.813368 ],
    [ -1.638020,  3.276037, -1.247389,  0.609378, -0.304687,  3.276037, -3.333333, -3.333333,
                  0.609374, -1.247389, -3.333333,  6.752593,  0.609378,  0.609374, -0.304687 ],
    [ -1.118922,  2.237835, -1.690100, -0.428805,  0.214407,  2.237840,  0.666666,  0.666666,
                 -0.428812, -7.690082,  0.666666,  6.309870,  7.571190,-16.428828,  8.214411 ],
    [  0.214407, -0.428805, -1.690100,  2.237835, -1.118922, -0.428812,  0.666666,  0.666666,
                  2.237840,  6.309870,  0.666666, -7.690082,-16.428828,  7.571190,  8.214411 ],
];

// C++: int const gRowIndices[15] = { 0,1,15,7,5, 2,4,8,6, 17,14,16, 11,12, 10 };
const G_ROW_INDICES: [i32; 15] = [0, 1, 15, 7, 5, 2, 4, 8, 6, 17, 14, 16, 11, 12, 10];

// ---------------------------------------------------------------------------
// convertToLinear
// ---------------------------------------------------------------------------

fn convert_to_linear_generic<R>(sp: &SourcePatch, m: &mut SparseMatrix<R>)
where R: Copy + Default + num_traits::Float + std::ops::AddAssign,
{
    let nsrc = sp.get_num_source_points();
    let mut rsz = [0i32; 3];
    let mut ne  = 0i32;
    for ci in 0..3 {
        let c = &sp.corners[ci];
        rsz[ci] = if c.sharp { 1 } else if c.boundary { 3 } else { 1 + sp.get_corner_ring_size(ci) };
        ne += rsz[ci];
    }
    m.resize(3, nsrc, ne);
    for i in 0..3 { m.set_row_size(i as i32, rsz[i]); }

    let mut has_val2 = false;
    for ci in 0..3usize {
        let rs   = sp.get_corner_ring_size(ci) as usize;
        let c    = &sp.corners[ci];
        let mut ibuf = vec![0i32; 1 + rs];
        ibuf[0] = ci as i32;
        sp.get_corner_ring_points(ci as i32, &mut ibuf[1..]);

        if c.sharp {
            assign_row(m, ci as i32, &[ci as i32], &[R::one()]);
        } else if c.boundary {
            let bv = c.num_faces as usize + 1;
            let mut pw = vec![R::zero(); bv + 1];
            loop_limits_boundary::<R>(bv, c.patch_face as usize, &mut pw, None, None);
            assign_row(m, ci as i32,
                &[ibuf[0], ibuf[1], ibuf[bv]],
                &[pw[0],   pw[1],   pw[bv]]);
        } else {
            let nf = c.num_faces as usize;
            let mut pw = vec![R::zero(); 1 + nf];
            loop_limits_interior::<R>(nf, c.patch_face as usize, &mut pw, None, None);
            let sz = 1 + rs;
            assign_row(m, ci as i32, &ibuf[..sz], &pw[..sz]);
        }
        has_val2 |= c.val2_interior;
    }
    if has_val2 { remove_valence2_duplicates(m); }
}

fn convert_to_linear_f32(sp: &SourcePatch, m: &mut SparseMatrix<f32>) { convert_to_linear_generic(sp, m); }
fn convert_to_linear_f64(sp: &SourcePatch, m: &mut SparseMatrix<f64>) { convert_to_linear_generic(sp, m); }

// ---------------------------------------------------------------------------
// convertToLoop
// ---------------------------------------------------------------------------

fn convert_to_loop_f32(sp: &SourcePatch, m: &mut SparseMatrix<f32>) {
    let mut g = SparseMatrix::new();
    convert_to_gregory_f32(sp, &mut g);
    let nc = g.get_num_columns();
    init_full_matrix(m, 12, nc);
    for i in 0..12i32 {
        let rw: Vec<f32> = GREGORY_TO_LOOP[i as usize].to_vec();
        combine_rows_in_full(m, i, &g, &G_ROW_INDICES, &rw);
    }
}

fn convert_to_loop_f64(sp: &SourcePatch, m: &mut SparseMatrix<f64>) {
    let mut g = SparseMatrix::new();
    convert_to_gregory_f64(sp, &mut g);
    let nc = g.get_num_columns();
    init_full_matrix(m, 12, nc);
    for i in 0..12i32 {
        let rw: Vec<f64> = GREGORY_TO_LOOP[i as usize].iter().map(|&v| v as f64).collect();
        combine_rows_in_full(m, i, &g, &G_ROW_INDICES, &rw);
    }
}

// ---------------------------------------------------------------------------
// GregoryTriConverter — Full port of C++ GregoryTriConverter<REAL>
//
// Computes the 18-point quartic Gregory triangle from source patch vertices.
//
// Row layout:
//   [0..4]   = corner 0: P, Ep, Em, Fp, Fm
//   [5..9]   = corner 1: P, Ep, Em, Fp, Fm
//   [10..14] = corner 2: P, Ep, Em, Fp, Fm
//   [15..17] = mid-edge M0, M1, M2 (between corners 01, 12, 20)
// ---------------------------------------------------------------------------

/// Topological info for one corner. Mirrors C++ CornerTopology.
#[derive(Clone, Default)]
struct CornerTopo {
    is_bnd:    bool,
    is_sharp:  bool,
    #[allow(dead_code)]
    is_dart:   bool,
    is_reg:    bool,
    #[allow(dead_code)]
    is_v2int:  bool,
    is_corner: bool,   // numFaces == 1

    ep_bnd:    bool,   // ep is on boundary edge
    em_bnd:    bool,   // em is on boundary edge

    fp_reg:    bool,
    fm_reg:    bool,
    fp_copy:   bool,
    fm_copy:   bool,

    val:       i32,    // valence = numFaces + isBoundary
    nf:        i32,    // numFaces
    fi:        i32,    // faceInRing

    cos_fa:    f64,    // cos(faceAngle)
    ring:      Vec<i32>,
}

struct GtConverter<R> {
    nsrc:      i32,
    max_val:   i32,
    isolated:  bool,
    has_v2:    bool,
    iso_c:     i32,
    iso_val:   i32,
    c:         [CornerTopo; 3],
    _ph:       std::marker::PhantomData<R>,
}

fn default_ctopo() -> [CornerTopo; 3] {
    [CornerTopo::default(), CornerTopo::default(), CornerTopo::default()]
}

impl<R> GtConverter<R>
where R: Copy + Default + num_traits::Float + std::ops::AddAssign,
{
    fn new(sp: &SourcePatch) -> Self {
        let mut g = Self {
            nsrc: 0, max_val: 0, isolated: false, has_v2: false,
            iso_c: -1, iso_val: -1, c: default_ctopo(), _ph: std::marker::PhantomData,
        };
        g.init(sp);
        g
    }

    fn init(&mut self, sp: &SourcePatch) {
        self.nsrc    = sp.get_num_source_points();
        self.max_val = sp.max_valence;

        let mut bc = 0i32; let mut ic = 0i32;
        let mut ico = -1i32; let mut iv = -1i32;
        let mut sc = 0i32; let mut v2c = 0i32;

        for ci in 0..3usize {
            let src = &sp.corners[ci];
            let c   = &mut self.c[ci];

            c.is_bnd   = src.boundary;
            c.is_sharp = src.sharp;
            c.is_dart  = src.dart;
            c.is_corner = src.num_faces == 1;
            c.nf       = src.num_faces as i32;
            c.fi       = src.patch_face as i32;
            c.is_v2int = src.val2_interior;
            c.val      = c.nf + c.is_bnd as i32;

            // Regular: (nf << isBoundary) == 6 and not sharp
            c.is_reg   = ((c.nf << (c.is_bnd as i32)) == 6) && !c.is_sharp;
            c.cos_fa   = if c.is_reg { 0.5 }
                         else if c.is_bnd { (PI / c.nf as f64).cos() }
                         else { (2.0 * PI / c.nf as f64).cos() };

            let rs = sp.get_corner_ring_size(ci) as usize;
            c.ring.resize(rs, 0);
            sp.get_corner_ring_points(ci as i32, &mut c.ring);

            bc += c.is_bnd as i32;
            if !c.is_reg { ic += 1; ico = ci as i32; iv = c.val; }
            sc  += c.is_sharp as i32;
            v2c += c.is_v2int as i32;
        }

        // Second pass: adjacent-corner-dependent flags
        for ci in 0..3usize {
            let cn = (ci+1)%3; let cp = (ci+2)%3;
            self.c[ci].fp_reg  = self.c[ci].is_reg && self.c[cn].is_reg;
            self.c[ci].fm_reg  = self.c[ci].is_reg && self.c[cp].is_reg;
            self.c[ci].fp_copy = false; self.c[ci].fm_copy = false;
            self.c[ci].ep_bnd  = false; self.c[ci].em_bnd  = false;

            if self.c[ci].is_bnd {
                let fi = self.c[ci].fi; let nf = self.c[ci].nf;
                self.c[ci].ep_bnd = fi == 0;
                self.c[ci].em_bnd = fi == nf - 1;
                if nf > 1 {
                    if self.c[ci].ep_bnd { self.c[ci].fp_reg = self.c[ci].fm_reg; self.c[ci].fp_copy = !self.c[ci].fp_reg; }
                    if self.c[ci].em_bnd { self.c[ci].fm_reg = self.c[ci].fp_reg; self.c[ci].fm_copy = !self.c[ci].fm_reg; }
                } else {
                    self.c[ci].fp_reg = true; self.c[ci].fm_reg = true;
                }
            }
        }

        self.isolated = (ic == 1) && (bc == 0) && (iv > 2) && (sc == 0);
        if self.isolated { self.iso_c = ico; self.iso_val = iv; }
        self.has_v2 = v2c > 0;
    }

    // -----------------------------------------------------------------------
    // Sizing
    // -----------------------------------------------------------------------

    fn size_isolated(&self, m: &mut SparseMatrix<R>) {
        let ci = self.iso_c as usize;
        let cp = (ci+1)%3; let cm = (ci+2)%3;
        let rs = 1 + self.iso_val;
        let mut sz = [0i32; 18];

        let s = &mut sz[ci*5..ci*5+5];
        s[0]=rs; s[1]=rs; s[2]=rs; s[3]=3+rs; s[4]=3+rs;
        let s = &mut sz[cp*5..cp*5+5];
        s[0]=7; s[1]=7; s[2]=7; s[3]=5; s[4]=3+rs;
        let s = &mut sz[cm*5..cm*5+5];
        s[0]=7; s[1]=7; s[2]=7; s[3]=3+rs; s[4]=5;
        sz[15+ci]=3+rs; sz[15+cp]=4; sz[15+cm]=3+rs;

        resize_matrix(m, 18, self.nsrc, 9*rs+74, &sz);
    }

    fn size_unisolated(&self, m: &mut SparseMatrix<R>) {
        let mut sz = [0i32; 18];
        let mut ne = 0i32;
        for ci in 0..3usize {
            let c = &self.c[ci]; let cn=(ci+1)%3; let cp=(ci+2)%3;
            let s = &mut sz[ci*5..ci*5+5];
            if c.is_reg {
                if !c.is_bnd { s[0]=7; s[1]=7; s[2]=7; }
                else { s[0]=3; s[1]=if c.ep_bnd{3}else{5}; s[2]=if c.em_bnd{3}else{5}; }
            } else if c.is_sharp { s[0]=1; s[1]=2; s[2]=2; }
            else if !c.is_bnd { let r=1+c.val; s[0]=r; s[1]=r; s[2]=r; }
            else if c.nf > 1 { let r=1+c.val; s[0]=3; s[1]=if c.ep_bnd{3}else{r}; s[2]=if c.em_bnd{3}else{r}; }
            else { s[0]=3; s[1]=3; s[2]=3; }
            ne += s[0]+s[1]+s[2];

            let bfp = 5 - c.ep_bnd as i32 - c.em_bnd as i32;
            s[3] = if !c.fp_reg { self.fp_size(ci, if c.fp_copy{cp}else{cn}) } else { bfp };
            s[4] = if !c.fm_reg { self.fp_size(ci, if c.fm_copy{cn}else{cp}) } else { bfp };
            ne += s[3]+s[4];

            let cn_c = &self.c[cn];
            sz[15+ci] = if c.ep_bnd && cn_c.em_bnd { 2 }
                else if c.is_reg && cn_c.is_reg && (c.ep_bnd == cn_c.em_bnd) { 4 }
                else { self.fp_size(ci, cn) };
            ne += sz[15+ci];
        }
        resize_matrix(m, 18, self.nsrc, ne, &sz);
    }

    fn fp_size(&self, cn: usize, cf: usize) -> i32 {
        let n = &self.c[cn]; let f = &self.c[cf];
        if n.is_sharp && f.is_sharp { return 2; }
        let ns = n.ring.len() as i32 - 3;
        let fs = f.ring.len() as i32 - 3;
        4 + (if ns>0 && !n.is_sharp {ns} else {0})
          + (if fs>0 && !f.is_sharp  {fs} else {0})
    }

    // -----------------------------------------------------------------------
    // Edge points
    // -----------------------------------------------------------------------

    fn assign_reg_edge(&self, ci: usize, m: &mut SparseMatrix<R>) {
        let c    = &self.c[ci];
        let ring = c.ring.clone();
        let b    = (5*ci) as i32;

        if !c.is_bnd {
            let ps  = R::from(1.0/12.0).unwrap_or_default();
            let h   = R::from(0.5_f64).unwrap_or_default();
            let ew  = [7.0f64,5.0,1.0,-1.0,1.0,5.0];
            let es  = R::from(1.0/36.0).unwrap_or_default();
            let fi  = c.fi as usize;

            // P
            let mut ix=[0i32;7]; let mut wx=[R::zero();7];
            ix[0]=ci as i32; wx[0]=h;
            for i in 0..6 { ix[1+i]=ring[i]; wx[1+i]=ps; }
            assign_row(m, b, &ix, &wx);

            // Ep
            let mut ix=[0i32;7]; let mut wx=[R::zero();7];
            ix[0]=ci as i32; wx[0]=h;
            for i in 0..6 { ix[1+i]=ring[(fi+i)%6]; wx[1+i]=es*R::from(ew[i]).unwrap_or_default(); }
            assign_row(m, b+1, &ix, &wx);

            // Em
            let fim=(fi+1)%6;
            let mut ix=[0i32;7]; let mut wx=[R::zero();7];
            ix[0]=ci as i32; wx[0]=h;
            for i in 0..6 { ix[1+i]=ring[(fim+i)%6]; wx[1+i]=es*R::from(ew[i]).unwrap_or_default(); }
            assign_row(m, b+2, &ix, &wx);
        } else {
            let t3=R::from(1.0/3.0).unwrap_or_default();
            let t23=R::from(2.0/3.0).unwrap_or_default();
            let s6=R::from(1.0/6.0).unwrap_or_default();
            let h=R::from(0.5_f64).unwrap_or_default();
            let z=R::zero();

            assign_row(m, b, &[ci as i32, ring[0], ring[3]], &[t23, s6, s6]);

            if c.ep_bnd {
                assign_row(m, b+1, &[ci as i32, ring[0], ring[3]], &[t23, t3, z]);
            } else {
                let eo=if c.em_bnd{3}else{0}; let ez=if c.em_bnd{0}else{3};
                assign_row(m, b+1, &[ci as i32, ring[1], ring[2], ring[eo], ring[ez]], &[h,s6,s6,s6,z]);
            }

            if c.em_bnd {
                assign_row(m, b+2, &[ci as i32, ring[3], ring[0]], &[t23, t3, z]);
            } else {
                let eo=if c.ep_bnd{0}else{3}; let ez=if c.ep_bnd{3}else{0};
                assign_row(m, b+2, &[ci as i32, ring[1], ring[2], ring[eo], ring[ez]], &[h,s6,s6,s6,z]);
            }
        }
    }

    fn compute_irreg_edge(&self, ci: usize, m: &mut SparseMatrix<R>, wb: &mut Vec<R>) {
        let c = &self.c[ci]; let b = (5*ci) as i32;
        if c.is_sharp {
            let t23=R::from(2.0/3.0).unwrap_or_default();
            let t13=R::from(1.0/3.0).unwrap_or_default();
            assign_row(m, b,   &[ci as i32], &[R::one()]);
            assign_row(m, b+1, &[ci as i32, ((ci+1)%3) as i32], &[t23,t13]);
            assign_row(m, b+2, &[ci as i32, ((ci+2)%3) as i32], &[t23,t13]);
        } else if !c.is_bnd {
            self.compute_irreg_int_edge(ci, m, wb);
        } else if c.nf > 1 {
            self.compute_irreg_bnd_edge(ci, m, wb);
        } else {
            let f46=R::from(4.0/6.0).unwrap_or_default();
            let f16=R::from(1.0/6.0).unwrap_or_default();
            let f23=R::from(2.0/3.0).unwrap_or_default();
            let f13=R::from(1.0/3.0).unwrap_or_default();
            let z=R::zero();
            assign_row(m, b,   &[ci as i32,((ci+1)%3)as i32,((ci+2)%3)as i32], &[f46,f16,f16]);
            assign_row(m, b+1, &[ci as i32,((ci+1)%3)as i32,((ci+2)%3)as i32], &[f23,f13,z]);
            assign_row(m, b+2, &[ci as i32,((ci+2)%3)as i32,((ci+1)%3)as i32], &[f23,f13,z]);
        }
    }

    fn compute_irreg_int_edge(&self, ci: usize, m: &mut SparseMatrix<R>, wb: &mut Vec<R>) {
        let c = &self.c[ci]; let val = c.val as usize; let ww = val+1;
        let b = (5*ci) as i32; let ring = c.ring.clone();
        wb.resize(3*ww, R::zero());
        let (pb,rest)=wb.split_at_mut(ww); let (ep,em)=rest.split_at_mut(ww);
        loop_limits_interior::<R>(val, c.fi as usize, pb, Some(ep), Some(em));
        let pw=pb.to_vec(); let epw=ep.to_vec(); let emw=em.to_vec();
        let mut ix=vec![ci as i32]; for r in &ring { ix.push(*r); }
        assign_row(m, b,   &ix, &pw);
        assign_row(m, b+1, &ix, &epw);
        assign_row(m, b+2, &ix, &emw);
    }

    fn compute_irreg_bnd_edge(&self, ci: usize, m: &mut SparseMatrix<R>, wb: &mut Vec<R>) {
        let c=&self.c[ci]; let val=c.val as usize; let ww=1+val;
        let b=(5*ci) as i32; let ring=c.ring.clone(); let bn=ww-1;
        wb.resize(3*ww, R::zero());
        let (pb,rest)=wb.split_at_mut(ww); let (ep,em)=rest.split_at_mut(ww);
        loop_limits_boundary::<R>(val, c.fi as usize, pb, Some(ep), Some(em));
        let pw=pb.to_vec(); let epw=ep.to_vec(); let emw=em.to_vec();
        let p0=ci as i32; let p1=ring[0]; let pn=ring[val-1];
        assign_row(m, b, &[p0,p1,pn], &[pw[0],pw[1],pw[bn]]);
        if c.ep_bnd {
            assign_row(m, b+1, &[p0,p1,pn], &[epw[0],epw[1],R::zero()]);
        } else {
            let mut ix=vec![p0]; let mut wv=vec![epw[0]];
            for i in 1..ww { ix.push(ring[i-1]); wv.push(epw[i]); }
            assign_row(m, b+1, &ix, &wv);
        }
        if c.em_bnd {
            assign_row(m, b+2, &[p0,pn,p1], &[emw[0],emw[bn],R::zero()]);
        } else {
            let mut ix=vec![p0]; let mut wv=vec![emw[0]];
            for i in 1..ww { ix.push(ring[i-1]); wv.push(emw[i]); }
            assign_row(m, b+2, &ix, &wv);
        }
    }

    // -----------------------------------------------------------------------
    // Face points
    // -----------------------------------------------------------------------

    fn assign_reg_face(&self, ci: usize, m: &mut SparseMatrix<R>) {
        let c=&self.c[ci]; let cn=(ci+1)%3; let cp=(ci+2)%3;
        let ring=c.ring.clone(); let b=(5*ci) as i32;
        for fisfm in 0..2usize {
            let is_reg=if fisfm==0{c.fp_reg}else{c.fm_reg};
            if !is_reg { continue; }
            let fr=b+3+fisfm as i32;
            if c.is_corner {
                assign_row(m, fr, &[ci as i32,cn as i32,cp as i32],
                    &[R::from(0.5).unwrap_or_default(),R::from(0.25).unwrap_or_default(),R::from(0.25).unwrap_or_default()]);
            } else if c.ep_bnd {
                assign_row(m, fr, &[ci as i32,ring[0],ring[1],ring[2]],
                    &[R::from(11.0/24.0).unwrap_or_default(),R::from(7.0/24.0).unwrap_or_default(),
                      R::from(5.0/24.0).unwrap_or_default(),R::from(1.0/24.0).unwrap_or_default()]);
            } else if c.em_bnd {
                assign_row(m, fr, &[ci as i32,ring[3],ring[2],ring[1]],
                    &[R::from(11.0/24.0).unwrap_or_default(),R::from(7.0/24.0).unwrap_or_default(),
                      R::from(5.0/24.0).unwrap_or_default(),R::from(1.0/24.0).unwrap_or_default()]);
            } else {
                let en=if c.is_bnd{0}else{((c.fi+5)%6)as usize};
                let ep=if c.is_bnd{3}else{((c.fi+2)%6)as usize};
                assign_row(m, fr, &[ci as i32,cp as i32,cn as i32,ring[ep],ring[en]],
                    &[R::from(10.0/24.0).unwrap_or_default(),R::from(0.25).unwrap_or_default(),
                      R::from(0.25).unwrap_or_default(),R::from(1.0/24.0).unwrap_or_default(),
                      R::from(1.0/24.0).unwrap_or_default()]);
            }
        }
    }

    fn compute_irreg_face(
        &self, ci: usize, m: &mut SparseMatrix<R>, rw: &mut Vec<R>, cm: &mut Vec<i32>,
    ) {
        let c=&self.c[ci]; let cn=(ci+1)%3; let cp=(ci+2)%3;
        let b=(5*ci) as i32;

        if !c.fp_reg && !c.fp_copy {
            self.one_face_pt(ci,c.fi as usize,cn, b,b+1,5*cn as i32+2,b+3, 1.0, m,rw,cm);
        }
        if !c.fm_reg && !c.fm_copy {
            let ie=((c.fi+1)%c.val) as usize;
            self.one_face_pt(ci,ie,cp, b,b+2,5*cp as i32+1,b+4, -1.0, m,rw,cm);
        }
        if c.fp_copy {
            let fc=m.get_row_columns(b+4).to_vec(); let fw=m.get_row_elements(b+4).to_vec();
            assign_row(m, b+3, &fc, &fw);
        }
        if c.fm_copy {
            let fc=m.get_row_columns(b+3).to_vec(); let fw=m.get_row_elements(b+3).to_vec();
            assign_row(m, b+4, &fc, &fw);
        }
    }

    /// Compute one face point (fp or fm).
    /// F = (1/4)*(cosFar*P + (4-2*cosNear-cosFar)*eNear + 2*cosNear*eFar)
    ///   + rScale*(-sign*ring[iPrev] + sign*ring[iNext])
    #[allow(clippy::too_many_arguments)]
    fn one_face_pt(
        &self, cn: usize, en: usize, cf: usize,
        pr: i32, enr: i32, efr: i32, fr: i32, sign: f64,
        m: &mut SparseMatrix<R>, rw: &mut Vec<R>, cm: &mut Vec<i32>,
    ) {
        let cosn=self.c[cn].cos_fa; let cosf=self.c[cf].cos_fa;
        let ring=self.c[cn].ring.clone(); let val=self.c[cn].val as usize;
        let pc =R::from(cosf/4.0).unwrap_or_default();
        let ec =R::from((4.0-2.0*cosn-cosf)/4.0).unwrap_or_default();
        let efc=R::from(2.0*cosn/4.0).unwrap_or_default();
        let n  =self.nsrc as usize;
        rw[..n].iter_mut().for_each(|v| *v=R::zero());
        cm[..n].iter_mut().for_each(|v| *v=0);

        for (row, coeff) in [(pr,pc),(enr,ec),(efr,efc)] {
            let idx=m.get_row_columns(row).to_vec();
            let wgt=m.get_row_elements(row).to_vec();
            for (&i,&w) in idx.iter().zip(wgt.iter()) {
                rw[i as usize] += coeff*w;
                cm[i as usize]  = 1+i;
            }
        }

        let rs=R::from(0.25*(7.0/18.0)).unwrap_or_default();
        let sg=R::from(sign).unwrap_or_default();
        let ip=(en+val-1)%val; let inx=(en+1)%val;
        let rp=ring[ip] as usize; let rn=ring[inx] as usize;
        rw[rp] += -sg*rs; cm[rp]=1+rp as i32;
        rw[rn] +=  sg*rs; cm[rn]=1+rn as i32;

        let dsz=m.get_row_size(fr) as usize;
        let mut oi=Vec::with_capacity(dsz); let mut ow=Vec::with_capacity(dsz);
        let mut cnt=0;
        for i in 0..n {
            if cm[i]!=0 { oi.push(cm[i]-1); ow.push(rw[i]); cnt+=1; if cnt>=dsz {break;} }
        }
        if self.has_v2 { while cnt<dsz { oi.push(cn as i32); ow.push(R::zero()); cnt+=1; } }
        let l=dsz.min(oi.len());
        assign_row(m, fr, &oi[..l], &ow[..l]);
    }

    // -----------------------------------------------------------------------
    // Mid-edge points
    // -----------------------------------------------------------------------

    fn assign_reg_mid_edge(&self, ei: usize, m: &mut SparseMatrix<R>) {
        let mr=(15+ei) as i32; let c=&self.c[ei]; let cn=(ei+1)%3;
        if c.ep_bnd {
            let h=R::from(0.5).unwrap_or_default();
            assign_row(m, mr, &[ei as i32, cn as i32], &[h,h]);
        } else {
            let opp=if c.is_bnd {(c.fi-1)as usize} else {((c.fi+5)%6)as usize};
            let ov=c.ring[opp];
            let t3=R::from(1.0/3.0).unwrap_or_default(); let s6=R::from(1.0/6.0).unwrap_or_default();
            assign_row(m, mr, &[ei as i32, cn as i32, ((ei+2)%3) as i32, ov], &[t3,t3,s6,s6]);
        }
    }

    fn compute_irreg_mid_edge(
        &self, ei: usize, m: &mut SparseMatrix<R>, rw: &mut Vec<R>, cm: &mut Vec<i32>,
    ) {
        let e0p=(5*ei+1) as i32; let e1m=(5*((ei+1)%3)+2) as i32;
        let mr=(15+ei) as i32; let h=R::from(0.5).unwrap_or_default();
        let (oi,ow)=combine_two_rows(m,e0p,h,e1m,h,self.nsrc as usize,rw,cm);
        let dsz=m.get_row_size(mr) as usize;
        let mut oi=oi; let mut ow=ow;
        while oi.len()<dsz { oi.push(0); ow.push(R::zero()); }
        assign_row(m, mr, &oi[..dsz], &ow[..dsz]);
    }

    // -----------------------------------------------------------------------
    // Quartic promotion
    // -----------------------------------------------------------------------

    fn promote_quartic(&self, m: &mut SparseMatrix<R>, rw: &mut Vec<R>, cm: &mut Vec<i32>) {
        let bw:[f64;3]=[16.0,7.0,1.0];
        let rbw:[f64;5]=[13.0,3.0,3.0,4.0,1.0];
        let riw:[f64;7]=[12.0,4.0,3.0,1.0,0.0,1.0,3.0];
        let ov24=R::from(1.0/24.0).unwrap_or_default();

        for ci in 0..3usize {
            let c=&self.c[ci]; let pr=(5*ci) as i32;
            for ep in 0..2usize {
                let er=(5*ci+1+ep) as i32;
                let ob=if ep==0{c.ep_bnd}else{c.em_bnd};
                let tbl: Option<&[f64]> = if ob && !c.is_sharp { Some(&bw) }
                    else if c.is_reg { if c.is_bnd {Some(&rbw)} else {Some(&riw)} }
                    else { None };
                if let Some(t) = tbl {
                    let el=m.get_row_elements_mut(er);
                    for (i,&w) in t.iter().enumerate() { if i<el.len() { el[i]=ov24*R::from(w).unwrap_or_default(); } }
                } else {
                    let qtr=R::from(0.25).unwrap_or_default();
                    let tqr=R::from(0.75).unwrap_or_default();
                    let (oi,ow)=combine_two_rows(m,pr,qtr,er,tqr,self.nsrc as usize,rw,cm);
                    let dsz=m.get_row_size(er) as usize;
                    let mut oi=oi; let mut ow=ow;
                    while oi.len()<dsz { oi.push(0); ow.push(R::zero()); }
                    assign_row(m, er, &oi[..dsz], &ow[..dsz]);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Main convert
    // -----------------------------------------------------------------------

    fn convert(&self, m: &mut SparseMatrix<R>) {
        if self.isolated { self.size_isolated(m); } else { self.size_unisolated(m); }

        let rs  = 1 + self.max_val as usize;
        let bsz = std::cmp::max(3*rs, 2*self.nsrc as usize);
        let mut rw: Vec<R>   = vec![R::zero(); bsz];
        let mut cm: Vec<i32> = vec![0; bsz];

        for ci in 0..3 {
            if self.c[ci].is_reg { self.assign_reg_edge(ci, m); }
            else                 { self.compute_irreg_edge(ci, m, &mut rw); }
        }
        for ci in 0..3 {
            let c=&self.c[ci];
            if c.fp_reg || c.fm_reg { self.assign_reg_face(ci, m); }
            if !c.fp_reg || !c.fm_reg { self.compute_irreg_face(ci, m, &mut rw, &mut cm); }
        }
        for ei in 0..3 {
            let c0=&self.c[ei]; let c1=&self.c[(ei+1)%3];
            let bnd = c0.ep_bnd && c1.em_bnd;
            let drt = c0.ep_bnd != c1.em_bnd;
            if bnd || (c0.is_reg && c1.is_reg && !drt) { self.assign_reg_mid_edge(ei, m); }
            else { self.compute_irreg_mid_edge(ei, m, &mut rw, &mut cm); }
        }
        self.promote_quartic(m, &mut rw, &mut cm);
        if self.has_v2 { remove_valence2_duplicates(m); }
    }
}

// ---------------------------------------------------------------------------
// convertToGregory
// ---------------------------------------------------------------------------

fn convert_to_gregory_generic<R>(sp: &SourcePatch, m: &mut SparseMatrix<R>)
where R: Copy + Default + num_traits::Float + std::ops::AddAssign,
{ GtConverter::<R>::new(sp).convert(m); }

fn convert_to_gregory_f32(sp: &SourcePatch, m: &mut SparseMatrix<f32>) { convert_to_gregory_generic(sp, m); }
fn convert_to_gregory_f64(sp: &SourcePatch, m: &mut SparseMatrix<f64>) { convert_to_gregory_generic(sp, m); }

// ---------------------------------------------------------------------------
// remove_valence2_duplicates — mirrors C++ _removeValence2Duplicates
// ---------------------------------------------------------------------------
fn remove_valence2_duplicates<R: Copy + Default>(m: &mut SparseMatrix<R>)
where R: std::ops::AddAssign,
{
    let nr=m.get_num_rows(); let nc=m.get_num_columns() as usize;
    let mut t: SparseMatrix<R> = SparseMatrix::new();
    t.resize(nr, nc as i32, m.get_num_elements());

    for row in 0..nr {
        let si: Vec<i32> = m.get_row_columns(row).to_vec();
        let sw: Vec<R>   = m.get_row_elements(row).to_vec();
        let ss           = si.len();

        let mut used=[false;4]; let mut dups=0;
        for &i in &si { if (i as usize)<4 { if used[i as usize]{dups+=1;} used[i as usize]=true; } }

        let ds=ss-dups;
        t.set_row_size(row, ds as i32);
        let mut oc=vec![0i32;ds]; let mut ow=vec![R::default();ds];

        if dups>0 {
            let mut pos=[usize::MAX;4]; let mut oi=0;
            for i in 0..ss {
                let si_=si[i] as usize; let sw_=sw[i];
                if si_<4 { if pos[si_]!=usize::MAX { ow[pos[si_]]+=sw_; continue; } pos[si_]=oi; }
                oc[oi]=si_ as i32; ow[oi]=sw_; oi+=1;
            }
        } else { oc.copy_from_slice(&si); ow.copy_from_slice(&sw); }

        t.get_row_columns_mut(row).copy_from_slice(&oc);
        t.get_row_elements_mut(row).copy_from_slice(&ow);
    }
    m.swap(&mut t);
}

/// Standalone entry point called from PatchBuilder.
pub fn convert_loop(
    sp: &SourcePatch, pt: PatchType, m: &mut SparseMatrix<f32>,
) -> i32 {
    match pt {
        PatchType::Loop            => { convert_to_loop_f32(sp, m);    m.get_num_rows() }
        PatchType::Triangles       => { convert_to_linear_f32(sp, m);  m.get_num_rows() }
        PatchType::GregoryTriangle => { convert_to_gregory_f32(sp, m); m.get_num_rows() }
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::{Options, types::SchemeType};
    use super::super::topology_refiner::TopologyRefiner;

    fn make_reg_patch() -> SourcePatch {
        let mut sp = SourcePatch::new();
        for i in 0..3 {
            sp.corners[i].num_faces  = 6;
            sp.corners[i].patch_face = 0;
            sp.corners[i].boundary   = false;
            sp.corners[i].sharp      = false;
        }
        sp.finalize(3);
        sp
    }

    #[test]
    fn loop_patch_type_defaults() {
        let r=TopologyRefiner::new(SchemeType::Loop, Options::default());
        let pb=LoopPatchBuilder::new(&r, PatchBuilderOptions::default());
        assert_eq!(pb.base.get_regular_patch_type(), PatchType::Loop);
        assert_eq!(pb.base.native_patch_type, PatchType::Loop);
        assert_eq!(pb.base.linear_patch_type, PatchType::Triangles);
    }

    #[test]
    fn loop_patch_type_from_basis_map() {
        let r=TopologyRefiner::new(SchemeType::Loop, Options::default());
        let pb=LoopPatchBuilder::new(&r, PatchBuilderOptions::default());
        assert_eq!(pb.patch_type_from_basis(BasisType::Regular), PatchType::Loop);
        assert_eq!(pb.patch_type_from_basis(BasisType::Gregory), PatchType::GregoryTriangle);
        assert_eq!(pb.patch_type_from_basis(BasisType::Linear),  PatchType::Triangles);
    }

    #[test]
    fn convert_linear_3_rows() {
        let sp=make_reg_patch();
        let r=TopologyRefiner::new(SchemeType::Loop, Options::default());
        let pb=LoopPatchBuilder::new(&r, PatchBuilderOptions::default());
        let mut m=SparseMatrix::new();
        assert_eq!(pb.convert_to_patch_type_f32(&sp, PatchType::Triangles, &mut m), 3);
    }

    #[test]
    fn convert_gregory_18_rows() {
        let sp=make_reg_patch();
        let r=TopologyRefiner::new(SchemeType::Loop, Options::default());
        let pb=LoopPatchBuilder::new(&r, PatchBuilderOptions::default());
        let mut m=SparseMatrix::new();
        assert_eq!(pb.convert_to_patch_type_f32(&sp, PatchType::GregoryTriangle, &mut m), 18);
    }

    #[test]
    fn convert_loop_12_rows() {
        let sp=make_reg_patch();
        let r=TopologyRefiner::new(SchemeType::Loop, Options::default());
        let pb=LoopPatchBuilder::new(&r, PatchBuilderOptions::default());
        let mut m=SparseMatrix::new();
        assert_eq!(pb.convert_to_patch_type_f32(&sp, PatchType::Loop, &mut m), 12);
    }

    #[test]
    fn interior_limit_weights_sum_1() {
        for &v in &[3usize,4,5,6,7,8,10] {
            let mut p=vec![0.0f32;v+1];
            loop_limits_interior::<f32>(v,0,&mut p,None,None);
            let s:f32=p.iter().sum();
            assert!((s-1.0).abs()<1e-5, "valence {v}: sum={s}");
        }
    }

    #[test]
    fn interior_limit_regular_n6() {
        let mut p=vec![0.0f32;7];
        loop_limits_interior::<f32>(6,0,&mut p,None,None);
        assert!((p[0]-0.5).abs()<1e-6, "center={}", p[0]);
        for i in 1..=6 {
            assert!((p[i]-1.0/12.0).abs()<1e-6, "ring[{i}]={}", p[i]);
        }
    }

    #[test]
    fn boundary_limit_weights_sum_1() {
        let mut p=vec![0.0f32;4];
        loop_limits_boundary::<f32>(3,0,&mut p,None,None);
        let s:f32=p.iter().sum();
        assert!((s-1.0).abs()<1e-5, "sum={s}");
    }

    #[test]
    fn gregory_converter_boundary() {
        let mut sp=SourcePatch::new();
        sp.corners[0].num_faces=3; sp.corners[0].patch_face=0;
        sp.corners[0].boundary=true; sp.corners[0].sharp=false;
        for i in 1..3 {
            sp.corners[i].num_faces=6; sp.corners[i].patch_face=0;
            sp.corners[i].boundary=false; sp.corners[i].sharp=false;
        }
        sp.finalize(3);
        let mut m=SparseMatrix::new();
        convert_to_gregory_f32(&sp, &mut m);
        assert_eq!(m.get_num_rows(), 18);
    }
}
