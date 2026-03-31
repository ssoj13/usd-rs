use usd_tf::pointer_and_bits::PointerAndBits;

// The C++ test uses `short` (2-byte, alignment 2 => 1 available bit).
// Rust equivalent: u16 has alignment 2 on all supported platforms.

#[test]
fn test_short_max_value_and_num_bits() {
    // C++: TF_AXIOM(pbs.GetMaxValue() > 0)
    assert!(PointerAndBits::<u16>::max_bits() > 0);
    // C++: TF_AXIOM(pbs.GetNumBitsValues() > 1)
    // GetNumBitsValues() returns 2^num_bits; num_bits() gives the count.
    let num_bits_values = 1usize << PointerAndBits::<u16>::num_bits();
    assert!(num_bits_values > 1);
}

#[test]
fn test_assign_pointer_and_get() {
    let mut data: u16 = 1234;
    let ptr: *mut u16 = &mut data;

    let mut pbs = PointerAndBits::<u16>::null();
    pbs.set_ptr(ptr);

    // C++: TF_AXIOM(pbs.Get() == &data)
    assert_eq!(pbs.get(), ptr);
}

#[test]
fn test_set_bits_and_read_back() {
    let mut data: u16 = 1234;
    let ptr: *mut u16 = &mut data;

    let mut pbs = PointerAndBits::new(ptr, 0);
    pbs.set_bits(1);

    // C++: TF_AXIOM(pbs.BitsAs<int>() == 1)
    let bits_int: Option<i32> = pbs.bits_as();
    assert_eq!(bits_int, Some(1));

    // C++: TF_AXIOM(pbs.BitsAs<bool>() == true)
    assert_eq!(pbs.bits(), 1);

    // C++: TF_AXIOM(pbs.Get() == &data)  — pointer unchanged after set_bits
    assert_eq!(pbs.get(), ptr);
}

#[test]
fn test_copy_and_swap() {
    let mut data: u16 = 1234;
    let mut data2: u16 = 4321;
    let ptr: *mut u16 = &mut data;
    let ptr2: *mut u16 = &mut data2;

    let mut pbs = PointerAndBits::new(ptr, 1);

    // C++: TfPointerAndBits<short>(&data2).Swap(pbs)
    // Creates a temporary with &data2 (bits=0 by default) and swaps into pbs.
    let mut tmp = PointerAndBits::new(ptr2, 0);
    tmp.swap(&mut pbs);

    // After swap: pbs holds what tmp had (ptr2, bits=0).
    // C++: TF_AXIOM(pbs.Get() == &data2)
    assert_eq!(pbs.get(), ptr2);
    // C++: TF_AXIOM(pbs.BitsAs<bool>() == false)
    assert_eq!(pbs.bits(), 0);
}

#[test]
fn test_construct_with_bits() {
    let mut data: u16 = 1234;
    let ptr: *mut u16 = &mut data;

    // C++: TF_AXIOM(TfPointerAndBits<short>(&data, 1).BitsAs<bool>() == true)
    let pab = PointerAndBits::new(ptr, 1);
    assert_eq!(pab.bits(), 1);

    // C++: TF_AXIOM(TfPointerAndBits<short>(&data, 1).Get() == &data)
    assert_eq!(pab.get(), ptr);
}

#[test]
fn test_get_literal() {
    let mut data: u16 = 1234;
    let mut data2: u16 = 4321;
    let ptr: *mut u16 = &mut data;
    let ptr2: *mut u16 = &mut data2;

    // pbs = ptr2, bits=0  (result of the swap in test_copy_and_swap scenario)
    let pbs = PointerAndBits::new(ptr2, 0);

    // C++: TF_AXIOM(TfPointerAndBits<short>(&data, 1).GetLiteral() != pbs.GetLiteral())
    assert_ne!(PointerAndBits::new(ptr, 1).literal(), pbs.literal());

    // C++: TF_AXIOM(TfPointerAndBits<short>(&data, 0).GetLiteral() != pbs.GetLiteral())
    assert_ne!(PointerAndBits::new(ptr, 0).literal(), pbs.literal());

    // C++: TF_AXIOM(TfPointerAndBits<short>(&data2, 1).GetLiteral() != pbs.GetLiteral())
    assert_ne!(PointerAndBits::new(ptr2, 1).literal(), pbs.literal());

    // C++: TF_AXIOM(TfPointerAndBits<short>(&data2, 0).GetLiteral() == pbs.GetLiteral())
    assert_eq!(PointerAndBits::new(ptr2, 0).literal(), pbs.literal());
}
