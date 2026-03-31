//! Dynamic-dimensional point vector storage.
//! Reference: `_ref/draco/src/draco/compression/attributes/point_d_vector.h`.
//!
//! Provides a contiguous storage for points with runtime-selected dimensionality.
//! Used by kd-tree attribute encoders to stage integer point clouds.

use draco_core::draco_dcheck_eq;
use draco_core::draco_dcheck_lt;

/// Contiguous vector of N-dimensional points where N is specified at runtime.
#[derive(Debug, Clone)]
pub struct PointDVector<InternalT: Copy + Default> {
    n_items: usize,
    dimensionality: usize,
    data: Vec<InternalT>,
}

impl<InternalT: Copy + Default> PointDVector<InternalT> {
    pub fn new(n_items: usize, dimensionality: usize) -> Self {
        Self {
            n_items,
            dimensionality,
            data: vec![InternalT::default(); n_items * dimensionality],
        }
    }

    pub fn size(&self) -> usize {
        self.n_items
    }

    pub fn dimensionality(&self) -> usize {
        self.dimensionality
    }

    pub fn data(&self) -> &[InternalT] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [InternalT] {
        &mut self.data
    }

    pub fn point(&self, index: usize) -> &[InternalT] {
        draco_dcheck_lt!(index, self.n_items);
        let start = index * self.dimensionality;
        &self.data[start..start + self.dimensionality]
    }

    pub fn point_mut(&mut self, index: usize) -> &mut [InternalT] {
        draco_dcheck_lt!(index, self.n_items);
        let start = index * self.dimensionality;
        &mut self.data[start..start + self.dimensionality]
    }

    /// Swaps two points in-place.
    pub fn swap_points(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let dim = self.dimensionality;
        let a_start = a * dim;
        let b_start = b * dim;
        for i in 0..dim {
            self.data.swap(a_start + i, b_start + i);
        }
    }

    /// Copy a single item from another PointDVector.
    pub fn copy_item(&mut self, source: &Self, source_index: usize, dest_index: usize) {
        draco_dcheck_lt!(source_index, source.n_items);
        draco_dcheck_lt!(dest_index, self.n_items);
        draco_dcheck_eq!(true, source.dimensionality == self.dimensionality);

        let src = source.point(source_index);
        let dst = self.point_mut(dest_index);
        dst.copy_from_slice(src);
    }

    /// Copy attribute bytes for a single item into the point vector.
    pub fn copy_attribute_bytes(
        &mut self,
        attribute_dimensionality: usize,
        offset_dimensionality: usize,
        index: usize,
        attribute_item_data: *const u8,
    ) {
        draco_dcheck_lt!(index, self.n_items);
        let dst_start = index * self.dimensionality + offset_dimensionality;
        unsafe {
            let src = std::slice::from_raw_parts(
                attribute_item_data as *const InternalT,
                attribute_dimensionality,
            );
            let dst = &mut self.data[dst_start..dst_start + attribute_dimensionality];
            dst.copy_from_slice(src);
        }
    }

    /// Copy attribute data from a contiguous buffer into the point vector.
    pub fn copy_attribute_buffer(
        &mut self,
        attribute_dimensionality: usize,
        offset_dimensionality: usize,
        attribute_mem: &[InternalT],
    ) {
        draco_dcheck_lt!(offset_dimensionality, self.dimensionality);
        if self.dimensionality == attribute_dimensionality {
            draco_dcheck_eq!(offset_dimensionality, 0);
            self.data.copy_from_slice(attribute_mem);
            return;
        }

        let copy_size = attribute_dimensionality;
        for item in 0..self.n_items {
            let dst_start = item * self.dimensionality + offset_dimensionality;
            let src_start = item * attribute_dimensionality;
            self.data[dst_start..dst_start + copy_size]
                .copy_from_slice(&attribute_mem[src_start..src_start + copy_size]);
        }
    }
}
