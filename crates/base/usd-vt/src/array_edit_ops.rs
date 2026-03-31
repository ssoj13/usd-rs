//! Low-level array edit operations and instruction encoding.
//!
//! This module provides the foundational types for encoding sequences of array
//! edit operations in a compact, efficient format. Operations are stored as
//! packed 64-bit integers with opcodes and argument counts.

use std::fmt;

/// Edit operations that can be performed on arrays.
///
/// Each operation specifies how to modify an array element or the array's size.
/// Operations are encoded with their arguments into a compact instruction stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EditOp {
    /// Write a literal value to an index: `array[index] = literal`
    WriteLiteral = 0,

    /// Copy from one index to another: `array[dst] = array[src]`
    WriteRef = 1,

    /// Insert a literal value at an index
    InsertLiteral = 2,

    /// Insert a copy of an element at an index
    InsertRef = 3,

    /// Erase the element at an index
    EraseRef = 4,

    /// Ensure minimum size (resize with default values if needed)
    MinSize = 5,

    /// Ensure minimum size (resize with fill value if needed)
    MinSizeFill = 6,

    /// Set exact size (resize with default values)
    SetSize = 7,

    /// Set exact size (resize with fill value)
    SetSizeFill = 8,

    /// Ensure maximum size (truncate if needed)
    MaxSize = 9,
}

impl EditOp {
    /// Total number of operation types
    pub const NUM_OPS: u8 = 10;

    /// Returns the number of arguments this operation requires.
    ///
    /// # Returns
    /// - `2` for operations with two arguments (write/insert literal/ref, size with fill)
    /// - `1` for operations with one argument (erase, size operations)
    #[inline]
    pub const fn arity(self) -> usize {
        match self {
            EditOp::WriteLiteral
            | EditOp::WriteRef
            | EditOp::InsertLiteral
            | EditOp::InsertRef
            | EditOp::MinSizeFill
            | EditOp::SetSizeFill => 2,

            EditOp::EraseRef | EditOp::MinSize | EditOp::SetSize | EditOp::MaxSize => 1,
        }
    }

    /// Converts from u8 representation
    #[inline]
    #[allow(unsafe_code)]
    pub const fn from_u8(val: u8) -> Option<Self> {
        if val < Self::NUM_OPS {
            // SAFETY: We validated val is in range [0, NUM_OPS), which matches all enum variants
            Some(unsafe { std::mem::transmute(val) })
        } else {
            None
        }
    }

    /// Converts to u8 representation
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for EditOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditOp::WriteLiteral => write!(f, "write-literal"),
            EditOp::WriteRef => write!(f, "write-ref"),
            EditOp::InsertLiteral => write!(f, "insert-literal"),
            EditOp::InsertRef => write!(f, "insert-ref"),
            EditOp::EraseRef => write!(f, "erase"),
            EditOp::MinSize => write!(f, "minsize"),
            EditOp::MinSizeFill => write!(f, "minsize-fill"),
            EditOp::SetSize => write!(f, "resize"),
            EditOp::SetSizeFill => write!(f, "resize-fill"),
            EditOp::MaxSize => write!(f, "maxsize"),
        }
    }
}

/// Special index value representing "end of array" for insert operations.
///
/// This allows appending elements without knowing the array's current size.
pub const END_INDEX: i64 = i64::MIN;

/// Packed operation and count stored in a single 64-bit integer.
///
/// Layout: [56 bits: count | 8 bits: op]
#[derive(Clone, Copy, PartialEq, Eq)]
struct OpAndCount {
    /// Number of times to repeat this operation (56 bits)
    count: i64,
    /// The operation type (8 bits)
    op: u8,
}

impl OpAndCount {
    /// Packs op and count into a single i64
    #[inline]
    fn pack(op: EditOp, count: i64) -> i64 {
        debug_assert!((0..(1i64 << 56)).contains(&count), "count out of range");
        (count << 8) | (op.as_u8() as i64)
    }

    /// Unpacks i64 into op and count
    #[inline]
    fn unpack(packed: i64) -> Option<(EditOp, i64)> {
        let op_byte = (packed & 0xFF) as u8;
        let count = packed >> 8;

        if count < 0 {
            return None;
        }

        EditOp::from_u8(op_byte).map(|op| (op, count))
    }
}

/// Container for a sequence of array edit operations.
///
/// Operations are encoded as a sequence of i64 values:
/// - First i64: packed operation and count
/// - Following i64s: arguments (number depends on operation arity)
///
/// # Example Encoding
///
/// ```text
/// resize 1024
/// write <literal 0> to [2]
/// write <literal 1> to [4]
/// erase [9]
/// ```
///
/// Encoded as:
/// ```text
/// [1 << 8 | SetSize] [1024]
/// [2 << 8 | WriteLiteral] [0] [2] [1] [4]
/// [1 << 8 | EraseRef] [9]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArrayEditOps {
    /// Packed instruction stream
    instructions: Vec<i64>,
}

impl ArrayEditOps {
    /// Creates an empty operation sequence
    #[inline]
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }

    /// Returns true if there are no operations
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Returns the raw instruction vector (for serialization)
    #[inline]
    pub fn instructions(&self) -> &[i64] {
        &self.instructions
    }

    /// Creates from raw instruction vector (for deserialization)
    #[inline]
    pub fn from_instructions(instructions: Vec<i64>) -> Self {
        Self { instructions }
    }

    /// Iterates over all operations, validating and normalizing indexes.
    ///
    /// # Arguments
    /// - `num_literals`: Number of literal values available
    /// - `initial_size`: Initial array size for index normalization
    /// - `f`: Callback invoked for each valid operation
    pub fn for_each_valid<F>(&self, num_literals: usize, initial_size: usize, mut f: F)
    where
        F: FnMut(EditOp, i64, i64),
    {
        let mut working_size = initial_size;
        let mut iter = self.instructions.iter().copied();

        while let Some(packed) = iter.next() {
            let Some((op, count)) = OpAndCount::unpack(packed) else {
                eprintln!("Invalid packed operation: {:#x}", packed);
                return;
            };

            let arity = op.arity();

            for _ in 0..count {
                // Read arguments
                let a1 = if arity >= 1 {
                    iter.next().unwrap_or(0)
                } else {
                    0
                };

                let a2 = if arity >= 2 {
                    iter.next().unwrap_or(0)
                } else {
                    -1
                };

                // Validate and normalize
                let valid = match op {
                    EditOp::WriteLiteral => {
                        check_literal_idx(a1, num_literals)
                            && normalize_and_check_ref_idx(a2, working_size).is_some()
                    }
                    EditOp::WriteRef => {
                        normalize_and_check_ref_idx(a1, working_size).is_some()
                            && normalize_and_check_ref_idx(a2, working_size).is_some()
                    }
                    EditOp::InsertLiteral => {
                        check_literal_idx(a1, num_literals)
                            && normalize_and_check_insert_idx(a2, working_size).is_some()
                    }
                    EditOp::InsertRef => {
                        normalize_and_check_ref_idx(a1, working_size).is_some()
                            && normalize_and_check_insert_idx(a2, working_size).is_some()
                    }
                    EditOp::EraseRef => normalize_and_check_ref_idx(a1, working_size).is_some(),
                    EditOp::MinSizeFill => {
                        check_size_arg(a1) && check_literal_idx(a2, num_literals)
                    }
                    EditOp::SetSizeFill => {
                        check_size_arg(a1) && check_literal_idx(a2, num_literals)
                    }
                    EditOp::MinSize | EditOp::SetSize | EditOp::MaxSize => check_size_arg(a1),
                };

                if !valid {
                    continue;
                }

                // Normalize indexes
                let (norm_a1, norm_a2) = match op {
                    EditOp::WriteLiteral => (
                        a1,
                        normalize_and_check_ref_idx(a2, working_size).expect("idx normalized"),
                    ),
                    EditOp::WriteRef => (
                        normalize_and_check_ref_idx(a1, working_size).expect("idx normalized"),
                        normalize_and_check_ref_idx(a2, working_size).expect("idx normalized"),
                    ),
                    EditOp::InsertLiteral => (
                        a1,
                        normalize_and_check_insert_idx(a2, working_size).expect("idx normalized"),
                    ),
                    EditOp::InsertRef => (
                        normalize_and_check_ref_idx(a1, working_size).expect("idx normalized"),
                        normalize_and_check_insert_idx(a2, working_size).expect("idx normalized"),
                    ),
                    EditOp::EraseRef => (
                        normalize_and_check_ref_idx(a1, working_size).expect("idx normalized"),
                        a2,
                    ),
                    _ => (a1, a2),
                };

                // Update working size for operations that change it
                match op {
                    EditOp::InsertLiteral | EditOp::InsertRef => {
                        working_size += 1;
                    }
                    EditOp::EraseRef => {
                        working_size = working_size.saturating_sub(1);
                    }
                    EditOp::MinSize | EditOp::MinSizeFill => {
                        working_size = working_size.max(a1 as usize);
                    }
                    EditOp::SetSize | EditOp::SetSizeFill => {
                        working_size = a1 as usize;
                    }
                    EditOp::MaxSize => {
                        working_size = working_size.min(a1 as usize);
                    }
                    _ => {}
                }

                f(op, norm_a1, norm_a2);
            }
        }
    }

    /// Iterates over all operations without validation or normalization.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(EditOp, i64, i64),
    {
        let mut iter = self.instructions.iter().copied();

        while let Some(packed) = iter.next() {
            let Some((op, count)) = OpAndCount::unpack(packed) else {
                continue;
            };

            let arity = op.arity();

            for _ in 0..count {
                let a1 = if arity >= 1 {
                    iter.next().unwrap_or(0)
                } else {
                    0
                };
                let a2 = if arity >= 2 {
                    iter.next().unwrap_or(0)
                } else {
                    -1
                };

                f(op, a1, a2);
            }
        }
    }

    /// Iterates and allows modification of arguments.
    pub fn modify_each<F>(&mut self, mut f: F)
    where
        F: FnMut(EditOp, &mut i64, &mut i64),
    {
        let mut i = 0;

        while i < self.instructions.len() {
            let packed = self.instructions[i];
            let Some((op, count)) = OpAndCount::unpack(packed) else {
                i += 1;
                continue;
            };

            i += 1;
            let arity = op.arity();

            for _ in 0..count {
                let mut a1 = if arity >= 1 && i < self.instructions.len() {
                    self.instructions[i]
                } else {
                    0
                };

                let mut a2 = if arity >= 2 && i + 1 < self.instructions.len() {
                    self.instructions[i + 1]
                } else {
                    -1
                };

                f(op, &mut a1, &mut a2);

                if arity >= 1 && i < self.instructions.len() {
                    self.instructions[i] = a1;
                }
                if arity >= 2 && i + 1 < self.instructions.len() {
                    self.instructions[i + 1] = a2;
                }

                i += arity;
            }
        }
    }
}

// Helper functions for index validation

#[inline]
fn check_literal_idx(idx: i64, size: usize) -> bool {
    idx >= 0 && (idx as usize) < size
}

#[inline]
fn normalize_and_check_ref_idx(mut idx: i64, size: usize) -> Option<i64> {
    if idx < 0 {
        idx += size as i64;
    }
    if idx >= 0 && (idx as usize) < size {
        Some(idx)
    } else {
        None
    }
}

#[inline]
fn normalize_and_check_insert_idx(mut idx: i64, size: usize) -> Option<i64> {
    if idx == END_INDEX {
        return Some(size as i64);
    }
    if idx < 0 {
        idx += size as i64;
    }
    if idx >= 0 && (idx as usize) <= size {
        Some(idx)
    } else {
        None
    }
}

#[inline]
fn check_size_arg(arg: i64) -> bool {
    arg >= 0
}

/// Builder for ArrayEditOps with operation batching.
#[derive(Debug, Default)]
pub struct ArrayEditOpsBuilder {
    instructions: Vec<i64>,
    last_op_idx: usize,
}

impl ArrayEditOpsBuilder {
    /// Creates a new builder
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an operation with two arguments
    pub fn add_op2(&mut self, op: EditOp, a1: i64, a2: i64) {
        debug_assert_eq!(op.arity(), 2, "operation requires 2 arguments");

        // Try to batch with previous operation
        if can_batch(&self.instructions, self.last_op_idx, op) {
            increment_count(&mut self.instructions, self.last_op_idx);
            self.instructions.push(a1);
            self.instructions.push(a2);
        } else {
            self.last_op_idx = self.instructions.len();
            self.instructions.push(OpAndCount::pack(op, 1));
            self.instructions.push(a1);
            self.instructions.push(a2);
        }
    }

    /// Adds an operation with one argument
    pub fn add_op1(&mut self, op: EditOp, a1: i64) {
        debug_assert_eq!(op.arity(), 1, "operation requires 1 argument");

        if can_batch(&self.instructions, self.last_op_idx, op) {
            increment_count(&mut self.instructions, self.last_op_idx);
            self.instructions.push(a1);
        } else {
            self.last_op_idx = self.instructions.len();
            self.instructions.push(OpAndCount::pack(op, 1));
            self.instructions.push(a1);
        }
    }

    /// Builds the final ArrayEditOps
    #[inline]
    pub fn build(self) -> ArrayEditOps {
        ArrayEditOps {
            instructions: self.instructions,
        }
    }
}

#[inline]
fn can_batch(ins: &[i64], last_op_idx: usize, op: EditOp) -> bool {
    if last_op_idx >= ins.len() {
        return false;
    }

    let Some((last_op, count)) = OpAndCount::unpack(ins[last_op_idx]) else {
        return false;
    };

    last_op == op && count < (1i64 << 55) // Leave room to increment
}

#[inline]
fn increment_count(ins: &mut [i64], idx: usize) {
    if let Some((op, count)) = OpAndCount::unpack(ins[idx]) {
        ins[idx] = OpAndCount::pack(op, count + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_op_arity() {
        assert_eq!(EditOp::WriteLiteral.arity(), 2);
        assert_eq!(EditOp::WriteRef.arity(), 2);
        assert_eq!(EditOp::InsertLiteral.arity(), 2);
        assert_eq!(EditOp::InsertRef.arity(), 2);
        assert_eq!(EditOp::EraseRef.arity(), 1);
        assert_eq!(EditOp::MinSize.arity(), 1);
        assert_eq!(EditOp::MinSizeFill.arity(), 2);
        assert_eq!(EditOp::SetSize.arity(), 1);
        assert_eq!(EditOp::SetSizeFill.arity(), 2);
        assert_eq!(EditOp::MaxSize.arity(), 1);
    }

    #[test]
    fn test_op_and_count_packing() {
        let packed = OpAndCount::pack(EditOp::WriteLiteral, 42);
        let (op, count) = OpAndCount::unpack(packed).unwrap();
        assert_eq!(op, EditOp::WriteLiteral);
        assert_eq!(count, 42);
    }

    #[test]
    fn test_builder_batching() {
        let mut builder = ArrayEditOpsBuilder::new();
        builder.add_op2(EditOp::WriteLiteral, 0, 1);
        builder.add_op2(EditOp::WriteLiteral, 1, 2);
        builder.add_op1(EditOp::EraseRef, 5);

        let ops = builder.build();

        // Should batch the two WriteLiteral ops
        let mut count = 0;
        ops.for_each(|op, a1, a2| {
            match count {
                0 => {
                    assert_eq!(op, EditOp::WriteLiteral);
                    assert_eq!(a1, 0);
                    assert_eq!(a2, 1);
                }
                1 => {
                    assert_eq!(op, EditOp::WriteLiteral);
                    assert_eq!(a1, 1);
                    assert_eq!(a2, 2);
                }
                2 => {
                    assert_eq!(op, EditOp::EraseRef);
                    assert_eq!(a1, 5);
                }
                _ => panic!("unexpected operation"),
            }
            count += 1;
        });

        assert_eq!(count, 3);
    }

    #[test]
    fn test_end_index_constant() {
        assert_eq!(END_INDEX, i64::MIN);
    }

    #[test]
    fn test_normalize_negative_index() {
        assert_eq!(normalize_and_check_ref_idx(-1, 10), Some(9));
        assert_eq!(normalize_and_check_ref_idx(-2, 10), Some(8));
        assert_eq!(normalize_and_check_ref_idx(-10, 10), Some(0));
        assert_eq!(normalize_and_check_ref_idx(-11, 10), None);
    }

    #[test]
    fn test_normalize_insert_index() {
        assert_eq!(normalize_and_check_insert_idx(END_INDEX, 10), Some(10));
        assert_eq!(normalize_and_check_insert_idx(0, 10), Some(0));
        assert_eq!(normalize_and_check_insert_idx(10, 10), Some(10));
        assert_eq!(normalize_and_check_insert_idx(11, 10), None);
        assert_eq!(normalize_and_check_insert_idx(-1, 10), Some(9));
    }
}
