// Copyright 2017 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/sparseMatrix.h

/// Compressed Sparse Row (CSR) matrix.
///
/// Used by `PatchBuilder` to store coefficients for patch-point stencils.
/// Each row corresponds to one output point; each column index references
/// a contributing source point.
///
/// Mirrors C++ `Far::SparseMatrix<REAL>`.
pub struct SparseMatrix<R: Copy + Default> {
    num_rows: i32,
    num_columns: i32,
    num_elements: i32,
    /// row_offsets has num_rows+1 entries; row_offsets[i] is the index
    /// into columns/elements where row i begins.
    row_offsets: Vec<i32>,
    columns: Vec<i32>,
    elements: Vec<R>,
}

impl<R: Copy + Default> SparseMatrix<R> {
    pub fn new() -> Self {
        Self {
            num_rows: 0,
            num_columns: 0,
            num_elements: 0,
            row_offsets: Vec::new(),
            columns: Vec::new(),
            elements: Vec::new(),
        }
    }

    pub fn get_num_rows(&self) -> i32 {
        self.num_rows
    }
    pub fn get_num_columns(&self) -> i32 {
        self.num_columns
    }
    pub fn get_num_elements(&self) -> i32 {
        self.num_elements
    }

    /// Current capacity (allocated element count).
    pub fn get_capacity(&self) -> i32 {
        self.elements.len() as i32
    }

    pub fn get_row_size(&self, row: i32) -> i32 {
        self.row_offsets[(row + 1) as usize] - self.row_offsets[row as usize]
    }

    pub fn get_row_columns(&self, row: i32) -> &[i32] {
        let start = self.row_offsets[row as usize] as usize;
        let end = self.row_offsets[(row + 1) as usize] as usize;
        &self.columns[start..end]
    }

    pub fn get_row_elements(&self, row: i32) -> &[R] {
        let start = self.row_offsets[row as usize] as usize;
        let end = self.row_offsets[(row + 1) as usize] as usize;
        &self.elements[start..end]
    }

    pub fn get_row_columns_mut(&mut self, row: i32) -> &mut [i32] {
        let start = self.row_offsets[row as usize] as usize;
        let end = self.row_offsets[(row + 1) as usize] as usize;
        &mut self.columns[start..end]
    }

    pub fn get_row_elements_mut(&mut self, row: i32) -> &mut [R] {
        let start = self.row_offsets[row as usize] as usize;
        let end = self.row_offsets[(row + 1) as usize] as usize;
        &mut self.elements[start..end]
    }

    /// Return mutable slices for both the column indices and element weights of a row
    /// in a single call, avoiding the double-borrow limitation of two separate calls.
    /// Safe because `columns` and `elements` are distinct Vec fields with no aliasing.
    pub fn get_row_data_mut(&mut self, row: i32) -> (&mut [i32], &mut [R]) {
        let start = self.row_offsets[row as usize] as usize;
        let end = self.row_offsets[(row + 1) as usize] as usize;
        (
            &mut self.columns[start..end],
            &mut self.elements[start..end],
        )
    }

    pub fn get_columns(&self) -> &[i32] {
        &self.columns[..self.num_elements as usize]
    }
    pub fn get_elements(&self) -> &[R] {
        &self.elements[..self.num_elements as usize]
    }

    /// (Re)initialise the matrix, reserving storage for `num_elements_to_reserve`
    /// non-zero entries.  Row sizes must still be set via `set_row_size`.
    pub fn resize(&mut self, num_rows: i32, num_cols: i32, num_elements_to_reserve: i32) {
        self.num_rows = num_rows;
        self.num_columns = num_cols;
        self.num_elements = 0;

        self.row_offsets.clear();
        self.row_offsets.resize((num_rows + 1) as usize, -1);
        self.row_offsets[0] = 0;

        if num_elements_to_reserve > self.get_capacity() {
            let n = num_elements_to_reserve as usize;
            self.columns.resize(n, 0);
            self.elements.resize(n, R::default());
        }
    }

    /// Declare the size of row `row_index`.
    /// Must be called for each row in order (0, 1, …, num_rows-1).
    pub fn set_row_size(&mut self, row_index: i32, row_size: i32) {
        debug_assert_eq!(
            self.row_offsets[row_index as usize], self.num_elements,
            "set_row_size must be called in row order"
        );
        let new_end = self.row_offsets[row_index as usize] + row_size;
        self.row_offsets[(row_index + 1) as usize] = new_end;
        self.num_elements = new_end;
        if new_end > self.get_capacity() {
            self.columns.resize(new_end as usize, 0);
            self.elements.resize(new_end as usize, R::default());
        }
    }

    /// Deep-copy from `src`.
    pub fn copy_from(&mut self, src: &SparseMatrix<R>) {
        self.num_rows = src.num_rows;
        self.num_columns = src.num_columns;
        self.num_elements = src.num_elements;
        self.row_offsets = src.row_offsets.clone();
        self.columns = src.columns.clone();
        self.elements = src.elements.clone();
    }

    /// Write column indices and weights into a pre-sized row in a single call.
    ///
    /// Avoids the E0499 double-mutable-borrow pattern that arises when calling
    /// `get_row_columns_mut` and `get_row_elements_mut` in the same scope.
    pub fn assign_row(&mut self, row: i32, indices: &[i32], weights: &[R]) {
        let start = self.row_offsets[row as usize] as usize;
        let len = indices.len();
        self.columns[start..start + len].copy_from_slice(indices);
        self.elements[start..start + len].copy_from_slice(weights);
    }

    /// Swap contents with another matrix.
    pub fn swap(&mut self, other: &mut SparseMatrix<R>) {
        std::mem::swap(&mut self.num_rows, &mut other.num_rows);
        std::mem::swap(&mut self.num_columns, &mut other.num_columns);
        std::mem::swap(&mut self.num_elements, &mut other.num_elements);
        std::mem::swap(&mut self.row_offsets, &mut other.row_offsets);
        std::mem::swap(&mut self.columns, &mut other.columns);
        std::mem::swap(&mut self.elements, &mut other.elements);
    }
}

impl<R: Copy + Default> Default for SparseMatrix<R> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_usage() {
        let mut m: SparseMatrix<f32> = SparseMatrix::new();
        m.resize(3, 4, 6);

        m.set_row_size(0, 2);
        m.set_row_size(1, 3);
        m.set_row_size(2, 1);

        assert_eq!(m.get_num_elements(), 6);
        assert_eq!(m.get_row_size(0), 2);
        assert_eq!(m.get_row_size(1), 3);
        assert_eq!(m.get_row_size(2), 1);

        // Write columns
        {
            let c = m.get_row_columns_mut(0);
            c[0] = 1;
            c[1] = 3;
        }
        {
            let e = m.get_row_elements_mut(0);
            e[0] = 0.5;
            e[1] = 0.5;
        }

        let cols = m.get_row_columns(0);
        assert_eq!(cols, &[1, 3]);
        let elems = m.get_row_elements(0);
        assert!((elems[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn swap() {
        let mut a: SparseMatrix<f32> = SparseMatrix::new();
        a.resize(2, 3, 2);
        a.set_row_size(0, 1);
        a.set_row_size(1, 1);

        let mut b: SparseMatrix<f32> = SparseMatrix::new();
        a.swap(&mut b);

        assert_eq!(b.get_num_rows(), 2);
        assert_eq!(a.get_num_rows(), 0);
    }
}
