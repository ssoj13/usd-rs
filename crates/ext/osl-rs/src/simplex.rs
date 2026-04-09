//! Simplex noise — 1D, 2D, 3D, 4D simplex noise.
//!
//! Port of `simplexnoise.cpp` from OSL. Uses hash-based lookup (bjfinal/scramble)
//! for aperiodic noise per C++ (plan #47).

use crate::Float;
use crate::math::Vec3;

/// Bob Jenkins hash finalizer — matches OIIO::bjhash::bjfinal.
#[inline]
fn bjfinal(a: u32, b: u32, c: u32) -> u32 {
    let rotl = |x: u32, k: i32| x.rotate_left(k as u32);
    let mut c = c ^ b;
    c = c.wrapping_sub(rotl(b, 14));
    let mut a = a ^ c;
    a = a.wrapping_sub(rotl(c, 11));
    let mut b = b ^ a;
    b = b.wrapping_sub(rotl(a, 25));
    c ^= b;
    c = c.wrapping_sub(rotl(b, 16));
    a ^= c;
    a = a.wrapping_sub(rotl(c, 4));
    b ^= a;
    b = b.wrapping_sub(rotl(a, 14));
    c ^= b;
    c = c.wrapping_sub(rotl(b, 24));
    c
}

/// Scramble for simplex — matches C++ scramble(i, j, seed).
#[inline]
fn scramble(v0: u32, v1: u32, v2: u32) -> u32 {
    bjfinal(v0, v1, v2 ^ 0xdeadbeef)
}

// Skewing/unskewing factors for 2D, 3D, 4D
const F2: Float = 0.366_025_42; // (sqrt(3) - 1) / 2
const G2: Float = 0.211_324_87; // (3 - sqrt(3)) / 6
const F3: Float = 1.0 / 3.0;
const G3: Float = 1.0 / 6.0;
const F4: Float = 0.309_017; // (sqrt(5) - 1) / 4
const G4: Float = 0.138_196_6; // (5 - sqrt(5)) / 20

// Zero gradient for inactive corners (matches C++ zero)
const ZERO2: [Float; 2] = [0.0, 0.0];

// Gradient table for 2D — matches C++ grad2lut[8][2]
const GRAD2: [[Float; 2]; 8] = [
    [-1.0, -1.0],
    [1.0, 0.0],
    [-1.0, 0.0],
    [1.0, 1.0],
    [-1.0, 1.0],
    [0.0, -1.0],
    [0.0, 1.0],
    [1.0, -1.0],
];

// Zero gradient for inactive 3D corners
const ZERO3: [Float; 3] = [0.0, 0.0, 0.0];

// Gradient vectors for 3D — matches C++ grad3lut[16][3]
static GRAD3: [[Float; 3]; 16] = [
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [-1.0, 0.0, 1.0],
    [0.0, -1.0, 1.0],
    [1.0, 0.0, -1.0],
    [0.0, 1.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, -1.0, 0.0],
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

// Gradient vectors for 4D
static GRAD4: [[Float; 4]; 32] = [
    [0.0, 1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0, -1.0],
    [0.0, 1.0, -1.0, 1.0],
    [0.0, 1.0, -1.0, -1.0],
    [0.0, -1.0, 1.0, 1.0],
    [0.0, -1.0, 1.0, -1.0],
    [0.0, -1.0, -1.0, 1.0],
    [0.0, -1.0, -1.0, -1.0],
    [1.0, 0.0, 1.0, 1.0],
    [1.0, 0.0, 1.0, -1.0],
    [1.0, 0.0, -1.0, 1.0],
    [1.0, 0.0, -1.0, -1.0],
    [-1.0, 0.0, 1.0, 1.0],
    [-1.0, 0.0, 1.0, -1.0],
    [-1.0, 0.0, -1.0, 1.0],
    [-1.0, 0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0, 1.0],
    [1.0, 1.0, 0.0, -1.0],
    [1.0, -1.0, 0.0, 1.0],
    [1.0, -1.0, 0.0, -1.0],
    [-1.0, 1.0, 0.0, 1.0],
    [-1.0, 1.0, 0.0, -1.0],
    [-1.0, -1.0, 0.0, 1.0],
    [-1.0, -1.0, 0.0, -1.0],
    [1.0, 1.0, 1.0, 0.0],
    [1.0, 1.0, -1.0, 0.0],
    [1.0, -1.0, 1.0, 0.0],
    [1.0, -1.0, -1.0, 0.0],
    [-1.0, 1.0, 1.0, 0.0],
    [-1.0, 1.0, -1.0, 0.0],
    [-1.0, -1.0, 1.0, 0.0],
    [-1.0, -1.0, -1.0, 0.0],
];

fn fastfloor(x: Float) -> i32 {
    let xi = x as i32;
    if x < xi as Float { xi - 1 } else { xi }
}

fn dot3(g: &[Float; 3], x: Float, y: Float, z: Float) -> Float {
    g[0] * x + g[1] * y + g[2] * z
}

fn dot4(g: &[Float; 4], x: Float, y: Float, z: Float, w: Float) -> Float {
    g[0] * x + g[1] * y + g[2] * z + g[3] * w
}

/// 1D simplex noise (plan #47 — hash-based, matches C++ simplexnoise1).
pub fn simplex1(x: Float) -> Float {
    simplex1_seeded(x, 0)
}

/// 1D simplex with explicit seed (C++ simplexnoise1(x, seed)).
pub fn simplex1_seeded(x: Float, seed: i32) -> Float {
    let i0 = fastfloor(x);
    let i1 = i0 + 1;
    let x0 = x - i0 as Float;
    let x1 = x0 - 1.0;

    let t0 = 1.0 - x0 * x0;
    let t20 = t0 * t0;
    let t40 = t20 * t20;
    let gx0 = grad1_hash(i0, seed);
    let n0 = t40 * gx0 * x0;

    let t1 = 1.0 - x1 * x1;
    let t21 = t1 * t1;
    let t41 = t21 * t21;
    let gx1 = grad1_hash(i1, seed);
    let n1 = t41 * gx1 * x1;

    const SCALE: Float = 0.36; // C++ scale
    SCALE * (n0 + n1)
}

fn grad1_hash(i: i32, seed: i32) -> Float {
    let h = scramble(i as u32, seed as u32, 0);
    let mut g = 1.0 + (h & 7) as Float;
    if (h & 8) != 0 {
        g = -g;
    }
    g
}

/// 2D simplex noise (plan #47 — hash-based, matches C++ simplexnoise2).
pub fn simplex2(x: Float, y: Float) -> Float {
    simplex2_seeded(x, y, 0)
}

/// 2D simplex with explicit seed.
pub fn simplex2_seeded(x: Float, y: Float, seed: i32) -> Float {
    let s = (x + y) * F2;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let t = (i + j) as Float * G2;
    let x0 = x - (i as Float - t);
    let y0 = y - (j as Float - t);

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - i1 as Float + G2;
    let y1 = y0 - j1 as Float + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    let mut n0 = 0.0;
    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 >= 0.0 {
        let h = scramble(i as u32, j as u32, seed as u32);
        let g = &GRAD2[(h & 7) as usize];
        let t20 = t0 * t0;
        n0 = t20 * t20 * dot2(g, x0, y0);
    }

    let mut n1 = 0.0;
    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 >= 0.0 {
        let h = scramble(
            (i as u32).wrapping_add(i1 as u32),
            (j as u32).wrapping_add(j1 as u32),
            seed as u32,
        );
        let g = &GRAD2[(h & 7) as usize];
        let t21 = t1 * t1;
        n1 = t21 * t21 * dot2(g, x1, y1);
    }

    let mut n2 = 0.0;
    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 >= 0.0 {
        let h = scramble(
            (i as u32).wrapping_add(1),
            (j as u32).wrapping_add(1),
            seed as u32,
        );
        let g = &GRAD2[(h & 7) as usize];
        let t22 = t2 * t2;
        n2 = t22 * t22 * dot2(g, x2, y2);
    }

    const SCALE: Float = 64.0; // C++ scale
    SCALE * (n0 + n1 + n2)
}

fn dot2(g: &[Float; 2], x: Float, y: Float) -> Float {
    g[0] * x + g[1] * y
}

/// 3D simplex noise (plan #47 — hash-based, matches C++ simplexnoise3).
pub fn simplex3(p: Vec3) -> Float {
    simplex3_seeded(p, 0)
}

/// 3D simplex with explicit seed.
pub fn simplex3_seeded(p: Vec3, seed: i32) -> Float {
    let x = p.x;
    let y = p.y;
    let z = p.z;

    let s = (x + y + z) * F3;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let k = fastfloor(z + s);
    let t = (i + j + k) as Float * G3;

    let x0 = x - (i as Float - t);
    let y0 = y - (j as Float - t);
    let z0 = z - (k as Float - t);

    let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
        if y0 >= z0 {
            (1, 0, 0, 1, 1, 0)
        } else if x0 >= z0 {
            (1, 0, 0, 1, 0, 1)
        } else {
            (0, 0, 1, 1, 0, 1)
        }
    } else {
        if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        }
    };

    let x1 = x0 - i1 as Float + G3;
    let y1 = y0 - j1 as Float + G3;
    let z1 = z0 - k1 as Float + G3;
    let x2 = x0 - i2 as Float + 2.0 * G3;
    let y2 = y0 - j2 as Float + 2.0 * G3;
    let z2 = z0 - k2 as Float + 2.0 * G3;
    let x3 = x0 - 1.0 + 3.0 * G3;
    let y3 = y0 - 1.0 + 3.0 * G3;
    let z3 = z0 - 1.0 + 3.0 * G3;

    fn grad3_idx(i: i32, j: i32, k: i32, seed: i32) -> usize {
        let sk = scramble(k as u32, seed as u32, 0);
        (scramble(i as u32, j as u32, sk) & 15) as usize
    }

    let mut n0 = 0.0;
    let t0 = 0.5 - x0 * x0 - y0 * y0 - z0 * z0;
    if t0 >= 0.0 {
        let gi = grad3_idx(i, j, k, seed);
        let t20 = t0 * t0;
        n0 = t20 * t20 * dot3(&GRAD3[gi], x0, y0, z0);
    }

    let mut n1 = 0.0;
    let t1 = 0.5 - x1 * x1 - y1 * y1 - z1 * z1;
    if t1 >= 0.0 {
        let gi = grad3_idx(i + i1, j + j1, k + k1, seed);
        let t21 = t1 * t1;
        n1 = t21 * t21 * dot3(&GRAD3[gi], x1, y1, z1);
    }

    let mut n2 = 0.0;
    let t2 = 0.5 - x2 * x2 - y2 * y2 - z2 * z2;
    if t2 >= 0.0 {
        let gi = grad3_idx(i + i2, j + j2, k + k2, seed);
        let t22 = t2 * t2;
        n2 = t22 * t22 * dot3(&GRAD3[gi], x2, y2, z2);
    }

    let mut n3 = 0.0;
    let t3 = 0.5 - x3 * x3 - y3 * y3 - z3 * z3;
    if t3 >= 0.0 {
        let gi = grad3_idx(i + 1, j + 1, k + 1, seed);
        let t23 = t3 * t3;
        n3 = t23 * t23 * dot3(&GRAD3[gi], x3, y3, z3);
    }

    const SCALE: Float = 68.0; // C++ scale
    SCALE * (n0 + n1 + n2 + n3)
}

/// 4D simplex noise (plan #47 — hash-based, matches C++ simplexnoise4).
pub fn simplex4(x: Float, y: Float, z: Float, w: Float) -> Float {
    simplex4_seeded(x, y, z, w, 0)
}

/// 4D simplex with explicit seed.
pub fn simplex4_seeded(x: Float, y: Float, z: Float, w: Float, seed: i32) -> Float {
    fn grad4_idx(i: i32, j: i32, k: i32, l: i32, seed: i32) -> usize {
        let skl = scramble(k as u32, l as u32, seed as u32);
        (scramble(i as u32, j as u32, skl) & 31) as usize
    }

    let s = (x + y + z + w) * F4;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let k = fastfloor(z + s);
    let l = fastfloor(w + s);
    let t = (i + j + k + l) as Float * G4;
    let x0 = x - (i as Float - t);
    let y0 = y - (j as Float - t);
    let z0 = z - (k as Float - t);
    let w0 = w - (l as Float - t);

    // Rank ordering for simplex vertex selection (matches C++ simplex[])
    let c1 = if x0 > y0 { 32 } else { 0 };
    let c2 = if x0 > z0 { 16 } else { 0 };
    let c3 = if y0 > z0 { 8 } else { 0 };
    let c4 = if x0 > w0 { 4 } else { 0 };
    let c5 = if y0 > w0 { 2 } else { 0 };
    let c6 = if z0 > w0 { 1 } else { 0 };
    let c = c1 | c2 | c3 | c4 | c5 | c6;

    let i1 = if SIMPLEX_LUT[c][0] >= 3 { 1 } else { 0 };
    let j1 = if SIMPLEX_LUT[c][1] >= 3 { 1 } else { 0 };
    let k1 = if SIMPLEX_LUT[c][2] >= 3 { 1 } else { 0 };
    let l1 = if SIMPLEX_LUT[c][3] >= 3 { 1 } else { 0 };
    let i2 = if SIMPLEX_LUT[c][0] >= 2 { 1 } else { 0 };
    let j2 = if SIMPLEX_LUT[c][1] >= 2 { 1 } else { 0 };
    let k2 = if SIMPLEX_LUT[c][2] >= 2 { 1 } else { 0 };
    let l2 = if SIMPLEX_LUT[c][3] >= 2 { 1 } else { 0 };
    let i3 = if SIMPLEX_LUT[c][0] >= 1 { 1 } else { 0 };
    let j3 = if SIMPLEX_LUT[c][1] >= 1 { 1 } else { 0 };
    let k3 = if SIMPLEX_LUT[c][2] >= 1 { 1 } else { 0 };
    let l3 = if SIMPLEX_LUT[c][3] >= 1 { 1 } else { 0 };

    let x1 = x0 - i1 as Float + G4;
    let y1 = y0 - j1 as Float + G4;
    let z1 = z0 - k1 as Float + G4;
    let w1 = w0 - l1 as Float + G4;
    let x2 = x0 - i2 as Float + 2.0 * G4;
    let y2 = y0 - j2 as Float + 2.0 * G4;
    let z2 = z0 - k2 as Float + 2.0 * G4;
    let w2 = w0 - l2 as Float + 2.0 * G4;
    let x3 = x0 - i3 as Float + 3.0 * G4;
    let y3 = y0 - j3 as Float + 3.0 * G4;
    let z3 = z0 - k3 as Float + 3.0 * G4;
    let w3 = w0 - l3 as Float + 3.0 * G4;
    let x4 = x0 - 1.0 + 4.0 * G4;
    let y4 = y0 - 1.0 + 4.0 * G4;
    let z4 = z0 - 1.0 + 4.0 * G4;
    let w4 = w0 - 1.0 + 4.0 * G4;

    let mut n0 = 0.0;
    let t0 = 0.5 - x0 * x0 - y0 * y0 - z0 * z0 - w0 * w0;
    if t0 >= 0.0 {
        let t20 = t0 * t0;
        n0 = t20 * t20 * dot4(&GRAD4[grad4_idx(i, j, k, l, seed)], x0, y0, z0, w0);
    }

    let mut n1 = 0.0;
    let t1 = 0.5 - x1 * x1 - y1 * y1 - z1 * z1 - w1 * w1;
    if t1 >= 0.0 {
        let t21 = t1 * t1;
        n1 = t21
            * t21
            * dot4(
                &GRAD4[grad4_idx(i + i1, j + j1, k + k1, l + l1, seed)],
                x1,
                y1,
                z1,
                w1,
            );
    }

    let mut n2 = 0.0;
    let t2 = 0.5 - x2 * x2 - y2 * y2 - z2 * z2 - w2 * w2;
    if t2 >= 0.0 {
        let t22 = t2 * t2;
        n2 = t22
            * t22
            * dot4(
                &GRAD4[grad4_idx(i + i2, j + j2, k + k2, l + l2, seed)],
                x2,
                y2,
                z2,
                w2,
            );
    }

    let mut n3 = 0.0;
    let t3 = 0.5 - x3 * x3 - y3 * y3 - z3 * z3 - w3 * w3;
    if t3 >= 0.0 {
        let t23 = t3 * t3;
        n3 = t23
            * t23
            * dot4(
                &GRAD4[grad4_idx(i + i3, j + j3, k + k3, l + l3, seed)],
                x3,
                y3,
                z3,
                w3,
            );
    }

    let mut n4 = 0.0;
    let t4 = 0.5 - x4 * x4 - y4 * y4 - z4 * z4 - w4 * w4;
    if t4 >= 0.0 {
        let t24 = t4 * t4;
        n4 = t24
            * t24
            * dot4(
                &GRAD4[grad4_idx(i + 1, j + 1, k + 1, l + 1, seed)],
                x4,
                y4,
                z4,
                w4,
            );
    }

    const SCALE: Float = 54.0; // C++ scale
    SCALE * (n0 + n1 + n2 + n3 + n4)
}

// ---------------------------------------------------------------------------
// Analytical derivatives
// ---------------------------------------------------------------------------

/// 1D simplex noise with analytical derivative (plan #47 — hash-based, matches C++ simplexnoise1).
/// Returns (value, dvalue/dx).
pub fn simplex1_deriv(x: Float) -> (Float, Float) {
    simplex1_deriv_seeded(x, 0)
}

/// 1D simplex derivative with explicit seed.
pub fn simplex1_deriv_seeded(x: Float, seed: i32) -> (Float, Float) {
    let i0 = fastfloor(x);
    let i1 = i0 + 1;
    let x0 = x - i0 as Float;
    let x1 = x0 - 1.0;

    let x20 = x0 * x0;
    let t0 = 1.0 - x20;
    let t20 = t0 * t0;
    let t40 = t20 * t20;
    let gx0 = grad1_hash(i0, seed);
    let n0 = t40 * gx0 * x0;

    let x21 = x1 * x1;
    let t1 = 1.0 - x21;
    let t21 = t1 * t1;
    let t41 = t21 * t21;
    let gx1 = grad1_hash(i1, seed);
    let n1 = t41 * gx1 * x1;

    const SCALE: Float = 0.36; // C++ scale
    let val = SCALE * (n0 + n1);

    // C++ derivative: temp_i = t2i*ti*gxi*xi²; sum *= -8; sum += t40*gx0 + t41*gx1; *= scale
    let mut dn = t20 * t0 * gx0 * x20 + t21 * t1 * gx1 * x21;
    dn *= -8.0;
    dn += t40 * gx0 + t41 * gx1;
    dn *= SCALE;

    (val, dn)
}

/// 2D simplex noise with analytical derivatives (plan #47 — hash-based, matches C++ simplexnoise2).
/// Returns (value, dvalue/dx, dvalue/dy).
pub fn simplex2_deriv(x: Float, y: Float) -> (Float, Float, Float) {
    simplex2_deriv_seeded(x, y, 0)
}

/// 2D simplex derivative with explicit seed.
pub fn simplex2_deriv_seeded(x: Float, y: Float, seed: i32) -> (Float, Float, Float) {
    let s = (x + y) * F2;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let t = (i + j) as Float * G2;
    let x0 = x - (i as Float - t);
    let y0 = y - (j as Float - t);

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - i1 as Float + G2;
    let y1 = y0 - j1 as Float + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    fn grad2_idx(i: i32, j: i32, seed: i32) -> usize {
        (scramble(i as u32, j as u32, seed as u32) & 7) as usize
    }

    let mut t20 = 0.0_f32;
    let mut t40 = 0.0_f32;
    let mut t21 = 0.0_f32;
    let mut t41 = 0.0_f32;
    let mut t22 = 0.0_f32;
    let mut t42 = 0.0_f32;
    let mut n0 = 0.0_f32;
    let mut n1 = 0.0_f32;
    let mut n2 = 0.0_f32;
    let mut g0: &[Float; 2] = &ZERO2;
    let mut g1: &[Float; 2] = &ZERO2;
    let mut g2: &[Float; 2] = &ZERO2;

    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 >= 0.0 {
        g0 = &GRAD2[grad2_idx(i, j, seed)];
        t20 = t0 * t0;
        t40 = t20 * t20;
        n0 = t40 * (g0[0] * x0 + g0[1] * y0);
    }

    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 >= 0.0 {
        g1 = &GRAD2[grad2_idx(i + i1, j + j1, seed)];
        t21 = t1 * t1;
        t41 = t21 * t21;
        n1 = t41 * (g1[0] * x1 + g1[1] * y1);
    }

    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 >= 0.0 {
        g2 = &GRAD2[grad2_idx(i + 1, j + 1, seed)];
        t22 = t2 * t2;
        t42 = t22 * t22;
        n2 = t42 * (g2[0] * x2 + g2[1] * y2);
    }

    const SCALE: Float = 64.0; // C++ scale
    let val = SCALE * (n0 + n1 + n2);

    let temp0 = t20 * t0 * (g0[0] * x0 + g0[1] * y0);
    let temp1 = t21 * t1 * (g1[0] * x1 + g1[1] * y1);
    let temp2 = t22 * t2 * (g2[0] * x2 + g2[1] * y2);
    let mut dnx = temp0 * x0 + temp1 * x1 + temp2 * x2;
    let mut dny = temp0 * y0 + temp1 * y1 + temp2 * y2;
    dnx *= -8.0;
    dny *= -8.0;
    dnx += t40 * g0[0] + t41 * g1[0] + t42 * g2[0];
    dny += t40 * g0[1] + t41 * g1[1] + t42 * g2[1];
    dnx *= SCALE;
    dny *= SCALE;

    (val, dnx, dny)
}

/// 3D simplex noise with analytical derivatives (plan #47 — hash-based, matches C++ simplexnoise3).
/// Returns (value, gradient).
pub fn simplex3_deriv(p: Vec3) -> (Float, Vec3) {
    simplex3_deriv_seeded(p, 0)
}

/// 3D simplex derivative with explicit seed.
pub fn simplex3_deriv_seeded(p: Vec3, seed: i32) -> (Float, Vec3) {
    let x = p.x;
    let y = p.y;
    let z = p.z;

    let s = (x + y + z) * F3;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let k = fastfloor(z + s);
    let t = (i + j + k) as Float * G3;

    let x0 = x - (i as Float - t);
    let y0 = y - (j as Float - t);
    let z0 = z - (k as Float - t);

    let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
        if y0 >= z0 {
            (1, 0, 0, 1, 1, 0)
        } else if x0 >= z0 {
            (1, 0, 0, 1, 0, 1)
        } else {
            (0, 0, 1, 1, 0, 1)
        }
    } else {
        if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        }
    };

    let x1 = x0 - i1 as Float + G3;
    let y1 = y0 - j1 as Float + G3;
    let z1 = z0 - k1 as Float + G3;
    let x2 = x0 - i2 as Float + 2.0 * G3;
    let y2 = y0 - j2 as Float + 2.0 * G3;
    let z2 = z0 - k2 as Float + 2.0 * G3;
    let x3 = x0 - 1.0 + 3.0 * G3;
    let y3 = y0 - 1.0 + 3.0 * G3;
    let z3 = z0 - 1.0 + 3.0 * G3;

    fn grad3_idx(i: i32, j: i32, k: i32, seed: i32) -> usize {
        let sk = scramble(k as u32, seed as u32, 0);
        (scramble(i as u32, j as u32, sk) & 15) as usize
    }

    let mut t20 = 0.0_f32;
    let mut t40 = 0.0_f32;
    let mut t21 = 0.0_f32;
    let mut t41 = 0.0_f32;
    let mut t22 = 0.0_f32;
    let mut t42 = 0.0_f32;
    let mut t23 = 0.0_f32;
    let mut t43 = 0.0_f32;
    let mut n0 = 0.0_f32;
    let mut n1 = 0.0_f32;
    let mut n2 = 0.0_f32;
    let mut n3 = 0.0_f32;
    let mut g0: &[Float; 3] = &ZERO3;
    let mut g1: &[Float; 3] = &ZERO3;
    let mut g2: &[Float; 3] = &ZERO3;
    let mut g3: &[Float; 3] = &ZERO3;

    let t0 = 0.5 - x0 * x0 - y0 * y0 - z0 * z0;
    if t0 >= 0.0 {
        g0 = &GRAD3[grad3_idx(i, j, k, seed)];
        t20 = t0 * t0;
        t40 = t20 * t20;
        n0 = t40 * (g0[0] * x0 + g0[1] * y0 + g0[2] * z0);
    }

    let t1 = 0.5 - x1 * x1 - y1 * y1 - z1 * z1;
    if t1 >= 0.0 {
        g1 = &GRAD3[grad3_idx(i + i1, j + j1, k + k1, seed)];
        t21 = t1 * t1;
        t41 = t21 * t21;
        n1 = t41 * (g1[0] * x1 + g1[1] * y1 + g1[2] * z1);
    }

    let t2 = 0.5 - x2 * x2 - y2 * y2 - z2 * z2;
    if t2 >= 0.0 {
        g2 = &GRAD3[grad3_idx(i + i2, j + j2, k + k2, seed)];
        t22 = t2 * t2;
        t42 = t22 * t22;
        n2 = t42 * (g2[0] * x2 + g2[1] * y2 + g2[2] * z2);
    }

    let t3 = 0.5 - x3 * x3 - y3 * y3 - z3 * z3;
    if t3 >= 0.0 {
        g3 = &GRAD3[grad3_idx(i + 1, j + 1, k + 1, seed)];
        t23 = t3 * t3;
        t43 = t23 * t23;
        n3 = t43 * (g3[0] * x3 + g3[1] * y3 + g3[2] * z3);
    }

    const SCALE: Float = 68.0; // C++ scale
    let val = SCALE * (n0 + n1 + n2 + n3);

    let temp0 = t20 * t0 * (g0[0] * x0 + g0[1] * y0 + g0[2] * z0);
    let temp1 = t21 * t1 * (g1[0] * x1 + g1[1] * y1 + g1[2] * z1);
    let temp2 = t22 * t2 * (g2[0] * x2 + g2[1] * y2 + g2[2] * z2);
    let temp3 = t23 * t3 * (g3[0] * x3 + g3[1] * y3 + g3[2] * z3);
    let mut dnx = temp0 * x0 + temp1 * x1 + temp2 * x2 + temp3 * x3;
    let mut dny = temp0 * y0 + temp1 * y1 + temp2 * y2 + temp3 * y3;
    let mut dnz = temp0 * z0 + temp1 * z1 + temp2 * z2 + temp3 * z3;
    dnx *= -8.0;
    dny *= -8.0;
    dnz *= -8.0;
    dnx += t40 * g0[0] + t41 * g1[0] + t42 * g2[0] + t43 * g3[0];
    dny += t40 * g0[1] + t41 * g1[1] + t42 * g2[1] + t43 * g3[1];
    dnz += t40 * g0[2] + t41 * g1[2] + t42 * g2[2] + t43 * g3[2];
    dnx *= SCALE;
    dny *= SCALE;
    dnz *= SCALE;

    (val, Vec3::new(dnx, dny, dnz))
}

/// Unsigned simplex noise [0, 1].
pub fn usimplex1(x: Float) -> Float {
    simplex1(x) * 0.5 + 0.5
}
pub fn usimplex2(x: Float, y: Float) -> Float {
    simplex2(x, y) * 0.5 + 0.5
}
pub fn usimplex3(p: Vec3) -> Float {
    simplex3(p) * 0.5 + 0.5
}
pub fn usimplex4(x: Float, y: Float, z: Float, w: Float) -> Float {
    simplex4(x, y, z, w) * 0.5 + 0.5
}

// 4D simplex lookup table
static SIMPLEX_LUT: [[u8; 4]; 64] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 0, 0, 0],
    [0, 2, 3, 1],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 2, 3, 0],
    [0, 2, 1, 3],
    [0, 0, 0, 0],
    [0, 3, 1, 2],
    [0, 3, 2, 1],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 3, 2, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 2, 0, 3],
    [0, 0, 0, 0],
    [1, 3, 0, 2],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [2, 3, 0, 1],
    [2, 3, 1, 0],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [2, 0, 3, 1],
    [0, 0, 0, 0],
    [2, 1, 3, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [2, 0, 1, 3],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [3, 0, 1, 2],
    [3, 0, 2, 1],
    [0, 0, 0, 0],
    [3, 1, 2, 0],
    [2, 1, 0, 3],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [3, 1, 0, 2],
    [0, 0, 0, 0],
    [3, 2, 0, 1],
    [3, 2, 1, 0],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplex1_range() {
        for i in 0..100 {
            let x = i as Float * 0.137;
            let v = simplex1(x);
            assert!(
                (-1.0..=1.0).contains(&v),
                "simplex1({x}) = {v} out of range"
            );
        }
    }

    #[test]
    fn test_simplex2_range() {
        for i in 0..50 {
            for j in 0..50 {
                let x = i as Float * 0.137;
                let y = j as Float * 0.173;
                let v = simplex2(x, y);
                assert!(
                    (-1.0..=1.0).contains(&v),
                    "simplex2({x},{y}) = {v} out of range"
                );
            }
        }
    }

    #[test]
    fn test_simplex3_range() {
        for i in 0..20 {
            for j in 0..20 {
                for k in 0..20 {
                    let p = Vec3::new(i as Float * 0.37, j as Float * 0.41, k as Float * 0.43);
                    let v = simplex3(p);
                    assert!(
                        (-1.5..=1.5).contains(&v),
                        "simplex3({:?}) = {v} out of range",
                        p
                    );
                }
            }
        }
    }

    #[test]
    fn test_simplex4_range() {
        for i in 0..10 {
            for j in 0..10 {
                let v = simplex4(i as Float * 0.7, j as Float * 0.8, 1.0, 2.0);
                assert!((-2.0..=2.0).contains(&v), "simplex4 = {v} out of range");
            }
        }
    }

    #[test]
    fn test_usimplex_range() {
        let v = usimplex1(0.5);
        assert!((0.0..=1.0).contains(&v));
        let v = usimplex2(0.5, 0.7);
        assert!((0.0..=1.0).contains(&v));
        let v = usimplex3(Vec3::new(0.5, 0.7, 0.9));
        assert!((-0.1..=1.1).contains(&v));
    }

    #[test]
    fn test_simplex1_deriv_numerical() {
        let x = 1.7;
        let (val, dval) = simplex1_deriv(x);
        let eps = 1e-4;
        let num = (simplex1(x + eps) - simplex1(x - eps)) / (2.0 * eps);
        assert!(
            (val - simplex1(x)).abs() < 1e-6,
            "value mismatch: {} vs {}",
            val,
            simplex1(x)
        );
        assert!(
            (dval - num).abs() < 0.1,
            "deriv mismatch: analytical {} vs numerical {}",
            dval,
            num
        );
    }

    #[test]
    fn test_simplex2_deriv_numerical() {
        let (x, y) = (1.3, 2.7);
        let (val, dvdx, dvdy) = simplex2_deriv(x, y);
        let eps = 1e-4;
        let num_dx = (simplex2(x + eps, y) - simplex2(x - eps, y)) / (2.0 * eps);
        let num_dy = (simplex2(x, y + eps) - simplex2(x, y - eps)) / (2.0 * eps);
        assert!((val - simplex2(x, y)).abs() < 1e-6);
        assert!((dvdx - num_dx).abs() < 0.2, "dx: {} vs {}", dvdx, num_dx);
        assert!((dvdy - num_dy).abs() < 0.2, "dy: {} vs {}", dvdy, num_dy);
    }

    #[test]
    fn test_simplex3_deriv_numerical() {
        let p = Vec3::new(1.5, 2.5, 3.5);
        let (val, grad) = simplex3_deriv(p);
        let eps = 1e-4;
        let num_dx = (simplex3(Vec3::new(p.x + eps, p.y, p.z))
            - simplex3(Vec3::new(p.x - eps, p.y, p.z)))
            / (2.0 * eps);
        let num_dy = (simplex3(Vec3::new(p.x, p.y + eps, p.z))
            - simplex3(Vec3::new(p.x, p.y - eps, p.z)))
            / (2.0 * eps);
        let num_dz = (simplex3(Vec3::new(p.x, p.y, p.z + eps))
            - simplex3(Vec3::new(p.x, p.y, p.z - eps)))
            / (2.0 * eps);
        assert!((val - simplex3(p)).abs() < 1e-5);
        assert!(
            (grad.x - num_dx).abs() < 0.5,
            "dx: {} vs {}",
            grad.x,
            num_dx
        );
        assert!(
            (grad.y - num_dy).abs() < 0.5,
            "dy: {} vs {}",
            grad.y,
            num_dy
        );
        assert!(
            (grad.z - num_dz).abs() < 0.5,
            "dz: {} vs {}",
            grad.z,
            num_dz
        );
    }

    #[test]
    fn test_simplex_not_constant() {
        let v1 = simplex3(Vec3::new(0.0, 0.0, 0.0));
        let v2 = simplex3(Vec3::new(1.0, 1.0, 1.0));
        let v3 = simplex3(Vec3::new(2.7, 3.1, 4.5));
        // Not all the same
        assert!(!(v1 == v2 && v2 == v3));
    }
}
