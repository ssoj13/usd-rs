// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/crease.h + sdc/crease.cpp

use super::options::{CreasingMethod, Options, VtxBoundaryInterpolation};

/// Semi-sharp crease subdivision rule for a vertex.
///
/// Values are bit-positions so collections of vertex rules can be tested with
/// bitwise operations (mirrors C++ `Sdc::Crease::Rule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rule {
    Unknown = 0,
    Smooth  = 1 << 0,   // 1
    Dart    = 1 << 1,   // 2
    Crease  = 1 << 2,   // 4
    Corner  = 1 << 3,   // 8
}

impl Rule {
    /// Convert raw bits (as stored in `VTag::rule()` bits [10:7]) back to a `Rule`.
    #[inline]
    pub fn from_bits(bits: u8) -> Self {
        match bits {
            1 => Rule::Smooth,
            2 => Rule::Dart,
            4 => Rule::Crease,
            8 => Rule::Corner,
            _ => Rule::Unknown,
        }
    }

    /// Return the raw bit value.
    #[inline]
    pub fn bits(self) -> u8 { self as u8 }
}

/// Light-weight struct holding crease-related operations.
///
/// Constructed from an `Options` value; all methods are pure (no mutable state).
/// Mirrors the C++ `Sdc::Crease` class.
#[derive(Debug, Clone, Copy)]
pub struct Crease {
    options: Options,
}

// ── Sharpness constants ───────────────────────────────────────────────────────

/// A fully smooth sharpness value (0.0).
pub const SHARPNESS_SMOOTH: f32 = 0.0;

/// An infinitely sharp sharpness value (10.0).
pub const SHARPNESS_INFINITE: f32 = 10.0;

// ── Sharpness predicates (free functions mirroring C++ static methods) ────────
//
// C++ exposes these as `static` member methods of `Crease`, e.g. `Crease::IsSmooth(s)`.
// We provide BOTH free functions (for internal use) and associated functions on `Crease`
// (for public API parity).  See `impl Crease` below for the associated versions.

#[inline] pub fn is_smooth(s: f32)     -> bool { s <= SHARPNESS_SMOOTH }
#[inline] pub fn is_sharp(s: f32)      -> bool { s >  SHARPNESS_SMOOTH }
#[inline] pub fn is_infinite(s: f32)   -> bool { s >= SHARPNESS_INFINITE }
#[inline] pub fn is_semi_sharp(s: f32) -> bool { s > SHARPNESS_SMOOTH && s < SHARPNESS_INFINITE }

impl Crease {
    /// Construct with default options.
    #[inline]
    pub fn new() -> Self {
        Self { options: Options::default() }
    }

    /// Construct with explicit options.
    #[inline]
    pub fn with_options(options: Options) -> Self {
        Self { options }
    }

    #[inline]
    pub fn is_uniform(&self) -> bool {
        self.options.get_creasing_method() == CreasingMethod::Uniform
    }

    // ── Sharpness predicates as associated functions ──────────────────────────────
    //
    // C++ `Sdc::Crease` exposes these as `static` member methods so callers can
    // write `Crease::IsSmooth(s)`.  We mirror that here alongside the module-level
    // free functions (which remain for backward-compat and internal use).

    /// `true` when `sharpness <= SHARPNESS_SMOOTH` (0.0). Mirrors C++ `Crease::IsSmooth`.
    #[inline] pub fn is_smooth(sharpness: f32) -> bool { sharpness <= SHARPNESS_SMOOTH }

    /// `true` when `sharpness > SHARPNESS_SMOOTH` (0.0). Mirrors C++ `Crease::IsSharp`.
    #[inline] pub fn is_sharp(sharpness: f32) -> bool { sharpness > SHARPNESS_SMOOTH }

    /// `true` when `sharpness >= SHARPNESS_INFINITE` (10.0). Mirrors C++ `Crease::IsInfinite`.
    #[inline] pub fn is_infinite(sharpness: f32) -> bool { sharpness >= SHARPNESS_INFINITE }

    /// `true` when strictly between smooth and infinite. Mirrors C++ `Crease::IsSemiSharp`.
    #[inline] pub fn is_semi_sharp(sharpness: f32) -> bool {
        sharpness > SHARPNESS_SMOOTH && sharpness < SHARPNESS_INFINITE
    }

    // ── Boundary sharpening ───────────────────────────────────────────────────

    /// Always returns `SHARPNESS_INFINITE` — boundary edges are always sharp.
    ///
    /// Despite the BOUNDARY_NONE option, the rest of the code relies on
    /// sharpness to indicate boundary topology.
    #[inline]
    pub fn sharpen_boundary_edge(&self, _edge_sharpness: f32) -> f32 {
        SHARPNESS_INFINITE
    }

    /// Sharpen a boundary vertex according to `VTX_BOUNDARY_EDGE_AND_CORNER`.
    #[inline]
    pub fn sharpen_boundary_vertex(&self, vertex_sharpness: f32) -> f32 {
        if self.options.get_vtx_boundary_interpolation()
            == VtxBoundaryInterpolation::EdgeAndCorner
        {
            SHARPNESS_INFINITE
        } else {
            vertex_sharpness
        }
    }

    // ── Sharpness subdivision ─────────────────────────────────────────────────

    /// Subdivide a uniform (non-Chaikin) sharpness value by one level.
    ///
    /// - smooth stays smooth
    /// - infinite stays infinite
    /// - semi-sharp → max(0, s − 1)
    #[inline]
    pub fn subdivide_uniform_sharpness(&self, s: f32) -> f32 {
        self.decrement_sharpness(s)
    }

    /// Subdivide vertex sharpness (always uniform regardless of creasing method).
    #[inline]
    pub fn subdivide_vertex_sharpness(&self, vertex_sharpness: f32) -> f32 {
        self.decrement_sharpness(vertex_sharpness)
    }

    /// Subdivide the sharpness of a single edge at one end-vertex, considering
    /// the Chaikin average over that vertex's incident edges.
    ///
    /// `edge_sharpness` — the sharpness of *this* edge
    /// `inc_edge_sharpness` — all incident edge sharpnesses at the end vertex
    pub fn subdivide_edge_sharpness_at_vertex(
        &self,
        mut edge_sharpness: f32,
        inc_edge_sharpness: &[f32],
    ) -> f32 {
        let inc_count = inc_edge_sharpness.len();

        // Uniform or degenerate: simple decrement
        if self.is_uniform() || inc_count < 2 {
            return self.decrement_sharpness(edge_sharpness);
        }

        if is_smooth(edge_sharpness)   { return SHARPNESS_SMOOTH; }
        if is_infinite(edge_sharpness) { return SHARPNESS_INFINITE; }

        // Chaikin: weighted average of semi-sharp neighbours
        let mut sharp_sum   = 0.0f32;
        let mut sharp_count = 0i32;
        for &s in inc_edge_sharpness {
            if is_semi_sharp(s) {
                sharp_count += 1;
                sharp_sum   += s;
            }
        }

        if sharp_count > 1 {
            // 3/4 this edge + 1/4 average of the others
            let avg = (sharp_sum - edge_sharpness) / (sharp_count - 1) as f32;
            edge_sharpness = 0.75 * edge_sharpness + 0.25 * avg;
        }

        edge_sharpness -= 1.0;
        if is_sharp(edge_sharpness) { edge_sharpness } else { SHARPNESS_SMOOTH }
    }

    /// Subdivide all incident edge sharpnesses around a vertex in one pass,
    /// which is more efficient for Chaikin because the sum is computed once.
    pub fn subdivide_edge_sharpnesses_around_vertex(
        &self,
        parent: &[f32],
        child:  &mut [f32],
    ) {
        let edge_count = parent.len();
        debug_assert_eq!(child.len(), edge_count);

        // Uniform or degenerate: simple element-wise decrement
        if self.is_uniform() || edge_count < 2 {
            for i in 0..edge_count {
                child[i] = self.decrement_sharpness(parent[i]);
            }
            return;
        }

        // Chaikin: sum semi-sharp values once, then process each edge
        debug_assert_eq!(self.options.get_creasing_method(), CreasingMethod::Chaikin);

        let mut sharp_sum   = 0.0f32;
        let mut sharp_count = 0i32;
        for &s in parent {
            if is_semi_sharp(s) {
                sharp_count += 1;
                sharp_sum   += s;
            }
        }

        if sharp_count == 0 {
            // All smooth — copy unchanged
            child.copy_from_slice(parent);
            return;
        }

        for i in 0..edge_count {
            let p = parent[i];
            if is_smooth(p) {
                child[i] = SHARPNESS_SMOOTH;
            } else if is_infinite(p) {
                child[i] = SHARPNESS_INFINITE;
            } else if sharp_count == 1 {
                // Only this edge is semi-sharp — avoid divide-by-zero
                child[i] = self.decrement_sharpness(p);
            } else {
                let other_avg = (sharp_sum - p) / (sharp_count - 1) as f32;
                // Chaikin: 3/4 * p + 1/4 * avg_others, then subtract 1
                let c = (0.75 * p + 0.25 * other_avg) - 1.0;
                child[i] = if is_smooth(c) { SHARPNESS_SMOOTH } else { c };
            }
        }
    }

    // ── Rule determination ────────────────────────────────────────────────────

    /// Determine the vertex-vertex subdivision rule from vertex sharpness and a
    /// pre-counted number of sharp incident edges.
    ///
    /// Mirrors C++ `DetermineVertexVertexRule(float, int)`.
    pub fn determine_vertex_vertex_rule_from_count(
        &self,
        vertex_sharpness: f32,
        sharp_edge_count: i32,
    ) -> Rule {
        if is_sharp(vertex_sharpness) {
            return Rule::Corner;
        }
        // RULE_SMOOTH=1<<0, RULE_DART=1<<1, RULE_CREASE=1<<2, RULE_CORNER=1<<3
        // sharpEdgeCount: 0→Smooth, 1→Dart, 2→Crease, >2→Corner
        if sharp_edge_count > 2 {
            Rule::Corner
        } else {
            match sharp_edge_count {
                0 => Rule::Smooth,
                1 => Rule::Dart,
                2 => Rule::Crease,
                _ => Rule::Corner,
            }
        }
    }

    /// Determine the vertex-vertex subdivision rule by inspecting incident edge
    /// sharpness values.
    ///
    /// Mirrors C++ `DetermineVertexVertexRule(float, int, float const*)`.
    pub fn determine_vertex_vertex_rule(
        &self,
        vertex_sharpness: f32,
        incident_edge_sharpness: &[f32],
    ) -> Rule {
        if is_sharp(vertex_sharpness) {
            return Rule::Corner;
        }
        let sharp_edge_count = incident_edge_sharpness
            .iter()
            .filter(|&&s| is_sharp(s))
            .count() as i32;
        self.determine_vertex_vertex_rule_from_count(vertex_sharpness, sharp_edge_count)
    }

    // ── Fractional (transitional) weight ─────────────────────────────────────

    /// Compute the fractional blend weight used when the parent and child rules
    /// differ (a sharpness value is transitioning to zero this level).
    ///
    /// Returns a value in [0, 1]: 0 means fully child rule, 1 means fully
    /// parent rule.
    ///
    /// `child_sharpness` — pass `None` to fall back to uniform analysis.
    pub fn compute_fractional_weight_at_vertex(
        &self,
        parent_vertex_sharpness: f32,
        child_vertex_sharpness:  f32,
        parent_sharpness:        &[f32],
        child_sharpness:         Option<&[f32]>,
    ) -> f32 {
        let edge_count = parent_sharpness.len();

        let mut transition_count = 0i32;
        let mut transition_sum   = 0.0f32;

        // If the vertex itself transitions from sharp to smooth, include it
        if is_sharp(parent_vertex_sharpness) && is_smooth(child_vertex_sharpness) {
            transition_count = 1;
            transition_sum   = parent_vertex_sharpness;
        }

        if self.is_uniform() || child_sharpness.is_none() {
            // Uniform or no child values available: look for parent edges in (0,1]
            for i in 0..edge_count {
                let ps = parent_sharpness[i];
                if is_sharp(ps) && ps <= 1.0 {
                    transition_sum   += ps;
                    transition_count += 1;
                }
            }
        } else {
            let cs = child_sharpness.unwrap();
            for i in 0..edge_count {
                let ps = parent_sharpness[i];
                if is_sharp(ps) && is_smooth(cs[i]) {
                    transition_sum   += ps;
                    transition_count += 1;
                }
            }
        }

        if transition_count == 0 {
            return 0.0;
        }
        let w = transition_sum / transition_count as f32;
        w.min(1.0)
    }

    /// Find the two indices of the sharp edges that form the crease at a vertex.
    ///
    /// Exactly two sharp edges are expected (caller must ensure a crease exists).
    /// Mirrors C++ `GetSharpEdgePairOfCrease`.
    pub fn get_sharp_edge_pair_of_crease(
        &self,
        incident_edge_sharpness: &[f32],
    ) -> [usize; 2] {
        let count = incident_edge_sharpness.len();

        // Scan forward from index 0 for the first sharp edge
        let mut first = 0;
        while first < count && is_smooth(incident_edge_sharpness[first]) {
            first += 1;
        }

        // Scan backward from the last index for the second sharp edge
        let mut second = count - 1;
        while second > first && is_smooth(incident_edge_sharpness[second]) {
            second -= 1;
        }

        [first, second]
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Decrement a sharpness value by one level.
    ///
    /// smooth   → smooth
    /// infinite → infinite
    /// > 1      → s − 1.0
    /// ≤ 1      → smooth
    #[inline]
    fn decrement_sharpness(&self, s: f32) -> f32 {
        if is_smooth(s)   { return SHARPNESS_SMOOTH; }   // most common path
        if is_infinite(s) { return SHARPNESS_INFINITE; }
        if s > 1.0        { return s - 1.0; }
        SHARPNESS_SMOOTH
    }
}

impl Default for Crease {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn crease() -> Crease { Crease::new() }

    fn crease_chaikin() -> Crease {
        let mut o = Options::default();
        o.set_creasing_method(CreasingMethod::Chaikin);
        Crease::with_options(o)
    }

    // ── Predicates ────────────────────────────────────────────────────────────

    #[test]
    fn sharpness_predicates() {
        assert!( is_smooth(0.0));
        assert!(!is_smooth(0.001));
        assert!( is_sharp(1.5));
        assert!(!is_sharp(0.0));
        assert!( is_infinite(10.0));
        assert!(!is_infinite(9.99));
        assert!( is_semi_sharp(5.0));
        assert!(!is_semi_sharp(0.0));
        assert!(!is_semi_sharp(10.0));
    }

    // ── Sharpness subdivision ─────────────────────────────────────────────────

    #[test]
    fn subdivide_uniform_decrement() {
        let c = crease();
        assert_abs_diff_eq!(c.subdivide_uniform_sharpness(0.0),  0.0);
        assert_abs_diff_eq!(c.subdivide_uniform_sharpness(10.0), 10.0);
        assert_abs_diff_eq!(c.subdivide_uniform_sharpness(2.5),  1.5);
        assert_abs_diff_eq!(c.subdivide_uniform_sharpness(0.5),  0.0);  // clamp
        assert_abs_diff_eq!(c.subdivide_uniform_sharpness(1.0),  0.0);  // clamp at exactly 1
        assert_abs_diff_eq!(c.subdivide_uniform_sharpness(1.001), 0.001, epsilon = 1e-5);
    }

    #[test]
    fn subdivide_vertex_sharpness() {
        let c = crease();
        assert_abs_diff_eq!(c.subdivide_vertex_sharpness(3.0), 2.0);
        assert_abs_diff_eq!(c.subdivide_vertex_sharpness(10.0), 10.0);
    }

    // ── Chaikin edge subdivision ──────────────────────────────────────────────

    #[test]
    fn chaikin_around_vertex_all_smooth() {
        let c = crease_chaikin();
        let parent = [0.0f32, 0.0, 0.0, 0.0];
        let mut child = [f32::NAN; 4];
        c.subdivide_edge_sharpnesses_around_vertex(&parent, &mut child);
        for &v in &child { assert_abs_diff_eq!(v, 0.0); }
    }

    #[test]
    fn chaikin_around_vertex_two_semi_sharp() {
        let c = crease_chaikin();
        // Two creased edges at sharpness 2.0; rest smooth
        let parent = [2.0f32, 0.0, 2.0, 0.0];
        let mut child = [f32::NAN; 4];
        c.subdivide_edge_sharpnesses_around_vertex(&parent, &mut child);

        // Chaikin: 0.75*2 + 0.25*2 - 1 = 1.5 for the creased edges
        assert_abs_diff_eq!(child[0], 1.0, epsilon = 1e-5);
        assert_abs_diff_eq!(child[2], 1.0, epsilon = 1e-5);
        assert_abs_diff_eq!(child[1], 0.0);
        assert_abs_diff_eq!(child[3], 0.0);
    }

    #[test]
    fn chaikin_at_vertex_single_edge() {
        let c = crease_chaikin();
        // Single incident edge — falls back to simple decrement
        let inc = [3.0f32];
        let result = c.subdivide_edge_sharpness_at_vertex(3.0, &inc);
        assert_abs_diff_eq!(result, 2.0);
    }

    // ── Rule determination ────────────────────────────────────────────────────

    #[test]
    fn rule_smooth() {
        let c = crease();
        let r = c.determine_vertex_vertex_rule(0.0, &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(r, Rule::Smooth);
    }

    #[test]
    fn rule_dart() {
        let c = crease();
        let r = c.determine_vertex_vertex_rule(0.0, &[0.0, 2.0, 0.0, 0.0]);
        assert_eq!(r, Rule::Dart);
    }

    #[test]
    fn rule_crease() {
        let c = crease();
        let r = c.determine_vertex_vertex_rule(0.0, &[0.0, 2.0, 0.0, 2.0]);
        assert_eq!(r, Rule::Crease);
    }

    #[test]
    fn rule_corner_from_vertex_sharpness() {
        let c = crease();
        let r = c.determine_vertex_vertex_rule(5.0, &[0.0, 0.0]);
        assert_eq!(r, Rule::Corner);
    }

    #[test]
    fn rule_corner_three_sharp_edges() {
        let c = crease();
        let r = c.determine_vertex_vertex_rule(0.0, &[1.0, 2.0, 3.0]);
        assert_eq!(r, Rule::Corner);
    }

    #[test]
    fn rule_from_count() {
        let c = crease();
        assert_eq!(c.determine_vertex_vertex_rule_from_count(0.0, 0), Rule::Smooth);
        assert_eq!(c.determine_vertex_vertex_rule_from_count(0.0, 1), Rule::Dart);
        assert_eq!(c.determine_vertex_vertex_rule_from_count(0.0, 2), Rule::Crease);
        assert_eq!(c.determine_vertex_vertex_rule_from_count(0.0, 3), Rule::Corner);
    }

    // ── Boundary sharpening ───────────────────────────────────────────────────

    #[test]
    fn boundary_edge_always_infinite() {
        let c = crease();
        assert_abs_diff_eq!(c.sharpen_boundary_edge(0.0),  SHARPNESS_INFINITE);
        assert_abs_diff_eq!(c.sharpen_boundary_edge(5.0),  SHARPNESS_INFINITE);
    }

    #[test]
    fn boundary_vertex_none() {
        // VTX_BOUNDARY_NONE: vertex sharpness unchanged
        let c = crease();
        assert_abs_diff_eq!(c.sharpen_boundary_vertex(3.0), 3.0);
    }

    #[test]
    fn boundary_vertex_edge_and_corner() {
        let mut o = Options::default();
        o.set_vtx_boundary_interpolation(VtxBoundaryInterpolation::EdgeAndCorner);
        let c = Crease::with_options(o);
        assert_abs_diff_eq!(c.sharpen_boundary_vertex(0.0), SHARPNESS_INFINITE);
    }

    // ── Sharp edge pair ───────────────────────────────────────────────────────

    #[test]
    fn sharp_edge_pair() {
        let c = crease();
        let sharpness = [0.0f32, 2.0, 0.0, 3.0, 0.0];
        let pair = c.get_sharp_edge_pair_of_crease(&sharpness);
        assert_eq!(pair, [1, 3]);
    }

    // ── Fractional weight ─────────────────────────────────────────────────────

    #[test]
    fn fractional_weight_no_transition() {
        let c = crease();
        // Vertex and all edges stay sharp → no transition → weight 0
        let parent = [0.0f32; 4];
        let child  = [0.0f32; 4];
        let w = c.compute_fractional_weight_at_vertex(0.0, 0.0, &parent, Some(&child));
        assert_abs_diff_eq!(w, 0.0);
    }

    #[test]
    fn fractional_weight_single_decaying_edge() {
        let c = crease();
        // One edge at 0.5 decays to 0: weight should be 0.5
        let parent = [0.5f32, 0.0, 0.0, 0.0];
        let child  = [0.0f32, 0.0, 0.0, 0.0];
        let w = c.compute_fractional_weight_at_vertex(0.0, 0.0, &parent, Some(&child));
        assert_abs_diff_eq!(w, 0.5, epsilon = 1e-6);
    }

    #[test]
    fn fractional_weight_clamped_to_one() {
        let c = crease();
        // Vertex sharpness 5.0 decays to 0: sum > 1 → clamp to 1.0
        let parent = [];
        let w = c.compute_fractional_weight_at_vertex(5.0, 0.0, &parent, None);
        assert_abs_diff_eq!(w, 1.0, epsilon = 1e-6);
    }
}
