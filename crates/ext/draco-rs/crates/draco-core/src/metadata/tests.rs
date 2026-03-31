//! Metadata tests ported from Draco C++ reference.
//!
//! What: Verifies metadata, property tables, and structural metadata behaviors.
//! Why: Ensures Rust parity with C++ EXT_structural_metadata semantics.
//! How: Mirrors reference unit tests for setters, copying, and comparisons.
//! Where: Exercises `crate::metadata` modules in `draco-core`.

use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::metadata::geometry_metadata::{AttributeMetadata, GeometryMetadata};
use crate::metadata::metadata::{Metadata, MetadataString};
use crate::metadata::property_attribute::{
    Property as PropertyAttributeProperty, PropertyAttribute,
};
use crate::metadata::property_table::{
    Data as PropertyTableData, Offsets as PropertyTableOffsets, Property as PropertyTableProperty,
    PropertyTable,
};
use crate::metadata::structural_metadata::StructuralMetadata;
use crate::metadata::structural_metadata_schema::{
    Object as StructuralMetadataObject, ObjectType as StructuralMetadataObjectType,
    StructuralMetadataSchema,
};
use crate::metadata::{MetadataDecoder, MetadataEncoder};

#[test]
fn metadata_remove_entry() {
    let mut metadata = Metadata::new();
    metadata.add_entry_int("int", 100);
    metadata.remove_entry("int");
    let mut int_value = 0i32;
    assert!(!metadata.get_entry_int("int", &mut int_value));
}

#[test]
fn metadata_single_entry() {
    let mut metadata = Metadata::new();
    metadata.add_entry_int("int", 100);
    let mut int_value = 0i32;
    assert!(metadata.get_entry_int("int", &mut int_value));
    assert_eq!(int_value, 100);

    metadata.add_entry_double("double", 1.234);
    let mut double_value = 0.0f64;
    assert!(metadata.get_entry_double("double", &mut double_value));
    assert_eq!(double_value, 1.234);
}

#[test]
fn metadata_write_over_entry() {
    let mut metadata = Metadata::new();
    metadata.add_entry_int("int", 100);
    metadata.add_entry_int("int", 200);
    let mut int_value = 0i32;
    assert!(metadata.get_entry_int("int", &mut int_value));
    assert_eq!(int_value, 200);
}

#[test]
fn metadata_array_entry() {
    let mut metadata = Metadata::new();
    let int_array = vec![1i32, 2, 3];
    metadata.add_entry_int_array("int_array", &int_array);
    let mut return_int_array: Vec<i32> = Vec::new();
    assert!(metadata.get_entry_int_array("int_array", &mut return_int_array));
    assert_eq!(return_int_array.len(), 3);
    assert_eq!(return_int_array[0], 1);
    assert_eq!(return_int_array[1], 2);
    assert_eq!(return_int_array[2], 3);

    let double_array = vec![0.1f64, 0.2, 0.3];
    metadata.add_entry_double_array("double_array", &double_array);
    let mut return_double_array: Vec<f64> = Vec::new();
    assert!(metadata.get_entry_double_array("double_array", &mut return_double_array));
    assert_eq!(return_double_array.len(), 3);
    assert_eq!(return_double_array[0], 0.1);
    assert_eq!(return_double_array[1], 0.2);
    assert_eq!(return_double_array[2], 0.3);
}

#[test]
fn metadata_string_entry() {
    let mut metadata = Metadata::new();
    let entry_value = "test string entry";
    metadata.add_entry_string("string", entry_value);
    let mut return_value = MetadataString::default();
    assert!(metadata.get_entry_string("string", &mut return_value));
    assert_eq!(entry_value.as_bytes().len(), return_value.as_bytes().len());
    assert_eq!(entry_value.as_bytes(), return_value.as_bytes());
}

#[test]
fn metadata_binary_entry() {
    let mut metadata = Metadata::new();
    let binarydata: Vec<u8> = vec![0x1, 0x2, 0x3, 0x4];
    metadata.add_entry_binary("binary_data", &binarydata);
    let mut return_binarydata: Vec<u8> = Vec::new();
    assert!(metadata.get_entry_binary("binary_data", &mut return_binarydata));
    assert_eq!(binarydata.len(), return_binarydata.len());
    for i in 0..binarydata.len() {
        assert_eq!(binarydata[i], return_binarydata[i]);
    }
}

#[test]
fn metadata_binary_name_round_trip() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let name = [0xffu8, b'a', 0x00, b'b'];
    let value = [0x1u8, 0x2, 0x3];

    let mut metadata = Metadata::new();
    metadata.add_entry_binary(name, &value);

    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));

    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));

    let mut decoded_value = Vec::new();
    assert!(decoded_metadata.get_entry_binary(name, &mut decoded_value));
    assert_eq!(decoded_value, value);
}

#[test]
fn metadata_binary_string_payload_round_trip() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let value = [0xffu8, b'a', 0x00, b'b'];

    let mut metadata = Metadata::new();
    metadata.add_entry_string("string", value);

    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));

    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));

    let mut decoded_value = MetadataString::default();
    assert!(decoded_metadata.get_entry_string("string", &mut decoded_value));
    assert_eq!(decoded_value.as_bytes(), value);
}

#[test]
fn metadata_nested_metadata() {
    let mut metadata = Metadata::new();
    let mut sub_metadata = Metadata::new();
    sub_metadata.add_entry_int("int", 100);
    assert!(metadata.add_sub_metadata("sub0", sub_metadata));

    let sub_metadata_ptr = metadata.sub_metadata("sub0").expect("Missing sub0");
    let mut int_value = 0i32;
    assert!(sub_metadata_ptr.get_entry_int("int", &mut int_value));
    assert_eq!(int_value, 100);

    sub_metadata_ptr.add_entry_int("new_entry", 20);
    assert!(sub_metadata_ptr.get_entry_int("new_entry", &mut int_value));
    assert_eq!(int_value, 20);
}

#[test]
fn metadata_hard_copy() {
    let mut metadata = Metadata::new();
    metadata.add_entry_int("int", 100);
    let mut sub_metadata = Metadata::new();
    sub_metadata.add_entry_int("int", 200);
    assert!(metadata.add_sub_metadata("sub0", sub_metadata));

    let copied_metadata = metadata.clone();
    let mut int_value = 0i32;
    assert!(copied_metadata.get_entry_int("int", &mut int_value));
    assert_eq!(int_value, 100);

    let sub_metadata_ptr = copied_metadata
        .get_sub_metadata("sub0")
        .expect("Missing sub0");
    let mut sub_int_value = 0i32;
    assert!(sub_metadata_ptr.get_entry_int("int", &mut sub_int_value));
    assert_eq!(sub_int_value, 200);
}

#[test]
fn metadata_geometry_metadata() {
    let mut geometry_metadata = GeometryMetadata::new();
    let mut att_metadata = AttributeMetadata::new();
    att_metadata.set_att_unique_id(10);
    att_metadata.add_entry_int("int", 100);
    att_metadata.add_entry_string("name", "pos");

    assert!(!geometry_metadata.add_attribute_metadata(None));
    assert!(geometry_metadata.add_attribute_metadata(Some(Box::new(att_metadata))));

    assert!(geometry_metadata
        .get_attribute_metadata_by_unique_id(10)
        .is_some());
    assert!(geometry_metadata
        .get_attribute_metadata_by_unique_id(1)
        .is_none());

    let requested_att_metadata = geometry_metadata
        .get_attribute_metadata_by_string_entry("name", "pos")
        .expect("Missing metadata by string entry");
    assert_eq!(requested_att_metadata.att_unique_id(), 10);
    assert!(geometry_metadata
        .get_attribute_metadata_by_string_entry("name", "not_exists")
        .is_none());
}

fn check_blob_of_data_are_equal(data0: &[u8], data1: &[u8]) {
    assert_eq!(data0.len(), data1.len());
    for i in 0..data0.len() {
        assert_eq!(data0[i], data1[i]);
    }
}

fn check_metadatas_are_equal(metadata0: &Metadata, metadata1: &Metadata) {
    assert_eq!(metadata0.num_entries(), metadata1.num_entries());
    let entries0 = metadata0.entries();
    let entries1 = metadata1.entries();
    for (entry_name, entry_value0) in entries0 {
        let entry1 = entries1.get(entry_name).expect("Missing entry");
        check_blob_of_data_are_equal(entry_value0.data(), entry1.data());
    }

    assert_eq!(
        metadata0.sub_metadatas().len(),
        metadata1.sub_metadatas().len()
    );
    for (name, sub_metadata0) in metadata0.sub_metadatas() {
        let sub_metadata1 = metadata1
            .get_sub_metadata(name)
            .expect("Missing sub-metadata");
        check_metadatas_are_equal(sub_metadata0, sub_metadata1);
    }
}

fn check_geometry_metadatas_are_equal(metadata0: &GeometryMetadata, metadata1: &GeometryMetadata) {
    assert_eq!(
        metadata0.attribute_metadatas().len(),
        metadata1.attribute_metadatas().len()
    );
    let att_metadatas0 = metadata0.attribute_metadatas();
    let att_metadatas1 = metadata1.attribute_metadatas();
    for i in 0..att_metadatas0.len() {
        check_metadatas_are_equal(att_metadatas0[i].as_ref(), att_metadatas1[i].as_ref());
    }
    check_metadatas_are_equal(metadata0, metadata1);
}

#[test]
fn metadata_encoder_single_entry() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let mut metadata = Metadata::new();
    metadata.add_entry_int("int", 100);
    assert_eq!(metadata.num_entries(), 1);
    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));

    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));
    check_metadatas_are_equal(&metadata, &decoded_metadata);
}

#[test]
fn metadata_encoder_multiple_entries() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let mut metadata = Metadata::new();
    metadata.add_entry_int("int", 100);
    metadata.add_entry_double("double", 1.234);
    metadata.add_entry_string("string", "test string entry");
    assert_eq!(metadata.num_entries(), 3);

    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));
    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));
    check_metadatas_are_equal(&metadata, &decoded_metadata);
}

#[test]
fn metadata_encoder_array_entries() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let mut metadata = Metadata::new();
    let int_array = vec![1i32, 2, 3];
    metadata.add_entry_int_array("int_array", &int_array);
    let double_array = vec![0.1f64, 0.2, 0.3];
    metadata.add_entry_double_array("double_array", &double_array);
    assert_eq!(metadata.num_entries(), 2);

    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));
    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));
    check_metadatas_are_equal(&metadata, &decoded_metadata);
}

#[test]
fn metadata_encoder_binary_entry() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let mut metadata = Metadata::new();
    let binarydata: Vec<u8> = vec![0x1, 0x2, 0x3, 0x4];
    metadata.add_entry_binary("binary_data", &binarydata);

    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));
    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));
    check_metadatas_are_equal(&metadata, &decoded_metadata);
}

#[test]
fn metadata_encoder_nested_metadata() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let mut metadata = Metadata::new();
    metadata.add_entry_double("double", 1.234);
    let mut sub_metadata = Metadata::new();
    sub_metadata.add_entry_int("int", 100);
    assert!(metadata.add_sub_metadata("sub0", sub_metadata));

    assert!(encoder.encode_metadata(&mut encoder_buffer, &metadata));
    let mut decoded_metadata = Metadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_metadata(&mut decoder_buffer, &mut decoded_metadata));
    check_metadatas_are_equal(&metadata, &decoded_metadata);
}

#[test]
fn metadata_encoder_geometry_metadata() {
    let encoder = MetadataEncoder::new();
    let mut decoder = MetadataDecoder::new();
    let mut encoder_buffer = EncoderBuffer::new();
    let mut decoder_buffer = DecoderBuffer::new();

    let mut geometry_metadata = GeometryMetadata::new();
    let mut att_metadata = AttributeMetadata::new();
    att_metadata.add_entry_int("int", 100);
    att_metadata.add_entry_string("name", "pos");
    assert!(geometry_metadata.add_attribute_metadata(Some(Box::new(att_metadata))));

    assert!(encoder.encode_geometry_metadata(&mut encoder_buffer, &geometry_metadata));
    let mut decoded_metadata = GeometryMetadata::new();
    decoder_buffer.init(encoder_buffer.data());
    assert!(decoder.decode_geometry_metadata(&mut decoder_buffer, &mut decoded_metadata));
    check_geometry_metadatas_are_equal(&geometry_metadata, &decoded_metadata);
}

#[test]
fn property_attribute_property_defaults() {
    let property = PropertyAttributeProperty::new();
    assert!(property.name().is_empty());
    assert!(property.attribute_name().is_empty());
}

#[test]
fn property_attribute_defaults() {
    let attribute = PropertyAttribute::new();
    assert!(attribute.name().is_empty());
    assert!(attribute.class().is_empty());
    assert_eq!(attribute.num_properties(), 0);
}

#[test]
fn property_attribute_property_setters_and_getters() {
    let mut property = PropertyAttributeProperty::new();
    property.set_name("The magnitude.");
    property.set_attribute_name("_MAGNITUDE");
    assert_eq!(property.name(), "The magnitude.");
    assert_eq!(property.attribute_name(), "_MAGNITUDE");
}

#[test]
fn property_attribute_setters_and_getters() {
    let mut attribute = PropertyAttribute::new();
    attribute.set_name("The movement.");
    attribute.set_class("movement");
    {
        let mut property = PropertyAttributeProperty::new();
        property.set_name("The magnitude.");
        property.set_attribute_name("_MAGNITUDE");
        assert_eq!(attribute.add_property(property), 0);
    }
    {
        let mut property = PropertyAttributeProperty::new();
        property.set_name("The direction.");
        property.set_attribute_name("_DIRECTION");
        assert_eq!(attribute.add_property(property), 1);
    }

    assert_eq!(attribute.name(), "The movement.");
    assert_eq!(attribute.class(), "movement");
    assert_eq!(attribute.num_properties(), 2);
    assert_eq!(attribute.property(0).name(), "The magnitude.");
    assert_eq!(attribute.property(0).attribute_name(), "_MAGNITUDE");
    assert_eq!(attribute.property(1).name(), "The direction.");
    assert_eq!(attribute.property(1).attribute_name(), "_DIRECTION");

    attribute.remove_property(0);
    assert_eq!(attribute.num_properties(), 1);
    assert_eq!(attribute.property(0).name(), "The direction.");
    assert_eq!(attribute.property(0).attribute_name(), "_DIRECTION");
    attribute.remove_property(0);
    assert_eq!(attribute.num_properties(), 0);
}

#[test]
fn property_attribute_property_copy() {
    let mut property = PropertyAttributeProperty::new();
    property.set_name("The direction.");
    property.set_attribute_name("_DIRECTION");

    let mut copy = PropertyAttributeProperty::new();
    copy.copy_from(&property);

    assert_eq!(copy.name(), "The direction.");
    assert_eq!(copy.attribute_name(), "_DIRECTION");
}

#[test]
fn property_attribute_copy() {
    let mut attribute = PropertyAttribute::new();
    attribute.set_name("The movement.");
    attribute.set_class("movement");
    {
        let mut property = PropertyAttributeProperty::new();
        property.set_name("The magnitude.");
        property.set_attribute_name("_MAGNITUDE");
        assert_eq!(attribute.add_property(property), 0);
    }
    {
        let mut property = PropertyAttributeProperty::new();
        property.set_name("The direction.");
        property.set_attribute_name("_DIRECTION");
        assert_eq!(attribute.add_property(property), 1);
    }

    let mut copy = PropertyAttribute::new();
    copy.copy_from(&attribute);

    assert_eq!(copy.name(), "The movement.");
    assert_eq!(copy.class(), "movement");
    assert_eq!(copy.num_properties(), 2);
    assert_eq!(copy.property(0).name(), "The magnitude.");
    assert_eq!(copy.property(0).attribute_name(), "_MAGNITUDE");
    assert_eq!(copy.property(1).name(), "The direction.");
    assert_eq!(copy.property(1).attribute_name(), "_DIRECTION");
}

#[test]
fn property_attribute_property_compare() {
    {
        let property = PropertyAttributeProperty::new();
        assert_eq!(property, property);
    }
    {
        let a = PropertyAttributeProperty::new();
        let b = PropertyAttributeProperty::new();
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyAttributeProperty::new();
        let mut b = PropertyAttributeProperty::new();
        a.set_name("The magnitude.");
        b.set_name("The direction.");
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyAttributeProperty::new();
        let mut b = PropertyAttributeProperty::new();
        a.set_attribute_name("_MAGNITUDE");
        b.set_attribute_name("_DIRECTION");
        assert_ne!(a, b);
    }
}

#[test]
fn property_attribute_compare() {
    {
        let attribute = PropertyAttribute::new();
        assert_eq!(attribute, attribute);
    }
    {
        let a = PropertyAttribute::new();
        let b = PropertyAttribute::new();
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyAttribute::new();
        let mut b = PropertyAttribute::new();
        a.set_name("The movement.");
        b.set_name("The reflection.");
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyAttribute::new();
        let mut b = PropertyAttribute::new();
        a.set_class("movement");
        b.set_class("reflection");
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyAttribute::new();
        let mut b = PropertyAttribute::new();
        a.add_property(PropertyAttributeProperty::new());
        b.add_property(PropertyAttributeProperty::new());
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyAttribute::new();
        let mut b = PropertyAttribute::new();
        a.add_property(PropertyAttributeProperty::new());
        b.add_property(PropertyAttributeProperty::new());
        b.add_property(PropertyAttributeProperty::new());
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyAttribute::new();
        let mut b = PropertyAttribute::new();
        let mut p1 = PropertyAttributeProperty::new();
        let mut p2 = PropertyAttributeProperty::new();
        p1.set_name("The magnitude.");
        p2.set_name("The direction.");
        a.add_property(p1);
        b.add_property(p2);
        assert_ne!(a, b);
    }
}

#[test]
fn property_table_property_data_defaults() {
    let data = PropertyTableData::default();
    assert!(data.data.is_empty());
    assert_eq!(data.target, 0);
}

#[test]
fn property_table_property_defaults() {
    let property = PropertyTableProperty::new();
    assert!(property.name().is_empty());
    assert!(property.data().data.is_empty());
    {
        let offsets = property.array_offsets();
        assert!(offsets.type_name.is_empty());
        assert!(offsets.data.data.is_empty());
        assert_eq!(offsets.data.target, 0);
    }
    {
        let offsets = property.string_offsets();
        assert!(offsets.type_name.is_empty());
        assert!(offsets.data.data.is_empty());
        assert_eq!(offsets.data.target, 0);
    }
}

#[test]
fn property_table_defaults() {
    let table = PropertyTable::new();
    assert!(table.name().is_empty());
    assert!(table.class().is_empty());
    assert_eq!(table.count(), 0);
    assert_eq!(table.num_properties(), 0);
}

#[test]
fn property_table_property_setters_and_getters() {
    let mut property = PropertyTableProperty::new();
    property.set_name("Unfortunate Conflict Of Evidence");
    property.data_mut().data.push(2);
    assert_eq!(property.name(), "Unfortunate Conflict Of Evidence");
    assert_eq!(property.data().data.len(), 1);
    assert_eq!(property.data().data[0], 2);
}

#[test]
fn property_table_setters_and_getters() {
    let mut table = PropertyTable::new();
    table.set_name("Just Read The Instructions");
    table.set_class("General Contact Unit");
    table.set_count(456);
    {
        let mut property = PropertyTableProperty::new();
        property.set_name("Determinist");
        assert_eq!(table.add_property(property), 0);
    }
    {
        let mut property = PropertyTableProperty::new();
        property.set_name("Revisionist");
        assert_eq!(table.add_property(property), 1);
    }

    assert_eq!(table.name(), "Just Read The Instructions");
    assert_eq!(table.class(), "General Contact Unit");
    assert_eq!(table.count(), 456);
    assert_eq!(table.num_properties(), 2);
    assert_eq!(table.property(0).name(), "Determinist");
    assert_eq!(table.property(1).name(), "Revisionist");

    table.remove_property(0);
    assert_eq!(table.num_properties(), 1);
    assert_eq!(table.property(0).name(), "Revisionist");
    table.remove_property(0);
    assert_eq!(table.num_properties(), 0);
}

#[test]
fn property_table_property_copy() {
    let mut property = PropertyTableProperty::new();
    property.set_name("Unfortunate Conflict Of Evidence");
    property.data_mut().data.push(2);

    let mut copy = PropertyTableProperty::new();
    copy.copy_from(&property);

    assert_eq!(copy.name(), "Unfortunate Conflict Of Evidence");
    assert_eq!(copy.data().data.len(), 1);
    assert_eq!(copy.data().data[0], 2);
}

#[test]
fn property_table_copy() {
    let mut table = PropertyTable::new();
    table.set_name("Just Read The Instructions");
    table.set_class("General Contact Unit");
    table.set_count(456);
    {
        let mut property = PropertyTableProperty::new();
        property.set_name("Determinist");
        table.add_property(property);
    }
    {
        let mut property = PropertyTableProperty::new();
        property.set_name("Revisionist");
        table.add_property(property);
    }

    let mut copy = PropertyTable::new();
    copy.copy_from(&table);

    assert_eq!(copy.name(), "Just Read The Instructions");
    assert_eq!(copy.class(), "General Contact Unit");
    assert_eq!(copy.count(), 456);
    assert_eq!(copy.num_properties(), 2);
    assert_eq!(copy.property(0).name(), "Determinist");
    assert_eq!(copy.property(1).name(), "Revisionist");
}

#[test]
fn property_table_property_data_compare() {
    {
        let data = PropertyTableData::default();
        assert_eq!(data, data);
    }
    {
        let a = PropertyTableData::default();
        let b = PropertyTableData::default();
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyTableData::default();
        let mut b = PropertyTableData::default();
        a.target = 1;
        b.target = 2;
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTableData::default();
        let mut b = PropertyTableData::default();
        a.data = vec![1];
        b.data = vec![2];
        assert_ne!(a, b);
    }
}

#[test]
fn property_table_offsets_compare() {
    {
        let offsets = PropertyTableOffsets::default();
        assert_eq!(offsets, offsets);
    }
    {
        let a = PropertyTableOffsets::default();
        let b = PropertyTableOffsets::default();
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyTableOffsets::default();
        let mut b = PropertyTableOffsets::default();
        a.type_name = "UINT8".to_string();
        b.type_name = "UINT16".to_string();
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTableOffsets::default();
        let mut b = PropertyTableOffsets::default();
        a.data.target = 1;
        b.data.target = 2;
        assert_ne!(a, b);
    }
}

#[test]
fn property_table_property_compare() {
    {
        let property = PropertyTableProperty::new();
        assert_eq!(property, property);
    }
    {
        let a = PropertyTableProperty::new();
        let b = PropertyTableProperty::new();
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyTableProperty::new();
        let mut b = PropertyTableProperty::new();
        a.set_name("one");
        b.set_name("two");
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTableProperty::new();
        let mut b = PropertyTableProperty::new();
        a.data_mut().target = 1;
        b.data_mut().target = 2;
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTableProperty::new();
        let mut b = PropertyTableProperty::new();
        a.array_offsets_mut().data.target = 1;
        b.array_offsets_mut().data.target = 2;
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTableProperty::new();
        let mut b = PropertyTableProperty::new();
        a.string_offsets_mut().data.target = 1;
        b.string_offsets_mut().data.target = 2;
        assert_ne!(a, b);
    }
}

#[test]
fn property_table_compare() {
    {
        let table = PropertyTable::new();
        assert_eq!(table, table);
    }
    {
        let a = PropertyTable::new();
        let b = PropertyTable::new();
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyTable::new();
        let mut b = PropertyTable::new();
        a.set_name("one");
        b.set_name("two");
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTable::new();
        let mut b = PropertyTable::new();
        a.set_class("one");
        b.set_class("two");
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTable::new();
        let mut b = PropertyTable::new();
        a.set_count(1);
        b.set_count(2);
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTable::new();
        let mut b = PropertyTable::new();
        a.add_property(PropertyTableProperty::new());
        b.add_property(PropertyTableProperty::new());
        assert_eq!(a, b);
    }
    {
        let mut a = PropertyTable::new();
        let mut b = PropertyTable::new();
        a.add_property(PropertyTableProperty::new());
        b.add_property(PropertyTableProperty::new());
        b.add_property(PropertyTableProperty::new());
        assert_ne!(a, b);
    }
    {
        let mut a = PropertyTable::new();
        let mut b = PropertyTable::new();
        let mut p1 = PropertyTableProperty::new();
        let mut p2 = PropertyTableProperty::new();
        p1.set_name("one");
        p2.set_name("two");
        a.add_property(p1);
        b.add_property(p2);
        assert_ne!(a, b);
    }
}

#[test]
fn property_table_offsets_encode_decode() {
    {
        let sample_offsets = vec![0x5u64, 0x21, 0x7, 0x32, 0xff];
        let encoded_offsets = PropertyTableOffsets::make_from_ints(&sample_offsets);
        assert_eq!(
            encoded_offsets.data.data,
            vec![0x5u8, 0x21, 0x7, 0x32, 0xff]
        );
        assert_eq!(encoded_offsets.type_name, "UINT8");

        let decoded_offsets = encoded_offsets.parse_to_ints();
        assert!(decoded_offsets.is_ok());
        let decoded_offsets = decoded_offsets.into_value();
        assert_eq!(decoded_offsets, sample_offsets);
    }
    {
        let sample_offsets = vec![0x5u64, 0x21, 0xffff];
        let encoded_offsets = PropertyTableOffsets::make_from_ints(&sample_offsets);
        assert_eq!(
            encoded_offsets.data.data,
            vec![0x5u8, 0, 0x21, 0, 0xff, 0xff]
        );
        assert_eq!(encoded_offsets.type_name, "UINT16");

        let decoded_offsets = encoded_offsets.parse_to_ints();
        assert!(decoded_offsets.is_ok());
        let decoded_offsets = decoded_offsets.into_value();
        assert_eq!(decoded_offsets, sample_offsets);
    }
    {
        let sample_offsets = vec![0x5u64, 0x21, 0xffffffff];
        let encoded_offsets = PropertyTableOffsets::make_from_ints(&sample_offsets);
        assert_eq!(
            encoded_offsets.data.data,
            vec![0x5u8, 0, 0, 0, 0x21, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]
        );
        assert_eq!(encoded_offsets.type_name, "UINT32");

        let decoded_offsets = encoded_offsets.parse_to_ints();
        assert!(decoded_offsets.is_ok());
        let decoded_offsets = decoded_offsets.into_value();
        assert_eq!(decoded_offsets, sample_offsets);
    }
    {
        let sample_offsets = vec![0x5u64, 0x21, 0x100000000];
        let encoded_offsets = PropertyTableOffsets::make_from_ints(&sample_offsets);
        assert_eq!(
            encoded_offsets.data.data,
            vec![0x5u8, 0, 0, 0, 0, 0, 0, 0, 0x21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]
        );
        assert_eq!(encoded_offsets.type_name, "UINT64");

        let decoded_offsets = encoded_offsets.parse_to_ints();
        assert!(decoded_offsets.is_ok());
        let decoded_offsets = decoded_offsets.into_value();
        assert_eq!(decoded_offsets, sample_offsets);
    }
    {
        let mut broken_offsets = PropertyTableOffsets::default();
        broken_offsets.data.data = vec![0, 0, 0, 0];
        broken_offsets.type_name = "BROKEN_TYPE".to_string();
        let decoded_offsets = broken_offsets.parse_to_ints();
        assert!(!decoded_offsets.is_ok());
    }
}

#[test]
fn structural_metadata_schema_defaults() {
    let schema = StructuralMetadataSchema::new();
    assert!(schema.is_empty());
    assert_eq!(schema.json.name(), "schema");
    assert_eq!(
        schema.json.object_type(),
        StructuralMetadataObjectType::Object
    );
    assert!(schema.json.objects().is_empty());
    assert!(schema.json.array().is_empty());
    assert!(schema.json.string().is_empty());
    assert_eq!(schema.json.integer(), 0);
    assert!(!schema.json.boolean());
}

#[test]
fn structural_metadata_schema_object_default_constructor() {
    let object = StructuralMetadataObject::new();
    assert!(object.name().is_empty());
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Object);
    assert!(object.objects().is_empty());
    assert!(object.array().is_empty());
    assert!(object.string().is_empty());
    assert_eq!(object.integer(), 0);
    assert!(!object.boolean());
}

#[test]
fn structural_metadata_schema_object_named_constructor() {
    let object = StructuralMetadataObject::with_name("Flexible Demeanour");
    assert_eq!(object.name(), "Flexible Demeanour");
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Object);
    assert!(object.objects().is_empty());
}

#[test]
fn structural_metadata_schema_object_string_constructor() {
    let object = StructuralMetadataObject::with_string("Flexible Demeanour", "GCU");
    assert_eq!(object.name(), "Flexible Demeanour");
    assert_eq!(object.object_type(), StructuralMetadataObjectType::String);
    assert_eq!(object.string(), "GCU");
}

#[test]
fn structural_metadata_schema_object_integer_constructor() {
    let object = StructuralMetadataObject::with_integer("Flexible Demeanour", 12);
    assert_eq!(object.name(), "Flexible Demeanour");
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Integer);
    assert_eq!(object.integer(), 12);
}

#[test]
fn structural_metadata_schema_object_boolean_constructor() {
    let object = StructuralMetadataObject::with_boolean("Flexible Demeanour", true);
    assert_eq!(object.name(), "Flexible Demeanour");
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Boolean);
    assert!(object.boolean());
}

#[test]
fn structural_metadata_schema_object_setters_and_getters() {
    let mut object = StructuralMetadataObject::new();
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Object);

    object
        .set_array()
        .push(StructuralMetadataObject::with_integer("entry", 12));
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Array);
    assert_eq!(object.array().len(), 1);
    assert_eq!(object.array()[0].name(), "entry");
    assert_eq!(object.array()[0].integer(), 12);

    object
        .set_objects()
        .push(StructuralMetadataObject::with_integer("object", 9));
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Object);
    assert_eq!(object.objects().len(), 1);
    assert_eq!(object.objects()[0].name(), "object");
    assert_eq!(object.objects()[0].integer(), 9);

    object.set_string("matter");
    assert_eq!(object.object_type(), StructuralMetadataObjectType::String);
    assert_eq!(object.string(), "matter");

    object.set_integer(5);
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Integer);
    assert_eq!(object.integer(), 5);

    object.set_boolean(true);
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Boolean);
    assert!(object.boolean());
}

#[test]
fn structural_metadata_schema_object_lookup_by_name() {
    let mut object = StructuralMetadataObject::new();
    assert_eq!(object.object_type(), StructuralMetadataObjectType::Object);

    let objects = object.set_objects();
    objects.push(StructuralMetadataObject::with_integer("object1", 1));
    objects.push(StructuralMetadataObject::with_string("object2", "two"));

    let mut object3 = StructuralMetadataObject::with_name("object3");
    object3
        .set_objects()
        .push(StructuralMetadataObject::with_string(
            "child_object",
            "child",
        ));
    objects.push(object3);

    assert!(object.get_object_by_name("child_object").is_none());
    assert_eq!(
        object
            .get_object_by_name("object1")
            .expect("missing object1")
            .integer(),
        1
    );
    assert_eq!(
        object
            .get_object_by_name("object2")
            .expect("missing object2")
            .string(),
        "two"
    );
    let nested = object
        .get_object_by_name("object3")
        .expect("missing object3")
        .get_object_by_name("child_object")
        .expect("missing child_object");
    assert_eq!(nested.string(), "child");
}

#[test]
fn structural_metadata_schema_compare() {
    {
        let schema = StructuralMetadataSchema::new();
        assert_eq!(schema, schema);
    }
    {
        let a = StructuralMetadataSchema::new();
        let b = StructuralMetadataSchema::new();
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadataSchema::new();
        let mut b = StructuralMetadataSchema::new();
        a.json.set_boolean(true);
        b.json.set_boolean(false);
        assert_ne!(a, b);
    }
}

#[test]
fn structural_metadata_schema_object_compare() {
    {
        let object = StructuralMetadataObject::new();
        assert_eq!(object, object);
    }
    {
        let a = StructuralMetadataObject::new();
        let b = StructuralMetadataObject::new();
        assert_eq!(a, b);
    }
    {
        let a = StructuralMetadataObject::with_name("one");
        let b = StructuralMetadataObject::with_name("two");
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_integer(1);
        b.set_string("one");
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_string("one");
        b.set_string("one");
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_string("one");
        b.set_string("two");
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_integer(1);
        b.set_integer(1);
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_integer(1);
        b.set_integer(2);
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_boolean(true);
        b.set_boolean(true);
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_boolean(true);
        b.set_boolean(false);
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_objects()
            .push(StructuralMetadataObject::with_name("one"));
        b.set_objects()
            .push(StructuralMetadataObject::with_name("one"));
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_objects()
            .push(StructuralMetadataObject::with_name("one"));
        b.set_objects()
            .push(StructuralMetadataObject::with_name("two"));
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_objects()
            .push(StructuralMetadataObject::with_name("one"));
        b.set_objects()
            .push(StructuralMetadataObject::with_name("one"));
        b.set_objects()
            .push(StructuralMetadataObject::with_name("two"));
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_array()
            .push(StructuralMetadataObject::with_integer("", 1));
        b.set_array()
            .push(StructuralMetadataObject::with_integer("", 1));
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_array()
            .push(StructuralMetadataObject::with_integer("", 1));
        b.set_array()
            .push(StructuralMetadataObject::with_integer("", 2));
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadataObject::new();
        let mut b = StructuralMetadataObject::new();
        a.set_array()
            .push(StructuralMetadataObject::with_integer("", 1));
        b.set_array()
            .push(StructuralMetadataObject::with_integer("", 1));
        b.set_array()
            .push(StructuralMetadataObject::with_integer("", 2));
        assert_ne!(a, b);
    }
}

#[test]
fn structural_metadata_copy() {
    let mut structural_metadata = StructuralMetadata::new();

    let mut schema = StructuralMetadataSchema::new();
    schema.json.set_string("Culture");
    structural_metadata.set_schema(schema);

    let mut table = PropertyTable::new();
    table.set_name("Just Read The Instructions");
    table.set_class("General Contact Unit");
    table.set_count(456);
    {
        let mut property = PropertyTableProperty::new();
        property.set_name("Determinist");
        table.add_property(property);
    }
    {
        let mut property = PropertyTableProperty::new();
        property.set_name("Revisionist");
        table.add_property(property);
    }
    assert_eq!(structural_metadata.add_property_table(table), 0);

    let mut copy = StructuralMetadata::new();
    copy.copy_from(&structural_metadata);

    assert_eq!(copy.schema().json.string(), "Culture");
    assert_eq!(copy.num_property_tables(), 1);
    assert_eq!(copy.property_table(0).name(), "Just Read The Instructions");
    assert_eq!(copy.property_table(0).class(), "General Contact Unit");
    assert_eq!(copy.property_table(0).count(), 456);
    assert_eq!(copy.property_table(0).num_properties(), 2);
    assert_eq!(copy.property_table(0).property(0).name(), "Determinist");
    assert_eq!(copy.property_table(0).property(1).name(), "Revisionist");
}

#[test]
fn structural_metadata_property_tables() {
    let mut structural_metadata = StructuralMetadata::new();
    {
        let mut table = PropertyTable::new();
        table.set_name("Just Read The Instructions");
        assert_eq!(structural_metadata.add_property_table(table), 0);
    }
    {
        let mut table = PropertyTable::new();
        table.set_name("So Much For Subtlety");
        assert_eq!(structural_metadata.add_property_table(table), 1);
    }
    {
        let mut table = PropertyTable::new();
        table.set_name("Of Course I Still Love You");
        assert_eq!(structural_metadata.add_property_table(table), 2);
    }

    assert_eq!(structural_metadata.num_property_tables(), 3);
    assert_eq!(
        structural_metadata.property_table(0).name(),
        "Just Read The Instructions"
    );
    assert_eq!(
        structural_metadata.property_table(1).name(),
        "So Much For Subtlety"
    );
    assert_eq!(
        structural_metadata.property_table(2).name(),
        "Of Course I Still Love You"
    );

    structural_metadata.remove_property_table(1);
    assert_eq!(structural_metadata.num_property_tables(), 2);
    assert_eq!(
        structural_metadata.property_table(0).name(),
        "Just Read The Instructions"
    );
    assert_eq!(
        structural_metadata.property_table(1).name(),
        "Of Course I Still Love You"
    );

    structural_metadata.remove_property_table(1);
    assert_eq!(structural_metadata.num_property_tables(), 1);
    assert_eq!(
        structural_metadata.property_table(0).name(),
        "Just Read The Instructions"
    );

    structural_metadata.remove_property_table(0);
    assert_eq!(structural_metadata.num_property_tables(), 0);
}

#[test]
fn structural_metadata_compare() {
    {
        let metadata = StructuralMetadata::new();
        assert_eq!(metadata, metadata);
    }
    {
        let a = StructuralMetadata::new();
        let b = StructuralMetadata::new();
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadata::new();
        let mut b = StructuralMetadata::new();
        let mut s1 = StructuralMetadataSchema::new();
        let mut s2 = StructuralMetadataSchema::new();
        s1.json.set_string("one");
        s2.json.set_string("two");
        a.set_schema(s1);
        b.set_schema(s2);
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadata::new();
        let mut b = StructuralMetadata::new();
        a.add_property_table(PropertyTable::new());
        b.add_property_table(PropertyTable::new());
        b.add_property_table(PropertyTable::new());
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadata::new();
        let mut b = StructuralMetadata::new();
        let mut p1 = PropertyTable::new();
        let mut p2 = PropertyTable::new();
        p1.set_name("one");
        p2.set_name("one");
        a.add_property_table(p1);
        b.add_property_table(p2);
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadata::new();
        let mut b = StructuralMetadata::new();
        let mut p1 = PropertyTable::new();
        let mut p2 = PropertyTable::new();
        p1.set_name("one");
        p2.set_name("two");
        a.add_property_table(p1);
        b.add_property_table(p2);
        assert_ne!(a, b);
    }
    {
        let mut a = StructuralMetadata::new();
        let mut b = StructuralMetadata::new();
        let mut p1 = PropertyAttribute::new();
        let mut p2 = PropertyAttribute::new();
        p1.set_name("one");
        p2.set_name("one");
        a.add_property_attribute(p1);
        b.add_property_attribute(p2);
        assert_eq!(a, b);
    }
    {
        let mut a = StructuralMetadata::new();
        let mut b = StructuralMetadata::new();
        let mut p1 = PropertyAttribute::new();
        let mut p2 = PropertyAttribute::new();
        p1.set_name("one");
        p2.set_name("two");
        a.add_property_attribute(p1);
        b.add_property_attribute(p2);
        assert_ne!(a, b);
    }
}
