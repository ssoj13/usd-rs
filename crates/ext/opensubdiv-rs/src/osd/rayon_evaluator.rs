/// Parallel CPU evaluator using Rayon — mirrors `Osd::TbbEvaluator` pattern.
///
/// Same API as `CpuEvaluator` but stencil loops are parallelised with
/// `rayon::par_chunks`.  Gated behind `#[cfg(feature = "parallel")]`.
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::osd::{
    BufferDescriptor,
    cpu_evaluator::CpuEvaluator,
    cpu_vertex_buffer::VertexBuffer,
    patch_basis::{
        OsdPatchParam, osd_evaluate_patch_basis, osd_evaluate_patch_basis_d1,
        osd_evaluate_patch_basis_d2,
    },
    types::{PatchArray, PatchCoord, PatchParam},
};

/// Parallel evaluator — stateless, same interface as `CpuEvaluator`.
pub struct RayonEvaluator;

impl RayonEvaluator {
    // -----------------------------------------------------------------------
    //  Stencil evaluation
    // -----------------------------------------------------------------------

    /// Evaluate stencils in parallel using Rayon.
    ///
    /// Falls back to the single-threaded kernel for small workloads
    /// (< `PARALLEL_THRESHOLD` stencils).
    pub fn eval_stencils_raw(
        src: &[f32],
        src_desc: &BufferDescriptor,
        dst: &mut [f32],
        dst_desc: &BufferDescriptor,
        sizes: &[i32],
        offsets: &[i32],
        indices: &[i32],
        weights: &[f32],
        start: i32,
        end: i32,
    ) -> bool {
        if end <= start {
            return true;
        }
        if src_desc.length != dst_desc.length {
            return false;
        }

        const PARALLEL_THRESHOLD: i32 = 512;
        if end - start < PARALLEL_THRESHOLD {
            // Small workloads don't benefit from thread overhead
            return CpuEvaluator::eval_stencils_raw(
                src, src_desc, dst, dst_desc, sizes, offsets, indices, weights, start, end,
            );
        }

        let len = src_desc.length as usize;
        let n = (end - start) as usize;

        let sizes_s = &sizes[start as usize..start as usize + n];
        let src_base = &src[src_desc.offset as usize..];
        let stride = dst_desc.stride as usize;
        let dst_base_off = dst_desc.offset as usize;
        let src_stride = src_desc.stride as usize;

        // Build per-stencil (index_offset, stencil_size) pairs so each
        // Rayon task can work independently on its index range.
        let mut stencil_offsets = Vec::with_capacity(n);
        let base_offset = if start > 0 {
            offsets[start as usize] as usize
        } else {
            0
        };
        let mut cursor = base_offset;
        for &sz in sizes_s {
            stencil_offsets.push((cursor, sz as usize));
            cursor += sz as usize;
        }

        // Partition dst into n non-overlapping stride-sized chunks and evaluate
        // each chunk in parallel.  This avoids the O(n * len) intermediate
        // Vec<Vec<f32>> allocation from the previous collect-then-scatter approach.
        //
        // par_chunks_mut gives Rayon direct mutable access to disjoint slices,
        // so no unsafe is required.
        let dst_region = &mut dst[dst_base_off..dst_base_off + n * stride];
        dst_region
            .par_chunks_mut(stride)
            .zip(stencil_offsets.par_iter())
            .for_each(|(chunk, &(off, sz))| {
                // Zero the destination element then accumulate weights.
                for v in chunk[..len].iter_mut() {
                    *v = 0.0;
                }
                for j in 0..sz {
                    let cv = indices[off + j] as usize;
                    let w = weights[off + j];
                    let src_off = cv * src_stride;
                    for k in 0..len {
                        chunk[k] += src_base[src_off + k] * w;
                    }
                }
            });

        true
    }

    /// Evaluate stencils with first derivatives in parallel.
    ///
    /// Mirrors `TbbEvaluator::EvalStencils` with du/dv outputs.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_stencils_d1_raw(
        src: &[f32],
        src_desc: &BufferDescriptor,
        dst: &mut [f32],
        dst_desc: &BufferDescriptor,
        dst_du: &mut [f32],
        du_desc: &BufferDescriptor,
        dst_dv: &mut [f32],
        dv_desc: &BufferDescriptor,
        sizes: &[i32],
        offsets: &[i32],
        indices: &[i32],
        weights: &[f32],
        du_weights: &[f32],
        dv_weights: &[f32],
        start: i32,
        end: i32,
    ) -> bool {
        if end <= start {
            return true;
        }
        if src_desc.length != dst_desc.length {
            return false;
        }
        if src_desc.length != du_desc.length {
            return false;
        }
        if src_desc.length != dv_desc.length {
            return false;
        }

        const PARALLEL_THRESHOLD: i32 = 512;
        if end - start < PARALLEL_THRESHOLD {
            return CpuEvaluator::eval_stencils_d1_raw(
                src, src_desc, dst, dst_desc, dst_du, du_desc, dst_dv, dv_desc, sizes, offsets,
                indices, weights, du_weights, dv_weights, start, end,
            );
        }

        let len = src_desc.length as usize;
        let n = (end - start) as usize;
        let sizes_s = &sizes[start as usize..start as usize + n];
        let src_base = &src[src_desc.offset as usize..];

        let base_offset = if start > 0 {
            offsets[start as usize] as usize
        } else {
            0
        };
        let mut stencil_offsets = Vec::with_capacity(n);
        let mut cursor = base_offset;
        for &sz in sizes_s {
            stencil_offsets.push((cursor, sz as usize));
            cursor += sz as usize;
        }

        let src_stride = src_desc.stride as usize;
        let stride_p = dst_desc.stride as usize;
        let stride_u = du_desc.stride as usize;
        let stride_v = dv_desc.stride as usize;
        let dst_base = dst_desc.offset as usize;
        let du_base = du_desc.offset as usize;
        let dv_base = dv_desc.offset as usize;

        // Write directly into the three output buffers using pre-computed
        // per-stencil index offsets.  Three-way par_chunks_mut is not
        // possible directly; use par_iter + enumerate with a single
        // per-stencil stack-size accumulator to avoid heap allocation.
        //
        // Safety: each stencil index `i` accesses a disjoint range of each
        // output buffer (offset + i*stride .. + len), so concurrent writes
        // are safe.  We use unsafe pointers to bypass the borrow-checker
        // restriction on multiple mutable aliases of distinct sub-slices.
        let dst_ptr = dst.as_mut_ptr();
        let du_ptr = dst_du.as_mut_ptr();
        let dv_ptr = dst_dv.as_mut_ptr();
        // SAFETY: Each stencil writes to `[base + i*stride, base + i*stride + len)`.
        // These ranges are disjoint across stencil indices, so no data race occurs.
        stencil_offsets
            .par_iter()
            .enumerate()
            .for_each(|(i, &(off, sz))| {
                let mut acc = [0.0f32; 20]; // max stencil width in patch eval
                let mut acc_du = [0.0f32; 20];
                let mut acc_dv = [0.0f32; 20];
                let acc = &mut acc[..len];
                let acc_du = &mut acc_du[..len];
                let acc_dv = &mut acc_dv[..len];
                for j in 0..sz {
                    let cv = indices[off + j] as usize;
                    let w = weights[off + j];
                    let wu = du_weights[off + j];
                    let wv = dv_weights[off + j];
                    let src_off = cv * src_stride;
                    for k in 0..len {
                        let v = src_base[src_off + k];
                        acc[k] += v * w;
                        acc_du[k] += v * wu;
                        acc_dv[k] += v * wv;
                    }
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        acc.as_ptr(),
                        dst_ptr.add(dst_base + i * stride_p),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        acc_du.as_ptr(),
                        du_ptr.add(du_base + i * stride_u),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        acc_dv.as_ptr(),
                        dv_ptr.add(dv_base + i * stride_v),
                        len,
                    );
                }
            });
        true
    }

    /// Evaluate stencils with first and second derivatives in parallel.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_stencils_d2_raw(
        src: &[f32],
        src_desc: &BufferDescriptor,
        dst: &mut [f32],
        dst_desc: &BufferDescriptor,
        dst_du: &mut [f32],
        du_desc: &BufferDescriptor,
        dst_dv: &mut [f32],
        dv_desc: &BufferDescriptor,
        dst_duu: &mut [f32],
        duu_desc: &BufferDescriptor,
        dst_duv: &mut [f32],
        duv_desc: &BufferDescriptor,
        dst_dvv: &mut [f32],
        dvv_desc: &BufferDescriptor,
        sizes: &[i32],
        offsets: &[i32],
        indices: &[i32],
        weights: &[f32],
        du_weights: &[f32],
        dv_weights: &[f32],
        duu_weights: &[f32],
        duv_weights: &[f32],
        dvv_weights: &[f32],
        start: i32,
        end: i32,
    ) -> bool {
        if end <= start {
            return true;
        }
        for d in &[dst_desc, du_desc, dv_desc, duu_desc, duv_desc, dvv_desc] {
            if src_desc.length != d.length {
                return false;
            }
        }

        const PARALLEL_THRESHOLD: i32 = 512;
        if end - start < PARALLEL_THRESHOLD {
            return CpuEvaluator::eval_stencils_d2_raw(
                src,
                src_desc,
                dst,
                dst_desc,
                dst_du,
                du_desc,
                dst_dv,
                dv_desc,
                dst_duu,
                duu_desc,
                dst_duv,
                duv_desc,
                dst_dvv,
                dvv_desc,
                sizes,
                offsets,
                indices,
                weights,
                du_weights,
                dv_weights,
                duu_weights,
                duv_weights,
                dvv_weights,
                start,
                end,
            );
        }

        let len = src_desc.length as usize;
        let n = (end - start) as usize;
        let sizes_s = &sizes[start as usize..start as usize + n];
        let src_base = &src[src_desc.offset as usize..];

        let base_offset = if start > 0 {
            offsets[start as usize] as usize
        } else {
            0
        };
        let mut stencil_offsets = Vec::with_capacity(n);
        let mut cursor = base_offset;
        for &sz in sizes_s {
            stencil_offsets.push((cursor, sz as usize));
            cursor += sz as usize;
        }

        let src_stride = src_desc.stride as usize;
        let stride_p = dst_desc.stride as usize;
        let stride_u = du_desc.stride as usize;
        let stride_v = dv_desc.stride as usize;
        let stride_uu = duu_desc.stride as usize;
        let stride_uv = duv_desc.stride as usize;
        let stride_vv = dvv_desc.stride as usize;
        let base_p = dst_desc.offset as usize;
        let base_u = du_desc.offset as usize;
        let base_v = dv_desc.offset as usize;
        let base_uu = duu_desc.offset as usize;
        let base_uv = duv_desc.offset as usize;
        let base_vv = dvv_desc.offset as usize;

        // Avoid O(n * 6 * len) heap allocation by using per-stencil stack
        // accumulators and writing directly into the output buffers.
        let dst_ptr = dst.as_mut_ptr();
        let du_ptr = dst_du.as_mut_ptr();
        let dv_ptr = dst_dv.as_mut_ptr();
        let duu_ptr = dst_duu.as_mut_ptr();
        let duv_ptr = dst_duv.as_mut_ptr();
        let dvv_ptr = dst_dvv.as_mut_ptr();
        // SAFETY: stencil i writes to disjoint range [base + i*stride, +len)
        // in each output buffer; no data race across parallel stencil tasks.
        stencil_offsets
            .par_iter()
            .enumerate()
            .for_each(|(i, &(off, sz))| {
                let mut a = [0.0f32; 20];
                let mut a_du = [0.0f32; 20];
                let mut a_dv = [0.0f32; 20];
                let mut a_duu = [0.0f32; 20];
                let mut a_duv = [0.0f32; 20];
                let mut a_dvv = [0.0f32; 20];
                let a = &mut a[..len];
                let a_du = &mut a_du[..len];
                let a_dv = &mut a_dv[..len];
                let a_duu = &mut a_duu[..len];
                let a_duv = &mut a_duv[..len];
                let a_dvv = &mut a_dvv[..len];
                for j in 0..sz {
                    let cv = indices[off + j] as usize;
                    let w = weights[off + j];
                    let wu = du_weights[off + j];
                    let wv = dv_weights[off + j];
                    let wuu = duu_weights[off + j];
                    let wuv = duv_weights[off + j];
                    let wvv = dvv_weights[off + j];
                    let src_off = cv * src_stride;
                    for k in 0..len {
                        let v = src_base[src_off + k];
                        a[k] += v * w;
                        a_du[k] += v * wu;
                        a_dv[k] += v * wv;
                        a_duu[k] += v * wuu;
                        a_duv[k] += v * wuv;
                        a_dvv[k] += v * wvv;
                    }
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        a.as_ptr(),
                        dst_ptr.add(base_p + i * stride_p),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        a_du.as_ptr(),
                        du_ptr.add(base_u + i * stride_u),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        a_dv.as_ptr(),
                        dv_ptr.add(base_v + i * stride_v),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        a_duu.as_ptr(),
                        duu_ptr.add(base_uu + i * stride_uu),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        a_duv.as_ptr(),
                        duv_ptr.add(base_uv + i * stride_uv),
                        len,
                    );
                    std::ptr::copy_nonoverlapping(
                        a_dvv.as_ptr(),
                        dvv_ptr.add(base_vv + i * stride_vv),
                        len,
                    );
                }
            });
        true
    }

    /// Evaluate stencils via VertexBuffer objects in parallel.
    pub fn eval_stencils<SB: VertexBuffer, DB: VertexBuffer>(
        src_buf: &SB,
        src_desc: &BufferDescriptor,
        dst_buf: &mut DB,
        dst_desc: &BufferDescriptor,
        sizes: &[i32],
        offsets: &[i32],
        indices: &[i32],
        weights: &[f32],
        start: i32,
        end: i32,
    ) -> bool {
        let src = src_buf.bind_cpu_buffer().to_vec();
        let dst = dst_buf.bind_cpu_buffer_mut();
        Self::eval_stencils_raw(
            &src, src_desc, dst, dst_desc, sizes, offsets, indices, weights, start, end,
        )
    }

    // -----------------------------------------------------------------------
    //  Patch evaluation (parallelised over coords)
    // -----------------------------------------------------------------------

    /// Evaluate patches in parallel.  Each coord is evaluated independently.
    pub fn eval_patches_raw(
        src: &[f32],
        src_desc: &BufferDescriptor,
        dst: &mut [f32],
        dst_desc: &BufferDescriptor,
        patch_coords: &[PatchCoord],
        patch_arrays: &[PatchArray],
        patch_index_buf: &[i32],
        patch_param_buf: &[PatchParam],
    ) -> bool {
        if src.len() <= src_desc.offset as usize {
            return false;
        }
        if src_desc.length != dst_desc.length {
            return false;
        }

        let len = src_desc.length as usize;
        let src_base = &src[src_desc.offset as usize..];
        let src_stride = src_desc.stride as usize;
        let dst_base = dst_desc.offset as usize;
        let stride = dst_desc.stride as usize;

        // Write directly into dst via par_chunks_mut — no intermediate allocation.
        let dst_region = &mut dst[dst_base..dst_base + patch_coords.len() * stride];
        dst_region
            .par_chunks_mut(stride)
            .zip(patch_coords.par_iter())
            .for_each(|(chunk, coord)| {
                let array = &patch_arrays[coord.handle.array_index as usize];
                let pp = &patch_param_buf[coord.handle.patch_index as usize];
                let osd_p = OsdPatchParam::new(pp.field0, pp.field1, pp.sharpness);

                let patch_type = if osd_p.is_regular() {
                    array.get_patch_type_regular()
                } else {
                    array.get_patch_type_irregular()
                };

                let mut wp = [0f32; 20];
                let n_pts = osd_evaluate_patch_basis(patch_type, &osd_p, coord.s, coord.t, &mut wp);

                let idx_base = array.get_index_base()
                    + array.get_stride()
                        * (coord.handle.patch_index - array.get_primitive_id_base());
                let cvs = &patch_index_buf[idx_base as usize..];

                for v in chunk[..len].iter_mut() {
                    *v = 0.0;
                }
                for j in 0..n_pts as usize {
                    let cv = cvs[j] as usize;
                    let off = cv * src_stride;
                    for k in 0..len {
                        chunk[k] += src_base[off + k] * wp[j];
                    }
                }
            });

        true
    }

    /// Evaluate patches with first derivatives in parallel.
    ///
    /// Derivative outputs are optional: pass `None` to skip writing that output.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_d1_raw(
        src: &[f32],
        src_desc: &BufferDescriptor,
        dst: &mut [f32],
        dst_desc: &BufferDescriptor,
        dst_du: Option<&mut [f32]>,
        du_desc: Option<&BufferDescriptor>,
        dst_dv: Option<&mut [f32]>,
        dv_desc: Option<&BufferDescriptor>,
        patch_coords: &[PatchCoord],
        patch_arrays: &[PatchArray],
        patch_index_buf: &[i32],
        patch_param_buf: &[PatchParam],
    ) -> bool {
        if src.len() <= src_desc.offset as usize {
            return false;
        }
        if src_desc.length != dst_desc.length {
            return false;
        }
        if let Some(d) = du_desc {
            if src_desc.length != d.length {
                return false;
            }
        }
        if let Some(d) = dv_desc {
            if src_desc.length != d.length {
                return false;
            }
        }

        let len = src_desc.length as usize;
        let src_base = &src[src_desc.offset as usize..];

        let results: Vec<(Vec<f32>, Vec<f32>, Vec<f32>)> = patch_coords
            .par_iter()
            .map(|coord| {
                let array = &patch_arrays[coord.handle.array_index as usize];
                let pp = &patch_param_buf[coord.handle.patch_index as usize];
                let osd_p = OsdPatchParam::new(pp.field0, pp.field1, pp.sharpness);
                let patch_type = if osd_p.is_regular() {
                    array.get_patch_type_regular()
                } else {
                    array.get_patch_type_irregular()
                };

                let mut wp = [0f32; 20];
                let mut wds = [0f32; 20];
                let mut wdt = [0f32; 20];
                let n_pts = osd_evaluate_patch_basis_d1(
                    patch_type, &osd_p, coord.s, coord.t, &mut wp, &mut wds, &mut wdt,
                );

                let idx_base = array.get_index_base()
                    + array.get_stride()
                        * (coord.handle.patch_index - array.get_primitive_id_base());
                let cvs = &patch_index_buf[idx_base as usize..];

                let mut acc = vec![0.0f32; len];
                let mut acc_du = vec![0.0f32; len];
                let mut acc_dv = vec![0.0f32; len];
                for j in 0..n_pts as usize {
                    let cv = cvs[j] as usize;
                    let off = cv * src_desc.stride as usize;
                    for k in 0..len {
                        let v = src_base[off + k];
                        acc[k] += v * wp[j];
                        acc_du[k] += v * wds[j];
                        acc_dv[k] += v * wdt[j];
                    }
                }
                (acc, acc_du, acc_dv)
            })
            .collect();

        let default_desc = BufferDescriptor::new(0, src_desc.length, src_desc.length);
        let du_d = du_desc.unwrap_or(&default_desc);
        let dv_d = dv_desc.unwrap_or(&default_desc);

        let dst_base = dst_desc.offset as usize;
        let du_base = du_d.offset as usize;
        let dv_base = dv_d.offset as usize;
        let stride_p = dst_desc.stride as usize;
        let stride_u = du_d.stride as usize;
        let stride_v = dv_d.stride as usize;
        for (i, (r, ru, rv)) in results.iter().enumerate() {
            let po = dst_base + i * stride_p;
            dst[po..po + len].copy_from_slice(r);
            if let Some(ref mut b) = dst_du {
                let uo = du_base + i * stride_u;
                b[uo..uo + len].copy_from_slice(ru);
            }
            if let Some(ref mut b) = dst_dv {
                let vo = dv_base + i * stride_v;
                b[vo..vo + len].copy_from_slice(rv);
            }
        }
        true
    }

    /// Evaluate patches with first and second derivatives in parallel.
    ///
    /// All derivative outputs are optional: pass `None` to skip writing that output.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_patches_d2_raw(
        src: &[f32],
        src_desc: &BufferDescriptor,
        dst: &mut [f32],
        dst_desc: &BufferDescriptor,
        dst_du: Option<&mut [f32]>,
        du_desc: Option<&BufferDescriptor>,
        dst_dv: Option<&mut [f32]>,
        dv_desc: Option<&BufferDescriptor>,
        dst_duu: Option<&mut [f32]>,
        duu_desc: Option<&BufferDescriptor>,
        dst_duv: Option<&mut [f32]>,
        duv_desc: Option<&BufferDescriptor>,
        dst_dvv: Option<&mut [f32]>,
        dvv_desc: Option<&BufferDescriptor>,
        patch_coords: &[PatchCoord],
        patch_arrays: &[PatchArray],
        patch_index_buf: &[i32],
        patch_param_buf: &[PatchParam],
    ) -> bool {
        if src.len() <= src_desc.offset as usize {
            return false;
        }
        if src_desc.length != dst_desc.length {
            return false;
        }
        for d in [du_desc, dv_desc, duu_desc, duv_desc, dvv_desc]
            .into_iter()
            .flatten()
        {
            if src_desc.length != d.length {
                return false;
            }
        }

        let len = src_desc.length as usize;
        let src_base = &src[src_desc.offset as usize..];

        type D2Result = (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>);
        let results: Vec<D2Result> = patch_coords
            .par_iter()
            .map(|coord| {
                let array = &patch_arrays[coord.handle.array_index as usize];
                let pp = &patch_param_buf[coord.handle.patch_index as usize];
                let osd_p = OsdPatchParam::new(pp.field0, pp.field1, pp.sharpness);
                let patch_type = if osd_p.is_regular() {
                    array.get_patch_type_regular()
                } else {
                    array.get_patch_type_irregular()
                };

                let mut wp = [0f32; 20];
                let mut wds = [0f32; 20];
                let mut wdt = [0f32; 20];
                let mut wdss = [0f32; 20];
                let mut wdst = [0f32; 20];
                let mut wdtt = [0f32; 20];
                let n_pts = osd_evaluate_patch_basis_d2(
                    patch_type, &osd_p, coord.s, coord.t, &mut wp, &mut wds, &mut wdt, &mut wdss,
                    &mut wdst, &mut wdtt,
                );

                let idx_base = array.get_index_base()
                    + array.get_stride()
                        * (coord.handle.patch_index - array.get_primitive_id_base());
                let cvs = &patch_index_buf[idx_base as usize..];

                let mut acc = vec![0.0f32; len];
                let mut acc_du = vec![0.0f32; len];
                let mut acc_dv = vec![0.0f32; len];
                let mut acc_duu = vec![0.0f32; len];
                let mut acc_duv = vec![0.0f32; len];
                let mut acc_dvv = vec![0.0f32; len];
                for j in 0..n_pts as usize {
                    let cv = cvs[j] as usize;
                    let off = cv * src_desc.stride as usize;
                    for k in 0..len {
                        let v = src_base[off + k];
                        acc[k] += v * wp[j];
                        acc_du[k] += v * wds[j];
                        acc_dv[k] += v * wdt[j];
                        acc_duu[k] += v * wdss[j];
                        acc_duv[k] += v * wdst[j];
                        acc_dvv[k] += v * wdtt[j];
                    }
                }
                (acc, acc_du, acc_dv, acc_duu, acc_duv, acc_dvv)
            })
            .collect();

        let default_desc = BufferDescriptor::new(0, src_desc.length, src_desc.length);
        let du_d = du_desc.unwrap_or(&default_desc);
        let dv_d = dv_desc.unwrap_or(&default_desc);
        let duu_d = duu_desc.unwrap_or(&default_desc);
        let duv_d = duv_desc.unwrap_or(&default_desc);
        let dvv_d = dvv_desc.unwrap_or(&default_desc);

        let dst_stride = dst_desc.stride as usize;
        let du_stride = du_d.stride as usize;
        let dv_stride = dv_d.stride as usize;
        let duu_stride = duu_d.stride as usize;
        let duv_stride = duv_d.stride as usize;
        let dvv_stride = dvv_d.stride as usize;

        let dst_base = dst_desc.offset as usize;
        let du_base = du_d.offset as usize;
        let dv_base = dv_d.offset as usize;
        let duu_base = duu_d.offset as usize;
        let duv_base = duv_d.offset as usize;
        let dvv_base = dvv_d.offset as usize;

        let mut du_buf = dst_du;
        let mut dv_buf = dst_dv;
        let mut duu_buf = dst_duu;
        let mut duv_buf = dst_duv;
        let mut dvv_buf = dst_dvv;

        for (i, (r, ru, rv, ruu, ruv, rvv)) in results.iter().enumerate() {
            let po = dst_base + i * dst_stride;
            dst[po..po + len].copy_from_slice(r);
            if let Some(ref mut b) = du_buf {
                let o = du_base + i * du_stride;
                b[o..o + len].copy_from_slice(ru);
            }
            if let Some(ref mut b) = dv_buf {
                let o = dv_base + i * dv_stride;
                b[o..o + len].copy_from_slice(rv);
            }
            if let Some(ref mut b) = duu_buf {
                let o = duu_base + i * duu_stride;
                b[o..o + len].copy_from_slice(ruu);
            }
            if let Some(ref mut b) = duv_buf {
                let o = duv_base + i * duv_stride;
                b[o..o + len].copy_from_slice(ruv);
            }
            if let Some(ref mut b) = dvv_buf {
                let o = dvv_base + i * dvv_stride;
                b[o..o + len].copy_from_slice(rvv);
            }
        }
        true
    }

    /// Synchronize — no-op (Rayon is synchronous).
    pub fn synchronize() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(length: i32) -> BufferDescriptor {
        BufferDescriptor::new(0, length, length)
    }

    #[test]
    fn rayon_stencil_identity() {
        let src = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut dst = vec![0.0f32; 6];
        let d = uniform(3);
        let ok = RayonEvaluator::eval_stencils_raw(
            &src,
            &d,
            &mut dst,
            &d,
            &[1, 1],
            &[0, 1],
            &[0, 1],
            &[1.0, 1.0],
            0,
            2,
        );
        assert!(ok);
        assert!((dst[0] - 1.0).abs() < 1e-6);
        assert!((dst[3] - 4.0).abs() < 1e-6);
    }
}
