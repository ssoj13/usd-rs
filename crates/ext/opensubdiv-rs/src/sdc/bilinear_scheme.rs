// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/bilinearScheme.h

use super::crease::Rule;
use super::options::Options;
use super::scheme::{
    EdgeNeighborhood, MaskInterface, SchemeKernel, VertexNeighborhood,
    assign_corner_mask_for_vertex_common, assign_crease_mask_for_edge_common,
};
use super::types::Split;

/// Kernel implementing the Bilinear subdivision scheme.
pub struct BilinearKernel;

impl SchemeKernel for BilinearKernel {
    // ── Traits ────────────────────────────────────────────────────────────────

    #[inline]
    fn topological_split_type() -> Split {
        Split::ToQuads
    }
    #[inline]
    fn regular_face_size() -> i32 {
        4
    }
    #[inline]
    fn regular_vertex_valence() -> i32 {
        4
    }
    #[inline]
    fn local_neighborhood_size() -> i32 {
        0
    }

    // ── Full-override hooks (S2 fix) ──────────────────────────────────────────
    //
    // C++ bilinearScheme.h provides full specialisations of both
    // `ComputeEdgeVertexMask` and `ComputeVertexVertexMask` that completely
    // bypass the generic crease/sharpness logic in the base scheme.  Bilinear
    // ignores ALL sharpness values:
    //   - edge-vertex: always the crease midpoint (0.5, 0.5)
    //   - vertex-vertex: always the corner identity (1.0)
    //
    // These two overrides replicate those C++ specialisations exactly.

    /// S2 fix: bypass all sharpness/Rule logic.  Always assign the midpoint mask.
    ///
    /// C++ `Scheme<SCHEME_BILINEAR>::ComputeEdgeVertexMask` ignores `parentRule`,
    /// `childRule`, and the edge sharpness entirely -- it directly calls
    /// `assignCreaseMaskForEdge` (which sets vw0=0.5, vw1=0.5).
    #[inline]
    fn override_compute_edge_vertex_mask<E: EdgeNeighborhood, M: MaskInterface>(
        _options: &Options,
        edge: &E,
        mask: &mut M,
        _p_rule: Rule,
        _c_rule: Rule,
    ) -> bool {
        assign_crease_mask_for_edge_common(edge, mask);
        true
    }

    /// S2 fix: bypass all sharpness/Rule logic.  Always assign the identity mask.
    ///
    /// C++ `Scheme<SCHEME_BILINEAR>::ComputeVertexVertexMask` ignores `parentRule`,
    /// `childRule`, and all sharpness values -- it directly calls
    /// `assignCornerMaskForVertex` (which sets vw0=1.0, no edge/face weights).
    #[inline]
    fn override_compute_vertex_vertex_mask<V: VertexNeighborhood, M: MaskInterface>(
        _options: &Options,
        vertex: &V,
        mask: &mut M,
        _p_rule: Rule,
        _c_rule: Rule,
    ) -> bool {
        assign_corner_mask_for_vertex_common(vertex, mask);
        true
    }

    // ── Edge-vertex masks ─────────────────────────────────────────────────────
    //
    // These are called by the generic base logic (which bilinear bypasses via
    // the overrides above), but must still be implemented since they are
    // required by the SchemeKernel trait.

    /// Bilinear crease: midpoint (0.5, 0.5) -- shared with all schemes.
    fn assign_crease_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        _opts: &Options,
        edge: &E,
        mask: &mut M,
    ) {
        assign_crease_mask_for_edge_common(edge, mask);
    }

    /// Bilinear smooth: same as crease (bilinear has no smooth edge rule).
    fn assign_smooth_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        opts: &Options,
        edge: &E,
        mask: &mut M,
    ) {
        // Bilinear defers to crease -- the ComputeEdgeVertexMask specialisation
        // in C++ directly calls assignCreaseMaskForEdge.
        Self::assign_crease_mask_for_edge(opts, edge, mask);
    }

    // ── Vertex-vertex masks ───────────────────────────────────────────────────

    /// Bilinear corner: identity (1.0).
    fn assign_corner_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options,
        vertex: &V,
        mask: &mut M,
    ) {
        assign_corner_mask_for_vertex_common(vertex, mask);
    }

    /// Bilinear crease: identity (same as corner for bilinear).
    fn assign_crease_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options,
        vertex: &V,
        mask: &mut M,
        _crease_ends: [usize; 2],
    ) {
        Self::assign_corner_mask_for_vertex(opts, vertex, mask);
    }

    /// Bilinear smooth: identity (same as corner for bilinear).
    fn assign_smooth_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options,
        vertex: &V,
        mask: &mut M,
    ) {
        Self::assign_corner_mask_for_vertex(opts, vertex, mask);
    }

    // ── Limit position masks ──────────────────────────────────────────────────

    /// Bilinear corner limit: identity (vertex limit = refined vertex).
    fn assign_corner_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options,
        _vertex: &V,
        mask: &mut M,
    ) {
        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(0);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);
        mask.set_vertex_weight(0, 1.0);
    }

    /// Bilinear crease limit: same as corner.
    fn assign_crease_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options,
        vertex: &V,
        mask: &mut M,
        _crease_ends: [usize; 2],
    ) {
        Self::assign_corner_limit_mask(opts, vertex, mask);
    }

    /// Bilinear smooth limit: same as corner.
    fn assign_smooth_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options,
        vertex: &V,
        mask: &mut M,
    ) {
        Self::assign_corner_limit_mask(opts, vertex, mask);
    }

    // ── Limit tangent masks ───────────────────────────────────────────────────

    /// Bilinear corner tangents: differences along the first two incident edges.
    ///
    /// tan1 = e0 - v,  tan2 = e1 - v
    ///
    /// Mirrors the C++ specialisation which uses 2 edge weights regardless of
    /// actual valence.
    fn assign_corner_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options,
        _vertex: &V,
        tan1: &mut M,
        tan2: &mut M,
    ) {
        for m in [&mut *tan1, &mut *tan2] {
            m.set_num_vertex_weights(1);
            m.set_num_edge_weights(2);
            m.set_num_face_weights(0);
            m.set_face_weights_for_face_centers(false);
        }

        tan1.set_vertex_weight(0, -1.0);
        tan1.set_edge_weight(0, 1.0);
        tan1.set_edge_weight(1, 0.0);

        tan2.set_vertex_weight(0, -1.0);
        tan2.set_edge_weight(0, 0.0);
        tan2.set_edge_weight(1, 1.0);
    }

    /// Bilinear crease tangents: same as corner.
    fn assign_crease_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options,
        vertex: &V,
        tan1: &mut M,
        tan2: &mut M,
        _crease_ends: [usize; 2],
    ) {
        Self::assign_corner_limit_tangent_masks(opts, vertex, tan1, tan2);
    }

    /// Bilinear smooth tangents: same as corner.
    fn assign_smooth_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options,
        vertex: &V,
        tan1: &mut M,
        tan2: &mut M,
    ) {
        Self::assign_corner_limit_tangent_masks(opts, vertex, tan1, tan2);
    }
}

/// Type alias for the Bilinear scheme.
pub type BilinearScheme = super::scheme::Scheme<BilinearKernel>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::crease::Rule;
    use crate::sdc::scheme::WeightMask;

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct DummyEdge {
        sharpness: f32,
    }
    impl EdgeNeighborhood for DummyEdge {
        fn num_faces(&self) -> usize {
            2
        }
        fn sharpness(&self) -> f32 {
            self.sharpness
        }
        fn num_vertices_per_face(&self, _: &mut [usize]) {}
        fn child_sharpnesses(&self, _: &super::super::crease::Crease, out: &mut [f32; 2]) {
            out[0] = 0.0;
            out[1] = 0.0;
        }
    }

    struct DummyVertex {
        num_edges: usize,
        sharpness: f32,
    }
    impl VertexNeighborhood for DummyVertex {
        fn num_edges(&self) -> usize {
            self.num_edges
        }
        fn num_faces(&self) -> usize {
            self.num_edges
        }
        fn sharpness(&self) -> f32 {
            self.sharpness
        }
        fn sharpness_per_edge<'a>(&self, out: &'a mut [f32]) -> &'a [f32] {
            for s in out.iter_mut() {
                *s = 0.0;
            }
            out
        }
        fn child_sharpness(&self, _: &super::super::crease::Crease) -> f32 {
            0.0
        }
        fn child_sharpness_per_edge<'a>(
            &self,
            _: &super::super::crease::Crease,
            out: &'a mut [f32],
        ) -> &'a [f32] {
            for s in out.iter_mut() {
                *s = 0.0;
            }
            out
        }
    }

    // ── Edge-vertex ──────────────────────────────────────────────────────────

    #[test]
    fn edge_vertex_smooth_is_midpoint() {
        let scheme = BilinearScheme::new();
        let edge = DummyEdge { sharpness: 0.0 };
        let mut mask = WeightMask::new(2, 0, 0);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        assert_eq!(mask.num_vertex_weights(), 2);
        assert!((mask.vertex_weight(0) - 0.5).abs() < 1e-6);
        assert!((mask.vertex_weight(1) - 0.5).abs() < 1e-6);
    }

    /// S2 regression: even a sharp edge must produce the midpoint for bilinear,
    /// not a crease mask from the generic sharpness path.
    #[test]
    fn edge_vertex_crease_is_still_midpoint() {
        let scheme = BilinearScheme::new();
        let edge = DummyEdge { sharpness: 5.0 };
        let mut mask = WeightMask::new(2, 0, 0);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        // Bilinear always returns the midpoint regardless of sharpness.
        assert!(
            (mask.vertex_weight(0) - 0.5).abs() < 1e-6,
            "vw0 = {} (bilinear must ignore sharpness)",
            mask.vertex_weight(0)
        );
        assert!((mask.vertex_weight(1) - 0.5).abs() < 1e-6);
        // No face weights (crease mask has none)
        assert_eq!(mask.num_face_weights(), 0);
    }

    // ── Vertex-vertex ────────────────────────────────────────────────────────

    #[test]
    fn vertex_vertex_is_identity() {
        let scheme = BilinearScheme::new();
        let v = DummyVertex {
            num_edges: 4,
            sharpness: 0.0,
        };
        let mut mask = WeightMask::new(1, 4, 4);
        scheme.compute_vertex_vertex_mask(&v, &mut mask, Rule::Unknown, Rule::Unknown);

        assert_eq!(mask.num_vertex_weights(), 1);
        assert!((mask.vertex_weight(0) - 1.0).abs() < 1e-6);
        assert_eq!(mask.num_edge_weights(), 0);
        assert_eq!(mask.num_face_weights(), 0);
    }

    /// S2 regression: a sharp/crease vertex must still produce the identity mask
    /// for bilinear -- sharpness must be completely ignored.
    #[test]
    fn vertex_vertex_sharp_is_still_identity() {
        let scheme = BilinearScheme::new();
        // Simulate a fully sharp corner vertex
        let v = DummyVertex {
            num_edges: 4,
            sharpness: 10.0,
        };
        let mut mask = WeightMask::new(1, 4, 4);
        scheme.compute_vertex_vertex_mask(&v, &mut mask, Rule::Corner, Rule::Corner);

        assert!(
            (mask.vertex_weight(0) - 1.0).abs() < 1e-6,
            "vw = {} (bilinear must ignore sharpness and return identity)",
            mask.vertex_weight(0)
        );
        assert_eq!(
            mask.num_edge_weights(),
            0,
            "bilinear vertex-vertex must have 0 edge weights (identity)"
        );
        assert_eq!(mask.num_face_weights(), 0);
    }

    /// S2 regression: bilinear bypass must work regardless of what Rule is passed.
    /// Any Rule combination must always yield the identity mask.
    #[test]
    fn vertex_vertex_any_rule_is_identity() {
        let scheme = BilinearScheme::new();
        let v = DummyVertex {
            num_edges: 3,
            sharpness: 0.5,
        };

        for p_rule in [
            Rule::Unknown,
            Rule::Smooth,
            Rule::Dart,
            Rule::Crease,
            Rule::Corner,
        ] {
            for c_rule in [
                Rule::Unknown,
                Rule::Smooth,
                Rule::Dart,
                Rule::Crease,
                Rule::Corner,
            ] {
                let mut mask = WeightMask::new(1, 3, 3);
                scheme.compute_vertex_vertex_mask(&v, &mut mask, p_rule, c_rule);
                assert!(
                    (mask.vertex_weight(0) - 1.0).abs() < 1e-6,
                    "vw = {} for p={:?} c={:?} (bilinear must always return identity)",
                    mask.vertex_weight(0),
                    p_rule,
                    c_rule
                );
                assert_eq!(mask.num_edge_weights(), 0);
                assert_eq!(mask.num_face_weights(), 0);
            }
        }
    }

    // ── Limit position ───────────────────────────────────────────────────────

    #[test]
    fn limit_position_is_identity() {
        let scheme = BilinearScheme::new();
        let v = DummyVertex {
            num_edges: 4,
            sharpness: 0.0,
        };
        let mut mask = WeightMask::new(1, 0, 0);
        scheme.compute_vertex_limit_mask(&v, &mut mask, Rule::Corner);

        assert!((mask.vertex_weight(0) - 1.0).abs() < 1e-6);
    }
}
