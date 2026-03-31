//! Integration tests for array editing functionality.
//!
//! These tests verify complex scenarios combining multiple operations
//! and composition of edits.

#[cfg(test)]
mod tests {
    use crate::{Array, ArrayEdit, ArrayEditBuilder};

    #[test]
    fn test_complex_edit_sequence() {
        let mut builder = ArrayEditBuilder::new();
        builder
            .write(10, 0)
            .append(20)
            .prepend(5)
            .insert(15, 2)
            .erase(3)
            .write_ref(0, 1);

        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3, 4]);
        edit.apply(&mut array);

        // Expected operations:
        // Start: [1, 2, 3, 4]
        // write 10 to 0: [10, 2, 3, 4]
        // append 20: [10, 2, 3, 4, 20]
        // prepend 5: [5, 10, 2, 3, 4, 20]
        // insert 15 at 2: [5, 10, 15, 2, 3, 4, 20]
        // erase 3: [5, 10, 15, 3, 4, 20]
        // write_ref 0 to 1: [5, 5, 15, 3, 4, 20]

        assert_eq!(array.as_slice(), &[5, 5, 15, 3, 4, 20]);
    }

    #[test]
    fn test_monoid_associativity() {
        // Test that compose_over is associative:
        // strong.compose_over(weak) means apply weak first, then strong
        // So for associativity: c.compose_over(b.compose_over(a)) == (c.compose_over(b)).compose_over(a)

        let mut b1 = ArrayEditBuilder::new();
        b1.append(10);
        let edit_a = b1.build();

        let mut b2 = ArrayEditBuilder::new();
        b2.append(20);
        let edit_b = b2.build();

        let mut b3 = ArrayEditBuilder::new();
        b3.append(30);
        let edit_c = b3.build();

        // c.compose_over(b.compose_over(a))
        let ba = edit_b.compose_over(&edit_a);
        let c_ba = edit_c.compose_over(&ba);

        // (c.compose_over(b)).compose_over(a)
        let cb = edit_c.compose_over(&edit_b);
        let cb_a = cb.compose_over(&edit_a);

        let mut array1 = Array::from(vec![1, 2, 3]);
        let mut array2 = Array::from(vec![1, 2, 3]);

        c_ba.apply(&mut array1);
        cb_a.apply(&mut array2);

        assert_eq!(array1, array2, "Composition should be associative");
    }

    #[test]
    fn test_identity_is_neutral() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0).append(99);
        let edit = builder.build();

        let identity: ArrayEdit<i32> = ArrayEdit::new();

        // edit . identity == edit
        let composed1 = edit.compose_over(&identity);

        // identity . edit == edit
        let composed2 = identity.compose_over(&edit);

        let mut array1 = Array::from(vec![1, 2, 3]);
        let mut array2 = Array::from(vec![1, 2, 3]);
        let mut array3 = Array::from(vec![1, 2, 3]);

        edit.apply(&mut array1);
        composed1.apply(&mut array2);
        composed2.apply(&mut array3);

        assert_eq!(array1, array2);
        assert_eq!(array1, array3);
    }

    #[test]
    fn test_size_operations_composition() {
        let mut b1 = ArrayEditBuilder::<i32>::new();
        b1.min_size(10);
        let edit1 = b1.build();

        let mut b2 = ArrayEditBuilder::<i32>::new();
        b2.max_size(5);
        let edit2 = b2.build();

        let composed = edit2.compose_over(&edit1);

        let mut array = Array::from(vec![1, 2, 3]);
        composed.apply(&mut array);

        // Should first extend to 10, then truncate to 5
        assert_eq!(array.len(), 5);
    }

    #[test]
    fn test_negative_indices_composition() {
        let mut b1 = ArrayEditBuilder::new();
        b1.write(99, -1); // Last element
        let edit1 = b1.build();

        let mut b2 = ArrayEditBuilder::new();
        b2.write(88, -2); // Second-to-last
        let edit2 = b2.build();

        let composed = edit2.compose_over(&edit1);

        let mut array = Array::from(vec![1, 2, 3, 4, 5]);
        composed.apply(&mut array);

        assert_eq!(array[3], 88);
        assert_eq!(array[4], 99);
    }

    #[test]
    fn test_insert_and_erase_composition() {
        let mut b1 = ArrayEditBuilder::new();
        b1.insert(10, 1);
        b1.insert(20, 2);
        let edit1 = b1.build();

        let mut b2 = ArrayEditBuilder::new();
        b2.erase(0);
        let edit2 = b2.build();

        let composed = edit2.compose_over(&edit1);

        let mut array = Array::from(vec![1, 2, 3]);
        composed.apply(&mut array);

        // Start: [1, 2, 3]
        // insert 10 at 1: [1, 10, 2, 3]
        // insert 20 at 2: [1, 10, 20, 2, 3]
        // erase 0: [10, 20, 2, 3]

        assert_eq!(array.as_slice(), &[10, 20, 2, 3]);
    }

    #[test]
    fn test_ref_operations() {
        let mut builder = ArrayEditBuilder::new();
        builder.write_ref(0, 2);
        builder.insert_ref(1, 3);
        builder.append_ref(0);

        let edit = builder.build();

        let mut array = Array::from(vec![10, 20, 30]);
        edit.apply(&mut array);

        // Start: [10, 20, 30]
        // write_ref 0->2: [10, 20, 10]
        // insert_ref 1 at 3: [10, 20, 10, 20]
        // append_ref 0: [10, 20, 10, 20, 10]

        assert_eq!(array.as_slice(), &[10, 20, 10, 20, 10]);
    }

    #[test]
    fn test_fill_operations() {
        let mut builder = ArrayEditBuilder::new();
        builder.min_size_fill(5, 99);
        builder.set_size_fill(8, 88);

        let edit = builder.build();

        let mut array = Array::from(vec![1, 2]);
        edit.apply(&mut array);

        // Start: [1, 2]
        // min_size_fill 5 with 99: [1, 2, 99, 99, 99]
        // set_size_fill 8 with 88: [1, 2, 99, 99, 99, 88, 88, 88]

        assert_eq!(array.len(), 8);
        assert_eq!(array[5], 88);
        assert_eq!(array[6], 88);
        assert_eq!(array[7], 88);
    }

    #[test]
    fn test_empty_array_operations() {
        let mut builder = ArrayEditBuilder::new();
        builder.append(1);
        builder.append(2);
        builder.append(3);

        let edit = builder.build();

        let mut array: Array<i32> = Array::new();
        edit.apply(&mut array);

        assert_eq!(array.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_chain_multiple_compositions() {
        let mut b1 = ArrayEditBuilder::new();
        b1.append(1);
        let e1 = b1.build();

        let mut b2 = ArrayEditBuilder::new();
        b2.append(2);
        let e2 = b2.build();

        let mut b3 = ArrayEditBuilder::new();
        b3.append(3);
        let e3 = b3.build();

        let mut b4 = ArrayEditBuilder::new();
        b4.append(4);
        let e4 = b4.build();

        // Chain compositions
        let composed = e4.compose_over(&e3.compose_over(&e2.compose_over(&e1)));

        let mut array: Array<i32> = Array::new();
        composed.apply(&mut array);

        assert_eq!(array.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_optimization_preserves_behavior() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(10, 0);
        builder.append(20);
        builder.insert(15, 1);
        let original = builder.build();

        let optimized = ArrayEditBuilder::optimize(original.clone());

        let mut array1 = Array::from(vec![1, 2, 3]);
        let mut array2 = Array::from(vec![1, 2, 3]);

        original.apply(&mut array1);
        optimized.apply(&mut array2);

        assert_eq!(array1, array2, "Optimization should preserve behavior");
    }

    #[test]
    fn test_serialization_with_complex_edit() {
        let mut builder = ArrayEditBuilder::new();
        builder
            .write(42, 0)
            .append(99)
            .insert(50, 1)
            .erase(2)
            .min_size_fill(10, 77);

        let original = builder.build();

        let (lits, ins) = ArrayEditBuilder::get_serialization_data(&original);
        let reconstructed = ArrayEditBuilder::from_serialization_data(lits, ins);

        let mut array1 = Array::from(vec![1, 2, 3, 4, 5]);
        let mut array2 = Array::from(vec![1, 2, 3, 4, 5]);

        original.apply(&mut array1);
        reconstructed.apply(&mut array2);

        assert_eq!(array1, array2);
    }

    #[test]
    fn test_boundary_conditions() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(99, 0); // First element
        builder.write(88, -1); // Last element

        let edit = builder.build();

        let mut array = Array::from(vec![1]);
        edit.apply(&mut array);

        // Should write to same element twice
        assert_eq!(array.len(), 1);
        assert_eq!(array[0], 88); // Last write wins
    }

    #[test]
    fn test_out_of_bounds_ignored() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(99, 100); // Out of bounds
        builder.write(42, 1); // Valid

        let edit = builder.build();

        let mut array = Array::from(vec![1, 2, 3]);
        edit.apply(&mut array);

        // Out of bounds write should be ignored
        assert_eq!(array.as_slice(), &[1, 42, 3]);
    }

    #[test]
    fn test_literal_dedup_across_operations() {
        let mut builder = ArrayEditBuilder::new();
        builder.write(42, 0);
        builder.insert(42, 1);
        builder.append(42);
        builder.min_size_fill(10, 42);

        let edit = builder.build();

        // All operations use the same value, should only have one literal
        assert_eq!(edit.literals().len(), 1);
        assert_eq!(edit.literals()[0], 42);
    }
}
