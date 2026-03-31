use std::mem::size_of;

use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::PointIndex;
use crate::compression::DracoCompressionOptions;
use crate::core::draco_types::DataType;
use crate::metadata::geometry_metadata::{AttributeMetadata, GeometryMetadata};
use crate::metadata::metadata::MetadataString;
use crate::point_cloud::{PointCloud, PointCloudBuilder};

fn as_bytes<T>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * size_of::<T>()) }
}

#[test]
fn point_cloud_builder_individual_no_dedup() {
    let pos_data: [f32; 30] = [
        10.0, 0.0, 1.0, 11.0, 1.0, 2.0, 12.0, 2.0, 8.0, 13.0, 4.0, 7.0, 14.0, 5.0, 6.0, 15.0, 6.0,
        5.0, 16.0, 1.0, 3.0, 17.0, 1.0, 2.0, 11.0, 1.0, 2.0, 10.0, 0.0, 1.0,
    ];
    let intensity_data: [i16; 10] = [100, 200, 500, 700, 400, 400, 400, 100, 100, 100];

    let mut builder = PointCloudBuilder::new();
    builder.start(10);
    let pos_att_id = builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let intensity_att_id =
        builder.add_attribute(GeometryAttributeType::Generic, 1, DataType::Int16);
    for i in 0..10u32 {
        let pos_offset = (i as usize) * 3;
        let pos_slice = &pos_data[pos_offset..pos_offset + 3];
        let intensity_slice = &intensity_data[i as usize..i as usize + 1];
        builder.set_attribute_value_for_point(pos_att_id, PointIndex::from(i), as_bytes(pos_slice));
        builder.set_attribute_value_for_point(
            intensity_att_id,
            PointIndex::from(i),
            as_bytes(intensity_slice),
        );
    }

    builder.set_attribute_name(pos_att_id, "Bob");

    let res = builder
        .finalize(false)
        .expect("PointCloudBuilder returned None");
    assert_eq!(res.num_points(), 10);
    assert_eq!(res.attribute(pos_att_id).unwrap().name(), "Bob");
}

#[test]
fn point_cloud_builder_individual_dedup() {
    let pos_data: [f32; 30] = [
        10.0, 0.0, 1.0, 11.0, 1.0, 2.0, 12.0, 2.0, 8.0, 13.0, 4.0, 7.0, 14.0, 5.0, 6.0, 15.0, 6.0,
        5.0, 16.0, 1.0, 3.0, 17.0, 1.0, 2.0, 11.0, 1.0, 2.0, 10.0, 0.0, 1.0,
    ];
    let intensity_data: [i16; 10] = [100, 200, 500, 700, 400, 400, 400, 100, 100, 100];

    let mut builder = PointCloudBuilder::new();
    builder.start(10);
    let pos_att_id = builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let intensity_att_id =
        builder.add_attribute(GeometryAttributeType::Generic, 1, DataType::Int16);
    for i in 0..10u32 {
        let pos_offset = (i as usize) * 3;
        let pos_slice = &pos_data[pos_offset..pos_offset + 3];
        let intensity_slice = &intensity_data[i as usize..i as usize + 1];
        builder.set_attribute_value_for_point(pos_att_id, PointIndex::from(i), as_bytes(pos_slice));
        builder.set_attribute_value_for_point(
            intensity_att_id,
            PointIndex::from(i),
            as_bytes(intensity_slice),
        );
    }

    let res = builder
        .finalize(true)
        .expect("PointCloudBuilder returned None");
    assert_eq!(res.num_points(), 9);
}

#[test]
fn point_cloud_builder_batch() {
    let pos_data: [f32; 30] = [
        10.0, 0.0, 1.0, 11.0, 1.0, 2.0, 12.0, 2.0, 8.0, 13.0, 4.0, 7.0, 14.0, 5.0, 6.0, 15.0, 6.0,
        5.0, 16.0, 1.0, 3.0, 17.0, 1.0, 2.0, 11.0, 1.0, 2.0, 10.0, 0.0, 1.0,
    ];
    let intensity_data: [i16; 10] = [100, 200, 500, 700, 400, 400, 400, 100, 100, 100];

    let mut builder = PointCloudBuilder::new();
    builder.start(10);
    let pos_att_id = builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let intensity_att_id =
        builder.add_attribute(GeometryAttributeType::Generic, 1, DataType::Int16);
    builder.set_attribute_values_for_all_points(pos_att_id, as_bytes(&pos_data), 0);
    builder.set_attribute_values_for_all_points(intensity_att_id, as_bytes(&intensity_data), 0);

    let res = builder
        .finalize(false)
        .expect("PointCloudBuilder returned None");
    assert_eq!(res.num_points(), 10);

    let pos_att = res
        .attribute(pos_att_id)
        .expect("Missing position attribute");
    let intensity_att = res
        .attribute(intensity_att_id)
        .expect("Missing intensity attribute");
    for i in 0..10u32 {
        let pos_index = pos_att.mapped_index(PointIndex::from(i));
        let pos_val: [f32; 3] = pos_att.get_value_array(pos_index);
        assert_eq!(pos_val[0], pos_data[(i as usize) * 3]);
        assert_eq!(pos_val[1], pos_data[(i as usize) * 3 + 1]);
        assert_eq!(pos_val[2], pos_data[(i as usize) * 3 + 2]);

        let intensity_index = intensity_att.mapped_index(PointIndex::from(i));
        let intensity_val: [i16; 1] = intensity_att.get_value_array(intensity_index);
        assert_eq!(intensity_val[0], intensity_data[i as usize]);
    }
}

#[test]
fn point_cloud_builder_multi_use() {
    let pos_data: [f32; 30] = [
        10.0, 0.0, 1.0, 11.0, 1.0, 2.0, 12.0, 2.0, 8.0, 13.0, 4.0, 7.0, 14.0, 5.0, 6.0, 15.0, 6.0,
        5.0, 16.0, 1.0, 3.0, 17.0, 1.0, 2.0, 11.0, 1.0, 2.0, 10.0, 0.0, 1.0,
    ];
    let intensity_data: [i16; 10] = [100, 200, 500, 700, 400, 400, 400, 100, 100, 100];

    let mut builder = PointCloudBuilder::new();
    {
        builder.start(10);
        let pos_att_id =
            builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
        let intensity_att_id =
            builder.add_attribute(GeometryAttributeType::Generic, 1, DataType::Int16);
        builder.set_attribute_values_for_all_points(pos_att_id, as_bytes(&pos_data), 0);
        builder.set_attribute_values_for_all_points(intensity_att_id, as_bytes(&intensity_data), 0);
        let res = builder
            .finalize(false)
            .expect("PointCloudBuilder returned None");
        assert_eq!(res.num_points(), 10);
    }

    {
        builder.start(4);
        let pos_att_id =
            builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
        let intensity_att_id =
            builder.add_attribute(GeometryAttributeType::Generic, 1, DataType::Int16);
        let offset = 5usize;
        builder.set_attribute_values_for_all_points(
            pos_att_id,
            as_bytes(&pos_data[offset * 3..offset * 3 + 12]),
            0,
        );
        builder.set_attribute_values_for_all_points(
            intensity_att_id,
            as_bytes(&intensity_data[offset..offset + 4]),
            0,
        );
        let res = builder
            .finalize(false)
            .expect("PointCloudBuilder returned None");
        assert_eq!(res.num_points(), 4);

        let pos_att = res
            .attribute(pos_att_id)
            .expect("Missing position attribute");
        let intensity_att = res
            .attribute(intensity_att_id)
            .expect("Missing intensity attribute");
        for i in 0..4u32 {
            let pos_index = pos_att.mapped_index(PointIndex::from(i));
            let pos_val: [f32; 3] = pos_att.get_value_array(pos_index);
            let src_index = i as usize + offset;
            assert_eq!(pos_val[0], pos_data[src_index * 3]);
            assert_eq!(pos_val[1], pos_data[src_index * 3 + 1]);
            assert_eq!(pos_val[2], pos_data[src_index * 3 + 2]);

            let intensity_index = intensity_att.mapped_index(PointIndex::from(i));
            let intensity_val: [i16; 1] = intensity_att.get_value_array(intensity_index);
            assert_eq!(intensity_val[0], intensity_data[src_index]);
        }
    }
}

#[test]
fn point_cloud_builder_propagates_attribute_unique_ids() {
    let pos_data: [f32; 30] = [
        10.0, 0.0, 1.0, 11.0, 1.0, 2.0, 12.0, 2.0, 8.0, 13.0, 4.0, 7.0, 14.0, 5.0, 6.0, 15.0, 6.0,
        5.0, 16.0, 1.0, 3.0, 17.0, 1.0, 2.0, 11.0, 1.0, 2.0, 10.0, 0.0, 1.0,
    ];
    let intensity_data: [i16; 10] = [100, 200, 500, 700, 400, 400, 400, 100, 100, 100];

    let mut builder = PointCloudBuilder::new();
    builder.start(10);
    let pos_att_id = builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let intensity_att_id =
        builder.add_attribute(GeometryAttributeType::Generic, 1, DataType::Int16);
    for i in 0..10u32 {
        let pos_offset = (i as usize) * 3;
        let pos_slice = &pos_data[pos_offset..pos_offset + 3];
        let intensity_slice = &intensity_data[i as usize..i as usize + 1];
        builder.set_attribute_value_for_point(pos_att_id, PointIndex::from(i), as_bytes(pos_slice));
        builder.set_attribute_value_for_point(
            intensity_att_id,
            PointIndex::from(i),
            as_bytes(intensity_slice),
        );
    }
    builder.set_attribute_unique_id(pos_att_id, 1234);
    let res = builder
        .finalize(false)
        .expect("PointCloudBuilder returned None");
    let by_unique = res
        .get_attribute_by_unique_id(1234)
        .expect("Missing attribute by unique id");
    let by_index = res.attribute(pos_att_id).expect("Missing attribute by id");
    assert_eq!(by_unique.unique_id(), by_index.unique_id());
}

#[test]
fn point_cloud_attribute_deletion() {
    let mut pc = PointCloud::new();

    let mut pos_att = GeometryAttribute::new();
    pos_att.init(
        GeometryAttributeType::Position,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );
    let mut norm_att = GeometryAttribute::new();
    norm_att.init(
        GeometryAttributeType::Normal,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );
    let mut gen_att = GeometryAttribute::new();
    gen_att.init(
        GeometryAttributeType::Generic,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );

    pc.add_attribute_from_geometry(&pos_att, false, 0);
    pc.add_attribute_from_geometry(&gen_att, false, 0);
    pc.add_attribute_from_geometry(&norm_att, false, 0);
    pc.add_attribute_from_geometry(&gen_att, false, 0);
    pc.add_attribute_from_geometry(&norm_att, false, 0);

    assert_eq!(pc.num_attributes(), 5);
    assert_eq!(
        pc.attribute(0).unwrap().attribute_type(),
        GeometryAttributeType::Position
    );
    assert_eq!(
        pc.attribute(3).unwrap().attribute_type(),
        GeometryAttributeType::Generic
    );

    pc.delete_attribute(1);
    assert_eq!(pc.num_attributes(), 4);
    assert_eq!(
        pc.attribute(1).unwrap().attribute_type(),
        GeometryAttributeType::Normal
    );
    assert_eq!(pc.num_named_attributes(GeometryAttributeType::Normal), 2);
    assert_eq!(
        pc.get_named_attribute_id_by_index(GeometryAttributeType::Normal, 1),
        3
    );

    pc.delete_attribute(1);
    assert_eq!(pc.num_attributes(), 3);
    assert_eq!(
        pc.attribute(1).unwrap().attribute_type(),
        GeometryAttributeType::Generic
    );
    assert_eq!(pc.num_named_attributes(GeometryAttributeType::Normal), 1);
    assert_eq!(
        pc.get_named_attribute_id_by_index(GeometryAttributeType::Normal, 0),
        2
    );
}

#[test]
fn point_cloud_with_metadata() {
    let mut pc = PointCloud::new();

    let mut pos_att = GeometryAttribute::new();
    pos_att.init(
        GeometryAttributeType::Position,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );
    let pos_att_id = pc.add_attribute_from_geometry(&pos_att, false, 0) as u32;
    assert_eq!(pos_att_id, 0);
    let mut pos_metadata = AttributeMetadata::new();
    pos_metadata.add_entry_string("name", "position");
    pc.add_attribute_metadata(pos_att_id as i32, pos_metadata);
    assert!(pc.get_metadata().is_some());

    let mut material_att = GeometryAttribute::new();
    material_att.init(
        GeometryAttributeType::Generic,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );
    let material_att_id = pc.add_attribute_from_geometry(&material_att, false, 0) as u32;
    assert_eq!(material_att_id, 1);
    let mut material_metadata = AttributeMetadata::new();
    material_metadata.add_entry_string("name", "material");
    pc.add_attribute_metadata(material_att_id as i32, material_metadata);

    let requested_pos_metadata = pc
        .get_attribute_metadata_by_string_entry("name", "position")
        .expect("Missing position metadata");
    let requested_mat_metadata = pc
        .get_attribute_metadata_by_string_entry("name", "material")
        .expect("Missing material metadata");

    assert_eq!(
        pc.get_attribute_id_by_unique_id(requested_pos_metadata.att_unique_id()),
        0
    );
    assert_eq!(
        pc.get_attribute_id_by_unique_id(requested_mat_metadata.att_unique_id()),
        1
    );

    pc.delete_attribute(pos_att_id as i32);
    assert!(pc
        .get_attribute_metadata_by_string_entry("name", "position")
        .is_none());

    let requested_mat_metadata = pc
        .get_attribute_metadata_by_string_entry("name", "material")
        .expect("Missing material metadata after deletion");
    assert_eq!(requested_mat_metadata.att_unique_id(), 1);
    assert_eq!(
        pc.get_attribute_id_by_unique_id(requested_mat_metadata.att_unique_id()),
        0
    );
    assert!(pc.get_attribute_metadata_by_attribute_id(0).is_some());
}

#[test]
fn point_cloud_copy_transcoder() {
    let mut builder = PointCloudBuilder::new();
    builder.start(2);
    let pos_att_id = builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let color_att_id = builder.add_attribute(GeometryAttributeType::Color, 3, DataType::Uint8);

    let pos0 = [0.0f32, 0.0f32, 0.0f32];
    let pos1 = [1.0f32, 0.0f32, 0.0f32];
    builder.set_attribute_value_for_point(pos_att_id, PointIndex::from(0u32), as_bytes(&pos0));
    builder.set_attribute_value_for_point(pos_att_id, PointIndex::from(1u32), as_bytes(&pos1));

    let col0 = [255u8, 0u8, 0u8];
    let col1 = [0u8, 255u8, 0u8];
    builder.set_attribute_value_for_point(color_att_id, PointIndex::from(0u32), &col0);
    builder.set_attribute_value_for_point(color_att_id, PointIndex::from(1u32), &col1);

    let mut pc = builder
        .finalize(false)
        .expect("PointCloudBuilder returned None");

    let mut metadata = GeometryMetadata::new();
    metadata.add_entry_int("speed", 1050);
    metadata.add_entry_string("code", "YT-1300f");
    pc.add_metadata(metadata);

    let mut att_metadata = AttributeMetadata::new();
    att_metadata.add_entry_int("attribute_test", 3);
    pc.add_attribute_metadata(0, att_metadata);

    let mut pc_copy = PointCloud::new();
    pc_copy.copy(&pc);

    assert_eq!(pc.num_points(), pc_copy.num_points());
    assert_eq!(pc.num_attributes(), pc_copy.num_attributes());
    for i in 0..pc.num_attributes() {
        assert_eq!(
            pc.attribute(i).unwrap().attribute_type(),
            pc_copy.attribute(i).unwrap().attribute_type()
        );
    }

    let metadata_copy = pc_copy.get_metadata().expect("Missing metadata");
    let mut speed = 0i32;
    let mut code = MetadataString::default();
    assert!(metadata_copy.get_entry_int("speed", &mut speed));
    assert!(metadata_copy.get_entry_string("code", &mut code));
    assert_eq!(speed, 1050);
    assert_eq!(code.as_bytes(), b"YT-1300f");

    let att_metadata_copy = metadata_copy
        .get_attribute_metadata_by_unique_id(0)
        .expect("Missing attribute metadata");
    let mut att_test = 0i32;
    assert!(att_metadata_copy.get_entry_int("attribute_test", &mut att_test));
    assert_eq!(att_test, 3);
}

#[test]
fn point_cloud_compression_settings() {
    let mut pc = PointCloud::new();

    assert!(!pc.is_compression_enabled());
    let default_options = DracoCompressionOptions::default();
    assert_eq!(pc.compression_options(), &default_options);

    let mut compression_options = DracoCompressionOptions::default();
    compression_options.quantization_bits_normal = 12;
    pc.set_compression_options(compression_options.clone());
    assert_eq!(pc.compression_options(), &compression_options);
    assert!(!pc.is_compression_enabled());

    pc.set_compression_enabled(true);
    assert!(pc.is_compression_enabled());

    let options_mut = pc.compression_options_mut();
    options_mut.compression_level += 1;
    options_mut.compression_level -= 1;

    let mut pc_copy = PointCloud::new();
    pc_copy.copy(&pc);
    assert!(pc_copy.is_compression_enabled());
    assert_eq!(pc_copy.compression_options(), &compression_options);
}

#[test]
fn point_cloud_get_named_attribute_by_name() {
    let mut pc = PointCloud::new();
    let mut pos_att = GeometryAttribute::new();
    let mut gen_att0 = GeometryAttribute::new();
    let mut gen_att1 = GeometryAttribute::new();

    pos_att.init(
        GeometryAttributeType::Position,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );
    gen_att0.init(
        GeometryAttributeType::Generic,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );
    gen_att1.init(
        GeometryAttributeType::Generic,
        None,
        3,
        DataType::Float32,
        false,
        12,
        0,
    );

    pos_att.set_name("Zero");
    gen_att0.set_name("Zero");
    gen_att1.set_name("One");

    pc.add_attribute_from_geometry(&pos_att, false, 0);
    pc.add_attribute_from_geometry(&gen_att0, false, 0);
    pc.add_attribute_from_geometry(&gen_att1, false, 0);

    assert_eq!(
        pc.attribute(0).unwrap().attribute_type(),
        GeometryAttributeType::Position
    );
    assert_eq!(
        pc.attribute(1).unwrap().attribute_type(),
        GeometryAttributeType::Generic
    );
    assert_eq!(
        pc.attribute(2).unwrap().attribute_type(),
        GeometryAttributeType::Generic
    );
    assert_eq!(pc.attribute(0).unwrap().name(), "Zero");
    assert_eq!(pc.attribute(1).unwrap().name(), "Zero");
    assert_eq!(pc.attribute(2).unwrap().name(), "One");

    assert_eq!(
        pc.get_named_attribute_by_name(GeometryAttributeType::Position, "Zero")
            .unwrap()
            .unique_id(),
        pc.attribute(0).unwrap().unique_id()
    );
    assert_eq!(
        pc.get_named_attribute_by_name(GeometryAttributeType::Generic, "Zero")
            .unwrap()
            .unique_id(),
        pc.attribute(1).unwrap().unique_id()
    );
    assert_eq!(
        pc.get_named_attribute_by_name(GeometryAttributeType::Generic, "One")
            .unwrap()
            .unique_id(),
        pc.attribute(2).unwrap().unique_id()
    );
}
