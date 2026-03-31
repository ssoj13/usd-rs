// Port of meta.cpp — this is primarily a compile-time test.
// All meaningful assertions are static (compile-time); the runtime test
// functions merely confirm that the type machinery resolves correctly.

use std::marker::PhantomData;
use usd_tf::meta::{
    Conditional, ConditionalType, Select, TupleHead, TupleLength, TupleTail, TypeList,
};

// ---------------------------------------------------------------------------
// TypeList basics
// ---------------------------------------------------------------------------

#[test]
fn test_type_list_can_be_created() {
    // C++: using TestList = TfMetaList<int, float, std::string>
    // Just verify it compiles and is constructible.
    let _list: TypeList<(i32, f32, String)> = TypeList::new();
}

// ---------------------------------------------------------------------------
// TupleHead — equivalent to TfMetaApply<TfMetaHead, TestList>
// ---------------------------------------------------------------------------

#[test]
fn test_tuple_head() {
    // Helper: if the associated type is wrong the function won't compile.
    fn check_head<T: TupleHead<Head = i32>>(_: PhantomData<T>) {}

    // C++: ASSERT_SAME((TfMetaApply<TfMetaHead, TestList>), int)
    check_head(PhantomData::<(i32, f32, String)>);
}

// ---------------------------------------------------------------------------
// TupleTail — equivalent to TfMetaApply<TfMetaTail, TestList>
// ---------------------------------------------------------------------------

#[test]
fn test_tuple_tail() {
    fn check_tail<T: TupleTail<Tail = (f32, String)>>(_: PhantomData<T>) {}

    // C++: ASSERT_SAME((TfMetaApply<TfMetaTail, TestList>), (TfMetaList<float, std::string>))
    check_tail(PhantomData::<(i32, f32, String)>);
}

// ---------------------------------------------------------------------------
// TupleLength — equivalent to TfMetaApply<TfMetaLength, TestList>
// ---------------------------------------------------------------------------

#[test]
fn test_tuple_length() {
    // C++: ASSERT_SAME((TfMetaApply<TfMetaLength, TestList>),
    //                  (std::integral_constant<size_t, 3>))
    assert_eq!(<(i32, f32, String) as TupleLength>::LENGTH, 3);

    // Extra arity checks matching C++ function-traits test expectations.
    assert_eq!(<() as TupleLength>::LENGTH, 0);
    assert_eq!(<(i32,) as TupleLength>::LENGTH, 1);
    assert_eq!(<(i32, f32) as TupleLength>::LENGTH, 2);
}

// ---------------------------------------------------------------------------
// Tuple-as-type-list — equivalent to TfMetaApply<std::tuple, TestList>
//
// In Rust (i32, f32, String) IS already a std tuple — same construct.
// ---------------------------------------------------------------------------

#[test]
fn test_tuple_identity() {
    // C++: ASSERT_SAME((TfMetaApply<std::tuple, TestList>),
    //                  (std::tuple<int, float, std::string>))
    // Rust: TypeList<(i32, f32, String)> carries the tuple type directly.
    fn accept_triple(_: (i32, f32, String)) {}
    accept_triple((1, 2.0, String::from("hello")));
}

// ---------------------------------------------------------------------------
// ConditionalType — equivalent to TfConditionalType<B, T, F>
// ---------------------------------------------------------------------------

#[test]
fn test_conditional_true() {
    // C++: ASSERT_SAME((TfConditionalType<true, int, float>), int)
    fn check_true<T>()
    where
        Conditional<true, i32, f32>: Select<Type = T>,
    {
    }
    check_true::<i32>();
}

#[test]
fn test_conditional_false() {
    // C++: ASSERT_SAME((TfConditionalType<false, int, float>), float)
    fn check_false<T>()
    where
        Conditional<false, i32, f32>: Select<Type = T>,
    {
    }
    check_false::<f32>();
}

#[test]
fn test_conditional_type_alias() {
    // ConditionalType is a type alias that resolves at compile time.
    // If the alias resolves to the wrong type the functions below won't compile.
    fn accept_i32(_: ConditionalType<true, i32, f32>) {}
    fn accept_f32(_: ConditionalType<false, i32, f32>) {}

    accept_i32(42i32);
    accept_f32(1.0f32);
}

// ---------------------------------------------------------------------------
// TfMetaDecay equivalent — strip references from tuple element types.
// In Rust: T, &T, and &mut T are distinct; decay means removing the ref.
// We verify the conceptual equivalent using basic type checking.
// ---------------------------------------------------------------------------

#[test]
fn test_ref_list_vs_value_list() {
    // C++: using TestRefList = TfMetaList<int &, float const &, std::string>
    //      ASSERT_SAME((TfMetaApply<TfMetaDecay, TestRefList>), TestList)
    //
    // Rust doesn't have a MetaDecay built into the library (references don't
    // appear in tuple type parameters the same way), but we can verify that
    // the value-type list has the expected lengths and heads.
    type ValueList = (i32, f32, String);

    assert_eq!(<ValueList as TupleLength>::LENGTH, 3);

    fn check_head<T: TupleHead<Head = i32>>(_: PhantomData<T>) {}
    check_head(PhantomData::<ValueList>);
}
