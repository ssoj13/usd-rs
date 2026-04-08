/// Describes a channel of primvar data inside an interleaved float buffer.
///
/// Mirrors Osd::BufferDescriptor exactly — all field semantics match C++:
///
/// ```text
/// |  X  Y  Z  R  G  B  A  Xu Yu Zu Xv Yv Zv |
///    <---- stride = 13 ---->
///  XYZ:      offset=0,  length=3, stride=13
///  RGBA:     offset=3,  length=4, stride=13
///  uTangent: offset=7,  length=3, stride=13
///  vTangent: offset=10, length=3, stride=13
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BufferDescriptor {
    /// Absolute offset of the first element (in floats) within the buffer.
    /// When multiple objects are batched together, this encodes the batch offset.
    pub offset: i32,
    /// Number of floats per element (e.g. 3 for XYZ).
    pub length: i32,
    /// Stride between consecutive elements (in floats).
    pub stride: i32,
}

impl BufferDescriptor {
    /// Construct with explicit fields.
    pub fn new(offset: i32, length: i32, stride: i32) -> Self {
        Self {
            offset,
            length,
            stride,
        }
    }

    /// Returns the element-local offset within one stride period.
    ///
    /// `offset % stride` when stride > 0, else 0.
    pub fn get_local_offset(&self) -> i32 {
        if self.stride > 0 {
            self.offset % self.stride
        } else {
            0
        }
    }

    /// True when the descriptor is internally consistent:
    /// length > 0 and the data fits within one stride period.
    pub fn is_valid(&self) -> bool {
        self.length > 0 && self.length <= self.stride - self.get_local_offset()
    }

    /// Reset all fields to zero (equivalent to default).
    pub fn reset(&mut self) {
        self.offset = 0;
        self.length = 0;
        self.stride = 0;
    }

    /// True when two descriptors have matching local offset, length, stride.
    ///
    /// Used by `EvaluatorCache` to find a cached evaluator — only the
    /// element-local (intra-stride) part of the offset matters, not the
    /// batch base offset.
    ///
    /// # C++ equivalence
    ///
    /// C++ `operator==` compares *absolute* offsets (used for exact equality)
    /// and is mapped to Rust `PartialEq` / `==`.  The private
    /// `EvaluatorCacheT::isEqual()` helper compares *local* offsets for cache
    /// lookup — that is what this `matches()` function implements.
    /// The split between `==` (absolute) and `matches()` (local) is intentional
    /// and correct; callers that want cache-key semantics must use `matches()`.
    #[doc(alias = "isEqual")]
    pub fn matches(&self, other: &Self) -> bool {
        let lo_self = if self.stride > 0 {
            self.offset % self.stride
        } else {
            0
        };
        let lo_other = if other.stride > 0 {
            other.offset % other.stride
        } else {
            0
        };
        lo_self == lo_other && self.length == other.length && self.stride == other.stride
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_offset_with_stride() {
        let d = BufferDescriptor::new(7, 3, 13);
        assert_eq!(d.get_local_offset(), 7);
    }

    #[test]
    fn local_offset_zero_stride() {
        let d = BufferDescriptor::new(5, 3, 0);
        assert_eq!(d.get_local_offset(), 0);
    }

    #[test]
    fn is_valid_fits() {
        // offset=0, length=3, stride=13 → local_offset=0, 3 <= 13-0 = 13 ✓
        let d = BufferDescriptor::new(0, 3, 13);
        assert!(d.is_valid());
    }

    #[test]
    fn is_valid_overflow() {
        // offset=11, length=5, stride=13 → local_offset=11, 5 <= 13-11=2 ✗
        let d = BufferDescriptor::new(11, 5, 13);
        assert!(!d.is_valid());
    }

    #[test]
    fn is_valid_zero_length() {
        let d = BufferDescriptor::new(0, 0, 13);
        assert!(!d.is_valid());
    }

    #[test]
    fn reset_clears() {
        let mut d = BufferDescriptor::new(5, 3, 13);
        d.reset();
        assert_eq!(d, BufferDescriptor::default());
    }

    #[test]
    fn matches_ignores_batch_offset() {
        // Two descriptors that differ only in batch base offset should match
        let a = BufferDescriptor::new(13, 3, 13); // local offset = 0
        let b = BufferDescriptor::new(26, 3, 13); // local offset = 0
        assert!(a.matches(&b));
    }

    #[test]
    fn matches_different_local() {
        let a = BufferDescriptor::new(0, 3, 13);
        let b = BufferDescriptor::new(3, 4, 13);
        assert!(!a.matches(&b));
    }
}
