//! Batched (SIMD-width) types and renderer services — C++ parity port.
//!
//! Port of `batched_rendererservices.h`, `wide.h`, and
//! `batched_shaderglobals.h` from C++ OSL.
//!
//! # Architecture
//!
//! C++ OSL supports processing multiple shading points simultaneously using
//! SIMD-width data types. The template parameter `WidthT` (typically 8 for
//! AVX or 16 for AVX-512) determines how many lanes are processed in parallel.
//!
//! In Rust, we use `const WIDTH: usize` const generics for the same purpose:
//!
//! - [`Wide<T, WIDTH>`] — array of `WIDTH` values of type `T`
//! - [`Mask<WIDTH>`] — bitmask of active lanes
//! - [`BatchedShaderGlobals<WIDTH>`] — per-shading-point state, widened
//! - [`BatchedRendererServices<WIDTH>`] — trait for batched renderer callbacks
//!
//! # Performance note
//!
//! The current implementation uses scalar arrays (`[T; WIDTH]`) and processes
//! lanes in loops. For production SIMD performance, these types can be backed
//! by platform intrinsics (AVX `__m256`, etc.) without changing the public API.

use std::ffi::c_void;
use std::fmt;

use crate::Float;
use crate::math::{Color3, Matrix44, Vec3};
use crate::renderer::{TextureHandle, TexturePerthread, TraceOpt};
use crate::shaderglobals::ShaderGlobals;
use crate::typedesc::TypeDesc;
use crate::ustring::UStringHash;

/// Re-export pointer type for opaque renderer data in batched context.
/// Each lane may reference a different transformation.
pub type TransformationPtr = *const c_void;
/// Per-lane closure color pointer (opaque to the shading system).
pub type ClosureColorPtr = *mut c_void;

// ---------------------------------------------------------------------------
// Wide<T, WIDTH> — SIMD-width array
// ---------------------------------------------------------------------------

/// A "wide" value: `WIDTH` lanes of scalar `T`.
///
/// Binary-compatible with C++ `OSL::Block<T, WidthT>`.
/// Each lane corresponds to one shading point in the batch.
#[repr(C)]
#[derive(Clone)]
pub struct Wide<T: Copy, const WIDTH: usize> {
    pub data: [T; WIDTH],
}

impl<T: Copy, const WIDTH: usize> Wide<T, WIDTH> {
    /// All lanes set to the same value.
    pub fn splat(val: T) -> Self {
        Self { data: [val; WIDTH] }
    }

    /// Read one lane.
    #[inline]
    pub fn load(&self, lane: usize) -> T {
        self.data[lane]
    }

    /// Write one lane.
    #[inline]
    pub fn store(&mut self, lane: usize, val: T) {
        self.data[lane] = val;
    }

    /// Number of lanes.
    pub const fn width(&self) -> usize {
        WIDTH
    }
}

impl<T: Copy + Default, const WIDTH: usize> Wide<T, WIDTH> {
    /// All lanes set to `T::default()`.
    pub fn zero() -> Self
    where
        T: Default,
    {
        Self {
            data: [T::default(); WIDTH],
        }
    }
}

impl<T: Copy + Default, const WIDTH: usize> Default for Wide<T, WIDTH> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<T: Copy + fmt::Debug, const WIDTH: usize> fmt::Debug for Wide<T, WIDTH> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

// ---------------------------------------------------------------------------
// SIMD-accelerated specializations (x86_64 AVX for WIDTH=8)
// ---------------------------------------------------------------------------

/// Low-level AVX-256 helpers for `Wide<f32, 8>`.
#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
pub mod simd_avx_inner {
    use std::arch::x86_64::*;

    #[inline]
    pub unsafe fn add8(a: &[f32; 8], b: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_add_ps(va, vb));
    }
    #[inline]
    pub unsafe fn sub8(a: &[f32; 8], b: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_sub_ps(va, vb));
    }
    #[inline]
    pub unsafe fn mul8(a: &[f32; 8], b: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_mul_ps(va, vb));
    }
    #[inline]
    pub unsafe fn safe_div8(a: &[f32; 8], b: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        let zero = _mm256_setzero_ps();
        let mask = _mm256_cmp_ps(vb, zero, _CMP_NEQ_OQ);
        let quotient = _mm256_div_ps(va, vb);
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_and_ps(quotient, mask));
    }
    #[inline]
    pub unsafe fn sqrt8(a: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let zero = _mm256_setzero_ps();
        let clamped = _mm256_max_ps(va, zero);
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_sqrt_ps(clamped));
    }
    #[inline]
    #[cfg(target_feature = "fma")]
    pub unsafe fn fma8(a: &[f32; 8], b: &[f32; 8], c: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        let vc = _mm256_loadu_ps(c.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_fmadd_ps(va, vb, vc));
    }
    #[inline]
    #[cfg(not(target_feature = "fma"))]
    pub unsafe fn fma8(a: &[f32; 8], b: &[f32; 8], c: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        let vc = _mm256_loadu_ps(c.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_add_ps(_mm256_mul_ps(va, vb), vc));
    }
    #[inline]
    pub unsafe fn min8(a: &[f32; 8], b: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_min_ps(va, vb));
    }
    #[inline]
    pub unsafe fn max8(a: &[f32; 8], b: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_max_ps(va, vb));
    }
    #[inline]
    pub unsafe fn abs8(a: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let sign_mask = _mm256_castsi256_ps(_mm256_set1_epi32(0x7FFF_FFFFu32 as i32));
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_and_ps(va, sign_mask));
    }
    #[inline]
    pub unsafe fn mul_scalar8(a: &[f32; 8], s: f32, out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vs = _mm256_set1_ps(s);
        _mm256_storeu_ps(out.as_mut_ptr(), _mm256_mul_ps(va, vs));
    }
    #[inline]
    pub unsafe fn hsum8(a: &[f32; 8]) -> f32 {
        let va = _mm256_loadu_ps(a.as_ptr());
        let hi128 = _mm256_extractf128_ps(va, 1);
        let lo128 = _mm256_castps256_ps128(va);
        let sum4 = _mm_add_ps(lo128, hi128);
        let shuf = _mm_movehdup_ps(sum4);
        let sums = _mm_add_ps(sum4, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let final_sum = _mm_add_ss(sums, shuf2);
        _mm_cvtss_f32(final_sum)
    }
    #[inline]
    pub unsafe fn lerp8(a: &[f32; 8], b: &[f32; 8], t: &[f32; 8], out: &mut [f32; 8]) {
        let va = _mm256_loadu_ps(a.as_ptr());
        let vb = _mm256_loadu_ps(b.as_ptr());
        let vt = _mm256_loadu_ps(t.as_ptr());
        let one = _mm256_set1_ps(1.0);
        let omt = _mm256_sub_ps(one, vt);
        let r = _mm256_add_ps(_mm256_mul_ps(va, omt), _mm256_mul_ps(vb, vt));
        _mm256_storeu_ps(out.as_mut_ptr(), r);
    }
}

/// Bridge macro: when WIDTH == 8 and AVX is compiled in, use SIMD intrinsics.
#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
macro_rules! wide8_simd_dispatch {
    ($self:expr, $other:expr, $result:expr, $scalar_body:block, $simd_fn:ident) => {
        if WIDTH == 8 {
            unsafe {
                let a = &*($self.data.as_ptr() as *const [f32; 8]);
                let b = &*($other.data.as_ptr() as *const [f32; 8]);
                let out = &mut *($result.data.as_mut_ptr() as *mut [f32; 8]);
                simd_avx_inner::$simd_fn(a, b, out);
            }
        } else $scalar_body
    };
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "avx")))]
macro_rules! wide8_simd_dispatch {
    ($self:expr, $other:expr, $result:expr, $scalar_body:block, $simd_fn:ident) => {
        $scalar_body
    };
}

// Convenience operations for Wide<Float>
impl<const WIDTH: usize> Wide<Float, WIDTH> {
    /// Component-wise add (AVX-accelerated for WIDTH=8).
    pub fn add(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        wide8_simd_dispatch!(
            self,
            other,
            result,
            {
                for i in 0..WIDTH {
                    result.data[i] = self.data[i] + other.data[i];
                }
            },
            add8
        );
        result
    }

    /// Component-wise subtract (AVX-accelerated for WIDTH=8).
    pub fn sub(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        wide8_simd_dispatch!(
            self,
            other,
            result,
            {
                for i in 0..WIDTH {
                    result.data[i] = self.data[i] - other.data[i];
                }
            },
            sub8
        );
        result
    }

    /// Component-wise multiply (AVX-accelerated for WIDTH=8).
    pub fn mul(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        wide8_simd_dispatch!(
            self,
            other,
            result,
            {
                for i in 0..WIDTH {
                    result.data[i] = self.data[i] * other.data[i];
                }
            },
            mul8
        );
        result
    }

    /// Component-wise divide (with zero protection, AVX-accelerated for WIDTH=8).
    pub fn div(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        wide8_simd_dispatch!(
            self,
            other,
            result,
            {
                for i in 0..WIDTH {
                    result.data[i] = if other.data[i] != 0.0 {
                        self.data[i] / other.data[i]
                    } else {
                        0.0
                    };
                }
            },
            safe_div8
        );
        result
    }

    /// Scalar multiply (AVX-accelerated for WIDTH=8).
    pub fn mul_scalar(&self, s: Float) -> Self {
        let mut result = Self::zero();
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        if WIDTH == 8 {
            unsafe {
                let a = &*(self.data.as_ptr() as *const [f32; 8]);
                let out = &mut *(result.data.as_mut_ptr() as *mut [f32; 8]);
                simd_avx_inner::mul_scalar8(a, s, out);
            }
            return result;
        }
        for i in 0..WIDTH {
            result.data[i] = self.data[i] * s;
        }
        result
    }

    /// Component-wise absolute value (AVX-accelerated for WIDTH=8).
    pub fn abs(&self) -> Self {
        let mut result = Self::zero();
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        if WIDTH == 8 {
            unsafe {
                let a = &*(self.data.as_ptr() as *const [f32; 8]);
                let out = &mut *(result.data.as_mut_ptr() as *mut [f32; 8]);
                simd_avx_inner::abs8(a, out);
            }
            return result;
        }
        for i in 0..WIDTH {
            result.data[i] = self.data[i].abs();
        }
        result
    }

    /// Component-wise min (AVX-accelerated for WIDTH=8).
    pub fn min(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        wide8_simd_dispatch!(
            self,
            other,
            result,
            {
                for i in 0..WIDTH {
                    result.data[i] = self.data[i].min(other.data[i]);
                }
            },
            min8
        );
        result
    }

    /// Component-wise max (AVX-accelerated for WIDTH=8).
    pub fn max(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        wide8_simd_dispatch!(
            self,
            other,
            result,
            {
                for i in 0..WIDTH {
                    result.data[i] = self.data[i].max(other.data[i]);
                }
            },
            max8
        );
        result
    }

    /// Component-wise clamp to [lo, hi].
    pub fn clamp(&self, lo: &Self, hi: &Self) -> Self {
        self.max(lo).min(hi)
    }

    /// Component-wise sqrt (AVX-accelerated for WIDTH=8).
    pub fn sqrt(&self) -> Self {
        let mut result = Self::zero();
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        if WIDTH == 8 {
            unsafe {
                let a = &*(self.data.as_ptr() as *const [f32; 8]);
                let out = &mut *(result.data.as_mut_ptr() as *mut [f32; 8]);
                simd_avx_inner::sqrt8(a, out);
            }
            return result;
        }
        for i in 0..WIDTH {
            result.data[i] = self.data[i].max(0.0).sqrt();
        }
        result
    }

    /// Fused multiply-add: self * a + b (AVX+FMA accelerated for WIDTH=8).
    pub fn fma(&self, a: &Self, b: &Self) -> Self {
        let mut result = Self::zero();
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        if WIDTH == 8 {
            unsafe {
                let va = &*(self.data.as_ptr() as *const [f32; 8]);
                let vb = &*(a.data.as_ptr() as *const [f32; 8]);
                let vc = &*(b.data.as_ptr() as *const [f32; 8]);
                let out = &mut *(result.data.as_mut_ptr() as *mut [f32; 8]);
                simd_avx_inner::fma8(va, vb, vc, out);
            }
            return result;
        }
        for i in 0..WIDTH {
            result.data[i] = self.data[i].mul_add(a.data[i], b.data[i]);
        }
        result
    }

    /// Component-wise sin (scalar loop; transcendentals need libm per-lane).
    pub fn sin(&self) -> Self {
        let mut result = Self::zero();
        for i in 0..WIDTH {
            result.data[i] = self.data[i].sin();
        }
        result
    }

    /// Component-wise cos (scalar loop; transcendentals need libm per-lane).
    pub fn cos(&self) -> Self {
        let mut result = Self::zero();
        for i in 0..WIDTH {
            result.data[i] = self.data[i].cos();
        }
        result
    }

    /// Reduction: sum all lanes (AVX-accelerated for WIDTH=8).
    pub fn horizontal_sum(&self) -> Float {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        if WIDTH == 8 {
            return unsafe {
                let a = &*(self.data.as_ptr() as *const [f32; 8]);
                simd_avx_inner::hsum8(a)
            };
        }
        let mut s = 0.0;
        for i in 0..WIDTH {
            s += self.data[i];
        }
        s
    }

    /// Linear interpolation: self * (1-t) + other * t (AVX-accelerated for WIDTH=8).
    pub fn lerp(&self, other: &Self, t: &Self) -> Self {
        let mut result = Self::zero();
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        if WIDTH == 8 {
            unsafe {
                let a = &*(self.data.as_ptr() as *const [f32; 8]);
                let b = &*(other.data.as_ptr() as *const [f32; 8]);
                let vt = &*(t.data.as_ptr() as *const [f32; 8]);
                let out = &mut *(result.data.as_mut_ptr() as *mut [f32; 8]);
                simd_avx_inner::lerp8(a, b, vt, out);
            }
            return result;
        }
        for i in 0..WIDTH {
            result.data[i] = self.data[i] * (1.0 - t.data[i]) + other.data[i] * t.data[i];
        }
        result
    }
}

// Convenience operations for Wide<Vec3>
impl<const WIDTH: usize> Wide<Vec3, WIDTH> {
    /// All lanes set to Vec3::ZERO.
    pub fn zeros() -> Self {
        Self {
            data: [Vec3::ZERO; WIDTH],
        }
    }

    /// Component-wise add.
    pub fn add(&self, other: &Self) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            result.data[i] = self.data[i] + other.data[i];
        }
        result
    }

    /// Component-wise dot product per lane.
    pub fn dot(&self, other: &Self) -> Wide<Float, WIDTH> {
        let mut result = Wide::<Float, WIDTH>::zero();
        for i in 0..WIDTH {
            result.data[i] = self.data[i].dot(other.data[i]);
        }
        result
    }

    /// Component-wise subtract.
    pub fn sub(&self, other: &Self) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            result.data[i] = self.data[i] - other.data[i];
        }
        result
    }

    /// Scalar multiply (each Vec3 lane scaled by corresponding float lane).
    pub fn mul_scalar(&self, s: &Wide<Float, WIDTH>) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            result.data[i] = self.data[i] * s.data[i];
        }
        result
    }

    /// Normalize each lane.
    pub fn normalize(&self) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            result.data[i] = self.data[i].normalize();
        }
        result
    }

    /// Cross product per lane.
    pub fn cross(&self, other: &Self) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            result.data[i] = self.data[i].cross(other.data[i]);
        }
        result
    }

    /// Length per lane.
    pub fn length(&self) -> Wide<Float, WIDTH> {
        let mut result = Wide::<Float, WIDTH>::zero();
        for i in 0..WIDTH {
            result.data[i] = self.data[i].length();
        }
        result
    }

    /// Reflect per lane: I - 2 * dot(N, I) * N.
    pub fn reflect(&self, n: &Self) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            let d = self.data[i].dot(n.data[i]);
            result.data[i] = self.data[i] - n.data[i] * (2.0 * d);
        }
        result
    }

    /// Linear interpolation per lane.
    pub fn lerp(&self, other: &Self, t: &Wide<Float, WIDTH>) -> Self {
        let mut result = Self::zeros();
        for i in 0..WIDTH {
            let ti = t.data[i];
            result.data[i] = self.data[i] * (1.0 - ti) + other.data[i] * ti;
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Mask<WIDTH> — lane activity bitmask
// ---------------------------------------------------------------------------

/// Bitmask of active lanes in a SIMD batch.
///
/// Matches C++ `OSL::Mask<WidthT>`. Lane `i` is active if bit `i` is set.
/// Methods that process batches skip inactive lanes.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Mask<const WIDTH: usize> {
    bits: u32,
}

impl<const WIDTH: usize> Mask<WIDTH> {
    /// No lanes active.
    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    /// All lanes active.
    pub fn all() -> Self {
        Self {
            bits: (1u32 << WIDTH) - 1,
        }
    }

    /// Create from a raw bitmask.
    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Get the raw bitmask.
    pub const fn bits(&self) -> u32 {
        self.bits
    }

    /// Check if lane `i` is active.
    #[inline]
    pub fn is_set(&self, lane: usize) -> bool {
        (self.bits >> lane) & 1 != 0
    }

    /// Set lane `i` to active.
    #[inline]
    pub fn set(&mut self, lane: usize) {
        self.bits |= 1 << lane;
    }

    /// Clear lane `i`.
    #[inline]
    pub fn clear(&mut self, lane: usize) {
        self.bits &= !(1 << lane);
    }

    /// Toggle a lane.
    #[inline]
    pub fn toggle(&mut self, lane: usize) {
        self.bits ^= 1 << lane;
    }

    /// Number of active lanes.
    #[inline]
    pub fn count(&self) -> u32 {
        self.bits.count_ones()
    }

    /// True if any lane is active.
    #[inline]
    pub fn any(&self) -> bool {
        self.bits != 0
    }

    /// True if all lanes are active.
    #[inline]
    pub fn all_set(&self) -> bool {
        self.bits == (1u32 << WIDTH) - 1
    }

    /// Bitwise AND (intersection).
    pub fn and(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }

    /// Bitwise OR (union).
    pub fn or(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Bitwise NOT (complement). Named `complement` to avoid confusion with [`std::ops::Not::not`].
    pub fn complement(self) -> Self {
        Self {
            bits: !self.bits & ((1u32 << WIDTH) - 1),
        }
    }
}

impl<const WIDTH: usize> fmt::Debug for Mask<WIDTH> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mask<{WIDTH}>(0b{:0>width$b})", self.bits, width = WIDTH)
    }
}

// ---------------------------------------------------------------------------
// Masked — wide value + mask for conditional operations
// ---------------------------------------------------------------------------

/// A reference to a wide value with an associated activity mask.
///
/// Operations through a `Masked` only affect lanes where the mask bit is set.
/// Matches C++ `OSL::Masked<T, WidthT>`.
pub struct Masked<'a, T: Copy, const WIDTH: usize> {
    pub data: &'a mut Wide<T, WIDTH>,
    pub mask: Mask<WIDTH>,
}

impl<'a, T: Copy + Default, const WIDTH: usize> Masked<'a, T, WIDTH> {
    pub fn new(data: &'a mut Wide<T, WIDTH>, mask: Mask<WIDTH>) -> Self {
        Self { data, mask }
    }

    /// Store a value only to active lanes.
    pub fn assign(&mut self, val: T) {
        for i in 0..WIDTH {
            if self.mask.is_set(i) {
                self.data.data[i] = val;
            }
        }
    }

    /// Store per-lane values only to active lanes.
    pub fn assign_wide(&mut self, src: &Wide<T, WIDTH>) {
        for i in 0..WIDTH {
            if self.mask.is_set(i) {
                self.data.data[i] = src.data[i];
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BatchedShaderGlobals<WIDTH>
// ---------------------------------------------------------------------------

/// Batched shader globals: `WIDTH` shading points processed simultaneously.
///
/// Mirrors C++ `BatchedShaderGlobals<WidthT>`. Each field from
/// [`ShaderGlobals`] is widened to `Wide<T, WIDTH>`.
pub struct BatchedShaderGlobals<const WIDTH: usize> {
    // -- Surface position and derivatives --
    pub p: Wide<Vec3, WIDTH>,
    pub dp_dx: Wide<Vec3, WIDTH>,
    pub dp_dy: Wide<Vec3, WIDTH>,
    pub dp_dz: Wide<Vec3, WIDTH>,

    // -- Incident ray --
    pub i: Wide<Vec3, WIDTH>,
    pub di_dx: Wide<Vec3, WIDTH>,
    pub di_dy: Wide<Vec3, WIDTH>,

    // -- Normals --
    pub n: Wide<Vec3, WIDTH>,
    pub ng: Wide<Vec3, WIDTH>,

    // -- UV parameters --
    pub u: Wide<Float, WIDTH>,
    pub dudx: Wide<Float, WIDTH>,
    pub dudy: Wide<Float, WIDTH>,
    pub v: Wide<Float, WIDTH>,
    pub dvdx: Wide<Float, WIDTH>,
    pub dvdy: Wide<Float, WIDTH>,

    // -- Surface tangents --
    pub dp_du: Wide<Vec3, WIDTH>,
    pub dp_dv: Wide<Vec3, WIDTH>,

    // -- Time --
    pub time: Wide<Float, WIDTH>,
    pub dtime: Wide<Float, WIDTH>,
    pub dp_dtime: Wide<Vec3, WIDTH>,

    // -- Light point --
    pub ps: Wide<Vec3, WIDTH>,
    pub dps_dx: Wide<Vec3, WIDTH>,
    pub dps_dy: Wide<Vec3, WIDTH>,

    // -- Coordinate transforms (varying, per-lane opaque pointers) --
    /// Object-to-common transformation pointer per lane.
    /// C++ type: `Block<TransformationPtr>`.
    pub object2common: Wide<usize, WIDTH>,
    /// Shader-to-common transformation pointer per lane.
    /// C++ type: `Block<TransformationPtr>`.
    pub shader2common: Wide<usize, WIDTH>,

    // -- Output closure (varying, per-lane) --
    /// Closure color output pointer per lane.
    /// C++ type: `Block<ClosureColor*>`.
    pub ci: Wide<usize, WIDTH>,

    // -- Miscellaneous (varying) --
    pub surfacearea: Wide<Float, WIDTH>,
    /// If nonzero, flip the result of calculatenormal().
    /// C++ type: `Block<int>`.
    pub flip_handedness: Wide<i32, WIDTH>,
    pub backfacing: Wide<i32, WIDTH>,

    // -- Lane activity --
    pub mask: Mask<WIDTH>,

    // -- Uniform (shared) data that doesn't vary per lane --
    // Note: raytype is UNIFORM in C++ (UniformShaderGlobals).
    // Access via `self.uniform.raytype`.
    pub uniform: ShaderGlobals,

    /// Texture options (wrap, filter, MIP). Matches C++ context->batched_texture_options().
    /// Set before execution to affect texture/texture3d/environment lookups.
    pub texture_options: crate::context::BatchedTextureOptions,
}

impl<const WIDTH: usize> BatchedShaderGlobals<WIDTH> {
    /// Create with all lanes zeroed, mask = all active.
    pub fn new() -> Self {
        Self {
            p: Wide::default(),
            dp_dx: Wide::default(),
            dp_dy: Wide::default(),
            dp_dz: Wide::default(),
            i: Wide::default(),
            di_dx: Wide::default(),
            di_dy: Wide::default(),
            n: Wide::default(),
            ng: Wide::default(),
            u: Wide::default(),
            dudx: Wide::default(),
            dudy: Wide::default(),
            v: Wide::default(),
            dvdx: Wide::default(),
            dvdy: Wide::default(),
            dp_du: Wide::default(),
            dp_dv: Wide::default(),
            time: Wide::default(),
            dtime: Wide::default(),
            dp_dtime: Wide::default(),
            ps: Wide::default(),
            dps_dx: Wide::default(),
            dps_dy: Wide::default(),
            object2common: Wide::default(),
            shader2common: Wide::default(),
            ci: Wide::default(),
            surfacearea: Wide::default(),
            flip_handedness: Wide::default(),
            backfacing: Wide::default(),
            mask: Mask::all(),
            uniform: ShaderGlobals::default(),
            texture_options: crate::context::BatchedTextureOptions::default(),
        }
    }

    /// Load one lane from a scalar ShaderGlobals.
    pub fn load_lane(&mut self, lane: usize, sg: &ShaderGlobals) {
        self.p.store(lane, sg.p);
        self.dp_dx.store(lane, sg.dp_dx);
        self.dp_dy.store(lane, sg.dp_dy);
        self.dp_dz.store(lane, sg.dp_dz);
        self.i.store(lane, sg.i);
        self.di_dx.store(lane, sg.di_dx);
        self.di_dy.store(lane, sg.di_dy);
        self.n.store(lane, sg.n);
        self.ng.store(lane, sg.ng);
        self.u.store(lane, sg.u);
        self.dudx.store(lane, sg.dudx);
        self.dudy.store(lane, sg.dudy);
        self.v.store(lane, sg.v);
        self.dvdx.store(lane, sg.dvdx);
        self.dvdy.store(lane, sg.dvdy);
        self.dp_du.store(lane, sg.dp_du);
        self.dp_dv.store(lane, sg.dp_dv);
        self.time.store(lane, sg.time);
        self.dtime.store(lane, sg.dtime);
        self.dp_dtime.store(lane, sg.dp_dtime);
        self.ps.store(lane, sg.ps);
        self.dps_dx.store(lane, sg.dps_dx);
        self.dps_dy.store(lane, sg.dps_dy);
        self.object2common.store(lane, sg.object2common as usize);
        self.shader2common.store(lane, sg.shader2common as usize);
        self.ci.store(lane, sg.ci as usize);
        self.surfacearea.store(lane, sg.surfacearea);
        self.flip_handedness.store(lane, sg.flip_handedness);
        self.backfacing.store(lane, sg.backfacing);
    }

    /// Extract one lane as a scalar ShaderGlobals.
    pub fn extract_lane(&self, lane: usize) -> ShaderGlobals {
        ShaderGlobals {
            p: self.p.load(lane),
            dp_dx: self.dp_dx.load(lane),
            dp_dy: self.dp_dy.load(lane),
            dp_dz: self.dp_dz.load(lane),
            i: self.i.load(lane),
            di_dx: self.di_dx.load(lane),
            di_dy: self.di_dy.load(lane),
            n: self.n.load(lane),
            ng: self.ng.load(lane),
            u: self.u.load(lane),
            dudx: self.dudx.load(lane),
            dudy: self.dudy.load(lane),
            v: self.v.load(lane),
            dvdx: self.dvdx.load(lane),
            dvdy: self.dvdy.load(lane),
            dp_du: self.dp_du.load(lane),
            dp_dv: self.dp_dv.load(lane),
            time: self.time.load(lane),
            dtime: self.dtime.load(lane),
            dp_dtime: self.dp_dtime.load(lane),
            ps: self.ps.load(lane),
            dps_dx: self.dps_dx.load(lane),
            dps_dy: self.dps_dy.load(lane),
            object2common: self.object2common.load(lane) as TransformationPtr,
            shader2common: self.shader2common.load(lane) as TransformationPtr,
            ci: self.ci.load(lane) as ClosureColorPtr,
            surfacearea: self.surfacearea.load(lane),
            raytype: self.uniform.raytype,
            flip_handedness: self.flip_handedness.load(lane),
            backfacing: self.backfacing.load(lane),
            ..Default::default()
        }
    }

    /// Write a scalar ShaderGlobals back into a specific lane.
    ///
    /// This is the inverse of `extract_lane` — after a scalar JIT function
    /// modifies a ShaderGlobals, inject the results back into the batch.
    #[inline]
    pub fn inject_lane(&mut self, lane: usize, sg: &ShaderGlobals) {
        self.load_lane(lane, sg);
    }
}

impl<const WIDTH: usize> Default for BatchedShaderGlobals<WIDTH> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BatchedRendererServices<WIDTH> trait
// ---------------------------------------------------------------------------

/// Batched renderer interface — SIMD-width version of [`RendererServices`].
///
/// All methods operate on `WIDTH` shading points simultaneously. The `mask`
/// parameter indicates which lanes are active; inactive lanes should be
/// skipped. Return masks indicate which lanes succeeded.
///
/// Matches C++ `BatchedRendererServices<WidthT>`.
///
/// [`RendererServices`]: crate::renderer::RendererServices
#[allow(unused_variables)]
pub trait BatchedRendererServices<const WIDTH: usize>: Send + Sync {
    /// Return whether this renderer supports a named feature.
    fn supports(&self, feature: &str) -> bool {
        false
    }

    // -- Coordinate transformations ----------------------------------------

    /// Get transform matrix for named space, per-lane time.
    fn get_matrix_named(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        from: UStringHash,
        time: &Wide<Float, WIDTH>,
        result: &mut Wide<Matrix44, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Get inverse matrix for named space.
    fn get_inverse_matrix_named(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        to: UStringHash,
        time: &Wide<Float, WIDTH>,
        result: &mut Wide<Matrix44, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Transform points between coordinate systems.
    fn transform_points(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        from: UStringHash,
        to: UStringHash,
        time: &Wide<Float, WIDTH>,
        pin: &Wide<Vec3, WIDTH>,
        pout: &mut Wide<Vec3, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    // -- Attributes --------------------------------------------------------

    /// Check if an attribute can be treated as uniform across all batched lanes.
    /// Matches C++ `BatchedRendererServices::is_attribute_uniform`.
    fn is_attribute_uniform(&self, object: UStringHash, name: UStringHash) -> bool {
        false
    }

    /// Batched attribute query.
    fn get_attribute(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        derivatives: bool,
        object: UStringHash,
        type_desc: TypeDesc,
        name: UStringHash,
        result_float: &mut Wide<Float, WIDTH>,
        result_int: &mut Wide<i32, WIDTH>,
        result_vec3: &mut Wide<Vec3, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Batched array attribute query.
    fn get_array_attribute(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        derivatives: bool,
        object: UStringHash,
        type_desc: TypeDesc,
        name: UStringHash,
        index: &Wide<i32, WIDTH>,
        result_float: &mut Wide<Float, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Uniform (non-varying) attribute query.
    /// Matches C++ `BatchedRendererServices::get_attribute_uniform`.
    fn get_attribute_uniform(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        object: UStringHash,
        name: UStringHash,
        type_desc: TypeDesc,
        val: *mut c_void,
    ) -> bool {
        false
    }

    /// Uniform array attribute query.
    /// Matches C++ `BatchedRendererServices::get_array_attribute_uniform`.
    fn get_array_attribute_uniform(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        object: UStringHash,
        name: UStringHash,
        index: i32,
        type_desc: TypeDesc,
        val: *mut c_void,
    ) -> bool {
        false
    }

    // -- User data ---------------------------------------------------------

    /// Batched user data query.
    fn get_userdata(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        derivatives: bool,
        name: UStringHash,
        type_desc: TypeDesc,
        result_float: &mut Wide<Float, WIDTH>,
        result_vec3: &mut Wide<Vec3, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    // -- Textures ----------------------------------------------------------

    /// Batched 2D texture lookup.
    /// Matches C++ BatchedRendererServices::texture (options param).
    fn texture(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        perthread: TexturePerthread,
        options: &crate::context::BatchedTextureOptions,
        s: &Wide<Float, WIDTH>,
        t: &Wide<Float, WIDTH>,
        dsdx: &Wide<Float, WIDTH>,
        dtdx: &Wide<Float, WIDTH>,
        dsdy: &Wide<Float, WIDTH>,
        dtdy: &Wide<Float, WIDTH>,
        nchannels: i32,
        result: &mut [Wide<Float, WIDTH>],
        dresultds: Option<&mut [Wide<Float, WIDTH>]>,
        dresultdt: Option<&mut [Wide<Float, WIDTH>]>,
    ) -> Mask<WIDTH> {
        let _ = (
            bsg, mask, filename, handle, perthread, options, s, t, dsdx, dtdx, dsdy, dtdy,
            nchannels, result, dresultds, dresultdt,
        );
        Mask::none()
    }

    /// Batched 3D texture lookup.
    /// Matches C++ BatchedRendererServices::texture3d (options param).
    fn texture3d(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        perthread: TexturePerthread,
        options: &crate::context::BatchedTextureOptions,
        p: &Wide<Vec3, WIDTH>,
        dpdx: &Wide<Vec3, WIDTH>,
        dpdy: &Wide<Vec3, WIDTH>,
        dpdz: &Wide<Vec3, WIDTH>,
        nchannels: i32,
        result: &mut [Wide<Float, WIDTH>],
    ) -> Mask<WIDTH> {
        let _ = (
            bsg, mask, filename, handle, perthread, options, p, dpdx, dpdy, dpdz, nchannels, result,
        );
        Mask::none()
    }

    /// Batched environment map lookup.
    /// Matches C++ BatchedRendererServices::environment (options param).
    fn environment(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        perthread: TexturePerthread,
        options: &crate::context::BatchedTextureOptions,
        r: &Wide<Vec3, WIDTH>,
        drdx: &Wide<Vec3, WIDTH>,
        drdy: &Wide<Vec3, WIDTH>,
        nchannels: i32,
        result: &mut [Wide<Float, WIDTH>],
    ) -> Mask<WIDTH> {
        let _ = (
            bsg, mask, filename, handle, perthread, options, r, drdx, drdy, nchannels, result,
        );
        Mask::none()
    }

    /// Batched texture info query.
    fn get_texture_info(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        subimage: i32,
        dataname: UStringHash,
        datatype: TypeDesc,
        data: *mut c_void,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Uniform texture info query (single result, not per-lane).
    /// Matches C++ `BatchedRendererServices::get_texture_info_uniform`.
    fn get_texture_info_uniform(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        subimage: i32,
        dataname: UStringHash,
        datatype: TypeDesc,
        data: *mut c_void,
    ) -> bool {
        false
    }

    /// Resolve a UDIM texture handle uniformly (single result).
    /// Matches C++ `BatchedRendererServices::resolve_udim_uniform`.
    fn resolve_udim_uniform(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        s: f32,
        t: f32,
    ) -> TextureHandle {
        std::ptr::null_mut()
    }

    /// Resolve UDIM texture handles per-lane.
    /// Matches C++ `BatchedRendererServices::resolve_udim`.
    fn resolve_udim(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        handle: TextureHandle,
        s: &Wide<Float, WIDTH>,
        t: &Wide<Float, WIDTH>,
        result: &mut Wide<usize, WIDTH>,
    ) {
        // Default: leave result unchanged (null handles)
    }

    // -- Point cloud -------------------------------------------------------

    /// Batched point cloud search.
    fn pointcloud_search(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        center: &Wide<Vec3, WIDTH>,
        radius: &Wide<Float, WIDTH>,
        max_points: i32,
        sort: bool,
        out_indices: &mut Wide<i32, WIDTH>,
        out_count: &mut Wide<i32, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Batched point cloud get.
    fn pointcloud_get(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        indices: &Wide<i32, WIDTH>,
        attr_name: UStringHash,
        attr_type: TypeDesc,
        out_data: *mut c_void,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    /// Batched point cloud write.
    /// C++ ref: `BatchedRendererServices<WidthT>::pointcloud_write`.
    fn pointcloud_write(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        filename: UStringHash,
        pos: &Wide<Vec3, WIDTH>,
        nattribs: i32,
        attr_names: &[UStringHash],
        attr_types: &[TypeDesc],
        attr_data: &[*const c_void],
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    // -- Messages -----------------------------------------------------------

    /// Batched getmessage (e.g. from trace results).
    /// C++ ref: `BatchedRendererServices<WidthT>::getmessage`.
    fn getmessage(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        source: UStringHash,
        name: UStringHash,
        result_type: TypeDesc,
        result: *mut c_void,
        result_int: &mut Wide<i32, WIDTH>,
    ) {
        // Default: set result to 0 for all lanes (no message found)
        for lane in 0..WIDTH {
            result_int.store(lane, 0);
        }
    }

    // -- Ray tracing -------------------------------------------------------

    /// Batched ray trace.
    fn trace(
        &self,
        bsg: &BatchedShaderGlobals<WIDTH>,
        mask: Mask<WIDTH>,
        options: &TraceOpt,
        p: &Wide<Vec3, WIDTH>,
        dpdx: &Wide<Vec3, WIDTH>,
        dpdy: &Wide<Vec3, WIDTH>,
        r: &Wide<Vec3, WIDTH>,
        drdx: &Wide<Vec3, WIDTH>,
        drdy: &Wide<Vec3, WIDTH>,
    ) -> Mask<WIDTH> {
        Mask::none()
    }

    // -- Renderer info ---------------------------------------------------

    fn renderer_name(&self) -> &str {
        "unknown_batched"
    }
}

// ---------------------------------------------------------------------------
// NullBatchedRenderer — default no-op implementation
// ---------------------------------------------------------------------------

/// A null batched renderer that returns no results for everything.
pub struct NullBatchedRenderer;

impl<const WIDTH: usize> BatchedRendererServices<WIDTH> for NullBatchedRenderer {
    fn renderer_name(&self) -> &str {
        "null_batched"
    }
}

// ---------------------------------------------------------------------------
// Standard width type aliases
// ---------------------------------------------------------------------------

/// Standard AVX width (8 lanes).
pub const WIDTH_AVX: usize = 8;
/// AVX-512 width (16 lanes).
pub const WIDTH_AVX512: usize = 16;

pub type Wide8F = Wide<Float, WIDTH_AVX>;
pub type Wide8I = Wide<i32, WIDTH_AVX>;
pub type Wide8V3 = Wide<Vec3, WIDTH_AVX>;
pub type Wide8C3 = Wide<Color3, WIDTH_AVX>;
pub type Wide8M44 = Wide<Matrix44, WIDTH_AVX>;
pub type Mask8 = Mask<WIDTH_AVX>;
pub type BatchedSG8 = BatchedShaderGlobals<WIDTH_AVX>;

pub type Wide16F = Wide<Float, WIDTH_AVX512>;
pub type Wide16I = Wide<i32, WIDTH_AVX512>;
pub type Wide16V3 = Wide<Vec3, WIDTH_AVX512>;
pub type Wide16C3 = Wide<Color3, WIDTH_AVX512>;
pub type Wide16M44 = Wide<Matrix44, WIDTH_AVX512>;
pub type Mask16 = Mask<WIDTH_AVX512>;
pub type BatchedSG16 = BatchedShaderGlobals<WIDTH_AVX512>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wide_splat() {
        let w: Wide<Float, 8> = Wide::splat(3.14);
        for i in 0..8 {
            assert!((w.load(i) - 3.14).abs() < 1e-6);
        }
    }

    #[test]
    fn test_wide_store_load() {
        let mut w: Wide<Float, 8> = Wide::zero();
        w.store(3, 42.0);
        assert!((w.load(3) - 42.0).abs() < 1e-6);
        assert!((w.load(0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_wide_add() {
        let a: Wide<Float, 8> = Wide::splat(1.0);
        let b: Wide<Float, 8> = Wide::splat(2.0);
        let c = a.add(&b);
        for i in 0..8 {
            assert!((c.load(i) - 3.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_wide_vec3() {
        let mut w: Wide<Vec3, 4> = Wide::default();
        w.store(0, Vec3::new(1.0, 2.0, 3.0));
        w.store(1, Vec3::new(4.0, 5.0, 6.0));
        assert!((w.load(0).x - 1.0).abs() < 1e-6);
        assert!((w.load(1).y - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_mask_basic() {
        let mut m: Mask<8> = Mask::none();
        assert!(!m.any());
        assert_eq!(m.count(), 0);

        m.set(0);
        m.set(3);
        m.set(7);
        assert!(m.any());
        assert_eq!(m.count(), 3);
        assert!(m.is_set(0));
        assert!(!m.is_set(1));
        assert!(m.is_set(3));
    }

    #[test]
    fn test_mask_all() {
        let m: Mask<8> = Mask::all();
        assert!(m.all_set());
        assert_eq!(m.count(), 8);
        for i in 0..8 {
            assert!(m.is_set(i));
        }
    }

    #[test]
    fn test_mask_ops() {
        let a = Mask::<8>::from_bits(0b10101010);
        let b = Mask::<8>::from_bits(0b11001100);

        let and = a.and(b);
        assert_eq!(and.bits(), 0b10001000);

        let or = a.or(b);
        assert_eq!(or.bits(), 0b11101110);

        let not_a = a.complement();
        assert_eq!(not_a.bits(), 0b01010101);
    }

    #[test]
    fn test_masked_assign() {
        let mut w: Wide<Float, 8> = Wide::zero();
        let mask = Mask::from_bits(0b00001010); // lanes 1, 3

        {
            let mut m = Masked::new(&mut w, mask);
            m.assign(99.0);
        }

        assert!((w.load(0) - 0.0).abs() < 1e-6);
        assert!((w.load(1) - 99.0).abs() < 1e-6);
        assert!((w.load(2) - 0.0).abs() < 1e-6);
        assert!((w.load(3) - 99.0).abs() < 1e-6);
    }

    #[test]
    fn test_batched_sg_default() {
        let bsg: BatchedShaderGlobals<8> = BatchedShaderGlobals::new();
        assert!(bsg.mask.all_set());
        assert!((bsg.p.load(0).x - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_batched_sg_load_extract() {
        let mut bsg: BatchedShaderGlobals<4> = BatchedShaderGlobals::new();

        let mut sg = ShaderGlobals::default();
        sg.p = Vec3::new(1.0, 2.0, 3.0);
        sg.n = Vec3::new(0.0, 1.0, 0.0);
        sg.u = 0.5;

        bsg.load_lane(2, &sg);

        let extracted = bsg.extract_lane(2);
        assert!((extracted.p.x - 1.0).abs() < 1e-6);
        assert!((extracted.n.y - 1.0).abs() < 1e-6);
        assert!((extracted.u - 0.5).abs() < 1e-6);

        // Other lanes should be zero
        let other = bsg.extract_lane(0);
        assert!((other.p.x - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_null_batched_renderer() {
        let r = NullBatchedRenderer;
        let bsg: BatchedShaderGlobals<8> = BatchedShaderGlobals::new();
        let mask = Mask::all();
        let time = Wide::zero();
        let mut result = Wide::default();

        let ok = BatchedRendererServices::<8>::get_matrix_named(
            &r,
            &bsg,
            mask,
            UStringHash::EMPTY,
            &time,
            &mut result,
        );
        assert!(!ok.any());
        assert_eq!(
            BatchedRendererServices::<8>::renderer_name(&r),
            "null_batched"
        );
    }

    #[test]
    fn test_type_aliases() {
        let _: Wide8F = Wide::zero();
        let _: Wide8V3 = Wide::default();
        let _: Mask8 = Mask::all();
        let _: Wide16F = Wide::zero();
        let _: Mask16 = Mask::all();
    }

    #[test]
    fn test_batched_sg_missing_fields_parity() {
        // Verify the 4 fields that were missing from C++ parity are present
        let bsg: BatchedShaderGlobals<4> = BatchedShaderGlobals::new();
        // object2common, shader2common: default is 0 (null pointer as usize)
        assert_eq!(bsg.object2common.load(0), 0);
        assert_eq!(bsg.shader2common.load(0), 0);
        // ci: default is 0 (null pointer)
        assert_eq!(bsg.ci.load(0), 0);
        // flip_handedness: default is 0
        assert_eq!(bsg.flip_handedness.load(0), 0);
    }

    #[test]
    fn test_batched_sg_raytype_is_uniform() {
        // C++ parity: raytype is UNIFORM (in UniformShaderGlobals),
        // NOT varying. We access it via uniform.raytype.
        let mut bsg: BatchedShaderGlobals<4> = BatchedShaderGlobals::new();
        bsg.uniform.raytype = 42;
        // extract_lane should return the uniform raytype for all lanes
        for lane in 0..4 {
            let sg = bsg.extract_lane(lane);
            assert_eq!(sg.raytype, 42);
        }
    }

    #[test]
    fn test_batched_sg_load_lane_new_fields() {
        let mut bsg: BatchedShaderGlobals<4> = BatchedShaderGlobals::new();
        let mut sg = ShaderGlobals::default();
        sg.flip_handedness = 1;
        sg.backfacing = 1;
        // Set object2common/shader2common to some sentinel values
        sg.object2common = 0xDEAD as *const std::ffi::c_void;
        sg.shader2common = 0xBEEF as *const std::ffi::c_void;

        bsg.load_lane(1, &sg);

        assert_eq!(bsg.flip_handedness.load(1), 1);
        assert_eq!(bsg.backfacing.load(1), 1);
        assert_eq!(bsg.object2common.load(1), 0xDEAD);
        assert_eq!(bsg.shader2common.load(1), 0xBEEF);

        let extracted = bsg.extract_lane(1);
        assert_eq!(extracted.flip_handedness, 1);
        assert_eq!(extracted.object2common as usize, 0xDEAD);
        assert_eq!(extracted.shader2common as usize, 0xBEEF);
    }

    #[test]
    fn test_wide_splat_non_default() {
        // Test Wide::splat works without Default bound (raw pointer-like types)
        let w: Wide<usize, 8> = Wide::splat(42);
        for i in 0..8 {
            assert_eq!(w.load(i), 42);
        }
    }

    #[test]
    fn test_mask_value_method() {
        // C++ parity: Mask::value() is equivalent to our bits()
        let m = Mask::<8>::from_bits(0b10110011);
        assert_eq!(m.bits(), 0b10110011);
        // Verify Mask(true) == all, Mask(false) == none
        assert_eq!(Mask::<8>::all().bits(), 0xFF);
        assert_eq!(Mask::<8>::none().bits(), 0);
    }
}
