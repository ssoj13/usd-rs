//! PatchTreeBuilder -- assembles a PatchTree from a Far::TopologyRefiner.
//!
//! Ported from OpenSubdiv bfr/patchTreeBuilder.h/.cpp.
//!
//! The real pipeline (used by IrregularPatchBuilder::build) is:
//!   1. Caller gathers control hull topology into a TopologyDescriptor
//!   2. TopologyDescriptorFactory::create() builds a TopologyRefiner
//!   3. RefinerFaceAdapter::refine_and_create() runs adaptive refinement
//!      and wraps the refiner with PatchBuilder + PtexIndices
//!   4. PatchTreeBuilder assembles the PatchTree (patches + stencils + quadtree)
//!
//! The FaceRefiner trait provides a seam so test doubles can be used without
//! a real TopologyRefiner.

use super::patch_tree::PatchTree;
use crate::far::{
    PatchType, PatchParam, AdaptiveOptions, PtexIndices,
    TopologyRefiner,
};
use crate::far::patch_builder::{PatchBuilder, PatchBuilderOptions, BasisType};
use crate::far::primvar_refiner::{PrimvarRefiner, Interpolatable};
use crate::far::sparse_matrix::SparseMatrix;
use crate::vtr::VSpan;

// ---------------------------------------------------------------------------
//  PatchTreeBuilder options
// ---------------------------------------------------------------------------

/// Basis type used for irregular patches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrregularBasis {
    Regular,
    Gregory,
    Linear,
}

/// Construction options for `PatchTreeBuilder`.
#[derive(Clone, Copy, Debug)]
pub struct PatchTreeBuilderOptions {
    /// Basis type for irregular patches.
    pub irregular_basis:          IrregularBasis,
    /// Maximum refinement depth for sharp features.
    pub max_patch_depth_sharp:    u8,
    /// Maximum refinement depth for smooth features.
    pub max_patch_depth_smooth:   u8,
    /// Include non-leaf patches in the tree.
    pub include_interior_patches: bool,
    /// Use double-precision stencil matrix.
    pub use_double_precision:     bool,
}

impl Default for PatchTreeBuilderOptions {
    fn default() -> Self {
        PatchTreeBuilderOptions {
            irregular_basis:          IrregularBasis::Gregory,
            max_patch_depth_sharp:    4,
            max_patch_depth_smooth:   15,
            include_interior_patches: false,
            use_double_precision:     false,
        }
    }
}

// ---------------------------------------------------------------------------
//  PatchFace -- internal record for one patch in the refinement hierarchy
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PatchFace {
    pub face:       i32,
    pub level:      i16,
    pub is_regular: bool,
}

impl PatchFace {
    pub fn new(level: i32, face: i32, is_regular: bool) -> Self {
        PatchFace { face, level: level as i16, is_regular }
    }
}

// ---------------------------------------------------------------------------
//  FaceRefiner trait -- abstraction over Far::TopologyRefiner
// ---------------------------------------------------------------------------

/// Minimal interface required from a topology refiner by `PatchTreeBuilder`.
///
/// Implement this for `Far::TopologyRefiner` or a test double.
pub trait FaceRefiner {
    /// Return the number of refinement levels (including base level 0).
    fn num_levels(&self) -> i32;
    /// Return the number of vertices at `level`.
    fn num_vertices_at(&self, level: i32) -> i32;
    /// Return the total number of vertices across all levels.
    fn num_vertices_total(&self) -> i32;
    /// Return the number of faces at `level`.
    fn num_faces_at(&self, level: i32) -> i32;
    /// Return the number of vertices per face at level 0 (the root face).
    fn root_face_size(&self) -> i32;
    /// Return the regular face size for this scheme.
    fn regular_face_size(&self) -> i32;
    /// Return the patch type assigned to a face.
    fn face_is_patch(&self, level: i32, face: i32) -> bool;
    /// Return whether the patch at (level, face) is a leaf.
    fn face_is_leaf(&self, level: i32, face: i32) -> bool;
    /// Return whether the patch at (level, face) is regular.
    fn patch_is_regular(&self, level: i32, face: i32) -> bool;
    /// Gather the patch-point indices for (level, face) into `out`.
    fn gather_patch_points(&self, level: i32, face: i32, out: &mut [i32]) -> i32;
    /// Return the PatchParam (u, v, depth, boundary, face_id, non_quad_root).
    fn patch_param(&self, level: i32, face: i32) -> crate::far::PatchParam;
    /// Get the patch type for regular patches in this scheme.
    fn regular_patch_type(&self) -> PatchType;
    /// Get the patch type for irregular (Gregory) patches in this scheme.
    fn irregular_patch_type(&self) -> PatchType;
    /// Compute stencil matrix rows for refined points above the control points.
    /// `out_f32` or `out_f64` will be filled with `num_refined * num_control` rows.
    fn compute_stencil_matrix_f32(&self, num_control: i32, out: &mut Vec<f32>);
    fn compute_stencil_matrix_f64(&self, num_control: i32, out: &mut Vec<f64>);

    /// Return true when ancestor-filtering of child faces is required.
    ///
    /// Mirrors C++ `PatchTreeBuilder::testFaceAncestors()`: only needed for
    /// triangular (Loop) schemes where a single root tri shares vertices with
    /// neighbour faces and adaptive refinement can emit child faces descended
    /// from those neighbours rather than from the root.
    ///
    /// Default: false (quad schemes never require this filter).
    fn needs_ancestor_test(&self) -> bool { false }

    /// Return true when `face` at `level` is a descendant of face 0 (root).
    ///
    /// Only called when `needs_ancestor_test()` returns true.
    /// Default: true (keep every face when filtering is not implemented).
    fn face_ancestor_is_root(&self, level: i32, face: i32) -> bool {
        let _ = (level, face);
        true
    }
}

// ---------------------------------------------------------------------------
//  RefinerFaceAdapter -- implements FaceRefiner for the real TopologyRefiner
// ---------------------------------------------------------------------------

/// Adapts `Far::TopologyRefiner` + `Far::PatchBuilder` to the `FaceRefiner`
/// trait, enabling `PatchTreeBuilder` to work with the real refinement pipeline.
///
/// The refiner must already have been adaptively refined before construction.
pub struct RefinerFaceAdapter<'a> {
    refiner:       &'a TopologyRefiner,
    patch_builder: PatchBuilder<'a>,
    ptex_indices:  PtexIndices,
    level_offsets: Vec<i32>,
}

/// Return true when the root face (face 0 at level 0) has features that
/// prevent the PatchBuilder from producing a single patch without at least
/// one level of adaptive refinement.
///
/// Mirrors C++ `PatchTreeBuilder::rootFaceNeedsRefinement()`.
/// Called before adaptive refinement, so only the base level (level 0) is
/// available.  Uses the internal `Level` API to access composite vertex tags.
fn root_face_needs_refinement(refiner: &TopologyRefiner) -> bool {
    let base    = refiner.get_level_internal(0);
    let f_tags  = base.get_face_composite_vtag(0);
    let f_verts = base.get_face_vertices(0);

    // Any vertex adjacent to an irregular (non-quad) face forces refinement.
    if f_tags.incid_irreg_face() { return true; }

    // An inf-sharp dart vertex requires refinement (may be isolatable in the
    // future, but conservatively always refine for now).
    if (f_tags.rule() & crate::sdc::crease::Rule::Dart.bits() as u16 != 0)
        && f_tags.inf_irregular()
    {
        for i in 0..f_verts.size() {
            let vt = base.get_vertex_tag(f_verts[i]);
            if (vt.rule() & crate::sdc::crease::Rule::Dart.bits() as u16 != 0)
                && vt.inf_sharp_edges()
            {
                return true;
            }
        }
    }

    // Interior extraordinary vertices of very low valence cannot be patched
    // directly at level 0 (val-2 always problematic; val-3 for triangular).
    if f_tags.xordinary() {
        let f_size = f_verts.size();
        for i in 0..f_size {
            let vt = base.get_vertex_tag(f_verts[i]);
            if vt.xordinary() && !vt.boundary() && !vt.inf_sharp_edges() {
                let valence = base.get_num_vertex_faces(f_verts[i]);
                if valence == 2 || (valence == 3 && f_size == 3) {
                    return true;
                }
            }
        }
    }

    false
}

impl<'a> RefinerFaceAdapter<'a> {
    /// Construct from an already-refined TopologyRefiner and irregular basis choice.
    pub fn new(refiner: &'a TopologyRefiner, irreg_basis: IrregularBasis) -> Self {
        let irreg_type = match irreg_basis {
            IrregularBasis::Regular => BasisType::Regular,
            IrregularBasis::Gregory => BasisType::Gregory,
            IrregularBasis::Linear  => BasisType::Linear,
        };

        let pb_opts = PatchBuilderOptions {
            reg_basis:                       BasisType::Regular,
            irreg_basis:                     irreg_type,
            fill_missing_boundary_points:    true,
            approx_inf_sharp_with_smooth:    false,
            approx_smooth_corner_with_sharp: false,
        };

        let patch_builder = PatchBuilder::create(refiner, pb_opts);
        let ptex_indices  = PtexIndices::new(refiner);

        // Cumulative vertex offsets per level (mirrors C++ _levelOffsets)
        let num_levels = refiner.get_num_levels() as usize;
        let mut offsets = vec![0i32; num_levels + 1];
        for i in 0..num_levels {
            offsets[i + 1] = offsets[i] + refiner.get_level(i as i32).get_num_vertices();
        }

        Self { refiner, patch_builder, ptex_indices, level_offsets: offsets }
    }

    /// Apply adaptive refinement to a TopologyRefiner, then return a new adapter.
    ///
    /// Mirrors what C++ `PatchTreeBuilder` constructor does internally.
    pub fn refine_and_create(
        refiner: &'a mut TopologyRefiner,
        opts:    &PatchTreeBuilderOptions,
    ) -> Self {
        let primary   = opts.max_patch_depth_sharp as u32;
        let mut secondary = (opts.max_patch_depth_smooth as u32).min(primary);
        let mut primary_out = primary;

        // Bug 1 guard: mirrors C++ PatchTreeBuilder constructor logic.
        // When both depth levels are 0, the PatchBuilder cannot construct a
        // patch from level 0 if the root face has irregular features.  Force
        // at least one level of refinement in that case so the feature is
        // isolated before patch construction begins.
        if secondary == 0 && root_face_needs_refinement(refiner) {
            primary_out = primary_out.max(1);
            secondary   = 1;
        }

        let mut adapt_opts = AdaptiveOptions::new(primary_out);
        adapt_opts.set_secondary_level(secondary);
        adapt_opts.use_inf_sharp_patch     = true;
        adapt_opts.use_single_crease_patch = false;
        adapt_opts.consider_fvar_channels  = false;

        // Refine only face 0 (the single root face for this local neighborhood)
        refiner.refine_adaptive(
            adapt_opts,
            crate::far::types::ConstIndexArray::new(&[0]),
        );

        Self::new(refiner, opts.irregular_basis)
    }

    /// Build a complete PatchTree from this adapter.
    pub fn build_patch_tree(self, opts: &PatchTreeBuilderOptions) -> Box<PatchTree> {
        let builder = PatchTreeBuilder::new(&self, *opts);
        builder.build(&self)
    }

    // -- Internal helpers --

    /// Collect irregular patch conversion data: (sparse_matrix, source_point_indices).
    ///
    /// Mirrors C++ `getIrregularPatchConversion<REAL>`.
    fn irregular_patch_conversion(
        &self,
        pf: &PatchFace,
    ) -> (SparseMatrix<f32>, Vec<i32>) {
        let mut corner_spans = [VSpan::default(); 4];
        self.patch_builder.get_irregular_patch_corner_spans(
            pf.level as i32, pf.face, &mut corner_spans, -1);

        let mut conv_matrix = SparseMatrix::new();
        self.patch_builder.get_irregular_patch_conversion_matrix(
            pf.level as i32, pf.face, &corner_spans, &mut conv_matrix);

        let num_src = conv_matrix.get_num_columns() as usize;
        let mut source_points = vec![0i32; num_src];
        self.patch_builder.get_irregular_patch_source_points(
            pf.level as i32, pf.face, &corner_spans, &mut source_points, -1);

        // Translate source point indices from level-local to global stencil space
        let offset = self.level_offsets[pf.level as usize];
        for sp in source_points.iter_mut() {
            *sp += offset;
        }

        (conv_matrix, source_points)
    }
}

impl<'a> FaceRefiner for RefinerFaceAdapter<'a> {
    fn num_levels(&self) -> i32 { self.refiner.get_num_levels() }

    fn num_vertices_at(&self, level: i32) -> i32 {
        self.refiner.get_level(level).get_num_vertices()
    }

    fn num_vertices_total(&self) -> i32 { self.refiner.get_num_vertices_total() }

    fn num_faces_at(&self, level: i32) -> i32 {
        self.refiner.get_level(level).get_num_faces()
    }

    fn root_face_size(&self) -> i32 {
        self.refiner.get_level(0).get_face_vertices(0).size()
    }

    fn regular_face_size(&self) -> i32 {
        self.patch_builder.get_regular_face_size()
    }

    fn face_is_patch(&self, level: i32, face: i32) -> bool {
        self.patch_builder.is_face_a_patch(level, face)
    }

    fn face_is_leaf(&self, level: i32, face: i32) -> bool {
        self.patch_builder.is_face_a_leaf(level, face)
    }

    fn patch_is_regular(&self, level: i32, face: i32) -> bool {
        self.patch_builder.is_patch_regular(level, face, -1)
    }

    fn gather_patch_points(&self, level: i32, face: i32, out: &mut [i32]) -> i32 {
        // Only called for regular patches; irregular patches get local-point indices
        // assigned directly in initialize_patches().
        let bnd_mask = self.patch_builder.get_regular_patch_boundary_mask(level, face, -1);
        let n = self.patch_builder.get_regular_patch_points(level, face, bnd_mask, out, -1);
        // Offset by cumulative level vertex offset so indices are global
        let offset = self.level_offsets[level as usize];
        for v in out[..n as usize].iter_mut() {
            *v += offset;
        }
        n
    }

    fn patch_param(&self, level: i32, face: i32) -> PatchParam {
        let is_regular    = self.patch_is_regular(level, face);
        let bnd_mask      = if is_regular {
            self.patch_builder.get_regular_patch_boundary_mask(level, face, -1)
        } else { 0 };
        let compute_trans = level < self.refiner.get_max_level();
        self.patch_builder.compute_patch_param(
            level, face, &self.ptex_indices, is_regular, bnd_mask, compute_trans)
    }

    fn regular_patch_type(&self) -> PatchType {
        self.patch_builder.get_regular_patch_type()
    }

    fn irregular_patch_type(&self) -> PatchType {
        self.patch_builder.get_irreg_patch_type()
    }

    fn compute_stencil_matrix_f32(&self, num_control: i32, out: &mut Vec<f32>) {
        build_stencil_matrix_f32(self, num_control, out);
    }

    fn compute_stencil_matrix_f64(&self, num_control: i32, out: &mut Vec<f64>) {
        build_stencil_matrix_f64(self, num_control, out);
    }

    /// Ancestor filtering is needed for triangular (Loop) schemes when adaptive
    /// refinement of the single root triangle can generate child faces from
    /// neighbouring triangles that share vertices with the root.
    ///
    /// Mirrors C++ `PatchTreeBuilder::testFaceAncestors()`:
    ///   `regFaceSize == 3 && baseLevel.getNumEdges() == 3 && baseLevel.getNumFaces() > 1`
    fn needs_ancestor_test(&self) -> bool {
        let base = self.refiner.get_level(0);
        self.patch_builder.get_regular_face_size() == 3
            && base.get_num_edges() == 3
            && base.get_num_faces() > 1
    }

    /// Walk parent links from `level`/`face` up to level 0 and check it
    /// reaches face 0 (the root face).
    ///
    /// Mirrors C++ `PatchTreeBuilder::faceAncestorIsRoot()`.
    fn face_ancestor_is_root(&self, level: i32, face: i32) -> bool {
        let mut f = face;
        for lv in (1..=level).rev() {
            // TopologyLevel::get_face_parent_face uses the parent refinement
            f = self.refiner.get_level(lv).get_face_parent_face(f);
        }
        f == 0
    }
}

// ---------------------------------------------------------------------------
//  Stencil matrix computation
// ---------------------------------------------------------------------------

/// A stencil row: one row of `nc` f32 weights expressing a refined point
/// as a linear combination of the control points.
///
/// Implements `Interpolatable` so `PrimvarRefiner` can propagate it through
/// subdivision levels in a single call.
#[derive(Clone)]
struct StencilRow {
    weights: Vec<f32>,
}

impl StencilRow {
    fn new(nc: usize) -> Self { StencilRow { weights: vec![0.0; nc] } }
    fn identity(nc: usize, ctrl_idx: usize) -> Self {
        let mut r = Self::new(nc);
        r.weights[ctrl_idx] = 1.0;
        r
    }
}

impl Default for StencilRow {
    /// Zero-length default; `PrimvarRefiner` always calls `clear()` before use.
    fn default() -> Self { StencilRow { weights: Vec::new() } }
}

impl Interpolatable for StencilRow {
    fn clear(&mut self) {
        for w in self.weights.iter_mut() { *w = 0.0; }
    }
    fn add_with_weight(&mut self, src: &Self, weight: f32) {
        debug_assert_eq!(self.weights.len(), src.weights.len());
        for (d, s) in self.weights.iter_mut().zip(src.weights.iter()) {
            *d += s * weight;
        }
    }
}

/// A stencil row storing f64 weights.
///
/// Mirrors C++ `StencilRow<double>`.  The subdivision mask weights from
/// `PrimvarRefiner` arrive as `f32` (that is what the C++ mask machinery
/// produces), but they are widened to f64 *before* the multiply so that
/// all accumulation takes place in double precision.  This matches what
/// C++ `PrimvarRefinerReal<double>` does internally.
#[derive(Clone)]
struct StencilRowF64 {
    weights: Vec<f64>,
}

impl StencilRowF64 {
    fn new(nc: usize) -> Self { StencilRowF64 { weights: vec![0.0f64; nc] } }
    fn identity(nc: usize, ctrl_idx: usize) -> Self {
        let mut r = Self::new(nc);
        r.weights[ctrl_idx] = 1.0;
        r
    }
}

impl Default for StencilRowF64 {
    fn default() -> Self { StencilRowF64 { weights: Vec::new() } }
}

impl Interpolatable for StencilRowF64 {
    fn clear(&mut self) {
        for w in self.weights.iter_mut() { *w = 0.0; }
    }
    /// Weight arrives as f32 from the subdivision mask; widen to f64
    /// before multiply so the entire accumulation is in double precision.
    fn add_with_weight(&mut self, src: &Self, weight: f32) {
        debug_assert_eq!(self.weights.len(), src.weights.len());
        let w64 = weight as f64;
        for (d, s) in self.weights.iter_mut().zip(src.weights.iter()) {
            *d += s * w64;
        }
    }
}

/// Build the f32 stencil matrix for all refined + irregular-patch points.
///
/// Mirrors C++ `PatchTreeBuilder::initializeStencilMatrix<float>()`.
//
// NOTE: keep the structure of this function in sync with build_stencil_matrix_f64().
fn build_stencil_matrix_f32(
    adapter:     &RefinerFaceAdapter<'_>,
    num_control: i32,
    out:         &mut Vec<f32>,
) {
    let refiner  = adapter.refiner;
    let nc       = num_control as usize;

    let num_refined = refiner.get_num_vertices_total() - num_control;

    // Count irregular patches among all leaf patches
    let num_levels     = refiner.get_num_levels();
    let mut num_irreg  = 0i32;
    let irreg_size     = patch_cv_count(adapter.patch_builder.get_irreg_patch_type());

    // Collect irregular leaf patches while we count them
    let mut irreg_patches: Vec<PatchFace> = Vec::new();
    for level in 0..num_levels {
        let nf = refiner.get_level(level).get_num_faces();
        for face in 0..nf {
            if adapter.face_is_patch(level, face) && adapter.face_is_leaf(level, face)
                && !adapter.patch_is_regular(level, face)
            {
                irreg_patches.push(PatchFace::new(level, face, false));
                num_irreg += 1;
            }
        }
    }

    let total_rows = num_refined + num_irreg * irreg_size;
    if total_rows <= 0 { return; }

    out.clear();
    out.resize(total_rows as usize * nc, 0.0f32);

    // ---- Refined point rows via PrimvarRefiner ----
    // Each control point starts as an identity row (weight 1.0 at its own column).
    // PrimvarRefiner::interpolate propagates these through each subdivision level,
    // accumulating blended stencils exactly as C++ does.
    if num_levels > 1 && num_refined > 0 {
        // Build identity rows for all control points (level 0)
        let mut src_rows: Vec<StencilRow> = (0..nc)
            .map(|i| StencilRow::identity(nc, i))
            .collect();

        let primvar_refiner = PrimvarRefiner::new(refiner);

        let mut dst_rows: Vec<StencilRow> = Vec::new();

        for level in 1..num_levels {
            let n_child = refiner.get_level(level).get_num_vertices() as usize;
            dst_rows.clear();
            dst_rows.resize(n_child, StencilRow::new(nc));

            primvar_refiner.interpolate(level, &src_rows, &mut dst_rows);

            // Write the dst_rows into the stencil matrix.
            // Rows for level `level` start at offset = (cumulative verts up to level) - nc
            let level_vert_offset = adapter.level_offsets[level as usize] as usize;
            let row_base = level_vert_offset - nc;  // stencil rows are for refined pts only

            for (i, row) in dst_rows.iter().enumerate() {
                let dst = &mut out[(row_base + i) * nc..(row_base + i + 1) * nc];
                dst.copy_from_slice(&row.weights);
            }

            src_rows = dst_rows.clone();
        }
    }

    // ---- Irregular patch rows via conversion matrix ----
    // These rows start immediately after the refined-point rows.
    if num_irreg > 0 {
        let stencil_base = num_refined as usize;

        for (patch_idx, pf) in irreg_patches.iter().enumerate() {
            let (conv_matrix, source_points) = adapter.irregular_patch_conversion(pf);

            // Append conversion stencils, mirrors C++ appendConversionStencilsToMatrix
            append_conversion_stencils(
                stencil_base + patch_idx * irreg_size as usize,
                &conv_matrix,
                &source_points,
                num_control as usize,
                out,
            );
        }
    }
}

/// Append stencil rows for one irregular patch's conversion matrix (f32).
///
/// Mirrors C++ `PatchTreeBuilder::appendConversionStencilsToMatrix<float>`.
fn append_conversion_stencils(
    dst_row_base:  usize,
    conv:          &SparseMatrix<f32>,
    src_pts:       &[i32],
    nc:            usize,
    stencil:       &mut Vec<f32>,
) {
    let num_rows = conv.get_num_rows();

    for i in 0..num_rows {
        let dst_base = (dst_row_base + i as usize) * nc;

        // Zero this destination row
        for k in 0..nc { stencil[dst_base + k] = 0.0; }

        let indices = conv.get_row_columns(i);
        let weights = conv.get_row_elements(i);

        for j in 0..conv.get_row_size(i) as usize {
            let src_global = src_pts[indices[j] as usize] as usize;
            let w          = weights[j];

            if src_global < nc {
                // Control point: direct delta contribution
                stencil[dst_base + src_global] += w;
            } else {
                // Refined point: blend its stencil row
                let src_row = src_global - nc;
                let src_base = src_row * nc;
                if src_base + nc <= stencil.len() {
                    // Avoid aliasing: read then write (same Vec, non-overlapping ranges)
                    let src_slice: Vec<f32> = stencil[src_base..src_base + nc].to_vec();
                    for k in 0..nc {
                        stencil[dst_base + k] += w * src_slice[k];
                    }
                }
            }
        }
    }
}

/// Build the f64 stencil matrix for all refined + irregular-patch points.
///
/// Mirrors C++ `PatchTreeBuilder::initializeStencilMatrix<double>()`.
/// All accumulation is in f64; the f32 subdivision mask weights are widened
/// before use so no precision is lost to intermediate f32 rounding.
fn build_stencil_matrix_f64(
    adapter:     &RefinerFaceAdapter<'_>,
    num_control: i32,
    out:         &mut Vec<f64>,
) {
    let refiner  = adapter.refiner;
    let nc       = num_control as usize;

    let num_refined = refiner.get_num_vertices_total() - num_control;

    let num_levels     = refiner.get_num_levels();
    let mut num_irreg  = 0i32;
    let irreg_size     = patch_cv_count(adapter.patch_builder.get_irreg_patch_type());

    let mut irreg_patches: Vec<PatchFace> = Vec::new();
    for level in 0..num_levels {
        let nf = refiner.get_level(level).get_num_faces();
        for face in 0..nf {
            if adapter.face_is_patch(level, face) && adapter.face_is_leaf(level, face)
                && !adapter.patch_is_regular(level, face)
            {
                irreg_patches.push(PatchFace::new(level, face, false));
                num_irreg += 1;
            }
        }
    }

    let total_rows = num_refined + num_irreg * irreg_size;
    if total_rows <= 0 { return; }

    out.clear();
    out.resize(total_rows as usize * nc, 0.0f64);

    // ---- Refined point rows via PrimvarRefiner ----
    // StencilRowF64 accumulates in f64; mask weights arrive as f32 and are
    // widened to f64 inside add_with_weight(), matching C++ PrimvarRefinerReal<double>.
    if num_levels > 1 && num_refined > 0 {
        let mut src_rows: Vec<StencilRowF64> = (0..nc)
            .map(|i| StencilRowF64::identity(nc, i))
            .collect();

        let primvar_refiner = PrimvarRefiner::new(refiner);

        let mut dst_rows: Vec<StencilRowF64> = Vec::new();

        for level in 1..num_levels {
            let n_child = refiner.get_level(level).get_num_vertices() as usize;
            dst_rows.clear();
            dst_rows.resize(n_child, StencilRowF64::new(nc));

            primvar_refiner.interpolate(level, &src_rows, &mut dst_rows);

            let level_vert_offset = adapter.level_offsets[level as usize] as usize;
            let row_base = level_vert_offset - nc;

            for (i, row) in dst_rows.iter().enumerate() {
                let dst = &mut out[(row_base + i) * nc..(row_base + i + 1) * nc];
                dst.copy_from_slice(&row.weights);
            }

            src_rows = dst_rows.clone();
        }
    }

    // ---- Irregular patch rows via conversion matrix ----
    if num_irreg > 0 {
        let stencil_base = num_refined as usize;

        for (patch_idx, pf) in irreg_patches.iter().enumerate() {
            let (conv_matrix, source_points) = adapter.irregular_patch_conversion(pf);

            append_conversion_stencils_f64(
                stencil_base + patch_idx * irreg_size as usize,
                &conv_matrix,
                &source_points,
                num_control as usize,
                out,
            );
        }
    }
}

/// Append stencil rows for one irregular patch's conversion matrix (f64).
///
/// Mirrors C++ `PatchTreeBuilder::appendConversionStencilsToMatrix<double>`.
/// Conversion matrix weights are f32 (that is what PatchBuilder produces,
/// matching C++) but every multiply/accumulate is promoted to f64 first.
fn append_conversion_stencils_f64(
    dst_row_base:  usize,
    conv:          &SparseMatrix<f32>,
    src_pts:       &[i32],
    nc:            usize,
    stencil:       &mut Vec<f64>,
) {
    let num_rows = conv.get_num_rows();

    for i in 0..num_rows {
        let dst_base = (dst_row_base + i as usize) * nc;

        for k in 0..nc { stencil[dst_base + k] = 0.0; }

        let indices = conv.get_row_columns(i);
        let weights = conv.get_row_elements(i);

        for j in 0..conv.get_row_size(i) as usize {
            let src_global = src_pts[indices[j] as usize] as usize;
            // Widen f32 weight to f64 before any arithmetic
            let w: f64     = weights[j] as f64;

            if src_global < nc {
                stencil[dst_base + src_global] += w;
            } else {
                let src_row  = src_global - nc;
                let src_base = src_row * nc;
                if src_base + nc <= stencil.len() {
                    // Avoid aliasing: copy src row then accumulate
                    let src_slice: Vec<f64> = stencil[src_base..src_base + nc].to_vec();
                    for k in 0..nc {
                        stencil[dst_base + k] += w * src_slice[k];
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
//  PatchTreeBuilder
// ---------------------------------------------------------------------------

/// Builds a `PatchTree` from a `FaceRefiner`.
///
/// Mirrors `Bfr::PatchTreeBuilder`.
pub struct PatchTreeBuilder {
    patch_tree:    Box<PatchTree>,
    options:       PatchTreeBuilderOptions,
    patch_faces:   Vec<PatchFace>,
    #[allow(dead_code)]
    level_offsets: Vec<i32>,
}

impl PatchTreeBuilder {
    /// Construct from a `FaceRefiner` and options.
    pub fn new(refiner: &dyn FaceRefiner, options: PatchTreeBuilderOptions) -> Self {
        let mut pt = Box::new(PatchTree::new());

        let root_face_size = refiner.root_face_size();
        let reg_face_size  = refiner.regular_face_size();

        pt.use_double_precision     = options.use_double_precision;
        pt.patches_include_non_leaf = options.include_interior_patches;
        pt.patches_are_triangular   = reg_face_size == 3;

        pt.reg_patch_type   = refiner.regular_patch_type();
        pt.irreg_patch_type = refiner.irregular_patch_type();

        pt.reg_patch_size    = patch_cv_count(pt.reg_patch_type);
        pt.irreg_patch_size  = patch_cv_count(pt.irreg_patch_type);
        pt.patch_point_stride = pt.reg_patch_size.max(pt.irreg_patch_size);

        pt.num_sub_faces      = if root_face_size == reg_face_size { 0 } else { root_face_size };
        pt.num_control_points = refiner.num_vertices_at(0);
        pt.num_refined_points = refiner.num_vertices_total() - pt.num_control_points;
        pt.num_sub_patch_points = pt.num_refined_points;

        let num_levels = refiner.num_levels() as usize;
        let mut offsets = vec![0i32; num_levels + 1];
        for i in 0..num_levels {
            offsets[i + 1] = offsets[i] + refiner.num_vertices_at(i as i32);
        }

        PatchTreeBuilder {
            patch_tree:   pt,
            options,
            patch_faces:  Vec::new(),
            level_offsets: offsets,
        }
    }

    /// Build the PatchTree and return it.
    pub fn build(mut self, refiner: &dyn FaceRefiner) -> Box<PatchTree> {
        self.identify_patches(refiner);
        self.initialize_patches(refiner);
        if self.options.use_double_precision {
            self.initialize_stencil_matrix_f64(refiner);
        } else {
            self.initialize_stencil_matrix_f32(refiner);
        }
        self.patch_tree.build_quadtree();
        self.patch_tree
    }

    pub fn get_patch_tree(&self) -> &PatchTree { &self.patch_tree }

    // -----------------------------------------------------------------------
    //  identify_patches — mirrors C++ identifyPatches()
    // -----------------------------------------------------------------------

    fn identify_patches(&mut self, refiner: &dyn FaceRefiner) {
        let inc_non_leaf = self.patch_tree.patches_include_non_leaf;
        self.patch_faces.clear();

        // Face 0 at base level is the root face
        if refiner.face_is_patch(0, 0) {
            if inc_non_leaf || refiner.face_is_leaf(0, 0) {
                let is_reg = refiner.patch_is_regular(0, 0);
                self.patch_faces.push(PatchFace::new(0, 0, is_reg));
            }
        }

        // Bug 2 guard: mirrors C++ PatchTreeBuilder::identifyPatches().
        // For triangular (Loop) schemes, adaptive refinement of a single root
        // face can generate child faces descended from neighbouring faces that
        // share vertices with the root.  Those impostor faces must be excluded.
        let test_base_face = refiner.needs_ancestor_test();

        let num_levels = refiner.num_levels();
        for level in 1..num_levels {
            let nf = refiner.num_faces_at(level);
            for face in 0..nf {
                // Skip faces not descended from the root face (face 0 at level 0)
                if test_base_face && !refiner.face_ancestor_is_root(level, face) {
                    continue;
                }
                if refiner.face_is_patch(level, face) {
                    if inc_non_leaf || refiner.face_is_leaf(level, face) {
                        let is_reg = refiner.patch_is_regular(level, face);
                        self.patch_faces.push(PatchFace::new(level, face, is_reg));
                    }
                }
            }
        }

        let n      = self.patch_faces.len();
        let stride = self.patch_tree.patch_point_stride as usize;
        self.patch_tree.patch_points.resize(n * stride, 0);
        self.patch_tree.patch_params.resize(n, Default::default());
        self.patch_tree.num_irreg_patches = self.patch_faces.iter()
            .filter(|p| !p.is_regular).count() as i32;

        // Irregular patches add local-point rows on top of the refined rows
        self.patch_tree.num_sub_patch_points +=
            self.patch_tree.num_irreg_patches * self.patch_tree.irreg_patch_size;
    }

    // -----------------------------------------------------------------------
    //  initialize_patches — mirrors C++ initializePatches()
    // -----------------------------------------------------------------------

    fn initialize_patches(&mut self, refiner: &dyn FaceRefiner) {
        let stride = self.patch_tree.patch_point_stride as usize;
        // Irregular patch local points come after all refined points
        let mut irreg_pt_base = self.patch_tree.num_control_points
            + self.patch_tree.num_refined_points;

        for (pi, pf) in self.patch_faces.iter().enumerate() {
            self.patch_tree.patch_params[pi] =
                refiner.patch_param(pf.level as i32, pf.face);

            let out = &mut self.patch_tree.patch_points[pi * stride..(pi + 1) * stride];

            if pf.is_regular {
                let n = refiner.gather_patch_points(pf.level as i32, pf.face, out);
                debug_assert!(n as usize <= stride);
            } else {
                // Assign sequential local-point indices for this irregular patch
                for j in 0..self.patch_tree.irreg_patch_size as usize {
                    out[j] = irreg_pt_base + j as i32;
                }
                irreg_pt_base += self.patch_tree.irreg_patch_size;
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Stencil matrix delegation
    // -----------------------------------------------------------------------

    fn initialize_stencil_matrix_f32(&mut self, refiner: &dyn FaceRefiner) {
        let nc = self.patch_tree.num_control_points;
        refiner.compute_stencil_matrix_f32(nc, &mut self.patch_tree.stencil_matrix_f32);
    }

    fn initialize_stencil_matrix_f64(&mut self, refiner: &dyn FaceRefiner) {
        let nc = self.patch_tree.num_control_points;
        refiner.compute_stencil_matrix_f64(nc, &mut self.patch_tree.stencil_matrix_f64);
    }
}

// ---------------------------------------------------------------------------
//  Helper: CV count per patch type
// ---------------------------------------------------------------------------

pub fn patch_cv_count(pt: PatchType) -> i32 {
    match pt {
        PatchType::Regular          => 16,
        PatchType::Loop             => 12,
        PatchType::Gregory
        | PatchType::GregoryBasis
        | PatchType::GregoryTriangle => 20,
        _                           =>  4, // linear / non-patch fallback
    }
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default() {
        let opts = PatchTreeBuilderOptions::default();
        assert_eq!(opts.max_patch_depth_sharp, 4);
        assert!(!opts.use_double_precision);
        assert!(!opts.include_interior_patches);
    }

    #[test]
    fn patch_face_stores_level_and_face() {
        let pf = PatchFace::new(3, 7, true);
        assert_eq!(pf.level, 3);
        assert_eq!(pf.face, 7);
        assert!(pf.is_regular);
    }

    #[test]
    fn patch_cv_count_regular_16() {
        assert_eq!(patch_cv_count(PatchType::Regular), 16);
    }

    #[test]
    fn patch_cv_count_loop_12() {
        assert_eq!(patch_cv_count(PatchType::Loop), 12);
    }
}
