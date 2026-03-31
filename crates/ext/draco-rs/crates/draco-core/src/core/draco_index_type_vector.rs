//! IndexTypeVector utilities.
//! Reference: `_ref/draco/src/draco/core/draco_index_type_vector.h`.

use crate::core::draco_index_type::DracoIndex;

#[derive(Clone, Debug, Default)]
pub struct IndexTypeVector<IndexT, ValueT> {
    vector: Vec<ValueT>,
    _index: std::marker::PhantomData<IndexT>,
}

impl<IndexT, ValueT> IndexTypeVector<IndexT, ValueT> {
    pub fn new() -> Self {
        Self {
            vector: Vec::new(),
            _index: std::marker::PhantomData,
        }
    }

    pub fn with_size(size: usize) -> Self
    where
        ValueT: Default + Clone,
    {
        Self {
            vector: vec![ValueT::default(); size],
            _index: std::marker::PhantomData,
        }
    }

    pub fn with_size_value(size: usize, val: ValueT) -> Self
    where
        ValueT: Clone,
    {
        Self {
            vector: vec![val; size],
            _index: std::marker::PhantomData,
        }
    }

    pub fn clear(&mut self) {
        self.vector.clear();
    }

    pub fn reserve(&mut self, size: usize) {
        self.vector.reserve(size);
    }

    pub fn resize(&mut self, size: usize)
    where
        ValueT: Default + Clone,
    {
        self.vector.resize(size, ValueT::default());
    }

    pub fn resize_with_value(&mut self, size: usize, val: ValueT)
    where
        ValueT: Clone,
    {
        self.vector.resize(size, val);
    }

    pub fn assign(&mut self, size: usize, val: ValueT)
    where
        ValueT: Clone,
    {
        self.vector.clear();
        self.vector.resize(size, val);
    }

    pub fn erase(&mut self, position: usize) -> Option<ValueT> {
        if position >= self.vector.len() {
            return None;
        }
        Some(self.vector.remove(position))
    }

    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(self, other);
    }

    pub fn size(&self) -> usize {
        self.vector.len()
    }

    pub fn empty(&self) -> bool {
        self.vector.is_empty()
    }

    pub fn push_back(&mut self, val: ValueT) {
        self.vector.push(val);
    }

    pub fn data(&self) -> &[ValueT] {
        &self.vector
    }

    pub fn data_mut(&mut self) -> &mut [ValueT] {
        &mut self.vector
    }

    pub fn get(&self, index: IndexT) -> Option<&ValueT>
    where
        IndexT: DracoIndex,
    {
        self.vector.get(index.to_usize())
    }

    pub fn get_mut(&mut self, index: IndexT) -> Option<&mut ValueT>
    where
        IndexT: DracoIndex,
    {
        self.vector.get_mut(index.to_usize())
    }
}

impl<IndexT, ValueT> std::ops::Index<IndexT> for IndexTypeVector<IndexT, ValueT>
where
    IndexT: DracoIndex,
{
    type Output = ValueT;
    fn index(&self, index: IndexT) -> &Self::Output {
        &self.vector[index.to_usize()]
    }
}

impl<IndexT, ValueT> std::ops::IndexMut<IndexT> for IndexTypeVector<IndexT, ValueT>
where
    IndexT: DracoIndex,
{
    fn index_mut(&mut self, index: IndexT) -> &mut Self::Output {
        let idx = index.to_usize();
        &mut self.vector[idx]
    }
}
