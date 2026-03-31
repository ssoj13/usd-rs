//! Shared helpers for adaptive rANS bit coding.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/adaptive_rans_bit_coding_shared.h`.
//!
//! Provides probability clamping and adaptive update used by adaptive rANS.

use draco_core::{draco_dcheck_ge, draco_dcheck_le};

/// Clamp the probability p to a u8 in [1, 255].
#[inline]
pub fn clamp_probability(p: f64) -> u8 {
    draco_dcheck_le!(p, 1.0);
    draco_dcheck_ge!(p, 0.0);
    let mut p_int = (p * 256.0 + 0.5) as u32;
    if p_int == 256 {
        p_int -= 1;
    }
    if p_int == 0 {
        p_int += 1;
    }
    p_int as u8
}

/// Update the probability according to new incoming bit.
#[inline]
pub fn update_probability(old_p: f64, bit: bool) -> f64 {
    const W: f64 = 128.0;
    const W0: f64 = (W - 1.0) / W;
    const W1: f64 = 1.0 / W;
    old_p * W0 + ((!bit) as i32 as f64) * W1
}
