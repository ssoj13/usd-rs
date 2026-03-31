//! Layer time offset and scale.
//!
//! `LayerOffset` represents an affine transformation (scale and offset)
//! applied to time values when composing layers.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Mul;

use super::TimeCode;

/// Epsilon used for fuzzy LayerOffset comparison, matching C++ `#define EPSILON (1e-6)`.
const EPSILON: f64 = 1e-6;

/// GfIsClose equivalent: absolute comparison |a - b| < eps.
/// Matches C++ `GfIsClose(double a, double b, double epsilon)` in gf/math.h.
#[inline]
fn gf_is_close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

/// Represents a time offset and scale between layers.
///
/// `LayerOffset` is an affine transform providing both scale and translate.
/// It supports composition via multiplication. The class is unitless: it
/// does not refer to seconds or frames directly.
///
/// When bringing animation from layer B into layer A with offset X:
/// first apply the scale, then the offset. With scale=2 and offset=24,
/// animation from B will take twice as long and start 24 frames later.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::LayerOffset;
///
/// // Create an offset with scale 2 and offset 24
/// let offset = LayerOffset::new(24.0, 2.0);
///
/// // Apply to a time value
/// let time = 10.0;
/// let result = offset * time;
/// assert_eq!(result, 44.0); // 10 * 2 + 24
/// ```
#[derive(Clone, Copy)]
pub struct LayerOffset {
    /// The time offset value.
    offset: f64,
    /// The time scale factor.
    scale: f64,
}

impl Default for LayerOffset {
    fn default() -> Self {
        Self::identity()
    }
}

impl LayerOffset {
    /// Creates a new layer offset with the given offset and scale.
    ///
    /// # Arguments
    ///
    /// * `offset` - The time offset (default 0.0)
    /// * `scale` - The time scale factor (default 1.0)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerOffset;
    ///
    /// let offset = LayerOffset::new(10.0, 2.0);
    /// assert_eq!(offset.offset(), 10.0);
    /// assert_eq!(offset.scale(), 2.0);
    /// ```
    pub fn new(offset: f64, scale: f64) -> Self {
        Self { offset, scale }
    }

    /// Creates an identity layer offset (offset=0, scale=1).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerOffset;
    ///
    /// let id = LayerOffset::identity();
    /// assert!(id.is_identity());
    /// ```
    pub fn identity() -> Self {
        Self {
            offset: 0.0,
            scale: 1.0,
        }
    }

    /// Returns the time offset.
    pub fn offset(&self) -> f64 {
        self.offset
    }

    /// Returns the time scale factor.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Sets the time offset.
    pub fn set_offset(&mut self, offset: f64) {
        self.offset = offset;
    }

    /// Sets the time scale factor.
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    /// Returns true if this is an identity transformation.
    ///
    /// Uses epsilon comparison (1e-6) matching C++ GfIsClose/operator== behavior.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerOffset;
    ///
    /// assert!(LayerOffset::identity().is_identity());
    /// assert!(!LayerOffset::new(1.0, 1.0).is_identity());
    /// assert!(!LayerOffset::new(0.0, 2.0).is_identity());
    /// ```
    pub fn is_identity(&self) -> bool {
        // C++ uses operator==() for the identity check, which itself uses GfIsClose
        *self == LayerOffset::identity()
    }

    /// Returns true if this offset is valid.
    ///
    /// A valid offset has finite (not infinite or NaN) offset and scale values.
    /// Note that a valid layer offset's inverse may be invalid.
    pub fn is_valid(&self) -> bool {
        self.offset.is_finite() && self.scale.is_finite()
    }

    /// Returns the inverse offset.
    ///
    /// The inverse performs the opposite transformation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerOffset;
    ///
    /// let offset = LayerOffset::new(10.0, 2.0);
    /// let inv = offset.inverse();
    ///
    /// // Applying offset then inverse should give identity
    /// let composed = offset * inv;
    /// assert!((composed.offset() - 0.0).abs() < 1e-10);
    /// assert!((composed.scale() - 1.0).abs() < 1e-10);
    /// ```
    pub fn inverse(&self) -> Self {
        // Matches C++: identity shortcut, then compute 1/scale (infinity for zero)
        if self.is_identity() {
            return *self;
        }
        if self.scale == 0.0 {
            // Return invalid offset (infinity) matching C++ behavior
            Self::new(f64::INFINITY, f64::INFINITY)
        } else {
            let new_scale = 1.0 / self.scale;
            Self::new(-self.offset * new_scale, new_scale)
        }
    }

    /// Composes this offset with another.
    ///
    /// The result is equivalent to first applying `other`, then `self`.
    /// This is the same as `self * other` but provided as a method for
    /// convenience and code clarity.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerOffset;
    ///
    /// let a = LayerOffset::new(10.0, 2.0);
    /// let b = LayerOffset::new(5.0, 3.0);
    ///
    /// // Composing a.compose(b): first apply b, then a
    /// let composed = a.compose(&b);
    ///
    /// // For time t: b(t) = 3*t + 5, then a(b(t)) = 2*(3*t + 5) + 10 = 6*t + 20
    /// assert_eq!(composed.apply(1.0), 26.0);
    /// ```
    pub fn compose(&self, other: &LayerOffset) -> Self {
        // Compose: first apply other (s2 * t + o2), then self (s1 * t + o1)
        // Result: s1 * (s2 * t + o2) + o1 = (s1 * s2) * t + (s1 * o2 + o1)
        Self::new(
            self.scale * other.offset + self.offset,
            self.scale * other.scale,
        )
    }

    /// Returns the hash of this offset.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Applies this offset to a time value.
    ///
    /// The formula is: result = scale * time + offset
    pub fn apply(&self, time: f64) -> f64 {
        self.scale * time + self.offset
    }

    /// Applies this offset to a time code.
    pub fn apply_to_time_code(&self, time: TimeCode) -> TimeCode {
        TimeCode::new(self.apply(time.value()))
    }
}

impl PartialEq for LayerOffset {
    fn eq(&self, other: &Self) -> bool {
        // Matches C++: (!IsValid() && !rhs.IsValid()) || (GfIsClose(offset) && GfIsClose(scale))
        (!self.is_valid() && !other.is_valid())
            || (gf_is_close(self.offset, other.offset, EPSILON)
                && gf_is_close(self.scale, other.scale, EPSILON))
    }
}

impl Eq for LayerOffset {}

impl PartialOrd for LayerOffset {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LayerOffset {
    fn cmp(&self, other: &Self) -> Ordering {
        // Matches C++ operator<:
        // - invalid (non-finite) is "greater than" anything valid (never less)
        // - compare scale first (with epsilon), then offset (with epsilon)
        if !self.is_valid() {
            return Ordering::Greater;
        }
        if !other.is_valid() {
            return Ordering::Less;
        }
        // Scale first, then offset (matching C++)
        if gf_is_close(self.scale, other.scale, EPSILON) {
            if gf_is_close(self.offset, other.offset, EPSILON) {
                Ordering::Equal
            } else if self.offset < other.offset {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        } else if self.scale < other.scale {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}

impl Hash for LayerOffset {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.offset.to_bits().hash(state);
        self.scale.to_bits().hash(state);
    }
}

/// Composes two layer offsets.
///
/// The result is equivalent to first applying `self`, then `rhs`.
/// Matches C++ `SdfLayerOffset::operator*()`.
impl Mul for LayerOffset {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        // Concatenation: apply rhs first, then self
        // Matches C++ SdfLayerOffset::operator*()
        // If self transforms t -> s1*t + o1, and rhs transforms t -> s2*t + o2,
        // then self * rhs transforms t -> s1*(s2*t + o2) + o1 = (s1*s2)*t + (s1*o2 + o1)
        Self::new(
            self.scale * rhs.offset + self.offset,
            self.scale * rhs.scale,
        )
    }
}

/// Applies the offset to a time value.
impl Mul<f64> for LayerOffset {
    type Output = f64;

    fn mul(self, rhs: f64) -> Self::Output {
        self.apply(rhs)
    }
}

/// Applies the offset to a time code.
impl Mul<TimeCode> for LayerOffset {
    type Output = TimeCode;

    fn mul(self, rhs: TimeCode) -> Self::Output {
        self.apply_to_time_code(rhs)
    }
}

impl fmt::Debug for LayerOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LayerOffset(offset={}, scale={})",
            self.offset, self.scale
        )
    }
}

impl fmt::Display for LayerOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_identity() {
            write!(f, "LayerOffset()")
        } else if self.scale == 1.0 {
            write!(f, "LayerOffset(offset={})", self.offset)
        } else if self.offset == 0.0 {
            write!(f, "LayerOffset(scale={})", self.scale)
        } else {
            write!(
                f,
                "LayerOffset(offset={}, scale={})",
                self.offset, self.scale
            )
        }
    }
}

/// A vector of layer offsets.
pub type LayerOffsetVector = Vec<LayerOffset>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let offset = LayerOffset::new(10.0, 2.0);
        assert_eq!(offset.offset(), 10.0);
        assert_eq!(offset.scale(), 2.0);
    }

    #[test]
    fn test_identity() {
        let id = LayerOffset::identity();
        assert_eq!(id.offset(), 0.0);
        assert_eq!(id.scale(), 1.0);
        assert!(id.is_identity());
    }

    #[test]
    fn test_default() {
        let def = LayerOffset::default();
        assert!(def.is_identity());
    }

    #[test]
    fn test_is_valid() {
        assert!(LayerOffset::new(10.0, 2.0).is_valid());
        assert!(!LayerOffset::new(f64::INFINITY, 1.0).is_valid());
        assert!(!LayerOffset::new(0.0, f64::NAN).is_valid());
    }

    #[test]
    fn test_apply() {
        let offset = LayerOffset::new(24.0, 2.0);
        // result = scale * time + offset = 2 * 10 + 24 = 44
        assert_eq!(offset.apply(10.0), 44.0);
    }

    #[test]
    fn test_apply_operator() {
        let offset = LayerOffset::new(24.0, 2.0);
        assert_eq!(offset * 10.0, 44.0);
    }

    #[test]
    fn test_apply_time_code() {
        let offset = LayerOffset::new(24.0, 2.0);
        let tc = TimeCode::new(10.0);
        let result = offset * tc;
        assert_eq!(result.value(), 44.0);
    }

    #[test]
    fn test_inverse() {
        let offset = LayerOffset::new(10.0, 2.0);
        let inv = offset.inverse();

        // Apply offset then inverse should give original value
        let time = 5.0;
        let transformed = offset.apply(time);
        let back = inv.apply(transformed);
        assert!((back - time).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_identity() {
        // Matches C++: inverse of identity is identity
        let id = LayerOffset::identity();
        assert!(id.inverse().is_identity());
    }

    #[test]
    fn test_compose() {
        let a = LayerOffset::new(10.0, 2.0);
        let b = LayerOffset::new(5.0, 3.0);

        // Composing a * b: first apply b, then a
        let composed = a * b;

        // For time t: b(t) = 3*t + 5, then a(b(t)) = 2*(3*t + 5) + 10 = 6*t + 20
        let time = 1.0;
        let expected = 6.0 * time + 20.0;
        assert_eq!(composed.apply(time), expected);
    }

    #[test]
    fn test_equality() {
        let a = LayerOffset::new(10.0, 2.0);
        let b = LayerOffset::new(10.0, 2.0);
        let c = LayerOffset::new(10.0, 3.0);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_equality_epsilon() {
        // Two invalid offsets are equal (matching C++)
        let inv1 = LayerOffset::new(f64::NAN, 1.0);
        let inv2 = LayerOffset::new(f64::INFINITY, 1.0);
        assert_eq!(inv1, inv2); // both invalid
    }

    #[test]
    fn test_ordering() {
        // scale compared first, then offset
        let a = LayerOffset::new(10.0, 2.0);
        let b = LayerOffset::new(20.0, 2.0); // same scale, bigger offset
        let c = LayerOffset::new(10.0, 3.0); // bigger scale

        assert!(a < b); // same scale 2.0, offset 10 < 20
        assert!(a < c); // scale 2.0 < 3.0
    }

    #[test]
    fn test_ordering_invalid() {
        // Invalid offsets are "greater than" valid ones (matching C++ operator<)
        let valid = LayerOffset::new(10.0, 2.0);
        let invalid = LayerOffset::new(f64::INFINITY, 1.0);
        assert!(valid < invalid);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let a = LayerOffset::new(10.0, 2.0);
        let b = LayerOffset::new(10.0, 2.0);
        let c = LayerOffset::new(10.0, 3.0);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", LayerOffset::identity()), "LayerOffset()");
        assert_eq!(
            format!("{}", LayerOffset::new(10.0, 1.0)),
            "LayerOffset(offset=10)"
        );
        assert_eq!(
            format!("{}", LayerOffset::new(0.0, 2.0)),
            "LayerOffset(scale=2)"
        );
        assert_eq!(
            format!("{}", LayerOffset::new(10.0, 2.0)),
            "LayerOffset(offset=10, scale=2)"
        );
    }

    #[test]
    fn test_setters() {
        let mut offset = LayerOffset::identity();
        offset.set_offset(10.0);
        offset.set_scale(2.0);
        assert_eq!(offset.offset(), 10.0);
        assert_eq!(offset.scale(), 2.0);
    }
}
