//! FaceVertexSubset — a subset of the topology around a corner vertex.
//!
//! Mirrors `Bfr::FaceVertexSubset` from `faceVertexSubset.h`.

use super::vertex_tag::VertexTag;

/// Identifies a connected subset of incident faces around a corner that
/// contribute to the limit surface.
///
/// Mirrors `Bfr::FaceVertexSubset`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaceVertexSubset {
    pub tag: VertexTag,

    /// Faces before (clockwise from) the base face in the subset.
    pub num_faces_before: i16,
    /// Faces after (counter-clockwise from) the base face in the subset.
    pub num_faces_after: i16,
    /// Total faces in the subset.
    pub num_faces_total: i16,

    /// Sharpness override for this subset (rarely non-zero).
    pub local_sharpness: f32,
}

impl FaceVertexSubset {
    /// Initialise to a single-face subset with the given `VertexTag`.
    pub fn initialize(&mut self, tag: VertexTag) {
        self.tag = tag;
        self.num_faces_before = 0;
        self.num_faces_after = 0;
        self.num_faces_total = 1;
        self.local_sharpness = 0.0;
    }

    // -----------------------------------------------------------------------
    // Simple queries
    // -----------------------------------------------------------------------

    #[inline]
    pub fn get_tag(&self) -> VertexTag {
        self.tag
    }
    #[inline]
    pub fn get_num_faces(&self) -> i32 {
        self.num_faces_total as i32
    }

    #[inline]
    pub fn is_boundary(&self) -> bool {
        self.tag.0.boundary_verts()
    }
    #[inline]
    pub fn is_sharp(&self) -> bool {
        self.tag.0.inf_sharp_verts()
    }

    #[inline]
    pub fn set_boundary(&mut self, on: bool) {
        self.tag.0.set_boundary_verts(on);
    }
    #[inline]
    pub fn set_sharp(&mut self, on: bool) {
        self.tag.0.set_inf_sharp_verts(on);
    }

    // -----------------------------------------------------------------------
    // Comparison to superset
    // -----------------------------------------------------------------------

    /// `true` when this subset has the same face count and boundary status as `sup`.
    pub fn extent_matches_superset(&self, sup: &FaceVertexSubset) -> bool {
        self.get_num_faces() == sup.get_num_faces() && self.is_boundary() == sup.is_boundary()
    }

    /// `true` when extent and sharpness both match `sup`.
    pub fn shape_matches_superset(&self, sup: &FaceVertexSubset) -> bool {
        self.extent_matches_superset(sup) && self.is_sharp() == sup.is_sharp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_single_face() {
        let mut s = FaceVertexSubset::default();
        let tag = VertexTag::default();
        s.initialize(tag);
        assert_eq!(s.get_num_faces(), 1);
        assert!(!s.is_boundary());
        assert!(!s.is_sharp());
    }

    #[test]
    fn extent_matches() {
        let mut a = FaceVertexSubset::default();
        let mut b = FaceVertexSubset::default();
        a.initialize(VertexTag::default());
        b.initialize(VertexTag::default());

        a.num_faces_total = 4;
        b.num_faces_total = 4;
        assert!(a.extent_matches_superset(&b));

        b.num_faces_total = 3;
        assert!(!a.extent_matches_superset(&b));
    }
}
