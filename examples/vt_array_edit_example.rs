//! Example demonstrating VtArrayEdit functionality.
//!
//! This example shows how to use ArrayEdit and ArrayEditBuilder to construct
//! and compose array modifications, which is critical for SDF list operations.

use usd::vt::{Array, ArrayEditBuilder};

fn main() {
    println!("=== VtArrayEdit Example ===\n");

    // Example 1: Basic edit operations
    println!("Example 1: Basic Operations");
    basic_operations();

    // Example 2: Composing edits (monoid operations)
    println!("\nExample 2: Composing Edits");
    compose_edits();

    // Example 3: Python-style negative indexes
    println!("\nExample 3: Negative Indexes");
    negative_indexes();

    // Example 4: Size operations
    println!("\nExample 4: Size Operations");
    size_operations();

    // Example 5: Reference operations (copy from existing elements)
    println!("\nExample 5: Reference Operations");
    reference_operations();

    // Example 6: Optimization and serialization
    println!("\nExample 6: Optimization & Serialization");
    optimization_example();
}

fn basic_operations() {
    let mut builder = ArrayEditBuilder::new();

    // Build an edit that modifies an array
    builder
        .write(42, 0) // Write 42 to index 0
        .append(100) // Append 100 to end
        .insert(50, 2) // Insert 50 at index 2
        .prepend(10) // Prepend 10 to start
        .erase(3); // Erase element at index 3

    let edit = builder.build();

    let mut array = Array::from(vec![1, 2, 3, 4, 5]);
    println!("  Original: {:?}", array.as_slice());

    edit.apply(&mut array);
    println!("  Modified: {:?}", array.as_slice());
    println!("  Debug: {}", edit);
}

fn compose_edits() {
    // Create multiple edits
    let mut b1 = ArrayEditBuilder::new();
    b1.append(10);
    let edit1 = b1.build();

    let mut b2 = ArrayEditBuilder::new();
    b2.write(99, 0);
    let edit2 = b2.build();

    let mut b3 = ArrayEditBuilder::new();
    b3.prepend(5);
    let edit3 = b3.build();

    // Compose edits: stronger.compose_over(weaker)
    // The weaker edit is applied first, then stronger
    let composed = edit3.compose_over(&edit2.compose_over(&edit1));

    let mut array = Array::from(vec![1, 2, 3]);
    println!("  Original: {:?}", array.as_slice());

    composed.apply(&mut array);
    println!("  After composed edit: {:?}", array.as_slice());
    println!("  (append 10, write 99 to [0], prepend 5)");
}

fn negative_indexes() {
    let mut builder = ArrayEditBuilder::new();

    // Negative indexes work like Python: -1 is last element
    builder
        .write(99, -1) // Last element
        .write(88, -2) // Second to last
        .insert(77, -1); // Insert before last element

    let edit = builder.build();

    let mut array = Array::from(vec![1, 2, 3, 4, 5]);
    println!("  Original: {:?}", array.as_slice());

    edit.apply(&mut array);
    println!("  Modified: {:?}", array.as_slice());
}

fn size_operations() {
    let mut builder = ArrayEditBuilder::new();

    builder
        .min_size(8) // Ensure at least 8 elements
        .max_size(10) // Ensure at most 10 elements
        .set_size_fill(6, 42); // Set to exactly 6, fill with 42

    let edit = builder.build();

    let mut array = Array::from(vec![1, 2, 3]);
    println!("  Original (len={}): {:?}", array.len(), array.as_slice());

    edit.apply(&mut array);
    println!("  Modified (len={}): {:?}", array.len(), array.as_slice());
}

fn reference_operations() {
    let mut builder = ArrayEditBuilder::new();

    // Copy elements from existing positions
    builder
        .write_ref(0, 2) // Copy [0] to [2]
        .insert_ref(1, 3) // Copy [1] and insert at [3]
        .append_ref(0) // Append copy of [0]
        .prepend_ref(2); // Prepend copy of [2]

    let edit = builder.build();

    let mut array = Array::from(vec![10, 20, 30]);
    println!("  Original: {:?}", array.as_slice());

    edit.apply(&mut array);
    println!("  Modified: {:?}", array.as_slice());
    println!("  (operations reference array elements)");
}

fn optimization_example() {
    // Create an edit with multiple operations
    let mut builder = ArrayEditBuilder::new();
    builder.write(42, 0);
    builder.write(42, 1); // Same value reused
    builder.write(42, 2);
    let edit = builder.build();

    println!("  Literals before optimization: {}", edit.literals().len());

    // Optimize the edit (deduplicate literals)
    let optimized = ArrayEditBuilder::optimize(edit.clone());
    println!(
        "  Literals after optimization: {}",
        optimized.literals().len()
    );

    // Serialize and deserialize
    let (lits, ins) = ArrayEditBuilder::get_serialization_data(&edit);
    println!(
        "  Serialized: {} literals, {} instructions",
        lits.len(),
        ins.len()
    );

    let reconstructed = ArrayEditBuilder::from_serialization_data(lits, ins);

    // Verify they produce the same result
    let mut arr1 = Array::from(vec![1, 2, 3]);
    let mut arr2 = Array::from(vec![1, 2, 3]);

    edit.apply(&mut arr1);
    reconstructed.apply(&mut arr2);

    println!("  Original result: {:?}", arr1.as_slice());
    println!("  Reconstructed result: {:?}", arr2.as_slice());
    println!("  Results match: {}", arr1 == arr2);
}
