//! Valence cache utilities.
//! Reference: `_ref/draco/src/draco/mesh/valence_cache.h`.

use std::marker::PhantomData;

use crate::attributes::geometry_indices::{
    CornerIndex, VertexIndex, INVALID_CORNER_INDEX, INVALID_VERTEX_INDEX,
};
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::{draco_dcheck_eq, draco_dcheck_lt};

pub trait ValenceCacheTable {
    fn num_vertices(&self) -> usize;
    fn valence(&self, v: VertexIndex) -> i32;
    fn confident_vertex(&self, c: CornerIndex) -> VertexIndex;
    fn vertex(&self, c: CornerIndex) -> VertexIndex;
}

// ValenceCache provides support for caching of valences for CornerTable-like
// types. The cached values must be invalidated when the underlying topology
// changes.
pub struct ValenceCache<T> {
    vertex_valence_cache_8_bit: IndexTypeVector<VertexIndex, i8>,
    vertex_valence_cache_32_bit: IndexTypeVector<VertexIndex, i32>,
    _phantom: PhantomData<T>,
}

impl<T> ValenceCache<T> {
    pub fn new() -> Self {
        Self {
            vertex_valence_cache_8_bit: IndexTypeVector::new(),
            vertex_valence_cache_32_bit: IndexTypeVector::new(),
            _phantom: PhantomData,
        }
    }

    pub fn is_cache_empty(&self) -> bool {
        self.vertex_valence_cache_8_bit.size() == 0 && self.vertex_valence_cache_32_bit.size() == 0
    }

    pub fn clear_valence_cache_inaccurate(&mut self) {
        self.vertex_valence_cache_8_bit.clear();
        let mut empty = IndexTypeVector::new();
        self.vertex_valence_cache_8_bit.swap(&mut empty);
    }

    pub fn clear_valence_cache(&mut self) {
        self.vertex_valence_cache_32_bit.clear();
        let mut empty = IndexTypeVector::new();
        self.vertex_valence_cache_32_bit.swap(&mut empty);
    }
}

impl<T: ValenceCacheTable> ValenceCache<T> {
    pub fn valence_from_cache_inaccurate(&self, table: &T, c: CornerIndex) -> i8 {
        if c == INVALID_CORNER_INDEX {
            return -1;
        }
        self.valence_from_cache_inaccurate_vertex(table, table.vertex(c))
    }

    pub fn valence_from_cache(&self, table: &T, c: CornerIndex) -> i32 {
        if c == INVALID_CORNER_INDEX {
            return -1;
        }
        self.valence_from_cache_vertex(table, table.vertex(c))
    }

    pub fn confident_valence_from_cache(&self, table: &T, v: VertexIndex) -> i32 {
        draco_dcheck_lt!(v.value() as usize, table.num_vertices());
        draco_dcheck_eq!(
            self.vertex_valence_cache_32_bit.size(),
            table.num_vertices()
        );
        self.vertex_valence_cache_32_bit[v]
    }

    pub fn cache_valences_inaccurate(&mut self, table: &T) {
        if self.vertex_valence_cache_8_bit.size() != 0 {
            return;
        }
        let vertex_count = table.num_vertices();
        self.vertex_valence_cache_8_bit.resize(vertex_count);
        for v in 0..vertex_count {
            let v_idx = VertexIndex::new(v as u32);
            let valence = table.valence(v_idx);
            let clipped = std::cmp::min(i8::MAX as i32, valence) as i8;
            self.vertex_valence_cache_8_bit[v_idx] = clipped;
        }
    }

    pub fn cache_valences(&mut self, table: &T) {
        if self.vertex_valence_cache_32_bit.size() != 0 {
            return;
        }
        let vertex_count = table.num_vertices();
        self.vertex_valence_cache_32_bit.resize(vertex_count);
        for v in 0..vertex_count {
            let v_idx = VertexIndex::new(v as u32);
            self.vertex_valence_cache_32_bit[v_idx] = table.valence(v_idx);
        }
    }

    pub fn confident_valence_from_cache_inaccurate(&self, table: &T, c: CornerIndex) -> i8 {
        self.confident_valence_from_cache_inaccurate_vertex(table, table.confident_vertex(c))
    }

    pub fn confident_valence_from_cache_corner(&self, table: &T, c: CornerIndex) -> i32 {
        self.confident_valence_from_cache(table, table.confident_vertex(c))
    }

    pub fn valence_from_cache_inaccurate_vertex(&self, table: &T, v: VertexIndex) -> i8 {
        draco_dcheck_eq!(self.vertex_valence_cache_8_bit.size(), table.num_vertices());
        if v == INVALID_VERTEX_INDEX || v.value() as usize >= table.num_vertices() {
            return -1;
        }
        self.confident_valence_from_cache_inaccurate_vertex(table, v)
    }

    pub fn confident_valence_from_cache_inaccurate_vertex(&self, table: &T, v: VertexIndex) -> i8 {
        draco_dcheck_lt!(v.value() as usize, table.num_vertices());
        draco_dcheck_eq!(self.vertex_valence_cache_8_bit.size(), table.num_vertices());
        self.vertex_valence_cache_8_bit[v]
    }

    pub fn valence_from_cache_vertex(&self, table: &T, v: VertexIndex) -> i32 {
        draco_dcheck_eq!(
            self.vertex_valence_cache_32_bit.size(),
            table.num_vertices()
        );
        if v == INVALID_VERTEX_INDEX || v.value() as usize >= table.num_vertices() {
            return -1;
        }
        self.confident_valence_from_cache(table, v)
    }
}

impl<T> Default for ValenceCache<T> {
    fn default() -> Self {
        Self::new()
    }
}
