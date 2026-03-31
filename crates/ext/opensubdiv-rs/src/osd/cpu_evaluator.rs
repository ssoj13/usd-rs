/// CPU evaluator — stateless stencil and patch evaluation.
///
/// Mirrors `Osd::CpuEvaluator` from OpenSubdiv 3.7.0 `osd/cpuEvaluator.h/.cpp`.
///
/// All methods are static (take no `&self`) — the evaluator holds no state.
/// Generic `SRC_BUFFER` and `DST_BUFFER` template parameters from C++ are
/// replaced by the `VertexBuffer` trait, and raw slice overloads are provided
/// for direct array access.
use crate::osd::{
    BufferDescriptor,
    types::{PatchCoord, PatchArray, PatchParam},
    cpu_kernel::{cpu_eval_stencils, cpu_eval_stencils_d1, cpu_eval_stencils_d2},
    cpu_vertex_buffer::VertexBuffer,
    patch_basis::{OsdPatchParam, osd_evaluate_patch_basis,
                  osd_evaluate_patch_basis_d1, osd_evaluate_patch_basis_d2},
};

// ---------------------------------------------------------------------------
//  Internal buffer adapter (mirrors C++ BufferAdapter<T>)
// ---------------------------------------------------------------------------

/// Adapts a raw `f32` pointer + length + stride for incremental weighted-sum
/// accumulation.  Mirrors the anonymous `BufferAdapter<T>` in cpuEvaluator.cpp.
struct BufferAdapter<'a> {
    data:   &'a mut [f32],
    length: usize,
    stride: usize,
    pos:    usize, // current element offset (in floats)
}

impl<'a> BufferAdapter<'a> {
    fn new(data: &'a mut [f32], desc: &BufferDescriptor) -> Self {
        let pos = desc.offset as usize;
        Self {
            data,
            length: desc.length as usize,
            stride: desc.stride as usize,
            pos,
        }
    }

    /// Zero current element.
    fn clear(&mut self) {
        for v in self.data[self.pos..self.pos + self.length].iter_mut() {
            *v = 0.0;
        }
    }

    /// Accumulate weighted source element.
    ///
    /// `src_stride` is the stride of the SOURCE buffer (may differ from
    /// the destination stride stored in `self.stride`).
    fn add_with_weight(&mut self, src: &[f32], src_index: usize, src_stride: usize, w: f32) {
        let src_off = src_index * src_stride;
        for k in 0..self.length {
            self.data[self.pos + k] += src[src_off + k] * w;
        }
    }

    /// Advance to the next output element.
    fn advance(&mut self) {
        self.pos += self.stride;
    }
}

/// Stateless CPU evaluator.
///
/// In C++ this is a class with only static methods.  In Rust we use a
/// unit struct so the same call patterns work.
pub struct CpuEvaluator;

impl CpuEvaluator {
    // -----------------------------------------------------------------------
    //  Stencil evaluation — raw slice API
    // -----------------------------------------------------------------------

    /// Evaluate stencils (position only) from raw slices.
    ///
    /// Mirrors the raw-pointer overload of `CpuEvaluator::EvalStencils`.
    #[doc(alias = "EvalStencils")]
    pub fn eval_stencils_raw(
        src:      &[f32],
        src_desc: &BufferDescriptor,
        dst:      &mut [f32],
        dst_desc: &BufferDescriptor,
        sizes:    &[i32],
        offsets:  &[i32],
        indices:  &[i32],
        weights:  &[f32],
        start: i32,
        end:   i32,
    ) -> bool {
        if end <= start { return true; }
        if src_desc.length != dst_desc.length { return false; }
        cpu_eval_stencils(
            src, src_desc, dst, dst_desc,
            sizes, offsets, indices, weights, start, end);
        true
    }

    /// Evaluate stencils with first derivatives from raw slices.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_stencils_d1_raw(
        src:        &[f32],
        src_desc:   &BufferDescriptor,
        dst:        &mut [f32],
        dst_desc:   &BufferDescriptor,
        dst_du:     &mut [f32],
        du_desc:    &BufferDescriptor,
        dst_dv:     &mut [f32],
        dv_desc:    &BufferDescriptor,
        sizes:      &[i32],
        offsets:    &[i32],
        indices:    &[i32],
        weights:    &[f32],
        du_weights: &[f32],
        dv_weights: &[f32],
        start: i32,
        end:   i32,
    ) -> bool {
        if end <= start { return true; }
        if src_desc.length != dst_desc.length { return false; }
        if src_desc.length != du_desc.length  { return false; }
        if src_desc.length != dv_desc.length  { return false; }
        cpu_eval_stencils_d1(
            src, src_desc, dst, dst_desc,
            dst_du, du_desc, dst_dv, dv_desc,
            sizes, offsets, indices,
            weights, du_weights, dv_weights,
            start, end);
        true
    }

    /// Evaluate stencils with first and second derivatives from raw slices.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_stencils_d2_raw(
        src:          &[f32],
        src_desc:     &BufferDescriptor,
        dst:          &mut [f32],
        dst_desc:     &BufferDescriptor,
        dst_du:       &mut [f32],
        du_desc:      &BufferDescriptor,
        dst_dv:       &mut [f32],
        dv_desc:      &BufferDescriptor,
        dst_duu:      &mut [f32],
        duu_desc:     &BufferDescriptor,
        dst_duv:      &mut [f32],
        duv_desc:     &BufferDescriptor,
        dst_dvv:      &mut [f32],
        dvv_desc:     &BufferDescriptor,
        sizes:        &[i32],
        offsets:      &[i32],
        indices:      &[i32],
        weights:      &[f32],
        du_weights:   &[f32],
        dv_weights:   &[f32],
        duu_weights:  &[f32],
        duv_weights:  &[f32],
        dvv_weights:  &[f32],
        start: i32,
        end:   i32,
    ) -> bool {
        if end <= start { return true; }
        for d in &[dst_desc, du_desc, dv_desc, duu_desc, duv_desc, dvv_desc] {
            if src_desc.length != d.length { return false; }
        }
        cpu_eval_stencils_d2(
            src, src_desc, dst, dst_desc,
            dst_du, du_desc, dst_dv, dv_desc,
            dst_duu, duu_desc, dst_duv, duv_desc, dst_dvv, dvv_desc,
            sizes, offsets, indices,
            weights, du_weights, dv_weights,
            duu_weights, duv_weights, dvv_weights,
            start, end);
        true
    }

    // -----------------------------------------------------------------------
    //  Stencil evaluation — VertexBuffer API
    // -----------------------------------------------------------------------

    /// Evaluate stencils via VertexBuffer objects.
    ///
    /// Mirrors `CpuEvaluator::EvalStencils(srcBuffer, srcDesc, dstBuffer, dstDesc,
    ///           stencilTable, instance, deviceContext)`.
    pub fn eval_stencils<SB: VertexBuffer, DB: VertexBuffer>(
        src_buf:  &SB,
        src_desc: &BufferDescriptor,
        dst_buf:  &mut DB,
        dst_desc: &BufferDescriptor,
        sizes:    &[i32],
        offsets:  &[i32],
        indices:  &[i32],
        weights:  &[f32],
        start: i32,
        end:   i32,
    ) -> bool {
        let src = src_buf.bind_cpu_buffer();
        let dst = dst_buf.bind_cpu_buffer_mut();
        Self::eval_stencils_raw(
            src, src_desc, dst, dst_desc,
            sizes, offsets, indices, weights, start, end)
    }

    // -----------------------------------------------------------------------
    //  Patch evaluation — raw slice API (position only)
    // -----------------------------------------------------------------------

    /// Evaluate patches at given parametric coordinates (position only).
    ///
    /// Mirrors `CpuEvaluator::EvalPatches(src, srcDesc, dst, dstDesc,
    ///           numPatchCoords, patchCoords, patchArrays,
    ///           patchIndexBuffer, patchParamBuffer)`.
    #[doc(alias = "EvalPatches")]
    pub fn eval_patches_raw(
        src:               &[f32],
        src_desc:          &BufferDescriptor,
        dst:               &mut [f32],
        dst_desc:          &BufferDescriptor,
        patch_coords:      &[PatchCoord],
        patch_arrays:      &[PatchArray],
        patch_index_buf:   &[i32],
        patch_param_buf:   &[PatchParam],
    ) -> bool {
        if src.len() <= src_desc.offset as usize { return false; }
        if dst.is_empty() { return false; }
        if src_desc.length != dst_desc.length { return false; }

        let src_s = &src[src_desc.offset as usize..];
        let src_stride = src_desc.stride as usize;
        let mut dst_a = BufferAdapter::new(dst, dst_desc);

        let mut wp = [0f32; 20];

        for coord in patch_coords {
            let array = &patch_arrays[coord.handle.array_index as usize];
            let pp    = &patch_param_buf[coord.handle.patch_index as usize];
            let osd_p = OsdPatchParam::new(pp.field0, pp.field1, pp.sharpness);

            let patch_type = if osd_p.is_regular() {
                array.get_patch_type_regular()
            } else {
                array.get_patch_type_irregular()
            };

            let n_pts = osd_evaluate_patch_basis(
                patch_type, &osd_p, coord.s, coord.t, &mut wp);

            let idx_base = array.get_index_base()
                + array.get_stride() * (coord.handle.patch_index - array.get_primitive_id_base());
            let cvs = &patch_index_buf[idx_base as usize..];

            dst_a.clear();
            for j in 0..n_pts as usize {
                dst_a.add_with_weight(src_s, cvs[j] as usize, src_stride, wp[j]);
            }
            dst_a.advance();
        }
        true
    }

    /// Evaluate patches with first derivatives.
    ///
    /// `dst_du`/`du_desc` and `dst_dv`/`dv_desc` are optional — pass `None`
    /// to skip writing that derivative output, matching C++ null-pointer semantics.
    pub fn eval_patches_d1_raw(
        src:             &[f32],
        src_desc:        &BufferDescriptor,
        dst:             &mut [f32],
        dst_desc:        &BufferDescriptor,
        dst_du:          Option<&mut [f32]>,
        du_desc:         Option<&BufferDescriptor>,
        dst_dv:          Option<&mut [f32]>,
        dv_desc:         Option<&BufferDescriptor>,
        patch_coords:    &[PatchCoord],
        patch_arrays:    &[PatchArray],
        patch_index_buf: &[i32],
        patch_param_buf: &[PatchParam],
    ) -> bool {
        if src.len() <= src_desc.offset as usize { return false; }
        if src_desc.length != dst_desc.length { return false; }
        if let Some(d) = du_desc { if src_desc.length != d.length { return false; } }
        if let Some(d) = dv_desc { if src_desc.length != d.length { return false; } }

        let src_s      = &src[src_desc.offset as usize..];
        let src_stride = src_desc.stride as usize;

        // Use a default descriptor for disabled outputs — the adapter will be
        // skipped when the corresponding buffer is None.
        let default_desc = BufferDescriptor::new(0, src_desc.length, src_desc.length);
        let du_d = du_desc.unwrap_or(&default_desc);
        let dv_d = dv_desc.unwrap_or(&default_desc);

        // Allocate scratch derivative buffers; only evaluated when needed.
        let compute_d1 = dst_du.is_some() || dst_dv.is_some();

        let mut wp  = [0f32; 20];
        let mut wds = [0f32; 20];
        let mut wdt = [0f32; 20];

        // We build local dst adapter and advance manually.
        let mut dst_off = dst_desc.offset as usize;
        let dst_stride  = dst_desc.stride as usize;
        let du_stride   = du_d.stride as usize;
        let dv_stride   = dv_d.stride as usize;
        let len         = src_desc.length as usize;

        // Shadow mutable outputs so we can index them in the loop.
        let mut du_buf = dst_du;
        let mut dv_buf = dst_dv;
        let mut du_off = du_d.offset as usize;
        let mut dv_off = dv_d.offset as usize;

        for coord in patch_coords {
            let array = &patch_arrays[coord.handle.array_index as usize];
            let pp    = &patch_param_buf[coord.handle.patch_index as usize];
            let osd_p = OsdPatchParam::new(pp.field0, pp.field1, pp.sharpness);

            let patch_type = if osd_p.is_regular() {
                array.get_patch_type_regular()
            } else {
                array.get_patch_type_irregular()
            };

            let n_pts = if compute_d1 {
                osd_evaluate_patch_basis_d1(
                    patch_type, &osd_p, coord.s, coord.t,
                    &mut wp, &mut wds, &mut wdt)
            } else {
                osd_evaluate_patch_basis(patch_type, &osd_p, coord.s, coord.t, &mut wp)
            };

            let idx_base = array.get_index_base()
                + array.get_stride() * (coord.handle.patch_index - array.get_primitive_id_base());
            let cvs = &patch_index_buf[idx_base as usize..];

            // Clear destination slots.
            for v in dst[dst_off..dst_off + len].iter_mut() { *v = 0.0; }
            if let Some(ref mut bu) = du_buf { for v in bu[du_off..du_off + len].iter_mut() { *v = 0.0; } }
            if let Some(ref mut bv) = dv_buf { for v in bv[dv_off..dv_off + len].iter_mut() { *v = 0.0; } }

            for j in 0..n_pts as usize {
                let cv      = cvs[j] as usize;
                let src_off = cv * src_stride;
                for k in 0..len {
                    dst[dst_off + k] += src_s[src_off + k] * wp[j];
                }
                if let Some(ref mut bu) = du_buf {
                    for k in 0..len { bu[du_off + k] += src_s[src_off + k] * wds[j]; }
                }
                if let Some(ref mut bv) = dv_buf {
                    for k in 0..len { bv[dv_off + k] += src_s[src_off + k] * wdt[j]; }
                }
            }
            dst_off += dst_stride;
            du_off  += du_stride;
            dv_off  += dv_stride;
        }
        true
    }

    /// Evaluate patches with first and second derivatives.
    ///
    /// All derivative outputs (`dst_du`/`dv`, `dst_duu`/`duv`/`dvv`) are optional.
    /// Pass `None` to skip writing that output, matching C++ null-pointer semantics.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_d2_raw(
        src:             &[f32],
        src_desc:        &BufferDescriptor,
        dst:             &mut [f32],
        dst_desc:        &BufferDescriptor,
        dst_du:          Option<&mut [f32]>,
        du_desc:         Option<&BufferDescriptor>,
        dst_dv:          Option<&mut [f32]>,
        dv_desc:         Option<&BufferDescriptor>,
        dst_duu:         Option<&mut [f32]>,
        duu_desc:        Option<&BufferDescriptor>,
        dst_duv:         Option<&mut [f32]>,
        duv_desc:        Option<&BufferDescriptor>,
        dst_dvv:         Option<&mut [f32]>,
        dvv_desc:        Option<&BufferDescriptor>,
        patch_coords:    &[PatchCoord],
        patch_arrays:    &[PatchArray],
        patch_index_buf: &[i32],
        patch_param_buf: &[PatchParam],
    ) -> bool {
        if src.len() <= src_desc.offset as usize { return false; }
        if src_desc.length != dst_desc.length { return false; }
        for d in [du_desc, dv_desc, duu_desc, duv_desc, dvv_desc].into_iter().flatten() {
            if src_desc.length != d.length { return false; }
        }

        let src_s      = &src[src_desc.offset as usize..];
        let src_stride = src_desc.stride as usize;
        let len        = src_desc.length as usize;

        let default_desc = BufferDescriptor::new(0, src_desc.length, src_desc.length);
        let du_d  = du_desc .unwrap_or(&default_desc);
        let dv_d  = dv_desc .unwrap_or(&default_desc);
        let duu_d = duu_desc.unwrap_or(&default_desc);
        let duv_d = duv_desc.unwrap_or(&default_desc);
        let dvv_d = dvv_desc.unwrap_or(&default_desc);

        let compute_d1 = dst_du.is_some() || dst_dv.is_some();
        let compute_d2 = dst_duu.is_some() || dst_duv.is_some() || dst_dvv.is_some();

        let mut wp   = [0f32; 20];
        let mut wds  = [0f32; 20];
        let mut wdt  = [0f32; 20];
        let mut wdss = [0f32; 20];
        let mut wdst = [0f32; 20];
        let mut wdtt = [0f32; 20];

        let mut dst_off = dst_desc.offset as usize;
        let mut du_off  = du_d.offset  as usize;
        let mut dv_off  = dv_d.offset  as usize;
        let mut duu_off = duu_d.offset as usize;
        let mut duv_off = duv_d.offset as usize;
        let mut dvv_off = dvv_d.offset as usize;

        let dst_stride = dst_desc.stride as usize;
        let du_stride  = du_d.stride  as usize;
        let dv_stride  = dv_d.stride  as usize;
        let duu_stride = duu_d.stride as usize;
        let duv_stride = duv_d.stride as usize;
        let dvv_stride = dvv_d.stride as usize;

        let mut du_buf  = dst_du;
        let mut dv_buf  = dst_dv;
        let mut duu_buf = dst_duu;
        let mut duv_buf = dst_duv;
        let mut dvv_buf = dst_dvv;

        for coord in patch_coords {
            let array = &patch_arrays[coord.handle.array_index as usize];
            let pp    = &patch_param_buf[coord.handle.patch_index as usize];
            let osd_p = OsdPatchParam::new(pp.field0, pp.field1, pp.sharpness);

            let patch_type = if osd_p.is_regular() {
                array.get_patch_type_regular()
            } else {
                array.get_patch_type_irregular()
            };

            let n_pts = if compute_d2 {
                osd_evaluate_patch_basis_d2(
                    patch_type, &osd_p, coord.s, coord.t,
                    &mut wp, &mut wds, &mut wdt,
                    &mut wdss, &mut wdst, &mut wdtt)
            } else if compute_d1 {
                osd_evaluate_patch_basis_d1(
                    patch_type, &osd_p, coord.s, coord.t,
                    &mut wp, &mut wds, &mut wdt)
            } else {
                osd_evaluate_patch_basis(patch_type, &osd_p, coord.s, coord.t, &mut wp)
            };

            let idx_base = array.get_index_base()
                + array.get_stride() * (coord.handle.patch_index - array.get_primitive_id_base());
            let cvs = &patch_index_buf[idx_base as usize..];

            // Clear destination slots.
            for v in dst[dst_off..dst_off + len].iter_mut() { *v = 0.0; }
            if let Some(ref mut b) = du_buf  { for v in b[du_off ..du_off  + len].iter_mut() { *v = 0.0; } }
            if let Some(ref mut b) = dv_buf  { for v in b[dv_off ..dv_off  + len].iter_mut() { *v = 0.0; } }
            if let Some(ref mut b) = duu_buf { for v in b[duu_off..duu_off + len].iter_mut() { *v = 0.0; } }
            if let Some(ref mut b) = duv_buf { for v in b[duv_off..duv_off + len].iter_mut() { *v = 0.0; } }
            if let Some(ref mut b) = dvv_buf { for v in b[dvv_off..dvv_off + len].iter_mut() { *v = 0.0; } }

            for j in 0..n_pts as usize {
                let cv      = cvs[j] as usize;
                let src_off = cv * src_stride;
                for k in 0..len { dst[dst_off + k] += src_s[src_off + k] * wp[j]; }
                if let Some(ref mut b) = du_buf  { for k in 0..len { b[du_off  + k] += src_s[src_off + k] * wds[j];  } }
                if let Some(ref mut b) = dv_buf  { for k in 0..len { b[dv_off  + k] += src_s[src_off + k] * wdt[j];  } }
                if let Some(ref mut b) = duu_buf { for k in 0..len { b[duu_off + k] += src_s[src_off + k] * wdss[j]; } }
                if let Some(ref mut b) = duv_buf { for k in 0..len { b[duv_off + k] += src_s[src_off + k] * wdst[j]; } }
                if let Some(ref mut b) = dvv_buf { for k in 0..len { b[dvv_off + k] += src_s[src_off + k] * wdtt[j]; } }
            }
            dst_off += dst_stride;
            du_off  += du_stride;
            dv_off  += dv_stride;
            duu_off += duu_stride;
            duv_off += duv_stride;
            dvv_off += dvv_stride;
        }
        true
    }

    // -----------------------------------------------------------------------
    //  VertexBuffer patch API
    // -----------------------------------------------------------------------

    /// Evaluate patches via VertexBuffer objects (position only).
    ///
    /// Mirrors C++ `CpuEvaluator::EvalPatches(srcBuffer, srcDesc, dstBuffer, dstDesc, ...)`.
    /// `src_buf` and `dst_buf` are required to be distinct objects; a borrow
    /// split is safe because `SB` and `DB` are different types.
    pub fn eval_patches<SB: VertexBuffer, DB: VertexBuffer>(
        src_buf:         &SB,
        src_desc:        &BufferDescriptor,
        dst_buf:         &mut DB,
        dst_desc:        &BufferDescriptor,
        patch_coords:    &[PatchCoord],
        patch_arrays:    &[PatchArray],
        patch_index_buf: &[i32],
        patch_param_buf: &[PatchParam],
    ) -> bool {
        // Bind src first (shared borrow), then dst (exclusive borrow on a
        // distinct object).  No clone needed when SB != DB, which is always
        // true in practice (C++ also assumes src != dst).
        let src = src_buf.bind_cpu_buffer();
        let dst = dst_buf.bind_cpu_buffer_mut();
        Self::eval_patches_raw(
            src, src_desc, dst, dst_desc,
            patch_coords, patch_arrays, patch_index_buf, patch_param_buf)
    }

    // -----------------------------------------------------------------------
    //  Face-varying patch evaluation (position only)
    // -----------------------------------------------------------------------

    /// Evaluate face-varying patches at given parametric locations (position only).
    ///
    /// Mirrors `CpuEvaluator::EvalPatchesFaceVarying(srcBuffer, srcDesc, dstBuffer, dstDesc,
    ///           numPatchCoords, patchCoords, patchTable, fvarChannel, ...)`.
    /// Delegates to `eval_patches_raw` using the FVar patch arrays/indices/params.
    pub fn eval_patches_face_varying_raw(
        src:             &[f32],
        src_desc:        &BufferDescriptor,
        dst:             &mut [f32],
        dst_desc:        &BufferDescriptor,
        patch_coords:    &[PatchCoord],
        fvar_arrays:     &[PatchArray],
        fvar_index_buf:  &[i32],
        fvar_param_buf:  &[PatchParam],
    ) -> bool {
        Self::eval_patches_raw(
            src, src_desc, dst, dst_desc,
            patch_coords, fvar_arrays, fvar_index_buf, fvar_param_buf,
        )
    }

    /// Evaluate face-varying patches with first derivatives.
    ///
    /// Derivative outputs are optional: pass `None` for any output to skip it.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_face_varying_d1_raw(
        src:             &[f32],
        src_desc:        &BufferDescriptor,
        dst:             &mut [f32],
        dst_desc:        &BufferDescriptor,
        dst_du:          Option<&mut [f32]>,
        du_desc:         Option<&BufferDescriptor>,
        dst_dv:          Option<&mut [f32]>,
        dv_desc:         Option<&BufferDescriptor>,
        patch_coords:    &[PatchCoord],
        fvar_arrays:     &[PatchArray],
        fvar_index_buf:  &[i32],
        fvar_param_buf:  &[PatchParam],
    ) -> bool {
        Self::eval_patches_d1_raw(
            src, src_desc, dst, dst_desc,
            dst_du, du_desc, dst_dv, dv_desc,
            patch_coords, fvar_arrays, fvar_index_buf, fvar_param_buf,
        )
    }

    /// Evaluate face-varying patches with first and second derivatives.
    ///
    /// All derivative outputs are optional: pass `None` for any output to skip it.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_face_varying_d2_raw(
        src:             &[f32],
        src_desc:        &BufferDescriptor,
        dst:             &mut [f32],
        dst_desc:        &BufferDescriptor,
        dst_du:          Option<&mut [f32]>,
        du_desc:         Option<&BufferDescriptor>,
        dst_dv:          Option<&mut [f32]>,
        dv_desc:         Option<&BufferDescriptor>,
        dst_duu:         Option<&mut [f32]>,
        duu_desc:        Option<&BufferDescriptor>,
        dst_duv:         Option<&mut [f32]>,
        duv_desc:        Option<&BufferDescriptor>,
        dst_dvv:         Option<&mut [f32]>,
        dvv_desc:        Option<&BufferDescriptor>,
        patch_coords:    &[PatchCoord],
        fvar_arrays:     &[PatchArray],
        fvar_index_buf:  &[i32],
        fvar_param_buf:  &[PatchParam],
    ) -> bool {
        Self::eval_patches_d2_raw(
            src, src_desc, dst, dst_desc,
            dst_du, du_desc, dst_dv, dv_desc,
            dst_duu, duu_desc, dst_duv, duv_desc, dst_dvv, dvv_desc,
            patch_coords, fvar_arrays, fvar_index_buf, fvar_param_buf,
        )
    }

    // -----------------------------------------------------------------------
    //  Varying patch evaluation
    // -----------------------------------------------------------------------

    /// Evaluate varying patches at given parametric coordinates (position only).
    ///
    /// Mirrors `CpuEvaluator::EvalPatchesVarying(srcBuffer, srcDesc, dstBuffer,
    /// dstDesc, numPatchCoords, patchCoords, patchTable, ...)` which delegates to
    /// `EvalPatches` using `GetVaryingPatchArrayBuffer` / `GetVaryingPatchIndexBuffer`.
    pub fn eval_patches_varying_raw(
        src:                  &[f32],
        src_desc:             &BufferDescriptor,
        dst:                  &mut [f32],
        dst_desc:             &BufferDescriptor,
        patch_coords:         &[PatchCoord],
        varying_arrays:       &[PatchArray],
        varying_index_buf:    &[i32],
        patch_param_buf:      &[PatchParam],
    ) -> bool {
        // Varying evaluation reuses the same kernel as vertex patches — the only
        // difference is which index/array buffer is used (varying vs vertex).
        Self::eval_patches_raw(
            src, src_desc, dst, dst_desc,
            patch_coords, varying_arrays, varying_index_buf, patch_param_buf,
        )
    }

    /// Evaluate varying patches with first derivatives.
    ///
    /// Derivative outputs are optional: pass `None` for any output to skip it.
    /// Mirrors `CpuEvaluator::EvalPatchesVarying(…, duBuffer, duDesc, dvBuffer,
    /// dvDesc, …)` overload.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_varying_d1_raw(
        src:               &[f32],
        src_desc:          &BufferDescriptor,
        dst:               &mut [f32],
        dst_desc:          &BufferDescriptor,
        dst_du:            Option<&mut [f32]>,
        du_desc:           Option<&BufferDescriptor>,
        dst_dv:            Option<&mut [f32]>,
        dv_desc:           Option<&BufferDescriptor>,
        patch_coords:      &[PatchCoord],
        varying_arrays:    &[PatchArray],
        varying_index_buf: &[i32],
        patch_param_buf:   &[PatchParam],
    ) -> bool {
        Self::eval_patches_d1_raw(
            src, src_desc, dst, dst_desc,
            dst_du, du_desc, dst_dv, dv_desc,
            patch_coords, varying_arrays, varying_index_buf, patch_param_buf,
        )
    }

    /// Evaluate varying patches with first and second derivatives.
    ///
    /// All derivative outputs are optional.
    /// Mirrors `CpuEvaluator::EvalPatchesVarying(…, duuBuffer, …)` overload.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_varying_d2_raw(
        src:               &[f32],
        src_desc:          &BufferDescriptor,
        dst:               &mut [f32],
        dst_desc:          &BufferDescriptor,
        dst_du:            Option<&mut [f32]>,
        du_desc:           Option<&BufferDescriptor>,
        dst_dv:            Option<&mut [f32]>,
        dv_desc:           Option<&BufferDescriptor>,
        dst_duu:           Option<&mut [f32]>,
        duu_desc:          Option<&BufferDescriptor>,
        dst_duv:           Option<&mut [f32]>,
        duv_desc:          Option<&BufferDescriptor>,
        dst_dvv:           Option<&mut [f32]>,
        dvv_desc:          Option<&BufferDescriptor>,
        patch_coords:      &[PatchCoord],
        varying_arrays:    &[PatchArray],
        varying_index_buf: &[i32],
        patch_param_buf:   &[PatchParam],
    ) -> bool {
        Self::eval_patches_d2_raw(
            src, src_desc, dst, dst_desc,
            dst_du, du_desc, dst_dv, dv_desc,
            dst_duu, duu_desc, dst_duv, duv_desc, dst_dvv, dvv_desc,
            patch_coords, varying_arrays, varying_index_buf, patch_param_buf,
        )
    }

    // -----------------------------------------------------------------------
    //  StencilTable-parameterised eval_stencils overloads
    // -----------------------------------------------------------------------

    /// Evaluate stencils directly from a `StencilTable` object (position only).
    ///
    /// Mirrors the C++ template overload:
    /// ```cpp
    /// EvalStencils(srcBuffer, srcDesc, dstBuffer, dstDesc, stencilTable, …)
    /// ```
    /// which calls `stencilTable->GetSizes/Offsets/Indices/Weights()` then
    /// delegates to the raw-pointer overload.
    pub fn eval_stencils_with_table<SB: VertexBuffer, DB: VertexBuffer>(
        src_buf:       &SB,
        src_desc:      &BufferDescriptor,
        dst_buf:       &mut DB,
        dst_desc:      &BufferDescriptor,
        stencil_table: &crate::far::StencilTable,
    ) -> bool {
        let n = stencil_table.get_num_stencils();
        if n == 0 { return false; }
        Self::eval_stencils(
            src_buf, src_desc, dst_buf, dst_desc,
            stencil_table.sizes(),
            stencil_table.offsets(),
            stencil_table.indices(),
            stencil_table.weights(),
            0, n,
        )
    }

    // -----------------------------------------------------------------------
    //  Synchronize (no-op for CPU)
    // -----------------------------------------------------------------------

    /// CPU evaluator is synchronous — this is a no-op.
    pub fn synchronize() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osd::cpu_vertex_buffer::CpuVertexBuffer;

    fn uniform(length: i32) -> BufferDescriptor {
        BufferDescriptor::new(0, length, length)
    }

    #[test]
    fn eval_stencils_identity() {
        let mut src = CpuVertexBuffer::new(3, 2);
        src.update_data(&[1.0, 2.0, 3.0,  4.0, 5.0, 6.0], 0, 2);

        let mut dst = CpuVertexBuffer::new(3, 2);
        let d = uniform(3);

        let ok = CpuEvaluator::eval_stencils(
            &src, &d, &mut dst, &d,
            &[1, 1], &[0, 1], &[0, 1], &[1.0, 1.0], 0, 2);

        assert!(ok);
        let buf = dst.bind_cpu_buffer();
        assert!((buf[0] - 1.0).abs() < 1e-6);
        assert!((buf[3] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn eval_stencils_empty_range() {
        let src = CpuVertexBuffer::new(3, 1);
        let mut dst = CpuVertexBuffer::new(3, 1);
        let d = uniform(3);
        // start == end → returns true immediately
        let ok = CpuEvaluator::eval_stencils(
            &src, &d, &mut dst, &d,
            &[], &[], &[], &[], 0, 0);
        assert!(ok);
    }

    #[test]
    fn eval_stencils_length_mismatch() {
        let src = CpuVertexBuffer::new(3, 1);
        let mut dst = CpuVertexBuffer::new(2, 1);
        let src_d = uniform(3);
        let dst_d = uniform(2);
        let ok = CpuEvaluator::eval_stencils(
            &src, &src_d, &mut dst, &dst_d,
            &[1], &[0], &[0], &[1.0], 0, 1);
        assert!(!ok); // length mismatch
    }
}
