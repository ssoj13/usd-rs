
use crate::far::types::Index;

// ---------------------------------------------------------------------------
// Stencil -- view into one stencil entry
// ---------------------------------------------------------------------------

/// View into a single stencil within a `StencilTable`.
///
/// Mirrors C++ `Far::Stencil` / `Far::StencilReal<REAL>`.
#[derive(Clone, Copy)]
pub struct Stencil<'a> {
    size:    &'a i32,
    indices: &'a [Index],
    weights: &'a [f32],
}

impl<'a> Stencil<'a> {
    /// Number of contributing control vertices.
    pub fn get_size(&self) -> i32 { *self.size }
    /// Control vertex indices for this stencil.
    pub fn get_vertex_indices(&self) -> &'a [Index] { self.indices }
    /// Interpolation weights (parallel with indices).
    pub fn get_weights(&self) -> &'a [f32] { self.weights }
}

// ---------------------------------------------------------------------------
// LimitStencil -- stencil with optional derivative weights
// ---------------------------------------------------------------------------

/// View into a single limit stencil.
///
/// Mirrors C++ `Far::LimitStencil` / `Far::LimitStencilReal<REAL>`.
pub struct LimitStencilView<'a> {
    size:        &'a i32,
    indices:     &'a [Index],
    weights:     &'a [f32],
    du_weights:  Option<&'a [f32]>,
    dv_weights:  Option<&'a [f32]>,
    duu_weights: Option<&'a [f32]>,
    duv_weights: Option<&'a [f32]>,
    dvv_weights: Option<&'a [f32]>,
}

impl<'a> LimitStencilView<'a> {
    pub fn get_size(&self)           -> i32              { *self.size }
    pub fn get_vertex_indices(&self) -> &'a [Index]      { self.indices }
    pub fn get_weights(&self)        -> &'a [f32]        { self.weights }
    pub fn get_du_weights(&self)     -> Option<&'a [f32]>{ self.du_weights }
    pub fn get_dv_weights(&self)     -> Option<&'a [f32]>{ self.dv_weights }
    pub fn get_duu_weights(&self)    -> Option<&'a [f32]>{ self.duu_weights }
    pub fn get_duv_weights(&self)    -> Option<&'a [f32]>{ self.duv_weights }
    pub fn get_dvv_weights(&self)    -> Option<&'a [f32]>{ self.dvv_weights }
}

// ---------------------------------------------------------------------------
// StencilTable
// ---------------------------------------------------------------------------

/// CPU stencil table — a set of weighted sums over source control vertices.
///
/// Mirrors Far::StencilTable.
#[derive(Debug, Default)]
pub struct StencilTable {
    /// Number of source control vertices used to build the table.
    num_control_vertices: i32,
    /// sizes[i] = number of (index,weight) pairs for stencil i.
    pub sizes: Vec<i32>,
    /// Cumulative offsets into indices/weights: offsets[i] = start of stencil i.
    pub offsets: Vec<i32>,
    /// Source vertex indices for each stencil coefficient.
    pub indices: Vec<i32>,
    /// Weights corresponding to each source vertex.
    pub weights: Vec<f32>,
}

impl StencilTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from pre-populated data (used by factory).
    pub fn from_data(
        num_control_vertices: i32,
        sizes:   Vec<i32>,
        offsets: Vec<i32>,
        indices: Vec<i32>,
        weights: Vec<f32>,
    ) -> Self {
        Self { num_control_vertices, sizes, offsets, indices, weights }
    }

    pub fn get_num_control_vertices(&self) -> i32 { self.num_control_vertices }
    pub fn get_num_stencils(&self)         -> i32 { self.sizes.len() as i32 }

    pub fn sizes(&self)   -> &[i32] { &self.sizes }
    pub fn offsets(&self) -> &[i32] { &self.offsets }
    pub fn indices(&self) -> &[i32] { &self.indices }
    pub fn weights(&self) -> &[f32] { &self.weights }

    pub fn set_num_control_vertices(&mut self, n: i32) { self.num_control_vertices = n; }

    /// Return a view of stencil `i`.
    ///
    /// Mirrors C++ `StencilTable::GetStencil(Index i)`.
    pub fn get_stencil(&self, i: Index) -> Stencil<'_> {
        assert!(!self.offsets.is_empty() && (i as usize) < self.offsets.len());
        let ofs  = self.offsets[i as usize] as usize;
        let size = self.sizes[i as usize] as usize;
        Stencil {
            size:    &self.sizes[i as usize],
            indices: &self.indices[ofs..ofs + size],
            weights: &self.weights[ofs..ofs + size],
        }
    }

    /// Populate offsets table from sizes.
    ///
    /// Mirrors C++ `StencilTableReal::generateOffsets()`.
    pub fn generate_offsets(&mut self) {
        let n = self.sizes.len();
        self.offsets.resize(n, 0);
        let mut offset = 0i32;
        for i in 0..n {
            self.offsets[i] = offset;
            offset += self.sizes[i];
        }
    }

    /// Resize storage arrays.
    ///
    /// Mirrors C++ `StencilTableReal::resize(int, int)`.
    pub fn resize(&mut self, nstencils: i32, nelems: i32) {
        self.sizes.resize(nstencils as usize, 0);
        self.indices.resize(nelems as usize, 0);
        self.weights.resize(nelems as usize, 0.0);
    }

    /// Reserve storage.
    ///
    /// Mirrors C++ `StencilTableReal::reserve(int, int)`.
    pub fn reserve(&mut self, nstencils: i32, nelems: i32) {
        self.sizes.reserve(nstencils as usize);
        self.indices.reserve(nelems as usize);
        self.weights.reserve(nelems as usize);
    }

    /// Shrink vectors to fit.
    ///
    /// Mirrors C++ `StencilTableReal::shrinkToFit()`.
    pub fn shrink_to_fit(&mut self) {
        self.sizes.shrink_to_fit();
        self.offsets.shrink_to_fit();
        self.indices.shrink_to_fit();
        self.weights.shrink_to_fit();
    }

    /// Finalize: shrink + generate offsets.
    ///
    /// Mirrors C++ `StencilTableReal::finalize()`.
    pub fn finalize(&mut self) {
        self.shrink_to_fit();
        self.generate_offsets();
    }

    /// Clear all stencil data.
    ///
    /// Mirrors C++ `StencilTableReal::Clear()`.
    pub fn clear(&mut self) {
        self.sizes.clear();
        self.offsets.clear();
        self.indices.clear();
        self.weights.clear();
    }

    /// Update `dst` values from `src` using the stencil weights.
    ///
    /// `src` and `dst` are plain `f32` slices. `start` / `end` select an
    /// optional stencil sub-range (pass `start = -1` for all stencils,
    /// matching C++ default arguments).
    ///
    /// This is a type-erased slice version of C++
    /// `StencilTable::UpdateValues(T const*, U*, Index start, Index end)`.
    pub fn update_values_f32_slice(
        &self,
        src: &[f32],
        dst: &mut [f32],
        start: i32,
        end:   i32,
    ) {
        if self.sizes.is_empty() { return; }
        let ns = self.get_num_stencils() as usize;
        let s  = if start > 0 { start as usize } else { 0 };
        let e  = if end > start { end as usize } else { ns };

        let mut idx = if s > 0 && !self.offsets.is_empty() {
            self.offsets[s] as usize
        } else {
            0
        };

        for i in s..e {
            let sz  = self.sizes[i] as usize;
            let mut val = 0.0f32;
            for _ in 0..sz {
                val += src[self.indices[idx] as usize] * self.weights[idx];
                idx += 1;
            }
            dst[i] = val;
        }
    }

    /// Functional callback variant of UpdateValues.
    ///
    /// `src(index) -> f32`, `dst(stencil_index, value)`.
    pub fn update_values_f32<T, U>(&self, src: &T, dst: &mut U, start: i32, end: i32)
    where
        T: Fn(i32) -> f32,
        U: FnMut(i32, f32),
    {
        let ns = self.get_num_stencils();
        let s  = if start > 0 { start as usize } else { 0 };
        let e  = if end > start { end as usize } else { ns as usize };

        let mut idx = if s > 0 && !self.offsets.is_empty() {
            self.offsets[s] as usize
        } else {
            0
        };

        for i in s..e {
            let sz  = self.sizes[i] as usize;
            let mut val = 0.0f32;
            for _ in 0..sz {
                val += src(self.indices[idx] as i32) * self.weights[idx];
                idx += 1;
            }
            dst(i as i32, val);
        }
    }

    /// Append all stencils from `other` into this table.
    ///
    /// Indices in `other` are offset by the number of control vertices in
    /// this table if `other.num_control_vertices` differs — no re-indexing is
    /// performed (caller must ensure compatible CV sets).
    ///
    /// Mirrors C++ factory `AppendLocalPointStencilTable` behaviour.
    pub fn append(&mut self, other: &StencilTable) {
        let base_elems = self.indices.len();
        self.sizes.extend_from_slice(&other.sizes);
        self.indices.extend_from_slice(&other.indices);
        self.weights.extend_from_slice(&other.weights);
        // Rebuild offsets for the whole table
        let n = self.sizes.len();
        self.offsets.resize(n, 0);
        let mut offset = 0i32;
        for i in 0..n {
            self.offsets[i] = offset;
            offset += self.sizes[i];
        }
        let _ = base_elems;
    }
}

// ---------------------------------------------------------------------------
// LimitStencilTable
// ---------------------------------------------------------------------------

/// Limit stencil table — extends StencilTable with du/dv derivative weights.
///
/// Mirrors Far::LimitStencilTable.
#[derive(Debug, Default)]
pub struct LimitStencilTable {
    pub base: StencilTable,
    /// Derivative weights in the u direction.
    pub du_weights:  Vec<f32>,
    /// Derivative weights in the v direction.
    pub dv_weights:  Vec<f32>,
    /// Second-order derivative weights (optional).
    pub duu_weights: Vec<f32>,
    pub duv_weights: Vec<f32>,
    pub dvv_weights: Vec<f32>,
}

impl LimitStencilTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from pre-populated data.
    #[allow(clippy::too_many_arguments)]
    pub fn from_data(
        num_control_vertices: i32,
        sizes:       Vec<i32>,
        offsets:     Vec<i32>,
        indices:     Vec<i32>,
        weights:     Vec<f32>,
        du_weights:  Vec<f32>,
        dv_weights:  Vec<f32>,
        duu_weights: Vec<f32>,
        duv_weights: Vec<f32>,
        dvv_weights: Vec<f32>,
    ) -> Self {
        Self {
            base: StencilTable::from_data(
                num_control_vertices, sizes, offsets, indices, weights),
            du_weights,
            dv_weights,
            duu_weights,
            duv_weights,
            dvv_weights,
        }
    }

    pub fn get_num_control_vertices(&self) -> i32 { self.base.get_num_control_vertices() }
    pub fn get_num_stencils(&self)         -> i32 { self.base.get_num_stencils() }

    pub fn get_stencil(&self, i: Index) -> Stencil<'_> { self.base.get_stencil(i) }

    /// Return a full limit-stencil view at index `i`.
    ///
    /// Mirrors C++ `LimitStencilTable::GetLimitStencil(Index i)`.
    pub fn get_limit_stencil(&self, i: Index) -> LimitStencilView<'_> {
        assert!(!self.base.offsets.is_empty() && (i as usize) < self.base.offsets.len());
        let ofs  = self.base.offsets[i as usize] as usize;
        let size = self.base.sizes[i as usize] as usize;

        let du_opt  = if self.du_weights.is_empty()  { None } else { Some(&self.du_weights[ofs..ofs + size]) };
        let dv_opt  = if self.dv_weights.is_empty()  { None } else { Some(&self.dv_weights[ofs..ofs + size]) };
        let duu_opt = if self.duu_weights.is_empty() { None } else { Some(&self.duu_weights[ofs..ofs + size]) };
        let duv_opt = if self.duv_weights.is_empty() { None } else { Some(&self.duv_weights[ofs..ofs + size]) };
        let dvv_opt = if self.dvv_weights.is_empty() { None } else { Some(&self.dvv_weights[ofs..ofs + size]) };

        LimitStencilView {
            size:        &self.base.sizes[i as usize],
            indices:     &self.base.indices[ofs..ofs + size],
            weights:     &self.base.weights[ofs..ofs + size],
            du_weights:  du_opt,
            dv_weights:  dv_opt,
            duu_weights: duu_opt,
            duv_weights: duv_opt,
            dvv_weights: dvv_opt,
        }
    }

    pub fn du_weights(&self)  -> &[f32] { &self.du_weights }
    pub fn dv_weights(&self)  -> &[f32] { &self.dv_weights }
    pub fn duu_weights(&self) -> &[f32] { &self.duu_weights }
    pub fn duv_weights(&self) -> &[f32] { &self.duv_weights }
    pub fn dvv_weights(&self) -> &[f32] { &self.dvv_weights }

    /// Update `dst` with u/v derivatives from `src` using limit weights.
    ///
    /// Mirrors C++ `LimitStencilTable::UpdateDerivs(src, uderivs, vderivs, start, end)`.
    pub fn update_derivs_f32_slice(
        &self,
        src:     &[f32],
        uderivs: &mut [f32],
        vderivs: &mut [f32],
        start:   i32,
        end:     i32,
    ) {
        update_weighted(src, uderivs, &self.base, &self.du_weights, start, end);
        update_weighted(src, vderivs, &self.base, &self.dv_weights, start, end);
    }

    /// Resize (factory helper).
    pub fn resize(&mut self, nstencils: i32, nelems: i32) {
        self.base.resize(nstencils, nelems);
        self.du_weights.resize(nelems as usize, 0.0);
        self.dv_weights.resize(nelems as usize, 0.0);
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.base.clear();
        self.du_weights.clear();
        self.dv_weights.clear();
        self.duu_weights.clear();
        self.duv_weights.clear();
        self.dvv_weights.clear();
    }
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

/// Apply weighted stencil data from `weights_vec` to `dst` given `src`.
fn update_weighted(
    src:         &[f32],
    dst:         &mut [f32],
    table:       &StencilTable,
    weights_vec: &[f32],
    start:       i32,
    end:         i32,
) {
    if table.sizes.is_empty() || weights_vec.is_empty() { return; }
    let ns = table.get_num_stencils() as usize;
    let s  = if start > 0 { start as usize } else { 0 };
    let e  = if end > start { end as usize } else { ns };
    let mut idx = if s > 0 && !table.offsets.is_empty() {
        table.offsets[s] as usize
    } else {
        0
    };
    for i in s..e {
        let sz  = table.sizes[i] as usize;
        let mut val = 0.0f32;
        for _ in 0..sz {
            val += src[table.indices[idx] as usize] * weights_vec[idx];
            idx += 1;
        }
        dst[i] = val;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn make_table() -> StencilTable {
        // Two stencils:
        // stencil 0: index 0 weight 0.5, index 1 weight 0.5
        // stencil 1: index 2 weight 1.0
        let mut t = StencilTable::from_data(
            3,
            vec![2, 1],
            vec![0, 2],
            vec![0, 1, 2],
            vec![0.5, 0.5, 1.0],
        );
        t.generate_offsets();
        t
    }

    #[test]
    fn basic_stencil() {
        let t = make_table();
        assert_eq!(t.get_num_stencils(), 2);
        let s0 = t.get_stencil(0);
        assert_eq!(s0.get_size(), 2);
        assert_eq!(s0.get_vertex_indices(), &[0, 1]);
        let s1 = t.get_stencil(1);
        assert_eq!(s1.get_size(), 1);
        assert_eq!(s1.get_vertex_indices(), &[2]);
    }

    #[test]
    fn update_values_slice() {
        let t   = make_table();
        let src = [1.0f32, 3.0, 5.0];
        let mut dst = [0.0f32; 2];
        t.update_values_f32_slice(&src, &mut dst, -1, -1);
        // stencil 0: 0.5*1 + 0.5*3 = 2.0
        assert!((dst[0] - 2.0).abs() < 1e-6);
        // stencil 1: 1.0*5 = 5.0
        assert!((dst[1] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn clear_resets_table() {
        let mut t = make_table();
        t.clear();
        assert_eq!(t.get_num_stencils(), 0);
        assert!(t.indices.is_empty());
    }

    #[test]
    fn append_stencils() {
        let t1 = make_table();
        let t2 = StencilTable::from_data(
            3, vec![1], vec![0], vec![0], vec![1.0]);
        let mut combined = t1;
        combined.append(&t2);
        assert_eq!(combined.get_num_stencils(), 3);
    }

    #[test]
    fn limit_stencil_table() {
        let base = StencilTable::from_data(
            2,
            vec![1, 1],
            vec![0, 1],
            vec![0, 1],
            vec![1.0, 1.0],
        );
        let lt = LimitStencilTable {
            base,
            du_weights: vec![0.1, 0.2],
            dv_weights: vec![0.3, 0.4],
            ..Default::default()
        };
        let v = lt.get_limit_stencil(0);
        assert!((v.get_du_weights().unwrap()[0] - 0.1).abs() < 1e-6);
        let v1 = lt.get_limit_stencil(1);
        assert!((v1.get_dv_weights().unwrap()[0] - 0.4).abs() < 1e-6);
    }
}
