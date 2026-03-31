/// Low-level CPU stencil evaluation kernels.
///
/// Mirrors `cpuKernel.h` / `cpuKernel.cpp` from OpenSubdiv 3.7.0.
/// These are the inner loops called by `CpuEvaluator`.  All operations are
/// pure data transforms over f32 slices — no allocations inside the hot path.
///
/// # SIMD performance note
///
/// The C++ `CpuEvalStencils` applies a compile-time specialisation for
/// `srcDesc.length == 4` and `srcDesc.length == 8` (stride-aligned cases)
/// via `ComputeStencilKernel<4>` / `ComputeStencilKernel<8>`.  These
/// specialisations allow the compiler to auto-vectorise the inner loop more
/// aggressively.  The Rust implementation uses a generic loop which Rust's
/// LLVM backend can also auto-vectorise for uniform strides, but without the
/// explicit template dispatch the compiler hint is weaker.  For the most
/// common interleaved XYZ (`length=3, stride=3`) and XYZW (`length=4,
/// stride=4`) cases this is a measurable performance gap on large meshes.
/// If profiling shows a bottleneck here, a `match src_desc.length` fast-path
/// with inlined fixed-width inner loops should be added.
use crate::osd::BufferDescriptor;

// ---------------------------------------------------------------------------
//  Stencil kernel — position only
// ---------------------------------------------------------------------------

/// Evaluate stencils, position only.
///
/// Mirrors `CpuEvalStencils(src, srcDesc, dst, dstDesc,
///                           sizes, offsets, indices, weights, start, end)`.
pub fn cpu_eval_stencils(
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
) {
    debug_assert!(start >= 0 && start < end);
    let n = (end - start) as usize;
    let len = src_desc.length as usize;

    let sizes_s   = &sizes[start as usize..];
    let idx_base  = if start > 0 { offsets[start as usize] as usize } else { 0 };
    let indices_s = &indices[idx_base..];
    let weights_s = &weights[idx_base..];
    let src_s = &src[src_desc.offset as usize..];

    let mut result = vec![0f32; len];
    let mut cursor = 0usize;

    for i in 0..n {
        let sz = sizes_s[i] as usize;
        for v in result.iter_mut() { *v = 0.0; }

        for j in 0..sz {
            let cv = indices_s[cursor + j] as usize;
            let w  = weights_s[cursor + j];
            let off = cv * src_desc.stride as usize;
            for k in 0..len {
                result[k] += src_s[off + k] * w;
            }
        }

        let dst_off = dst_desc.offset as usize + i * dst_desc.stride as usize;
        dst[dst_off..dst_off + len].copy_from_slice(&result);
        cursor += sz;
    }
}

// ---------------------------------------------------------------------------
//  Stencil kernel — position + first derivatives
// ---------------------------------------------------------------------------

/// Evaluate stencils with du/dv derivative weights.
pub fn cpu_eval_stencils_d1(
    src:      &[f32],
    src_desc: &BufferDescriptor,
    dst:      &mut [f32],
    dst_desc: &BufferDescriptor,
    dst_du:   &mut [f32],
    du_desc:  &BufferDescriptor,
    dst_dv:   &mut [f32],
    dv_desc:  &BufferDescriptor,
    sizes:      &[i32],
    offsets:    &[i32],
    indices:    &[i32],
    weights:    &[f32],
    du_weights: &[f32],
    dv_weights: &[f32],
    start: i32,
    end:   i32,
) {
    debug_assert!(start >= 0 && start < end);
    let n = (end - start) as usize;
    let len = src_desc.length as usize;

    let sizes_s    = &sizes[start as usize..];
    let idx_base   = if start > 0 { offsets[start as usize] as usize } else { 0 };
    let indices_s  = &indices[idx_base..];
    let w_s        = &weights[idx_base..];
    let wu_s       = &du_weights[idx_base..];
    let wv_s       = &dv_weights[idx_base..];
    let src_s      = &src[src_desc.offset as usize..];

    // Single allocation for all three accumulators
    let mut buf = vec![0f32; len * 3];
    let mut cursor = 0usize;

    for i in 0..n {
        let sz = sizes_s[i] as usize;
        for v in buf.iter_mut() { *v = 0.0; }
        let (r, rest) = buf.split_at_mut(len);
        let (ru, rv)  = rest.split_at_mut(len);

        for j in 0..sz {
            let cv  = indices_s[cursor + j] as usize;
            let off = cv * src_desc.stride as usize;
            let w   = w_s[cursor + j];
            let wu  = wu_s[cursor + j];
            let wv  = wv_s[cursor + j];
            for k in 0..len {
                let s = src_s[off + k];
                r[k]  += s * w;
                ru[k] += s * wu;
                rv[k] += s * wv;
            }
        }

        let off_p  = dst_desc.offset as usize + i * dst_desc.stride as usize;
        let off_u  = du_desc.offset  as usize + i * du_desc.stride  as usize;
        let off_v  = dv_desc.offset  as usize + i * dv_desc.stride  as usize;
        dst[off_p..off_p+len].copy_from_slice(r);
        dst_du[off_u..off_u+len].copy_from_slice(ru);
        dst_dv[off_v..off_v+len].copy_from_slice(rv);

        cursor += sz;
    }
}

// ---------------------------------------------------------------------------
//  Stencil kernel — position + first + second derivatives
// ---------------------------------------------------------------------------

/// Evaluate stencils with du/dv and duu/duv/dvv derivative weights.
#[allow(clippy::too_many_arguments)]
pub fn cpu_eval_stencils_d2(
    src:      &[f32],
    src_desc: &BufferDescriptor,
    dst:      &mut [f32],
    dst_desc: &BufferDescriptor,
    dst_du:   &mut [f32],
    du_desc:  &BufferDescriptor,
    dst_dv:   &mut [f32],
    dv_desc:  &BufferDescriptor,
    dst_duu:  &mut [f32],
    duu_desc: &BufferDescriptor,
    dst_duv:  &mut [f32],
    duv_desc: &BufferDescriptor,
    dst_dvv:  &mut [f32],
    dvv_desc: &BufferDescriptor,
    sizes:       &[i32],
    offsets:     &[i32],
    indices:     &[i32],
    weights:     &[f32],
    du_weights:  &[f32],
    dv_weights:  &[f32],
    duu_weights: &[f32],
    duv_weights: &[f32],
    dvv_weights: &[f32],
    start: i32,
    end:   i32,
) {
    debug_assert!(start >= 0 && start < end);
    let n = (end - start) as usize;
    let len = src_desc.length as usize;

    let sizes_s    = &sizes[start as usize..];
    let idx_base   = if start > 0 { offsets[start as usize] as usize } else { 0 };
    let indices_s  = &indices[idx_base..];
    let w_s        = &weights[idx_base..];
    let wu_s       = &du_weights[idx_base..];
    let wv_s       = &dv_weights[idx_base..];
    let wuu_s      = &duu_weights[idx_base..];
    let wuv_s      = &duv_weights[idx_base..];
    let wvv_s      = &dvv_weights[idx_base..];
    let src_s      = &src[src_desc.offset as usize..];

    let mut buf = vec![0f32; len * 6];
    let mut cursor = 0usize;

    for i in 0..n {
        let sz = sizes_s[i] as usize;
        for v in buf.iter_mut() { *v = 0.0; }

        let (r,   rest1) = buf.split_at_mut(len);
        let (ru,  rest2) = rest1.split_at_mut(len);
        let (rv,  rest3) = rest2.split_at_mut(len);
        let (ruu, rest4) = rest3.split_at_mut(len);
        let (ruv, rvv)   = rest4.split_at_mut(len);

        for j in 0..sz {
            let cv  = indices_s[cursor + j] as usize;
            let off = cv * src_desc.stride as usize;
            let w   = w_s[cursor + j];
            let wu  = wu_s[cursor + j];
            let wv  = wv_s[cursor + j];
            let wuu = wuu_s[cursor + j];
            let wuv = wuv_s[cursor + j];
            let wvv = wvv_s[cursor + j];
            for k in 0..len {
                let s = src_s[off + k];
                r[k]   += s * w;
                ru[k]  += s * wu;
                rv[k]  += s * wv;
                ruu[k] += s * wuu;
                ruv[k] += s * wuv;
                rvv[k] += s * wvv;
            }
        }

        macro_rules! write_out {
            ($buf:expr, $slice:expr, $desc:expr) => {{
                let off = $desc.offset as usize + i * $desc.stride as usize;
                $slice[off..off+len].copy_from_slice($buf);
            }};
        }
        write_out!(r,   dst,     dst_desc);
        write_out!(ru,  dst_du,  du_desc);
        write_out!(rv,  dst_dv,  dv_desc);
        write_out!(ruu, dst_duu, duu_desc);
        write_out!(ruv, dst_duv, duv_desc);
        write_out!(rvv, dst_dvv, dvv_desc);

        cursor += sz;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(length: i32) -> BufferDescriptor {
        BufferDescriptor::new(0, length, length)
    }

    #[test]
    fn single_identity_stencil() {
        let src = vec![1.0f32, 2.0, 3.0];
        let mut dst = vec![0.0f32; 3];
        let d = uniform(3);
        cpu_eval_stencils(&src, &d, &mut dst, &d,
            &[1], &[0], &[0], &[1.0], 0, 1);
        assert_eq!(dst, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn average_two_vertices() {
        // Two 2-float vertices; one stencil averages them
        let src = vec![0.0f32, 0.0, 4.0, 8.0];
        let mut dst = vec![0.0f32; 2];
        let d = uniform(2);
        cpu_eval_stencils(&src, &d, &mut dst, &d,
            &[2], &[0], &[0, 1], &[0.5, 0.5], 0, 1);
        assert!((dst[0] - 2.0).abs() < 1e-6);
        assert!((dst[1] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn start_offset_processes_second_only() {
        let src = vec![1.0f32, 100.0]; // two 1-float verts
        let mut dst = vec![0.0f32; 2];
        let d = uniform(1);
        // sizes=[1,1], offsets=[0,1], start=1 → only stencil 1
        cpu_eval_stencils(&src, &d, &mut dst, &d,
            &[1, 1], &[0, 1], &[0, 1], &[1.0, 1.0], 1, 2);
        // Result written to dst element 0 (relative index = i=0)
        assert!((dst[0] - 100.0).abs() < 1e-6);
    }

    #[test]
    fn d1_correct_accumulation() {
        // One 1-element stencil: two CVs with w=1/w_du=2/w_dv=3 each,
        // values [5, 7].  Expected: p=5+7=12, du=10+14=24, dv=15+21=36.
        let src = vec![5.0f32, 7.0];
        let mut dst    = vec![0.0f32; 1];
        let mut dst_du = vec![0.0f32; 1];
        let mut dst_dv = vec![0.0f32; 1];
        let d = uniform(1);
        cpu_eval_stencils_d1(
            &src, &d, &mut dst, &d,
            &mut dst_du, &d, &mut dst_dv, &d,
            &[2], &[0], &[0, 1],
            &[1.0, 1.0], &[2.0, 2.0], &[3.0, 3.0],
            0, 1);
        assert!((dst[0]    - 12.0).abs() < 1e-5);
        assert!((dst_du[0] - 24.0).abs() < 1e-5);
        assert!((dst_dv[0] - 36.0).abs() < 1e-5);
    }
}
