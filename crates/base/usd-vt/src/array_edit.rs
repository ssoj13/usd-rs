//! Array edit type representing a sequence of modifications to an array.
//!
//! `ArrayEdit<T>` forms a monoid under composition, where the identity is an
//! empty edit that performs no modifications. This algebraic structure is
//! critical for SDF list editing operations.

use super::array::Array;
use super::array_edit_ops::{ArrayEditOps, END_INDEX, EditOp};
use super::traits::VtArrayEdit;
use std::fmt;

/// Represents a sequence of edits to be applied to a `VtArray<T>`.
///
/// An `ArrayEdit` consists of:
/// - A collection of literal values referenced by edit operations
/// - A sequence of operations that modify the array
///
/// # Monoid Structure
///
/// `ArrayEdit` forms a monoid under `compose_over`:
/// - **Associativity**: `(a.compose_over(b)).compose_over(c) == a.compose_over(b.compose_over(c))`
/// - **Identity**: `ArrayEdit::default()` is the identity element
///
/// # Examples
///
/// ```
/// use usd_vt::{Array, ArrayEdit, ArrayEditBuilder};
///
/// let mut builder = ArrayEditBuilder::new();
/// builder.write(42, 0);
/// builder.append(100);
/// let edit = builder.build();
///
/// let mut array = Array::from(vec![1, 2, 3]);
/// edit.apply(&mut array);
/// assert_eq!(array[0], 42);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ArrayEdit<T: Clone + Send + Sync + Default + 'static> {
    /// Literal values referenced by operations
    pub(crate) literals: Vec<T>,

    /// Sequence of edit operations
    pub(crate) ops: ArrayEditOps,
}

impl<T: Clone + Send + Sync + Default + 'static> ArrayEdit<T> {
    /// Creates a new empty (identity) edit.
    ///
    /// The identity edit performs no modifications and acts as the neutral
    /// element in the monoid.
    #[inline]
    pub fn new() -> Self {
        Self {
            literals: Vec::new(),
            ops: ArrayEditOps::new(),
        }
    }

    /// Returns true if this is the identity edit (performs no operations).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::ArrayEdit;
    ///
    /// let edit: ArrayEdit<i32> = ArrayEdit::new();
    /// assert!(edit.is_identity());
    /// ```
    #[inline]
    pub fn is_identity(&self) -> bool {
        self.ops.is_empty()
    }

    /// Returns a reference to the literal values.
    #[inline]
    pub fn literals(&self) -> &[T] {
        &self.literals
    }

    /// Returns a mutable reference to the literal values.
    ///
    /// This can be useful for transforming or translating element values
    /// without reconstructing the entire edit.
    #[inline]
    pub fn literals_mut(&mut self) -> &mut [T] {
        &mut self.literals
    }

    /// Returns a reference to the operations.
    #[inline]
    pub fn ops(&self) -> &ArrayEditOps {
        &self.ops
    }

    /// Applies this edit to an array in-place.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Array, ArrayEditBuilder};
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.write(99, 1);
    /// let edit = builder.build();
    ///
    /// let mut array = Array::from(vec![1, 2, 3]);
    /// edit.apply(&mut array);
    /// assert_eq!(array.as_slice(), &[1, 99, 3]);
    /// ```
    pub fn apply(&self, array: &mut Array<T>) {
        if self.is_identity() {
            return;
        }

        self.apply_edits(array);
    }

    /// Composes this edit over a weaker edit.
    ///
    /// Returns a new edit representing the function composition where `weaker`
    /// is applied first, followed by `self`. This is the monoid operation.
    ///
    /// # Arguments
    /// - `weaker`: The edit to apply first (inner function)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Array, ArrayEditBuilder};
    ///
    /// let mut b1 = ArrayEditBuilder::new();
    /// b1.append(10);
    /// let edit1 = b1.build();
    ///
    /// let mut b2 = ArrayEditBuilder::new();
    /// b2.append(20);
    /// let edit2 = b2.build();
    ///
    /// let composed = edit2.compose_over(&edit1);
    ///
    /// let mut array = Array::from(vec![1, 2, 3]);
    /// composed.apply(&mut array);
    /// assert_eq!(array.len(), 5); // Original 3 + two appends
    /// ```
    pub fn compose_over(&self, weaker: &ArrayEdit<T>) -> ArrayEdit<T> {
        if self.is_identity() {
            return weaker.clone();
        }
        if weaker.is_identity() {
            return self.clone();
        }

        self.compose_edits(weaker)
    }

    /// Applies this edit over a weaker array, returning the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Array, ArrayEditBuilder};
    ///
    /// let mut builder = ArrayEditBuilder::new();
    /// builder.write(42, 0);
    /// let edit = builder.build();
    ///
    /// let array = Array::from(vec![1, 2, 3]);
    /// let result = edit.apply_to(array);
    /// assert_eq!(result[0], 42);
    /// ```
    pub fn apply_to(&self, mut array: Array<T>) -> Array<T> {
        self.apply(&mut array);
        array
    }

    // Internal: apply edits to array
    fn apply_edits(&self, result: &mut Array<T>) {
        let num_literals = self.literals.len();
        let initial_size = result.len();

        self.ops
            .for_each_valid(num_literals, initial_size, |op, a1, a2| {
                match op {
                    EditOp::WriteLiteral => {
                        result[a2 as usize] = self.literals[a1 as usize].clone();
                    }
                    EditOp::WriteRef => {
                        let val = result[a1 as usize].clone();
                        result[a2 as usize] = val;
                    }
                    EditOp::InsertLiteral => {
                        let val = self.literals[a1 as usize].clone();
                        // Convert to Vec, insert, convert back
                        let mut vec = std::mem::take(result).into_vec();
                        vec.insert(a2 as usize, val);
                        *result = Array::from(vec);
                    }
                    EditOp::InsertRef => {
                        let val = result[a1 as usize].clone();
                        let mut vec = std::mem::take(result).into_vec();
                        vec.insert(a2 as usize, val);
                        *result = Array::from(vec);
                    }
                    EditOp::EraseRef => {
                        let mut vec = std::mem::take(result).into_vec();
                        vec.remove(a1 as usize);
                        *result = Array::from(vec);
                    }
                    EditOp::MinSize => {
                        let target_size = a1 as usize;
                        if result.len() < target_size {
                            result.resize(target_size, T::default());
                        }
                    }
                    EditOp::MinSizeFill => {
                        let target_size = a1 as usize;
                        if result.len() < target_size {
                            result.resize(target_size, self.literals[a2 as usize].clone());
                        }
                    }
                    EditOp::SetSize => {
                        result.resize(a1 as usize, T::default());
                    }
                    EditOp::SetSizeFill => {
                        result.resize(a1 as usize, self.literals[a2 as usize].clone());
                    }
                    EditOp::MaxSize => {
                        let max_size = a1 as usize;
                        if result.len() > max_size {
                            result.resize(max_size, T::default());
                        }
                    }
                }
            });
    }

    // Internal: compose two edits
    fn compose_edits(&self, weaker: &ArrayEdit<T>) -> ArrayEdit<T> {
        // Concatenate literals: weaker's literals first, then ours
        let mut new_literals = weaker.literals.clone();
        let num_weaker_literals = new_literals.len();
        new_literals.extend_from_slice(&self.literals);

        // Clone weaker's ops
        let new_ops = weaker.ops.clone();

        // Bump literal indexes in our ops by weaker's literal count
        let mut our_ops = self.ops.clone();
        our_ops.modify_each(|op, a1, _a2| {
            match op {
                EditOp::WriteLiteral | EditOp::InsertLiteral => {
                    *a1 += num_weaker_literals as i64;
                }
                EditOp::MinSizeFill | EditOp::SetSizeFill => {
                    // a2 is the literal index for fill operations
                    // We need to handle this in a second pass
                }
                _ => {}
            }
        });

        // Fix a2 for fill operations
        our_ops.modify_each(|op, _a1, a2| match op {
            EditOp::MinSizeFill | EditOp::SetSizeFill => {
                *a2 += num_weaker_literals as i64;
            }
            _ => {}
        });

        // Append our ops to weaker's
        let weaker_ins = new_ops.instructions().to_vec();
        let our_ins = our_ops.instructions();

        let mut combined_ins = weaker_ins;
        combined_ins.extend_from_slice(our_ins);

        ArrayEdit {
            literals: new_literals,
            ops: ArrayEditOps::from_instructions(combined_ins),
        }
    }
}

// Implement VtArrayEdit trait marker
impl<T: Clone + Send + Sync + Default + 'static> VtArrayEdit for ArrayEdit<T> {}

impl<T: Clone + Send + Sync + Default + 'static> Default for ArrayEdit<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + Default + fmt::Debug + 'static> fmt::Debug for ArrayEdit<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_identity() {
            return write!(f, "ArrayEdit(identity)");
        }

        f.write_str("ArrayEdit [")?;
        let mut first = true;

        self.ops.for_each(|op, a1, a2| {
            if !first {
                let _ = write!(f, "; ");
            }
            first = false;

            match op {
                EditOp::WriteLiteral => {
                    let _ = write!(f, "write literal[{}] to [{}]", a1, a2);
                }
                EditOp::WriteRef => {
                    let _ = write!(f, "write [{}] to [{}]", a1, a2);
                }
                EditOp::InsertLiteral => {
                    if a2 == END_INDEX {
                        let _ = write!(f, "append literal[{}]", a1);
                    } else {
                        let _ = write!(f, "insert literal[{}] at [{}]", a1, a2);
                    }
                }
                EditOp::InsertRef => {
                    if a2 == END_INDEX {
                        let _ = write!(f, "append [{}]", a1);
                    } else {
                        let _ = write!(f, "insert [{}] at [{}]", a1, a2);
                    }
                }
                EditOp::EraseRef => {
                    let _ = write!(f, "erase [{}]", a1);
                }
                EditOp::MinSize => {
                    let _ = write!(f, "minsize {}", a1);
                }
                EditOp::MinSizeFill => {
                    let _ = write!(f, "minsize {} fill literal[{}]", a1, a2);
                }
                EditOp::SetSize => {
                    let _ = write!(f, "resize {}", a1);
                }
                EditOp::SetSizeFill => {
                    let _ = write!(f, "resize {} fill literal[{}]", a1, a2);
                }
                EditOp::MaxSize => {
                    let _ = write!(f, "maxsize {}", a1);
                }
            }
        });

        f.write_str("]")
    }
}

impl<T: Clone + Send + Sync + Default + fmt::Debug + fmt::Display + 'static> fmt::Display
    for ArrayEdit<T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArrayEditBuilder;

    #[test]
    fn test_identity() {
        let edit: ArrayEdit<i32> = ArrayEdit::new();
        assert!(edit.is_identity());

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_write_literal() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(99, 1);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 99, 3]);
    }

    #[test]
    fn test_write_ref() {
        let mut builder = ArrayEditBuilder::new();
        builder.write_ref(0, 2);
        let edit = builder.build();

        let mut array = Array::from(vec![10, 20, 30]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[10, 20, 10]);
    }

    #[test]
    fn test_insert_literal() {
        let mut builder = ArrayEditBuilder::new();
        builder.insert(99, 1);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 99, 2, 3]);
    }

    #[test]
    fn test_append() {
        let mut builder = ArrayEditBuilder::new();
        builder.append(99);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2, 3, 99]);
    }

    #[test]
    fn test_prepend() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(99);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[99, 1, 2, 3]);
    }

    #[test]
    fn test_erase() {
        let mut builder = ArrayEditBuilder::new();
        builder.erase(1);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 3]);
    }

    #[test]
    fn test_negative_index() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(99, -1); // Last element
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2, 99]);
    }

    #[test]
    fn test_set_size_grow() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.set_size(5);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.len(), 5);
    }

    #[test]
    fn test_set_size_shrink() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.set_size(2);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3, 4]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2]);
    }

    #[test]
    fn test_min_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.min_size(5);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);
        assert_eq!(array.len(), 5);

        let mut array2 = Array::from(vec![1, 2, 3, 4, 5, 6]);
        edit.apply(&mut array2);
        assert_eq!(array2.len(), 6); // Unchanged
    }

    #[test]
    fn test_max_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.max_size(2);
        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3, 4]);
        edit.apply(&mut array);
        assert_eq!(array.as_slice(), &[1, 2]);

        let mut array2 = Array::from(vec![1]);
        edit.apply(&mut array2);
        assert_eq!(array2.len(), 1); // Unchanged
    }

    #[test]
    fn test_compose_identity() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(99, 0);
        let edit = builder.build();

        let identity: ArrayEdit<i32> = ArrayEdit::new();

        let composed1 = edit.compose_over(&identity);
        let composed2 = identity.compose_over(&edit);

        let mut array1 = Array::from(vec![1, 2, 3]);
        let mut array2 = Array::from(vec![1, 2, 3]);

        composed1.apply(&mut array1);
        composed2.apply(&mut array2);

        assert_eq!(array1[0], 99);
        assert_eq!(array2[0], 99);
    }

    #[test]
    fn test_compose_multiple_edits() {
        let mut b1 = ArrayEditBuilder::new();
        b1.append(10);
        let edit1 = b1.build();

        let mut b2 = ArrayEditBuilder::new();
        b2.append(20);
        let edit2 = b2.build();

        let composed = edit2.compose_over(&edit1);

        let mut array = Array::from(vec![1, 2, 3]);
        composed.apply(&mut array);

        assert_eq!(array.as_slice(), &[1, 2, 3, 10, 20]);
    }

    #[test]
    fn test_apply_to() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0);
        let edit = builder.build();

        let array = Array::from(vec![1, 2, 3]);
        let result = edit.apply_to(array);

        assert_eq!(result[0], 42);
        assert_eq!(result.as_slice(), &[42, 2, 3]);
    }
}
