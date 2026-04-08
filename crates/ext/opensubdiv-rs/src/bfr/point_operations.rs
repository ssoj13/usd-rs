//! Weighted point combination utilities for patch evaluation.
//!
//! Mirrors `Bfr::points::*` from `pointOperations.h`.
//! All operations are generic over a floating-point type `R`.

// ---------------------------------------------------------------------------
// Common parameter structure
// ---------------------------------------------------------------------------

/// Common parameters for operations that combine a set of source points into
/// one or more result points with per-result weight arrays.
///
/// Mirrors `Bfr::points::CommonCombinationParameters<REAL>`.
pub struct CommonCombinationParams<'a, R> {
    pub point_data: &'a [R],
    pub point_size: usize,
    pub point_stride: usize,

    /// Optional index list; `None` means consecutive points.
    pub src_indices: Option<&'a [i32]>,
    pub src_count: usize,

    pub result_count: usize,
    /// One slice per result, each of length `point_size`.
    pub result_array: Vec<&'a mut [R]>,
    /// One weight slice per result, each of length `src_count`.
    pub weight_array: Vec<&'a [R]>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// `pDst[i] = w * pSrc[i]`  for `size` elements.
#[inline]
fn point_set<R: Copy + std::ops::Mul<Output = R>>(dst: &mut [R], w: R, src: &[R], size: usize) {
    for i in 0..size {
        dst[i] = w * src[i];
    }
}

/// `pDst[i] += w * pSrc[i]`  for `size` elements.
#[inline]
fn point_add<R: Copy + std::ops::Mul<Output = R> + std::ops::AddAssign>(
    dst: &mut [R],
    w: R,
    src: &[R],
    size: usize,
) {
    for i in 0..size {
        dst[i] += w * src[i];
    }
}

// ---------------------------------------------------------------------------
// Combine1 — single result
// ---------------------------------------------------------------------------

/// Combine source points into a **single** result point.
///
/// Mirrors `Bfr::points::Combine1<REAL>::Apply`.
pub fn combine1<R>(
    point_data: &[R],
    point_size: usize,
    point_stride: usize,
    src_indices: Option<&[i32]>,
    src_count: usize,
    weights: &[R],
    result: &mut [R],
) where
    R: Copy + std::ops::Mul<Output = R> + std::ops::AddAssign,
{
    if let Some(indices) = src_indices {
        let src = &point_data[point_stride * indices[0] as usize..];
        point_set(result, weights[0], src, point_size);
        for i in 1..src_count {
            let src = &point_data[point_stride * indices[i] as usize..];
            point_add(result, weights[i], src, point_size);
        }
    } else {
        point_set(result, weights[0], &point_data[0..], point_size);
        for i in 1..src_count {
            let src = &point_data[point_stride * i..];
            point_add(result, weights[i], src, point_size);
        }
    }
}

// ---------------------------------------------------------------------------
// Combine3 — three results (P, Du, Dv)
// ---------------------------------------------------------------------------

/// Combine source points into **three** result points (position + derivatives).
///
/// Mirrors `Bfr::points::Combine3<REAL>::Apply`.
pub fn combine3<R>(
    point_data: &[R],
    point_size: usize,
    point_stride: usize,
    src_indices: Option<&[i32]>,
    src_count: usize,
    weights: [&[R]; 3],
    results: [&mut [R]; 3],
) where
    R: Copy + std::ops::Mul<Output = R> + std::ops::AddAssign,
{
    let [r0, r1, r2] = results;
    let [w0, w1, w2] = weights;

    let src0 = if let Some(ids) = src_indices {
        &point_data[point_stride * ids[0] as usize..]
    } else {
        &point_data[0..]
    };

    point_set(r0, w0[0], src0, point_size);
    point_set(r1, w1[0], src0, point_size);
    point_set(r2, w2[0], src0, point_size);

    for i in 1..src_count {
        let src = if let Some(ids) = src_indices {
            &point_data[point_stride * ids[i] as usize..]
        } else {
            &point_data[point_stride * i..]
        };
        point_add(r0, w0[i], src, point_size);
        point_add(r1, w1[i], src, point_size);
        point_add(r2, w2[i], src, point_size);
    }
}

// ---------------------------------------------------------------------------
// CombineMultiple — arbitrary number of results
// ---------------------------------------------------------------------------

/// Combine source points into an **arbitrary** number of result points.
///
/// Mirrors `Bfr::points::CombineMultiple<REAL>::Apply`.
pub fn combine_multiple<R>(
    point_data: &[R],
    point_size: usize,
    point_stride: usize,
    src_indices: Option<&[i32]>,
    src_count: usize,
    result_count: usize,
    weight_array: &[&[R]],
    result_array: &mut [&mut [R]],
) where
    R: Copy + std::ops::Mul<Output = R> + std::ops::AddAssign,
{
    let src0 = if let Some(ids) = src_indices {
        &point_data[point_stride * ids[0] as usize..]
    } else {
        &point_data[0..]
    };

    for j in 0..result_count {
        point_set(result_array[j], weight_array[j][0], src0, point_size);
    }

    for i in 1..src_count {
        let src = if let Some(ids) = src_indices {
            &point_data[point_stride * ids[i] as usize..]
        } else {
            &point_data[point_stride * i..]
        };
        for j in 0..result_count {
            point_add(result_array[j], weight_array[j][i], src, point_size);
        }
    }
}

// ---------------------------------------------------------------------------
// CombineConsecutive — M results from N sources, weights stored consecutively
// ---------------------------------------------------------------------------

/// Parameters for `combine_consecutive`.
pub struct CombineConsecutiveParams<'a, R> {
    pub point_data: &'a [R],
    pub point_size: usize,
    pub point_stride: usize,
    pub src_count: usize,
    pub result_count: usize,
    pub result_data: &'a mut [R],
    pub weight_data: &'a [R],
}

/// Combine N source points into M consecutive result points.
///
/// Mirrors `Bfr::points::CombineConsecutive<REAL>::Apply`.
pub fn combine_consecutive<R>(p: &mut CombineConsecutiveParams<'_, R>)
where
    R: Copy + std::ops::Mul<Output = R> + std::ops::AddAssign,
{
    let mut w_off = 0usize;
    let mut p_off = 0usize;

    for _i in 0..p.result_count {
        let result = &mut p.result_data[p_off..p_off + p.point_size];
        let w = &p.weight_data[w_off..w_off + p.src_count];

        point_set(result, w[0], &p.point_data[0..], p.point_size);
        for j in 1..p.src_count {
            let src = &p.point_data[p.point_stride * j..];
            point_add(result, w[j], src, p.point_size);
        }

        p_off += p.point_stride;
        w_off += p.src_count;
    }
}

// ---------------------------------------------------------------------------
// SplitFace — face midpoint + edge midpoints
// ---------------------------------------------------------------------------

/// Parameters for `split_face`.
pub struct SplitFaceParams<'a, R> {
    pub point_data: &'a [R],
    pub point_size: usize,
    pub point_stride: usize,
    pub src_count: usize,
    /// Output buffer: first element is face center, next `src_count` are edge midpoints.
    pub result_data: &'a mut [R],
}

/// Compute the centroid and edge midpoints of an N-gon.
///
/// Mirrors `Bfr::points::SplitFace<REAL>::Apply`.
pub fn split_face<R>(p: &mut SplitFaceParams<'_, R>)
where
    R: Copy
        + std::ops::Mul<Output = R>
        + std::ops::AddAssign
        + num_traits::identities::Zero
        + num_traits::cast::FromPrimitive,
{
    let n = p.src_count;
    let inv_n = R::from_f32(1.0 / n as f32).expect("cast");

    // Zero out face center.
    let face_point = &mut p.result_data[0..p.point_size];
    for v in face_point.iter_mut() {
        *v = R::zero();
    }

    for i in 0..n {
        let j = if i < n - 1 { i + 1 } else { 0 };

        let pi = &p.point_data[p.point_stride * i..];
        let pj = &p.point_data[p.point_stride * j..];

        // Accumulate into face center.
        let fc = &mut p.result_data[0..p.point_size];
        point_add(fc, inv_n, pi, p.point_size);

        // Edge midpoint = 0.5*pi + 0.5*pj.
        let ep_off = p.point_stride * (1 + i);
        let ep = &mut p.result_data[ep_off..ep_off + p.point_size];
        let half = R::from_f32(0.5).expect("cast");
        point_set(ep, half, pi, p.point_size);
        point_add(ep, half, pj, p.point_size);
    }
}

// ---------------------------------------------------------------------------
// CopyConsecutive — copy indexed source points to consecutive destination
// ---------------------------------------------------------------------------

/// Copy a set of indexed source points to consecutive destination.
///
/// Mirrors `Bfr::points::CopyConsecutive<REAL_DST,REAL_SRC>::Apply`.
pub fn copy_consecutive<Dst, Src>(
    src_data: &[Src],
    src_size: usize,
    src_stride: usize,
    src_indices: &[i32],
    src_count: usize,
    dst_data: &mut [Dst],
    dst_stride: usize,
) where
    Dst: Copy + num_traits::cast::AsPrimitive<Dst>,
    Src: Copy + num_traits::cast::AsPrimitive<Dst>,
{
    for i in 0..src_count {
        let src_off = src_stride * src_indices[i] as usize;
        let dst_off = dst_stride * i;
        for j in 0..src_size {
            dst_data[dst_off + j] = src_data[src_off + j].as_();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine1_no_indices() {
        // Three 3D points, combine with weights [0.5, 0.3, 0.2].
        let pts = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let w = [0.5f32, 0.3, 0.2];
        let mut r = [0.0f32; 3];

        combine1(&pts, 3, 3, None, 3, &w, &mut r);

        // r = 0.5*(1,0,0) + 0.3*(0,1,0) + 0.2*(0,0,1) = (0.5, 0.3, 0.2)
        assert!((r[0] - 0.5).abs() < 1e-6);
        assert!((r[1] - 0.3).abs() < 1e-6);
        assert!((r[2] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn combine1_with_indices() {
        let pts = [
            1.0f32, 0.0, // point 0
            0.0, 2.0,
        ]; // point 1
        let indices = [1i32, 0];
        let w = [0.4f32, 0.6];
        let mut r = [0.0f32; 2];

        combine1(&pts, 2, 2, Some(&indices), 2, &w, &mut r);

        // r = 0.4*(0,2) + 0.6*(1,0) = (0.6, 0.8)
        assert!((r[0] - 0.6).abs() < 1e-6);
        assert!((r[1] - 0.8).abs() < 1e-6);
    }
}
