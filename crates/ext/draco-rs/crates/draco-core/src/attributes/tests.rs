//! Attribute module tests (parity with Draco reference).

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::AttributeValueIndex;
use crate::attributes::point_attribute::{PointAttribute, PointAttributeHasher};
use crate::core::draco_types::DataType;

#[test]
fn test_copy_point_attribute() {
    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Position,
        1,
        DataType::Int32,
        false,
        10,
    );

    for i in 0..10i32 {
        pa.set_attribute_value(AttributeValueIndex::from(i as u32), &i);
    }

    pa.set_unique_id(12);

    let mut other_pa = PointAttribute::new();
    other_pa.copy_from(&pa);

    let hasher = PointAttributeHasher;
    assert_eq!(hasher.hash(&pa), hasher.hash(&other_pa));
    assert_eq!(pa.unique_id(), other_pa.unique_id());

    for i in 0..10i32 {
        let mut bytes = [0u8; 4];
        other_pa.get_value_bytes(AttributeValueIndex::from(i as u32), &mut bytes);
        let data = i32::from_ne_bytes(bytes);
        assert_eq!(data, i);
    }
}

#[test]
fn test_get_value_float() {
    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Position,
        3,
        DataType::Float32,
        false,
        5,
    );
    for i in 0..5i32 {
        let points = [
            i as f32 * 3.0,
            (i as f32 * 3.0) + 1.0,
            (i as f32 * 3.0) + 2.0,
        ];
        pa.set_attribute_value_array(AttributeValueIndex::from(i as u32), &points);
    }

    for i in 0..5i32 {
        let att_value = pa.get_value_array::<f32, 3>(AttributeValueIndex::from(i as u32));
        assert_eq!(att_value[0], i as f32 * 3.0);
        assert_eq!(att_value[1], (i as f32 * 3.0) + 1.0);
        assert_eq!(att_value[2], (i as f32 * 3.0) + 2.0);
    }
}

#[test]
fn test_get_array() {
    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Position,
        3,
        DataType::Float32,
        false,
        5,
    );
    for i in 0..5i32 {
        let points = [
            i as f32 * 3.0,
            (i as f32 * 3.0) + 1.0,
            (i as f32 * 3.0) + 2.0,
        ];
        pa.set_attribute_value_array(AttributeValueIndex::from(i as u32), &points);
    }

    for i in 0..5i32 {
        let att_value = pa.get_value_array::<f32, 3>(AttributeValueIndex::from(i as u32));
        assert_eq!(att_value[0], i as f32 * 3.0);
        assert_eq!(att_value[1], (i as f32 * 3.0) + 1.0);
        assert_eq!(att_value[2], (i as f32 * 3.0) + 2.0);
    }

    for i in 0..5i32 {
        let mut att_value = [0.0f32; 3];
        assert!(
            pa.get_value_array_into::<f32, 3>(AttributeValueIndex::from(i as u32), &mut att_value)
        );
        assert_eq!(att_value[0], i as f32 * 3.0);
        assert_eq!(att_value[1], (i as f32 * 3.0) + 1.0);
        assert_eq!(att_value[2], (i as f32 * 3.0) + 2.0);
    }
}

#[test]
fn test_array_read_error() {
    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Position,
        3,
        DataType::Float32,
        false,
        5,
    );
    for i in 0..5i32 {
        let points = [
            i as f32 * 3.0,
            (i as f32 * 3.0) + 1.0,
            (i as f32 * 3.0) + 2.0,
        ];
        pa.set_attribute_value_array(AttributeValueIndex::from(i as u32), &points);
    }

    let mut att_value = [0.0f32; 3];
    assert!(!pa.get_value_array_into::<f32, 3>(AttributeValueIndex::from(5u32), &mut att_value));
}

#[test]
fn test_convert_value_all() {
    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Position,
        3,
        DataType::Float32,
        false,
        3,
    );
    for i in 0..3i32 {
        let v = [
            i as f32 * 10.0,
            i as f32 * 10.0 + 1.0,
            i as f32 * 10.0 + 2.0,
        ];
        pa.set_attribute_value_array(AttributeValueIndex::from(i as u32), &v);
    }
    let mut out = [0.0f32; 3];
    assert!(pa.convert_value_all(AttributeValueIndex::from(1), &mut out));
    assert_eq!(out[0], 10.0);
    assert_eq!(out[1], 11.0);
    assert_eq!(out[2], 12.0);
}

#[test]
fn test_resize() {
    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Position,
        3,
        DataType::Float32,
        false,
        5,
    );
    assert_eq!(pa.size(), 5);
    let buf = pa.buffer().unwrap();
    assert_eq!(buf.borrow().data_size(), 4 * 3 * 5);

    pa.resize(10);
    assert_eq!(pa.size(), 10);
    let buf = pa.buffer().unwrap();
    assert_eq!(buf.borrow().data_size(), 4 * 3 * 10);
}
