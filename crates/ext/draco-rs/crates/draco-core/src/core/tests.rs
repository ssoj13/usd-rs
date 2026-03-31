use super::bit_utils::{bits_required, most_significant_bit};
use super::decoder_buffer::BitDecoder;
use super::encoder_buffer::BitEncoder;
use super::math_utils::{increment_mod, int_sqrt};
use super::quantization_utils::{Dequantizer, Quantizer};
use super::status::{error_status, Status, StatusCode};
use super::vector_d::VectorD;
use super::vector_d::{cross_product, Vector2f, Vector2ui, Vector3f, Vector3ui, Vector4f};

type Vector3i = VectorD<i32, 3>;
type Vector4i = VectorD<i32, 4>;

#[test]
fn test_bits_required() {
    assert_eq!(bits_required(0), most_significant_bit(0) as u32);
    assert_eq!(bits_required(1), 0);
    assert_eq!(bits_required(2), 1);
    assert_eq!(bits_required(8), 3);
    assert_eq!(bits_required(255), 7);
    assert_eq!(bits_required(256), 8);
}

#[test]
fn test_bit_coders_byte_aligned() {
    let mut buffer = [0u8; 32];
    let mut encoder = BitEncoder::new(buffer.as_mut_ptr());
    let data: [u8; 8] = [0x76, 0x54, 0x32, 0x10, 0x76, 0x54, 0x32, 0x10];
    let bytes_to_encode = data.len();

    for i in 0..bytes_to_encode {
        encoder.put_bits(data[i] as u32, 8);
        assert_eq!((i + 1) * 8, encoder.bits() as usize);
    }

    let mut decoder = BitDecoder::new();
    decoder.reset(&buffer[..bytes_to_encode]);
    for i in 0..bytes_to_encode {
        let mut x = 0u32;
        assert!(decoder.get_bits(8, &mut x));
        assert_eq!(x, data[i] as u32);
    }

    assert_eq!((bytes_to_encode * 8) as u64, decoder.bits_decoded());
}

#[test]
fn test_bit_coders_non_byte() {
    let mut buffer = [0u8; 32];
    let mut encoder = BitEncoder::new(buffer.as_mut_ptr());
    let data: [u8; 8] = [0x76, 0x54, 0x32, 0x10, 0x76, 0x54, 0x32, 0x10];
    let bits_to_encode: u32 = 51;
    let bytes_to_encode = (bits_to_encode / 8) + 1;

    for i in 0..(bytes_to_encode as usize) {
        let num_bits = if encoder.bits() + 8 <= bits_to_encode as u64 {
            8u32
        } else {
            (bits_to_encode as u64 - encoder.bits()) as u32
        };
        encoder.put_bits(data[i] as u32, num_bits as i32);
    }

    let mut decoder = BitDecoder::new();
    decoder.reset(&buffer[..bytes_to_encode as usize]);
    let mut bits_to_decode = encoder.bits() as i64;
    for i in 0..(bytes_to_encode as usize) {
        let mut x = 0u32;
        let num_bits = if bits_to_decode > 8 {
            8u32
        } else {
            bits_to_decode as u32
        };
        assert!(decoder.get_bits(num_bits, &mut x));
        let bits_to_shift = 8 - num_bits;
        let test_byte = ((data[i] as u32) << bits_to_shift) & 0xff;
        let test_byte = test_byte >> bits_to_shift;
        assert_eq!(x, test_byte);
        bits_to_decode -= 8;
    }

    assert_eq!(bits_to_encode as u64, decoder.bits_decoded());
}

#[test]
fn test_single_bits() {
    let data: u16 = 0xaaaa;
    let bytes = data.to_le_bytes();

    let mut decoder = BitDecoder::new();
    decoder.reset(&bytes);

    for i in 0..16u32 {
        let mut x = 0u32;
        assert!(decoder.get_bits(1, &mut x));
        assert_eq!(x, (i % 2) as u32);
    }

    assert_eq!(16u64, decoder.bits_decoded());
}

#[test]
fn test_multiple_bits() {
    let data: [u8; 8] = [0x76, 0x54, 0x32, 0x10, 0x76, 0x54, 0x32, 0x10];
    let mut decoder = BitDecoder::new();
    decoder.reset(&data);

    let mut x = 0u32;
    for i in 0..2u32 {
        assert!(decoder.get_bits(16, &mut x));
        assert_eq!(x, 0x5476u32);
        assert_eq!(16 + (i * 32), decoder.bits_decoded() as u32);

        assert!(decoder.get_bits(16, &mut x));
        assert_eq!(x, 0x1032u32);
        assert_eq!(32 + (i * 32), decoder.bits_decoded() as u32);
    }
}

#[test]
fn test_math_utils() {
    assert_eq!(increment_mod(1, 1 << 1), 0);

    assert_eq!(int_sqrt(0), 0);
    let mut seed: u64 = 109;
    for _ in 0..10_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let number = seed & ((1u64 << 60) - 1);
        let expected = (number as f64).sqrt().floor() as u64;
        assert_eq!(int_sqrt(number), expected);
    }
}

#[test]
fn test_quantizer() {
    let mut quantizer = Quantizer::new();
    quantizer.init_range(10.0, 255);
    assert_eq!(quantizer.quantize_float(0.0), 0);
    assert_eq!(quantizer.quantize_float(10.0), 255);
    assert_eq!(quantizer.quantize_float(-10.0), -255);
    assert_eq!(quantizer.quantize_float(4.999), 127);
    assert_eq!(quantizer.quantize_float(5.0), 128);
    assert_eq!(quantizer.quantize_float(-4.9999), -127);
    assert_eq!(quantizer.quantize_float(-5.0), -127);
    assert_eq!(quantizer.quantize_float(-5.0001), -128);
    assert!(quantizer.quantize_float(-15.0) < -255);
    assert!(quantizer.quantize_float(15.0) > 255);
}

#[test]
fn test_dequantizer() {
    let mut dequantizer = Dequantizer::new();
    assert!(dequantizer.init_range(10.0, 255));
    assert_eq!(dequantizer.dequantize_float(0), 0.0);
    assert_eq!(dequantizer.dequantize_float(255), 10.0);
    assert_eq!(dequantizer.dequantize_float(-255), -10.0);
    assert_eq!(dequantizer.dequantize_float(128), 10.0 * (128.0 / 255.0));

    assert!(!dequantizer.init_range(1.0, 0));
    assert!(!dequantizer.init_range(1.0, -4));
}

#[test]
fn test_delta_quantization() {
    let mut quantizer_delta = Quantizer::new();
    quantizer_delta.init_delta(0.5);

    let mut quantizer_range = Quantizer::new();
    quantizer_range.init_range(50.0, 100);

    assert_eq!(quantizer_delta.quantize_float(1.2), 2);
    assert_eq!(
        quantizer_delta.quantize_float(10.0),
        quantizer_range.quantize_float(10.0)
    );
    assert_eq!(
        quantizer_delta.quantize_float(-3.3),
        quantizer_range.quantize_float(-3.3)
    );
    assert_eq!(
        quantizer_delta.quantize_float(0.25),
        quantizer_range.quantize_float(0.25)
    );

    let mut dequantizer_delta = Dequantizer::new();
    dequantizer_delta.init_delta(0.5);

    let mut dequantizer_range = Dequantizer::new();
    dequantizer_range.init_range(50.0, 100);

    assert_eq!(dequantizer_delta.dequantize_float(2), 1.0);
    assert_eq!(
        dequantizer_delta.dequantize_float(-4),
        dequantizer_range.dequantize_float(-4)
    );
    assert_eq!(
        dequantizer_delta.dequantize_float(9),
        dequantizer_range.dequantize_float(9)
    );
    assert_eq!(
        dequantizer_delta.dequantize_float(0),
        dequantizer_range.dequantize_float(0)
    );
}

#[test]
fn test_status_output() {
    let status = Status::new(StatusCode::DracoError, "Error msg.");
    assert_eq!(status.code(), StatusCode::DracoError);
    assert_eq!(status.code_string(), "DRACO_ERROR");
    assert_eq!(format!("{}", status), "Error msg.");

    let status2 = error_status("Error msg2.");
    assert_eq!(status2.code(), StatusCode::DracoError);
    assert_eq!(status2.error_msg_string(), "Error msg2.");
    assert_eq!(status2.code_string(), "DRACO_ERROR");
    assert_eq!(status2.code_and_error_string(), "DRACO_ERROR: Error msg2.");
}

#[test]
fn test_vector_operators() {
    let v = Vector3f::default();
    assert_eq!(v[0], 0.0);
    assert_eq!(v[1], 0.0);
    assert_eq!(v[2], 0.0);

    let v = Vector3f::new3(1.0, 2.0, 3.0);
    assert_eq!(v[0], 1.0);
    assert_eq!(v[1], 2.0);
    assert_eq!(v[2], 3.0);

    let mut w = v;
    assert_eq!(v, w);
    assert_ne!(v, Vector3f::new3(0.0, 0.0, 0.0));

    w = -v;
    assert_eq!(w[0], -1.0);
    assert_eq!(w[1], -2.0);
    assert_eq!(w[2], -3.0);

    w = v + v;
    assert_eq!(w[0], 2.0);
    assert_eq!(w[1], 4.0);
    assert_eq!(w[2], 6.0);

    w = w - v;
    assert_eq!(w[0], 1.0);
    assert_eq!(w[1], 2.0);
    assert_eq!(w[2], 3.0);

    w = v * 2.0;
    assert_eq!(w[0], 2.0);
    assert_eq!(w[1], 4.0);
    assert_eq!(w[2], 6.0);
    w = 2.0 * v;
    assert_eq!(w[0], 2.0);
    assert_eq!(w[1], 4.0);
    assert_eq!(w[2], 6.0);

    assert_eq!(v.squared_norm(), 14.0);
    assert_eq!(v.dot(&v), 14.0);

    let mut new_v = v;
    new_v.normalize();
    let tolerance = 1e-5_f32;
    let magnitude = v.squared_norm().sqrt();
    let new_magnitude = new_v.squared_norm().sqrt();
    assert!((new_magnitude - 1.0).abs() < tolerance);
    for i in 0..3 {
        new_v[i] *= magnitude;
        assert!((new_v[i] - v[i]).abs() < tolerance);
    }

    let mut x = Vector3f::new3(0.0, 0.0, 0.0);
    x.normalize();
    for i in 0..3 {
        assert_eq!(0.0, x[i]);
    }
}

#[test]
fn test_addition_assignment_operator() {
    let v = Vector3ui::new3(1, 2, 3);
    let mut w = Vector3ui::new3(4, 5, 6);

    w += v;
    assert_eq!(w[0], 5);
    assert_eq!(w[1], 7);
    assert_eq!(w[2], 9);

    w += w;
    assert_eq!(w[0], 10);
    assert_eq!(w[1], 14);
    assert_eq!(w[2], 18);
}

#[test]
fn test_subtraction_assignment_operator() {
    let v = Vector3ui::new3(1, 2, 3);
    let mut w = Vector3ui::new3(4, 6, 8);

    w -= v;
    assert_eq!(w[0], 3);
    assert_eq!(w[1], 4);
    assert_eq!(w[2], 5);

    w -= w;
    assert_eq!(w[0], 0);
    assert_eq!(w[1], 0);
    assert_eq!(w[2], 0);
}

#[test]
fn test_multiplication_assignment_operator() {
    let mut v = Vector3ui::new3(1, 2, 3);
    let mut w = Vector3ui::new3(4, 5, 6);

    w *= v;
    assert_eq!(w[0], 4);
    assert_eq!(w[1], 10);
    assert_eq!(w[2], 18);

    v *= v;
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 4);
    assert_eq!(v[2], 9);
}

#[test]
fn test_get_normalized() {
    let original = Vector3f::new3(2.0, 3.0, -4.0);
    let normalized = original.get_normalized();
    let magnitude = original.squared_norm().sqrt();
    let tolerance = 1e-5_f32;
    assert!((normalized[0] - original[0] / magnitude).abs() < tolerance);
    assert!((normalized[1] - original[1] / magnitude).abs() < tolerance);
    assert!((normalized[2] - original[2] / magnitude).abs() < tolerance);
}

#[test]
fn test_get_normalized_with_zero_length_vector() {
    let original = Vector3f::new3(0.0, 0.0, 0.0);
    let normalized = original.get_normalized();
    assert_eq!(normalized[0], 0.0);
    assert_eq!(normalized[1], 0.0);
    assert_eq!(normalized[2], 0.0);
}

#[test]
fn test_cross_product_3d() {
    let e1 = Vector3i::new3(1, 0, 0);
    let e2 = Vector3i::new3(0, 1, 0);
    let e3 = Vector3i::new3(0, 0, 1);
    let o = Vector3i::new3(0, 0, 0);
    assert_eq!(e3, cross_product(&e1, &e2));
    assert_eq!(e1, cross_product(&e2, &e3));
    assert_eq!(e2, cross_product(&e3, &e1));
    assert_eq!(-e3, cross_product(&e2, &e1));
    assert_eq!(-e1, cross_product(&e3, &e2));
    assert_eq!(-e2, cross_product(&e1, &e3));
    assert_eq!(o, cross_product(&e1, &e1));
    assert_eq!(o, cross_product(&e2, &e2));
    assert_eq!(o, cross_product(&e3, &e3));

    let v1 = Vector3i::new3(123, -62, 223);
    let v2 = Vector3i::new3(734, 244, -13);
    let orth = cross_product(&v1, &v2);
    assert_eq!(0, v1.dot(&orth));
    assert_eq!(0, v2.dot(&orth));
}

#[test]
fn test_abs_sum() {
    let v = Vector3i::new3(0, 0, 0);
    assert_eq!(v.abs_sum(), 0);
    assert_eq!(Vector3i::new3(0, 0, 0).abs_sum(), 0);
    assert_eq!(Vector3i::new3(1, 2, 3).abs_sum(), 6);
    assert_eq!(Vector3i::new3(-1, -2, -3).abs_sum(), 6);
    assert_eq!(Vector3i::new3(-2, 4, -8).abs_sum(), 14);
    assert_eq!(Vector4i::new4(-2, 4, -8, 3).abs_sum(), 17);
}

#[test]
fn test_min_max_coeff() {
    let vi = Vector4i::new4(-10, 5, 2, 3);
    assert_eq!(vi.min_coeff(), -10);
    assert_eq!(vi.max_coeff(), 5);

    let vf = Vector3f::new3(6.0, 1000.0, -101.0);
    assert_eq!(vf.min_coeff(), -101.0);
    assert_eq!(vf.max_coeff(), 1000.0);
}

#[test]
fn test_ostream() {
    let vector = VectorD::<i64, 3>::new3(1, 2, 3);
    let output = format!("{} ", vector);
    assert_eq!(output, "1 2 3 ");
}

#[test]
fn test_convert_constructor() {
    let vector = VectorD::<i64, 3>::new3(1, 2, 3);
    let vector3f = VectorD::<f32, 3>::from_vector(vector);
    assert_eq!(vector3f, Vector3f::new3(1.0, 2.0, 3.0));

    let vector2f = VectorD::<f32, 2>::from_vector(vector);
    assert_eq!(vector2f, Vector2f::new2(1.0, 2.0));

    let vector4f = VectorD::<f32, 4>::from_vector(vector3f);
    assert_eq!(vector4f, Vector4f::new4(1.0, 2.0, 3.0, 0.0));

    let vector1d = VectorD::<f64, 1>::from_vector(vector3f);
    assert_eq!(vector1d[0], 1.0);
}

#[test]
fn test_binary_ops() {
    let vector_0 = Vector4f::new4(1.0, 2.3, 4.2, -10.0);
    assert_eq!(vector_0 * Vector4f::new4(1.0, 1.0, 1.0, 1.0), vector_0);
    assert_eq!(
        vector_0 * Vector4f::new4(0.0, 0.0, 0.0, 0.0),
        Vector4f::new4(0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        vector_0 * Vector4f::new4(0.1, 0.2, 0.3, 0.4),
        Vector4f::new4(0.1, 0.46, 1.26, -4.0)
    );
}

#[test]
fn test_vector2_aliases() {
    let v = Vector2ui::new2(1, 2);
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 2);
}
