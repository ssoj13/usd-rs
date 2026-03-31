/// Patch parametrization data, mirroring Far::PatchParam.
///
/// Two packed integer fields encode face id, UV offsets, depth, boundary
/// and regularity information -- identical bit layout to C++ Far::PatchParam.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PatchParam {
    /// field0 bits: [0..27] face_id, [28..31] transition mask
    pub field0: i32,
    /// field1 bits: [0..3] depth, [4] non_quad_root, [5] is_regular,
    ///              [7..11] boundary, [12..21] v_offset, [22..31] u_offset
    pub field1: i32,
}

impl PatchParam {
    pub fn new(field0: i32, field1: i32) -> Self {
        Self { field0, field1 }
    }

    /// Pack all fields into a PatchParam. Mirrors C++ PatchParam::Set().
    pub fn set(
        &mut self,
        face_id: i32,
        u: i16,
        v: i16,
        depth: u16,
        non_quad_root: bool,
        boundary: u16,
        transition: u16,
        is_regular: bool,
    ) {
        self.field0 = (face_id & 0x0fff_ffff) | ((transition as i32 & 0xf) << 28);

        self.field1 = (depth as i32 & 0xf)
            | ((non_quad_root as i32) << 4)
            | ((is_regular as i32) << 5)
            | ((boundary as i32 & 0x1f) << 7)
            | (((v as i32) & 0x3ff) << 12)
            | (((u as i32) & 0x3ff) << 22);
    }

    /// Return packed bits as u64 (field0 low, field1 high). Used by osd interop.
    #[inline]
    pub fn bits(&self) -> u64 {
        (self.field0 as u32 as u64) | ((self.field1 as u32 as u64) << 32)
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

    pub fn is_regular(&self) -> bool {
        ((self.field1 >> 5) & 0x1) != 0
    }

    /// Return the parametric fraction 1/(2^(depth - non_quad_root)).
    ///
    /// The guard `depth <= non_quad` handles malformed inputs (invalid packed
    /// bitfields) defensively by returning 1.0.  C++ omits this guard and
    /// assumes `depth > non_quad_root` always holds for valid params, but the
    /// guard has no effect on correctly-formed params and prevents a shift
    /// overflow on corrupt data.
    pub fn get_param_fraction(&self) -> f32 {
        let depth = self.get_depth();
        let non_quad = self.get_non_quad_root();
        if depth <= non_quad {
            return 1.0;
        }
        1.0_f32 / (1 << (depth - non_quad)) as f32
    }

    /// Returns true if this patch is at the non-quad root level.
    #[inline]
    pub fn non_quad_root(&self) -> bool {
        self.get_non_quad_root() != 0
    }

    /// Normalize (s,t) from coarse to patch-local [0,1] coordinates.
    pub fn normalize(&self, s: &mut f32, t: &mut f32) {
        let frac_inv = 1.0 / self.get_param_fraction();
        *s = *s * frac_inv - self.get_u() as f32;
        *t = *t * frac_inv - self.get_v() as f32;
    }

    /// Normalize (s,t) in f64.
    pub fn normalize_f64(&self, s: &mut f64, t: &mut f64) {
        let frac_inv = 1.0 / self.get_param_fraction() as f64;
        *s = *s * frac_inv - self.get_u() as f64;
        *t = *t * frac_inv - self.get_v() as f64;
    }

    /// Returns true if a triangular patch is parametrically rotated 180 degrees.
    /// Mirrors C++ `PatchParam::IsTriangleRotated`.
    #[inline]
    pub fn is_triangle_rotated(&self) -> bool {
        (self.get_u() + self.get_v()) >= (1 << self.get_depth())
    }

    /// Reset all fields to zero.
    #[inline]
    pub fn clear(&mut self) {
        self.field0 = 0;
        self.field1 = 0;
    }

    /// Unnormalize (s,t) from patch-local [0,1] to coarse parametric coords.
    /// Mirrors C++ `PatchParam::Unnormalize`.
    pub fn unnormalize(&self, s: &mut f64, t: &mut f64) {
        let frac = self.get_param_fraction() as f64;
        *s = (*s + self.get_u() as f64) * frac;
        *t = (*t + self.get_v() as f64) * frac;
    }

    /// Unnormalize (s,t) from patch-local [0,1] to coarse parametric coords (f32).
    pub fn unnormalize_f32(&self, s: &mut f32, t: &mut f32) {
        let frac = self.get_param_fraction();
        *s = (*s + self.get_u() as f32) * frac;
        *t = (*t + self.get_v() as f32) * frac;
    }

    /// Normalize (s,t) for triangular patches, accounting for rotation.
    /// Mirrors C++ `PatchParam::NormalizeTriangle`.
    pub fn normalize_triangle(&self, s: &mut f64, t: &mut f64) {
        if self.is_triangle_rotated() {
            let frac_inv = 1.0 / self.get_param_fraction() as f64;
            let depth_factor = (1 << self.get_depth()) as f64;
            *s = (depth_factor - self.get_u() as f64) - (*s * frac_inv);
            *t = (depth_factor - self.get_v() as f64) - (*t * frac_inv);
        } else {
            self.normalize_f64(s, t);
        }
    }

    /// Unnormalize (s,t) from patch-local to triangle barycentric coords.
    /// Mirrors C++ `PatchParam::UnnormalizeTriangle` — respects rotation.
    pub fn unnormalize_triangle(&self, s: &mut f64, t: &mut f64) {
        if self.is_triangle_rotated() {
            let frac = self.get_param_fraction() as f64;
            let depth_factor = (1 << self.get_depth()) as f64;
            *s = (depth_factor - self.get_u() as f64 - *s) * frac;
            *t = (depth_factor - self.get_v() as f64 - *t) * frac;
        } else {
            self.unnormalize(s, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_roundtrip() {
        let mut pp = PatchParam::default();
        pp.set(42, 3, 5, 2, true, 7, 0xA, true);
        assert_eq!(pp.get_face_id(), 42);
        assert_eq!(pp.get_u(), 3);
        assert_eq!(pp.get_v(), 5);
        assert_eq!(pp.get_depth(), 2);
        assert!(pp.non_quad_root());
        assert_eq!(pp.get_boundary(), 7);
        assert_eq!(pp.get_transition(), 0xA);
        assert!(pp.is_regular());
    }

    #[test]
    fn bits_packing() {
        let pp = PatchParam::new(0x1234, 0x5678);
        let b = pp.bits();
        assert_eq!(b & 0xFFFF_FFFF, 0x1234);
        assert_eq!((b >> 32) & 0xFFFF_FFFF, 0x5678);
    }

    #[test]
    fn clear() {
        let mut pp = PatchParam::new(0x1234, 0x5678);
        pp.clear();
        assert_eq!(pp.field0, 0);
        assert_eq!(pp.field1, 0);
    }

    #[test]
    fn is_triangle_rotated() {
        let mut pp = PatchParam::default();
        // depth=2, u=3, v=2 => u+v=5 >= 1<<2=4 => rotated
        pp.set(0, 3, 2, 2, false, 0, 0, false);
        assert!(pp.is_triangle_rotated());

        // depth=2, u=0, v=1 => u+v=1 < 4 => not rotated
        pp.set(0, 0, 1, 2, false, 0, 0, false);
        assert!(!pp.is_triangle_rotated());
    }

    #[test]
    fn unnormalize_roundtrip() {
        let mut pp = PatchParam::default();
        pp.set(0, 1, 2, 3, false, 0, 0, false);
        let mut s = 0.5_f64;
        let mut t = 0.5_f64;
        pp.normalize_f64(&mut s, &mut t);
        pp.unnormalize(&mut s, &mut t);
        assert!((s - 0.5).abs() < 1e-10);
        assert!((t - 0.5).abs() < 1e-10);
    }

    #[test]
    fn unnormalize_triangle_rotated() {
        let mut pp = PatchParam::default();
        // depth=2, u=3, v=2 => rotated
        pp.set(0, 3, 2, 2, false, 0, 0, false);
        assert!(pp.is_triangle_rotated());
        let mut s = 0.25;
        let mut t = 0.25;
        // NormalizeTriangle then UnnormalizeTriangle should roundtrip
        pp.normalize_triangle(&mut s, &mut t);
        pp.unnormalize_triangle(&mut s, &mut t);
        assert!((s - 0.25).abs() < 1e-10);
        assert!((t - 0.25).abs() < 1e-10);
    }

    #[test]
    fn unnormalize_triangle_not_rotated() {
        let mut pp = PatchParam::default();
        // depth=2, u=0, v=1 => not rotated
        pp.set(0, 0, 1, 2, false, 0, 0, false);
        assert!(!pp.is_triangle_rotated());
        let mut s = 0.25;
        let mut t = 0.25;
        pp.normalize_triangle(&mut s, &mut t);
        pp.unnormalize_triangle(&mut s, &mut t);
        assert!((s - 0.25).abs() < 1e-10);
        assert!((t - 0.25).abs() < 1e-10);
    }
}
