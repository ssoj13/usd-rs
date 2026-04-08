//! Vertex feature tags used internally by the Bfr surface assembly.
//!
//! Mirrors `Bfr::FeatureBits`, `Bfr::VertexTag` and `Bfr::MultiVertexTag`
//! from `vertexTag.h`.

/// Packed bit-field of topological features for one or more vertices.
///
/// Stored as a `u16` that is bit-cast to/from the struct fields on demand,
/// matching the C++ `FeatureBits::GetBits` / `SetBits` behaviour.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct FeatureBits(pub u16);

impl FeatureBits {
    // Bit positions (same order as the C++ bit-field declaration):
    const BOUNDARY_VERTS: u16 = 1 << 0;
    const INF_SHARP_VERTS: u16 = 1 << 1;
    const INF_SHARP_EDGES: u16 = 1 << 2;
    const INF_SHARP_DARTS: u16 = 1 << 3;
    const SEMI_SHARP_VERTS: u16 = 1 << 4;
    const SEMI_SHARP_EDGES: u16 = 1 << 5;
    const UN_COMMON_FACE_SIZES: u16 = 1 << 6;
    const IRREGULAR_FACE_SIZES: u16 = 1 << 7;
    const UN_ORDERED_FACES: u16 = 1 << 8;
    const NON_MANIFOLD_VERTS: u16 = 1 << 9;
    const BOUNDARY_NON_SHARP: u16 = 1 << 10;

    /// Return the raw bit representation.
    #[inline]
    pub fn get_bits(self) -> u16 {
        self.0
    }

    /// Overwrite from raw bits.
    #[inline]
    pub fn set_bits(&mut self, bits: u16) {
        self.0 = bits;
    }

    /// Clear all bits to zero.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    // ---------- individual bit accessors ------------------------------------

    #[inline]
    fn get(&self, mask: u16) -> bool {
        self.0 & mask != 0
    }
    #[inline]
    fn set_bit(&mut self, mask: u16, on: bool) {
        if on {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    #[inline]
    pub fn boundary_verts(&self) -> bool {
        self.get(Self::BOUNDARY_VERTS)
    }
    #[inline]
    pub fn inf_sharp_verts(&self) -> bool {
        self.get(Self::INF_SHARP_VERTS)
    }
    #[inline]
    pub fn inf_sharp_edges(&self) -> bool {
        self.get(Self::INF_SHARP_EDGES)
    }
    #[inline]
    pub fn inf_sharp_darts(&self) -> bool {
        self.get(Self::INF_SHARP_DARTS)
    }
    #[inline]
    pub fn semi_sharp_verts(&self) -> bool {
        self.get(Self::SEMI_SHARP_VERTS)
    }
    #[inline]
    pub fn semi_sharp_edges(&self) -> bool {
        self.get(Self::SEMI_SHARP_EDGES)
    }
    #[inline]
    pub fn un_common_face_sizes(&self) -> bool {
        self.get(Self::UN_COMMON_FACE_SIZES)
    }
    #[inline]
    pub fn irregular_face_sizes(&self) -> bool {
        self.get(Self::IRREGULAR_FACE_SIZES)
    }
    #[inline]
    pub fn un_ordered_faces(&self) -> bool {
        self.get(Self::UN_ORDERED_FACES)
    }
    #[inline]
    pub fn non_manifold_verts(&self) -> bool {
        self.get(Self::NON_MANIFOLD_VERTS)
    }
    #[inline]
    pub fn boundary_non_sharp(&self) -> bool {
        self.get(Self::BOUNDARY_NON_SHARP)
    }

    #[inline]
    pub fn set_boundary_verts(&mut self, on: bool) {
        self.set_bit(Self::BOUNDARY_VERTS, on)
    }
    #[inline]
    pub fn set_inf_sharp_verts(&mut self, on: bool) {
        self.set_bit(Self::INF_SHARP_VERTS, on)
    }
    #[inline]
    pub fn set_inf_sharp_edges(&mut self, on: bool) {
        self.set_bit(Self::INF_SHARP_EDGES, on)
    }
    #[inline]
    pub fn set_inf_sharp_darts(&mut self, on: bool) {
        self.set_bit(Self::INF_SHARP_DARTS, on)
    }
    #[inline]
    pub fn set_semi_sharp_verts(&mut self, on: bool) {
        self.set_bit(Self::SEMI_SHARP_VERTS, on)
    }
    #[inline]
    pub fn set_semi_sharp_edges(&mut self, on: bool) {
        self.set_bit(Self::SEMI_SHARP_EDGES, on)
    }
    #[inline]
    pub fn set_un_common_face_sizes(&mut self, on: bool) {
        self.set_bit(Self::UN_COMMON_FACE_SIZES, on)
    }
    #[inline]
    pub fn set_irregular_face_sizes(&mut self, on: bool) {
        self.set_bit(Self::IRREGULAR_FACE_SIZES, on)
    }
    #[inline]
    pub fn set_un_ordered_faces(&mut self, on: bool) {
        self.set_bit(Self::UN_ORDERED_FACES, on)
    }
    #[inline]
    pub fn set_non_manifold_verts(&mut self, on: bool) {
        self.set_bit(Self::NON_MANIFOLD_VERTS, on)
    }
    #[inline]
    pub fn set_boundary_non_sharp(&mut self, on: bool) {
        self.set_bit(Self::BOUNDARY_NON_SHARP, on)
    }
}

// ---------------------------------------------------------------------------

/// Tag describing exceptional topological features at a single corner vertex.
///
/// Mirrors `Bfr::VertexTag`.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct VertexTag(pub FeatureBits);

impl VertexTag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return raw bits.
    #[inline]
    pub fn get_bits(self) -> u16 {
        self.0.get_bits()
    }
    #[inline]
    pub fn set_bits(&mut self, b: u16) {
        self.0.set_bits(b)
    }
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    // Single-vertex queries (some invert the sense of the bit):
    #[inline]
    pub fn is_boundary(&self) -> bool {
        self.0.boundary_verts()
    }
    #[inline]
    pub fn is_interior(&self) -> bool {
        !self.0.boundary_verts()
    }
    #[inline]
    pub fn is_inf_sharp(&self) -> bool {
        self.0.inf_sharp_verts()
    }
    #[inline]
    pub fn has_inf_sharp_edges(&self) -> bool {
        self.0.inf_sharp_edges()
    }
    #[inline]
    pub fn is_inf_sharp_dart(&self) -> bool {
        self.0.inf_sharp_darts()
    }
    #[inline]
    pub fn is_semi_sharp(&self) -> bool {
        self.0.semi_sharp_verts()
    }
    #[inline]
    pub fn has_semi_sharp_edges(&self) -> bool {
        self.0.semi_sharp_edges()
    }
    #[inline]
    pub fn has_un_common_face_sizes(&self) -> bool {
        self.0.un_common_face_sizes()
    }
    #[inline]
    pub fn has_irregular_face_sizes(&self) -> bool {
        self.0.irregular_face_sizes()
    }
    #[inline]
    pub fn is_ordered(&self) -> bool {
        !self.0.un_ordered_faces()
    }
    #[inline]
    pub fn is_un_ordered(&self) -> bool {
        self.0.un_ordered_faces()
    }
    #[inline]
    pub fn is_manifold(&self) -> bool {
        !self.0.non_manifold_verts()
    }
    #[inline]
    pub fn is_non_manifold(&self) -> bool {
        self.0.non_manifold_verts()
    }
    #[inline]
    pub fn has_non_sharp_boundary(&self) -> bool {
        self.0.boundary_non_sharp()
    }
    #[inline]
    pub fn has_sharp_edges(&self) -> bool {
        self.has_inf_sharp_edges() || self.has_semi_sharp_edges()
    }

    // Expose inner bits mutably for FaceVertex / FaceVertexSubset:
    #[inline]
    pub fn bits_mut(&mut self) -> &mut FeatureBits {
        &mut self.0
    }
    #[inline]
    pub fn bits(&self) -> &FeatureBits {
        &self.0
    }
}

// ---------------------------------------------------------------------------

/// Tag combining features from multiple corners via bitwise-OR.
///
/// Mirrors `Bfr::MultiVertexTag`.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MultiVertexTag(pub FeatureBits);

impl MultiVertexTag {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn get_bits(self) -> u16 {
        self.0.get_bits()
    }
    #[inline]
    pub fn set_bits(&mut self, b: u16) {
        self.0.set_bits(b)
    }
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    // Collective queries:
    #[inline]
    pub fn has_boundary_vertices(&self) -> bool {
        self.0.boundary_verts()
    }
    #[inline]
    pub fn has_inf_sharp_vertices(&self) -> bool {
        self.0.inf_sharp_verts()
    }
    #[inline]
    pub fn has_inf_sharp_edges(&self) -> bool {
        self.0.inf_sharp_edges()
    }
    #[inline]
    pub fn has_inf_sharp_darts(&self) -> bool {
        self.0.inf_sharp_darts()
    }
    #[inline]
    pub fn has_semi_sharp_vertices(&self) -> bool {
        self.0.semi_sharp_verts()
    }
    #[inline]
    pub fn has_semi_sharp_edges(&self) -> bool {
        self.0.semi_sharp_edges()
    }
    #[inline]
    pub fn has_un_common_face_sizes(&self) -> bool {
        self.0.un_common_face_sizes()
    }
    #[inline]
    pub fn has_irregular_face_sizes(&self) -> bool {
        self.0.irregular_face_sizes()
    }
    #[inline]
    pub fn has_un_ordered_vertices(&self) -> bool {
        self.0.un_ordered_faces()
    }
    #[inline]
    pub fn has_non_manifold_vertices(&self) -> bool {
        self.0.non_manifold_verts()
    }
    #[inline]
    pub fn has_non_sharp_boundary(&self) -> bool {
        self.0.boundary_non_sharp()
    }
    #[inline]
    pub fn has_sharp_vertices(&self) -> bool {
        self.has_inf_sharp_vertices() || self.has_semi_sharp_vertices()
    }
    #[inline]
    pub fn has_sharp_edges(&self) -> bool {
        self.has_inf_sharp_edges() || self.has_semi_sharp_edges()
    }

    /// Combine (bitwise-OR) a `VertexTag` into this multi-tag.
    #[inline]
    pub fn combine(&mut self, tag: VertexTag) {
        self.0.set_bits(self.0.get_bits() | tag.get_bits());
    }

    #[inline]
    pub fn bits_mut(&mut self) -> &mut FeatureBits {
        &mut self.0
    }
    #[inline]
    pub fn bits(&self) -> &FeatureBits {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_bits_roundtrip() {
        let mut fb = FeatureBits::default();
        fb.set_boundary_verts(true);
        fb.set_inf_sharp_edges(true);
        assert!(fb.boundary_verts());
        assert!(fb.inf_sharp_edges());
        assert!(!fb.semi_sharp_verts());

        let bits = fb.get_bits();
        let mut fb2 = FeatureBits::default();
        fb2.set_bits(bits);
        assert_eq!(fb, fb2);
    }

    #[test]
    fn vertex_tag_queries() {
        let mut tag = VertexTag::default();
        assert!(tag.is_interior());
        assert!(tag.is_ordered());
        assert!(tag.is_manifold());

        tag.0.set_boundary_verts(true);
        assert!(tag.is_boundary());
        assert!(!tag.is_interior());
    }

    #[test]
    fn multi_vertex_tag_combine() {
        let mut mv = MultiVertexTag::default();
        assert!(!mv.has_boundary_vertices());

        let mut vt = VertexTag::default();
        vt.0.set_boundary_verts(true);
        vt.0.set_inf_sharp_edges(true);

        mv.combine(vt);
        assert!(mv.has_boundary_vertices());
        assert!(mv.has_inf_sharp_edges());
        assert!(!mv.has_semi_sharp_vertices());
    }
}
