// Port of pxr/imaging/hd/testenv/testHdTypes.cpp

use usd_hd::types::HdVec4_2_10_10_10_Rev;

#[test]
fn test_vec4f_2_10_10_10_rev_round_trip() {
    // Test round tripping between Vec3f and HdVec4f_2_10_10_10_REV
    let a = (
        -0.1617791586913686_f32,
        -0.2533272416818153_f32,
        0.9537572083266245_f32,
    );
    let b = (
        0.12954827567352645_f32,
        -0.8348099306719063_f32,
        0.5350790819323653_f32,
    );

    let packed_a = HdVec4_2_10_10_10_Rev::from_vec3(a.0, a.1, a.2);
    let a_rt = packed_a.to_vec3();

    let packed_b = HdVec4_2_10_10_10_Rev::from_vec3(b.0, b.1, b.2);
    let b_rt = packed_b.to_vec3();

    let eps = 0.01_f32;

    let a_ok =
        (a.0 - a_rt.0).abs() < eps && (a.1 - a_rt.1).abs() < eps && (a.2 - a_rt.2).abs() < eps;

    let b_ok =
        (b.0 - b_rt.0).abs() < eps && (b.1 - b_rt.1).abs() < eps && (b.2 - b_rt.2).abs() < eps;

    assert!(a_ok, "Vec3 round-trip A failed: {:?} -> {:?}", a, a_rt);
    assert!(b_ok, "Vec3 round-trip B failed: {:?} -> {:?}", b, b_rt);
}
