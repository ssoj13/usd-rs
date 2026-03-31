//! Fluent builder for constructing array edits.
//!
//! `ArrayEditBuilder` provides a convenient API for building complex array
//! edit operations with automatic literal deduplication.

use super::array_edit::ArrayEdit;
use super::array_edit_ops::{ArrayEditOpsBuilder, END_INDEX, EditOp};
use std::collections::HashMap;
use std::hash::Hash;

/// Fluent builder for constructing `ArrayEdit<T>` instances.
///
/// The builder automatically deduplicates literal values to minimize memory
/// usage. Multiple operations can reference the same literal value.
///
/// # Examples
///
/// ```
/// use usd_vt::{Array, ArrayEditBuilder};
///
/// let mut builder = ArrayEditBuilder::new();
/// builder
///     .write(42, 0)
///     .append(100)
///     .insert(50, 2)
///     .erase(1);
///
/// let edit = builder.build();
///
/// let mut array = Array::from(vec![1, 2, 3]);
/// edit.apply(&mut array);
/// ```
pub struct ArrayEditBuilder<T: Clone + Send + Sync + Default + Eq + Hash + 'static> {
    /// Literal values
    literals: Vec<T>,

    /// Map from literal to its index for deduplication
    literal_to_index: HashMap<T, i64>,

    /// Operation builder
    ops_builder: ArrayEditOpsBuilder,
}

impl<T: Clone + Send + Sync + Default + Eq + Hash + 'static> ArrayEditBuilder<T> {
    /// Creates a new empty builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let builder: ArrayEditBuilder<i32> = ArrayEditBuilder::new();
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            literals: Vec::new(),
            literal_to_index: HashMap::new(),
            ops_builder: ArrayEditOpsBuilder::new(),
        }
    }

    /// Writes a literal value to the specified index.
    ///
    /// Negative indexes are supported (Python-style): -1 is the last element.
    /// Out-of-bounds indexes are ignored when the edit is applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.write(42, 0);
    /// builder.write(99, -1); // Last element
    /// ```
    pub fn write(&mut self, elem: T, index: i64) -> &mut Self {
        let lit_idx = self.find_or_add_literal(elem);
        self.ops_builder
            .add_op2(EditOp::WriteLiteral, lit_idx, index);
        self
    }

    /// Copies the element at `src_index` to `dst_index`.
    ///
    /// Both indexes may be negative (Python-style).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.write_ref(0, 2); // Copy first element to third position
    /// ```
    pub fn write_ref(&mut self, src_index: i64, dst_index: i64) -> &mut Self {
        self.ops_builder
            .add_op2(EditOp::WriteRef, src_index, dst_index);
        self
    }

    /// Inserts a literal value at the specified index.
    ///
    /// The index may be negative or `END_INDEX` to append.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.insert(42, 1); // Insert at index 1
    /// ```
    pub fn insert(&mut self, elem: T, index: i64) -> &mut Self {
        let lit_idx = self.find_or_add_literal(elem);
        self.ops_builder
            .add_op2(EditOp::InsertLiteral, lit_idx, index);
        self
    }

    /// Inserts a copy of the element at `src_index` at `dst_index`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.insert_ref(0, 1); // Copy first element and insert at index 1
    /// ```
    pub fn insert_ref(&mut self, src_index: i64, dst_index: i64) -> &mut Self {
        self.ops_builder
            .add_op2(EditOp::InsertRef, src_index, dst_index);
        self
    }

    /// Appends a literal value to the end of the array.
    ///
    /// This is equivalent to `insert(elem, END_INDEX)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.append(42);
    /// builder.append(99);
    /// ```
    pub fn append(&mut self, elem: T) -> &mut Self {
        let lit_idx = self.find_or_add_literal(elem);
        self.ops_builder
            .add_op2(EditOp::InsertLiteral, lit_idx, END_INDEX);
        self
    }

    /// Appends a copy of the element at `src_index` to the end.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.append_ref(0); // Append copy of first element
    /// ```
    pub fn append_ref(&mut self, src_index: i64) -> &mut Self {
        self.ops_builder
            .add_op2(EditOp::InsertRef, src_index, END_INDEX);
        self
    }

    /// Prepends a literal value to the beginning of the array.
    ///
    /// This is equivalent to `insert(elem, 0)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.prepend(42);
    /// ```
    pub fn prepend(&mut self, elem: T) -> &mut Self {
        let lit_idx = self.find_or_add_literal(elem);
        self.ops_builder.add_op2(EditOp::InsertLiteral, lit_idx, 0);
        self
    }

    /// Prepends a copy of the element at `src_index` to the beginning.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.prepend_ref(2); // Prepend copy of third element
    /// ```
    pub fn prepend_ref(&mut self, src_index: i64) -> &mut Self {
        self.ops_builder.add_op2(EditOp::InsertRef, src_index, 0);
        self
    }

    /// Erases the element at the specified index.
    ///
    /// The index may be negative (Python-style).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.erase(1);
    /// builder.erase(-1); // Erase last element
    /// ```
    pub fn erase(&mut self, index: i64) -> &mut Self {
        self.ops_builder.add_op1(EditOp::EraseRef, index);
        self
    }

    /// Ensures the array has at least `size` elements.
    ///
    /// If the array is smaller, it is extended with default values.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder: ArrayEditBuilder<i32> = ArrayEditBuilder::new();
    /// builder.min_size(10);
    /// ```
    pub fn min_size(&mut self, size: usize) -> &mut Self {
        self.ops_builder.add_op1(EditOp::MinSize, size as i64);
        self
    }

    /// Ensures the array has at least `size` elements, filling with `fill`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.min_size_fill(10, 42);
    /// ```
    pub fn min_size_fill(&mut self, size: usize, fill: T) -> &mut Self {
        let lit_idx = self.find_or_add_literal(fill);
        self.ops_builder
            .add_op2(EditOp::MinSizeFill, size as i64, lit_idx);
        self
    }

    /// Ensures the array has at most `size` elements.
    ///
    /// If the array is larger, it is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder: ArrayEditBuilder<i32> = ArrayEditBuilder::new();
    /// builder.max_size(5);
    /// ```
    pub fn max_size(&mut self, size: usize) -> &mut Self {
        self.ops_builder.add_op1(EditOp::MaxSize, size as i64);
        self
    }

    /// Sets the array to exactly `size` elements.
    ///
    /// If smaller, extends with default values. If larger, truncates.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder: ArrayEditBuilder<i32> = ArrayEditBuilder::new();
    /// builder.set_size(10);
    /// ```
    pub fn set_size(&mut self, size: usize) -> &mut Self {
        self.ops_builder.add_op1(EditOp::SetSize, size as i64);
        self
    }

    /// Sets the array to exactly `size` elements, filling with `fill`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.set_size_fill(10, 42);
    /// ```
    pub fn set_size_fill(&mut self, size: usize, fill: T) -> &mut Self {
        let lit_idx = self.find_or_add_literal(fill);
        self.ops_builder
            .add_op2(EditOp::SetSizeFill, size as i64, lit_idx);
        self
    }

    /// Builds the final `ArrayEdit` and resets the builder.
    ///
    /// After calling this, the builder is empty and can be reused.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEditBuilder;
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.append(42);
    /// let edit = builder.build();
    ///
    /// assert!(builder.is_empty());
    /// ```
    pub fn build(&mut self) -> ArrayEdit<T> {
        let literals = std::mem::take(&mut self.literals);
        let ops = std::mem::replace(&mut self.ops_builder, ArrayEditOpsBuilder::new()).build();
        self.literal_to_index.clear();

        ArrayEdit { literals, ops }
    }

    /// Returns true if no operations have been added.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    // Internal: find or add a literal, returning its index
    fn find_or_add_literal(&mut self, elem: T) -> i64 {
        if let Some(&idx) = self.literal_to_index.get(&elem) {
            return idx;
        }

        let idx = self.literals.len() as i64;
        self.literals.push(elem.clone());
        self.literal_to_index.insert(elem, idx);
        idx
    }

    /// Optimizes an `ArrayEdit` by deduplicating literals and removing no-ops.
    ///
    /// This is useful after composing multiple edits together.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{ArrayEdit, ArrayEditBuilder};
    ///
    /// let edit: ArrayEdit<i32> = ArrayEdit::new();
    /// let optimized = ArrayEditBuilder::optimize(edit);
    /// assert!(optimized.is_identity());
    /// ```
    pub fn optimize(edit: ArrayEdit<T>) -> ArrayEdit<T> {
        if edit.is_identity() {
            return edit;
        }

        let mut builder = ArrayEditBuilder::new();

        // Rebuild the edit, which will deduplicate literals
        edit.ops.for_each(|op, a1, a2| match op {
            EditOp::WriteLiteral => {
                if let Some(lit) = edit.literals.get(a1 as usize) {
                    builder.write(lit.clone(), a2);
                }
            }
            EditOp::WriteRef => {
                builder.write_ref(a1, a2);
            }
            EditOp::InsertLiteral => {
                if let Some(lit) = edit.literals.get(a1 as usize) {
                    builder.insert(lit.clone(), a2);
                }
            }
            EditOp::InsertRef => {
                builder.insert_ref(a1, a2);
            }
            EditOp::EraseRef => {
                builder.erase(a1);
            }
            EditOp::MinSize => {
                builder.min_size(a1 as usize);
            }
            EditOp::MinSizeFill => {
                if let Some(lit) = edit.literals.get(a2 as usize) {
                    builder.min_size_fill(a1 as usize, lit.clone());
                }
            }
            EditOp::SetSize => {
                builder.set_size(a1 as usize);
            }
            EditOp::SetSizeFill => {
                if let Some(lit) = edit.literals.get(a2 as usize) {
                    builder.set_size_fill(a1 as usize, lit.clone());
                }
            }
            EditOp::MaxSize => {
                builder.max_size(a1 as usize);
            }
        });

        builder.build()
    }

    /// Creates serialization data for storage/transmission.
    ///
    /// Returns `(literals, instructions)` that can be used to reconstruct
    /// the edit with `from_serialization_data`.
    pub fn get_serialization_data(edit: &ArrayEdit<T>) -> (Vec<T>, Vec<i64>) {
        (edit.literals.clone(), edit.ops.instructions().to_vec())
    }

    /// Reconstructs an `ArrayEdit` from serialization data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{ArrayEdit, ArrayEditBuilder};
    ///
    /// let original: ArrayEdit<i32> = ArrayEdit::new();
    /// let (lits, ins) = ArrayEditBuilder::get_serialization_data(&original);
    /// let reconstructed = ArrayEditBuilder::from_serialization_data(lits, ins);
    /// assert_eq!(original, reconstructed);
    /// ```
    pub fn from_serialization_data(literals: Vec<T>, instructions: Vec<i64>) -> ArrayEdit<T> {
        ArrayEdit {
            literals,
            ops: super::array_edit_ops::ArrayEditOps::from_instructions(instructions),
        }
    }
}

impl<T: Clone + Send + Sync + Default + Eq + Hash + 'static> Default for ArrayEditBuilder<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Array;

    #[test]
    fn test_builder_write() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array[0], 42);
    }

    #[test]
    fn test_builder_append() {
        let mut builder = ArrayEditBuilder::new();
        builder.append(10).append(20).append(30);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2, 3, 10, 20, 30]);
    }

    #[test]
    fn test_builder_prepend() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(99);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[99, 1, 2, 3]);
    }

    #[test]
    fn test_builder_insert() {
        let mut builder = ArrayEditBuilder::new();
        builder.insert(99, 1);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 99, 2, 3]);
    }

    #[test]
    fn test_builder_erase() {
        let mut builder = ArrayEditBuilder::new();
        builder.erase(1);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 3]);
    }

    #[test]
    fn test_builder_chaining() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(10, 0).append(99).insert(50, 2).erase(1);

        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        // write 10 to 0: [10, 2, 3]
        // append 99: [10, 2, 3, 99]
        // insert 50 at 2: [10, 2, 50, 3, 99]
        // erase 1: [10, 50, 3, 99]
        assert_eq!(array.as_slice(), &[10, 50, 3, 99]);
    }

    #[test]
    fn test_literal_deduplication() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0);
        builder.write(42, 1);
        builder.write(42, 2);

        let edit = builder.build();

        // Should only have one literal despite three writes
        assert_eq!(edit.literals.len(), 1);
        assert_eq!(edit.literals[0], 42);
    }

    #[test]
    fn test_optimize_identity() {
        let edit: ArrayEdit<i32> = ArrayEdit::new();
        let optimized = ArrayEditBuilder::optimize(edit);
        assert!(optimized.is_identity());
    }

    #[test]
    fn test_optimize_deduplication() {
        // Create an edit that will have duplicate values
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0);
        builder.write(99, 1);
        builder.write(42, 2); // Duplicate value
        let edit = builder.build();

        // Should already be deduplicated by builder
        assert_eq!(edit.literals.len(), 2);

        let optimized = ArrayEditBuilder::optimize(edit);

        // After optimization, should still be deduplicated
        assert_eq!(optimized.literals.len(), 2);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0).append(99);
        let original = builder.build();

        let (lits, ins) = ArrayEditBuilder::get_serialization_data(&original);
        let reconstructed = ArrayEditBuilder::from_serialization_data(lits, ins);

        assert_eq!(original, reconstructed);
    }

    #[test]
    fn test_builder_reset_after_build() {
        let mut builder = ArrayEditBuilder::new();
        builder.append(42);
        let _edit = builder.build();

        assert!(builder.is_empty());
    }

    #[test]
    fn test_set_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.set_size(5);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.len(), 5);
    }

    #[test]
    fn test_min_size_fill() {
        let mut builder = ArrayEditBuilder::new();
        builder.min_size_fill(5, 99);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2, 99, 99, 99]);
    }

    #[test]
    fn test_max_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.max_size(2);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3, 4]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2]);
    }

    #[test]
    fn test_write_ref() {
        let mut builder = ArrayEditBuilder::new();
        builder.write_ref(0, 2);
        let edit = builder.build();

        let mut array = Array::from(vec![99, 20, 30]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[99, 20, 99]);
    }

    #[test]
    fn test_insert_ref() {
        let mut builder = ArrayEditBuilder::new();
        builder.insert_ref(0, 1);
        let edit = builder.build();

        let mut array = Array::from(vec![99, 20, 30]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[99, 99, 20, 30]);
    }

    #[test]
    fn test_append_ref() {
        let mut builder = ArrayEditBuilder::new();
        builder.append_ref(0);
        let edit = builder.build();

        let mut array = Array::from(vec![99, 20, 30]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[99, 20, 30, 99]);
    }

    #[test]
    fn test_prepend_ref() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend_ref(2);
        let edit = builder.build();

        let mut array = Array::from(vec![10, 20, 99]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[99, 10, 20, 99]);
    }
}
