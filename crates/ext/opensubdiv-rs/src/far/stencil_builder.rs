
//! Internal stencil accumulator used by `StencilTableFactory`.
//!
//! Mirrors C++ `Far::internal::StencilBuilder<REAL>` and its inner
//! `WeightTable<REAL>`.

// ---------------------------------------------------------------------------
// WeightTable — internal accumulator (mirrors C++ anonymous inner class)
// ---------------------------------------------------------------------------

/// Internal weight-accumulation table.
///
/// Each stencil is a variable-length list of `(source_vertex, weight)` pairs.
/// Weights can be scalar-only or include 1st/2nd derivatives.
struct WeightTable {
    // Per-element data (parallel arrays)
    dests:    Vec<i32>,  // which stencil each element belongs to
    sources:  Vec<i32>,  // source vertex for each element
    weights:  Vec<f64>,  // point weight
    du:       Vec<f64>,  // u-derivative weight (empty if not needed)
    dv:       Vec<f64>,  // v-derivative weight
    duu:      Vec<f64>,
    duv:      Vec<f64>,
    dvv:      Vec<f64>,

    // Per-stencil metadata
    indices:  Vec<i32>,  // offset into sources/weights for stencil i
    sizes:    Vec<i32>,  // element count for stencil i

    // Counters
    size:          i32,  // total elements inserted
    last_offset:   i32,  // offset of the stencil currently being built
    coarse_count:  i32,  // number of coarse (control) vertices
    compact:       bool, // combine duplicate source entries
}

impl WeightTable {
    fn new(coarse_verts: i32, gen_ctrl_stencils: bool, compact: bool) -> Self {
        let cap = std::cmp::max(coarse_verts, std::cmp::min(5 * 1024 * 1024, coarse_verts * 2));
        let cap = cap as usize;

        let mut tbl = WeightTable {
            dests:   Vec::with_capacity(cap),
            sources: Vec::with_capacity(cap),
            weights: Vec::with_capacity(cap),
            du:  Vec::new(),
            dv:  Vec::new(),
            duu: Vec::new(),
            duv: Vec::new(),
            dvv: Vec::new(),
            indices: Vec::new(),
            sizes:   Vec::new(),
            size:        0,
            last_offset: 0,
            coarse_count: coarse_verts,
            compact,
        };

        if !gen_ctrl_stencils {
            return tbl;
        }

        // Generate trivial identity stencils for each coarse vertex
        let n = coarse_verts as usize;
        tbl.dests.resize(n, 0);
        tbl.sources.resize(n, 0);
        tbl.weights.resize(n, 0.0);
        tbl.indices.resize(n, 0);
        tbl.sizes.resize(n, 0);

        for i in 0..n {
            tbl.indices[i] = i as i32;
            tbl.sizes[i]   = 1;
            tbl.dests[i]   = i as i32;
            tbl.sources[i] = i as i32;
            tbl.weights[i] = 1.0;
        }

        tbl.size = n as i32;
        tbl.last_offset = if n > 0 { (n - 1) as i32 } else { 0 };
        tbl
    }

    fn set_coarse_vert_count(&mut self, n: i32) { self.coarse_count = n; }

    // ---- Internal helpers --------------------------------------------------

    /// Add a (src → dst) weight, potentially merging with an existing entry.
    fn add_with_weight_scalar(&mut self, src: i32, dst: i32, weight: f64) {
        if weight == 0.0 { return; }

        if src < self.coarse_count {
            self.merge_scalar(src, dst, weight, 1.0);
        } else {
            // Factorize: expand src's own stencil
            let len   = self.sizes[src as usize];
            let start = self.indices[src as usize];
            for k in start..start+len {
                let k = k as usize;
                debug_assert!(self.sources[k] < self.coarse_count);
                let src_w = self.weights[k];
                self.merge_scalar(self.sources[k], dst, src_w, weight);
            }
        }
    }

    fn merge_scalar(&mut self, src: i32, dst: i32, weight: f64, factor: f64) {
        let lo  = self.last_offset as usize;
        let sz  = self.size as usize;

        if self.compact && !self.dests.is_empty() && self.dests[lo] == dst {
            // Look for an existing entry for src in the current stencil
            for i in lo..sz {
                if self.sources[i] == src {
                    self.weights[i] += weight * factor;
                    return;
                }
            }
        }
        self.add_entry_scalar(src, dst, weight * factor);
    }

    fn add_entry_scalar(&mut self, src: i32, dst: i32, weight: f64) {
        if self.dests.is_empty() || dst != *self.dests.last().unwrap() {
            // Starting a new stencil
            if (dst + 1) as usize > self.indices.len() {
                self.indices.resize((dst + 1) as usize, 0);
                self.sizes.resize((dst + 1) as usize, 0);
            }
            self.indices[dst as usize] = self.sources.len() as i32;
            self.sizes[dst as usize]   = 0;
            self.last_offset = self.sources.len() as i32;
        }
        self.size += 1;
        self.sizes[dst as usize] += 1;
        self.dests.push(dst);
        self.sources.push(src);
        self.weights.push(weight);
    }

    /// Add with first derivatives (du, dv).
    fn add_with_weight_1st_deriv(
        &mut self, src: i32, dst: i32, w: f64, du: f64, dv: f64,
    ) {
        // Ensure du/dv arrays exist and are padded to weights length
        self.ensure_deriv_arrays();

        if src < self.coarse_count {
            self.merge_1st(src, dst, w, du, dv, 1.0);
        } else {
            let len   = self.sizes[src as usize];
            let start = self.indices[src as usize];
            for k in start..start+len {
                let k = k as usize;
                let sw = self.weights[k];
                self.merge_1st(self.sources[k], dst, sw * w, sw * du, sw * dv, 1.0);
            }
        }
    }

    fn merge_1st(&mut self, src: i32, dst: i32, w: f64, du: f64, dv: f64, _factor: f64) {
        let lo = self.last_offset as usize;
        let sz = self.size as usize;

        if self.compact && !self.dests.is_empty() && self.dests[lo] == dst {
            for i in lo..sz {
                if self.sources[i] == src {
                    self.weights[i] += w;
                    self.du[i] += du;
                    self.dv[i] += dv;
                    return;
                }
            }
        }
        self.add_entry_1st(src, dst, w, du, dv);
    }

    fn add_entry_1st(&mut self, src: i32, dst: i32, w: f64, du: f64, dv: f64) {
        if self.dests.is_empty() || dst != *self.dests.last().unwrap() {
            if (dst + 1) as usize > self.indices.len() {
                self.indices.resize((dst + 1) as usize, 0);
                self.sizes.resize((dst + 1) as usize, 0);
            }
            self.indices[dst as usize] = self.sources.len() as i32;
            self.sizes[dst as usize]   = 0;
            self.last_offset = self.sources.len() as i32;
        }
        self.size += 1;
        self.sizes[dst as usize] += 1;
        self.dests.push(dst);
        self.sources.push(src);
        self.weights.push(w);
        self.du.push(du);
        self.dv.push(dv);
    }

    fn ensure_deriv_arrays(&mut self) {
        let n = self.weights.len();
        if self.du.len() < n {
            self.du.resize(n, 0.0);
            self.dv.resize(n, 0.0);
        }
    }

    fn ensure_2nd_deriv_arrays(&mut self) {
        let n = self.weights.len();
        if self.duu.len() < n {
            self.duu.resize(n, 0.0);
            self.duv.resize(n, 0.0);
            self.dvv.resize(n, 0.0);
        }
    }

    /// Add with second derivatives (du, dv, duu, duv, dvv).
    fn add_with_weight_2nd_deriv(
        &mut self, src: i32, dst: i32,
        w: f64, du: f64, dv: f64, duu: f64, duv: f64, dvv: f64,
    ) {
        self.ensure_deriv_arrays();
        self.ensure_2nd_deriv_arrays();

        if src < self.coarse_count {
            self.add_entry_2nd(src, dst, w, du, dv, duu, duv, dvv);
        } else {
            let len   = self.sizes[src as usize];
            let start = self.indices[src as usize];
            for k in start..start+len {
                let k  = k as usize;
                let sw = self.weights[k];
                self.add_entry_2nd(
                    self.sources[k], dst,
                    sw*w, sw*du, sw*dv, sw*duu, sw*duv, sw*dvv,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_entry_2nd(
        &mut self, src: i32, dst: i32,
        w: f64, du: f64, dv: f64, duu: f64, duv: f64, dvv: f64,
    ) {
        if self.dests.is_empty() || dst != *self.dests.last().unwrap() {
            if (dst + 1) as usize > self.indices.len() {
                self.indices.resize((dst + 1) as usize, 0);
                self.sizes.resize((dst + 1) as usize, 0);
            }
            self.indices[dst as usize] = self.sources.len() as i32;
            self.sizes[dst as usize]   = 0;
            self.last_offset = self.sources.len() as i32;
        }
        self.size += 1;
        self.sizes[dst as usize] += 1;
        self.dests.push(dst);
        self.sources.push(src);
        self.weights.push(w);
        self.du.push(du);
        self.dv.push(dv);
        self.duu.push(duu);
        self.duv.push(duv);
        self.dvv.push(dvv);
    }
}

// ---------------------------------------------------------------------------
// StencilBuilder — public API wrapping WeightTable
// ---------------------------------------------------------------------------

/// Accumulates stencil weights during refinement.
///
/// Mirrors C++ `Far::internal::StencilBuilder<REAL>`.
pub struct StencilBuilder {
    table: WeightTable,
}

impl StencilBuilder {
    /// Create a builder for a mesh with `coarse_vert_count` control vertices.
    ///
    /// * `gen_ctrl_vert_stencils` — if true, identity stencils are pre-inserted
    ///   for all coarse vertices (weight = 1.0).
    /// * `compact_weights` — merge duplicate source entries per stencil.
    pub fn new(coarse_vert_count: i32, gen_ctrl_vert_stencils: bool, compact_weights: bool) -> Self {
        Self { table: WeightTable::new(coarse_vert_count, gen_ctrl_vert_stencils, compact_weights) }
    }

    pub fn get_num_vertices_total(&self) -> usize { self.table.weights.len() }

    pub fn get_num_verts_in_stencil(&self, stencil_index: usize) -> i32 {
        if stencil_index >= self.table.sizes.len() { return 0; }
        self.table.sizes[stencil_index]
    }

    pub fn set_coarse_vert_count(&mut self, n: i32) { self.table.set_coarse_vert_count(n); }

    pub fn get_stencil_offsets(&self) -> &[i32] { &self.table.indices }
    pub fn get_stencil_sizes(&self)   -> &[i32] { &self.table.sizes }
    pub fn get_stencil_sources(&self) -> &[i32] { &self.table.sources }
    pub fn get_stencil_weights(&self) -> &[f64] { &self.table.weights }
    pub fn get_stencil_du_weights(&self) -> &[f64] { &self.table.du }
    pub fn get_stencil_dv_weights(&self) -> &[f64] { &self.table.dv }
    pub fn get_stencil_duu_weights(&self) -> &[f64] { &self.table.duu }
    pub fn get_stencil_duv_weights(&self) -> &[f64] { &self.table.duv }
    pub fn get_stencil_dvv_weights(&self) -> &[f64] { &self.table.dvv }

    // ---- Index facade (returned by the factory) ----------------------------

    pub fn index(&mut self, i: i32) -> StencilIndex<'_> {
        StencilIndex { builder: self, index: i }
    }
}

// ---------------------------------------------------------------------------
// StencilIndex — facade that enables `AddWithWeight`-style calls on a builder
// ---------------------------------------------------------------------------

/// A reference to a single output vertex slot in a `StencilBuilder`.
///
/// Mirrors the inner `StencilBuilder<REAL>::Index` class.
pub struct StencilIndex<'a> {
    builder: &'a mut StencilBuilder,
    pub index: i32,
}

impl<'a> StencilIndex<'a> {
    /// Offset this index by `n` (returns a *new* value-type index).
    pub fn offset(&self, n: i32) -> i32 { self.index + n }

    /// Add `weight` * source vertex `src` to this stencil entry.
    pub fn add_with_weight_vertex(&mut self, src: i32, weight: f64) {
        if weight == 0.0 { return; }
        self.builder.table.add_with_weight_scalar(src, self.index, weight);
    }

    /// Add weights for a full existing stencil (factorize).
    pub fn add_with_weight_stencil(
        &mut self,
        src_size:    i32,
        src_indices: &[i32],
        src_weights: &[f32],
        weight:      f64,
    ) {
        if weight == 0.0 { return; }
        for k in 0..src_size as usize {
            let w = src_weights[k] as f64;
            if w == 0.0 { continue; }
            self.builder.table.add_with_weight_scalar(
                src_indices[k], self.index, w * weight,
            );
        }
    }

    /// Add with first-derivative weights.
    pub fn add_with_weight_1st(
        &mut self,
        src_size:    i32,
        src_indices: &[i32],
        src_weights: &[f32],
        weight:      f64,
        du:          f64,
        dv:          f64,
    ) {
        if weight == 0.0 && du == 0.0 && dv == 0.0 { return; }
        for k in 0..src_size as usize {
            let w = src_weights[k] as f64;
            if w == 0.0 { continue; }
            self.builder.table.add_with_weight_1st_deriv(
                src_indices[k], self.index,
                w * weight, w * du, w * dv,
            );
        }
    }

    /// Add with second-derivative weights.
    #[allow(clippy::too_many_arguments)]
    pub fn add_with_weight_2nd(
        &mut self,
        src_size:    i32,
        src_indices: &[i32],
        src_weights: &[f32],
        weight:      f64,
        du:          f64,
        dv:          f64,
        duu:         f64,
        duv:         f64,
        dvv:         f64,
    ) {
        if weight == 0.0 && du == 0.0 && dv == 0.0
            && duu == 0.0 && duv == 0.0 && dvv == 0.0 { return; }
        for k in 0..src_size as usize {
            let w = src_weights[k] as f64;
            if w == 0.0 { continue; }
            self.builder.table.add_with_weight_2nd_deriv(
                src_indices[k], self.index,
                w*weight, w*du, w*dv, w*duu, w*duv, w*dvv,
            );
        }
    }

    pub fn clear(&mut self) { /* nothing needed: builder never clears mid-row */ }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_stencils() {
        let b = StencilBuilder::new(3, true, true);
        assert_eq!(b.get_stencil_sizes().len(), 3);
        for i in 0..3 {
            assert_eq!(b.get_stencil_sizes()[i], 1);
            let off = b.get_stencil_offsets()[i] as usize;
            assert_eq!(b.get_stencil_sources()[off], i as i32);
            assert!((b.get_stencil_weights()[off] - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn add_weights() {
        let mut b = StencilBuilder::new(2, false, true);
        // stencil 0: 0.5*v0 + 0.5*v1
        {
            let mut idx = b.index(0);
            idx.add_with_weight_vertex(0, 0.5);
            idx.add_with_weight_vertex(1, 0.5);
        }
        assert_eq!(b.get_stencil_sizes()[0], 2);
        let off = b.get_stencil_offsets()[0] as usize;
        assert!((b.get_stencil_weights()[off] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn compact_merges_duplicates() {
        let mut b = StencilBuilder::new(2, false, true);
        {
            let mut idx = b.index(0);
            idx.add_with_weight_vertex(0, 0.3);
            idx.add_with_weight_vertex(0, 0.7); // should merge
        }
        // Only 1 unique source
        assert_eq!(b.get_stencil_sizes()[0], 1);
        let off = b.get_stencil_offsets()[0] as usize;
        assert!((b.get_stencil_weights()[off] - 1.0).abs() < 1e-9);
    }
}
