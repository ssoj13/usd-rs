// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/catmarkScheme.h

use std::f64::consts::PI;

use super::options::{Options, TriangleSubdivision};
use super::scheme::{
    assign_corner_mask_for_vertex_common, assign_crease_mask_for_edge_common,
    EdgeNeighborhood, MaskInterface, SchemeKernel, VertexNeighborhood, Weight,
};
use super::types::Split;

/// Kernel implementing the Catmull-Clark subdivision scheme.
pub struct CatmarkKernel;

impl SchemeKernel for CatmarkKernel {
    // ── Traits ────────────────────────────────────────────────────────────────

    #[inline] fn topological_split_type() -> Split { Split::ToQuads }
    #[inline] fn regular_face_size()       -> i32  { 4 }
    #[inline] fn regular_vertex_valence()  -> i32  { 4 }
    #[inline] fn local_neighborhood_size() -> i32  { 1 }

    // ── Edge-vertex masks ─────────────────────────────────────────────────────

    fn assign_crease_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        _opts: &Options, edge: &E, mask: &mut M,
    ) {
        assign_crease_mask_for_edge_common(edge, mask);
    }

    /// Catmark smooth edge-vertex: average of 2 end-vertices + 2 face-centres
    /// (or proportional face weights for non-manifold edges).
    ///
    /// With TRI_SUB_SMOOTH the weights are adjusted for incident triangles.
    fn assign_smooth_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        opts: &Options, edge: &E, mask: &mut M,
    ) {
        let face_count = edge.num_faces();

        mask.set_num_vertex_weights(2);
        mask.set_num_edge_weights(0);
        mask.set_num_face_weights(face_count);
        mask.set_face_weights_for_face_centers(true);

        // Determine if triangle-subdivision option is active for this edge
        let mut face0_is_tri = false;
        let mut face1_is_tri = false;
        let mut use_tri_opt  = opts.get_triangle_subdivision() == TriangleSubdivision::Smooth;

        if use_tri_opt {
            if face_count == 2 {
                let mut verts = [0usize; 2];
                edge.num_vertices_per_face(&mut verts);
                face0_is_tri = verts[0] == 3;
                face1_is_tri = verts[1] == 3;
                use_tri_opt  = face0_is_tri || face1_is_tri;
            } else {
                use_tri_opt = false;
            }
        }

        if !use_tri_opt {
            mask.set_vertex_weight(0, 0.25);
            mask.set_vertex_weight(1, 0.25);

            if face_count == 2 {
                mask.set_face_weight(0, 0.25);
                mask.set_face_weight(1, 0.25);
            } else {
                let fw = 0.5 / face_count as Weight;
                for i in 0..face_count {
                    mask.set_face_weight(i, fw);
                }
            }
        } else {
            // Hbr-matching tri-subdivision weights
            const CATMARK_SMOOTH_TRI_EDGE_WEIGHT: Weight = 0.470;

            let f0w: Weight = if face0_is_tri { CATMARK_SMOOTH_TRI_EDGE_WEIGHT } else { 0.25 };
            let f1w: Weight = if face1_is_tri { CATMARK_SMOOTH_TRI_EDGE_WEIGHT } else { 0.25 };

            let fw  = 0.5 * (f0w + f1w);
            let vw  = 0.5 * (1.0 - 2.0 * fw);

            mask.set_vertex_weight(0, vw);
            mask.set_vertex_weight(1, vw);
            mask.set_face_weight(0, fw);
            mask.set_face_weight(1, fw);
        }
    }

    // ── Vertex-vertex masks ───────────────────────────────────────────────────

    fn assign_corner_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M,
    ) {
        assign_corner_mask_for_vertex_common(vertex, mask);
    }

    /// Catmark crease vertex: 3/4 self + 1/8 each of the two crease-end edges.
    fn assign_crease_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M, crease_ends: [usize; 2],
    ) {
        let valence = vertex.num_edges();

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        mask.set_vertex_weight(0, 0.75);
        for i in 0..valence {
            mask.set_edge_weight(i, 0.0);
        }
        mask.set_edge_weight(crease_ends[0], 0.125);
        mask.set_edge_weight(crease_ends[1], 0.125);
    }

    /// Catmark smooth vertex: (n-2)/n self + 1/n² each edge + face mid.
    ///
    /// The vertex must be manifold and interior (`num_faces == num_edges`).
    fn assign_smooth_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M,
    ) {
        // Smooth is only valid for interior manifold vertices
        debug_assert_eq!(vertex.num_faces(), vertex.num_edges());

        let valence = vertex.num_faces() as Weight;
        let n       = valence as usize;

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(n);
        mask.set_num_face_weights(n);
        mask.set_face_weights_for_face_centers(true);

        let v_weight = (valence - 2.0) / valence;
        let f_weight = 1.0 / (valence * valence);
        let e_weight = f_weight;

        mask.set_vertex_weight(0, v_weight);
        for i in 0..n {
            mask.set_edge_weight(i, e_weight);
            mask.set_face_weight(i, f_weight);
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

    /// Catmark crease limit: (2/3) self + (1/6) each crease-end edge.
    fn assign_crease_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, mask: &mut M, crease_ends: [usize; 2],
    ) {
        let valence = vertex.num_edges();

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        mask.set_vertex_weight(0, 2.0 / 3.0);
        for i in 0..valence {
            mask.set_edge_weight(i, 0.0);
        }
        mask.set_edge_weight(crease_ends[0], 1.0 / 6.0);
        mask.set_edge_weight(crease_ends[1], 1.0 / 6.0);
    }

    /// Catmark smooth limit position.
    ///
    /// Specialised for valence 4 (regular); general formula for other valences.
    fn assign_smooth_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options, vertex: &V, mask: &mut M,
    ) {
        let valence = vertex.num_faces();
        if valence == 2 {
            // Degenerate — fall back to corner
            Self::assign_corner_limit_mask(opts, vertex, mask);
            return;
        }

        mask.set_num_vertex_weights(1);
        mask.set_num_edge_weights(valence);
        mask.set_num_face_weights(valence);
        mask.set_face_weights_for_face_centers(false);

        if valence == 4 {
            let f_weight: Weight = 1.0 / 36.0;
            let e_weight: Weight = 1.0 / 9.0;
            let v_weight: Weight = 4.0 / 9.0;

            mask.set_vertex_weight(0, v_weight);
            for i in 0..4 {
                mask.set_edge_weight(i, e_weight);
                mask.set_face_weight(i, f_weight);
            }
        } else {
            let n = valence as Weight;
            let f_weight = 1.0 / (n * (n + 5.0));
            let e_weight = 4.0 * f_weight;
            let v_weight = 1.0 - n * (e_weight + f_weight);

            mask.set_vertex_weight(0, v_weight);
            for i in 0..valence {
                mask.set_edge_weight(i, e_weight);
                mask.set_face_weight(i, f_weight);
            }
        }
    }

    // ── Limit tangent masks ───────────────────────────────────────────────────

    /// Catmark corner tangents: differences along first two incident edges.
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

        tan1.set_vertex_weight(0, -1.0);
        tan1.set_edge_weight(0,   1.0);
        tan1.set_edge_weight(1,   0.0);

        tan2.set_vertex_weight(0, -1.0);
        tan2.set_edge_weight(0,   0.0);
        tan2.set_edge_weight(1,   1.0);

        // Zero out remaining edge weights
        for i in 2..valence {
            tan1.set_edge_weight(i, 0.0);
            tan2.set_edge_weight(i, 0.0);
        }
    }

    /// Catmark crease limit tangents.
    ///
    /// tan1 — along the crease (leading edge direction)
    /// tan2 — across the interior faces (Biermann et al. formula for irregular)
    fn assign_crease_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        _opts: &Options, vertex: &V, tan1: &mut M, tan2: &mut M,
        crease_ends: [usize; 2],
    ) {
        let num_edges = vertex.num_edges();
        let num_faces = vertex.num_faces();

        // ── tan1: along crease ────────────────────────────────────────────
        tan1.set_num_vertex_weights(1);
        tan1.set_num_edge_weights(num_edges);
        tan1.set_num_face_weights(num_faces);
        tan1.set_face_weights_for_face_centers(false);

        tan1.set_vertex_weight(0, 0.0);
        for i in 0..num_edges { tan1.set_edge_weight(i, 0.0); }
        for i in 0..num_faces { tan1.set_face_weight(i, 0.0); }

        tan1.set_edge_weight(crease_ends[0],  0.5);
        tan1.set_edge_weight(crease_ends[1], -0.5);

        // ── tan2: across interior ─────────────────────────────────────────
        tan2.set_num_vertex_weights(1);
        tan2.set_num_edge_weights(num_edges);
        tan2.set_num_face_weights(num_faces);
        tan2.set_face_weights_for_face_centers(false);

        // Zero preceding the crease
        for i in 0..crease_ends[0] {
            tan2.set_edge_weight(i, 0.0);
            tan2.set_face_weight(i, 0.0);
        }

        let interior = crease_ends[1] - crease_ends[0] - 1;
        if interior == 1 {
            // Regular case: uniform B-spline cross-tangent
            tan2.set_vertex_weight(0, -4.0 / 6.0);
            tan2.set_edge_weight(crease_ends[0],     -1.0 / 6.0);
            tan2.set_edge_weight(crease_ends[0] + 1,  4.0 / 6.0);
            tan2.set_edge_weight(crease_ends[1],     -1.0 / 6.0);
            tan2.set_face_weight(crease_ends[0],      1.0 / 6.0);
            tan2.set_face_weight(crease_ends[0] + 1,  1.0 / 6.0);
        } else if interior > 1 {
            // Irregular: Biermann et al.
            let k          = interior as f64 + 1.0;
            let theta      = PI / k;
            let cos_theta  = theta.cos();
            let sin_theta  = theta.sin();

            let common_denom = 1.0 / (k * (3.0 + cos_theta));
            let r            = (cos_theta + 1.0) / sin_theta;

            let vertex_w = 4.0 * r * (cos_theta - 1.0);
            let crease_w = -r * (1.0 + 2.0 * cos_theta);

            tan2.set_vertex_weight(0, (vertex_w * common_denom) as Weight);
            tan2.set_edge_weight(crease_ends[0], (crease_w * common_denom) as Weight);
            tan2.set_edge_weight(crease_ends[1], (crease_w * common_denom) as Weight);
            tan2.set_face_weight(crease_ends[0], (sin_theta * common_denom) as Weight);

            #[allow(unused_assignments)]
            let mut sin_i       = 0.0f64;
            let mut sin_i_plus1 = sin_theta;
            for i in 1..(k as usize) {
                sin_i       = sin_i_plus1;
                sin_i_plus1 = ((i + 1) as f64 * theta).sin();

                tan2.set_edge_weight(
                    crease_ends[0] + i,
                    (4.0 * sin_i * common_denom) as Weight,
                );
                tan2.set_face_weight(
                    crease_ends[0] + i,
                    ((sin_i + sin_i_plus1) * common_denom) as Weight,
                );
            }
        } else {
            // Zero interior edges (one face): simple average of boundary edges
            tan2.set_vertex_weight(0, -6.0);
            tan2.set_edge_weight(crease_ends[0], 3.0);
            tan2.set_edge_weight(crease_ends[1], 3.0);
            tan2.set_face_weight(crease_ends[0], 0.0);
        }

        // Zero following the crease
        for i in crease_ends[1]..num_faces {
            tan2.set_face_weight(i, 0.0);
        }
        for i in (crease_ends[1] + 1)..num_edges {
            tan2.set_edge_weight(i, 0.0);
        }
    }

    /// Catmark smooth limit tangents.
    ///
    /// tan1 computed via sin/cos formula; tan2 is a 1-step rotation of tan1.
    fn assign_smooth_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        opts: &Options, vertex: &V, tan1: &mut M, tan2: &mut M,
    ) {
        let valence = vertex.num_faces();
        if valence == 2 {
            Self::assign_corner_limit_tangent_masks(opts, vertex, tan1, tan2);
            return;
        }

        // Build tan1
        tan1.set_num_vertex_weights(1);
        tan1.set_num_edge_weights(valence);
        tan1.set_num_face_weights(valence);
        tan1.set_face_weights_for_face_centers(false);
        tan1.set_vertex_weight(0, 0.0);

        if valence == 4 {
            tan1.set_edge_weight(0,  4.0);
            tan1.set_edge_weight(1,  0.0);
            tan1.set_edge_weight(2, -4.0);
            tan1.set_edge_weight(3,  0.0);

            tan1.set_face_weight(0,  1.0);
            tan1.set_face_weight(1, -1.0);
            tan1.set_face_weight(2, -1.0);
            tan1.set_face_weight(3,  1.0);
        } else {
            let theta      = 2.0 * PI / valence as f64;
            let cos_theta  = theta.cos();
            let cos_htheta = (theta * 0.5).cos();

            let lambda = (5.0 / 16.0)
                + (1.0 / 16.0) * (cos_theta + cos_htheta * (2.0 * (9.0 + cos_theta)).sqrt());

            let edge_scale = 4.0f64;
            let face_scale = 1.0 / (4.0 * lambda - 1.0);

            for i in 0..valence {
                let cos_i      = (i as f64 * theta).cos();
                let cos_i_plus = ((i + 1) as f64 * theta).cos();
                tan1.set_edge_weight(i, (edge_scale * cos_i) as Weight);
                tan1.set_face_weight(i, (face_scale * (cos_i + cos_i_plus)) as Weight);
            }
        }

        // tan2 is a 1-step rotation of tan1 (cyclic shift by -1)
        tan2.set_num_vertex_weights(1);
        tan2.set_num_edge_weights(valence);
        tan2.set_num_face_weights(valence);
        tan2.set_face_weights_for_face_centers(false);
        tan2.set_vertex_weight(0, 0.0);

        if valence == 4 {
            tan2.set_edge_weight(0,  0.0);
            tan2.set_edge_weight(1,  4.0);
            tan2.set_edge_weight(2,  0.0);
            tan2.set_edge_weight(3, -4.0);

            tan2.set_face_weight(0,  1.0);
            tan2.set_face_weight(1,  1.0);
            tan2.set_face_weight(2, -1.0);
            tan2.set_face_weight(3, -1.0);
        } else {
            // Cyclic shift: tan2[i] = tan1[i-1], wrapping
            tan2.set_edge_weight(0, tan1.edge_weight(valence - 1));
            tan2.set_face_weight(0, tan1.face_weight(valence - 1));
            for i in 1..valence {
                tan2.set_edge_weight(i, tan1.edge_weight(i - 1));
                tan2.set_face_weight(i, tan1.face_weight(i - 1));
            }
        }
    }
}

/// Type alias for the Catmull-Clark scheme.
pub type CatmarkScheme = super::scheme::Scheme<CatmarkKernel>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::scheme::WeightMask;
    use crate::sdc::crease::{Crease, Rule};

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct QuadEdge;
    impl EdgeNeighborhood for QuadEdge {
        fn num_faces(&self) -> usize { 2 }
        fn sharpness(&self) -> f32   { 0.0 }
        fn num_vertices_per_face(&self, verts: &mut [usize]) {
            verts[0] = 4; verts[1] = 4;
        }
        fn child_sharpnesses(&self, _: &Crease, out: &mut [f32; 2]) {
            out[0] = 0.0; out[1] = 0.0;
        }
    }

    struct TriEdge;
    impl EdgeNeighborhood for TriEdge {
        fn num_faces(&self) -> usize { 2 }
        fn sharpness(&self) -> f32   { 0.0 }
        fn num_vertices_per_face(&self, verts: &mut [usize]) {
            verts[0] = 3; verts[1] = 3;
        }
        fn child_sharpnesses(&self, _: &Crease, out: &mut [f32; 2]) {
            out[0] = 0.0; out[1] = 0.0;
        }
    }

    /// Interior manifold vertex with n edges/faces, all smooth.
    struct InteriorVertex { n: usize }
    impl VertexNeighborhood for InteriorVertex {
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

    // ── Edge-vertex smooth ────────────────────────────────────────────────────

    #[test]
    fn edge_smooth_quad_standard_weights() {
        let scheme = CatmarkScheme::new();
        let edge   = QuadEdge;
        let mut mask = WeightMask::new(2, 0, 2);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        // 0.25, 0.25, 0.25, 0.25
        assert!((mask.vertex_weight(0) - 0.25).abs() < 1e-6);
        assert!((mask.vertex_weight(1) - 0.25).abs() < 1e-6);
        assert!((mask.face_weight(0)   - 0.25).abs() < 1e-6);
        assert!((mask.face_weight(1)   - 0.25).abs() < 1e-6);
        assert!(mask.face_weights_for_face_centers());
    }

    #[test]
    fn edge_smooth_tri_smooth_option() {
        let mut opts = Options::default();
        opts.set_triangle_subdivision(TriangleSubdivision::Smooth);
        let scheme = CatmarkScheme::with_options(opts);
        let edge   = TriEdge;
        let mut mask = WeightMask::new(2, 0, 2);
        scheme.compute_edge_vertex_mask(&edge, &mut mask, Rule::Unknown, Rule::Unknown);

        // Both triangles → both use CATMARK_SMOOTH_TRI_EDGE_WEIGHT (0.470)
        // fw = 0.5*(0.47+0.47) = 0.47, vw = 0.5*(1 - 2*0.47) = 0.03
        let fw = 0.5 * (0.470 + 0.470);
        let vw = 0.5 * (1.0 - 2.0 * fw);
        assert!((mask.vertex_weight(0) - vw).abs() < 1e-5, "vw = {}", mask.vertex_weight(0));
        assert!((mask.face_weight(0)   - fw).abs() < 1e-5, "fw = {}", mask.face_weight(0));
    }

    // ── Vertex-vertex smooth ──────────────────────────────────────────────────

    #[test]
    fn vertex_smooth_regular_valence4() {
        let scheme = CatmarkScheme::new();
        let v      = InteriorVertex { n: 4 };
        let mut mask = WeightMask::new(1, 4, 4);
        scheme.compute_vertex_vertex_mask(&v, &mut mask, Rule::Smooth, Rule::Smooth);

        // (n-2)/n = 2/4 = 0.5
        assert!((mask.vertex_weight(0) - 0.5).abs() < 1e-6);
        // 1/(n*n) = 1/16
        for i in 0..4 {
            assert!((mask.edge_weight(i) - 1.0 / 16.0).abs() < 1e-6);
            assert!((mask.face_weight(i) - 1.0 / 16.0).abs() < 1e-6);
        }
    }

    #[test]
    fn vertex_smooth_valence3() {
        let scheme = CatmarkScheme::new();
        let v      = InteriorVertex { n: 3 };
        let mut mask = WeightMask::new(1, 3, 3);
        scheme.compute_vertex_vertex_mask(&v, &mut mask, Rule::Smooth, Rule::Smooth);

        // (n-2)/n = 1/3
        assert!((mask.vertex_weight(0) - 1.0 / 3.0).abs() < 1e-6);
        // 1/9
        for i in 0..3 {
            assert!((mask.edge_weight(i) - 1.0 / 9.0).abs() < 1e-6);
            assert!((mask.face_weight(i) - 1.0 / 9.0).abs() < 1e-6);
        }
        // weights sum to 1
        let sum = mask.vertex_weight(0)
            + (0..3).map(|i| mask.edge_weight(i) + mask.face_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {}", sum);
    }

    // ── Limit position ────────────────────────────────────────────────────────

    #[test]
    fn limit_smooth_regular_valence4() {
        let scheme = CatmarkScheme::new();
        let v      = InteriorVertex { n: 4 };
        let mut mask = WeightMask::new(1, 4, 4);
        scheme.compute_vertex_limit_mask(&v, &mut mask, Rule::Smooth);

        assert!((mask.vertex_weight(0) - 4.0 / 9.0).abs() < 1e-6);
        for i in 0..4 {
            assert!((mask.edge_weight(i) - 1.0 / 9.0).abs() < 1e-6);
            assert!((mask.face_weight(i) - 1.0 / 36.0).abs() < 1e-6);
        }
        // Limit weights must also sum to 1
        let sum = mask.vertex_weight(0)
            + (0..4).map(|i| mask.edge_weight(i) + mask.face_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5, "sum = {}", sum);
    }

    #[test]
    fn limit_crease_weights() {
        // `scheme` not used directly — kernel is called via static dispatch.
        let _scheme = CatmarkScheme::new();
        let v       = InteriorVertex { n: 4 };
        let mut mask = WeightMask::new(1, 4, 0);
        // Manually test the crease kernel directly
        CatmarkKernel::assign_crease_limit_mask(
            &Options::default(), &v, &mut mask, [0, 2],
        );

        assert!((mask.vertex_weight(0) - 2.0 / 3.0).abs() < 1e-6);
        assert!((mask.edge_weight(0)   - 1.0 / 6.0).abs() < 1e-6);
        assert!((mask.edge_weight(2)   - 1.0 / 6.0).abs() < 1e-6);
        assert!((mask.edge_weight(1)).abs() < 1e-6);
        assert!((mask.edge_weight(3)).abs() < 1e-6);

        let sum = mask.vertex_weight(0)
            + (0..4).map(|i| mask.edge_weight(i)).sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    // ── Limit tangents regular ────────────────────────────────────────────────

    #[test]
    fn limit_tangent_smooth_regular_valence4() {
        let scheme = CatmarkScheme::new();
        let v      = InteriorVertex { n: 4 };
        let mut pos  = WeightMask::new(1, 4, 4);
        let mut tan1 = WeightMask::new(1, 4, 4);
        let mut tan2 = WeightMask::new(1, 4, 4);
        scheme.compute_vertex_limit_mask_with_tangents(&v, &mut pos, &mut tan1, &mut tan2, Rule::Smooth);

        // tan1 vertex weight = 0
        assert!((tan1.vertex_weight(0)).abs() < 1e-6);
        // tan2 is tan1 rotated by one step
        assert!((tan2.edge_weight(0) - tan1.edge_weight(3)).abs() < 1e-6);
        assert!((tan2.edge_weight(1) - tan1.edge_weight(0)).abs() < 1e-6);
    }
}
