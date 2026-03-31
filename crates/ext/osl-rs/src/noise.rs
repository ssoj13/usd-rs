//! Noise functions matching OSL's `liboslnoise`.
//!
//! Implements: Perlin, Cell, Simplex, and Hash noise in 1D–4D,
//! with both signed and unsigned variants, periodic versions,
//! and derivative (Dual) support.

use crate::Float;
use crate::math::Vec3;

// ---------------------------------------------------------------------------
// Hash functions (for lattice-based noise)
// ---------------------------------------------------------------------------

// Bob Jenkins lookup3 hash primitives (matching C++ OSL oslnoise.h)

/// Jenkins lookup3 bjmix: mix three u32 values.
#[inline]
fn bjmix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(4);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(6);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(8);
    *b = b.wrapping_add(*a);
    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(16);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(19);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(4);
    *b = b.wrapping_add(*a);
}

/// Jenkins lookup3 bjfinal: final avalanche mix.
#[inline]
fn bjfinal(a_in: u32, b_in: u32, c_in: u32) -> u32 {
    let (mut a, mut b, mut c) = (a_in, b_in, c_in);
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    c
}

/// Hash one integer (Bob Jenkins lookup3, matching C++ OSL inthash).
#[inline]
fn hash_u32(k0: u32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(1 << 2).wrapping_add(13);
    let a = start.wrapping_add(k0);
    bjfinal(a, start, start)
}

/// Hash two integers to a u32.
#[inline]
fn hash2(x: i32, y: i32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(2 << 2).wrapping_add(13);
    let a = start.wrapping_add(x as u32);
    let b = start.wrapping_add(y as u32);
    bjfinal(a, b, start)
}

/// Hash three integers to a u32.
#[inline]
fn hash3(x: i32, y: i32, z: i32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(3 << 2).wrapping_add(13);
    let a = start.wrapping_add(x as u32);
    let b = start.wrapping_add(y as u32);
    let c = start.wrapping_add(z as u32);
    bjfinal(a, b, c)
}

/// Hash four integers to a u32.
#[inline]
fn hash4(x: i32, y: i32, z: i32, w: i32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(4 << 2).wrapping_add(13);
    let mut a = start.wrapping_add(x as u32);
    let mut b = start.wrapping_add(y as u32);
    let mut c = start.wrapping_add(z as u32);
    bjmix(&mut a, &mut b, &mut c);
    a = a.wrapping_add(w as u32);
    bjfinal(a, b, c)
}

/// Convert hash to float in [0, 1).
#[inline]
fn hash_to_float01(h: u32) -> Float {
    (h >> 8) as Float * (1.0 / 16777216.0) // divide by 2^24
}

/// Convert hash to float in [-1, 1).
#[inline]
#[allow(dead_code)]
fn hash_to_float_signed(h: u32) -> Float {
    hash_to_float01(h) * 2.0 - 1.0
}

// ---------------------------------------------------------------------------
// Perlin gradient vectors
// ---------------------------------------------------------------------------

/// Gradient for 1D Perlin noise.
#[inline]
fn grad1(hash: u32, x: Float) -> Float {
    if hash & 1 == 0 { x } else { -x }
}

/// Gradient for 2D Perlin noise.
#[inline]
fn grad2(hash: u32, x: Float, y: Float) -> Float {
    match hash & 3 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        _ => -x - y,
    }
}

/// Gradient for 3D Perlin noise.
#[inline]
fn grad3(hash: u32, x: Float, y: Float, z: Float) -> Float {
    match hash & 15 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        11 => -y - z,
        12 => y + x,
        13 => -y + z,
        14 => y - x,
        _ => -y - z,
    }
}

/// Gradient for 4D Perlin noise (edges of hypercube, matching C++ OSL).
#[inline]
fn grad4(hash: u32, x: Float, y: Float, z: Float, w: Float) -> Float {
    let h = (hash & 31) as i32;
    let u = if h < 24 { x } else { y };
    let v = if h < 16 { y } else { z };
    let s = if h < 8 { z } else { w };
    let mut r = u;
    if h & 1 != 0 {
        r = -r;
    }
    let mut vn = v;
    if h & 2 != 0 {
        vn = -vn;
    }
    let mut sn = s;
    if h & 4 != 0 {
        sn = -sn;
    }
    r + vn + sn
}

/// Quintic smoothstep (Perlin's improved interpolant).
#[inline]
fn fade(t: Float) -> Float {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Derivative of the quintic smoothstep.
#[inline]
fn fade_deriv(t: Float) -> Float {
    30.0 * t * t * (t * (t - 2.0) + 1.0)
}

/// Linear interpolation.
#[inline]
fn lerp(a: Float, b: Float, t: Float) -> Float {
    a + t * (b - a)
}

// ---------------------------------------------------------------------------
// Cell noise
// ---------------------------------------------------------------------------

/// 1D cell noise — random value per integer cell, range [0, 1).
pub fn cellnoise1(x: Float) -> Float {
    hash_to_float01(hash_u32(x.floor() as i32 as u32))
}

/// 2D cell noise.
pub fn cellnoise2(x: Float, y: Float) -> Float {
    hash_to_float01(hash2(x.floor() as i32, y.floor() as i32))
}

/// 3D cell noise.
pub fn cellnoise3(p: Vec3) -> Float {
    hash_to_float01(hash3(
        p.x.floor() as i32,
        p.y.floor() as i32,
        p.z.floor() as i32,
    ))
}

/// 3D cell noise returning a Vec3 (different hash per component).
/// Uses `inthash_vec3` matching C++ `CellNoise::hashVec(x,y,z)` which calls
/// `inthashVec` with bjmix + 3x bjfinal(a+0/1/2, b, c).
pub fn cellnoise3_v(p: Vec3) -> Vec3 {
    let v = crate::hashes::inthash_vec3(
        p.x.floor() as i32 as u32,
        p.y.floor() as i32 as u32,
        p.z.floor() as i32 as u32,
    );
    Vec3::new(v[0], v[1], v[2])
}

// ---------------------------------------------------------------------------
// Hash noise (no interpolation, just hashed coordinates)
// ---------------------------------------------------------------------------

/// 1D hash noise — random value, no interpolation.
pub fn hashnoise1(x: Float) -> Float {
    hash_to_float01(hash_u32(x.to_bits()))
}

/// 2D hash noise.
pub fn hashnoise2(x: Float, y: Float) -> Float {
    hash_to_float01(hash2(x.to_bits() as i32, y.to_bits() as i32))
}

/// 3D hash noise.
pub fn hashnoise3(p: Vec3) -> Float {
    hash_to_float01(hash3(
        p.x.to_bits() as i32,
        p.y.to_bits() as i32,
        p.z.to_bits() as i32,
    ))
}

// ---------------------------------------------------------------------------
// Perlin noise
// ---------------------------------------------------------------------------

/// 1D Perlin noise, range [-1, 1].
pub fn perlin1(x: Float) -> Float {
    let xi = x.floor() as i32;
    let xf = x - x.floor();

    let u = fade(xf);

    let a = grad1(hash_u32(xi as u32), xf);
    let b = grad1(hash_u32((xi + 1) as u32), xf - 1.0);

    lerp(a, b, u)
}

/// 2D Perlin noise, range approximately [-1, 1].
pub fn perlin2(x: Float, y: Float) -> Float {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();

    let u = fade(xf);
    let v = fade(yf);

    let aa = hash2(xi, yi);
    let ab = hash2(xi, yi + 1);
    let ba = hash2(xi + 1, yi);
    let bb = hash2(xi + 1, yi + 1);

    let x1 = lerp(grad2(aa, xf, yf), grad2(ba, xf - 1.0, yf), u);
    let x2 = lerp(grad2(ab, xf, yf - 1.0), grad2(bb, xf - 1.0, yf - 1.0), u);

    lerp(x1, x2, v)
}

/// 3D Perlin noise, range approximately [-1, 1].
pub fn perlin3(p: Vec3) -> Float {
    let xi = p.x.floor() as i32;
    let yi = p.y.floor() as i32;
    let zi = p.z.floor() as i32;
    let xf = p.x - p.x.floor();
    let yf = p.y - p.y.floor();
    let zf = p.z - p.z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let aaa = hash3(xi, yi, zi);
    let aab = hash3(xi, yi, zi + 1);
    let aba = hash3(xi, yi + 1, zi);
    let abb = hash3(xi, yi + 1, zi + 1);
    let baa = hash3(xi + 1, yi, zi);
    let bab = hash3(xi + 1, yi, zi + 1);
    let bba = hash3(xi + 1, yi + 1, zi);
    let bbb = hash3(xi + 1, yi + 1, zi + 1);

    let x1 = lerp(grad3(aaa, xf, yf, zf), grad3(baa, xf - 1.0, yf, zf), u);
    let x2 = lerp(
        grad3(aba, xf, yf - 1.0, zf),
        grad3(bba, xf - 1.0, yf - 1.0, zf),
        u,
    );
    let y1 = lerp(x1, x2, v);

    let x3 = lerp(
        grad3(aab, xf, yf, zf - 1.0),
        grad3(bab, xf - 1.0, yf, zf - 1.0),
        u,
    );
    let x4 = lerp(
        grad3(abb, xf, yf - 1.0, zf - 1.0),
        grad3(bbb, xf - 1.0, yf - 1.0, zf - 1.0),
        u,
    );
    let y2 = lerp(x3, x4, v);

    lerp(y1, y2, w)
}

/// Return the gradient vector for a 3D hash (not the dot product).
#[inline]
fn grad3_vec(hash: u32) -> Vec3 {
    match hash & 15 {
        0 => Vec3::new(1.0, 1.0, 0.0),
        1 => Vec3::new(-1.0, 1.0, 0.0),
        2 => Vec3::new(1.0, -1.0, 0.0),
        3 => Vec3::new(-1.0, -1.0, 0.0),
        4 => Vec3::new(1.0, 0.0, 1.0),
        5 => Vec3::new(-1.0, 0.0, 1.0),
        6 => Vec3::new(1.0, 0.0, -1.0),
        7 => Vec3::new(-1.0, 0.0, -1.0),
        8 => Vec3::new(0.0, 1.0, 1.0),
        9 => Vec3::new(0.0, -1.0, 1.0),
        10 => Vec3::new(0.0, 1.0, -1.0),
        11 => Vec3::new(0.0, -1.0, -1.0),
        12 => Vec3::new(1.0, 1.0, 0.0),
        13 => Vec3::new(0.0, -1.0, 1.0),
        14 => Vec3::new(-1.0, 1.0, 0.0),
        _ => Vec3::new(0.0, -1.0, -1.0),
    }
}

/// 3D Perlin noise with analytical derivatives.
/// Returns (value, gradient).
pub fn perlin3_deriv(p: Vec3) -> (Float, Vec3) {
    let xi = p.x.floor() as i32;
    let yi = p.y.floor() as i32;
    let zi = p.z.floor() as i32;
    let xf = p.x - p.x.floor();
    let yf = p.y - p.y.floor();
    let zf = p.z - p.z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);
    let du = fade_deriv(xf);
    let dv = fade_deriv(yf);
    let dw = fade_deriv(zf);

    // Hashes for 8 corners
    let h000 = hash3(xi, yi, zi);
    let h100 = hash3(xi + 1, yi, zi);
    let h010 = hash3(xi, yi + 1, zi);
    let h110 = hash3(xi + 1, yi + 1, zi);
    let h001 = hash3(xi, yi, zi + 1);
    let h101 = hash3(xi + 1, yi, zi + 1);
    let h011 = hash3(xi, yi + 1, zi + 1);
    let h111 = hash3(xi + 1, yi + 1, zi + 1);

    // Gradient vectors at each corner
    let ga = grad3_vec(h000);
    let gb = grad3_vec(h100);
    let gc = grad3_vec(h010);
    let gd = grad3_vec(h110);
    let ge = grad3_vec(h001);
    let gf = grad3_vec(h101);
    let gg = grad3_vec(h011);
    let gh = grad3_vec(h111);

    // Dot products (noise values at corners)
    let va = ga.x * xf + ga.y * yf + ga.z * zf;
    let vb = gb.x * (xf - 1.0) + gb.y * yf + gb.z * zf;
    let vc = gc.x * xf + gc.y * (yf - 1.0) + gc.z * zf;
    let vd = gd.x * (xf - 1.0) + gd.y * (yf - 1.0) + gd.z * zf;
    let ve = ge.x * xf + ge.y * yf + ge.z * (zf - 1.0);
    let vf = gf.x * (xf - 1.0) + gf.y * yf + gf.z * (zf - 1.0);
    let vg = gg.x * xf + gg.y * (yf - 1.0) + gg.z * (zf - 1.0);
    let vh = gh.x * (xf - 1.0) + gh.y * (yf - 1.0) + gh.z * (zf - 1.0);

    // Interpolation coefficients
    let k0 = va;
    let k1 = vb - va;
    let k2 = vc - va;
    let k3 = ve - va;
    let k4 = va - vb - vc + vd;
    let k5 = va - vc - ve + vg;
    let k6 = va - vb - ve + vf;
    let k7 = -va + vb + vc - vd + ve - vf - vg + vh;

    let val = k0 + k1 * u + k2 * v + k3 * w + k4 * u * v + k5 * v * w + k6 * u * w + k7 * u * v * w;

    // Derivative: gradient interpolation + fade derivative terms
    // Part 1: trilinear interpolation of gradient vectors
    let interp_g = Vec3::new(
        // x-component of gradient, trilinearly interpolated
        ga.x + u * (gb.x - ga.x)
            + v * (gc.x - ga.x)
            + w * (ge.x - ga.x)
            + u * v * (ga.x - gb.x - gc.x + gd.x)
            + v * w * (ga.x - gc.x - ge.x + gg.x)
            + u * w * (ga.x - gb.x - ge.x + gf.x)
            + u * v * w * (-ga.x + gb.x + gc.x - gd.x + ge.x - gf.x - gg.x + gh.x),
        // y-component
        ga.y + u * (gb.y - ga.y)
            + v * (gc.y - ga.y)
            + w * (ge.y - ga.y)
            + u * v * (ga.y - gb.y - gc.y + gd.y)
            + v * w * (ga.y - gc.y - ge.y + gg.y)
            + u * w * (ga.y - gb.y - ge.y + gf.y)
            + u * v * w * (-ga.y + gb.y + gc.y - gd.y + ge.y - gf.y - gg.y + gh.y),
        // z-component
        ga.z + u * (gb.z - ga.z)
            + v * (gc.z - ga.z)
            + w * (ge.z - ga.z)
            + u * v * (ga.z - gb.z - gc.z + gd.z)
            + v * w * (ga.z - gc.z - ge.z + gg.z)
            + u * w * (ga.z - gb.z - ge.z + gf.z)
            + u * v * w * (-ga.z + gb.z + gc.z - gd.z + ge.z - gf.z - gg.z + gh.z),
    );

    // Part 2: fade derivative contribution
    let fade_deriv_contrib = Vec3::new(
        du * (k1 + k4 * v + k6 * w + k7 * v * w),
        dv * (k2 + k4 * u + k5 * w + k7 * u * w),
        dw * (k3 + k5 * v + k6 * u + k7 * u * v),
    );

    let deriv = interp_g + fade_deriv_contrib;

    (val, deriv)
}

/// 4D Perlin noise with proper quadrilinear interpolation (matching C++ OSL).
pub fn perlin4(x: Float, y: Float, z: Float, w: Float) -> Float {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let wi = w.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();
    let fw = w - w.floor();

    let u = fade(fx);
    let v = fade(fy);
    let t = fade(fz);
    let s = fade(fw);

    // 16 corners of the 4D hypercube, w=0 slice (8 corners)
    let g0000 = grad4(hash4(xi, yi, zi, wi), fx, fy, fz, fw);
    let g1000 = grad4(hash4(xi + 1, yi, zi, wi), fx - 1.0, fy, fz, fw);
    let g0100 = grad4(hash4(xi, yi + 1, zi, wi), fx, fy - 1.0, fz, fw);
    let g1100 = grad4(hash4(xi + 1, yi + 1, zi, wi), fx - 1.0, fy - 1.0, fz, fw);
    let g0010 = grad4(hash4(xi, yi, zi + 1, wi), fx, fy, fz - 1.0, fw);
    let g1010 = grad4(hash4(xi + 1, yi, zi + 1, wi), fx - 1.0, fy, fz - 1.0, fw);
    let g0110 = grad4(hash4(xi, yi + 1, zi + 1, wi), fx, fy - 1.0, fz - 1.0, fw);
    let g1110 = grad4(
        hash4(xi + 1, yi + 1, zi + 1, wi),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
        fw,
    );
    // w=1 slice (8 corners)
    let g0001 = grad4(hash4(xi, yi, zi, wi + 1), fx, fy, fz, fw - 1.0);
    let g1001 = grad4(hash4(xi + 1, yi, zi, wi + 1), fx - 1.0, fy, fz, fw - 1.0);
    let g0101 = grad4(hash4(xi, yi + 1, zi, wi + 1), fx, fy - 1.0, fz, fw - 1.0);
    let g1101 = grad4(
        hash4(xi + 1, yi + 1, zi, wi + 1),
        fx - 1.0,
        fy - 1.0,
        fz,
        fw - 1.0,
    );
    let g0011 = grad4(hash4(xi, yi, zi + 1, wi + 1), fx, fy, fz - 1.0, fw - 1.0);
    let g1011 = grad4(
        hash4(xi + 1, yi, zi + 1, wi + 1),
        fx - 1.0,
        fy,
        fz - 1.0,
        fw - 1.0,
    );
    let g0111 = grad4(
        hash4(xi, yi + 1, zi + 1, wi + 1),
        fx,
        fy - 1.0,
        fz - 1.0,
        fw - 1.0,
    );
    let g1111 = grad4(
        hash4(xi + 1, yi + 1, zi + 1, wi + 1),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
        fw - 1.0,
    );

    // Trilinear interpolation within w=0 slice
    let x1 = lerp(g0000, g1000, u);
    let x2 = lerp(g0100, g1100, u);
    let y1 = lerp(x1, x2, v);
    let x3 = lerp(g0010, g1010, u);
    let x4 = lerp(g0110, g1110, u);
    let y2 = lerp(x3, x4, v);
    let z0 = lerp(y1, y2, t);

    // Trilinear interpolation within w=1 slice
    let x5 = lerp(g0001, g1001, u);
    let x6 = lerp(g0101, g1101, u);
    let y3 = lerp(x5, x6, v);
    let x7 = lerp(g0011, g1011, u);
    let x8 = lerp(g0111, g1111, u);
    let y4 = lerp(x7, x8, v);
    let z1 = lerp(y3, y4, t);

    // Final lerp across w dimension, scaled by 4D normalization factor
    0.8344 * lerp(z0, z1, s)
}

/// 4D Perlin noise with analytical derivatives.
/// Returns (value, dx, dy, dz, dw).
pub fn perlin4_deriv(
    x: Float,
    y: Float,
    z: Float,
    w: Float,
) -> (Float, Float, Float, Float, Float) {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let wi = w.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();
    let fw = w - w.floor();

    let u = fade(fx);
    let v = fade(fy);
    let t = fade(fz);
    let s = fade(fw);
    let du = fade_deriv(fx);
    let dv = fade_deriv(fy);
    let dt = fade_deriv(fz);
    let ds = fade_deriv(fw);

    // 16 corners
    let g0000 = grad4(hash4(xi, yi, zi, wi), fx, fy, fz, fw);
    let g1000 = grad4(hash4(xi + 1, yi, zi, wi), fx - 1.0, fy, fz, fw);
    let g0100 = grad4(hash4(xi, yi + 1, zi, wi), fx, fy - 1.0, fz, fw);
    let g1100 = grad4(hash4(xi + 1, yi + 1, zi, wi), fx - 1.0, fy - 1.0, fz, fw);
    let g0010 = grad4(hash4(xi, yi, zi + 1, wi), fx, fy, fz - 1.0, fw);
    let g1010 = grad4(hash4(xi + 1, yi, zi + 1, wi), fx - 1.0, fy, fz - 1.0, fw);
    let g0110 = grad4(hash4(xi, yi + 1, zi + 1, wi), fx, fy - 1.0, fz - 1.0, fw);
    let g1110 = grad4(
        hash4(xi + 1, yi + 1, zi + 1, wi),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
        fw,
    );
    let g0001 = grad4(hash4(xi, yi, zi, wi + 1), fx, fy, fz, fw - 1.0);
    let g1001 = grad4(hash4(xi + 1, yi, zi, wi + 1), fx - 1.0, fy, fz, fw - 1.0);
    let g0101 = grad4(hash4(xi, yi + 1, zi, wi + 1), fx, fy - 1.0, fz, fw - 1.0);
    let g1101 = grad4(
        hash4(xi + 1, yi + 1, zi, wi + 1),
        fx - 1.0,
        fy - 1.0,
        fz,
        fw - 1.0,
    );
    let g0011 = grad4(hash4(xi, yi, zi + 1, wi + 1), fx, fy, fz - 1.0, fw - 1.0);
    let g1011 = grad4(
        hash4(xi + 1, yi, zi + 1, wi + 1),
        fx - 1.0,
        fy,
        fz - 1.0,
        fw - 1.0,
    );
    let g0111 = grad4(
        hash4(xi, yi + 1, zi + 1, wi + 1),
        fx,
        fy - 1.0,
        fz - 1.0,
        fw - 1.0,
    );
    let g1111 = grad4(
        hash4(xi + 1, yi + 1, zi + 1, wi + 1),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
        fw - 1.0,
    );

    // Quadrilinear interpolation
    let x1 = lerp(g0000, g1000, u);
    let x2 = lerp(g0100, g1100, u);
    let y1 = lerp(x1, x2, v);
    let x3 = lerp(g0010, g1010, u);
    let x4 = lerp(g0110, g1110, u);
    let y2 = lerp(x3, x4, v);
    let z0 = lerp(y1, y2, t);

    let x5 = lerp(g0001, g1001, u);
    let x6 = lerp(g0101, g1101, u);
    let y3 = lerp(x5, x6, v);
    let x7 = lerp(g0011, g1011, u);
    let x8 = lerp(g0111, g1111, u);
    let y4 = lerp(x7, x8, v);
    let z1 = lerp(y3, y4, t);

    let val = 0.8344 * lerp(z0, z1, s);

    // Derivatives via chain rule on fade interpolants
    // k coefficients for the 4D polynomial
    let k1 = g1000 - g0000;
    let k2 = g0100 - g0000;
    let k3 = g0010 - g0000;
    let k4 = g0001 - g0000;
    let k5 = g0000 - g1000 - g0100 + g1100;
    let k6 = g0000 - g1000 - g0010 + g1010;
    let k7 = g0000 - g0100 - g0010 + g0110;
    let k8 = g0000 - g1000 - g0001 + g1001;
    let k9 = g0000 - g0100 - g0001 + g0101;
    let k10 = g0000 - g0010 - g0001 + g0011;
    let k11 = -g0000 + g1000 + g0100 - g1100 + g0010 - g1010 - g0110 + g1110;
    let k12 = -g0000 + g1000 + g0100 - g1100 + g0001 - g1001 - g0101 + g1101;
    let k13 = -g0000 + g1000 + g0010 - g1010 + g0001 - g1001 - g0011 + g1011;
    let k14 = -g0000 + g0100 + g0010 - g0110 + g0001 - g0101 - g0011 + g0111;
    let k15 = g0000 - g1000 - g0100 + g1100 - g0010 + g1010 + g0110 - g1110 - g0001 + g1001 + g0101
        - g1101
        + g0011
        - g1011
        - g0111
        + g1111;

    let ddx = du
        * (k1
            + k5 * v
            + k6 * t
            + k8 * s
            + k11 * v * t
            + k12 * v * s
            + k13 * t * s
            + k15 * v * t * s)
        * 0.8344;
    let ddy = dv
        * (k2
            + k5 * u
            + k7 * t
            + k9 * s
            + k11 * u * t
            + k12 * u * s
            + k14 * t * s
            + k15 * u * t * s)
        * 0.8344;
    let ddz = dt
        * (k3
            + k6 * u
            + k7 * v
            + k10 * s
            + k11 * u * v
            + k13 * u * s
            + k14 * v * s
            + k15 * u * v * s)
        * 0.8344;
    let ddw = ds
        * (k4
            + k8 * u
            + k9 * v
            + k10 * t
            + k12 * u * v
            + k13 * u * t
            + k14 * v * t
            + k15 * u * v * t)
        * 0.8344;

    (val, ddx, ddy, ddz, ddw)
}

/// 3D Perlin noise returning a Vec3 (vector-valued noise).
pub fn vperlin3(p: Vec3) -> Vec3 {
    Vec3::new(
        perlin3(p),
        perlin3(Vec3::new(p.x + 31.416, p.y + 47.853, p.z + 12.793)),
        perlin3(Vec3::new(p.x + 71.337, p.y + 23.519, p.z + 59.167)),
    )
}

/// 1D Perlin noise returning a Vec3.
pub fn vperlin1(x: Float) -> Vec3 {
    Vec3::new(perlin1(x), perlin1(x + 31.416), perlin1(x + 71.337))
}

/// 1D Perlin noise with analytical derivative.
pub fn perlin1_deriv(x: Float) -> (Float, Float) {
    let xi = x.floor() as i32;
    let xf = x - x.floor();
    let u = fade(xf);
    let du = fade_deriv(xf);
    let a = grad1(hash_u32(xi as u32), xf);
    let b = grad1(hash_u32((xi + 1) as u32), xf - 1.0);
    let ga = if hash_u32(xi as u32) & 1 == 0 {
        1.0
    } else {
        -1.0
    };
    let gb = if hash_u32((xi + 1) as u32) & 1 == 0 {
        1.0
    } else {
        -1.0
    };
    let val = lerp(a, b, u);
    let deriv = lerp(ga, gb, u) + du * (b - a);
    (val, deriv)
}

/// 2D Perlin noise with analytical derivatives.
pub fn perlin2_deriv(x: Float, y: Float) -> (Float, Float, Float) {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let du = fade_deriv(xf);
    let dv = fade_deriv(yf);
    let aa = hash2(xi, yi);
    let ab = hash2(xi, yi + 1);
    let ba = hash2(xi + 1, yi);
    let bb = hash2(xi + 1, yi + 1);
    let vaa = grad2(aa, xf, yf);
    let vba = grad2(ba, xf - 1.0, yf);
    let vab = grad2(ab, xf, yf - 1.0);
    let vbb = grad2(bb, xf - 1.0, yf - 1.0);
    let k0 = vaa;
    let k1 = vba - vaa;
    let k2 = vab - vaa;
    let k3 = vaa - vba - vab + vbb;
    let val = k0 + k1 * u + k2 * v + k3 * u * v;
    let dx = du * (k1 + k3 * v);
    let dy = dv * (k2 + k3 * u);
    (val, dx, dy)
}

/// 4D cell noise.
pub fn cellnoise4(x: Float, y: Float, z: Float, w: Float) -> Float {
    hash_to_float01(hash4(
        x.floor() as i32,
        y.floor() as i32,
        z.floor() as i32,
        w.floor() as i32,
    ))
}

/// 4D hash noise.
pub fn hashnoise4(x: Float, y: Float, z: Float, w: Float) -> Float {
    hash_to_float01(hash4(
        x.to_bits() as i32,
        y.to_bits() as i32,
        z.to_bits() as i32,
        w.to_bits() as i32,
    ))
}

/// 3D hash noise returning Vec3.
/// Uses `inthash_vec3` matching C++ `HashNoise::hashVec(x,y,z)` which uses
/// bit_cast (to_bits) for coordinate hashing.
pub fn vhashnoise3(p: Vec3) -> Vec3 {
    let v = crate::hashes::inthash_vec3(p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
    Vec3::new(v[0], v[1], v[2])
}

/// 1D periodic Perlin noise.
pub fn pperlin1(x: Float, period: Float) -> Float {
    let px = period.max(1.0) as i32;
    let xi = x.floor() as i32;
    let xf = x - x.floor();
    let u = fade(xf);
    let wrap = |c: i32, p: i32| ((c % p) + p) % p;
    let a = grad1(hash_u32(wrap(xi, px) as u32), xf);
    let b = grad1(hash_u32(wrap(xi + 1, px) as u32), xf - 1.0);
    lerp(a, b, u)
}

/// 2D periodic Perlin noise.
pub fn pperlin2(x: Float, y: Float, px: Float, py: Float) -> Float {
    let ppx = px.max(1.0) as i32;
    let ppy = py.max(1.0) as i32;
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let wrap = |c: i32, p: i32| ((c % p) + p) % p;
    let x0 = wrap(xi, ppx);
    let x1 = wrap(xi + 1, ppx);
    let y0 = wrap(yi, ppy);
    let y1 = wrap(yi + 1, ppy);
    let x1l = lerp(
        grad2(hash2(x0, y0), xf, yf),
        grad2(hash2(x1, y0), xf - 1.0, yf),
        u,
    );
    let x2l = lerp(
        grad2(hash2(x0, y1), xf, yf - 1.0),
        grad2(hash2(x1, y1), xf - 1.0, yf - 1.0),
        u,
    );
    lerp(x1l, x2l, v)
}

/// 3D periodic cell noise.
pub fn pcellnoise3(p: Vec3, period: Vec3) -> Float {
    let px = period.x.max(1.0) as i32;
    let py = period.y.max(1.0) as i32;
    let pz = period.z.max(1.0) as i32;
    let wrap = |c: i32, p: i32| ((c % p) + p) % p;
    hash_to_float01(hash3(
        wrap(p.x.floor() as i32, px),
        wrap(p.y.floor() as i32, py),
        wrap(p.z.floor() as i32, pz),
    ))
}

/// 3D periodic hash noise.
pub fn phashnoise3(p: Vec3, period: Vec3) -> Float {
    // Hash noise doesn't floor, but periodic means we wrap the raw value
    let px = period.x.max(1.0);
    let py = period.y.max(1.0);
    let pz = period.z.max(1.0);
    let wx = p.x - (p.x / px).floor() * px;
    let wy = p.y - (p.y / py).floor() * py;
    let wz = p.z - (p.z / pz).floor() * pz;
    hashnoise3(Vec3::new(wx, wy, wz))
}

// ---------------------------------------------------------------------------
// Unsigned Perlin noise [0, 1]
// ---------------------------------------------------------------------------

/// 1D unsigned Perlin noise, range [0, 1].
#[inline]
pub fn uperlin1(x: Float) -> Float {
    perlin1(x) * 0.5 + 0.5
}

/// 2D unsigned Perlin noise, range [0, 1].
#[inline]
pub fn uperlin2(x: Float, y: Float) -> Float {
    perlin2(x, y) * 0.5 + 0.5
}

/// 3D unsigned Perlin noise, range [0, 1].
#[inline]
pub fn uperlin3(p: Vec3) -> Float {
    perlin3(p) * 0.5 + 0.5
}

// ---------------------------------------------------------------------------
// Periodic Perlin noise
// ---------------------------------------------------------------------------

/// Periodic 3D Perlin noise with period `period` per axis.
pub fn pperlin3(p: Vec3, period: Vec3) -> Float {
    let px = period.x.max(1.0) as i32;
    let py = period.y.max(1.0) as i32;
    let pz = period.z.max(1.0) as i32;

    let xi = p.x.floor() as i32;
    let yi = p.y.floor() as i32;
    let zi = p.z.floor() as i32;
    let xf = p.x - p.x.floor();
    let yf = p.y - p.y.floor();
    let zf = p.z - p.z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let wrap = |c: i32, p: i32| ((c % p) + p) % p;

    let x0 = wrap(xi, px);
    let x1 = wrap(xi + 1, px);
    let y0 = wrap(yi, py);
    let y1 = wrap(yi + 1, py);
    let z0 = wrap(zi, pz);
    let z1 = wrap(zi + 1, pz);

    let aaa = grad3(hash3(x0, y0, z0), xf, yf, zf);
    let baa = grad3(hash3(x1, y0, z0), xf - 1.0, yf, zf);
    let aba = grad3(hash3(x0, y1, z0), xf, yf - 1.0, zf);
    let bba = grad3(hash3(x1, y1, z0), xf - 1.0, yf - 1.0, zf);
    let aab = grad3(hash3(x0, y0, z1), xf, yf, zf - 1.0);
    let bab = grad3(hash3(x1, y0, z1), xf - 1.0, yf, zf - 1.0);
    let abb = grad3(hash3(x0, y1, z1), xf, yf - 1.0, zf - 1.0);
    let bbb = grad3(hash3(x1, y1, z1), xf - 1.0, yf - 1.0, zf - 1.0);

    let x1l = lerp(aaa, baa, u);
    let x2l = lerp(aba, bba, u);
    let y1l = lerp(x1l, x2l, v);

    let x3l = lerp(aab, bab, u);
    let x4l = lerp(abb, bbb, u);
    let y2l = lerp(x3l, x4l, v);

    lerp(y1l, y2l, w)
}

/// 3D periodic Perlin noise with analytical derivatives.
/// Returns (value, gradient). The gradient accounts for the periodic wrapping.
pub fn pperlin3_deriv(p: Vec3, period: Vec3) -> (Float, Vec3) {
    let px = period.x.max(1.0) as i32;
    let py = period.y.max(1.0) as i32;
    let pz = period.z.max(1.0) as i32;

    let xi = p.x.floor() as i32;
    let yi = p.y.floor() as i32;
    let zi = p.z.floor() as i32;
    let xf = p.x - p.x.floor();
    let yf = p.y - p.y.floor();
    let zf = p.z - p.z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);
    let du = fade_deriv(xf);
    let dv = fade_deriv(yf);
    let dw = fade_deriv(zf);

    let wrap = |c: i32, p: i32| ((c % p) + p) % p;

    let x0 = wrap(xi, px);
    let x1 = wrap(xi + 1, px);
    let y0 = wrap(yi, py);
    let y1 = wrap(yi + 1, py);
    let z0 = wrap(zi, pz);
    let z1 = wrap(zi + 1, pz);

    // Hashes for 8 corners (using periodic wrapping)
    let h000 = hash3(x0, y0, z0);
    let h100 = hash3(x1, y0, z0);
    let h010 = hash3(x0, y1, z0);
    let h110 = hash3(x1, y1, z0);
    let h001 = hash3(x0, y0, z1);
    let h101 = hash3(x1, y0, z1);
    let h011 = hash3(x0, y1, z1);
    let h111 = hash3(x1, y1, z1);

    // Gradient vectors at each corner
    let ga = grad3_vec(h000);
    let gb = grad3_vec(h100);
    let gc = grad3_vec(h010);
    let gd = grad3_vec(h110);
    let ge = grad3_vec(h001);
    let gf = grad3_vec(h101);
    let gg = grad3_vec(h011);
    let gh = grad3_vec(h111);

    // Dot products (noise values at corners — fractional coords are not wrapped)
    let va = ga.x * xf + ga.y * yf + ga.z * zf;
    let vb = gb.x * (xf - 1.0) + gb.y * yf + gb.z * zf;
    let vc = gc.x * xf + gc.y * (yf - 1.0) + gc.z * zf;
    let vd = gd.x * (xf - 1.0) + gd.y * (yf - 1.0) + gd.z * zf;
    let ve = ge.x * xf + ge.y * yf + ge.z * (zf - 1.0);
    let vf = gf.x * (xf - 1.0) + gf.y * yf + gf.z * (zf - 1.0);
    let vg = gg.x * xf + gg.y * (yf - 1.0) + gg.z * (zf - 1.0);
    let vh = gh.x * (xf - 1.0) + gh.y * (yf - 1.0) + gh.z * (zf - 1.0);

    // Interpolation coefficients (trilinear polynomial form)
    let k0 = va;
    let k1 = vb - va;
    let k2 = vc - va;
    let k3 = ve - va;
    let k4 = va - vb - vc + vd;
    let k5 = va - vc - ve + vg;
    let k6 = va - vb - ve + vf;
    let k7 = -va + vb + vc - vd + ve - vf - vg + vh;

    let val = k0 + k1 * u + k2 * v + k3 * w + k4 * u * v + k5 * v * w + k6 * u * w + k7 * u * v * w;

    // Derivative: trilinear interpolation of gradient vectors + fade derivative terms
    let interp_g = Vec3::new(
        ga.x + u * (gb.x - ga.x)
            + v * (gc.x - ga.x)
            + w * (ge.x - ga.x)
            + u * v * (ga.x - gb.x - gc.x + gd.x)
            + v * w * (ga.x - gc.x - ge.x + gg.x)
            + u * w * (ga.x - gb.x - ge.x + gf.x)
            + u * v * w * (-ga.x + gb.x + gc.x - gd.x + ge.x - gf.x - gg.x + gh.x),
        ga.y + u * (gb.y - ga.y)
            + v * (gc.y - ga.y)
            + w * (ge.y - ga.y)
            + u * v * (ga.y - gb.y - gc.y + gd.y)
            + v * w * (ga.y - gc.y - ge.y + gg.y)
            + u * w * (ga.y - gb.y - ge.y + gf.y)
            + u * v * w * (-ga.y + gb.y + gc.y - gd.y + ge.y - gf.y - gg.y + gh.y),
        ga.z + u * (gb.z - ga.z)
            + v * (gc.z - ga.z)
            + w * (ge.z - ga.z)
            + u * v * (ga.z - gb.z - gc.z + gd.z)
            + v * w * (ga.z - gc.z - ge.z + gg.z)
            + u * w * (ga.z - gb.z - ge.z + gf.z)
            + u * v * w * (-ga.z + gb.z + gc.z - gd.z + ge.z - gf.z - gg.z + gh.z),
    );

    let fade_deriv_contrib = Vec3::new(
        du * (k1 + k4 * v + k6 * w + k7 * v * w),
        dv * (k2 + k4 * u + k5 * w + k7 * u * w),
        dw * (k3 + k5 * v + k6 * u + k7 * u * v),
    );

    let deriv = interp_g + fade_deriv_contrib;

    (val, deriv)
}

/// 4D periodic Perlin noise with modular hash wrapping.
pub fn pperlin4(x: Float, y: Float, z: Float, w: Float, period: [Float; 4]) -> Float {
    let px = period[0].max(1.0) as i32;
    let py = period[1].max(1.0) as i32;
    let pz = period[2].max(1.0) as i32;
    let pw = period[3].max(1.0) as i32;

    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let wi = w.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();
    let fw = w - w.floor();

    let u = fade(fx);
    let v = fade(fy);
    let t = fade(fz);
    let s = fade(fw);

    let wrap = |c: i32, p: i32| ((c % p) + p) % p;

    let x0 = wrap(xi, px);
    let x1 = wrap(xi + 1, px);
    let y0 = wrap(yi, py);
    let y1 = wrap(yi + 1, py);
    let z0 = wrap(zi, pz);
    let z1 = wrap(zi + 1, pz);
    let w0 = wrap(wi, pw);
    let w1 = wrap(wi + 1, pw);

    // 16 corners with periodic hash wrapping
    let g0000 = grad4(hash4(x0, y0, z0, w0), fx, fy, fz, fw);
    let g1000 = grad4(hash4(x1, y0, z0, w0), fx - 1.0, fy, fz, fw);
    let g0100 = grad4(hash4(x0, y1, z0, w0), fx, fy - 1.0, fz, fw);
    let g1100 = grad4(hash4(x1, y1, z0, w0), fx - 1.0, fy - 1.0, fz, fw);
    let g0010 = grad4(hash4(x0, y0, z1, w0), fx, fy, fz - 1.0, fw);
    let g1010 = grad4(hash4(x1, y0, z1, w0), fx - 1.0, fy, fz - 1.0, fw);
    let g0110 = grad4(hash4(x0, y1, z1, w0), fx, fy - 1.0, fz - 1.0, fw);
    let g1110 = grad4(hash4(x1, y1, z1, w0), fx - 1.0, fy - 1.0, fz - 1.0, fw);
    let g0001 = grad4(hash4(x0, y0, z0, w1), fx, fy, fz, fw - 1.0);
    let g1001 = grad4(hash4(x1, y0, z0, w1), fx - 1.0, fy, fz, fw - 1.0);
    let g0101 = grad4(hash4(x0, y1, z0, w1), fx, fy - 1.0, fz, fw - 1.0);
    let g1101 = grad4(hash4(x1, y1, z0, w1), fx - 1.0, fy - 1.0, fz, fw - 1.0);
    let g0011 = grad4(hash4(x0, y0, z1, w1), fx, fy, fz - 1.0, fw - 1.0);
    let g1011 = grad4(hash4(x1, y0, z1, w1), fx - 1.0, fy, fz - 1.0, fw - 1.0);
    let g0111 = grad4(hash4(x0, y1, z1, w1), fx, fy - 1.0, fz - 1.0, fw - 1.0);
    let g1111 = grad4(
        hash4(x1, y1, z1, w1),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
        fw - 1.0,
    );

    // Quadrilinear interpolation
    let lx1 = lerp(g0000, g1000, u);
    let lx2 = lerp(g0100, g1100, u);
    let ly1 = lerp(lx1, lx2, v);
    let lx3 = lerp(g0010, g1010, u);
    let lx4 = lerp(g0110, g1110, u);
    let ly2 = lerp(lx3, lx4, v);
    let lz0 = lerp(ly1, ly2, t);

    let lx5 = lerp(g0001, g1001, u);
    let lx6 = lerp(g0101, g1101, u);
    let ly3 = lerp(lx5, lx6, v);
    let lx7 = lerp(g0011, g1011, u);
    let lx8 = lerp(g0111, g1111, u);
    let ly4 = lerp(lx7, lx8, v);
    let lz1 = lerp(ly3, ly4, t);

    0.8344 * lerp(lz0, lz1, s)
}

// ---------------------------------------------------------------------------
// Noise dispatch (by name)
// ---------------------------------------------------------------------------

/// 4D unsigned Perlin noise.
#[inline]
pub fn uperlin4(x: Float, y: Float, z: Float, w: Float) -> Float {
    perlin4(x, y, z, w) * 0.5 + 0.5
}

/// Signed periodic Perlin noise (psnoise).
#[inline]
pub fn psnoise3(p: Vec3, period: Vec3) -> Float {
    pperlin3(p, period)
}

/// Unsigned periodic Perlin noise (pnoise).
#[inline]
pub fn upnoise3(p: Vec3, period: Vec3) -> Float {
    pperlin3(p, period) * 0.5 + 0.5
}

/// Evaluate scalar noise by name at a 3D point.
/// Returns signed noise for "perlin"/"snoise", unsigned for "noise"/"uperlin"/"cell"/"hash".
pub fn noise_by_name(name: &str, p: Vec3) -> Float {
    match name {
        "perlin" | "snoise" => perlin3(p),
        "uperlin" | "noise" => uperlin3(p),
        "cell" | "cellnoise" => cellnoise3(p),
        "hash" | "hashnoise" => hashnoise3(p),
        "simplex" | "simplexnoise" => crate::simplex::simplex3(p),
        "usimplex" | "usimplexnoise" => crate::simplex::usimplex3(p),
        "gabor" | "gabornoise" => crate::gabor::gabor3_default(p),
        _ => 0.0,
    }
}

/// Evaluate Vec3-valued noise by name at a 3D point.
pub fn vnoise_by_name(name: &str, p: Vec3) -> Vec3 {
    match name {
        "perlin" | "snoise" => vperlin3(p),
        "uperlin" | "noise" => {
            let v = vperlin3(p);
            Vec3::new(v.x * 0.5 + 0.5, v.y * 0.5 + 0.5, v.z * 0.5 + 0.5)
        }
        "cell" | "cellnoise" => cellnoise3_v(p),
        "hash" | "hashnoise" => vhashnoise3(p),
        "simplex" | "simplexnoise" => {
            // C++ uses simplex3<seed>(p) with seed=0/1/2 per component
            Vec3::new(
                crate::simplex::simplex3_seeded(p, 0),
                crate::simplex::simplex3_seeded(p, 1),
                crate::simplex::simplex3_seeded(p, 2),
            )
        }
        "usimplex" | "usimplexnoise" => {
            // Unsigned variant: remap [-1,1] -> [0,1]
            Vec3::new(
                crate::simplex::simplex3_seeded(p, 0) * 0.5 + 0.5,
                crate::simplex::simplex3_seeded(p, 1) * 0.5 + 0.5,
                crate::simplex::simplex3_seeded(p, 2) * 0.5 + 0.5,
            )
        }
        "gabor" | "gabornoise" => {
            let v = crate::gabor::gabor3_default(p);
            Vec3::new(v, v, v) // Gabor is scalar; broadcast
        }
        _ => Vec3::ZERO,
    }
}

/// Evaluate scalar periodic noise by name at a 3D point.
pub fn pnoise_by_name(name: &str, p: Vec3, period: Vec3) -> Float {
    match name {
        "perlin" | "snoise" | "pnoise" | "psnoise" => pperlin3(p, period),
        "uperlin" | "noise" => pperlin3(p, period) * 0.5 + 0.5,
        "cell" | "cellnoise" | "pcellnoise" => {
            // Periodic cell noise: wrap then cell
            let wp = Vec3::new(
                wrap_periodic(p.x, period.x),
                wrap_periodic(p.y, period.y),
                wrap_periodic(p.z, period.z),
            );
            cellnoise3(wp)
        }
        "hash" | "hashnoise" | "phashnoise" => {
            let wp = Vec3::new(
                wrap_periodic(p.x, period.x),
                wrap_periodic(p.y, period.y),
                wrap_periodic(p.z, period.z),
            );
            hashnoise3(wp)
        }
        _ => 0.0,
    }
}

/// Evaluate Vec3-valued periodic noise by name.
pub fn vpnoise_by_name(name: &str, p: Vec3, period: Vec3) -> Vec3 {
    match name {
        "perlin" | "snoise" | "pnoise" | "psnoise" => vpperlin3(p, period),
        "uperlin" | "noise" => {
            let v = vpperlin3(p, period);
            Vec3::new(v.x * 0.5 + 0.5, v.y * 0.5 + 0.5, v.z * 0.5 + 0.5)
        }
        "cell" | "cellnoise" | "pcellnoise" => {
            let wp = Vec3::new(
                wrap_periodic(p.x, period.x),
                wrap_periodic(p.y, period.y),
                wrap_periodic(p.z, period.z),
            );
            cellnoise3_v(wp)
        }
        "hash" | "hashnoise" | "phashnoise" => {
            let wp = Vec3::new(
                wrap_periodic(p.x, period.x),
                wrap_periodic(p.y, period.y),
                wrap_periodic(p.z, period.z),
            );
            vhashnoise3(wp)
        }
        _ => Vec3::ZERO,
    }
}

/// Vec3-valued periodic Perlin noise.
pub fn vpperlin3(p: Vec3, period: Vec3) -> Vec3 {
    Vec3::new(
        pperlin3(p, period),
        pperlin3(Vec3::new(p.x + 31.416, p.y + 47.853, p.z + 12.793), period),
        pperlin3(Vec3::new(p.x + 71.337, p.y + 23.519, p.z + 59.167), period),
    )
}

/// Wrap coordinate for periodic use.
#[inline]
fn wrap_periodic(x: Float, period: Float) -> Float {
    if period > 0.0 {
        x - (x / period).floor() * period
    } else {
        x
    }
}

/// Null noise — always returns 0. Matches C++ NullNoise.
pub fn nullnoise(_p: Vec3) -> Float {
    0.0
}
/// Unsigned null noise — always returns 0.5. Matches C++ UNullNoise.
pub fn unullnoise(_p: Vec3) -> Float {
    0.5
}

// ---------------------------------------------------------------------------
// Dual2-based noise — automatic derivative propagation
// Matches C++ oslnoise.h: noise functions that accept Dual2<Float> and
// Dual2<Vec3> arguments and return Dual2<Float> or Dual2<Vec3>.
// ---------------------------------------------------------------------------

use crate::dual::Dual2;

/// Dual2 Perlin noise at a 3D Dual2<Vec3> position.
/// Returns Dual2<Float> with properly propagated derivatives.
pub fn perlin3_dual(p: Dual2<Vec3>) -> Dual2<Float> {
    let (val, grad) = perlin3_deriv(p.val);
    // Chain rule: df/dt = grad . dp/dt
    let dx = grad.x * p.dx.x + grad.y * p.dx.y + grad.z * p.dx.z;
    let dy = grad.x * p.dy.x + grad.y * p.dy.y + grad.z * p.dy.z;
    Dual2 { val, dx, dy }
}

/// Dual2 unsigned Perlin noise at a 3D Dual2<Vec3> position.
pub fn uperlin3_dual(p: Dual2<Vec3>) -> Dual2<Float> {
    let d = perlin3_dual(p);
    Dual2 {
        val: d.val * 0.5 + 0.5,
        dx: d.dx * 0.5,
        dy: d.dy * 0.5,
    }
}

/// Dual2 Perlin noise at a 1D Dual2<Float> position.
pub fn perlin1_dual(x: Dual2<Float>) -> Dual2<Float> {
    let (val, dval) = perlin1_deriv(x.val);
    Dual2 {
        val,
        dx: dval * x.dx,
        dy: dval * x.dy,
    }
}

/// Dual2 Perlin noise at a 2D Dual2<Float> position.
pub fn perlin2_dual(x: Dual2<Float>, y: Dual2<Float>) -> Dual2<Float> {
    let (val, dvdx, dvdy) = perlin2_deriv(x.val, y.val);
    Dual2 {
        val,
        dx: dvdx * x.dx + dvdy * y.dx,
        dy: dvdx * x.dy + dvdy * y.dy,
    }
}

/// Dual2 cell noise at a 3D Dual2<Vec3> position.
/// Cell noise is constant per cell, so derivatives are always zero.
pub fn cellnoise3_dual(p: Dual2<Vec3>) -> Dual2<Float> {
    Dual2 {
        val: cellnoise3(p.val),
        dx: 0.0,
        dy: 0.0,
    }
}

/// Dual2 hash noise at a 3D Dual2<Vec3> position.
/// Hash noise is discontinuous, so derivatives are zero.
pub fn hashnoise3_dual(p: Dual2<Vec3>) -> Dual2<Float> {
    Dual2 {
        val: hashnoise3(p.val),
        dx: 0.0,
        dy: 0.0,
    }
}

/// Dual2 Vec3-valued Perlin noise at a 3D Dual2<Vec3> position.
pub fn vperlin3_dual(p: Dual2<Vec3>) -> Dual2<Vec3> {
    // Each component is an independent noise with the same position
    // but different offset seeds (matching vperlin3 offsets)
    let p0 = p;
    let p1 = Dual2 {
        val: Vec3::new(p.val.x + 31.416, p.val.y + 47.853, p.val.z + 12.793),
        dx: p.dx,
        dy: p.dy,
    };
    let p2 = Dual2 {
        val: Vec3::new(p.val.x + 71.337, p.val.y + 23.519, p.val.z + 59.167),
        dx: p.dx,
        dy: p.dy,
    };

    let n0 = perlin3_dual(p0);
    let n1 = perlin3_dual(p1);
    let n2 = perlin3_dual(p2);

    Dual2 {
        val: Vec3::new(n0.val, n1.val, n2.val),
        dx: Vec3::new(n0.dx, n1.dx, n2.dx),
        dy: Vec3::new(n0.dy, n1.dy, n2.dy),
    }
}

/// Dual2 simplex noise at a 3D Dual2<Vec3> position.
pub fn simplex3_dual(p: Dual2<Vec3>) -> Dual2<Float> {
    let (val, grad) = crate::simplex::simplex3_deriv(p.val);
    let dx = grad.x * p.dx.x + grad.y * p.dx.y + grad.z * p.dx.z;
    let dy = grad.x * p.dy.x + grad.y * p.dy.y + grad.z * p.dy.z;
    Dual2 { val, dx, dy }
}

/// Dual2 unsigned simplex noise at a 3D Dual2<Vec3> position.
pub fn usimplex3_dual(p: Dual2<Vec3>) -> Dual2<Float> {
    let d = simplex3_dual(p);
    Dual2 {
        val: d.val * 0.5 + 0.5,
        dx: d.dx * 0.5,
        dy: d.dy * 0.5,
    }
}

/// Dual2 Gabor noise at a 3D Dual2<Vec3> position.
pub fn gabor3_dual(p: Dual2<Vec3>, params: &crate::gabor::GaborParams) -> Dual2<Float> {
    let (val, grad) = crate::gabor::gabor3_deriv(p.val, params);
    let dx = grad.x * p.dx.x + grad.y * p.dx.y + grad.z * p.dx.z;
    let dy = grad.x * p.dy.x + grad.y * p.dy.y + grad.z * p.dy.z;
    Dual2 { val, dx, dy }
}

/// Dual2 periodic Perlin noise at a 3D Dual2<Vec3> position.
/// Uses analytical derivatives from `pperlin3_deriv` with chain rule.
pub fn pperlin3_dual(p: Dual2<Vec3>, period: Vec3) -> Dual2<Float> {
    let (val, grad) = pperlin3_deriv(p.val, period);
    let dx = grad.x * p.dx.x + grad.y * p.dx.y + grad.z * p.dx.z;
    let dy = grad.x * p.dy.x + grad.y * p.dy.y + grad.z * p.dy.z;
    Dual2 { val, dx, dy }
}

/// Dual2 unsigned periodic Perlin noise.
pub fn upperlin3_dual(p: Dual2<Vec3>, period: Vec3) -> Dual2<Float> {
    let d = pperlin3_dual(p, period);
    Dual2 {
        val: d.val * 0.5 + 0.5,
        dx: d.dx * 0.5,
        dy: d.dy * 0.5,
    }
}

/// Dual2 Vec3-valued periodic Perlin noise.
pub fn vpperlin3_dual(p: Dual2<Vec3>, period: Vec3) -> Dual2<Vec3> {
    let p0 = p;
    let p1 = Dual2 {
        val: Vec3::new(p.val.x + 31.416, p.val.y + 47.853, p.val.z + 12.793),
        dx: p.dx,
        dy: p.dy,
    };
    let p2 = Dual2 {
        val: Vec3::new(p.val.x + 71.337, p.val.y + 23.519, p.val.z + 59.167),
        dx: p.dx,
        dy: p.dy,
    };
    let n0 = pperlin3_dual(p0, period);
    let n1 = pperlin3_dual(p1, period);
    let n2 = pperlin3_dual(p2, period);
    Dual2 {
        val: Vec3::new(n0.val, n1.val, n2.val),
        dx: Vec3::new(n0.dx, n1.dx, n2.dx),
        dy: Vec3::new(n0.dy, n1.dy, n2.dy),
    }
}

/// Dispatch dual noise by name (scalar output).
pub fn noise_dual_by_name(name: &str, p: Dual2<Vec3>) -> Dual2<Float> {
    match name {
        "perlin" | "snoise" => perlin3_dual(p),
        "uperlin" | "noise" => uperlin3_dual(p),
        "cell" | "cellnoise" => cellnoise3_dual(p),
        "hash" | "hashnoise" => hashnoise3_dual(p),
        "simplex" | "simplexnoise" => simplex3_dual(p),
        "usimplex" | "usimplexnoise" => usimplex3_dual(p),
        "gabor" | "gabornoise" => gabor3_dual(p, &crate::gabor::GaborParams::default()),
        _ => Dual2 {
            val: 0.0,
            dx: 0.0,
            dy: 0.0,
        },
    }
}

/// Dispatch dual periodic noise by name (scalar output).
pub fn pnoise_dual_by_name(name: &str, p: Dual2<Vec3>, period: Vec3) -> Dual2<Float> {
    match name {
        "perlin" | "snoise" | "pnoise" | "psnoise" => pperlin3_dual(p, period),
        "uperlin" | "noise" => upperlin3_dual(p, period),
        "cell" | "cellnoise" => Dual2 {
            val: pcellnoise3(p.val, period),
            dx: 0.0,
            dy: 0.0,
        },
        "hash" | "hashnoise" => Dual2 {
            val: phashnoise3(p.val, period),
            dx: 0.0,
            dy: 0.0,
        },
        _ => pperlin3_dual(p, period),
    }
}

/// Dispatch dual periodic noise by name (Vec3 output).
pub fn vpnoise_dual_by_name(name: &str, p: Dual2<Vec3>, period: Vec3) -> Dual2<Vec3> {
    match name {
        "perlin" | "snoise" | "pnoise" | "psnoise" => vpperlin3_dual(p, period),
        "uperlin" | "noise" => {
            let v = vpperlin3_dual(p, period);
            Dual2 {
                val: Vec3::new(
                    v.val.x * 0.5 + 0.5,
                    v.val.y * 0.5 + 0.5,
                    v.val.z * 0.5 + 0.5,
                ),
                dx: Vec3::new(v.dx.x * 0.5, v.dx.y * 0.5, v.dx.z * 0.5),
                dy: Vec3::new(v.dy.x * 0.5, v.dy.y * 0.5, v.dy.z * 0.5),
            }
        }
        "cell" | "cellnoise" => Dual2 {
            val: cellnoise3_v(p.val), // cell noise = zero derivs (periodic too)
            dx: Vec3::ZERO,
            dy: Vec3::ZERO,
        },
        _ => vpperlin3_dual(p, period),
    }
}

/// Dual2 Vec3-valued simplex noise (3 independent channels with offset seeds).
pub fn vsimplex3_dual(p: Dual2<Vec3>) -> Dual2<Vec3> {
    let p0 = p;
    let p1 = Dual2 {
        val: Vec3::new(p.val.x + 31.416, p.val.y + 47.853, p.val.z + 12.793),
        dx: p.dx,
        dy: p.dy,
    };
    let p2 = Dual2 {
        val: Vec3::new(p.val.x + 71.337, p.val.y + 23.519, p.val.z + 59.167),
        dx: p.dx,
        dy: p.dy,
    };
    let n0 = simplex3_dual(p0);
    let n1 = simplex3_dual(p1);
    let n2 = simplex3_dual(p2);
    Dual2 {
        val: Vec3::new(n0.val, n1.val, n2.val),
        dx: Vec3::new(n0.dx, n1.dx, n2.dx),
        dy: Vec3::new(n0.dy, n1.dy, n2.dy),
    }
}

/// Dispatch dual noise by name (Vec3 output).
pub fn vnoise_dual_by_name(name: &str, p: Dual2<Vec3>) -> Dual2<Vec3> {
    match name {
        "perlin" | "snoise" => vperlin3_dual(p),
        "uperlin" | "noise" => {
            let v = vperlin3_dual(p);
            Dual2 {
                val: Vec3::new(
                    v.val.x * 0.5 + 0.5,
                    v.val.y * 0.5 + 0.5,
                    v.val.z * 0.5 + 0.5,
                ),
                dx: Vec3::new(v.dx.x * 0.5, v.dx.y * 0.5, v.dx.z * 0.5),
                dy: Vec3::new(v.dy.x * 0.5, v.dy.y * 0.5, v.dy.z * 0.5),
            }
        }
        "cell" | "cellnoise" => Dual2 {
            val: cellnoise3_v(p.val),
            dx: Vec3::ZERO,
            dy: Vec3::ZERO,
        },
        "simplex" | "simplexnoise" => vsimplex3_dual(p),
        "usimplex" | "usimplexnoise" => {
            let v = vsimplex3_dual(p);
            Dual2 {
                val: Vec3::new(
                    v.val.x * 0.5 + 0.5,
                    v.val.y * 0.5 + 0.5,
                    v.val.z * 0.5 + 0.5,
                ),
                dx: Vec3::new(v.dx.x * 0.5, v.dx.y * 0.5, v.dx.z * 0.5),
                dy: Vec3::new(v.dy.x * 0.5, v.dy.y * 0.5, v.dy.z * 0.5),
            }
        }
        _ => Dual2 {
            val: Vec3::ZERO,
            dx: Vec3::ZERO,
            dy: Vec3::ZERO,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cellnoise_deterministic() {
        let v1 = cellnoise3(Vec3::new(1.5, 2.5, 3.5));
        let v2 = cellnoise3(Vec3::new(1.5, 2.5, 3.5));
        assert_eq!(v1, v2);
        assert!(v1 >= 0.0 && v1 < 1.0);
    }

    #[test]
    fn test_cellnoise_integer_boundaries() {
        let v1 = cellnoise3(Vec3::new(1.1, 2.1, 3.1));
        let v2 = cellnoise3(Vec3::new(1.9, 2.9, 3.9));
        assert_eq!(v1, v2); // same cell
    }

    #[test]
    fn test_perlin_range() {
        // Perlin noise should roughly be in [-1, 1]
        let mut min_v = f32::MAX;
        let mut max_v = f32::MIN;
        for i in 0..1000 {
            let x = i as f32 * 0.1;
            let v = perlin1(x);
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
        assert!(min_v > -1.5);
        assert!(max_v < 1.5);
    }

    #[test]
    fn test_perlin3_deterministic() {
        let p = Vec3::new(1.234, 5.678, 9.012);
        assert_eq!(perlin3(p), perlin3(p));
    }

    #[test]
    fn test_perlin3_deriv() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let (_val, deriv) = perlin3_deriv(p);

        // Verify derivative numerically
        let eps = 1e-4;
        let dx = (perlin3(Vec3::new(p.x + eps, p.y, p.z))
            - perlin3(Vec3::new(p.x - eps, p.y, p.z)))
            / (2.0 * eps);
        let dy = (perlin3(Vec3::new(p.x, p.y + eps, p.z))
            - perlin3(Vec3::new(p.x, p.y - eps, p.z)))
            / (2.0 * eps);
        let dz = (perlin3(Vec3::new(p.x, p.y, p.z + eps))
            - perlin3(Vec3::new(p.x, p.y, p.z - eps)))
            / (2.0 * eps);

        assert!((deriv.x - dx).abs() < 0.05, "dx: {} vs {}", deriv.x, dx);
        assert!((deriv.y - dy).abs() < 0.05, "dy: {} vs {}", deriv.y, dy);
        assert!((deriv.z - dz).abs() < 0.05, "dz: {} vs {}", deriv.z, dz);
    }

    #[test]
    fn test_uperlin_range() {
        for i in 0..100 {
            let p = Vec3::new(i as f32 * 0.37, i as f32 * 0.53, i as f32 * 0.71);
            let v = uperlin3(p);
            assert!(v >= -0.1 && v <= 1.1, "uperlin out of range: {v}");
        }
    }

    #[test]
    fn test_periodic_perlin() {
        let period = Vec3::new(4.0, 4.0, 4.0);
        let p1 = Vec3::new(1.5, 2.5, 3.5);
        let p2 = Vec3::new(1.5 + 4.0, 2.5 + 4.0, 3.5 + 4.0);
        let v1 = pperlin3(p1, period);
        let v2 = pperlin3(p2, period);
        assert!((v1 - v2).abs() < 1e-5, "periodic failed: {v1} vs {v2}");
    }

    #[test]
    fn test_noise_by_name() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let _ = noise_by_name("perlin", p);
        let _ = noise_by_name("noise", p);
        let _ = noise_by_name("cell", p);
        let _ = noise_by_name("hash", p);
    }

    #[test]
    fn test_pperlin3_deriv() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let period = Vec3::new(8.0, 8.0, 8.0);
        let (val, deriv) = pperlin3_deriv(p, period);

        // Value should match pperlin3
        let val_ref = pperlin3(p, period);
        assert!(
            (val - val_ref).abs() < 1e-6,
            "value mismatch: {val} vs {val_ref}"
        );

        // Verify derivative numerically
        let eps = 1e-4;
        let dx = (pperlin3(Vec3::new(p.x + eps, p.y, p.z), period)
            - pperlin3(Vec3::new(p.x - eps, p.y, p.z), period))
            / (2.0 * eps);
        let dy = (pperlin3(Vec3::new(p.x, p.y + eps, p.z), period)
            - pperlin3(Vec3::new(p.x, p.y - eps, p.z), period))
            / (2.0 * eps);
        let dz = (pperlin3(Vec3::new(p.x, p.y, p.z + eps), period)
            - pperlin3(Vec3::new(p.x, p.y, p.z - eps), period))
            / (2.0 * eps);

        assert!((deriv.x - dx).abs() < 0.05, "dx: {} vs {}", deriv.x, dx);
        assert!((deriv.y - dy).abs() < 0.05, "dy: {} vs {}", deriv.y, dy);
        assert!((deriv.z - dz).abs() < 0.05, "dz: {} vs {}", deriv.z, dz);
    }

    #[test]
    fn test_pperlin3_deriv_periodic() {
        // Derivatives should also be periodic
        let period = Vec3::new(4.0, 4.0, 4.0);
        let p1 = Vec3::new(1.5, 2.5, 3.5);
        let p2 = Vec3::new(1.5 + 4.0, 2.5 + 4.0, 3.5 + 4.0);
        let (v1, d1) = pperlin3_deriv(p1, period);
        let (v2, d2) = pperlin3_deriv(p2, period);
        assert!((v1 - v2).abs() < 1e-5, "periodic value: {v1} vs {v2}");
        assert!(
            (d1.x - d2.x).abs() < 1e-4,
            "periodic dx: {} vs {}",
            d1.x,
            d2.x
        );
        assert!(
            (d1.y - d2.y).abs() < 1e-4,
            "periodic dy: {} vs {}",
            d1.y,
            d2.y
        );
        assert!(
            (d1.z - d2.z).abs() < 1e-4,
            "periodic dz: {} vs {}",
            d1.z,
            d2.z
        );
    }

    #[test]
    fn test_pnoise_dual_by_name() {
        let p = Dual2 {
            val: Vec3::new(1.5, 2.5, 3.5),
            dx: Vec3::new(1.0, 0.0, 0.0),
            dy: Vec3::new(0.0, 1.0, 0.0),
        };
        let period = Vec3::new(8.0, 8.0, 8.0);
        let result = pnoise_dual_by_name("perlin", p, period);
        // Just verify it runs and produces a valid value
        assert!(result.val.is_finite());
        assert!(result.dx.is_finite());
        assert!(result.dy.is_finite());
    }
}
