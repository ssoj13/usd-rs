//! glTF IO tests (utilities) parity with `_ref/draco/src/draco/io/gltf_utils_test.cc`.
//!
//! What: Validates JsonWriter formatting and GltfValue formatting/escaping.
//! Why: Ensures encoder JSON output matches Draco C++ expectations.
//! Where used: `cargo test -p draco-rs gltf_utils_test_*`.

use crate::io::gltf_utils::{GltfValue, JsonWriter, Mode};

fn compare_golden(json_writer: &mut JsonWriter, golden: &str) {
    let json = json_writer.move_data();
    assert_eq!(golden, json);
}

#[test]
fn gltf_utils_test_no_data() {
    let mut json_writer = JsonWriter::new();
    compare_golden(&mut json_writer, "");
}

#[test]
fn gltf_utils_test_values() {
    let mut json_writer = JsonWriter::new();
    json_writer.output_value(0);
    compare_golden(&mut json_writer, "0");

    json_writer.reset();
    json_writer.output_value(1);
    compare_golden(&mut json_writer, "1");

    json_writer.reset();
    json_writer.output_value(-1);
    compare_golden(&mut json_writer, "-1");

    json_writer.reset();
    json_writer.output_value(0.0_f64);
    compare_golden(&mut json_writer, "0");

    json_writer.reset();
    json_writer.output_value(1.0_f64);
    compare_golden(&mut json_writer, "1");

    json_writer.reset();
    json_writer.output_value(0.25_f64);
    compare_golden(&mut json_writer, "0.25");

    json_writer.reset();
    json_writer.output_value(-0.25_f64);
    compare_golden(&mut json_writer, "-0.25");

    json_writer.reset();
    json_writer.output_bool(false);
    compare_golden(&mut json_writer, "false");

    json_writer.reset();
    json_writer.output_bool(true);
    compare_golden(&mut json_writer, "true");

    json_writer.reset();
    json_writer.output_named_value("test int", -1);
    compare_golden(&mut json_writer, "\"test int\": -1");

    json_writer.reset();
    json_writer.output_named_value("test float", -10.25_f64);
    compare_golden(&mut json_writer, "\"test float\": -10.25");

    json_writer.reset();
    json_writer.output_named_string("test char*", "I am the string!");
    compare_golden(&mut json_writer, "\"test char*\": \"I am the string!\"");

    json_writer.reset();
    let value = "I am the string!";
    json_writer.output_named_string("test string", value);
    compare_golden(&mut json_writer, "\"test string\": \"I am the string!\"");

    json_writer.reset();
    json_writer.output_named_bool("test bool", false);
    compare_golden(&mut json_writer, "\"test bool\": false");

    json_writer.reset();
    json_writer.output_named_bool("test bool", true);
    compare_golden(&mut json_writer, "\"test bool\": true");
}

#[test]
fn gltf_utils_test_special_characters() {
    let mut json_writer = JsonWriter::new();
    let test_double_quote = "I am double quote\"";
    json_writer.output_named_string("test double quote", test_double_quote);
    compare_golden(
        &mut json_writer,
        "\"test double quote\": \"I am double quote\\\"\"",
    );

    json_writer.reset();
    let test_backspace = "I am backspace\u{8}";
    json_writer.output_named_string("test backspace", test_backspace);
    compare_golden(
        &mut json_writer,
        "\"test backspace\": \"I am backspace\\b\"",
    );

    json_writer.reset();
    let test_form_feed = "I am form feed\u{c}";
    json_writer.output_named_string("test form feed", test_form_feed);
    compare_golden(
        &mut json_writer,
        "\"test form feed\": \"I am form feed\\f\"",
    );

    json_writer.reset();
    let test_newline = "I am newline\n";
    json_writer.output_named_string("test newline", test_newline);
    compare_golden(&mut json_writer, "\"test newline\": \"I am newline\\n\"");

    json_writer.reset();
    let test_tab = "I am tab\t";
    json_writer.output_named_string("test tab", test_tab);
    compare_golden(&mut json_writer, "\"test tab\": \"I am tab\\t\"");

    json_writer.reset();
    let test_backslash = "I am backslash\\";
    json_writer.output_named_string("test backslash", test_backslash);
    compare_golden(
        &mut json_writer,
        "\"test backslash\": \"I am backslash\\\\\"",
    );

    json_writer.reset();
    let test_multiple_special_characters = "\"break\"and\\more\"\\";
    json_writer.output_named_string(
        "test multiple_special_characters",
        test_multiple_special_characters,
    );
    compare_golden(
        &mut json_writer,
        "\"test multiple_special_characters\": \"\\\"break\\\"and\\\\more\\\"\\\\\"",
    );
}

#[test]
fn gltf_utils_test_objects() {
    let mut json_writer = JsonWriter::new();
    json_writer.begin_object();
    json_writer.end_object();
    compare_golden(&mut json_writer, "{\n}");

    json_writer.reset();
    json_writer.begin_object_named("object");
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object\": {\n}");

    json_writer.reset();
    json_writer.begin_object_named("object");
    json_writer.output_value(0);
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object\": {\n  0\n}");

    json_writer.reset();
    json_writer.begin_object_named("object");
    json_writer.output_value(0);
    json_writer.output_value(1);
    json_writer.output_value(2);
    json_writer.output_value(3);
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object\": {\n  0,\n  1,\n  2,\n  3\n}");

    json_writer.reset();
    json_writer.begin_object_named("object1");
    json_writer.end_object();
    json_writer.begin_object_named("object2");
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object1\": {\n},\n\"object2\": {\n}");

    json_writer.reset();
    json_writer.begin_object_named("object1");
    json_writer.begin_object_named("object2");
    json_writer.end_object();
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object1\": {\n  \"object2\": {\n  }\n}");
}

#[test]
fn gltf_utils_test_arrays() {
    let mut json_writer = JsonWriter::new();
    json_writer.begin_array_named("array");
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array\": [\n]");

    json_writer.reset();
    json_writer.begin_array_named("array");
    json_writer.output_value(0);
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array\": [\n  0\n]");

    json_writer.reset();
    json_writer.begin_array_named("array");
    json_writer.output_value(0);
    json_writer.output_value(1);
    json_writer.output_value(2);
    json_writer.output_value(3);
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array\": [\n  0,\n  1,\n  2,\n  3\n]");

    json_writer.reset();
    json_writer.begin_array_named("array1");
    json_writer.end_array();
    json_writer.begin_array_named("array2");
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array1\": [\n],\n\"array2\": [\n]");

    json_writer.reset();
    json_writer.begin_array_named("array1");
    json_writer.begin_array_named("array2");
    json_writer.end_array();
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array1\": [\n  \"array2\": [\n  ]\n]");

    json_writer.reset();
    json_writer.begin_array_named("array1");
    json_writer.begin_array();
    json_writer.end_array();
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array1\": [\n  [\n  ]\n]");
}

#[test]
fn gltf_utils_test_gltf_values() {
    let mut json_writer = JsonWriter::new();
    let int8_value_low = GltfValue::from_i8(i8::MIN);
    let int8_value_high = GltfValue::from_i8(i8::MAX);
    json_writer.output_value(int8_value_low);
    json_writer.output_value(int8_value_high);
    compare_golden(&mut json_writer, "-128,\n127");

    json_writer.reset();
    let uint8_value_low = GltfValue::from_u8(u8::MIN);
    let uint8_value_high = GltfValue::from_u8(u8::MAX);
    json_writer.output_value(uint8_value_low);
    json_writer.output_value(uint8_value_high);
    compare_golden(&mut json_writer, "0,\n255");

    json_writer.reset();
    let int16_value_low = GltfValue::from_i16(i16::MIN);
    let int16_value_high = GltfValue::from_i16(i16::MAX);
    json_writer.output_value(int16_value_low);
    json_writer.output_value(int16_value_high);
    compare_golden(&mut json_writer, "-32768,\n32767");

    json_writer.reset();
    let uint16_value_low = GltfValue::from_u16(u16::MIN);
    let uint16_value_high = GltfValue::from_u16(u16::MAX);
    json_writer.output_value(uint16_value_low);
    json_writer.output_value(uint16_value_high);
    compare_golden(&mut json_writer, "0,\n65535");

    json_writer.reset();
    let uint32_value_low = GltfValue::from_u32(u32::MIN);
    let uint32_value_high = GltfValue::from_u32(u32::MAX);
    json_writer.output_value(uint32_value_low);
    json_writer.output_value(uint32_value_high);
    compare_golden(&mut json_writer, "0,\n4294967295");

    json_writer.reset();
    let float_value_low = GltfValue::from_f32(f32::MIN_POSITIVE);
    let float_value_high = GltfValue::from_f32(f32::MAX);
    json_writer.output_value(float_value_low);
    json_writer.output_value(float_value_high);
    compare_golden(
        &mut json_writer,
        "1.1754943508222875e-38,\n3.4028234663852886e+38",
    );

    json_writer.reset();
    let float_value_0 = GltfValue::from_f32(0.1_f32);
    let float_value_1 = GltfValue::from_f32(1.0_f32);
    json_writer.output_value(float_value_0);
    json_writer.output_value(float_value_1);
    compare_golden(&mut json_writer, "0.10000000149011612,\n1");
}

#[test]
fn gltf_utils_test_objects_compact() {
    let mut json_writer = JsonWriter::new();
    json_writer.set_mode(Mode::Compact);
    json_writer.begin_object();
    json_writer.end_object();
    compare_golden(&mut json_writer, "{}");

    json_writer.reset();
    json_writer.begin_object_named("object");
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object\":{}");

    json_writer.reset();
    json_writer.begin_object_named("object");
    json_writer.output_value(0);
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object\":{0}");

    json_writer.reset();
    json_writer.begin_object_named("object");
    json_writer.output_value(0);
    json_writer.output_value(1);
    json_writer.output_value(2);
    json_writer.output_value(3);
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object\":{0,1,2,3}");

    json_writer.reset();
    json_writer.begin_object_named("object1");
    json_writer.end_object();
    json_writer.begin_object_named("object2");
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object1\":{},\"object2\":{}");

    json_writer.reset();
    json_writer.begin_object_named("object1");
    json_writer.begin_object_named("object2");
    json_writer.end_object();
    json_writer.end_object();
    compare_golden(&mut json_writer, "\"object1\":{\"object2\":{}}");
}

#[test]
fn gltf_utils_test_arrays_compact() {
    let mut json_writer = JsonWriter::new();
    json_writer.set_mode(Mode::Compact);
    json_writer.begin_array_named("array");
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array\":[]");

    json_writer.reset();
    json_writer.begin_array_named("array");
    json_writer.output_value(0);
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array\":[0]");

    json_writer.reset();
    json_writer.begin_array_named("array");
    json_writer.output_value(0);
    json_writer.output_value(1);
    json_writer.output_value(2);
    json_writer.output_value(3);
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array\":[0,1,2,3]");

    json_writer.reset();
    json_writer.begin_array_named("array1");
    json_writer.end_array();
    json_writer.begin_array_named("array2");
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array1\":[],\"array2\":[]");

    json_writer.reset();
    json_writer.begin_array_named("array1");
    json_writer.begin_array_named("array2");
    json_writer.end_array();
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array1\":[\"array2\":[]]");

    json_writer.reset();
    json_writer.begin_array_named("array1");
    json_writer.begin_array();
    json_writer.end_array();
    json_writer.end_array();
    compare_golden(&mut json_writer, "\"array1\":[[]]");
}
