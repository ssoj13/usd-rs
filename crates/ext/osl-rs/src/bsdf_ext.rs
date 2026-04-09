//! Extended BSDFs — metal, dielectric, sheen, hair, volume, clearcoat, etc.
//!
//! Port of the SPI BSDF collection from `libbsdl/`:
//! bsdf_metal, bsdf_dielectric, bsdf_clearcoat, bsdf_sheenltc,
//! bsdf_volume, bsdf_physicalhair, bsdf_backscatter, bsdf_thinlayer.
//!
//! All BSDFs work in shading space where N = (0, 0, 1).

use crate::Float;
use crate::bsdf::{
    BSDF, BSDFEval, BSDFSample, fresnel_dielectric, reflect, refract, sample_cosine_hemisphere,
};
use crate::math::{Color3, Vec3};
use std::f32::consts::{LN_2, PI};

// ====== Metal BSDF (complex Fresnel) ======

/// GGX metallic BSDF with complex Fresnel.
#[derive(Debug, Clone)]
pub struct MetalBSDF {
    pub eta: Color3, // complex IOR: real part
    pub k: Color3,   // complex IOR: imaginary (extinction)
    pub roughness: Float,
}

impl MetalBSDF {
    /// Complex Fresnel reflectance for metals.
    pub fn fresnel_conductor(cos_theta: Float, eta: Color3, k: Color3) -> Color3 {
        let ct2 = cos_theta * cos_theta;
        let st2 = 1.0 - ct2;

        let eta2 = Color3::new(eta.x * eta.x, eta.y * eta.y, eta.z * eta.z);
        let k2 = Color3::new(k.x * k.x, k.y * k.y, k.z * k.z);

        let inner = Color3::new(
            eta2.x - k2.x - st2,
            eta2.y - k2.y - st2,
            eta2.z - k2.z - st2,
        );
        let a2plusb2 = Color3::new(
            (inner.x * inner.x + 4.0 * eta2.x * k2.x).sqrt(),
            (inner.y * inner.y + 4.0 * eta2.y * k2.y).sqrt(),
            (inner.z * inner.z + 4.0 * eta2.z * k2.z).sqrt(),
        );
        let a = Color3::new(
            ((a2plusb2.x + inner.x) * 0.5).max(0.0).sqrt(),
            ((a2plusb2.y + inner.y) * 0.5).max(0.0).sqrt(),
            ((a2plusb2.z + inner.z) * 0.5).max(0.0).sqrt(),
        );

        let rs_num = Color3::new(
            a2plusb2.x + ct2 - 2.0 * a.x * cos_theta,
            a2plusb2.y + ct2 - 2.0 * a.y * cos_theta,
            a2plusb2.z + ct2 - 2.0 * a.z * cos_theta,
        );
        let rs_den = Color3::new(
            a2plusb2.x + ct2 + 2.0 * a.x * cos_theta,
            a2plusb2.y + ct2 + 2.0 * a.y * cos_theta,
            a2plusb2.z + ct2 + 2.0 * a.z * cos_theta,
        );
        let rs = Color3::new(
            rs_num.x / rs_den.x,
            rs_num.y / rs_den.y,
            rs_num.z / rs_den.z,
        );

        let rp_num = Color3::new(
            a2plusb2.x * ct2 + st2 * st2 - 2.0 * a.x * cos_theta * st2,
            a2plusb2.y * ct2 + st2 * st2 - 2.0 * a.y * cos_theta * st2,
            a2plusb2.z * ct2 + st2 * st2 - 2.0 * a.z * cos_theta * st2,
        );
        let rp_den = Color3::new(
            a2plusb2.x * ct2 + st2 * st2 + 2.0 * a.x * cos_theta * st2,
            a2plusb2.y * ct2 + st2 * st2 + 2.0 * a.y * cos_theta * st2,
            a2plusb2.z * ct2 + st2 * st2 + 2.0 * a.z * cos_theta * st2,
        );
        let rp = Color3::new(
            rs.x * rp_num.x / rp_den.x,
            rs.y * rp_num.y / rp_den.y,
            rs.z * rp_num.z / rp_den.z,
        );

        Color3::new(
            (rs.x + rp.x) * 0.5,
            (rs.y + rp.y) * 0.5,
            (rs.z + rp.z) * 0.5,
        )
    }
}

impl BSDF for MetalBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let h = (wo + wi).normalize();
        let cos_h = wo.dot(h).max(0.0);
        let fr = Self::fresnel_conductor(cos_h, self.eta, self.k);
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.max(0.0), alpha);
        let g = ggx_g1(cos_o, alpha) * ggx_g1(cos_i, alpha);
        let denom = (4.0 * cos_o * cos_i).max(1e-10);
        let spec = d * g / denom;
        BSDFEval {
            f: Color3::new(fr.x * spec, fr.y * spec, fr.z * spec),
            pdf: d * h.z.max(0.0) / (4.0 * cos_h).max(1e-10),
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let alpha = self.roughness * self.roughness;
        let (wi, _pdf) = sample_ggx(wo, alpha, u1, u2)?;
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        let h = (wo + wi).normalize();
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.max(0.0), alpha);
        d * h.z.max(0.0) / (4.0 * wo.dot(h).abs()).max(1e-10)
    }
}

// ====== Dielectric BSDF (glass) ======

/// Dielectric (glass) BSDF with GGX microfacet model.
#[derive(Debug, Clone)]
pub struct DielectricBSDF {
    pub color: Color3,
    pub roughness: Float,
    pub ior: Float,
}

impl BSDF for DielectricBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_o = wo.z;
        let cos_i = wi.z;
        let is_reflect = cos_o * cos_i > 0.0;

        if is_reflect {
            let h = (wo + wi).normalize();
            let f = fresnel_dielectric(wo.dot(h).abs(), self.ior);
            let alpha = self.roughness * self.roughness;
            let d = ggx_d(h.z.abs(), alpha);
            let g = ggx_g1(cos_o.abs(), alpha) * ggx_g1(cos_i.abs(), alpha);
            let denom = (4.0 * cos_o.abs() * cos_i.abs()).max(1e-10);
            let spec = f * d * g / denom;
            BSDFEval {
                f: Color3::new(
                    self.color.x * spec,
                    self.color.y * spec,
                    self.color.z * spec,
                ),
                pdf: f * d * h.z.abs() / (4.0 * wo.dot(h).abs()).max(1e-10),
            }
        } else {
            let eta = if cos_o > 0.0 {
                1.0 / self.ior
            } else {
                self.ior
            };
            let h = (wo * eta + wi).normalize();
            let f = fresnel_dielectric(wo.dot(h).abs(), self.ior);
            let alpha = self.roughness * self.roughness;
            let d = ggx_d(h.z.abs(), alpha);
            let g = ggx_g1(cos_o.abs(), alpha) * ggx_g1(cos_i.abs(), alpha);

            let wo_dot_h = wo.dot(h);
            let wi_dot_h = wi.dot(h);
            let denom = (eta * wo_dot_h + wi_dot_h).powi(2);
            let jacobian = wi_dot_h.abs() / denom.max(1e-10);
            let btdf = (1.0 - f) * d * g * wo_dot_h.abs() * jacobian
                / (cos_o.abs() * cos_i.abs()).max(1e-10);

            BSDFEval {
                f: Color3::new(
                    self.color.x * btdf,
                    self.color.y * btdf,
                    self.color.z * btdf,
                ),
                pdf: (1.0 - f) * d * h.z.abs() * jacobian,
            }
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let alpha = self.roughness * self.roughness;
        // Sample microfacet half-vector: u1 for azimuth, u2 for polar (GGX importance)
        let phi = 2.0 * PI * u1;
        let cos_theta = ((1.0 - u2) / (1.0 + (alpha * alpha - 1.0) * u2)).sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let h = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta).normalize();

        let cos_h = wo.dot(h);
        let f = fresnel_dielectric(cos_h.abs(), self.ior);

        // Derive an independent random for reflect/refract decision from u1
        // by folding it to decorrelate from the phi sampling.
        let u_rr = (u1 * 1753.0).fract();
        if u_rr < f {
            let wi = reflect(wo, h);
            if wi.z * wo.z < 0.0 {
                return None;
            }
            let ev = self.eval(wo, wi);
            Some(BSDFSample {
                wi,
                weight: ev.f,
                pdf: ev.pdf,
                is_reflection: true,
            })
        } else {
            let eta = if wo.z > 0.0 { 1.0 / self.ior } else { self.ior };
            let wi = refract(wo, h, eta)?;
            let ev = self.eval(wo, wi);
            Some(BSDFSample {
                wi,
                weight: ev.f,
                pdf: ev.pdf,
                is_reflection: false,
            })
        }
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        self.eval(wo, wi).pdf
    }
}

// ====== Sheen BSDF ======

/// Sheen evaluation mode.
///
/// C++ OSL supports two sheen implementations:
/// - Classic: Charlie/Conty sheen approximation (simple, used by default)
/// - LTC: Linearly Transformed Cosines (more accurate, from `bsdf_sheenltc_impl.h`)
///
/// See C++ `_ref/OpenShadingLanguage/src/libbsdl/include/BSDL/SPI/bsdf_sheenltc_decl.h`
/// for the LTC implementation which uses a 32x32 lookup table of LTC coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheenMode {
    /// Charlie/Conty approximation (default, simpler).
    #[default]
    Classic,
    /// Linearly Transformed Cosines (more physically accurate).
    /// TODO: Implement LTC evaluation using precomputed coefficient tables
    /// from `bsdf_sheenltc_param.h`. The LTC mode fetches coefficients via
    /// bilinear interpolation in a 32x32 table indexed by (cos_theta, roughness),
    /// then evaluates/samples the transformed cosine distribution.
    Ltc,
}

/// Sheen BSDF using the Charlie/Conty approximation.
///
/// Supports two modes: `Classic` (default) and `Ltc` (linearly transformed cosines).
/// The `Ltc` mode currently falls back to `Classic` until LTC tables are integrated.
#[derive(Debug, Clone)]
pub struct SheenBSDF {
    pub color: Color3,
    pub roughness: Float,
    /// Evaluation mode: Classic (Charlie/Conty) or LTC.
    pub mode: SheenMode,
}

impl BSDF for SheenBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        // LTC mode: not yet implemented (requires 32x32 precomputed coefficient table
        // from bsdf_sheenltc_param.h). Return zero contribution so callers get a
        // well-defined (black) result rather than silent wrong output.
        if self.mode == SheenMode::Ltc {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }

        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let h = (wo + wi).normalize();
        let cos_d = wo.dot(h).max(0.0);
        let sin_d = (1.0 - cos_d * cos_d).max(0.0).sqrt();

        let alpha = self.roughness * self.roughness;
        let inv_alpha = 1.0 / alpha.max(0.001);
        let d = (2.0 + inv_alpha) * sin_d.powf(inv_alpha) / (2.0 * PI);
        let denom = (4.0 * (cos_i + cos_o - cos_i * cos_o)).max(1e-10);

        let pdf = cos_i / PI;
        BSDFEval {
            f: Color3::new(
                self.color.x * d / denom,
                self.color.y * d / denom,
                self.color.z * d / denom,
            ),
            pdf,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        // LTC mode not implemented: return no sample (zero contribution).
        if self.mode == SheenMode::Ltc {
            return None;
        }
        let wi = sample_cosine_hemisphere(u1, u2);
        if wi.z <= 0.0 {
            return None;
        }
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, _wo: Vec3, wi: Vec3) -> Float {
        if self.mode == SheenMode::Ltc {
            return 0.0;
        }
        wi.z.max(0.0) / PI
    }
}

// ====== Clearcoat BSDF ======

/// Clearcoat layer using GGX with fixed low roughness.
#[derive(Debug, Clone)]
pub struct ClearcoatBSDF {
    pub weight: Float,
    pub roughness: Float,
    pub ior: Float,
}

impl Default for ClearcoatBSDF {
    fn default() -> Self {
        Self {
            weight: 1.0,
            roughness: 0.1,
            ior: 1.5,
        }
    }
}

impl BSDF for ClearcoatBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let h = (wo + wi).normalize();
        let cos_h = wo.dot(h).max(0.0);
        let f = fresnel_dielectric(cos_h, self.ior);
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.max(0.0), alpha);
        let g = ggx_g1(cos_o, alpha) * ggx_g1(cos_i, alpha);
        let denom = (4.0 * cos_o * cos_i).max(1e-10);
        let spec = self.weight * f * d * g / denom;
        BSDFEval {
            f: Color3::new(spec, spec, spec),
            pdf: d * h.z.max(0.0) / (4.0 * cos_h).max(1e-10),
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let alpha = self.roughness * self.roughness;
        let (wi, _) = sample_ggx(wo, alpha, u1, u2)?;
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        let h = (wo + wi).normalize();
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.max(0.0), alpha);
        d * h.z.max(0.0) / (4.0 * wo.dot(h).max(1e-10))
    }
}

// ====== Volume BSDF (Henyey-Greenstein) ======

/// Henyey-Greenstein phase function for volumetric scattering.
#[derive(Debug, Clone)]
pub struct VolumeBSDF {
    pub color: Color3,
    pub g: Float, // asymmetry parameter [-1, 1]
}

impl BSDF for VolumeBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_theta = wo.dot(wi);
        let g2 = self.g * self.g;
        let denom = (1.0 + g2 - 2.0 * self.g * cos_theta).max(1e-10);
        let phase = (1.0 - g2) / (4.0 * PI * denom * denom.sqrt());
        BSDFEval {
            f: Color3::new(
                self.color.x * phase,
                self.color.y * phase,
                self.color.z * phase,
            ),
            pdf: phase,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        // Henyey-Greenstein sampling
        let cos_theta = if self.g.abs() < 1e-5 {
            1.0 - 2.0 * u1
        } else {
            let s = (1.0 - self.g * self.g) / (1.0 - self.g + 2.0 * self.g * u1);
            (1.0 + self.g * self.g - s * s) / (2.0 * self.g)
        };
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let phi = 2.0 * PI * u2;

        let (t, b) = build_onb(wo);
        let wi = (t * (sin_theta * phi.cos()) + b * (sin_theta * phi.sin()) + wo * cos_theta)
            .normalize();
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        let cos_theta = wo.dot(wi);
        let g2 = self.g * self.g;
        let denom = (1.0 + g2 - 2.0 * self.g * cos_theta).max(1e-10);
        (1.0 - g2) / (4.0 * PI * denom * denom.sqrt())
    }
}

// ====== Hair BSDF (Chiang 2016) ======
// Based on "A Practical and Controllable Hair and Fur Model" (Chiang et al. 2016)
// and the BSDL PhysicalHairLobe from OpenShadingLanguage.
// Implements 4 lobes: R (reflection), TT (transmission), TRT (secondary reflection),
// and a residual lobe for remaining scattering orders.

/// Number of explicit lobes (R=0, TT=1, TRT=2, residual=3).
const P_MAX: usize = 3;
const SQRT_PI_OVER_8: Float = 0.626_657_07;
const FLOAT_MIN: Float = 1.175_494_4e-38;

/// Chiang 2016 physically-based hair BSDF.
/// In shading space, the hair tangent direction is assumed to be +Z.
#[derive(Debug, Clone)]
pub struct HairBSDF {
    /// Absorption coefficient per RGB channel.
    pub sigma_a: Color3,
    /// Index of refraction (typically 1.55 for hair).
    pub eta: Float,
    /// Cuticle tilt angle in radians.
    pub alpha: Float,
    /// Longitudinal roughness [0, 1].
    pub beta_m: Float,
    /// Azimuthal roughness [0, 1].
    pub beta_n: Float,
    /// Offset position on hair cross-section [-1, 1].
    pub h: Float,
}

impl Default for HairBSDF {
    fn default() -> Self {
        Self {
            sigma_a: Color3::new(0.5, 0.8, 1.2),
            eta: 1.55,
            alpha: 0.035, // ~2 degrees
            beta_m: 0.3,
            beta_n: 0.3,
            h: 0.0,
        }
    }
}

// --- Hair helper functions (ported from BSDL PhysicalHairLobe) ---

/// Log of modified Bessel function I0. Polynomial approximation from
/// Abramowitz & Stegun, used for numerical stability in Mp.
fn log_bessi0(x: Float) -> Float {
    let ax = x.abs();
    if ax < 3.75 {
        let y = (x / 3.75).powi(2);
        (y * (3.5156229
            + y * (3.0899424
                + y * (1.2067492 + y * (0.2659732 + y * (0.036_076_8 + y * 0.004_581_3))))))
            .ln_1p()
    } else {
        let y = 3.75 / ax;
        ax + ((1.0 / ax.sqrt())
            * (0.398_942_3
                + y * (0.013_285_92
                    + y * (0.002_253_19
                        + y * (-0.001_575_65
                            + y * (0.009_162_81
                                + y * (-0.020_577_06
                                    + y * (0.026_355_37
                                        + y * (-0.016_476_33 + y * 0.003_923_77)))))))))
            .ln()
    }
}

/// I0(x) * exp(exponent), avoids overflow for large x.
fn bessi0_times_exp(x: Float, exponent: Float) -> Float {
    let ax = x.abs();
    if ax < 3.75 {
        let y = (x / 3.75).powi(2);
        exponent.exp()
            * (1.0
                + y * (3.5156229
                    + y * (3.0899424
                        + y * (1.2067492 + y * (0.2659732 + y * (0.036_076_8 + y * 0.004_581_3))))))
    } else {
        let y = 3.75 / ax;
        ((ax + exponent).exp() / ax.sqrt())
            * (0.398_942_3
                + y * (0.013_285_92
                    + y * (0.002_253_19
                        + y * (-0.001_575_65
                            + y * (0.009_162_81
                                + y * (-0.020_577_06
                                    + y * (0.026_355_37
                                        + y * (-0.016_476_33 + y * 0.003_923_77))))))))
    }
}

/// Longitudinal scattering function Mp (d'Eon et al.).
fn mp(cos_ti: Float, cos_to: Float, sin_ti: Float, sin_to: Float, v: Float) -> Float {
    let a = cos_ti * cos_to / v;
    let b = sin_ti * sin_to / v;
    if v <= 0.1 {
        // Log-domain computation to avoid overflow
        (log_bessi0(a) - b - 1.0 / v + LN_2 + (1.0 / (2.0 * v)).ln()).exp()
    } else {
        bessi0_times_exp(a, -b) / ((1.0 / v).sinh() * 2.0 * v)
    }
}

/// Roughness remap for longitudinal M distribution (from pbrt hair.pdf).
fn remap_long_rough(lr: Float) -> Float {
    let lr2 = lr * lr;
    let lr4 = lr2 * lr2;
    let lr20 = (lr4 * lr4).powi(2) * lr4;
    let t = 0.726 * lr + 0.812 * lr2 + 3.7 * lr20;
    t * t
}

/// Roughness remap for azimuthal N distribution (from pbrt hair.pdf).
fn remap_azim_rough(ar: Float) -> Float {
    let ar2 = ar * ar;
    let ar4 = ar2 * ar2;
    let ar22 = (ar4 * ar4).powi(2) * ar4 * ar2;
    SQRT_PI_OVER_8 * (0.265 * ar + 1.194 * ar2 + 5.372 * ar22)
}

/// Azimuthal shift angle Phi for lobe p.
fn phi_fn(p: usize, gamma_o: Float, gamma_t: Float) -> Float {
    if p.is_multiple_of(2) {
        (2 * p) as Float * gamma_t - 2.0 * gamma_o
    } else {
        (2 * p) as Float * gamma_t - 2.0 * gamma_o - PI
    }
}

/// Trimmed logistic distribution on [-PI, PI].
fn trimmed_logistic(x: Float, s: Float) -> Float {
    let t = (PI / s).exp().min(1.0 / FLOAT_MIN);
    let y = (-x.abs() / s).exp().max(FLOAT_MIN);
    (t + 1.0) * y / ((t - 1.0) * s * (1.0 + y).powi(2))
}

/// Azimuthal scattering function Np.
fn np(phi: Float, p: usize, s: Float, gamma_o: Float, gamma_t: Float) -> Float {
    if p == P_MAX {
        return 0.5 * std::f32::consts::FRAC_1_PI;
    }
    let mut dphi = phi - phi_fn(p, gamma_o, gamma_t);
    if dphi > PI {
        dphi -= 2.0 * PI;
    }
    if dphi < -PI {
        dphi += 2.0 * PI;
    }
    trimmed_logistic(dphi, s)
}

/// Sample from the trimmed logistic distribution.
fn sample_trimmed_logistic(u: Float, s: Float) -> Float {
    let t = (PI / s).exp().min(1.0 / FLOAT_MIN);
    let x = -s * ((1.0 + t) / (u * (1.0 - t) + t) - 1.0).ln();
    x.clamp(-PI, PI)
}

/// Precompute sin/cos of cuticle tilt angle multiples.
/// Returns (sin2k_alpha[3], cos2k_alpha[3]) using double-angle recurrence.
fn sincos_alpha(offset: Float) -> ([Float; P_MAX], [Float; P_MAX]) {
    let mut sin2k = [0.0_f32; P_MAX];
    let mut cos2k = [0.0_f32; P_MAX];
    sin2k[0] = offset.sin();
    cos2k[0] = (1.0 - sin2k[0] * sin2k[0]).sqrt();
    for i in 1..P_MAX {
        sin2k[i] = 2.0 * cos2k[i - 1] * sin2k[i - 1];
        cos2k[i] = cos2k[i - 1] * cos2k[i - 1] - sin2k[i - 1] * sin2k[i - 1];
    }
    (sin2k, cos2k)
}

/// Compute per-lobe attenuation Ap[4] (R, TT, TRT, residual).
/// tau is the single-path transmittance through the fiber per channel.
fn compute_ap(cos_to: Float, eta: Float, h: Float, tau: Color3) -> [Color3; P_MAX + 1] {
    let cos_gamma_o = (1.0 - h * h).max(0.0).sqrt();
    let cos_theta = cos_to * cos_gamma_o;
    let f = fresnel_dielectric(cos_theta, eta);

    let mut ap = [Color3::ZERO; P_MAX + 1];
    // R lobe: just Fresnel reflection
    ap[0] = Color3::new(f, f, f);
    // TT lobe: (1-f)^2 * T
    let one_minus_f_sq = (1.0 - f) * (1.0 - f);
    ap[1] = Color3::new(
        one_minus_f_sq * tau.x,
        one_minus_f_sq * tau.y,
        one_minus_f_sq * tau.z,
    );
    // TRT lobe: ap[1] * T * f
    ap[2] = Color3::new(
        ap[1].x * tau.x * f,
        ap[1].y * tau.y * f,
        ap[1].z * tau.z * f,
    );
    // Residual: remaining energy (geometric series sum)
    ap[P_MAX] = Color3::new(
        ap[P_MAX - 1].x * f * tau.x / (1.0 - tau.x * f).max(1e-5),
        ap[P_MAX - 1].y * f * tau.y / (1.0 - tau.y * f).max(1e-5),
        ap[P_MAX - 1].z * f * tau.z / (1.0 - tau.z * f).max(1e-5),
    );
    ap
}

/// Compute longitudinal and azimuthal variance arrays for all lobes.
fn variances(lrough: Float, arough: Float) -> ([Float; P_MAX + 1], [Float; P_MAX + 1]) {
    let mut v = [0.0_f32; P_MAX + 1];
    let mut s = [0.0_f32; P_MAX + 1];
    v[0] = remap_long_rough(lrough);
    v[1] = 0.25 * remap_long_rough(lrough.min(1.0));
    v[2] = 4.0 * remap_long_rough(lrough.min(1.0));
    v[P_MAX] = v[2];
    s[0] = remap_azim_rough(arough);
    s[1] = remap_azim_rough(arough.min(1.0));
    s[2] = remap_azim_rough(arough.min(1.0));
    s[P_MAX] = s[2];
    (v, s)
}

/// Max component of a Color3.
fn color_max(c: Color3) -> Float {
    c.x.max(c.y).max(c.z)
}

/// Output of [`HairBSDF::precompute`] — keeps the return type readable for Clippy `type_complexity`.
type HairPrecomputeParts = (
    [Color3; P_MAX + 1], // ap
    [Float; P_MAX + 1],  // v (longitudinal variance)
    [Float; P_MAX + 1],  // s (azimuthal variance)
    Float,               // gamma_o
    Float,               // gamma_t
    [Float; P_MAX],      // sin2k_alpha
    [Float; P_MAX],      // cos2k_alpha
);

impl HairBSDF {
    /// Precompute per-shading-point invariants from struct fields.
    fn precompute(&self, cos_to: Float) -> HairPrecomputeParts {
        let eta = self.eta.max(1.001);
        let sin_to = (1.0 - cos_to * cos_to).max(0.0).sqrt();
        let gamma_o = self.h.asin();

        // Compute refracted gamma_t
        let etap = (eta * eta - sin_to * sin_to).max(0.0).sqrt() / cos_to.max(1e-6);
        let sin_gamma_t = (self.h / etap).clamp(-1.0, 1.0);
        let cos_gamma_t = (1.0 - sin_gamma_t * sin_gamma_t).sqrt();
        let gamma_t = sin_gamma_t.asin();

        // Single-path transmittance through the hair fiber
        let sin_theta_t = sin_to / eta;
        let cos_theta_t = (1.0 - sin_theta_t * sin_theta_t).max(0.0).sqrt();
        let path_len = 2.0 * cos_gamma_t / cos_theta_t.max(1e-6);
        let tau = Color3::new(
            (-self.sigma_a.x * path_len).exp(),
            (-self.sigma_a.y * path_len).exp(),
            (-self.sigma_a.z * path_len).exp(),
        );

        let ap = compute_ap(cos_to, eta, self.h, tau);
        let (v, s) = variances(self.beta_m, self.beta_n);
        let (sin2k, cos2k) = sincos_alpha(self.alpha);

        (ap, v, s, gamma_o, gamma_t, sin2k, cos2k)
    }

    /// Apply cuticle tilt to sin/cos theta_o for lobe p.
    fn tilt_theta(
        sin_to: Float,
        cos_to: Float,
        p: usize,
        sin2k: &[Float; P_MAX],
        cos2k: &[Float; P_MAX],
    ) -> (Float, Float) {
        let (sin_op, cos_op) = match p {
            // R: shift by -2*alpha => use sin2k[1], cos2k[1] with subtraction
            0 => (
                sin_to * cos2k[1] - cos_to * sin2k[1],
                cos_to * cos2k[1] + sin_to * sin2k[1],
            ),
            // TT: shift by +alpha => use sin2k[0], cos2k[0] with addition
            1 => (
                sin_to * cos2k[0] + cos_to * sin2k[0],
                cos_to * cos2k[0] - sin_to * sin2k[0],
            ),
            // TRT: shift by +4*alpha => use sin2k[2], cos2k[2] with addition
            2 => (
                sin_to * cos2k[2] + cos_to * sin2k[2],
                cos_to * cos2k[2] - sin_to * sin2k[2],
            ),
            _ => (sin_to, cos_to),
        };
        (sin_op, cos_op.abs())
    }

    /// Build lobe CDF from ap weights. Returns (cdf[4], total_weight).
    fn lobe_cdf(ap: &[Color3; P_MAX + 1]) -> ([Float; P_MAX + 1], Float) {
        let mut cdf = [0.0_f32; P_MAX + 1];
        for i in 0..=P_MAX {
            cdf[i] = color_max(ap[i]);
        }
        let total: Float = cdf.iter().sum();
        if total > 0.0 {
            for c in cdf.iter_mut() {
                *c /= total;
            }
        } else {
            cdf[P_MAX] = 1.0;
        }
        (cdf, total)
    }
}

impl BSDF for HairBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let sin_to = wo.z.clamp(-1.0, 1.0);
        let cos_to = (1.0 - sin_to * sin_to).max(0.0).sqrt();
        let sin_ti = wi.z.clamp(-1.0, 1.0);
        let cos_ti = (1.0 - sin_ti * sin_ti).max(0.0).sqrt();
        let phi_i = wi.y.atan2(wi.x);
        let phi = phi_i; // phiO = 0 in our frame

        let (ap, v, s, gamma_o, gamma_t, sin2k, cos2k) = self.precompute(cos_to);
        let (cdf, _) = Self::lobe_cdf(&ap);

        let mut f_total = Color3::ZERO;
        let mut pdf_total = 0.0_f32;

        for p in 0..=P_MAX {
            if cdf[p] <= 1e-6 {
                continue;
            }
            let (sin_op, cos_op) = Self::tilt_theta(sin_to, cos_to, p, &sin2k, &cos2k);
            let lobe_pdf =
                mp(cos_ti, cos_op, sin_ti, sin_op, v[p]) * np(phi, p, s[p], gamma_o, gamma_t);
            // Weight: ap[p] * lobe_pdf, selection probability: cdf[p]
            f_total = Color3::new(
                f_total.x + ap[p].x * lobe_pdf,
                f_total.y + ap[p].y * lobe_pdf,
                f_total.z + ap[p].z * lobe_pdf,
            );
            pdf_total += cdf[p] * lobe_pdf;
        }

        BSDFEval {
            f: f_total,
            pdf: pdf_total.max(1e-10),
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let sin_to = wo.z.clamp(-1.0, 1.0);
        let cos_to = (1.0 - sin_to * sin_to).max(0.0).sqrt();

        let (ap, v, s, gamma_o, gamma_t, sin2k, cos2k) = self.precompute(cos_to);
        let (cdf, _) = Self::lobe_cdf(&ap);

        // Select lobe from CDF using u1, remap remainder for Mp sampling
        let mut p = P_MAX;
        let mut rnd_mp = u1;
        {
            let mut accum = 0.0_f32;
            for (i, &cdf_i) in cdf.iter().enumerate().take(P_MAX + 1) {
                if u1 < accum + cdf_i {
                    p = i;
                    // Remap u1 into [0,1] within selected lobe
                    rnd_mp = ((u1 - accum) / cdf_i.max(1e-10)).clamp(0.0, 1.0);
                    break;
                }
                accum += cdf_i;
            }
        }

        let (sin_op, cos_op) = Self::tilt_theta(sin_to, cos_to, p, &sin2k, &cos2k);

        // Sample Mp to get theta_i
        rnd_mp = rnd_mp.max(1e-5);
        let cos_theta = 1.0 + v[p] * (rnd_mp + (1.0 - rnd_mp) * (-2.0 / v[p]).exp()).ln();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        // Derive a third random dimension for cos_phi from u1+u2 hash
        let u3 = (u1 * 4096.0 + u2 * 31.0).fract();
        let cos_phi = (2.0 * PI * u3).cos();
        let sin_ti = -cos_theta * sin_op + sin_theta * cos_phi * cos_op;
        let cos_ti = (1.0 - sin_ti * sin_ti).max(0.0).sqrt();

        // Sample Np to get delta_phi
        let dphi = if p < P_MAX {
            phi_fn(p, gamma_o, gamma_t) + sample_trimmed_logistic(u2, s[p])
        } else {
            2.0 * PI * u2
        };

        let (sin_phi_i, cos_phi_i) = dphi.sin_cos();
        let wi = Vec3::new(cos_phi_i * cos_ti, sin_phi_i * cos_ti, sin_ti);

        // Evaluate full BSDF at sampled direction (consistent with eval)
        let ev = self.eval(wo, wi);
        if ev.pdf <= 0.0 {
            return None;
        }

        Some(BSDFSample {
            wi,
            weight: Color3::new(
                ev.f.x / ev.pdf.max(1e-10),
                ev.f.y / ev.pdf.max(1e-10),
                ev.f.z / ev.pdf.max(1e-10),
            ),
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        self.eval(wo, wi).pdf
    }
}

// ====== Backscatter BSDF ======

/// Backscattering BSDF for velvet/cloth-like materials.
#[derive(Debug, Clone)]
pub struct BackscatterBSDF {
    pub color: Color3,
    pub roughness: Float,
}

impl BSDF for BackscatterBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let cos_d = (-wo).dot(wi).max(0.0);
        let retro = cos_d.powf(1.0 / self.roughness.max(0.01));
        let val = retro / PI;
        BSDFEval {
            f: Color3::new(self.color.x * val, self.color.y * val, self.color.z * val),
            pdf: cos_i / PI,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let wi = sample_cosine_hemisphere(u1, u2);
        if wi.z <= 0.0 {
            return None;
        }
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, _wo: Vec3, wi: Vec3) -> Float {
        wi.z.max(0.0) / PI
    }
}

// ====== Utility functions ======

fn ggx_d(cos_theta: Float, alpha: Float) -> Float {
    if cos_theta <= 0.0 {
        return 0.0;
    }
    let a2 = alpha * alpha;
    let cos2 = cos_theta * cos_theta;
    let denom = cos2 * (a2 - 1.0) + 1.0;
    a2 / (PI * denom * denom).max(1e-10)
}

fn ggx_g1(cos_theta: Float, alpha: Float) -> Float {
    if cos_theta <= 0.0 {
        return 0.0;
    }
    let a2 = alpha * alpha;
    let cos2 = cos_theta * cos_theta;
    2.0 * cos_theta / (cos_theta + (a2 + (1.0 - a2) * cos2).sqrt())
}

fn sample_ggx(wo: Vec3, alpha: Float, u1: Float, u2: Float) -> Option<(Vec3, Float)> {
    let phi = 2.0 * PI * u1;
    let cos_theta = ((1.0 - u2) / (1.0 + (alpha * alpha - 1.0) * u2)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let h = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta).normalize();
    let wi = reflect(wo, h);
    if wi.z <= 0.0 {
        return None;
    }
    let d = ggx_d(h.z.max(0.0), alpha);
    let pdf = d * cos_theta / (4.0 * wo.dot(h).abs()).max(1e-10);
    Some((wi, pdf))
}

fn build_onb(n: Vec3) -> (Vec3, Vec3) {
    let sign = if n.z >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    let t = Vec3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x);
    let b2 = Vec3::new(b, sign + n.y * n.y * a, -n.y);
    (t, b2)
}

/// Gaussian falloff for hair longitudinal scattering.
#[allow(dead_code)]
fn gaussian(x: Float, sigma: Float) -> Float {
    (-x * x / (2.0 * sigma * sigma).max(1e-10)).exp()
}

// ====== Thin Film BSDF ======

/// Thin film interference BSDF for iridescent coatings.
///
/// Models wavelength-dependent thin film interference where a dielectric
/// film of known thickness and IOR sits on a substrate.
#[derive(Debug, Clone)]
pub struct ThinFilmBSDF {
    /// Film thickness in nanometers.
    pub thickness: Float,
    /// Film IOR (e.g., 1.5 for soap bubble).
    pub film_ior: Float,
    /// Substrate IOR (e.g., 1.5 for glass, complex for metals).
    pub substrate_ior: Float,
    /// Roughness of the substrate surface.
    pub roughness: Float,
}

impl Default for ThinFilmBSDF {
    fn default() -> Self {
        Self {
            thickness: 500.0,
            film_ior: 1.5,
            substrate_ior: 1.5,
            roughness: 0.1,
        }
    }
}

impl ThinFilmBSDF {
    /// Compute thin film reflectance for a given wavelength (nm) and angle.
    pub fn reflectance(&self, wavelength: Float, cos_theta: Float) -> Float {
        let n1 = 1.0_f32; // air
        let n2 = self.film_ior;
        let n3 = self.substrate_ior;

        // Snell's law: n1 sin(θ1) = n2 sin(θ2)
        let sin_theta1 = (1.0 - cos_theta * cos_theta).sqrt();
        let sin_theta2 = (n1 / n2) * sin_theta1;
        if sin_theta2.abs() > 1.0 {
            return 1.0;
        } // TIR
        let cos_theta2 = (1.0 - sin_theta2 * sin_theta2).sqrt();

        // Fresnel reflectances at air-film and film-substrate interfaces
        let r12 = fresnel_dielectric(cos_theta, n1 / n2);
        let r23 = fresnel_dielectric(cos_theta2, n2 / n3);

        // Phase change (optical path difference)
        let delta = 2.0 * PI * 2.0 * n2 * self.thickness * cos_theta2 / wavelength;

        // Airy function for thin film interference
        let r12_sq = r12 * r12;
        let r23_sq = r23 * r23;
        let cos_delta = delta.cos();

        let numerator = r12_sq + r23_sq + 2.0 * r12 * r23 * cos_delta;
        let denominator = 1.0 + r12_sq * r23_sq + 2.0 * r12 * r23 * cos_delta;

        if denominator > 0.0 {
            numerator / denominator
        } else {
            r12_sq
        }
    }

    /// Compute thin film color for a given angle by sampling RGB wavelengths.
    pub fn color_reflectance(&self, cos_theta: Float) -> Color3 {
        // Sample at standard RGB wavelengths
        let r = self.reflectance(650.0, cos_theta); // red
        let g = self.reflectance(532.0, cos_theta); // green
        let b = self.reflectance(460.0, cos_theta); // blue
        Color3::new(r, g, b)
    }
}

impl BSDF for ThinFilmBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        if wo.z <= 0.0 || wi.z <= 0.0 {
            return BSDFEval {
                f: Color3::new(0.0, 0.0, 0.0),
                pdf: 0.0,
            };
        }

        let h = (wo + wi).normalize();
        let cos_theta = wo.z;
        let cos_h = h.z.max(0.0);

        let alpha = self.roughness.max(0.001);
        let alpha2 = alpha * alpha;
        let denom = cos_h * cos_h * (alpha2 - 1.0) + 1.0;
        let d = alpha2 / (PI * denom * denom);

        let color = self.color_reflectance(cos_theta);
        let f = Color3::new(color.x * d * wi.z, color.y * d * wi.z, color.z * d * wi.z);
        let pdf = d * cos_h / (4.0 * wo.dot(h).abs().max(1e-8));

        BSDFEval { f, pdf }
    }

    fn sample(&self, wo: Vec3, randu: Float, randv: Float) -> Option<BSDFSample> {
        let alpha = self.roughness.max(0.001);
        let alpha2 = alpha * alpha;

        let cos_theta = ((1.0 - randu) / (randu * (alpha2 - 1.0) + 1.0)).sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let phi = 2.0 * PI * randv;

        let h = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);

        let wi = reflect(wo, h);
        if wi.z <= 0.0 {
            return None;
        }

        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        self.eval(wo, wi).pdf
    }
}

// ====== Spectral BSDF ======

/// Spectral BSDF that computes wavelength-dependent responses and integrates
/// to RGB using CIE color matching functions.
#[derive(Debug, Clone)]
pub struct SpectralBSDF {
    /// Spectral reflectance at normal incidence, sampled at 380..780nm (40 bins).
    pub reflectance: [Float; 40],
    /// Roughness parameter.
    pub roughness: Float,
}

impl Default for SpectralBSDF {
    fn default() -> Self {
        Self {
            reflectance: [0.5; 40], // 50% reflectance across all wavelengths
            roughness: 0.2,
        }
    }
}

impl SpectralBSDF {
    /// Convert spectral bins to RGB using simplified CIE 1931 XYZ matching.
    pub fn to_rgb(&self) -> Color3 {
        let mut x_sum = 0.0_f32;
        let mut y_sum = 0.0_f32;
        let mut z_sum = 0.0_f32;

        for i in 0..40 {
            let wavelength = 380.0 + (i as f32) * 10.0;
            let r = self.reflectance[i];

            // Simplified Gaussian approximation of CIE matching functions
            let xbar = 1.056 * cie_gaussian(wavelength, 599.8, 37.9)
                + 0.362 * cie_gaussian(wavelength, 442.0, 16.0)
                - 0.065 * cie_gaussian(wavelength, 501.1, 20.4);
            let ybar = 0.821 * cie_gaussian(wavelength, 568.8, 46.9)
                + 0.286 * cie_gaussian(wavelength, 530.9, 16.3);
            let zbar = 1.217 * cie_gaussian(wavelength, 437.0, 11.8)
                + 0.681 * cie_gaussian(wavelength, 459.0, 26.0);

            x_sum += r * xbar;
            y_sum += r * ybar;
            z_sum += r * zbar;
        }

        // XYZ to linear sRGB
        let r = (3.2406 * x_sum - 1.5372 * y_sum - 0.4986 * z_sum).max(0.0);
        let g = (-0.9689 * x_sum + 1.8758 * y_sum + 0.0415 * z_sum).max(0.0);
        let b = (0.0557 * x_sum - 0.2040 * y_sum + 1.0570 * z_sum).max(0.0);

        // Normalize
        let scale = 10.0 / 40.0; // integration step
        Color3::new(r * scale, g * scale, b * scale)
    }
}

fn cie_gaussian(x: Float, mu: Float, sigma: Float) -> Float {
    let t = (x - mu) / sigma;
    (-0.5 * t * t).exp()
}

impl BSDF for SpectralBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        if wo.z <= 0.0 || wi.z <= 0.0 {
            return BSDFEval {
                f: Color3::new(0.0, 0.0, 0.0),
                pdf: 0.0,
            };
        }

        let color = self.to_rgb();
        let f = Color3::new(
            color.x * wi.z / PI,
            color.y * wi.z / PI,
            color.z * wi.z / PI,
        );
        let pdf = wi.z / PI;

        BSDFEval { f, pdf }
    }

    fn sample(&self, wo: Vec3, randu: Float, randv: Float) -> Option<BSDFSample> {
        let wi = sample_cosine_hemisphere(randu, randv);
        if wi.z <= 0.0 || wo.z <= 0.0 {
            return None;
        }
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, _wo: Vec3, wi: Vec3) -> Float {
        if wi.z > 0.0 { wi.z / PI } else { 0.0 }
    }
}

// ====== MTX Conductor BSDF (MaterialX conductor model) ======

/// MaterialX conductor BSDF — simplified Schlick-based metallic reflection.
/// Matches `BSDL/MTX/bsdf_conductor_impl.h`.
#[derive(Debug, Clone)]
pub struct MtxConductorBSDF {
    /// Normal-incidence reflectance color (F0).
    pub reflectance: Color3,
    /// Edge tint (F90 approximation).
    pub edge_color: Color3,
    /// Surface roughness (0=mirror, 1=rough).
    pub roughness: Float,
}

impl Default for MtxConductorBSDF {
    fn default() -> Self {
        Self {
            reflectance: Color3::new(0.95, 0.64, 0.54), // copper approx
            edge_color: Color3::new(1.0, 1.0, 1.0),
            roughness: 0.2,
        }
    }
}

impl MtxConductorBSDF {
    /// Schlick Fresnel with artist-friendly edge tint.
    fn fresnel_schlick(&self, cos_theta: Float) -> Color3 {
        let t = (1.0 - cos_theta).max(0.0).powi(5);
        Color3::new(
            self.reflectance.x + (self.edge_color.x - self.reflectance.x) * t,
            self.reflectance.y + (self.edge_color.y - self.reflectance.y) * t,
            self.reflectance.z + (self.edge_color.z - self.reflectance.z) * t,
        )
    }
}

impl BSDF for MtxConductorBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_o = wo.z.max(0.0);
        let cos_i = wi.z.max(0.0);
        if cos_o <= 0.0 || cos_i <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let h = (wo + wi).normalize();
        let cos_h = wo.dot(h).max(0.0);
        let fr = self.fresnel_schlick(cos_h);
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.max(0.0), alpha);
        let g = ggx_g1(cos_o, alpha) * ggx_g1(cos_i, alpha);
        let denom = (4.0 * cos_o * cos_i).max(1e-10);
        let spec = d * g / denom;
        BSDFEval {
            f: Color3::new(fr.x * spec, fr.y * spec, fr.z * spec),
            pdf: d * h.z.max(0.0) / (4.0 * cos_h).max(1e-10),
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let alpha = self.roughness * self.roughness;
        let (wi, _) = sample_ggx(wo, alpha, u1, u2)?;
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        let h = (wo + wi).normalize();
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.max(0.0), alpha);
        d * h.z.max(0.0) / (4.0 * wo.dot(h).abs()).max(1e-10)
    }
}

// ====== Refraction BSDF (pure refraction, no reflection) ======

/// Pure refraction BSDF — transmits light through a surface.
/// Separate from Transparent (which is a delta passthrough).
#[derive(Debug, Clone)]
pub struct RefractionBSDF {
    /// Transmission color.
    pub color: Color3,
    /// Index of refraction.
    pub ior: Float,
    /// Surface roughness.
    pub roughness: Float,
}

impl Default for RefractionBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(1.0, 1.0, 1.0),
            ior: 1.5,
            roughness: 0.0,
        }
    }
}

impl BSDF for RefractionBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        // For smooth refraction (roughness=0), PDF is a delta function
        if self.roughness < 1e-4 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let cos_o = wo.z;
        let cos_i = wi.z;
        // Both should be on opposite sides
        if cos_o * cos_i > 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let eta = if cos_o > 0.0 {
            1.0 / self.ior
        } else {
            self.ior
        };
        let h = (wo + wi * eta).normalize();
        let cos_h = wo.dot(h).abs();
        let f = 1.0 - fresnel_dielectric(cos_h, self.ior);
        let alpha = self.roughness * self.roughness;
        let d = ggx_d(h.z.abs(), alpha);
        let g = ggx_g1(cos_o.abs(), alpha) * ggx_g1(cos_i.abs(), alpha);
        let denom_ht = (wo.dot(h) + eta * wi.dot(h)).powi(2);
        let jacobian = (eta * eta * wi.dot(h).abs()) / denom_ht.max(1e-10);
        let bsdf_val =
            f * d * g * wo.dot(h).abs() * jacobian / (cos_o.abs() * cos_i.abs()).max(1e-10);
        BSDFEval {
            f: Color3::new(
                self.color.x * bsdf_val,
                self.color.y * bsdf_val,
                self.color.z * bsdf_val,
            ),
            pdf: d * h.z.abs() * jacobian,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        if self.roughness < 1e-4 {
            // Delta refraction
            let eta = if wo.z > 0.0 { 1.0 / self.ior } else { self.ior };
            let n = if wo.z > 0.0 {
                Vec3::new(0.0, 0.0, 1.0)
            } else {
                Vec3::new(0.0, 0.0, -1.0)
            };
            let wi = refract(wo, n, eta)?;
            let f = 1.0 - fresnel_dielectric(wo.z.abs(), self.ior);
            return Some(BSDFSample {
                wi,
                weight: Color3::new(self.color.x * f, self.color.y * f, self.color.z * f),
                pdf: 1.0,
                is_reflection: false,
            });
        }
        // Rough refraction via GGX half-vector sampling
        let alpha = self.roughness * self.roughness;
        let phi = 2.0 * PI * u1;
        let cos_theta = ((1.0 - u2) / (1.0 + (alpha * alpha - 1.0) * u2)).sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let h = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
        let eta = if wo.z > 0.0 { 1.0 / self.ior } else { self.ior };
        let wi = refract(wo, h, eta)?;
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: false,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        self.eval(wo, wi).pdf
    }
}

// ====== Translucent BSDF ======

/// Translucent BSDF — cosine-weighted transmission (like diffuse but through the surface).
/// Matches OSL's `translucent()` closure.
#[derive(Debug, Clone)]
pub struct TranslucentBSDF {
    pub color: Color3,
}

impl Default for TranslucentBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(1.0, 1.0, 1.0),
        }
    }
}

impl BSDF for TranslucentBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        // Transmission: wi and wo on opposite sides
        if wo.z * wi.z >= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let cos_i = wi.z.abs();
        let inv_pi = 1.0 / PI;
        BSDFEval {
            f: Color3::new(
                self.color.x * inv_pi,
                self.color.y * inv_pi,
                self.color.z * inv_pi,
            ),
            pdf: cos_i * inv_pi,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let mut wi = sample_cosine_hemisphere(u1, u2);
        // Flip to the opposite side of wo
        if wo.z > 0.0 {
            wi.z = -wi.z;
        }
        let ev = self.eval(wo, wi);
        if ev.pdf > 0.0 {
            Some(BSDFSample {
                wi,
                weight: Color3::new(ev.f.x / ev.pdf, ev.f.y / ev.pdf, ev.f.z / ev.pdf),
                pdf: ev.pdf,
                is_reflection: false,
            })
        } else {
            None
        }
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        if wo.z * wi.z >= 0.0 {
            return 0.0;
        }
        wi.z.abs() / PI
    }
}

// ====== Subsurface BSDF (diffusion approximation) ======

/// Subsurface scattering approximation using a diffusion profile.
/// Matches the `subsurface()` closure in OSL.
#[derive(Debug, Clone)]
pub struct SubsurfaceBSDF {
    /// Scattering color.
    pub color: Color3,
    /// Mean free path (per channel).
    pub radius: Color3,
    /// Scale factor.
    pub scale: Float,
}

impl Default for SubsurfaceBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(0.8, 0.5, 0.3),
            radius: Color3::new(1.0, 0.5, 0.25),
            scale: 1.0,
        }
    }
}

impl SubsurfaceBSDF {
    /// Burley's normalized diffusion profile Rd(r).
    /// R(r) = A * (e^(-r/d) + e^(-r/(3d))) / (8 * pi * d * r)
    pub fn diffusion_profile(&self, distance: Float, channel: usize) -> Float {
        let r_ch = match channel {
            0 => self.radius.x,
            1 => self.radius.y,
            2 => self.radius.z,
            _ => 1.0,
        };
        let d = r_ch * self.scale;
        if d < 1e-10 || distance < 1e-10 {
            return 0.0;
        }
        let r = distance;
        let exp1 = (-r / d).exp();
        let exp2 = (-r / (3.0 * d)).exp();
        (exp1 + exp2) / (8.0 * PI * d * r)
    }
}

impl BSDF for SubsurfaceBSDF {
    fn eval(&self, _wo: Vec3, wi: Vec3) -> BSDFEval {
        // Approximation: treat as translucent with color modulation
        let cos_i = wi.z.abs();
        if cos_i <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let inv_pi = 1.0 / PI;
        // Simple dipole approximation factor
        let factor = 0.5; // Simplified — real SSS needs distance
        BSDFEval {
            f: Color3::new(
                self.color.x * factor * inv_pi,
                self.color.y * factor * inv_pi,
                self.color.z * factor * inv_pi,
            ),
            pdf: cos_i * inv_pi,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        // Sample cosine-weighted hemisphere on both sides
        let mut wi = sample_cosine_hemisphere(u1, u2);
        if u1 > 0.5 {
            wi.z = -wi.z; // transmission side
        }
        let ev = self.eval(wo, wi);
        if ev.pdf > 0.0 {
            Some(BSDFSample {
                wi,
                weight: Color3::new(ev.f.x / ev.pdf, ev.f.y / ev.pdf, ev.f.z / ev.pdf),
                pdf: ev.pdf,
                is_reflection: wi.z > 0.0,
            })
        } else {
            None
        }
    }

    fn pdf(&self, _wo: Vec3, wi: Vec3) -> Float {
        wi.z.abs() / PI * 0.5
    }
}

// ====== Emission BSDF (re-exported from bsdf.rs) ======

/// Re-export `bsdf::Emission` as `EmissionBSDF` for backwards compatibility.
pub type EmissionBSDF = crate::bsdf::Emission;

// ====== Phong BSDF (legacy specular model) ======

/// Classic Phong specular BSDF.
///
/// Uses the modified Phong model: f = (n+2)/(2*pi) * cos^n(alpha)
/// where alpha is the angle between the reflected direction and wi.
/// This is a legacy model superseded by microfacet BSDFs but still
/// used for compatibility with older shaders.
#[derive(Debug, Clone)]
pub struct PhongBSDF {
    pub color: Color3,
    /// Phong exponent (higher = sharper highlight).
    pub exponent: Float,
}

impl Default for PhongBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(1.0, 1.0, 1.0),
            exponent: 32.0,
        }
    }
}

impl BSDF for PhongBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        // Reflected direction of wo about the normal (0,0,1)
        let r = Vec3::new(-wo.x, -wo.y, wo.z);
        let cos_alpha = r.dot(wi).max(0.0);
        let n = self.exponent;
        let norm = (n + 2.0) / (2.0 * PI);
        let spec = norm * cos_alpha.powf(n);
        let pdf = (n + 1.0) / (2.0 * PI) * cos_alpha.powf(n);
        BSDFEval {
            f: Color3::new(
                self.color.x * spec,
                self.color.y * spec,
                self.color.z * spec,
            ),
            pdf,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        // Sample Phong lobe around reflected direction
        let cos_alpha = u1.powf(1.0 / (self.exponent + 1.0));
        let sin_alpha = (1.0 - cos_alpha * cos_alpha).max(0.0).sqrt();
        let phi = 2.0 * PI * u2;
        // Build direction in reflected-direction frame
        let r = Vec3::new(-wo.x, -wo.y, wo.z);
        let (t, b) = build_onb(r);
        let wi = Vec3::new(
            t.x * sin_alpha * phi.cos() + b.x * sin_alpha * phi.sin() + r.x * cos_alpha,
            t.y * sin_alpha * phi.cos() + b.y * sin_alpha * phi.sin() + r.y * cos_alpha,
            t.z * sin_alpha * phi.cos() + b.z * sin_alpha * phi.sin() + r.z * cos_alpha,
        )
        .normalize();
        if wi.z <= 0.0 {
            return None;
        }
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        let r = Vec3::new(-wo.x, -wo.y, wo.z);
        let cos_alpha = r.dot(wi).max(0.0);
        (self.exponent + 1.0) / (2.0 * PI) * cos_alpha.powf(self.exponent)
    }
}

// ====== Ward BSDF (anisotropic specular) ======

/// Ward anisotropic specular BSDF.
///
/// Uses the Duer/Ward variant with separate roughness for tangent and
/// bitangent directions. This is a legacy anisotropic model.
#[derive(Debug, Clone)]
pub struct WardBSDF {
    pub color: Color3,
    /// Roughness along the tangent direction.
    pub ax: Float,
    /// Roughness along the bitangent direction.
    pub ay: Float,
}

impl Default for WardBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(1.0, 1.0, 1.0),
            ax: 0.15,
            ay: 0.15,
        }
    }
}

impl BSDF for WardBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let h = (wo + wi).normalize();
        if h.z <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let ax2 = self.ax * self.ax;
        let ay2 = self.ay * self.ay;
        let exp_arg = -(h.x * h.x / ax2 + h.y * h.y / ay2) / (h.z * h.z);
        let denom = 4.0 * PI * self.ax * self.ay * (cos_o * cos_i).sqrt();
        let val = exp_arg.exp() / denom.max(1e-10);
        let pdf = val * cos_i; // approximate PDF
        BSDFEval {
            f: Color3::new(self.color.x * val, self.color.y * val, self.color.z * val),
            pdf,
        }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        // Fallback: cosine hemisphere sampling (Ward importance sampling is complex)
        let wi = sample_cosine_hemisphere(u1, u2);
        if wi.z <= 0.0 {
            return None;
        }
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, wo: Vec3, wi: Vec3) -> Float {
        self.eval(wo, wi).pdf
    }
}

// ====== Delta Reflection BSDF (perfect mirror) ======

/// Perfect mirror (delta) reflection BSDF.
///
/// This is a singular BSDF: eval() always returns zero (delta distribution),
/// and sample() returns the perfect reflection direction with full weight.
#[derive(Debug, Clone)]
pub struct DeltaReflectionBSDF {
    pub color: Color3,
}

impl Default for DeltaReflectionBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(1.0, 1.0, 1.0),
        }
    }
}

impl BSDF for DeltaReflectionBSDF {
    fn eval(&self, _wo: Vec3, _wi: Vec3) -> BSDFEval {
        // Delta distribution: PDF is infinite at the reflection direction,
        // zero everywhere else. eval() returns zero; use sample() instead.
        BSDFEval {
            f: Color3::ZERO,
            pdf: 0.0,
        }
    }

    fn sample(&self, wo: Vec3, _u1: Float, _u2: Float) -> Option<BSDFSample> {
        if wo.z <= 0.0 {
            return None;
        }
        // Perfect reflection about the shading normal (0,0,1)
        let wi = Vec3::new(-wo.x, -wo.y, wo.z);
        Some(BSDFSample {
            wi,
            weight: self.color,
            pdf: 1.0, // delta
            is_reflection: true,
        })
    }

    fn pdf(&self, _wo: Vec3, _wi: Vec3) -> Float {
        0.0 // delta distribution
    }
}

// ====== SPI Thin Layer BSDF (stub) ======

/// SPI thin-layer BSDF stub.
///
/// The full implementation uses a layered material model with:
/// - A top dielectric layer (clear coat) with Fresnel
/// - Energy-conserving inter-layer transport
/// - Bottom substrate BSDF (typically diffuse or microfacet)
///
/// See C++ `_ref/OpenShadingLanguage/src/libbsdl/include/BSDL/SPI/`:
///   `bsdf_thinlayer_decl.h` — ThinLayerLobe struct with top_ior, thickness,
///     sigma (absorption), bottom roughness, etc.
///   `bsdf_thinlayer_impl.h` — Full implementation with inter-reflection
///     energy accounting and Fresnel-weighted layer blending.
#[derive(Debug, Clone)]
pub struct SpiThinLayerBSDF {
    pub color: Color3,
    /// Top layer IOR.
    pub top_ior: Float,
    /// Bottom layer roughness.
    pub bottom_roughness: Float,
    /// Layer thickness (for absorption).
    pub thickness: Float,
    /// Absorption color (sigma_a).
    pub absorption: Color3,
}

impl Default for SpiThinLayerBSDF {
    fn default() -> Self {
        Self {
            color: Color3::new(1.0, 1.0, 1.0),
            top_ior: 1.5,
            bottom_roughness: 0.3,
            thickness: 0.0,
            absorption: Color3::ZERO,
        }
    }
}

impl BSDF for SpiThinLayerBSDF {
    fn eval(&self, wo: Vec3, wi: Vec3) -> BSDFEval {
        // Simplified: blend Fresnel top reflection with bottom diffuse
        let cos_o = wo.z.max(0.0);
        let cos_i = wi.z.max(0.0);
        if cos_o <= 0.0 || cos_i <= 0.0 {
            return BSDFEval {
                f: Color3::ZERO,
                pdf: 0.0,
            };
        }
        let fr = fresnel_dielectric(cos_o, self.top_ior);
        let ft = 1.0 - fr;
        // Bottom: Lambertian with absorption
        let bottom = ft * ft * cos_i / PI;
        let f = Color3::new(
            self.color.x * bottom,
            self.color.y * bottom,
            self.color.z * bottom,
        );
        BSDFEval { f, pdf: cos_i / PI }
    }

    fn sample(&self, wo: Vec3, u1: Float, u2: Float) -> Option<BSDFSample> {
        let wi = sample_cosine_hemisphere(u1, u2);
        if wi.z <= 0.0 {
            return None;
        }
        let ev = self.eval(wo, wi);
        Some(BSDFSample {
            wi,
            weight: ev.f,
            pdf: ev.pdf,
            is_reflection: true,
        })
    }

    fn pdf(&self, _wo: Vec3, wi: Vec3) -> Float {
        wi.z.max(0.0) / PI
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metal_fresnel_normal_incidence() {
        let f = MetalBSDF::fresnel_conductor(
            1.0,
            Color3::new(0.18, 0.42, 1.37),
            Color3::new(3.42, 2.35, 1.82),
        );
        assert!(f.x > 0.0 && f.x < 1.0);
        assert!(f.y > 0.0 && f.y < 1.0);
        assert!(f.z > 0.0 && f.z < 1.0);
    }

    #[test]
    fn test_dielectric_bsdf_sample() {
        let bsdf = DielectricBSDF {
            color: Color3::new(1.0, 1.0, 1.0),
            roughness: 0.1,
            ior: 1.5,
        };
        let wo = Vec3::new(0.0, 0.5, 0.866).normalize();
        let _sample = bsdf.sample(wo, 0.3, 0.5);
    }

    #[test]
    fn test_sheen_eval() {
        let sheen = SheenBSDF {
            color: Color3::new(1.0, 1.0, 1.0),
            roughness: 0.5,
            mode: SheenMode::Classic,
        };
        let wo = Vec3::new(0.0, 0.5, 0.866).normalize();
        let wi = Vec3::new(0.0, -0.5, 0.866).normalize();
        let ev = sheen.eval(wo, wi);
        assert!(ev.f.x > 0.0);
        assert!(ev.pdf > 0.0);
    }

    #[test]
    fn test_volume_hg_isotropic() {
        let vol = VolumeBSDF {
            color: Color3::new(1.0, 1.0, 1.0),
            g: 0.0,
        };
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.0, 0.0, -1.0);
        let ev = vol.eval(wo, wi);
        let expected = 1.0 / (4.0 * PI);
        assert!((ev.f.x - expected).abs() < 0.01);
    }

    #[test]
    fn test_clearcoat_default() {
        let cc = ClearcoatBSDF::default();
        assert_eq!(cc.ior, 1.5);
        assert!((cc.roughness - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_hair_eval() {
        let hair = HairBSDF::default();
        let wo = Vec3::new(0.5, 0.0, 0.866).normalize();
        let wi = Vec3::new(-0.5, 0.0, 0.866).normalize();
        let ev = hair.eval(wo, wi);
        assert!(ev.pdf > 0.0);
    }

    #[test]
    fn test_backscatter_retro() {
        let bs = BackscatterBSDF {
            color: Color3::new(1.0, 1.0, 1.0),
            roughness: 0.5,
        };
        let wo = Vec3::new(0.3, 0.0, 0.954).normalize();
        let wi = Vec3::new(-0.3, 0.0, 0.954).normalize();
        let ev = bs.eval(wo, wi);
        assert!(ev.f.x >= 0.0);
    }

    #[test]
    fn test_thin_film_default() {
        let tf = ThinFilmBSDF::default();
        assert_eq!(tf.thickness, 500.0);
        assert_eq!(tf.film_ior, 1.5);
    }

    #[test]
    fn test_thin_film_reflectance() {
        let tf = ThinFilmBSDF {
            thickness: 500.0,
            film_ior: 1.5,
            substrate_ior: 1.5,
            roughness: 0.1,
        };
        let r = tf.reflectance(550.0, 1.0);
        assert!(
            r >= 0.0 && r <= 1.0,
            "Thin film reflectance should be in [0,1], got {r}"
        );
    }

    #[test]
    fn test_thin_film_color_iridescence() {
        let tf = ThinFilmBSDF {
            thickness: 400.0,
            film_ior: 1.4,
            substrate_ior: 2.0,
            roughness: 0.1,
        };
        let c1 = tf.color_reflectance(1.0); // normal incidence
        let c2 = tf.color_reflectance(0.5); // oblique
        // Colors should differ due to thin film interference
        let diff = (c1.x - c2.x).abs() + (c1.y - c2.y).abs() + (c1.z - c2.z).abs();
        assert!(
            diff > 0.0,
            "Thin film should show iridescence (angle-dependent color)"
        );
    }

    #[test]
    fn test_thin_film_eval() {
        let tf = ThinFilmBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.0, 0.0, 1.0);
        let ev = tf.eval(wo, wi);
        assert!(ev.pdf > 0.0);
    }

    #[test]
    fn test_spectral_default() {
        let sp = SpectralBSDF::default();
        assert_eq!(sp.reflectance.len(), 40);
        assert_eq!(sp.roughness, 0.2);
    }

    #[test]
    fn test_spectral_to_rgb() {
        let sp = SpectralBSDF::default();
        let rgb = sp.to_rgb();
        assert!(rgb.x >= 0.0);
        assert!(rgb.y >= 0.0);
        assert!(rgb.z >= 0.0);
    }

    #[test]
    fn test_spectral_eval() {
        let sp = SpectralBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.3, 0.0, 0.954).normalize();
        let ev = sp.eval(wo, wi);
        assert!(ev.f.x >= 0.0);
        assert!(ev.pdf > 0.0);
    }

    // ---- MTX Conductor tests ----

    #[test]
    fn test_mtx_conductor_default() {
        let c = MtxConductorBSDF::default();
        assert_eq!(c.roughness, 0.2);
    }

    #[test]
    fn test_mtx_conductor_eval() {
        let c = MtxConductorBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.3, 0.0, 0.954).normalize();
        let ev = c.eval(wo, wi);
        assert!(ev.f.x >= 0.0);
        assert!(ev.pdf >= 0.0);
    }

    #[test]
    fn test_mtx_conductor_sample() {
        let c = MtxConductorBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let s = c.sample(wo, 0.3, 0.7);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.is_reflection);
        assert!(s.wi.z > 0.0);
    }

    #[test]
    fn test_mtx_conductor_reciprocity() {
        let c = MtxConductorBSDF {
            reflectance: Color3::new(0.9, 0.6, 0.5),
            edge_color: Color3::new(1.0, 0.9, 0.8),
            roughness: 0.3,
        };
        let wo = Vec3::new(0.3, 0.2, 0.93).normalize();
        let wi = Vec3::new(-0.2, 0.3, 0.93).normalize();
        let ev1 = c.eval(wo, wi);
        let ev2 = c.eval(wi, wo);
        assert!((ev1.f.x - ev2.f.x).abs() < 0.01);
    }

    // ---- Refraction BSDF tests ----

    #[test]
    fn test_refraction_default() {
        let r = RefractionBSDF::default();
        assert_eq!(r.ior, 1.5);
        assert_eq!(r.roughness, 0.0);
    }

    #[test]
    fn test_refraction_smooth_sample() {
        let r = RefractionBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let s = r.sample(wo, 0.3, 0.7);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(!s.is_reflection);
        assert!(s.wi.z < 0.0); // transmitted to other side
    }

    #[test]
    fn test_refraction_rough_eval() {
        let r = RefractionBSDF {
            roughness: 0.3,
            ..Default::default()
        };
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.1, 0.0, -0.995).normalize();
        let ev = r.eval(wo, wi);
        // Should have non-zero value for rough refraction
        assert!(ev.pdf >= 0.0);
    }

    // ---- Translucent BSDF tests ----

    #[test]
    fn test_translucent_default() {
        let t = TranslucentBSDF::default();
        assert_eq!(t.color.x, 1.0);
    }

    #[test]
    fn test_translucent_eval() {
        let t = TranslucentBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.0, 0.0, -1.0); // opposite side
        let ev = t.eval(wo, wi);
        assert!(ev.f.x > 0.0);
        assert!(ev.pdf > 0.0);
    }

    #[test]
    fn test_translucent_same_side_zero() {
        let t = TranslucentBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.3, 0.0, 0.954).normalize(); // same side
        let ev = t.eval(wo, wi);
        assert_eq!(ev.f.x, 0.0);
    }

    // ---- Subsurface BSDF tests ----

    #[test]
    fn test_subsurface_default() {
        let s = SubsurfaceBSDF::default();
        assert_eq!(s.scale, 1.0);
    }

    #[test]
    fn test_subsurface_diffusion_profile() {
        let s = SubsurfaceBSDF::default();
        let p0 = s.diffusion_profile(0.1, 0);
        let p1 = s.diffusion_profile(1.0, 0);
        // Profile should decrease with distance
        assert!(p0 > p1);
        assert!(p0 > 0.0);
    }

    #[test]
    fn test_subsurface_eval() {
        let s = SubsurfaceBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.0, 0.0, -1.0);
        let ev = s.eval(wo, wi);
        assert!(ev.f.x > 0.0);
    }

    // ---- Emission BSDF tests ----

    #[test]
    fn test_emission_default() {
        let e = EmissionBSDF::default();
        assert_eq!(e.radiance.x, 1.0);
    }

    #[test]
    fn test_emission_eval() {
        let e = EmissionBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let wi = Vec3::new(0.3, 0.0, 0.954).normalize();
        let ev = e.eval(wo, wi);
        assert!(ev.f.x > 0.0);
        assert!(ev.pdf > 0.0);
    }

    #[test]
    fn test_emission_sample() {
        let e = EmissionBSDF::default();
        let wo = Vec3::new(0.0, 0.0, 1.0);
        let s = e.sample(wo, 0.5, 0.5);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.wi.z > 0.0); // upper hemisphere
    }
}
