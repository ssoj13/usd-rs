//! Port of C++ testenv/testVtArrayEdit.cpp
//!
//! Tests for ArrayEdit basics, builder, composition, and serialization.

#[cfg(test)]
mod tests {
    use crate::Value;
    use crate::value_compose_over::{value_compose_over, value_try_compose_over};
    use crate::{Array, ArrayEdit, ArrayEditBuilder};

    // ========================================================================
    // testBasics() from C++
    // ========================================================================

    #[test]
    fn test_basics_identity() {
        let empty: Array<i32> = Array::new();
        let ident: ArrayEdit<i32> = ArrayEdit::new();

        assert!(ident.is_identity());

        // Ident over dense array leaves it unchanged.
        assert_eq!(ident.apply_to(empty.clone()), Array::<i32>::new());

        let one23 = Array::from(vec![1, 2, 3]);
        assert_eq!(ident.apply_to(one23.clone()), one23);
    }

    #[test]
    fn test_basics_equality() {
        let ident1: ArrayEdit<i32> = ArrayEdit::new();
        let ident2: ArrayEdit<i32> = ArrayEdit::new();
        assert_eq!(ident1, ident2);
    }

    // ========================================================================
    // testBuilderAndComposition() from C++
    // ========================================================================

    #[test]
    fn test_prepend_append() {
        let empty: Array<i32> = Array::new();

        // Create an editor that prepends 0 and appends 9.
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let zero_nine = builder.build();

        // Composing over dense arrays.
        assert_eq!(zero_nine.apply_to(empty.clone()), Array::from(vec![0, 9]));
        assert_eq!(
            zero_nine.apply_to(Array::from(vec![5])),
            Array::from(vec![0, 5, 9])
        );
    }

    #[test]
    fn test_compose_edit_over_edit() {
        let empty: Array<i32> = Array::new();

        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let zero_nine = builder.build();

        // Compose zeroNine itself to make a 00..99 appender.
        let zero09_nine = zero_nine.compose_over(&zero_nine);

        assert_eq!(
            zero09_nine.apply_to(empty.clone()),
            Array::from(vec![0, 0, 9, 9])
        );
        assert_eq!(
            zero09_nine.apply_to(Array::from(vec![3, 4, 5])),
            Array::from(vec![0, 0, 3, 4, 5, 9, 9])
        );
    }

    #[test]
    fn test_write_ref_and_erase() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let _zero_nine = builder.build();

        // Build an edit that writes the last element to index 2, the first
        // element to index 4, then erases the first and last element.
        let mut builder = ArrayEditBuilder::new();
        builder.write_ref(-1, 2).write_ref(0, 4).erase(-1).erase(0);
        let mix_and_trim = builder.build();

        assert_eq!(
            mix_and_trim.apply_to(Array::from(vec![0, 0, 3, 4, 5, 9, 9])),
            Array::from(vec![0, 9, 4, 0, 9])
        );

        // Out-of-bounds operations should be ignored.
        assert_eq!(
            mix_and_trim.apply_to(Array::from(vec![4, 5, 6, 7])),
            Array::from(vec![5, 7])
        );
    }

    #[test]
    fn test_compose_complex() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let zero_nine = builder.build();

        let mut builder = ArrayEditBuilder::new();
        builder.write_ref(-1, 2).write_ref(0, 4).erase(-1).erase(0);
        let mix_and_trim = builder.build();

        let zero_nine_mix_and_trim = mix_and_trim.compose_over(&zero_nine);
        assert_eq!(
            zero_nine_mix_and_trim.apply_to(Array::from(vec![1, 2, 3, 4, 5, 6, 7])),
            Array::from(vec![1, 9, 3, 0, 5, 6, 7])
        );
        assert_eq!(
            zero_nine_mix_and_trim.apply_to(Array::from(vec![4, 5])),
            Array::from(vec![4, 9])
        );
    }

    #[test]
    fn test_min_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.min_size(10);
        let min_size10 = builder.build();

        assert_eq!(
            min_size10.apply_to(Array::<i32>::new()),
            Array::from(vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        );
        assert_eq!(
            min_size10.apply_to(Array::from(vec![7; 15])),
            Array::from(vec![7; 15])
        );
    }

    #[test]
    fn test_min_size_fill() {
        let mut builder = ArrayEditBuilder::new();
        builder.min_size_fill(10, 9);
        let min_size10_fill9 = builder.build();

        assert_eq!(
            min_size10_fill9.apply_to(Array::<i32>::new()),
            Array::from(vec![9, 9, 9, 9, 9, 9, 9, 9, 9, 9])
        );
    }

    #[test]
    fn test_max_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.max_size(15);
        let max_size15 = builder.build();

        assert_eq!(
            max_size15.apply_to(Array::<i32>::new()),
            Array::<i32>::new()
        );
        assert_eq!(
            max_size15.apply_to(Array::from(vec![2; 20])),
            Array::from(vec![2; 15])
        );
    }

    #[test]
    fn test_size_range_composition() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.min_size(10);
        let min_size10 = builder.build();

        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.max_size(15);
        let max_size15 = builder.build();

        let size10to15 = max_size15.compose_over(&min_size10);

        assert_eq!(
            size10to15.apply_to(Array::from(vec![1; 7])),
            Array::from(vec![1, 1, 1, 1, 1, 1, 1, 0, 0, 0])
        );
        assert_eq!(
            size10to15.apply_to(Array::from(vec![2; 20])),
            Array::from(vec![2; 15])
        );
        assert_eq!(
            size10to15.apply_to(Array::from(vec![3; 13])),
            Array::from(vec![3; 13])
        );
    }

    #[test]
    fn test_set_size() {
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.set_size(7);
        let size7 = builder.build();

        assert_eq!(
            size7.apply_to(Array::from(vec![1; 7])),
            Array::from(vec![1; 7])
        );
        assert_eq!(size7.apply_to(Array::<i32>::new()), Array::from(vec![0; 7]));
        assert_eq!(
            size7.apply_to(Array::from(vec![9; 27])),
            Array::from(vec![9; 7])
        );
    }

    #[test]
    fn test_set_size_fill() {
        let mut builder = ArrayEditBuilder::new();
        builder.set_size_fill(7, 3);
        let size7_fill3 = builder.build();

        assert_eq!(
            size7_fill3.apply_to(Array::from(vec![1; 7])),
            Array::from(vec![1; 7])
        );
        assert_eq!(
            size7_fill3.apply_to(Array::<i32>::new()),
            Array::from(vec![3; 7])
        );
        assert_eq!(
            size7_fill3.apply_to(Array::from(vec![9; 27])),
            Array::from(vec![9; 7])
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        // Check that the serialization data will reproduce an equivalent edit.
        let check = |edit: &ArrayEdit<i32>| {
            let (vals, indexes) = ArrayEditBuilder::get_serialization_data(edit);
            let reconstituted = ArrayEditBuilder::from_serialization_data(vals, indexes);
            assert_eq!(edit, &reconstituted);
        };

        // size7Fill3
        let mut builder = ArrayEditBuilder::new();
        builder.set_size_fill(7, 3);
        check(&builder.build());

        // size7
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.set_size(7);
        check(&builder.build());

        // size10to15
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.min_size(10);
        let min_size10 = builder.build();
        let mut builder = ArrayEditBuilder::<i32>::new();
        builder.max_size(15);
        let max_size15 = builder.build();
        let size10to15 = max_size15.compose_over(&min_size10);
        check(&size10to15);

        // minSize10Fill9
        let mut builder = ArrayEditBuilder::new();
        builder.min_size_fill(10, 9);
        check(&builder.build());

        // zeroNineMixAndTrim
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let zero_nine = builder.build();
        let mut builder = ArrayEditBuilder::new();
        builder.write_ref(-1, 2).write_ref(0, 4).erase(-1).erase(0);
        let mix_and_trim = builder.build();
        let zero_nine_mix_and_trim = mix_and_trim.compose_over(&zero_nine);
        check(&zero_nine_mix_and_trim);

        // identity
        let identity: ArrayEdit<i32> = ArrayEdit::new();
        check(&identity);
    }

    // ========================================================================
    // Bug regression test from REPORT.md:
    // ArrayEdit does not compose over Array through value_compose_over()
    // ========================================================================

    #[test]
    fn test_array_edit_over_array_via_value_compose_over() {
        // Create ArrayEdit: prepend(0), append(9)
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let edit = builder.build();

        // Store edit and array as Values
        let stronger = Value::from_no_hash(edit);
        let weaker = Value::new(Array::from(vec![3, 2, 1]));

        // Compose: should apply the edit to the array
        let result = value_compose_over(&stronger, &weaker);

        // Expected: [0, 3, 2, 1, 9]
        let arr = result.get::<Array<i32>>().expect("Expected Array<i32>");
        assert_eq!(arr.as_slice(), &[0, 3, 2, 1, 9]);
    }

    #[test]
    fn test_array_edit_over_empty_via_value_compose_over() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let edit = builder.build();

        let stronger = Value::from_no_hash(edit);
        let weaker = Value::empty();

        let result = value_compose_over(&stronger, &weaker);

        let arr = result.get::<Array<i32>>().expect("Expected Array<i32>");
        assert_eq!(arr.as_slice(), &[0, 9]);
    }

    #[test]
    fn test_array_edit_over_array_edit_via_value_compose_over() {
        let mut b1 = ArrayEditBuilder::new();
        b1.prepend(0).append(9);
        let edit1 = b1.build();

        let mut b2 = ArrayEditBuilder::new();
        b2.prepend(0).append(9);
        let edit2 = b2.build();

        let stronger = Value::from_no_hash(edit2);
        let weaker = Value::from_no_hash(edit1);

        let result = value_compose_over(&stronger, &weaker);

        // The result should be a composed ArrayEdit<i32>
        assert!(
            result.get::<ArrayEdit<i32>>().is_some(),
            "Expected ArrayEdit<i32>"
        );

        // Apply to empty array to verify: should produce [0, 0, 9, 9]
        let composed = result.get::<ArrayEdit<i32>>().unwrap();
        assert_eq!(
            composed.apply_to(Array::<i32>::new()),
            Array::from(vec![0, 0, 9, 9])
        );
    }

    #[test]
    fn test_try_compose_array_edit_over_array() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(0).append(9);
        let edit = builder.build();

        let stronger = Value::from_no_hash(edit);
        let weaker = Value::new(Array::from(vec![5]));

        let result = value_try_compose_over(&stronger, &weaker);
        assert!(result.is_some(), "ArrayEdit over Array should compose");

        let arr = result
            .unwrap()
            .get::<Array<i32>>()
            .expect("Expected Array<i32>")
            .clone();
        assert_eq!(arr.as_slice(), &[0, 5, 9]);
    }

    #[test]
    fn test_array_edit_over_array_u32() {
        let mut builder = ArrayEditBuilder::new();
        builder.prepend(1_u32).append(9_u32);
        let edit = builder.build();

        let stronger = Value::from_no_hash(edit);
        let weaker = Value::new(Array::from(vec![5_u32]));

        let result = value_compose_over(&stronger, &weaker);
        let arr = result.get::<Array<u32>>().expect("Expected Array<u32>");
        assert_eq!(arr.as_slice(), &[1, 5, 9]);
    }

    #[test]
    fn test_array_edit_over_array_string() {
        let mut builder = ArrayEditBuilder::new();
        builder
            .prepend("first".to_string())
            .append("last".to_string());
        let edit = builder.build();

        let stronger = Value::from_no_hash(edit);
        let weaker = Value::new(Array::from(vec!["middle".to_string()]));

        let result = value_compose_over(&stronger, &weaker);
        let arr = result
            .get::<Array<String>>()
            .expect("Expected Array<String>");
        assert_eq!(arr.as_slice(), &["first", "middle", "last"]);
    }
}
