/// Osd-level patch param helpers — mirrors OsdPatchParam* functions in
/// patchBasisTypes.h.  All operate on the raw (field0, field1, sharpness)
/// representation so they work without a heap allocation.

/// Patch type constants (mirrors `Far::PatchDescriptor::Type` enum values).
/// These match the C++ enum: NON_PATCH=0, POINTS=1, LINES=2, QUADS=3,
/// TRIANGLES=4, LOOP=5, REGULAR=6, GREGORY=7, GREGORY_BOUNDARY=8,
/// GREGORY_BASIS=9, GREGORY_TRIANGLE=10.
pub mod patch_type {
    pub const QUADS: i32 = 3;
    pub const TRIANGLES: i32 = 4;
    pub const LOOP: i32 = 5;
    pub const REGULAR: i32 = 6;
    pub const GREGORY: i32 = 7;
    pub const GREGORY_BOUNDARY: i32 = 8;
    pub const GREGORY_BASIS: i32 = 9;
    pub const GREGORY_TRIANGLE: i32 = 10;
}

/// Packed patch param used by basis evaluation — mirrors OsdPatchParam.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsdPatchParam {
    pub field0: i32,
    pub field1: i32,
    pub sharpness: f32,
}

impl OsdPatchParam {
    pub fn new(field0: i32, field1: i32, sharpness: f32) -> Self {
        Self {
            field0,
            field1,
            sharpness,
        }
    }

    pub fn get_face_id(&self) -> i32 {
        self.field0 & 0xfffffff
    }

    pub fn get_u(&self) -> i32 {
        (self.field1 >> 22) & 0x3ff
    }

    pub fn get_v(&self) -> i32 {
        (self.field1 >> 12) & 0x3ff
    }

    pub fn get_transition(&self) -> i32 {
        (self.field0 >> 28) & 0xf
    }

    pub fn get_boundary(&self) -> i32 {
        (self.field1 >> 7) & 0x1f
    }

    pub fn get_non_quad_root(&self) -> i32 {
        (self.field1 >> 4) & 0x1
    }

    pub fn get_depth(&self) -> i32 {
        self.field1 & 0xf
    }

    pub fn get_param_fraction(&self) -> f32 {
        let d = self.get_depth() - self.get_non_quad_root();
        1.0_f32 / (1 << d) as f32
    }

    pub fn is_regular(&self) -> bool {
        ((self.field1 >> 5) & 0x1) != 0
    }

    pub fn is_triangle_rotated(&self) -> bool {
        (self.get_u() + self.get_v()) >= (1 << self.get_depth())
    }

    /// Transform (s, t) from coarse patch space into normalised [0,1] quad coords.
    pub fn normalize(&self, s: f32, t: f32) -> (f32, f32) {
        let frac_inv = 1.0 / self.get_param_fraction();
        (
            s * frac_inv - self.get_u() as f32,
            t * frac_inv - self.get_v() as f32,
        )
    }

    /// Inverse of normalize.
    pub fn unnormalize(&self, s: f32, t: f32) -> (f32, f32) {
        let frac = self.get_param_fraction();
        (
            (s + self.get_u() as f32) * frac,
            (t + self.get_v() as f32) * frac,
        )
    }

    /// Normalize for a triangular patch (handles rotated case).
    pub fn normalize_triangle(&self, s: f32, t: f32) -> (f32, f32) {
        if self.is_triangle_rotated() {
            let frac_inv = 1.0 / self.get_param_fraction();
            let depth_factor = (1 << self.get_depth()) as f32;
            (
                (depth_factor - self.get_u() as f32) - s * frac_inv,
                (depth_factor - self.get_v() as f32) - t * frac_inv,
            )
        } else {
            self.normalize(s, t)
        }
    }

    /// Unnormalize for a triangular patch (handles rotated case).
    pub fn unnormalize_triangle(&self, s: f32, t: f32) -> (f32, f32) {
        if self.is_triangle_rotated() {
            let frac = self.get_param_fraction();
            let depth_factor = (1 << self.get_depth()) as f32;
            (
                (depth_factor - self.get_u() as f32 - s) * frac,
                (depth_factor - self.get_v() as f32 - t) * frac,
            )
        } else {
            self.unnormalize(s, t)
        }
    }
}
