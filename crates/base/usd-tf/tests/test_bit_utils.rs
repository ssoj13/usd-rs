// Port of pxr/base/tf/testenv/bitUtils.cpp
//
// C++ TF_BITS_FOR_VALUES(n) = Tf_NumBits<n-1>::value = bits needed to store max value (n-1).
// For n >= 2 this equals bits_for_values(n) in Rust.
// For n = 1: C++ returns 1 (template base case), Rust returns 0 (mathematically correct).
// The test below documents and verifies the Rust behavior directly.
//
// C++ TF_BITS_FOR_ENUM_VALUES(n) = TF_BITS_FOR_VALUES(n) + 1.
// Rust bits_for_enum_values(n) = bits_for_values(n) + 1.

use usd_tf::bits::{bits_for_enum_values, bits_for_values};

#[test]
fn test_bits_for_values_enum_range() {
    // Matches C++ test with TF_BITS_FOR_VALUES(TestN) where TestN = N.
    // For N >= 2 the results are identical to C++.
    assert_eq!(bits_for_values(1), 0); // C++ TF_BITS_FOR_VALUES(1)==1; Rust diverges for n=1
    assert_eq!(bits_for_values(2), 1);
    assert_eq!(bits_for_values(3), 2);
    assert_eq!(bits_for_values(4), 2);
    assert_eq!(bits_for_values(5), 3);
    assert_eq!(bits_for_values(6), 3);
    assert_eq!(bits_for_values(7), 3);
    assert_eq!(bits_for_values(8), 3);
    assert_eq!(bits_for_values(9), 4);
    assert_eq!(bits_for_values(10), 4);
    assert_eq!(bits_for_values(11), 4);
    assert_eq!(bits_for_values(12), 4);
    assert_eq!(bits_for_values(13), 4);
    assert_eq!(bits_for_values(14), 4);
    assert_eq!(bits_for_values(15), 4);
    assert_eq!(bits_for_values(16), 4);
    assert_eq!(bits_for_values(17), 5);
    assert_eq!(bits_for_values(18), 5);
    assert_eq!(bits_for_values(65535), 16);
    assert_eq!(bits_for_values(65536), 16);
    assert_eq!(bits_for_values(65537), 17);
}

#[test]
fn test_bits_for_enum_values_enum_range() {
    assert_eq!(bits_for_enum_values(1), 1); // C++ would give 2; Rust bits_for_values(1)+1=1
    assert_eq!(bits_for_enum_values(2), 2);
    assert_eq!(bits_for_enum_values(3), 3);
    assert_eq!(bits_for_enum_values(4), 3);
    assert_eq!(bits_for_enum_values(5), 4);
    assert_eq!(bits_for_enum_values(6), 4);
    assert_eq!(bits_for_enum_values(7), 4);
    assert_eq!(bits_for_enum_values(8), 4);
    assert_eq!(bits_for_enum_values(9), 5);
    assert_eq!(bits_for_enum_values(10), 5);
    assert_eq!(bits_for_enum_values(11), 5);
    assert_eq!(bits_for_enum_values(12), 5);
    assert_eq!(bits_for_enum_values(13), 5);
    assert_eq!(bits_for_enum_values(14), 5);
    assert_eq!(bits_for_enum_values(15), 5);
    assert_eq!(bits_for_enum_values(16), 5);
    assert_eq!(bits_for_enum_values(17), 6);
    assert_eq!(bits_for_enum_values(18), 6);
    assert_eq!(bits_for_enum_values(65535), 17);
    assert_eq!(bits_for_enum_values(65536), 17);
    assert_eq!(bits_for_enum_values(65537), 18);
}

#[test]
fn test_bits_for_values_u64_boundaries() {
    // Exhaustive boundary tests from the C++ source.
    // C++ uses TF_BITS_FOR_VALUES(0x...ULL) which passes the literal as n.
    // For all values below >= 2, bits_for_values gives the same result as C++.
    assert_eq!(bits_for_values(0x0000000000000001usize), 0); // n=1 special case
    assert_eq!(bits_for_values(0x0000000000000002usize), 1);
    assert_eq!(bits_for_values(0x0000000000000003usize), 2);
    assert_eq!(bits_for_values(0x0000000000000004usize), 2);
    assert_eq!(bits_for_values(0x0000000000000005usize), 3);
    assert_eq!(bits_for_values(0x0000000000000008usize), 3);
    assert_eq!(bits_for_values(0x0000000000000009usize), 4);
    assert_eq!(bits_for_values(0x0000000000000010usize), 4);
    assert_eq!(bits_for_values(0x0000000000000011usize), 5);
    assert_eq!(bits_for_values(0x0000000000000020usize), 5);
    assert_eq!(bits_for_values(0x0000000000000021usize), 6);
    assert_eq!(bits_for_values(0x0000000000000040usize), 6);
    assert_eq!(bits_for_values(0x0000000000000041usize), 7);
    assert_eq!(bits_for_values(0x0000000000000080usize), 7);
    assert_eq!(bits_for_values(0x0000000000000081usize), 8);
    assert_eq!(bits_for_values(0x0000000000000100usize), 8);
    assert_eq!(bits_for_values(0x0000000000000101usize), 9);
    assert_eq!(bits_for_values(0x0000000000000200usize), 9);
    assert_eq!(bits_for_values(0x0000000000000201usize), 10);
    assert_eq!(bits_for_values(0x0000000000000400usize), 10);
    assert_eq!(bits_for_values(0x0000000000000401usize), 11);
    assert_eq!(bits_for_values(0x0000000000000800usize), 11);
    assert_eq!(bits_for_values(0x0000000000000801usize), 12);
    assert_eq!(bits_for_values(0x0000000000001000usize), 12);
    assert_eq!(bits_for_values(0x0000000000001001usize), 13);
    assert_eq!(bits_for_values(0x0000000000002000usize), 13);
    assert_eq!(bits_for_values(0x0000000000002001usize), 14);
    assert_eq!(bits_for_values(0x0000000000004000usize), 14);
    assert_eq!(bits_for_values(0x0000000000004001usize), 15);
    assert_eq!(bits_for_values(0x0000000000008000usize), 15);
    assert_eq!(bits_for_values(0x0000000000008001usize), 16);
    assert_eq!(bits_for_values(0x0000000000010000usize), 16);
    assert_eq!(bits_for_values(0x0000000000010001usize), 17);
    assert_eq!(bits_for_values(0x0000000000020000usize), 17);
    assert_eq!(bits_for_values(0x0000000000020001usize), 18);
    assert_eq!(bits_for_values(0x0000000000040000usize), 18);
    assert_eq!(bits_for_values(0x0000000000040001usize), 19);
    assert_eq!(bits_for_values(0x0000000000080000usize), 19);
    assert_eq!(bits_for_values(0x0000000000080001usize), 20);
    assert_eq!(bits_for_values(0x0000000000100000usize), 20);
    assert_eq!(bits_for_values(0x0000000000100001usize), 21);
    assert_eq!(bits_for_values(0x0000000000200000usize), 21);
    assert_eq!(bits_for_values(0x0000000000200001usize), 22);
    assert_eq!(bits_for_values(0x0000000000400000usize), 22);
    assert_eq!(bits_for_values(0x0000000000400001usize), 23);
    assert_eq!(bits_for_values(0x0000000000800000usize), 23);
    assert_eq!(bits_for_values(0x0000000000800001usize), 24);
    assert_eq!(bits_for_values(0x0000000001000000usize), 24);
    assert_eq!(bits_for_values(0x0000000001000001usize), 25);
    assert_eq!(bits_for_values(0x0000000002000000usize), 25);
    assert_eq!(bits_for_values(0x0000000002000001usize), 26);
    assert_eq!(bits_for_values(0x0000000004000000usize), 26);
    assert_eq!(bits_for_values(0x0000000004000001usize), 27);
    assert_eq!(bits_for_values(0x0000000008000000usize), 27);
    assert_eq!(bits_for_values(0x0000000008000001usize), 28);
    assert_eq!(bits_for_values(0x0000000010000000usize), 28);
    assert_eq!(bits_for_values(0x0000000010000001usize), 29);
    assert_eq!(bits_for_values(0x0000000020000000usize), 29);
    assert_eq!(bits_for_values(0x0000000020000001usize), 30);
    assert_eq!(bits_for_values(0x0000000040000000usize), 30);
    assert_eq!(bits_for_values(0x0000000040000001usize), 31);
    assert_eq!(bits_for_values(0x0000000080000000usize), 31);
    assert_eq!(bits_for_values(0x0000000080000001usize), 32);

    // 64-bit-only cases
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(bits_for_values(0x0000000100000000usize), 32);
        assert_eq!(bits_for_values(0x0000000100000001usize), 33);
        assert_eq!(bits_for_values(0x0000000200000000usize), 33);
        assert_eq!(bits_for_values(0x0000000200000001usize), 34);
        assert_eq!(bits_for_values(0x0000000400000000usize), 34);
        assert_eq!(bits_for_values(0x0000000400000001usize), 35);
        assert_eq!(bits_for_values(0x0000000800000000usize), 35);
        assert_eq!(bits_for_values(0x0000000800000001usize), 36);
        assert_eq!(bits_for_values(0x0000001000000000usize), 36);
        assert_eq!(bits_for_values(0x0000001000000001usize), 37);
        assert_eq!(bits_for_values(0x0000002000000000usize), 37);
        assert_eq!(bits_for_values(0x0000002000000001usize), 38);
        assert_eq!(bits_for_values(0x0000004000000000usize), 38);
        assert_eq!(bits_for_values(0x0000004000000001usize), 39);
        assert_eq!(bits_for_values(0x0000008000000000usize), 39);
        assert_eq!(bits_for_values(0x0000008000000001usize), 40);
        assert_eq!(bits_for_values(0x0000010000000000usize), 40);
        assert_eq!(bits_for_values(0x0000010000000001usize), 41);
        assert_eq!(bits_for_values(0x0000020000000000usize), 41);
        assert_eq!(bits_for_values(0x0000020000000001usize), 42);
        assert_eq!(bits_for_values(0x0000040000000000usize), 42);
        assert_eq!(bits_for_values(0x0000040000000001usize), 43);
        assert_eq!(bits_for_values(0x0000080000000000usize), 43);
        assert_eq!(bits_for_values(0x0000080000000001usize), 44);
        assert_eq!(bits_for_values(0x0000100000000000usize), 44);
        assert_eq!(bits_for_values(0x0000100000000001usize), 45);
        assert_eq!(bits_for_values(0x0000200000000000usize), 45);
        assert_eq!(bits_for_values(0x0000200000000001usize), 46);
        assert_eq!(bits_for_values(0x0000400000000000usize), 46);
        assert_eq!(bits_for_values(0x0000400000000001usize), 47);
        assert_eq!(bits_for_values(0x0000800000000000usize), 47);
        assert_eq!(bits_for_values(0x0000800000000001usize), 48);
        assert_eq!(bits_for_values(0x0001000000000000usize), 48);
        assert_eq!(bits_for_values(0x0001000000000001usize), 49);
        assert_eq!(bits_for_values(0x0002000000000000usize), 49);
        assert_eq!(bits_for_values(0x0002000000000001usize), 50);
        assert_eq!(bits_for_values(0x0004000000000000usize), 50);
        assert_eq!(bits_for_values(0x0004000000000001usize), 51);
        assert_eq!(bits_for_values(0x0008000000000000usize), 51);
        assert_eq!(bits_for_values(0x0008000000000001usize), 52);
        assert_eq!(bits_for_values(0x0010000000000000usize), 52);
        assert_eq!(bits_for_values(0x0010000000000001usize), 53);
        assert_eq!(bits_for_values(0x0020000000000000usize), 53);
        assert_eq!(bits_for_values(0x0020000000000001usize), 54);
        assert_eq!(bits_for_values(0x0040000000000000usize), 54);
        assert_eq!(bits_for_values(0x0040000000000001usize), 55);
        assert_eq!(bits_for_values(0x0080000000000000usize), 55);
        assert_eq!(bits_for_values(0x0080000000000001usize), 56);
        assert_eq!(bits_for_values(0x0100000000000000usize), 56);
        assert_eq!(bits_for_values(0x0100000000000001usize), 57);
        assert_eq!(bits_for_values(0x0200000000000000usize), 57);
        assert_eq!(bits_for_values(0x0200000000000001usize), 58);
        assert_eq!(bits_for_values(0x0400000000000000usize), 58);
        assert_eq!(bits_for_values(0x0400000000000001usize), 59);
        assert_eq!(bits_for_values(0x0800000000000000usize), 59);
        assert_eq!(bits_for_values(0x0800000000000001usize), 60);
        assert_eq!(bits_for_values(0x1000000000000000usize), 60);
        assert_eq!(bits_for_values(0x1000000000000001usize), 61);
        assert_eq!(bits_for_values(0x2000000000000000usize), 61);
        assert_eq!(bits_for_values(0x2000000000000001usize), 62);
        assert_eq!(bits_for_values(0x4000000000000000usize), 62);
        assert_eq!(bits_for_values(0x4000000000000001usize), 63);
        assert_eq!(bits_for_values(0x8000000000000000usize), 63);
        assert_eq!(bits_for_values(0x8000000000000001usize), 64);
    }
}
