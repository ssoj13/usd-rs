//! 3D rotation representation using axis-angle.
//!
//! [`Rotation`] represents a rotation in 3-space as a normalized axis vector
//! and an angle in degrees. Rotations follow the right-hand rule: a positive
//! rotation about an axis appears counter-clockwise when looking from the
//! end of the vector toward the origin.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Rotation, vec3d};
//! use std::f64::consts::PI;
//!
//! // 90 degrees around Z axis
//! let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
//!
//! // Transform a vector
//! let v = vec3d(1.0, 0.0, 0.0);
//! let result = rot.transform_dir(&v);
//! assert!((result.y - 1.0).abs() < 1e-10);
//! ```

use crate::limits::MIN_VECTOR_LENGTH;
use crate::math::{degrees_to_radians, is_close, radians_to_degrees};
use crate::matrix3::Matrix3d;
use crate::matrix4::Matrix4d;
use crate::quat::Quatd;
use crate::vec3::Vec3d;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Div, DivAssign, Mul, MulAssign};

/// A 3D rotation represented as axis-angle.
///
/// Stores a normalized axis vector and angle in degrees.
/// The rotation follows the right-hand rule.
///
/// # Examples
///
/// ```
/// use usd_gf::{Rotation, vec3d};
///
/// // Rotation from one vector to another
/// let from = vec3d(1.0, 0.0, 0.0);
/// let to = vec3d(0.0, 1.0, 0.0);
/// let rot = Rotation::from_rotate_into(&from, &to);
/// assert!((rot.angle() - 90.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Rotation {
    /// Normalized axis of rotation.
    axis: Vec3d,
    /// Angle in degrees.
    angle: f64,
}

impl Default for Rotation {
    /// Creates an identity rotation (0 degrees around X axis).
    fn default() -> Self {
        Self {
            axis: Vec3d::new(1.0, 0.0, 0.0),
            angle: 0.0,
        }
    }
}

impl Rotation {
    /// Creates an identity rotation.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a rotation of `angle` degrees about `axis`.
    ///
    /// The axis is normalized automatically.
    #[must_use]
    pub fn from_axis_angle(axis: Vec3d, angle: f64) -> Self {
        let mut rot = Self { axis, angle };
        // Normalize axis if not already unit length
        if !is_close(rot.axis.dot(&rot.axis), 1.0, 1e-10) {
            rot.axis = rot.axis.normalized();
        }
        rot
    }

    /// Creates a rotation from a quaternion.
    #[must_use]
    pub fn from_quat(quat: &Quatd) -> Self {
        let mut rot = Self::default();
        rot.set_quat(quat);
        rot
    }

    /// Creates a rotation from a Quaternion (legacy type).
    ///
    /// Converts the Quaternion to a Quatd and delegates.
    /// Matches C++ `GfRotation(GfQuaternion const &)`.
    #[must_use]
    pub fn from_quaternion(quat: &crate::quaternion::Quaternion) -> Self {
        let q = Quatd::new(
            quat.real(),
            crate::vec3::Vec3d::new(quat.imaginary().x, quat.imaginary().y, quat.imaginary().z),
        );
        Self::from_quat(&q)
    }

    /// Creates a rotation that brings `from` vector to align with `to`.
    ///
    /// The vectors need not be unit length.
    #[must_use]
    pub fn from_rotate_into(from: &Vec3d, to: &Vec3d) -> Self {
        let mut rot = Self::default();
        rot.set_rotate_into(from, to);
        rot
    }

    /// Sets the rotation to `angle` degrees about `axis`.
    pub fn set_axis_angle(&mut self, axis: Vec3d, angle: f64) -> &mut Self {
        self.axis = axis;
        self.angle = angle;
        if !is_close(self.axis.dot(&self.axis), 1.0, 1e-10) {
            self.axis = self.axis.normalized();
        }
        self
    }

    /// Sets the rotation from a quaternion.
    pub fn set_quat(&mut self, quat: &Quatd) -> &mut Self {
        let len = quat.imaginary().length();
        if len > MIN_VECTOR_LENGTH {
            let x = quat.real().clamp(-1.0, 1.0).acos();
            self.set_axis_angle(*quat.imaginary() / len, 2.0 * radians_to_degrees(x));
        } else {
            self.set_identity();
        }
        self
    }

    /// Sets the rotation to bring `from` to align with `to`.
    pub fn set_rotate_into(&mut self, from: &Vec3d, to: &Vec3d) -> &mut Self {
        let from_n = from.normalized();
        let to_n = to.normalized();

        let cos = from_n.dot(&to_n);

        // Vectors are parallel
        if cos > 0.9999999 {
            return self.set_identity();
        }

        // Vectors are opposite: rotate 180 degrees around perpendicular axis
        if cos < -0.9999999 {
            // Try cross with X, if too small use Y
            let mut tmp = from_n.cross(&Vec3d::new(1.0, 0.0, 0.0));
            if tmp.length() < 0.00001 {
                tmp = from_n.cross(&Vec3d::new(0.0, 1.0, 0.0));
            }
            return self.set_axis_angle(tmp.normalized(), 180.0);
        }

        // General case
        let axis = from.cross(to).normalized();
        self.set_axis_angle(axis, radians_to_degrees(cos.acos()))
    }

    /// Sets to identity rotation (0 degrees around X axis).
    pub fn set_identity(&mut self) -> &mut Self {
        self.axis = Vec3d::new(1.0, 0.0, 0.0);
        self.angle = 0.0;
        self
    }

    /// Returns the axis of rotation (normalized).
    #[inline]
    #[must_use]
    pub fn axis(&self) -> Vec3d {
        self.axis
    }

    /// Returns the angle in degrees.
    #[inline]
    #[must_use]
    pub fn angle(&self) -> f64 {
        self.angle
    }

    /// Returns the rotation as a quaternion.
    #[must_use]
    pub fn get_quat(&self) -> Quatd {
        let radians = degrees_to_radians(self.angle) / 2.0;
        let (sin_r, cos_r) = radians.sin_cos();
        let axis = self.axis * sin_r;
        Quatd::new(cos_r, axis).normalized()
    }

    /// Returns the inverse rotation (negated angle).
    #[inline]
    #[must_use]
    pub fn inverse(&self) -> Self {
        Self {
            axis: self.axis,
            angle: -self.angle,
        }
    }

    /// Transforms a direction vector by this rotation.
    #[must_use]
    pub fn transform_dir(&self, vec: &Vec3d) -> Vec3d {
        let mat = Matrix4d::from_rotation(self.axis, degrees_to_radians(self.angle));
        mat.transform_dir(vec)
    }

    /// Decomposes the rotation about 3 orthogonal axes.
    ///
    /// Returns (angle0, angle1, angle2) in degrees.
    /// Warns if axes are not orthogonal.
    #[must_use]
    pub fn decompose(&self, axis0: &Vec3d, axis1: &Vec3d, axis2: &Vec3d) -> Vec3d {
        let mat = Matrix4d::from_rotation(self.axis, degrees_to_radians(self.angle));

        // Normalize axes
        let n_axis0 = axis0.normalized();
        let n_axis1 = axis1.normalized();
        let n_axis2 = axis2.normalized();

        // Build axes matrix
        let axes = Matrix4d::new(
            n_axis0.x, n_axis1.x, n_axis2.x, 0.0, n_axis0.y, n_axis1.y, n_axis2.y, 0.0, n_axis0.z,
            n_axis1.z, n_axis2.z, 0.0, 0.0, 0.0, 0.0, 1.0,
        );

        // Transform to axis-aligned space
        let m = axes.transpose() * mat * axes;

        // Decompose using Graphics Gems IV algorithm (Ken Shoemake)
        let (i, j, k) = (0, 1, 2);
        let cy = (m[i][i] * m[i][i] + m[j][i] * m[j][i]).sqrt();

        let (r0, r1, r2);
        const EPSILON: f64 = 1e-6;
        if cy > EPSILON {
            r0 = m[k][j].atan2(m[k][k]);
            r1 = (-m[k][i]).atan2(cy);
            r2 = m[j][i].atan2(m[i][i]);
        } else {
            r0 = (-m[j][k]).atan2(m[j][j]);
            r1 = (-m[k][i]).atan2(cy);
            r2 = 0.0;
        }

        // Check handedness
        let axis_cross = n_axis0.cross(&n_axis1);
        let axis_hand = axis_cross.dot(&n_axis2);

        let (r0, r1, r2) = if axis_hand >= 0.0 {
            (-r0, -r1, -r2)
        } else {
            (r0, r1, r2)
        };

        Vec3d::new(
            radians_to_degrees(r0),
            radians_to_degrees(r1),
            radians_to_degrees(r2),
        )
    }

    /// Returns this rotation as a 3×3 matrix.
    ///
    /// C++ parity: `GfMatrix3d GfRotation::GetMatrix3() const`
    /// (implicit via `GfMatrix3d::SetRotate(*this)`).
    #[must_use]
    pub fn get_matrix3(&self) -> Matrix3d {
        let q = self.get_quat();
        let mut m = Matrix3d::identity();
        m.set_rotate_quat(&q);
        m
    }

    /// Returns this rotation as a 4×4 matrix (translation zeroed).
    ///
    /// C++ parity: `GfMatrix4d::SetRotate(const GfRotation &)` / implicit conversion.
    #[must_use]
    pub fn get_matrix4(&self) -> Matrix4d {
        let q = self.get_quat();
        let mut m = Matrix4d::identity();
        m.set_rotate(&q);
        m
    }

    /// Sets this rotation by extracting it from the upper-left 3×3 of `mat`.
    ///
    /// C++ parity: `GfRotation::SetMatrix(const GfMatrix3d &)`
    /// (used internally in ExtractRotation methods).
    pub fn set_matrix(&mut self, mat: &Matrix3d) -> &mut Self {
        let rot = mat.extract_rotation();
        self.axis = rot.axis;
        self.angle = rot.angle;
        self
    }

    // -----------------------------------------------------------------------
    // Internal helpers matching C++ static helpers
    // -----------------------------------------------------------------------

    /// Shifts `angle` to be within PI of `hint` (C++ `_PiShift` per-component).
    #[inline]
    pub(crate) fn pi_shift_angle(angle: f64, hint: f64) -> f64 {
        use std::f64::consts::PI;
        let two_pi = 2.0 * PI;
        let mut a = angle;
        while a > hint + PI {
            a -= two_pi;
        }
        while a < hint - PI {
            a += two_pi;
        }
        a
    }

    /// Adjusts gimbal-locked first/last angles when middle angle collapses axes.
    ///
    /// C++ parity: `static void _ShiftGimbalLock(...)`.
    fn shift_gimbal_lock(middle_angle: f64, first: &mut f64, last: &mut f64) {
        const EPSILON: f64 = 1e-6;
        use std::f64::consts::PI;
        // Middle = ±PI: axes flipped, use difference
        if (middle_angle.abs() - PI).abs() < EPSILON {
            let diff = *last - *first;
            *last = diff / 2.0;
            *first = -diff / 2.0;
        }
        // Middle = 0: axes identical, use sum
        if middle_angle.abs() < EPSILON {
            let sum = *last + *first;
            *last = sum / 2.0;
            *first = sum / 2.0;
        }
    }

    /// Projects v1, v2 onto the plane normal to `axis` and returns the rotation
    /// (and angle in radians) that brings the projected v1 onto projected v2.
    ///
    /// C++ parity: `static GfMatrix4d _RotateOntoProjected(..., double *thetaInRadians)`.
    fn rotate_onto_projected_mat(
        v1: &Vec3d,
        v2: &Vec3d,
        axis: &Vec3d,
        theta_out: Option<&mut f64>,
    ) -> Matrix4d {
        let r = Self::rotate_onto_projected(v1, v2, axis);
        let angle_rad = degrees_to_radians(r.angle());
        if let Some(t) = theta_out {
            *t = angle_rad;
        }
        r.get_matrix4()
    }

    /// Decomposes a rotation matrix into Euler angles (Cardanian angles).
    ///
    /// Axes must be normalized. Pass `None` for any angle to omit it (exactly
    /// one `None` is allowed; omitting two or more angles is an error and the
    /// function returns without writing anything).
    ///
    /// If `use_hint` is true, current values in `theta_*` are used as hints and
    /// the result is the closest equivalent rotation to those hints.
    ///
    /// `handedness` should be 1.0 or -1.0.
    ///
    /// All angles are in **radians**.
    ///
    /// C++ parity: `GfRotation::DecomposeRotation(const GfMatrix4d &, ...)` — exact port.
    #[allow(clippy::too_many_arguments)]
    pub fn decompose_rotation(
        rot: &Matrix4d,
        tw_axis: &Vec3d,
        fb_axis: &Vec3d,
        lr_axis: &Vec3d,
        handedness: f64,
        theta_tw: Option<&mut f64>,
        theta_fb: Option<&mut f64>,
        theta_lr: Option<&mut f64>,
        theta_sw: Option<&mut f64>,
        use_hint: bool,
        sw_shift: Option<f64>,
    ) {
        use std::f64::consts::PI;

        // Which angle slot is being zeroed out (None means omit)
        #[derive(Clone, Copy, PartialEq)]
        enum ZeroAngle {
            None,
            Tw,
            Fb,
            Lr,
            Sw,
        }

        // Standin storage when the caller passes None for an angle
        let mut standin_tw = 0.0_f64;
        let mut standin_fb = 0.0_f64;
        let mut standin_lr = 0.0_f64;
        let mut standin_sw = 0.0_f64;

        let mut zero = ZeroAngle::None;
        let mut num_nones = 0usize;

        let tw_is_none = theta_tw.is_none();
        let fb_is_none = theta_fb.is_none();
        let lr_is_none = theta_lr.is_none();
        let sw_is_none = theta_sw.is_none();

        if tw_is_none {
            zero = ZeroAngle::Tw;
            num_nones += 1;
        }
        if fb_is_none {
            zero = ZeroAngle::Fb;
            num_nones += 1;
        }
        if lr_is_none {
            zero = ZeroAngle::Lr;
            num_nones += 1;
        }
        if sw_is_none {
            zero = ZeroAngle::Sw;
            num_nones += 1;
        }

        // Need at least three angles
        if num_nones > 1 {
            // C++ TF_CODING_ERROR — we just return silently in Rust
            return;
        }

        if sw_shift.is_some() && zero != ZeroAngle::None {
            // C++ TF_WARN about ignored sw_shift — silently ignored in Rust
        }

        // Reborrow or use standins
        let tw: &mut f64 = theta_tw.unwrap_or(&mut standin_tw);
        let fb: &mut f64 = theta_fb.unwrap_or(&mut standin_fb);
        let lr: &mut f64 = theta_lr.unwrap_or(&mut standin_lr);
        let sw: &mut f64 = theta_sw.unwrap_or(&mut standin_sw);

        // Save hints before we overwrite
        let (hint_tw, hint_fb, hint_lr, hint_sw) = if use_hint {
            (*tw, *fb, *lr, *sw)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Apply the matrix to the axes
        let fb_axis_r = rot.transform_dir(fb_axis);
        let tw_axis_r = rot.transform_dir(tw_axis);

        // Iteratively undo the rotation by rotating transformed axes back.
        // Angles are negatives of the Euler angles.
        let mut r = Matrix4d::identity();

        match zero {
            ZeroAngle::None | ZeroAngle::Sw => {
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    tw_axis,
                    lr_axis,
                    Some(lr),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    tw_axis,
                    fb_axis,
                    Some(fb),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&fb_axis_r),
                    fb_axis,
                    tw_axis,
                    Some(tw),
                );
                let _ = r * mat; // r fully consumed; result not needed
                *fb *= -handedness;
                *lr *= -handedness;
                *tw *= -handedness;
                *sw = sw_shift.unwrap_or(0.0);
            }
            ZeroAngle::Tw => {
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&fb_axis_r),
                    fb_axis,
                    tw_axis,
                    Some(sw),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&fb_axis_r),
                    fb_axis,
                    lr_axis,
                    Some(lr),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    tw_axis,
                    fb_axis,
                    Some(fb),
                );
                let _ = r * mat;
                *sw *= -handedness;
                *fb *= -handedness;
                *lr *= -handedness;
            }
            ZeroAngle::Fb => {
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    fb_axis,
                    tw_axis,
                    Some(sw),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    tw_axis,
                    lr_axis,
                    Some(lr),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&fb_axis_r),
                    fb_axis,
                    tw_axis,
                    Some(tw),
                );
                let _ = r * mat;
                *sw *= -handedness;
                *lr *= -handedness;
                *tw *= -handedness;
            }
            ZeroAngle::Lr => {
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    lr_axis,
                    tw_axis,
                    Some(sw),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&tw_axis_r),
                    tw_axis,
                    fb_axis,
                    Some(fb),
                );
                r = r * mat;
                let mat = Self::rotate_onto_projected_mat(
                    &r.transform_dir(&fb_axis_r),
                    fb_axis,
                    tw_axis,
                    Some(tw),
                );
                let _ = r * mat;
                *sw *= -handedness;
                *fb *= -handedness;
                *tw *= -handedness;
            }
        }

        // Choose the closest rotation to the hint
        let (zero_tw, zero_fb, zero_lr, zero_sw) = match zero {
            ZeroAngle::Tw => (true, false, false, false),
            ZeroAngle::Fb => (false, true, false, false),
            ZeroAngle::Lr => (false, false, true, false),
            ZeroAngle::Sw => (false, false, false, true),
            ZeroAngle::None => (false, false, false, false),
        };
        Self::match_closest_euler_rotation_inner(
            hint_tw, hint_fb, hint_lr, hint_sw, tw, fb, lr, sw, zero_tw, zero_fb, zero_lr, zero_sw,
        );

        // Gimbal-lock correction
        let mut basis = Matrix3d::identity();
        basis.set_row(0, tw_axis);
        basis.set_row(1, fb_axis);
        basis.set_row(2, lr_axis);
        let h = basis.handedness();

        match zero {
            ZeroAngle::None | ZeroAngle::Sw => {
                Self::shift_gimbal_lock(*fb + PI / 2.0 * h, tw, lr);
            }
            ZeroAngle::Tw => {
                Self::shift_gimbal_lock(*lr + PI / 2.0 * h, fb, sw);
            }
            ZeroAngle::Fb => {
                Self::shift_gimbal_lock(*lr, tw, sw);
            }
            ZeroAngle::Lr => {
                Self::shift_gimbal_lock(*fb, tw, sw);
            }
        }
    }

    /// Inner implementation shared by `decompose_rotation` and `match_closest_euler_rotation`.
    ///
    /// `zero_*` flags indicate which angle slot is omitted (treated as standin 0.0).
    /// Angles are in radians. C++ parity: `GfRotation::MatchClosestEulerRotation(...)`.
    #[allow(clippy::too_many_arguments)]
    fn match_closest_euler_rotation_inner(
        target_tw: f64,
        target_fb: f64,
        target_lr: f64,
        target_sw: f64,
        tw: &mut f64,
        fb: &mut f64,
        lr: &mut f64,
        sw: &mut f64,
        zero_tw: bool,
        zero_fb: bool,
        zero_lr: bool,
        zero_sw: bool,
    ) {
        use std::f64::consts::PI;

        let num_angles =
            (!zero_tw) as usize + (!zero_fb) as usize + (!zero_lr) as usize + (!zero_sw) as usize;

        if num_angles == 0 {
            return;
        }

        let targets = [target_tw, target_fb, target_lr, target_sw];

        // With fewer than 3 angles, only pi-shift each toward its target
        if num_angles < 3 {
            *tw = Self::pi_shift_angle(*tw, target_tw);
            *fb = Self::pi_shift_angle(*fb, target_fb);
            *lr = Self::pi_shift_angle(*lr, target_lr);
            *sw = Self::pi_shift_angle(*sw, target_sw);
            return;
        }

        // Number of distinct candidate solutions
        let num_vals = if num_angles == 4 { 4 } else { 2 };

        // Each angle flipped by PI toward zero (min-abs direction)
        let lr_p = *lr + if *lr > 0.0 { -PI } else { PI };
        let fb_p = *fb + if *fb > 0.0 { -PI } else { PI };
        let tw_p = *tw + if *tw > 0.0 { -PI } else { PI };
        let sw_p = *sw + if *sw > 0.0 { -PI } else { PI };

        // Build candidate sets [tw, fb, lr, sw]; layout matches C++ vals[4]
        let mut vals: [[f64; 4]; 4] = [
            [*tw, *fb, *lr, *sw], // 0: identity (do nothing)
            [0.0, 0.0, 0.0, 0.0], // 1
            [0.0, 0.0, 0.0, 0.0], // 2
            [0.0, 0.0, 0.0, 0.0], // 3
        ];

        if zero_tw {
            // transform last 3
            vals[1] = [*tw, fb_p, -lr_p, sw_p];
        } else if zero_fb || zero_lr {
            // 1st+3rd composed
            vals[1] = [tw_p, -*fb, -*lr, sw_p];
        } else if zero_sw {
            // transform first 3
            vals[1] = [tw_p, -fb_p, lr_p, *sw];
        } else {
            // all four angles: 3 extra candidates
            vals[1] = [tw_p, -fb_p, lr_p, *sw];
            vals[2] = [tw_p, -*fb, -*lr, sw_p];
            vals[3] = [*tw, fb_p, -lr_p, sw_p];
        }

        // Pi-shift every candidate toward its target
        for v in vals[..num_vals].iter_mut() {
            for (i, t) in targets.iter().enumerate() {
                v[i] = Self::pi_shift_angle(v[i], *t);
            }
        }

        // Select the candidate with the smallest sum of |angle - target|
        let mut best = 0;
        let mut best_sum = f64::MAX;
        for i in 0..num_vals {
            let sum: f64 = vals[i]
                .iter()
                .zip(targets.iter())
                .map(|(v, t)| (v - t).abs())
                .sum();
            if sum < best_sum {
                best_sum = sum;
                best = i;
            }
        }

        *tw = vals[best][0];
        *fb = vals[best][1];
        *lr = vals[best][2];
        *sw = vals[best][3];
    }

    /// Replaces hint angles with the closest equivalent rotation.
    ///
    /// Each angle will be within PI of its hint, and the sum of absolute
    /// differences with hints is minimized. Pass `None` for any angle to
    /// treat it as 0.0 and ignore it in calculations.
    ///
    /// All angles are in radians. Rotation order: Tw/FB/LR/Sw.
    ///
    /// C++ parity: `GfRotation::MatchClosestEulerRotation(...)` — exact port.
    pub fn match_closest_euler_rotation(
        target_tw: f64,
        target_fb: f64,
        target_lr: f64,
        target_sw: f64,
        theta_tw: Option<&mut f64>,
        theta_fb: Option<&mut f64>,
        theta_lr: Option<&mut f64>,
        theta_sw: Option<&mut f64>,
    ) {
        let zero_tw = theta_tw.is_none();
        let zero_fb = theta_fb.is_none();
        let zero_lr = theta_lr.is_none();
        let zero_sw = theta_sw.is_none();

        let mut standin_tw = 0.0_f64;
        let mut standin_fb = 0.0_f64;
        let mut standin_lr = 0.0_f64;
        let mut standin_sw = 0.0_f64;

        let tw = theta_tw.unwrap_or(&mut standin_tw);
        let fb = theta_fb.unwrap_or(&mut standin_fb);
        let lr = theta_lr.unwrap_or(&mut standin_lr);
        let sw = theta_sw.unwrap_or(&mut standin_sw);

        Self::match_closest_euler_rotation_inner(
            target_tw, target_fb, target_lr, target_sw, tw, fb, lr, sw, zero_tw, zero_fb, zero_lr,
            zero_sw,
        );
    }

    /// Projects vectors onto a plane and returns rotation about the axis.
    ///
    /// Projects `v1` and `v2` onto the plane normal to `axis`,
    /// then returns the rotation about `axis` that brings `v1` onto `v2`.
    #[must_use]
    pub fn rotate_onto_projected(v1: &Vec3d, v2: &Vec3d, axis: &Vec3d) -> Self {
        let axis_n = axis.normalized();

        // Project vectors onto plane perpendicular to axis
        let v1_proj = (*v1 - axis_n * v1.dot(&axis_n)).normalized();
        let v2_proj = (*v2 - axis_n * v2.dot(&axis_n)).normalized();

        let cross_axis = v1_proj.cross(&v2_proj);
        let sin_theta = cross_axis.dot(&axis_n);
        let cos_theta = v1_proj.dot(&v2_proj);

        const EPSILON: f64 = 1e-6;
        let theta = if sin_theta.abs() < EPSILON && cos_theta.abs() < EPSILON {
            0.0
        } else {
            sin_theta.atan2(cos_theta)
        };

        Self::from_axis_angle(axis_n, radians_to_degrees(theta))
    }

    /// Spherically interpolates between two rotations.
    ///
    /// Uses quaternion SLERP for smooth constant-speed interpolation.
    /// `t` ranges from 0.0 (returns `start`) to 1.0 (returns `end`).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Rotation, vec3d};
    ///
    /// let start = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 0.0);
    /// let end = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
    /// let mid = Rotation::multi_rotate(&start, &end, 0.5);
    /// // mid should be ~45 degrees around Z
    /// assert!((mid.angle() - 45.0).abs() < 1e-6);
    /// ```
    #[must_use]
    pub fn multi_rotate(start: &Self, end: &Self, t: f64) -> Self {
        let q0 = start.get_quat();
        let q1 = end.get_quat();

        // Matches C++ GfSlerp: dot product, negate if needed for shortest path
        let mut cos_theta = q0.real() * q1.real() + q0.imaginary().dot(q1.imaginary());
        let flip = cos_theta < 0.0;
        if flip {
            cos_theta = -cos_theta;
        }

        // C++ threshold: 1.0 - cosTheta > 0.00001 (i.e., cosTheta < 0.99999)
        let (s0, mut s1) = if 1.0 - cos_theta > 1e-5 {
            // Standard SLERP
            let theta = cos_theta.acos();
            let sin_theta = theta.sin();
            (
                ((1.0 - t) * theta).sin() / sin_theta,
                (t * theta).sin() / sin_theta,
            )
        } else {
            // Nearly identical: linear interpolation (matching C++ exactly)
            (1.0 - t, t)
        };

        if flip {
            s1 = -s1;
        }

        let r = s0 * q0.real() + s1 * q1.real();
        let i = *q0.imaginary() * s0 + *q1.imaginary() * s1;
        Self::from_quat(&Quatd::new(r, i))
    }
}

impl PartialEq for Rotation {
    /// Component-wise equality of axis and angle.
    ///
    /// To compare actual rotations, convert to quaternions.
    fn eq(&self, other: &Self) -> bool {
        self.axis == other.axis && self.angle == other.angle
    }
}

impl Eq for Rotation {}

impl Hash for Rotation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.axis.x.to_bits().hash(state);
        self.axis.y.to_bits().hash(state);
        self.axis.z.to_bits().hash(state);
        self.angle.to_bits().hash(state);
    }
}

impl MulAssign for Rotation {
    /// Composes this rotation with another (post-multiply).
    fn mul_assign(&mut self, rhs: Self) {
        // Convert to quaternions and multiply
        let q = (rhs.get_quat() * self.get_quat()).normalized();

        let len = q.imaginary().length();
        if len > MIN_VECTOR_LENGTH {
            self.axis = *q.imaginary() / len;
            self.angle = 2.0 * radians_to_degrees(q.real().acos());
        } else {
            // Keep axis, set angle to 0
            self.angle = 0.0;
        }
    }
}

impl Mul for Rotation {
    type Output = Self;

    /// Returns the composite rotation.
    fn mul(mut self, rhs: Self) -> Self::Output {
        self *= rhs;
        self
    }
}

impl MulAssign<f64> for Rotation {
    /// Scales the angle by a factor.
    fn mul_assign(&mut self, scale: f64) {
        self.angle *= scale;
    }
}

impl Mul<f64> for Rotation {
    type Output = Self;

    /// Returns rotation with scaled angle.
    fn mul(mut self, scale: f64) -> Self::Output {
        self *= scale;
        self
    }
}

impl Mul<Rotation> for f64 {
    type Output = Rotation;

    /// Returns rotation with scaled angle.
    fn mul(self, mut rot: Rotation) -> Self::Output {
        rot *= self;
        rot
    }
}

impl DivAssign<f64> for Rotation {
    /// Divides the angle by a factor.
    fn div_assign(&mut self, scale: f64) {
        self.angle /= scale;
    }
}

impl Div<f64> for Rotation {
    type Output = Self;

    /// Returns rotation with divided angle.
    fn div(mut self, scale: f64) -> Self::Output {
        self /= scale;
        self
    }
}

impl fmt::Display for Rotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({} {} {}) {}]",
            self.axis.x, self.axis.y, self.axis.z, self.angle
        )
    }
}

// Extension methods for Matrix4d operating on Rotation
impl Matrix4d {
    /// Extracts the rotation from the upper-left 3x3 of this matrix.
    ///
    /// Assumes the matrix contains a rotation (orthonormal 3x3).
    /// For matrices with scale/shear, the result may be approximate.
    #[must_use]
    pub fn extract_rotation(&self) -> Rotation {
        Rotation::from_quat(&self.extract_rotation_quat())
    }

    /// Sets the matrix to a pure rotation from a `Rotation`, clearing translation.
    ///
    /// C++ parity: `GfMatrix4d::SetRotate(const GfRotation&)`.
    pub fn set_rotate_rotation(&mut self, rot: &Rotation) -> &mut Self {
        // Convert Rotation -> Quatd, delegate to the Quat-based set_rotate
        let q = rot.get_quat();
        self.set_rotate(&q)
    }

    /// Sets the upper-left 3x3 rotation from a `Rotation`, preserving translation.
    ///
    /// C++ parity: `GfMatrix4d::SetRotateOnly(const GfRotation&)`.
    pub fn set_rotate_only_rotation(&mut self, rot: &Rotation) -> &mut Self {
        let q = rot.get_quat();
        self.set_rotate_only(&q)
    }

    /// Sets the matrix to rotation + translation from a `Rotation` and `Vec3d`.
    ///
    /// C++ parity: `GfMatrix4d::SetTransform(const GfRotation&, const GfVec3d&)`.
    pub fn set_transform_rotation(&mut self, rot: &Rotation, translate: &Vec3d) -> &mut Self {
        self.set_rotate_rotation(rot);
        self.set_translate_only(translate)
    }

    /// Sets the matrix to a viewing transform from eye point and orientation rotation.
    ///
    /// The canonical frame looks along -Z with +Y up. The orientation rotation
    /// rigidly rotates this canonical frame into world space.
    /// C++ parity: `GfMatrix4d::SetLookAt(const GfVec3d&, const GfRotation&)`.
    pub fn set_look_at_orientation(&mut self, eye: &Vec3d, orientation: &Rotation) -> &mut Self {
        // C++: *this = translate(-eye) * rotate(orientation.GetInverse())
        let neg_eye = Vec3d::new(-eye.x, -eye.y, -eye.z);
        let mut m_translate = Matrix4d::identity();
        m_translate.set_translate(&neg_eye);

        let inv_rot = orientation.inverse();
        let mut m_rotate = Matrix4d::identity();
        m_rotate.set_rotate_rotation(&inv_rot);

        *self = m_translate * m_rotate;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3d;

    #[test]
    fn test_default() {
        let rot = Rotation::new();
        assert_eq!(rot.axis(), vec3d(1.0, 0.0, 0.0));
        assert_eq!(rot.angle(), 0.0);
    }

    #[test]
    fn test_from_axis_angle() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        assert!((rot.axis().z - 1.0).abs() < 1e-10);
        assert!((rot.angle() - 90.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalizes_axis() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 2.0), 45.0);
        assert!((rot.axis().length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_rotate_into() {
        let from = vec3d(1.0, 0.0, 0.0);
        let to = vec3d(0.0, 1.0, 0.0);
        let rot = Rotation::from_rotate_into(&from, &to);

        // Should be 90 degrees around Z
        assert!((rot.angle() - 90.0).abs() < 1e-10);
        assert!((rot.axis().z.abs() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_rotate_into_opposite() {
        let from = vec3d(1.0, 0.0, 0.0);
        let to = vec3d(-1.0, 0.0, 0.0);
        let rot = Rotation::from_rotate_into(&from, &to);

        // Should be 180 degrees
        assert!((rot.angle() - 180.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_rotate_into_same() {
        let from = vec3d(1.0, 0.0, 0.0);
        let to = vec3d(1.0, 0.0, 0.0);
        let rot = Rotation::from_rotate_into(&from, &to);

        // Should be identity
        assert!((rot.angle()).abs() < 1e-10);
    }

    #[test]
    fn test_transform_dir() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let v = vec3d(1.0, 0.0, 0.0);
        let result = rot.transform_dir(&v);

        // 90 degrees around Z takes X to Y
        assert!((result.x).abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
        assert!((result.z).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0);
        let inv = rot.inverse();

        assert_eq!(inv.axis(), rot.axis());
        assert!((inv.angle() - (-45.0)).abs() < 1e-10);
    }

    #[test]
    fn test_quat_roundtrip() {
        let rot = Rotation::from_axis_angle(vec3d(1.0, 1.0, 1.0).normalized(), 60.0);
        let q = rot.get_quat();
        let rot2 = Rotation::from_quat(&q);

        // Axes should match (up to sign)
        let dot = rot.axis().dot(&rot2.axis()).abs();
        assert!((dot - 1.0).abs() < 1e-10);
        // Angles should match (up to sign depending on axis flip)
        assert!((rot.angle().abs() - rot2.angle().abs()).abs() < 1e-10);
    }

    #[test]
    fn test_composition() {
        // Two 90-degree rotations around Z = 180 degrees
        let r1 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let r2 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let composed = r1 * r2;

        assert!((composed.angle().abs() - 180.0).abs() < 1e-10);
    }

    #[test]
    fn test_scale() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let scaled = rot * 0.5;

        assert!((scaled.angle() - 45.0).abs() < 1e-10);
    }

    #[test]
    fn test_divide() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let divided = rot / 2.0;

        assert!((divided.angle() - 45.0).abs() < 1e-10);
    }

    #[test]
    fn test_decompose() {
        // Create a rotation and decompose about standard axes
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0);
        let decomposed = rot.decompose(
            &vec3d(1.0, 0.0, 0.0),
            &vec3d(0.0, 1.0, 0.0),
            &vec3d(0.0, 0.0, 1.0),
        );

        // For rotation around Z, only the Z component should be non-zero
        assert!((decomposed.x).abs() < 1e-6);
        assert!((decomposed.y).abs() < 1e-6);
        assert!((decomposed.z.abs() - 45.0).abs() < 1e-6);
    }

    #[test]
    fn test_rotate_onto_projected() {
        let v1 = vec3d(1.0, 0.0, 0.0);
        let v2 = vec3d(0.0, 1.0, 0.0);
        let axis = vec3d(0.0, 0.0, 1.0);

        let rot = Rotation::rotate_onto_projected(&v1, &v2, &axis);

        // Should be 90 degrees
        assert!((rot.angle() - 90.0).abs() < 1e-10);
    }

    #[test]
    fn test_display() {
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0);
        let s = format!("{}", rot);
        assert!(s.contains("45"));
    }

    #[test]
    fn test_equality() {
        let r1 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0);
        let r2 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0);
        assert_eq!(r1, r2);
    }

    // =====================================================================
    // H-vt-5: multi_rotate (SLERP) tests
    // =====================================================================

    #[test]
    fn test_multi_rotate_identity() {
        let start = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 0.0);
        let end = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);

        // t=0 should return start
        let r0 = Rotation::multi_rotate(&start, &end, 0.0);
        assert!(r0.angle().abs() < 1e-6);

        // t=1 should return end
        let r1 = Rotation::multi_rotate(&start, &end, 1.0);
        assert!((r1.angle() - 90.0).abs() < 1e-6);
    }

    #[test]
    fn test_multi_rotate_midpoint() {
        let start = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 0.0);
        let end = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let mid = Rotation::multi_rotate(&start, &end, 0.5);
        assert!((mid.angle() - 45.0).abs() < 1e-4);
    }

    #[test]
    fn test_multi_rotate_same() {
        // Interpolating between identical rotations should return that rotation
        let r = Rotation::from_axis_angle(vec3d(1.0, 0.0, 0.0), 60.0);
        let result = Rotation::multi_rotate(&r, &r, 0.5);
        assert!((result.angle() - 60.0).abs() < 1e-4);
    }

    #[test]
    fn test_multi_rotate_different_axes() {
        let r1 = Rotation::from_axis_angle(vec3d(1.0, 0.0, 0.0), 90.0);
        let r2 = Rotation::from_axis_angle(vec3d(0.0, 1.0, 0.0), 90.0);
        // Just verify it doesn't panic and produces a valid rotation
        let mid = Rotation::multi_rotate(&r1, &r2, 0.5);
        assert!(mid.angle().is_finite());
    }

    // =========================================================================
    // M-gf: SLERP threshold matches C++ GfSlerp (1e-5 vs old 5e-4)
    // =========================================================================

    #[test]
    fn test_multi_rotate_nearly_identical() {
        // Two rotations very close (should hit linear path in C++ at 1e-5)
        let r1 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 10.0);
        let r2 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 10.000001);
        let mid = Rotation::multi_rotate(&r1, &r2, 0.5);
        assert!((mid.angle() - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_multi_rotate_opposite_hemispheres() {
        // When dot < 0, should take shortest path (flip q1)
        let r1 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 10.0);
        let r2 = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 350.0);
        let mid = Rotation::multi_rotate(&r1, &r2, 0.5);
        // Shortest path from 10 to 350 goes through 0/360
        assert!(mid.angle() < 180.0 || (360.0 - mid.angle()) < 10.0);
    }

    #[test]
    fn test_multi_rotate_quarter_steps() {
        let start = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 0.0);
        let end = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 120.0);
        let q1 = Rotation::multi_rotate(&start, &end, 0.25);
        let q2 = Rotation::multi_rotate(&start, &end, 0.75);
        assert!((q1.angle() - 30.0).abs() < 1e-4);
        assert!((q2.angle() - 90.0).abs() < 1e-4);
    }

    // =========================================================================
    // Rotation <-> Matrix conversions
    // =========================================================================

    #[test]
    fn test_get_matrix3_roundtrip() {
        // Rotation -> Matrix3d -> extract_rotation should recover the same rotation
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0);
        let m3 = rot.get_matrix3();
        let rot2 = m3.extract_rotation();
        // Compare via quaternion dot product (axis may flip for 0-degree case)
        let q1 = rot.get_quat();
        let q2 = rot2.get_quat();
        let dot = (q1.real() * q2.real() + q1.imaginary().dot(q2.imaginary())).abs();
        assert!((dot - 1.0).abs() < 1e-9, "dot={dot}");
    }

    #[test]
    fn test_get_matrix4_roundtrip() {
        // Rotation -> Matrix4d -> extract_rotation should recover the same rotation
        let rot = Rotation::from_axis_angle(vec3d(1.0, 0.0, 0.0), 60.0);
        let m4 = rot.get_matrix4();
        let rot2 = m4.extract_rotation();
        let q1 = rot.get_quat();
        let q2 = rot2.get_quat();
        let dot = (q1.real() * q2.real() + q1.imaginary().dot(q2.imaginary())).abs();
        assert!((dot - 1.0).abs() < 1e-9, "dot={dot}");
    }

    #[test]
    fn test_get_matrix4_translation_zero() {
        // get_matrix4() must zero the translation column
        let rot = Rotation::from_axis_angle(vec3d(0.0, 1.0, 0.0), 30.0);
        let m = rot.get_matrix4();
        assert_eq!(m[3][0], 0.0);
        assert_eq!(m[3][1], 0.0);
        assert_eq!(m[3][2], 0.0);
        assert_eq!(m[3][3], 1.0);
    }

    #[test]
    fn test_set_matrix() {
        // Build a rotation, get its Matrix3d, then set_matrix back and compare
        let rot_orig = Rotation::from_axis_angle(vec3d(0.0, 1.0, 0.0), 75.0);
        let m3 = rot_orig.get_matrix3();
        let mut rot2 = Rotation::new();
        rot2.set_matrix(&m3);
        let q1 = rot_orig.get_quat();
        let q2 = rot2.get_quat();
        let dot = (q1.real() * q2.real() + q1.imaginary().dot(q2.imaginary())).abs();
        assert!((dot - 1.0).abs() < 1e-9, "dot={dot}");
    }

    #[test]
    fn test_get_matrix3_identity() {
        // Identity rotation -> identity matrix
        let rot = Rotation::new(); // 0 deg around X
        let m = rot.get_matrix3();
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (m[i][j] - expected).abs() < 1e-12,
                    "m[{i}][{j}]={}",
                    m[i][j]
                );
            }
        }
    }

    #[test]
    fn test_get_matrix4_rotate_dir() {
        // 90 deg around Z: X -> Y via matrix4
        let rot = Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0);
        let m = rot.get_matrix4();
        let v = vec3d(1.0, 0.0, 0.0);
        let result = m.transform_dir(&v);
        assert!(result.x.abs() < 1e-10, "x={}", result.x);
        assert!((result.y - 1.0).abs() < 1e-10, "y={}", result.y);
    }

    // =========================================================================
    // pi_shift_angle helper
    // =========================================================================

    #[test]
    fn test_pi_shift_angle() {
        use std::f64::consts::PI;
        // Already within PI of hint -> unchanged
        let a = Rotation::pi_shift_angle(0.5, 0.0);
        assert!((a - 0.5).abs() < 1e-12);
        // More than PI above hint -> subtract 2*PI
        let a = Rotation::pi_shift_angle(PI + 0.1, 0.0);
        assert!(a < 0.0, "a={a}");
        // More than PI below hint -> add 2*PI
        let a = Rotation::pi_shift_angle(-PI - 0.1, 0.0);
        assert!(a > 0.0, "a={a}");
    }

    // =========================================================================
    // match_closest_euler_rotation
    // =========================================================================

    #[test]
    fn test_match_closest_all_four() {
        use std::f64::consts::PI;
        // All four angles provided; target = 0. Result should minimise total |diff|.
        let mut tw = 0.1_f64;
        let mut fb = 0.2_f64;
        let mut lr = -0.1_f64;
        let mut sw = 0.05_f64;
        Rotation::match_closest_euler_rotation(
            0.0,
            0.0,
            0.0,
            0.0,
            Some(&mut tw),
            Some(&mut fb),
            Some(&mut lr),
            Some(&mut sw),
        );
        // All should still be within PI of 0
        assert!(tw.abs() <= PI, "tw={tw}");
        assert!(fb.abs() <= PI, "fb={fb}");
        assert!(lr.abs() <= PI, "lr={lr}");
        assert!(sw.abs() <= PI, "sw={sw}");
    }

    #[test]
    fn test_match_closest_one_none() {
        use std::f64::consts::PI;
        // One angle omitted; should still adjust the other three
        let mut fb = 0.2_f64;
        let mut lr = -0.1_f64;
        let mut sw = 0.05_f64;
        Rotation::match_closest_euler_rotation(
            0.0,
            0.0,
            0.0,
            0.0,
            None,
            Some(&mut fb),
            Some(&mut lr),
            Some(&mut sw),
        );
        assert!(fb.abs() <= PI);
        assert!(lr.abs() <= PI);
        assert!(sw.abs() <= PI);
    }

    #[test]
    fn test_match_closest_wraps_toward_hint() {
        use std::f64::consts::PI;
        // Angle is just past PI from hint -> should wrap to the other side
        let hint = 0.0_f64;
        let mut tw = PI + 0.5; // > PI from hint
        let mut fb = 0.0_f64;
        let mut lr = 0.0_f64;
        let mut sw = 0.0_f64;
        Rotation::match_closest_euler_rotation(
            hint,
            0.0,
            0.0,
            0.0,
            Some(&mut tw),
            Some(&mut fb),
            Some(&mut lr),
            Some(&mut sw),
        );
        // After shift, tw should be closer to 0 than PI+0.5
        assert!((tw - hint).abs() <= PI, "tw={tw}");
    }

    // =========================================================================
    // decompose_rotation (basic smoke test)
    // =========================================================================

    #[test]
    fn test_decompose_rotation_identity() {
        // Identity matrix -> all angles should be ~0
        let m = Matrix4d::identity();
        let tw_axis = vec3d(0.0, 0.0, 1.0);
        let fb_axis = vec3d(1.0, 0.0, 0.0);
        let lr_axis = vec3d(0.0, 1.0, 0.0);
        let mut tw = 0.0_f64;
        let mut fb = 0.0_f64;
        let mut lr = 0.0_f64;
        let mut sw = 0.0_f64;
        Rotation::decompose_rotation(
            &m,
            &tw_axis,
            &fb_axis,
            &lr_axis,
            1.0,
            Some(&mut tw),
            Some(&mut fb),
            Some(&mut lr),
            Some(&mut sw),
            false,
            None,
        );
        assert!(tw.abs() < 1e-10, "tw={tw}");
        assert!(fb.abs() < 1e-10, "fb={fb}");
        assert!(lr.abs() < 1e-10, "lr={lr}");
        assert!(sw.abs() < 1e-10, "sw={sw}");
    }

    #[test]
    fn test_decompose_rotation_pure_tw() {
        // Pure twist (rotation around Z = tw_axis)
        let tw_axis = vec3d(0.0, 0.0, 1.0);
        let fb_axis = vec3d(1.0, 0.0, 0.0);
        let lr_axis = vec3d(0.0, 1.0, 0.0);
        let angle_deg = 30.0_f64;
        let rot = Rotation::from_axis_angle(tw_axis, angle_deg);
        let m = rot.get_matrix4();
        let mut tw = 0.0_f64;
        let mut fb = 0.0_f64;
        let mut lr = 0.0_f64;
        let mut sw = 0.0_f64;
        Rotation::decompose_rotation(
            &m,
            &tw_axis,
            &fb_axis,
            &lr_axis,
            1.0,
            Some(&mut tw),
            Some(&mut fb),
            Some(&mut lr),
            Some(&mut sw),
            false,
            None,
        );
        // fb and lr should be ~0, tw should be ~30 deg in radians
        let expected_rad = angle_deg.to_radians();
        assert!(fb.abs() < 1e-9, "fb={fb}");
        assert!(lr.abs() < 1e-9, "lr={lr}");
        assert!(
            (tw.abs() - expected_rad).abs() < 1e-9,
            "tw={tw}, expected={expected_rad}"
        );
    }

    #[test]
    fn test_decompose_rotation_pure_fb() {
        // Pure front-back rotation (around X = fb_axis)
        let tw_axis = vec3d(0.0, 0.0, 1.0);
        let fb_axis = vec3d(1.0, 0.0, 0.0);
        let lr_axis = vec3d(0.0, 1.0, 0.0);
        let angle_deg = 45.0_f64;
        let rot = Rotation::from_axis_angle(fb_axis, angle_deg);
        let m = rot.get_matrix4();
        let mut tw = 0.0_f64;
        let mut fb = 0.0_f64;
        let mut lr = 0.0_f64;
        let mut sw = 0.0_f64;
        Rotation::decompose_rotation(
            &m,
            &tw_axis,
            &fb_axis,
            &lr_axis,
            1.0,
            Some(&mut tw),
            Some(&mut fb),
            Some(&mut lr),
            Some(&mut sw),
            false,
            None,
        );
        let expected = angle_deg.to_radians();
        assert!(tw.abs() < 1e-9, "tw={tw}");
        assert!(lr.abs() < 1e-9, "lr={lr}");
        assert!(
            (fb.abs() - expected).abs() < 1e-9,
            "fb={fb}, expected={expected}"
        );
    }

    #[test]
    fn test_decompose_rotation_omit_sw() {
        // Omit sw (three-angle decomposition)
        let tw_axis = vec3d(0.0, 0.0, 1.0);
        let fb_axis = vec3d(1.0, 0.0, 0.0);
        let lr_axis = vec3d(0.0, 1.0, 0.0);
        let rot = Rotation::from_axis_angle(tw_axis, 20.0);
        let m = rot.get_matrix4();
        let mut tw = 0.0_f64;
        let mut fb = 0.0_f64;
        let mut lr = 0.0_f64;
        // sw = None means omit
        Rotation::decompose_rotation(
            &m,
            &tw_axis,
            &fb_axis,
            &lr_axis,
            1.0,
            Some(&mut tw),
            Some(&mut fb),
            Some(&mut lr),
            None,
            false,
            None,
        );
        assert!(fb.abs() < 1e-9, "fb={fb}");
        assert!(lr.abs() < 1e-9, "lr={lr}");
        assert!((tw.abs() - 20.0_f64.to_radians()).abs() < 1e-9, "tw={tw}");
    }

    #[test]
    fn test_rotate_onto_projected_90() {
        // v1=X, v2=Y, axis=Z -> 90 deg rotation
        let v1 = vec3d(1.0, 0.0, 0.0);
        let v2 = vec3d(0.0, 1.0, 0.0);
        let axis = vec3d(0.0, 0.0, 1.0);
        let rot = Rotation::rotate_onto_projected(&v1, &v2, &axis);
        assert!((rot.angle() - 90.0).abs() < 1e-9, "angle={}", rot.angle());
        // Axis should be +Z
        assert!((rot.axis().z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_rotate_onto_projected_already_aligned() {
        // v1 == v2 projected -> 0 deg rotation
        let v1 = vec3d(1.0, 0.5, 0.0); // some vector
        let v2 = vec3d(1.0, 0.5, 1.0); // same in XY plane after projection
        let axis = vec3d(0.0, 0.0, 1.0);
        let rot = Rotation::rotate_onto_projected(&v1, &v2, &axis);
        assert!(rot.angle().abs() < 1e-9, "angle={}", rot.angle());
    }
}
