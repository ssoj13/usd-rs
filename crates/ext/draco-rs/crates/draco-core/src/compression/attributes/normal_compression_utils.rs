//! Utilities for converting unit vectors to octahedral coordinates and back.
//! Reference: `_ref/draco/src/draco/compression/attributes/normal_compression_utils.h`.

use crate::draco_dcheck;

#[derive(Clone, Copy, Debug)]
pub struct OctahedronToolBox {
    quantization_bits: i32,
    max_quantized_value: i32,
    max_value: i32,
    dequantization_scale: f32,
    center_value: i32,
}

impl OctahedronToolBox {
    pub fn new() -> Self {
        Self {
            quantization_bits: -1,
            max_quantized_value: -1,
            max_value: -1,
            dequantization_scale: 1.0,
            center_value: -1,
        }
    }

    pub fn set_quantization_bits(&mut self, q: i32) -> bool {
        if q < 2 || q > 30 {
            return false;
        }
        self.quantization_bits = q;
        self.max_quantized_value = (1i32 << self.quantization_bits) - 1;
        self.max_value = self.max_quantized_value - 1;
        self.dequantization_scale = 2.0 / self.max_value as f32;
        self.center_value = self.max_value / 2;
        true
    }

    pub fn is_initialized(&self) -> bool {
        self.quantization_bits != -1
    }

    pub fn canonicalize_octahedral_coords(&self, mut s: i32, mut t: i32) -> (i32, i32) {
        if (s == 0 && t == 0) || (s == 0 && t == self.max_value) || (s == self.max_value && t == 0)
        {
            s = self.max_value;
            t = self.max_value;
        } else if s == 0 && t > self.center_value {
            t = self.center_value - (t - self.center_value);
        } else if s == self.max_value && t < self.center_value {
            t = self.center_value + (self.center_value - t);
        } else if t == self.max_value && s < self.center_value {
            s = self.center_value + (self.center_value - s);
        } else if t == 0 && s > self.center_value {
            s = self.center_value - (s - self.center_value);
        }
        (s, t)
    }

    pub fn integer_vector_to_quantized_octahedral_coords(
        &self,
        int_vec: &[i32; 3],
        out_s: &mut i32,
        out_t: &mut i32,
    ) {
        draco_dcheck!(int_vec[0].abs() + int_vec[1].abs() + int_vec[2].abs() == self.center_value);
        let (s, t) = if int_vec[0] >= 0 {
            (
                int_vec[1] + self.center_value,
                int_vec[2] + self.center_value,
            )
        } else {
            let s = if int_vec[1] < 0 {
                int_vec[2].abs()
            } else {
                self.max_value - int_vec[2].abs()
            };
            let t = if int_vec[2] < 0 {
                int_vec[1].abs()
            } else {
                self.max_value - int_vec[1].abs()
            };
            (s, t)
        };
        let (cs, ct) = self.canonicalize_octahedral_coords(s, t);
        *out_s = cs;
        *out_t = ct;
    }

    pub fn float_vector_to_quantized_octahedral_coords(
        &self,
        vector: &[f32; 3],
        out_s: &mut i32,
        out_t: &mut i32,
    ) {
        let abs_sum = vector[0].abs() as f64 + vector[1].abs() as f64 + vector[2].abs() as f64;
        let mut scaled = [0.0f64; 3];
        if abs_sum > 1e-6 {
            let scale = 1.0 / abs_sum;
            scaled[0] = vector[0] as f64 * scale;
            scaled[1] = vector[1] as f64 * scale;
            scaled[2] = vector[2] as f64 * scale;
        } else {
            scaled[0] = 1.0;
            scaled[1] = 0.0;
            scaled[2] = 0.0;
        }

        let mut int_vec = [0i32; 3];
        int_vec[0] = (scaled[0] * (self.center_value as f64) + 0.5).floor() as i32;
        int_vec[1] = (scaled[1] * (self.center_value as f64) + 0.5).floor() as i32;
        int_vec[2] = self.center_value - int_vec[0].abs() - int_vec[1].abs();
        if int_vec[2] < 0 {
            if int_vec[1] > 0 {
                int_vec[1] += int_vec[2];
            } else {
                int_vec[1] -= int_vec[2];
            }
            int_vec[2] = 0;
        }
        if scaled[2] < 0.0 {
            int_vec[2] *= -1;
        }
        self.integer_vector_to_quantized_octahedral_coords(&int_vec, out_s, out_t);
    }

    pub fn canonicalize_integer_vector(&self, vec: &mut [i32; 3]) {
        let abs_sum = vec[0].abs() as i64 + vec[1].abs() as i64 + vec[2].abs() as i64;
        if abs_sum == 0 {
            vec[0] = self.center_value;
            vec[1] = 0;
            vec[2] = 0;
        } else {
            vec[0] = ((vec[0] as i64 * self.center_value as i64) / abs_sum) as i32;
            vec[1] = ((vec[1] as i64 * self.center_value as i64) / abs_sum) as i32;
            if vec[2] >= 0 {
                vec[2] = self.center_value - vec[0].abs() - vec[1].abs();
            } else {
                vec[2] = -(self.center_value - vec[0].abs() - vec[1].abs());
            }
        }
    }

    pub fn quantized_octahedral_coords_to_unit_vector(
        &self,
        in_s: i32,
        in_t: i32,
        out_vector: &mut [f32; 3],
    ) {
        let s_scaled = in_s as f32 * self.dequantization_scale - 1.0;
        let t_scaled = in_t as f32 * self.dequantization_scale - 1.0;
        self.octahedral_coords_to_unit_vector(s_scaled, t_scaled, out_vector);
    }

    pub fn is_in_diamond(&self, s: i32, t: i32) -> bool {
        draco_dcheck!(s <= self.center_value);
        draco_dcheck!(t <= self.center_value);
        draco_dcheck!(s >= -self.center_value);
        draco_dcheck!(t >= -self.center_value);
        let st = (s.abs() as u32) + (t.abs() as u32);
        st <= self.center_value as u32
    }

    pub fn invert_diamond(&self, s: &mut i32, t: &mut i32) {
        draco_dcheck!(*s <= self.center_value);
        draco_dcheck!(*t <= self.center_value);
        draco_dcheck!(*s >= -self.center_value);
        draco_dcheck!(*t >= -self.center_value);
        let (sign_s, sign_t) = if *s >= 0 && *t >= 0 {
            (1, 1)
        } else if *s <= 0 && *t <= 0 {
            (-1, -1)
        } else {
            (if *s > 0 { 1 } else { -1 }, if *t > 0 { 1 } else { -1 })
        };

        let corner_point_s = (sign_s * self.center_value) as u32;
        let corner_point_t = (sign_t * self.center_value) as u32;
        let mut us = *s as u32;
        let mut ut = *t as u32;
        us = us.wrapping_add(us).wrapping_sub(corner_point_s);
        ut = ut.wrapping_add(ut).wrapping_sub(corner_point_t);
        if sign_s * sign_t >= 0 {
            let temp = us;
            us = ut.wrapping_neg();
            ut = temp.wrapping_neg();
        } else {
            std::mem::swap(&mut us, &mut ut);
        }
        us = us.wrapping_add(corner_point_s);
        ut = ut.wrapping_add(corner_point_t);

        *s = us as i32;
        *t = ut as i32;
        *s /= 2;
        *t /= 2;
    }

    pub fn invert_direction(&self, s: &mut i32, t: &mut i32) {
        draco_dcheck!(*s <= self.center_value);
        draco_dcheck!(*t <= self.center_value);
        draco_dcheck!(*s >= -self.center_value);
        draco_dcheck!(*t >= -self.center_value);
        *s *= -1;
        *t *= -1;
        self.invert_diamond(s, t);
    }

    pub fn mod_max(&self, x: i32) -> i32 {
        if x > self.center_value {
            return x - self.max_quantized_value;
        }
        if x < -self.center_value {
            return x + self.max_quantized_value;
        }
        x
    }

    pub fn make_positive(&self, x: i32) -> i32 {
        draco_dcheck!(x <= self.center_value * 2);
        if x < 0 {
            return x + self.max_quantized_value;
        }
        x
    }

    pub fn quantization_bits(&self) -> i32 {
        self.quantization_bits
    }

    pub fn max_quantized_value(&self) -> i32 {
        self.max_quantized_value
    }

    pub fn max_value(&self) -> i32 {
        self.max_value
    }

    pub fn center_value(&self) -> i32 {
        self.center_value
    }

    fn octahedral_coords_to_unit_vector(
        &self,
        in_s_scaled: f32,
        in_t_scaled: f32,
        out_vector: &mut [f32; 3],
    ) {
        let mut y = in_s_scaled;
        let mut z = in_t_scaled;
        let x = 1.0 - y.abs() - z.abs();

        let mut x_offset = -x;
        if x_offset < 0.0 {
            x_offset = 0.0;
        }

        y += if y < 0.0 { x_offset } else { -x_offset };
        z += if z < 0.0 { x_offset } else { -x_offset };

        let norm_squared = x * x + y * y + z * z;
        if norm_squared < 1e-6 {
            out_vector[0] = 0.0;
            out_vector[1] = 0.0;
            out_vector[2] = 0.0;
        } else {
            let d = 1.0 / norm_squared.sqrt();
            out_vector[0] = x * d;
            out_vector[1] = y * d;
            out_vector[2] = z * d;
        }
    }
}

impl Default for OctahedronToolBox {
    fn default() -> Self {
        Self::new()
    }
}
