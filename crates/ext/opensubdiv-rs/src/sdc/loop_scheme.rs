// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/loopScheme.h

use std::f64::consts::PI;

use super::options::Options;
use super::scheme::{
    assign_corner_mask_for_vertex_common, assign_crease_mask_for_edge_common,
    EdgeNeighborhood, MaskInterface, SchemeKernel, VertexNeighborhood, Weight,
};
use super::types::Split;

/// Kernel implementing the Loop subdivision scheme.
pub struct LoopKernel;

impl SchemeKernel for LoopKernel {
    // ── Traits ────────────────────────────────────────────────────────────────

    #[inline] fn topological_split_type() -> Split { Split::ToTris }
    #[inline] fn regular_face_size()       -> i32  { 3 }
    #[inline] fn regular_vertex_valence()  -> i32  { 6 }
    #[inline] fn local_neighborhood_size() -> i32  { 1 }

    // ── Edge-vertex masks ─────────────────────────────────────────────────────

    fn assign_crease_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        _opts: &Options, edge: &E, mask: &mut M,
    ) {
        assign_crease_mask_for_edge_common(edge, mask);
    }

    /// Loop smooth edge-vertex: 3/8 x 2 end vertices + 1/8 x 2 opposite vertices.
    ///
    /// C++ loopScheme.h explicitly calls `SetFaceWeightsForFaceCenters(false)` before
    /// reading the flag, so the output is always vw=0.375, fw=0.125 regardless of any
    /// prior flag state on the mask.  Face weights represent *opposite vertices*, not
    /// face centres.
    fn assign_smooth_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        _opts: &Options, edge: &E, mask: &mut M,
    ) {
        let face_count = edge.num_faces();

        mask.set_num_vertex_weights(2);
        mask.set_num_edge_weights(0);
        mask.set_num_face_weights(face_count);
        // C++ sets this to false unconditionally before reading it — so it is always
        // false here.  Face weights are for *opposite vertices*, not face centres.
        mask.set_face_weights_for_face_centers(false);

        // With the flag forced to false: vw=0.375, fw=0.125 (the "opposite vertex" mode).
        // The symmetry note in the C++ comment (0.125/0.375 swapped for true) applies only
        // when the caller pre-sets the flag; in practice C++ always produces 0.375/0.125.
        let v_weight: Weight = 0.375;
        let f_weight: Weight = 0.125;

        mask.set_vertex_weight(0, v_weight);
        mask.set_vertex_weight(1, v_weight);

        if face_count == 2 {
            mask.set_face_weight(0, f_weight);
            mask.set_face_weight(1, f_weight);
        } else {
            // Non-manifold: scale face weight to preserve v/f ratio
            let fw = f_weight * 2.0 / face_count as Weight;
            for i in 0..face_count {
                mask.set_face_weight(i, fw);
            }
        }
    }

    // ── Vertex-vertex masks ───────────────────────────────────────────────────

    fn assign_corner_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M,
    ) {
        assign_corner_mask_for_vertex_common(vertex, mask);
    }

    /// Loop crease vertex: 3/4 self + 1/8 each crease-end edge.
    fn assign_crease_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M, crease_ends: [usize; 2],
    ) {
        let valence = vertex.num_edges();

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        mask.set_vertex_weight(0, 0.75);
        for i in 0..valence { mask.set_edge_weight(i, 0.0); }
        mask.set_edge_weight(crease_ends[0], 0.125);
        mask.set_edge_weight(crease_ends[1], 0.125);
    }

    /// Loop smooth vertex.
    ///
    /// Regular case (valence 6): vertex = 5/8, each edge = 1/16.
    /// Irregular case: uses the Warren/Levin formula.
    fn assign_smooth_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M,
    ) {
        let valence = vertex.num_faces();   // = num_edges for manifold interior

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        let (e_weight, v_weight): (Weight, Weight) = if valence == 6 {
            (0.0625, 0.625)
        } else {
            let n         = valence as f64;
            let inv_n     = 1.0 / n;
            let cos_theta = (PI * 2.0 * inv_n).cos();
            let beta      = 0.25 * cos_theta + 0.375;

            let ew = ((0.625 - beta * beta) * inv_n) as Weight;
            let vw = 1.0 - ew * valence as Weight;
            (ew, vw)
        };

        mask.set_vertex_weight(0, v_weight);
        for i in 0..valence {
            mask.set_edge_weight(i, e_weight);
        }
    }

    // ── Limit position masks ──────────────────────────────────────────────────

    fn assign_corner_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, _vertex: &V, mask: &mut M,
    ) {
        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(0);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);
        mask.set_vertex_weight(0, 1.0);
    }

    /// Loop crease limit: (4/6) self + (1/6) each crease-end edge.
    ///
    /// Produces a uniform B-spline curve along the crease for all valences.
    fn assign_crease_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M, crease_ends: [usize; 2],
    ) {
        let valence = vertex.num_edges();

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        let v_weight: Weight = 4.0 / 6.0;
        let e_weight: Weight = 1.0 / 6.0;

        mask.set_vertex_weight(0, v_weight);
        for i in 0..valence { mask.set_edge_weight(i, 0.0); }
        mask.set_edge_weight(crease_ends[0], e_weight);
        mask.set_edge_weight(crease_ends[1], e_weight);
    }

    /// Loop smooth limit position.
    ///
    /// Regular (valence 6): vertex = 1/2, each edge = 1/12.
    /// Irregular: generalised formula from Warren/Levin.
    fn assign_smooth_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M,
    ) {
        let valence = vertex.num_faces();

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        let (v_weight, e_weight): (Weight, Weight) = if valence == 6 {
            (0.5, 1.0 / 12.0)
        } else {
            let n         = valence as f64;
            let inv_n     = 1.0 / n;
            let cos_theta = (PI * 2.0 * inv_n).cos();
            let beta      = 0.25 * cos_theta + 0.375;
            let gamma     = (0.625 - beta * beta) * inv_n;

            let ew = (1.0 / (n + 3.0 / (8.0 * gamma))) as Weight;
            let vw = 1.0 - ew * valence as Weight;
            (vw, ew)
        };

        mask.set_vertex_weight(0, v_weight);
        for i in 0..valence {
            mask.set_edge_weight(i, e_weight);
        }
    }

    // ── Limit tangent masks ───────────────────────────────────────────────────

    /// Loop corner tangents: scale factor 3.0 versus the simpler -1/+1.
    ///
    /// tan1 = 3*(e0 - v),  tan2 = 3*(e1 - v)
    fn assign_corner_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, tan1: &mut M, tan2: &mut M,
    ) {
        let valence = vertex.num_edges();

        for m in [&mut *tan1, &mut *tan2] {
            m.set_num_vertex_weights(1);
            m.set_num_edge_weights(valence);
            m.set_num_face_weights(0);
            m.set_face_weights_for_face_centers(false);
        }

        tan1.set_vertex_weight(0, -3.0);
        tan1.set_edge_weight(0,   3.0);
        tan1.set_edge_weight(1,   0.0);

        tan2.set_vertex_weight(0, -3.0);
        tan2.set_edge_weight(0,   0.0);
        tan2.set_edge_weight(1,   3.0);

        for i in 2..valence {
            tan1.set_edge_weight(i, 0.0);
            tan2.set_edge_weight(i, 0.0);
        }
    }

    /// Loop crease limit tangents.
    ///
    /// tan1 -- along the crease (scale 1.5 for magnitude consistency)
    /// tan2 -- across interior, sign-corrected for consistent orientation
    fn assign_crease_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, tan1: &mut M, tan2: &mut M,
        crease_ends: [usize; 2],
    ) {
        let valence = vertex.num_edges();

        // ── tan1: along crease ────────────────────────────────────────────
        tan1.set_num_vertex_weights(1);
        tan1.set_num_edge_weights(valence);
        tan1.set_num_face_weights(0);
        tan1.set_face_weights_for_face_centers(false);

        tan1.set_vertex_weight(0, 0.0);
        for i in 0..valence { tan1.set_edge_weight(i, 0.0); }
        tan1.set_edge_weight(crease_ends[0],  1.5);
        tan1.set_edge_weight(crease_ends[1], -1.5);

        // ── tan2: across interior ─────────────────────────────────────────
        tan2.set_num_vertex_weights(1);
        tan2.set_num_edge_weights(valence);
        tan2.set_num_face_weights(0);
        tan2.set_face_weights_for_face_centers(false);

        for i in 0..crease_ends[0] { tan2.set_edge_weight(i, 0.0); }

        let interior = crease_ends[1] - crease_ends[0] - 1;

        if interior == 2 {
            // Regular case: sqrt(3)/2 scale
            const ROOT3: f64    = 1.732_050_807_568_877_29;
            const ROOT3BY2: Weight = (ROOT3 * 0.5) as Weight;

            tan2.set_vertex_weight(0, -(ROOT3 as Weight));

            tan2.set_edge_weight(crease_ends[0],     -ROOT3BY2);
            tan2.set_edge_weight(crease_ends[1],     -ROOT3BY2);
            tan2.set_edge_weight(crease_ends[0] + 1,  ROOT3 as Weight);
            tan2.set_edge_weight(crease_ends[0] + 2,  ROOT3 as Weight);
        } else if interior > 2 {
            // Irregular: general formula (-3.0 combined scale factor, see C++ comment)
            let theta = PI / (interior as f64 + 1.0);

            tan2.set_vertex_weight(0, 0.0);

            let c_weight = (-3.0 * theta.sin()) as Weight;
            tan2.set_edge_weight(crease_ends[0], c_weight);
            tan2.set_edge_weight(crease_ends[1], c_weight);

            let e_coeff = -3.0 * 2.0 * (theta.cos() - 1.0);
            for i in 1..=interior {
                let w = (e_coeff * (i as f64 * theta).sin()) as Weight;
                tan2.set_edge_weight(crease_ends[0] + i, w);
            }
        } else if interior == 1 {
            // One interior edge -- scale 3.0
            tan2.set_vertex_weight(0, -3.0);
            tan2.set_edge_weight(crease_ends[0],     0.0);
            tan2.set_edge_weight(crease_ends[1],     0.0);
            tan2.set_edge_weight(crease_ends[0] + 1, 3.0);
        } else {
            // Zero interior edges (one face) -- scale 3.0
            tan2.set_vertex_weight(0, -6.0);
            tan2.set_edge_weight(crease_ends[0], 3.0);
            tan2.set_edge_weight(crease_ends[1], 3.0);
        }

        for i in (crease_ends[1] + 1)..valence {
            tan2.set_edge_weight(i, 0.0);
        }
    }

    /// Loop smooth limit tangents using sin/cos formula.
    ///
    /// Regular (valence 6): tabulated values; otherwise 2*pi/n rotation formula.
    fn assign_smooth_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, tan1: &mut M, tan2: &mut M,
    ) {
        let valence = vertex.num_faces();

        for m in [&mut *tan1, &mut *tan2] {
            m.set_num_vertex_weights(1);
            m.set_num_edge_weights(valence);
            m.set_num_face_weights(0);
            m.set_face_weights_for_face_centers(false);
            m.set_vertex_weight(0, 0.0);
        }

        if valence == 6 {
            const ROOT3BY2: Weight = (1.732_050_807_568_877_29 * 0.5) as Weight;

            tan1.set_edge_weight(0,  1.0);
            tan1.set_edge_weight(1,  0.5);
            tan1.set_edge_weight(2, -0.5);
            tan1.set_edge_weight(3, -1.0);
            tan1.set_edge_weight(4, -0.5);
            tan1.set_edge_weight(5,  0.5);

            tan2.set_edge_weight(0,  0.0);
            tan2.set_edge_weight(1,  ROOT3BY2);
            tan2.set_edge_weight(2,  ROOT3BY2);
            tan2.set_edge_weight(3,  0.0);
            tan2.set_edge_weight(4, -ROOT3BY2);
            tan2.set_edge_weight(5, -ROOT3BY2);
        } else {
            let alpha = 2.0 * PI / valence as f64;
            for i in 0..valence {
                let ai = alpha * i as f64;
                tan1.set_edge_weight(i, ai.cos() as Weight);
                tan2.set_edge_weight(i, ai.sin() as Weight);
            }
        }
    }
}

/// Type alias for the Loop scheme.
pub type LoopScheme = super::scheme::Scheme<LoopKernel>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::scheme::WeightMask;
    use crate::sdc::crease::{Crease, Rule};

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct ManifoldEdge { sharpness: f32 }
    impl EdgeNeighborhood for ManifoldEdge {
        fn num_faces(&self) -> usize { 2 }
        fn sharpness(&self) -> f32   { self.sharpness }
        fn num_vertices_per_face(&self, _: &mut [usize]) {}
        fn child_sharpnesses(&self, _: &Crease, out: &mut [f32; 2]) {
            out[0] = 0.0; out[1] = 0.0;
        }
    }

    /// Smooth interior manifold vertex (num_edges == num_faces).
    struct SmoothVertex { n: usize }
    impl VertexNeighborhood for SmoothVertex {
        fn num_edges(&self) -> usize { self.n }
        fn num_faces(&self) -> usize { self.n }
        fn sharpness(&self) -> f32   { 0.0 }
        fn sharpness_per_edge<'a>(&self, out: &'a mut [f32]) -> &'a [f32] {
            for s in out.iter_mut() { *s = 0.0; }
            out
        }
        fn child_sharpness(&self, _: &Crease) -> f32 { 0.0 }
        fn child_sharpness_per_edge<'a>(&self, _: &Crease, out: &'a mut [f32]) -> &'a [f32] {
            for s in out.iter_mut() { *s = 0.0; }
            out
        }
    }

    // ── Edge-vertex ──────────────────────────────────────────────────────────

    #[test]
    fn edge_smooth_opposite_vertex_weights() {
        let scheme = LoopScheme::new();
        let edge   = ManifoldEdge { sharpness: 0.0 };
        let mut mask = WeightMask::new(2, 0, 2);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        // C++ always produces vw=0.375, fw=0.125 (opposite-vertex mode, flag forced false).
        assert!((mask.vertex_weight(0) - 0.375).abs() < 1e-6,
            "vw = {}", mask.vertex_weight(0));
        assert!((mask.vertex_weight(1) - 0.375).abs() < 1e-6);
        assert!((mask.face_weight(0) - 0.125).abs() < 1e-6,
            "fw = {}", mask.face_weight(0));
        assert!((mask.face_weight(1) - 0.125).abs() < 1e-6);

        // Flag must be false after the call (C++ sets it explicitly).
        assert!(!mask.face_weights_for_face_centers());

        // Total must sum to 1.0
        let sum = mask.vertex_weight(0) + mask.vertex_weight(1)
            + mask.face_weight(0)   + mask.face_weight(1);
        assert!((sum - 1.0).abs() < 1e-6, "sum = {}", sum);
    }

    /// S1 regression: even if the caller pre-sets face_weights_for_face_centers=true,
    /// C++ Loop resets it to false and always returns vw=0.375, fw=0.125.
    #[test]
    fn edge_smooth_flag_always_reset_to_false() {
        use crate::sdc::scheme::MaskInterface;
        let scheme = LoopScheme::new();
        let edge   = ManifoldEdge { sharpness: 0.0 };
        let mut mask = WeightMask::new(2, 0, 2);
        // Pre-set flag to true -- C++ overrides this to false unconditionally.
        mask.set_face_weights_for_face_centers(true);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        // Must always be opposite-vertex mode: vw=0.375, fw=0.125.
        assert!((mask.vertex_weight(0) - 0.375).abs() < 1e-6,
            "vw = {} (expected 0.375, flag must be overridden to false)", mask.vertex_weight(0));
        assert!((mask.face_weight(0) - 0.125).abs() < 1e-6,
            "fw = {} (expected 0.125, flag must be overridden to false)", mask.face_weight(0));
        // Flag must be false after the call.
        assert!(!mask.face_weights_for_face_centers(),
            "face_weights_for_face_centers must be false after Loop smooth edge mask");

        let sum = mask.vertex_weight(0) + mask.vertex_weight(1)
            + mask.face_weight(0)   + mask.face_weight(1);
        assert!((sum - 1.0).abs() < 1e-6, "sum = {}", sum);
    }

    #[test]
    fn edge_crease_is_midpoint() {
        let scheme = LoopScheme::new();
        let edge   = ManifoldEdge { sharpness: 5.0 };
        let mut mask = WeightMask::new(2, 0, 0);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        assert!((mask.vertex_weight(0) - 0.5).abs() < 1e-6);
        assert!((mask.vertex_weight(1) - 0.5).abs() < 1e-6);
    }

    // ── Vertex-vertex smooth ──────────────────────────────────────────────────

    #[test]
    fn vertex_smooth_regular_valence6() {
        let scheme = LoopScheme::new();
        let v      = SmoothVertex { n: 6 };
        let mut mask = WeightMask::new(1, 6, 0);
        scheme.compute_vertex_vertex_mask(&v, &mut mask, Rule::Smooth, Rule::Smooth);

        assert!((mask.vertex_weight(0) - 0.625).abs() < 1e-6);
        for i in 0..6 {
            assert!((mask.edge_weight(i) - 0.0625).abs() < 1e-6,
                "edge[{}] = {}", i, mask.edge_weight(i));
        }
        let sum = mask.vertex_weight(0)
            + (0..6).map(|i| mask.edge_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {}", sum);
    }

    #[test]
    fn vertex_smooth_irregular_valence5() {
        let scheme = LoopScheme::new();
        let v      = SmoothVertex { n: 5 };
        let mut mask = WeightMask::new(1, 5, 0);
        scheme.compute_vertex_vertex_mask(&v, &mut mask, Rule::Smooth, Rule::Smooth);

        // Weights should sum to 1
        let sum = mask.vertex_weight(0)
            + (0..5).map(|i| mask.edge_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {}", sum);
        // All edge weights equal
        let ew0 = mask.edge_weight(0);
        for i in 1..5 {
            assert!((mask.edge_weight(i) - ew0).abs() < 1e-6);
        }
    }

    // ── Limit position ────────────────────────────────────────────────────────

    #[test]
    fn limit_smooth_regular_valence6() {
        let scheme = LoopScheme::new();
        let v      = SmoothVertex { n: 6 };
        let mut mask = WeightMask::new(1, 6, 0);
        scheme.compute_vertex_limit_mask(&v, &mut mask, Rule::Smooth);

        assert!((mask.vertex_weight(0) - 0.5).abs() < 1e-6);
        for i in 0..6 {
            assert!((mask.edge_weight(i) - 1.0 / 12.0).abs() < 1e-6);
        }
        let sum = mask.vertex_weight(0)
            + (0..6).map(|i| mask.edge_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {}", sum);
    }

    #[test]
    fn limit_smooth_irregular_valence5() {
        let scheme = LoopScheme::new();
        let v      = SmoothVertex { n: 5 };
        let mut mask = WeightMask::new(1, 5, 0);
        scheme.compute_vertex_limit_mask(&v, &mut mask, Rule::Smooth);

        let sum = mask.vertex_weight(0)
            + (0..5).map(|i| mask.edge_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {}", sum);
    }

    #[test]
    fn limit_crease_sums_to_one() {
        let v = SmoothVertex { n: 4 };
        let mut mask = WeightMask::new(1, 4, 0);
        LoopKernel::assign_crease_limit_mask(
            &Options::default(), &v, &mut mask, [0, 2],
        );
        assert!((mask.vertex_weight(0) - 4.0 / 6.0).abs() < 1e-6);
        assert!((mask.edge_weight(0)   - 1.0 / 6.0).abs() < 1e-6);
        assert!((mask.edge_weight(2)   - 1.0 / 6.0).abs() < 1e-6);

        let sum = mask.vertex_weight(0)
            + (0..4).map(|i| mask.edge_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    // ── Limit tangents ────────────────────────────────────────────────────────

    #[test]
    fn tangent_smooth_regular_valence6() {
        let scheme = LoopScheme::new();
        let v      = SmoothVertex { n: 6 };
        let mut pos  = WeightMask::new(1, 6, 0);
        let mut tan1 = WeightMask::new(1, 6, 0);
        let mut tan2 = WeightMask::new(1, 6, 0);
        scheme.compute_vertex_limit_mask_with_tangents(&v, &mut pos, &mut tan1, &mut tan2, Rule::Smooth);

        // Vertex weights = 0
        assert!(tan1.vertex_weight(0).abs() < 1e-6);
        assert!(tan2.vertex_weight(0).abs() < 1e-6);

        // tan1 edge weights for valence-6: 1, 0.5, -0.5, -1, -0.5, 0.5
        let expected_tan1 = [1.0f32, 0.5, -0.5, -1.0, -0.5, 0.5];
        for (i, &e) in expected_tan1.iter().enumerate() {
            assert!((tan1.edge_weight(i) - e).abs() < 1e-5,
                "tan1 edge[{}]: got {}, expected {}", i, tan1.edge_weight(i), e);
        }
    }

    #[test]
    fn tangent_corner_scale_factor() {
        // Loop corner tangents use scale factor 3.0
        let scheme = LoopScheme::new();
        let v      = SmoothVertex { n: 4 };
        let mut pos  = WeightMask::new(1, 4, 0);
        let mut tan1 = WeightMask::new(1, 4, 0);
        let mut tan2 = WeightMask::new(1, 4, 0);
        scheme.compute_vertex_limit_mask_with_tangents(&v, &mut pos, &mut tan1, &mut tan2, Rule::Corner);

        assert!((tan1.vertex_weight(0) - (-3.0)).abs() < 1e-6);
        assert!((tan1.edge_weight(0)   - 3.0).abs() < 1e-6);
        assert!((tan1.edge_weight(1)).abs() < 1e-6);
    }
}
