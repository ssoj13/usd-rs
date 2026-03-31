//! Limits for Bfr topology (max valence, max face size).

use crate::far::VALENCE_LIMIT;

/// Simple struct exposing topology limits for Bfr.
///
/// Mirrors `Bfr::Limits` from `limits.h`.
#[derive(Debug, Clone, Copy)]
pub struct Limits;

impl Limits {
    /// Maximum allowable valence for a vertex.
    #[inline]
    pub fn max_valence() -> i32 {
        VALENCE_LIMIT
    }

    /// Maximum allowable size for a face (number of vertices).
    #[inline]
    pub fn max_face_size() -> i32 {
        VALENCE_LIMIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_positive() {
        assert!(Limits::max_valence() > 0);
        assert!(Limits::max_face_size() > 0);
        assert_eq!(Limits::max_valence(), Limits::max_face_size());
    }
}
