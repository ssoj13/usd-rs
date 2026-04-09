//! Gabor noise — isotropic, anisotropic, hybrid modes.
//!
//! Port of `gabornoise.cpp` / `gabornoise.h` from OSL (plan #48–49).
//! C++ reference: scramble, GaborParams formulas, fast_rng, variance scale.

use crate::Float;
use crate::hashes::{bits_to_01, inthash4};
use crate::math::Vec3;

const GABOR_FREQUENCY: Float = 2.0;
const GABOR_IMPULSE_WEIGHT: Float = 1.0;
const GABOR_TRUNCATE: Float = 0.02;
const SQRT_PI_OVER_LN2: Float = 2.128934e+00;

/// Gabor noise parameters (matches C++ NoiseParams / GaborParams).
#[derive(Debug, Clone, Copy)]
pub struct GaborParams {
    /// Anisotropy: 0=isotropic, 1=anisotropic, 2=hybrid.
    pub anisotropic: i32,
    /// Whether to apply footprint filtering (requires derivs).
    pub do_filter: bool,
    /// Preferred direction for anisotropic.
    pub direction: Vec3,
    /// Bandwidth (clamped 0.01..100).
    pub bandwidth: Float,
    /// Impulses per cell (clamped 1..32).
    pub impulses: Float,
    /// If Some, use periodic mode (pgabor) with this period.
    pub period: Option<Vec3>,
}

impl Default for GaborParams {
    fn default() -> Self {
        Self {
            anisotropic: 0,
            do_filter: true,
            direction: Vec3::new(1.0, 0.0, 0.0),
            bandwidth: 1.0,
            impulses: 64.0,
            period: None,
        }
    }
}

/// Internal params derived from GaborParams (matches C++ GaborParams ctor).
struct GaborParamsInternal {
    a: Float,
    weight: Float,
    radius: Float,
    radius2: Float,
    radius3: Float,
    radius_inv: Float,
    lambda: Float,
    sqrt_lambda_inv: Float,
    omega: Vec3,
    anisotropic: i32,
    periodic: bool,
    period: Vec3,
}

impl GaborParamsInternal {
    fn from_opt(opt: &GaborParams) -> Self {
        let bandwidth = opt.bandwidth.clamp(0.01, 100.0);
        let two_bw = Float::exp2(bandwidth);
        let a = GABOR_FREQUENCY * ((two_bw - 1.0) / (two_bw + 1.0)) * SQRT_PI_OVER_LN2;
        let radius = (-GABOR_TRUNCATE.ln() / std::f32::consts::PI).sqrt() / a;
        let radius2 = radius * radius;
        let radius3 = radius2 * radius;
        let radius_inv = 1.0 / radius;
        let impulses = opt.impulses.clamp(1.0, 32.0);
        let lambda = impulses / (1.33333 * std::f32::consts::PI * radius3);
        let sqrt_lambda_inv = 1.0 / lambda.sqrt();
        let omega = opt.direction.normalize();
        let (periodic, period) = match opt.period {
            Some(p) => (true, Vec3::new(p.x.max(1.0), p.y.max(1.0), p.z.max(1.0))),
            None => (false, Vec3::ZERO),
        };

        Self {
            a,
            weight: GABOR_IMPULSE_WEIGHT,
            radius,
            radius2,
            radius3,
            radius_inv,
            lambda,
            sqrt_lambda_inv,
            omega,
            anisotropic: opt.anisotropic,
            periodic,
            period,
        }
    }
}

/// Fast RNG matching C++ fast_rng (Borosh–Niederreiter LCG).
struct FastRng {
    seed: u32,
}

impl FastRng {
    fn new(p: Vec3, seed: i32) -> Self {
        Self::new_inner(p, seed)
    }
    fn new_wrapped(p: Vec3, period: Vec3, seed: i32) -> Self {
        let wp = wrap_vec3(p, period);
        Self::new_inner(wp, seed)
    }
    fn new_inner(p: Vec3, seed: i32) -> Self {
        let ix = p.x.floor() as i32;
        let iy = p.y.floor() as i32;
        let iz = p.z.floor() as i32;
        let mut s = inthash4(ix as u32, iy as u32, iz as u32, seed as u32);
        if s == 0 {
            s = 1;
        }
        Self { seed: s }
    }

    fn next(&mut self) -> Float {
        self.seed = self.seed.wrapping_mul(3039177861);
        bits_to_01(self.seed)
    }

    fn poisson(&mut self, mean: Float) -> u32 {
        let g = (-mean).exp();
        let mut em: u32 = 0;
        let mut t = self.next();
        while t > g {
            em += 1;
            t *= self.next();
        }
        em
    }
}

/// Gabor kernel: weight * exp(-π a² |x|²) * cos(2π dot(ω,x) + φ)
#[inline]
fn gabor_kernel_val(weight: Float, omega: Vec3, phi: Float, a: Float, x: Vec3) -> Float {
    let x2 = x.dot(x);
    let g = (-std::f32::consts::PI * a * a * x2).exp();
    let h = (2.0 * std::f32::consts::PI * omega.dot(x) + phi).cos();
    weight * g * h
}

/// Sample omega and phi for one impulse (matches C++ gabor_sample).
fn gabor_sample(gp: &GaborParamsInternal, _x_c: Vec3, rng: &mut FastRng) -> (Vec3, Float) {
    let omega = match gp.anisotropic {
        1 => gp.omega,
        0 => {
            let omega_t = 2.0 * std::f32::consts::PI * rng.next();
            let cos_omega_p = lerp(-1.0, 1.0, rng.next());
            let sin_omega_p = (1.0 - cos_omega_p * cos_omega_p).max(0.0).sqrt();
            let (sin_t, cos_t) = omega_t.sin_cos();
            Vec3::new(cos_t * sin_omega_p, sin_t * sin_omega_p, cos_omega_p).normalize()
        }
        _ => {
            let omega_r = gp.omega.length();
            let omega_t = 2.0 * std::f32::consts::PI * rng.next();
            let (sin_t, cos_t) = omega_t.sin_cos();
            Vec3::new(omega_r * cos_t, omega_r * sin_t, 0.0)
        }
    };
    let phi = 2.0 * std::f32::consts::PI * rng.next();
    (omega, phi)
}

#[inline]
fn lerp(a: Float, b: Float, t: Float) -> Float {
    a + t * (b - a)
}

fn wrap_val(s: Float, period: Float) -> Float {
    let p = period.floor().max(1.0);
    s - p * (s / p).floor()
}

fn wrap_vec3(s: Vec3, period: Vec3) -> Vec3 {
    Vec3::new(
        wrap_val(s.x, period.x),
        wrap_val(s.y, period.y),
        wrap_val(s.z, period.z),
    )
}

/// Evaluate one cell's impulses (matches C++ gabor_cell, unfiltered path).
fn gabor_cell(gp: &GaborParamsInternal, c_i: Vec3, x_c_i: Vec3, seed: i32) -> Float {
    let mut rng = if gp.periodic {
        FastRng::new_wrapped(c_i, gp.period, seed)
    } else {
        FastRng::new(c_i, seed)
    };
    let n_impulses = rng.poisson(gp.lambda * gp.radius3);

    let mut sum = 0.0_f32;
    for _ in 0..n_impulses {
        let z_rng = rng.next();
        let y_rng = rng.next();
        let x_rng = rng.next();
        let x_i_c = Vec3::new(x_rng, y_rng, z_rng);
        let x_k_i = gp.radius * (x_c_i - x_i_c);
        let x_k_i_2 = x_k_i.dot(x_k_i);
        if x_k_i_2 < gp.radius2 {
            let (omega_i, phi_i) = gabor_sample(gp, c_i, &mut rng);
            sum += gabor_kernel_val(gp.weight, omega_i, phi_i, gp.a, x_k_i);
        }
    }
    sum
}

/// Sum over 3×3×3 cells (matches C++ gabor_grid).
fn gabor_grid(gp: &GaborParamsInternal, x_g: Vec3, seed: i32) -> Float {
    let floor_x_g = Vec3::new(x_g.x.floor(), x_g.y.floor(), x_g.z.floor());
    let x_c = x_g - floor_x_g;

    let mut sum = 0.0_f32;
    for k in -1..=1 {
        for j in -1..=1 {
            for i in -1..=1 {
                let c = Vec3::new(i as Float, j as Float, k as Float);
                let c_i = floor_x_g + c;
                let x_c_i = x_c - c;
                sum += gabor_cell(gp, c_i, x_c_i, seed);
            }
        }
    }
    sum * gp.sqrt_lambda_inv
}

/// Variance normalization (matches C++ gabor/gabor3 result scaling).
fn gabor_variance_scale(a: Float) -> Float {
    let gabor_variance = 1.0 / (4.0 * 2.0_f32.sqrt() * (a * a * a));
    0.5 / (3.0 * gabor_variance.sqrt())
}

/// Evaluate 3D Gabor noise at position `p` (plan #48–49 parity).
pub fn gabor3(p: Vec3, params: &GaborParams) -> Float {
    let gp = GaborParamsInternal::from_opt(params);
    let x_g = p * gp.radius_inv;
    let raw = gabor_grid(&gp, x_g, 0);
    raw * gabor_variance_scale(gp.a)
}

/// Evaluate with default parameters.
pub fn gabor3_default(p: Vec3) -> Float {
    gabor3(p, &GaborParams::default())
}

/// Gabor kernel derivative for analytical deriv path.
/// C++ `gabor_kernel` multiplies by weight: `weight * g * h`.
fn gabor_kernel_deriv(weight: Float, r: Vec3, omega: Vec3, phi: Float, a: Float) -> (Float, Vec3) {
    let r2 = r.dot(r);
    let a2 = a * a;
    let pi = std::f32::consts::PI;
    let envelope = (-pi * a2 * r2).exp();
    let phase = 2.0 * pi * omega.dot(r) + phi;
    let (sin_p, cos_p) = phase.sin_cos();
    let val = weight * envelope * cos_p;
    let grad = Vec3::new(
        weight * envelope * (-sin_p * 2.0 * pi * omega.x - cos_p * 2.0 * pi * a2 * r.x),
        weight * envelope * (-sin_p * 2.0 * pi * omega.y - cos_p * 2.0 * pi * a2 * r.y),
        weight * envelope * (-sin_p * 2.0 * pi * omega.z - cos_p * 2.0 * pi * a2 * r.z),
    );
    (val, grad)
}

/// Evaluate 3D Gabor with analytical derivatives.
pub fn gabor3_deriv(p: Vec3, params: &GaborParams) -> (Float, Vec3) {
    let gp = GaborParamsInternal::from_opt(params);
    let scale = gabor_variance_scale(gp.a);

    let x_g = p * gp.radius_inv;
    let floor_x_g = Vec3::new(x_g.x.floor(), x_g.y.floor(), x_g.z.floor());
    let x_c = x_g - floor_x_g;

    let mut sum_val = 0.0_f32;
    let mut sum_grad = Vec3::ZERO;

    for k in -1..=1 {
        for j in -1..=1 {
            for i in -1..=1 {
                let c = Vec3::new(i as Float, j as Float, k as Float);
                let c_i = floor_x_g + c;
                let x_c_i = x_c - c;

                let mut rng = if gp.periodic {
                    FastRng::new_wrapped(c_i, gp.period, 0)
                } else {
                    FastRng::new(c_i, 0)
                };
                let n_impulses = rng.poisson(gp.lambda * gp.radius3);

                for _ in 0..n_impulses {
                    let z_rng = rng.next();
                    let y_rng = rng.next();
                    let x_rng = rng.next();
                    let x_i_c = Vec3::new(x_rng, y_rng, z_rng);
                    let x_k_i = gp.radius * (x_c_i - x_i_c);
                    if x_k_i.dot(x_k_i) < gp.radius2 {
                        let (omega_i, phi_i) = gabor_sample(&gp, c_i, &mut rng);
                        let (kval, kgrad) =
                            gabor_kernel_deriv(gp.weight, x_k_i, omega_i, phi_i, gp.a);
                        sum_val += kval;
                        // d(sum)/dp = kgrad (since dx_k_i/dp = 1 via radius*radius_inv)
                        sum_grad += kgrad;
                    }
                }
            }
        }
    }

    let raw = sum_val * gp.sqrt_lambda_inv;
    let grad_raw = sum_grad * gp.sqrt_lambda_inv;
    (raw * scale, grad_raw * scale)
}

/// Isotropic 3D Gabor.
pub fn gabor3_isotropic(p: Vec3, bandwidth: Float, _frequency: Float) -> Float {
    gabor3(
        p,
        &GaborParams {
            anisotropic: 0,
            bandwidth,
            ..Default::default()
        },
    )
}

/// Anisotropic 3D Gabor.
pub fn gabor3_anisotropic(p: Vec3, direction: Vec3, bandwidth: Float, _frequency: Float) -> Float {
    gabor3(
        p,
        &GaborParams {
            anisotropic: 1,
            direction,
            bandwidth,
            ..Default::default()
        },
    )
}

// ---------------------------------------------------------------------------
// 2D / periodic (C++ slices 3D for 2D)
// ---------------------------------------------------------------------------

/// 2D Gabor noise (C++ gabor(x,y) slices 3D at z=0).
pub fn gabor2d(x: Float, y: Float, params: &GaborParams) -> Float {
    gabor3(Vec3::new(x, y, 0.0), params)
}

/// Periodic 3D Gabor (C++ pgabor with Pperiod). Periodicity via wrapped cell RNG seed.
pub fn pgabor3d(p: Vec3, params: &GaborParams, period: Vec3) -> Float {
    let mut pparams = *params;
    pparams.period = Some(period);
    gabor3(p, &pparams)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gabor_deterministic() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let v1 = gabor3_default(p);
        let v2 = gabor3_default(p);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_gabor_isotropic() {
        let p = Vec3::new(0.5, 0.5, 0.5);
        let v = gabor3_isotropic(p, 1.0, 1.0);
        assert!(v.is_finite());
    }

    #[test]
    fn test_gabor_anisotropic() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let v = gabor3_anisotropic(p, dir, 1.0, 2.0);
        assert!(v.is_finite());
    }

    #[test]
    fn test_gabor_deriv_finite() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let (val, grad) = gabor3_deriv(p, &GaborParams::default());
        assert!(val.is_finite());
        assert!(grad.x.is_finite());
        assert!(grad.y.is_finite());
        assert!(grad.z.is_finite());
    }

    #[test]
    fn test_gabor_deriv_numerical() {
        let p = Vec3::new(0.7, 1.3, 2.1);
        let params = GaborParams::default();
        let (val, grad) = gabor3_deriv(p, &params);
        let eps = 1e-3;
        let num_dx = (gabor3(Vec3::new(p.x + eps, p.y, p.z), &params)
            - gabor3(Vec3::new(p.x - eps, p.y, p.z), &params))
            / (2.0 * eps);
        let num_dy = (gabor3(Vec3::new(p.x, p.y + eps, p.z), &params)
            - gabor3(Vec3::new(p.x, p.y - eps, p.z), &params))
            / (2.0 * eps);
        let num_dz = (gabor3(Vec3::new(p.x, p.y, p.z + eps), &params)
            - gabor3(Vec3::new(p.x, p.y, p.z - eps), &params))
            / (2.0 * eps);
        assert!((grad.x - num_dx).abs() < 1.0);
        assert!((grad.y - num_dy).abs() < 1.0);
        assert!((grad.z - num_dz).abs() < 1.0);
        assert!((val - gabor3(p, &params)).abs() < 1e-5);
    }

    #[test]
    fn test_gabor_params_default() {
        let p = GaborParams::default();
        assert_eq!(p.anisotropic, 0);
        assert!(p.do_filter);
        assert_eq!(p.bandwidth, 1.0);
    }

    #[test]
    fn test_pgabor3d_finite() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let period = Vec3::new(4.0, 4.0, 4.0);
        let v = pgabor3d(p, &GaborParams::default(), period);
        assert!(v.is_finite());
    }
}
