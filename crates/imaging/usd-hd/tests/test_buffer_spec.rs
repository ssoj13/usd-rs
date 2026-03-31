// Port of pxr/imaging/hd/testenv/testHdBufferSpec.cpp

use usd_hd::resource::HdBufferSpec;
use usd_hd::types::{HdTupleType, HdType};
use usd_tf::Token;

fn spec(name: &str, type_: HdType) -> HdBufferSpec {
    HdBufferSpec::new(Token::new(name), HdTupleType::new(type_, 1))
}

#[test]
fn test_comparison_operators() {
    // Same name, same type => equal
    assert_eq!(
        spec("points", HdType::FloatVec3),
        spec("points", HdType::FloatVec3)
    );

    // Same name, different type => not equal
    assert_ne!(
        spec("points", HdType::FloatVec3),
        spec("points", HdType::FloatVec4)
    );

    // Different name, same type => not equal
    assert_ne!(
        spec("points", HdType::FloatVec3),
        spec("normals", HdType::FloatVec3)
    );

    // Same name, different base type => not equal
    assert_ne!(
        spec("points", HdType::FloatVec3),
        spec("points", HdType::DoubleVec3)
    );

    // Self is not less than self
    assert!(!(spec("points", HdType::FloatVec3) < spec("points", HdType::FloatVec3)));

    // "normals" < "points" lexicographically
    assert!(spec("normals", HdType::FloatVec3) < spec("points", HdType::FloatVec3));

    // Same name, FloatVec3 < DoubleVec3
    assert!(spec("points", HdType::FloatVec3) < spec("points", HdType::DoubleVec3));

    // Same name, FloatVec3 < FloatVec4
    assert!(spec("points", HdType::FloatVec3) < spec("points", HdType::FloatVec4));
}

#[test]
fn test_set_operations() {
    let spec1 = vec![
        spec("points", HdType::FloatVec3),
        spec("displayColor", HdType::FloatVec3),
    ];
    let mut spec2 = vec![spec("points", HdType::FloatVec3)];

    assert!(HdBufferSpec::is_subset(&spec2, &spec1));
    assert!(!HdBufferSpec::is_subset(&spec1, &spec2));

    spec2.push(spec("normals", HdType::FloatVec4));

    assert!(!HdBufferSpec::is_subset(&spec2, &spec1));
    assert!(!HdBufferSpec::is_subset(&spec1, &spec2));

    let spec3 = HdBufferSpec::compute_union(&spec1, &spec2);

    assert!(HdBufferSpec::is_subset(&spec1, &spec3));
    assert!(HdBufferSpec::is_subset(&spec2, &spec3));
}
