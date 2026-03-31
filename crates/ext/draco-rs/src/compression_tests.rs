//! Compression test ports from `_ref/draco/src/draco/compression/*_test.cc`.
//!
//! What: Exercises encode/decode APIs, quantization, and compression options.
//! Why: Ensures bitstream behavior matches the reference C++ implementation.
//! Where used: `draco-rs` test suite with `crates/draco-rs/test` assets.

use crate::attributes::attribute_quantization_transform::AttributeQuantizationTransform;
use crate::attributes::attribute_transform::AttributeTransform;
use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use crate::attributes::point_attribute::PointAttribute;
use crate::compression::DracoCompressionOptions;
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::DataType;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::io::test_utils::{
    get_test_file_full_path, read_mesh_from_test_file, read_point_cloud_from_test_file,
};
use crate::mesh::TriangleSoupMeshBuilder;
use crate::point_cloud::PointCloudBuilder;
use draco_bitstream::compression::config::compression_shared::{
    MeshEncoderMethod, PointCloudEncodingMethod,
};
use draco_bitstream::compression::decode::Decoder;
use draco_bitstream::compression::encode::Encoder;
use draco_bitstream::compression::expert_encode::ExpertEncoder;
use std::fs;

/// Asserts that a Status-like value is OK (test-only helper).
macro_rules! draco_assert_ok {
    ($expression:expr) => {{
        let _local_status = $expression;
        assert!(
            _local_status.is_ok(),
            "{}",
            _local_status.error_msg_string()
        );
    }};
}

fn read_test_file_bytes(file_name: &str) -> Vec<u8> {
    let path = get_test_file_full_path(file_name);
    let data = fs::read(&path).unwrap_or_else(|_| panic!("Failed to read {}", path));
    assert!(!data.is_empty(), "{} is empty", file_name);
    data
}

fn as_bytes<T: Copy>(values: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            values.as_ptr() as *const u8,
            std::mem::size_of::<T>() * values.len(),
        )
    }
}

fn create_test_mesh() -> crate::mesh::Mesh {
    let mut mesh_builder = TriangleSoupMeshBuilder::new();
    mesh_builder.start(1);

    let pos_att_id =
        mesh_builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let tex_att_id_0 =
        mesh_builder.add_attribute(GeometryAttributeType::TexCoord, 2, DataType::Float32);
    let tex_att_id_1 =
        mesh_builder.add_attribute(GeometryAttributeType::TexCoord, 2, DataType::Float32);

    mesh_builder.set_attribute_values_for_face(
        pos_att_id,
        crate::attributes::geometry_indices::FaceIndex::from(0u32),
        &[0.0_f32, 0.0_f32, 0.0_f32],
        &[1.0_f32, 0.0_f32, 0.0_f32],
        &[1.0_f32, 1.0_f32, 0.0_f32],
    );
    mesh_builder.set_attribute_values_for_face(
        tex_att_id_0,
        crate::attributes::geometry_indices::FaceIndex::from(0u32),
        &[0.0_f32, 0.0_f32],
        &[1.0_f32, 0.0_f32],
        &[1.0_f32, 1.0_f32],
    );
    mesh_builder.set_attribute_values_for_face(
        tex_att_id_1,
        crate::attributes::geometry_indices::FaceIndex::from(0u32),
        &[0.0_f32, 0.0_f32],
        &[1.0_f32, 0.0_f32],
        &[1.0_f32, 1.0_f32],
    );

    mesh_builder.finalize().expect("Failed to finalize mesh")
}

fn create_test_point_cloud() -> crate::point_cloud::PointCloud {
    let mut pc_builder = PointCloudBuilder::new();

    const NUM_POINTS: u32 = 100;
    const NUM_GEN_ATT_COORDS_0: usize = 4;
    const NUM_GEN_ATT_COORDS_1: usize = 6;
    pc_builder.start(NUM_POINTS);

    let pos_att_id =
        pc_builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let gen_att_id_0 = pc_builder.add_attribute(
        GeometryAttributeType::Generic,
        NUM_GEN_ATT_COORDS_0 as i8,
        DataType::Uint32,
    );
    let gen_att_id_1 = pc_builder.add_attribute(
        GeometryAttributeType::Generic,
        NUM_GEN_ATT_COORDS_1 as i8,
        DataType::Uint8,
    );

    for i in 0..NUM_POINTS {
        let pos_coord = i as f32;
        let pos = [pos_coord, -pos_coord, pos_coord];
        pc_builder.set_attribute_value_for_point(pos_att_id, PointIndex::from(i), as_bytes(&pos));

        let mut gen_att_data_0 = [0u32; NUM_GEN_ATT_COORDS_0];
        for entry in gen_att_data_0.iter_mut() {
            *entry = i as u32;
        }
        pc_builder.set_attribute_value_for_point(
            gen_att_id_0,
            PointIndex::from(i),
            as_bytes(&gen_att_data_0),
        );

        let mut gen_att_data_1 = [0u8; NUM_GEN_ATT_COORDS_1];
        for entry in gen_att_data_1.iter_mut() {
            *entry = (-(i as i32)) as u8;
        }
        pc_builder.set_attribute_value_for_point(
            gen_att_id_1,
            PointIndex::from(i),
            &gen_att_data_1,
        );
    }

    pc_builder
        .finalize(false)
        .expect("Failed to finalize point cloud")
}

fn get_quantization_bits_from_attribute(att: Option<&PointAttribute>) -> i32 {
    let Some(att) = att else {
        return -1;
    };
    let mut transform = AttributeQuantizationTransform::new();
    if !transform.init_from_attribute(att) {
        return -1;
    }
    transform.quantization_bits()
}

fn verify_num_quantization_bits(
    buffer: &EncoderBuffer,
    pos_quantization: i32,
    tex_coord_0_quantization: i32,
    tex_coord_1_quantization: i32,
) {
    let mut decoder = Decoder::new();
    decoder.set_skip_attribute_transform(GeometryAttributeType::Position);
    decoder.set_skip_attribute_transform(GeometryAttributeType::TexCoord);

    let mut in_buffer = DecoderBuffer::new();
    in_buffer.init(buffer.data());
    let mesh = decoder.decode_mesh_from_buffer(&mut in_buffer).into_value();

    assert_eq!(
        get_quantization_bits_from_attribute(mesh.attribute(0)),
        pos_quantization
    );
    assert_eq!(
        get_quantization_bits_from_attribute(mesh.attribute(1)),
        tex_coord_0_quantization
    );
    assert_eq!(
        get_quantization_bits_from_attribute(mesh.attribute(2)),
        tex_coord_1_quantization
    );
}

fn test_skip_attribute_transform_on_point_cloud_with_color(file_name: &str) {
    let data = read_test_file_bytes(file_name);

    let mut buffer = DecoderBuffer::new();
    buffer.init(&data);

    let mut decoder = Decoder::new();
    decoder.set_skip_attribute_transform(GeometryAttributeType::Position);

    let decoded_pc = decoder
        .decode_point_cloud_from_buffer(&mut buffer)
        .into_value();
    let pc = decoded_pc.as_point_cloud();

    let pos_att = pc
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Missing position attribute");
    assert!(
        matches!(pos_att.data_type(), DataType::Int32 | DataType::Uint32),
        "Unexpected position data type: {:?}",
        pos_att.data_type()
    );
    assert!(pos_att.get_attribute_transform_data().is_some());

    let clr_att = pc
        .get_named_attribute(GeometryAttributeType::Color)
        .expect("Missing color attribute");
    assert_eq!(clr_att.data_type(), DataType::Uint8);

    let mut buffer_2 = DecoderBuffer::new();
    buffer_2.init(&data);
    let mut decoder_2 = Decoder::new();
    let decoded_pc_2 = decoder_2
        .decode_point_cloud_from_buffer(&mut buffer_2)
        .into_value();
    let pc_2 = decoded_pc_2.as_point_cloud();
    let clr_att_2 = pc_2
        .get_named_attribute(GeometryAttributeType::Color)
        .expect("Missing color attribute");

    for i in 0..pc_2.num_points() {
        let pi = PointIndex::from(i);
        let mut color_a = vec![0u8; clr_att.byte_stride() as usize];
        let mut color_b = vec![0u8; clr_att_2.byte_stride() as usize];
        clr_att.get_value_bytes(clr_att.mapped_index(pi), &mut color_a);
        clr_att_2.get_value_bytes(clr_att_2.mapped_index(pi), &mut color_b);
        assert_eq!(color_a, color_b);
    }
}

#[test]
fn test_skip_attribute_transform_legacy() {
    let data = read_test_file_bytes("test_nm_quant.0.9.0.drc");

    let mut buffer = DecoderBuffer::new();
    buffer.init(&data);

    let mut decoder = Decoder::new();
    decoder.set_skip_attribute_transform(GeometryAttributeType::Position);

    let decoded_pc = decoder
        .decode_point_cloud_from_buffer(&mut buffer)
        .into_value();
    let pc = decoded_pc.as_point_cloud();

    let pos_att = pc
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Missing position attribute");
    assert_eq!(pos_att.data_type(), DataType::Int32);
    assert!(pos_att.get_attribute_transform_data().is_some());

    let norm_att = pc
        .get_named_attribute(GeometryAttributeType::Normal)
        .expect("Missing normal attribute");
    assert_eq!(norm_att.data_type(), DataType::Float32);
    assert!(norm_att.get_attribute_transform_data().is_none());
}

#[test]
fn test_skip_attribute_transform_on_point_cloud() {
    test_skip_attribute_transform_on_point_cloud_with_color("pc_color.drc");
    test_skip_attribute_transform_on_point_cloud_with_color("pc_kd_color.drc");
}

#[test]
fn test_skip_attribute_transform_with_no_quantization() {
    let data = read_test_file_bytes("point_cloud_no_qp.drc");

    let mut buffer = DecoderBuffer::new();
    buffer.init(&data);

    let mut decoder = Decoder::new();
    decoder.set_skip_attribute_transform(GeometryAttributeType::Position);

    let decoded_pc = decoder
        .decode_point_cloud_from_buffer(&mut buffer)
        .into_value();
    let pc = decoded_pc.as_point_cloud();

    let pos_att = pc
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Missing position attribute");
    assert_eq!(pos_att.data_type(), DataType::Float32);
    assert!(pos_att.get_attribute_transform_data().is_none());
}

#[test]
fn test_skip_attribute_transform_unique_id() {
    let status_or = read_mesh_from_test_file("cube_att.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut src_mesh = status_or.into_value();

    const POS_UNIQUE_ID: u32 = 7;
    const NORM_UNIQUE_ID: u32 = 42;

    let pos_att_id = src_mesh.get_named_attribute_id(GeometryAttributeType::Position);
    src_mesh
        .attribute_mut(pos_att_id)
        .expect("Missing position attribute")
        .set_unique_id(POS_UNIQUE_ID);
    let norm_att_id = src_mesh.get_named_attribute_id(GeometryAttributeType::Normal);
    src_mesh
        .attribute_mut(norm_att_id)
        .expect("Missing normal attribute")
        .set_unique_id(NORM_UNIQUE_ID);

    let mut encoder_buffer = EncoderBuffer::new();
    let mut encoder = Encoder::new();
    encoder.set_attribute_quantization(GeometryAttributeType::Position, 10);
    encoder.set_attribute_quantization(GeometryAttributeType::Normal, 11);
    draco_assert_ok!(encoder.encode_mesh_to_buffer(&src_mesh, &mut encoder_buffer));

    let mut buffer = DecoderBuffer::new();
    buffer.init(encoder_buffer.data());

    let mut decoder_no_skip = Decoder::new();
    let mesh_no_skip = decoder_no_skip
        .decode_mesh_from_buffer(&mut buffer)
        .into_value();

    let mut decoder_skip = Decoder::new();
    decoder_skip.set_skip_attribute_transform(GeometryAttributeType::Position);
    decoder_skip.set_skip_attribute_transform(GeometryAttributeType::Normal);

    buffer.init(encoder_buffer.data());
    let mesh_skip = decoder_skip
        .decode_mesh_from_buffer(&mut buffer)
        .into_value();

    let pos_att_no_skip = mesh_no_skip
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Missing position attribute");
    assert_eq!(pos_att_no_skip.data_type(), DataType::Float32);

    let pos_att_skip = mesh_skip
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Missing position attribute");
    assert_eq!(pos_att_skip.data_type(), DataType::Int32);

    let norm_att_no_skip = mesh_no_skip
        .get_named_attribute(GeometryAttributeType::Normal)
        .expect("Missing normal attribute");
    assert_eq!(norm_att_no_skip.data_type(), DataType::Float32);

    let norm_att_skip = mesh_skip
        .get_named_attribute(GeometryAttributeType::Normal)
        .expect("Missing normal attribute");
    assert_eq!(norm_att_skip.data_type(), DataType::Int32);

    assert_eq!(pos_att_skip.unique_id(), pos_att_no_skip.unique_id());
    assert_eq!(norm_att_skip.unique_id(), norm_att_no_skip.unique_id());
}

#[test]
fn test_expert_encoder_quantization() {
    let mesh = create_test_mesh();
    let mut encoder = ExpertEncoder::new_mesh(&mesh);
    encoder.set_attribute_quantization(0, 16);
    encoder.set_attribute_quantization(1, 15);
    encoder.set_attribute_quantization(2, 14);

    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_to_buffer(&mut buffer));
    verify_num_quantization_bits(&buffer, 16, 15, 14);
}

#[test]
fn test_encoder_quantization() {
    let mesh = create_test_mesh();
    let mut encoder = Encoder::new();
    encoder.set_attribute_quantization(GeometryAttributeType::Position, 16);
    encoder.set_attribute_quantization(GeometryAttributeType::TexCoord, 15);

    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_mesh_to_buffer(&mesh, &mut buffer));
    verify_num_quantization_bits(&buffer, 16, 15, 15);
}

#[test]
fn test_lines_obj() {
    let status_or = read_mesh_from_test_file("test_lines.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mesh = status_or.into_value();
    assert_eq!(mesh.num_faces(), 0);

    let status_or_pc = read_point_cloud_from_test_file("test_lines.obj");
    assert!(
        status_or_pc.is_ok(),
        "{}",
        status_or_pc.status().error_msg_string()
    );
    let decoded = status_or_pc.into_value();
    let pc = decoded.as_ref();

    let mut encoder = Encoder::new();
    encoder.set_attribute_quantization(GeometryAttributeType::Position, 16);

    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_point_cloud_to_buffer(pc, &mut buffer));
}

#[test]
fn test_quantized_infinity() {
    let status_or_pc = read_point_cloud_from_test_file("float_inf_point_cloud.ply");
    assert!(
        status_or_pc.is_ok(),
        "{}",
        status_or_pc.status().error_msg_string()
    );
    let decoded = status_or_pc.into_value();
    let pc = decoded.as_ref();

    {
        let mut encoder = Encoder::new();
        encoder.set_encoding_method(PointCloudEncodingMethod::PointCloudSequentialEncoding as i32);
        encoder.set_attribute_quantization(GeometryAttributeType::Position, 11);

        let mut buffer = EncoderBuffer::new();
        assert!(!encoder
            .encode_point_cloud_to_buffer(pc, &mut buffer)
            .is_ok());
    }

    {
        let mut encoder = Encoder::new();
        encoder.set_encoding_method(PointCloudEncodingMethod::PointCloudKdTreeEncoding as i32);
        encoder.set_attribute_quantization(GeometryAttributeType::Position, 11);

        let mut buffer = EncoderBuffer::new();
        assert!(!encoder
            .encode_point_cloud_to_buffer(pc, &mut buffer)
            .is_ok());
    }
}

#[test]
fn test_unquantized_infinity() {
    let status_or_pc = read_point_cloud_from_test_file("float_inf_point_cloud.ply");
    assert!(
        status_or_pc.is_ok(),
        "{}",
        status_or_pc.status().error_msg_string()
    );
    let decoded = status_or_pc.into_value();
    let pc = decoded.as_ref();

    let mut encoder = Encoder::new();
    encoder.set_encoding_method(PointCloudEncodingMethod::PointCloudSequentialEncoding as i32);

    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_point_cloud_to_buffer(pc, &mut buffer));
}

#[test]
fn test_quantized_and_unquantized_attributes() {
    let status_or_pc = read_point_cloud_from_test_file("float_two_att_point_cloud.ply");
    assert!(
        status_or_pc.is_ok(),
        "{}",
        status_or_pc.status().error_msg_string()
    );
    let decoded = status_or_pc.into_value();
    let pc = decoded.as_ref();

    let mut encoder = Encoder::new();
    encoder.set_attribute_quantization(GeometryAttributeType::Position, 11);
    encoder.set_attribute_quantization(GeometryAttributeType::Normal, 0);

    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_point_cloud_to_buffer(pc, &mut buffer));
}

#[test]
fn test_kd_tree_encoding() {
    let pc = create_test_point_cloud();

    let mut buffer = EncoderBuffer::new();
    let mut encoder = Encoder::new();
    encoder.set_encoding_method(PointCloudEncodingMethod::PointCloudKdTreeEncoding as i32);
    assert!(!encoder
        .encode_point_cloud_to_buffer(&pc, &mut buffer)
        .is_ok());

    encoder.set_attribute_quantization(GeometryAttributeType::Position, 16);
    draco_assert_ok!(encoder.encode_point_cloud_to_buffer(&pc, &mut buffer));
}

fn test_number_of_encoded_entries(file_name: &str, encoding_method: i32) {
    let is_mesh = encoding_method == MeshEncoderMethod::MeshEdgebreakerEncoding as i32
        || encoding_method == MeshEncoderMethod::MeshSequentialEncoding as i32;

    let mut mesh_opt: Option<Box<crate::mesh::Mesh>> = None;
    let mut decoded_pc: Option<Box<crate::point_cloud::PointCloud>> = None;

    if is_mesh {
        let status_or = read_mesh_from_test_file(file_name);
        assert!(
            status_or.is_ok(),
            "{}",
            status_or.status().error_msg_string()
        );
        let mut mesh = status_or.into_value();
        if !mesh.deduplicate_attribute_values() {
            return;
        }
        mesh.deduplicate_point_ids();
        mesh_opt = Some(mesh);
    } else {
        let status_or = read_point_cloud_from_test_file(file_name);
        assert!(
            status_or.is_ok(),
            "{}",
            status_or.status().error_msg_string()
        );
        decoded_pc = Some(status_or.into_value());
    }

    let mut encoder = Encoder::new();
    encoder.set_attribute_quantization(GeometryAttributeType::Position, 14);
    encoder.set_attribute_quantization(GeometryAttributeType::TexCoord, 12);
    encoder.set_attribute_quantization(GeometryAttributeType::Normal, 10);
    encoder.set_encoding_method(encoding_method);
    encoder.set_track_encoded_properties(true);

    let mut buffer = EncoderBuffer::new();
    if let Some(mesh) = mesh_opt.as_ref() {
        draco_assert_ok!(encoder.encode_mesh_to_buffer(mesh, &mut buffer));
    } else if let Some(decoded) = decoded_pc.as_ref() {
        draco_assert_ok!(encoder.encode_point_cloud_to_buffer(decoded.as_ref(), &mut buffer));
    }

    let mut decoder_buffer = DecoderBuffer::new();
    decoder_buffer.init(buffer.data());
    let mut decoder = Decoder::new();

    if let Some(_mesh) = mesh_opt.as_ref() {
        let decoded_mesh = decoder
            .decode_mesh_from_buffer(&mut decoder_buffer)
            .into_value();
        assert_eq!(
            decoded_mesh.num_points(),
            encoder.num_encoded_points() as u32
        );
        assert_eq!(decoded_mesh.num_faces(), encoder.num_encoded_faces() as u32);
    } else {
        let decoded_cloud = decoder
            .decode_point_cloud_from_buffer(&mut decoder_buffer)
            .into_value();
        assert_eq!(
            decoded_cloud.as_point_cloud().num_points(),
            encoder.num_encoded_points() as u32
        );
    }
}

#[test]
fn test_tracking_of_number_of_encoded_entries() {
    test_number_of_encoded_entries(
        "deg_faces.obj",
        MeshEncoderMethod::MeshEdgebreakerEncoding as i32,
    );
    test_number_of_encoded_entries(
        "deg_faces.obj",
        MeshEncoderMethod::MeshSequentialEncoding as i32,
    );
    test_number_of_encoded_entries(
        "cube_att.obj",
        MeshEncoderMethod::MeshEdgebreakerEncoding as i32,
    );
    test_number_of_encoded_entries(
        "test_nm.obj",
        MeshEncoderMethod::MeshEdgebreakerEncoding as i32,
    );
    test_number_of_encoded_entries(
        "test_nm.obj",
        MeshEncoderMethod::MeshSequentialEncoding as i32,
    );
    test_number_of_encoded_entries(
        "cube_subd.obj",
        PointCloudEncodingMethod::PointCloudKdTreeEncoding as i32,
    );
    test_number_of_encoded_entries(
        "cube_subd.obj",
        PointCloudEncodingMethod::PointCloudSequentialEncoding as i32,
    );
}

#[test]
fn test_tracking_of_number_of_encoded_entries_not_set() {
    let status_or = read_mesh_from_test_file("cube_att.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mesh = status_or.into_value();

    let mut buffer = EncoderBuffer::new();
    let mut encoder = Encoder::new();
    draco_assert_ok!(encoder.encode_mesh_to_buffer(&mesh, &mut buffer));

    assert_eq!(encoder.num_encoded_points(), 0);
    assert_eq!(encoder.num_encoded_faces(), 0);
}

#[test]
fn test_no_pos_quantization_normal_coding() {
    let status_or = read_mesh_from_test_file("test_nm.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mesh = status_or.into_value();

    assert!(mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .is_some());
    assert!(mesh
        .get_named_attribute(GeometryAttributeType::Normal)
        .is_some());

    let mut buffer = EncoderBuffer::new();
    let mut encoder = Encoder::new();
    encoder.set_attribute_quantization(GeometryAttributeType::Normal, 8);

    draco_assert_ok!(encoder.encode_mesh_to_buffer(&mesh, &mut buffer));

    let mut decoder = Decoder::new();
    let mut in_buffer = DecoderBuffer::new();
    in_buffer.init(buffer.data());
    let decoded_mesh = decoder.decode_mesh_from_buffer(&mut in_buffer).into_value();
    assert!(decoded_mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .is_some());
    assert!(decoded_mesh
        .get_named_attribute(GeometryAttributeType::Normal)
        .is_some());
}

#[test]
fn test_draco_compression_options() {
    let status_or = read_mesh_from_test_file("test_nm.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut mesh = status_or.into_value();

    let mut encoder_manual = Encoder::new();
    let mut buffer_manual = EncoderBuffer::new();
    encoder_manual.set_attribute_quantization(GeometryAttributeType::Position, 8);
    encoder_manual.set_attribute_quantization(GeometryAttributeType::Normal, 7);
    encoder_manual.set_speed_options(4, 4);
    draco_assert_ok!(encoder_manual.encode_mesh_to_buffer(&mesh, &mut buffer_manual));

    let mut compression_options = DracoCompressionOptions::default();
    compression_options.compression_level = 6;
    compression_options
        .quantization_position
        .set_quantization_bits(8);
    compression_options.quantization_bits_normal = 7;
    mesh.set_compression_options(compression_options.clone());
    mesh.set_compression_enabled(true);

    let mut encoder_auto = Encoder::new();
    let mut buffer_auto = EncoderBuffer::new();
    draco_assert_ok!(encoder_auto.encode_mesh_to_buffer(&mesh, &mut buffer_auto));

    assert_eq!(buffer_manual.size(), buffer_auto.size());

    compression_options.compression_level = 7;
    mesh.set_compression_options(compression_options);
    buffer_auto.clear();
    draco_assert_ok!(encoder_auto.encode_mesh_to_buffer(&mesh, &mut buffer_auto));
    assert_ne!(buffer_manual.size(), buffer_auto.size());

    let mesh_options = mesh.compression_options_mut();
    mesh_options.compression_level = 10;
    mesh_options.quantization_position.set_quantization_bits(10);
    mesh_options.quantization_bits_normal = 10;
    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder_manual.encode_mesh_to_buffer(&mesh, &mut buffer));
    assert_eq!(buffer.size(), buffer_manual.size());
}

#[test]
fn test_draco_compression_options_manual_override() {
    let status_or = read_mesh_from_test_file("test_nm.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut mesh = status_or.into_value();

    let mut compression_options = DracoCompressionOptions::default();
    compression_options.compression_level = 6;
    compression_options
        .quantization_position
        .set_quantization_bits(8);
    compression_options.quantization_bits_normal = 7;
    mesh.set_compression_options(compression_options);
    mesh.set_compression_enabled(true);

    let mut encoder = Encoder::new();
    let mut buffer_no_override = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_mesh_to_buffer(&mesh, &mut buffer_no_override));

    encoder.set_attribute_quantization(GeometryAttributeType::Position, 5);
    let mut buffer_with_override = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_mesh_to_buffer(&mesh, &mut buffer_with_override));
    assert!(buffer_with_override.size() < buffer_no_override.size());
}

#[test]
fn test_draco_compression_options_grid_quantization() {
    let status_or = read_mesh_from_test_file("cube_att.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut mesh = status_or.into_value();
    mesh.set_compression_enabled(true);

    let mut compression_options = DracoCompressionOptions::default();
    compression_options.quantization_position.set_grid(0.1);
    mesh.set_compression_options(compression_options);

    let mut encoder = ExpertEncoder::new_mesh(&mesh);
    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_to_buffer(&mut buffer));

    let pos_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Position);
    let mut origin = [0.0f32; 3];
    assert!(encoder.options().get_attribute_vector(
        &pos_att_id,
        "quantization_origin",
        3,
        &mut origin
    ));
    assert_eq!(origin, [0.0, 0.0, 0.0]);

    assert_eq!(
        encoder
            .options()
            .get_attribute_int(&pos_att_id, "quantization_bits", -1),
        4
    );

    let range = encoder
        .options()
        .get_attribute_float(&pos_att_id, "quantization_range", 0.0);
    assert!((range - 1.5).abs() < 1e-6);
}

#[test]
fn test_point_cloud_grid_quantization() {
    let status_or = read_point_cloud_from_test_file("cube_att.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let decoded = status_or.into_value();
    let pc = decoded.as_ref();
    let pos_att_id = pc.get_named_attribute_id(GeometryAttributeType::Position);

    let mut encoder = ExpertEncoder::new_point_cloud(pc);
    draco_assert_ok!(encoder.set_attribute_grid_quantization(pc, pos_att_id, 0.15));
    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_to_buffer(&mut buffer));

    let mut origin = [0.0f32; 3];
    assert!(encoder.options().get_attribute_vector(
        &pos_att_id,
        "quantization_origin",
        3,
        &mut origin
    ));
    assert_eq!(origin, [0.0, 0.0, 0.0]);

    assert_eq!(
        encoder
            .options()
            .get_attribute_int(&pos_att_id, "quantization_bits", -1),
        3
    );

    let range = encoder
        .options()
        .get_attribute_float(&pos_att_id, "quantization_range", 0.0);
    assert!((range - 1.05).abs() < 1e-6);
}

#[test]
fn test_point_cloud_grid_quantization_from_compression_options() {
    let status_or = read_point_cloud_from_test_file("cube_att.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut decoded = status_or.into_value();
    let pc = decoded.as_mut();
    pc.set_compression_enabled(true);

    let mut compression_options = DracoCompressionOptions::default();
    compression_options.quantization_position.set_grid(0.15);
    pc.set_compression_options(compression_options);

    let mut encoder = ExpertEncoder::new_point_cloud(pc);
    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_to_buffer(&mut buffer));

    let pos_att_id = pc.get_named_attribute_id(GeometryAttributeType::Position);
    let mut origin = [0.0f32; 3];
    assert!(encoder.options().get_attribute_vector(
        &pos_att_id,
        "quantization_origin",
        3,
        &mut origin
    ));
    assert_eq!(origin, [0.0, 0.0, 0.0]);

    assert_eq!(
        encoder
            .options()
            .get_attribute_int(&pos_att_id, "quantization_bits", -1),
        3
    );

    let range = encoder
        .options()
        .get_attribute_float(&pos_att_id, "quantization_range", 0.0);
    assert!((range - 1.05).abs() < 1e-6);
}

#[test]
fn test_draco_compression_options_grid_quantization_with_offset() {
    let status_or = read_mesh_from_test_file("cube_att.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut mesh = status_or.into_value();

    let pos_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Position);
    let pos_att = mesh
        .attribute_mut(pos_att_id)
        .expect("Missing position attribute");
    for i in 0..pos_att.size() {
        let avi = AttributeValueIndex::from(i as u32);
        let mut pos = [0.0f32; 3];
        pos_att
            .geometry_attribute()
            .get_value_array_into(avi, &mut pos);
        pos[0] += -0.55;
        pos[1] += 0.65;
        pos[2] += 10.75;
        pos_att.set_attribute_value_array(avi, &pos);
    }

    mesh.set_compression_enabled(true);
    let mut compression_options = DracoCompressionOptions::default();
    compression_options.quantization_position.set_grid(0.0625);
    mesh.set_compression_options(compression_options);

    let mut encoder = ExpertEncoder::new_mesh(&mesh);
    let mut buffer = EncoderBuffer::new();
    draco_assert_ok!(encoder.encode_to_buffer(&mut buffer));

    let pos_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Position);
    let mut origin = [0.0f32; 3];
    assert!(encoder.options().get_attribute_vector(
        &pos_att_id,
        "quantization_origin",
        3,
        &mut origin
    ));
    assert!((origin[0] + 0.5625).abs() < 1e-6);
    assert!((origin[1] - 0.625).abs() < 1e-6);
    assert!((origin[2] - 10.75).abs() < 1e-6);

    assert_eq!(
        encoder
            .options()
            .get_attribute_int(&pos_att_id, "quantization_bits", -1),
        5
    );

    let range = encoder
        .options()
        .get_attribute_float(&pos_att_id, "quantization_range", 0.0);
    assert!((range - (31.0 * 0.0625)).abs() < 1e-6);
}

// --- Ported from ref rans_coding_test.cc (linker test: ensure RANS types link) ---

#[test]
fn rans_coding_test_linker() {
    use draco_bitstream::compression::bit_coders::adaptive_rans_bit_decoder::AdaptiveRAnsBitDecoder;
    use draco_bitstream::compression::bit_coders::adaptive_rans_bit_encoder::AdaptiveRAnsBitEncoder;
    use draco_bitstream::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
    use draco_bitstream::compression::bit_coders::rans_bit_encoder::RAnsBitEncoder;
    let _enc = RAnsBitEncoder::new();
    let _dec = RAnsBitDecoder::new();
    let _aenc = AdaptiveRAnsBitEncoder::new();
    let _adec = AdaptiveRAnsBitDecoder::new();
}

// --- Ported from ref decoder_options_test.cc ---

#[test]
fn decoder_options_test_options() {
    use draco_bitstream::compression::config::decoder_options::DecoderOptions;
    let mut options = DecoderOptions::new();
    options.set_global_int("test", 3);
    assert_eq!(options.get_global_int("test", -1), 3);

    options.set_attribute_int(&GeometryAttributeType::Position, "test", 1);
    options.set_attribute_int(&GeometryAttributeType::Generic, "test", 2);
    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::TexCoord, "test", -1),
        3
    );
    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::Position, "test", -1),
        1
    );
    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::Generic, "test", -1),
        2
    );
}

#[test]
fn decoder_options_test_attribute_options_accessors() {
    use draco_bitstream::compression::config::decoder_options::DecoderOptions;
    let mut options = DecoderOptions::new();
    options.set_global_int("test", 1);
    options.set_attribute_int(&GeometryAttributeType::Position, "test", 2);
    options.set_attribute_int(&GeometryAttributeType::TexCoord, "test", 3);

    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::Position, "test", -1),
        2
    );
    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::Position, "test2", -1),
        -1
    );
    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::TexCoord, "test", -1),
        3
    );
    assert_eq!(
        options.get_attribute_int(&GeometryAttributeType::Normal, "test", -1),
        1
    );
}

// --- Ported from ref shannon_entropy_test.cc ---

#[test]
fn shannon_entropy_test_binary_entropy() {
    use draco_bitstream::compression::entropy::shannon_entropy::compute_binary_shannon_entropy;
    assert_eq!(compute_binary_shannon_entropy(0, 0), 0.0);
    assert_eq!(compute_binary_shannon_entropy(10, 0), 0.0);
    assert_eq!(compute_binary_shannon_entropy(10, 10), 0.0);
    assert!((compute_binary_shannon_entropy(10, 5) - 1.0).abs() < 1e-4);
}

#[test]
fn shannon_entropy_test_stream_entropy() {
    use draco_bitstream::compression::entropy::shannon_entropy::{
        compute_shannon_entropy, ShannonEntropyTracker,
    };
    let symbols: Vec<u32> = vec![1, 5, 1, 100, 2, 1];
    let mut tracker = ShannonEntropyTracker::new();
    assert_eq!(tracker.current_number_of_data_bits(), 0);

    let mut max_symbol: i32 = 0;
    for i in 0..symbols.len() {
        if symbols[i] as i32 > max_symbol {
            max_symbol = symbols[i] as i32;
        }
        let entropy_data = tracker.push(&symbols[i..i + 1], 1);
        let stream_bits = tracker.current_number_of_data_bits();
        assert_eq!(
            ShannonEntropyTracker::get_number_of_data_bits(&entropy_data),
            stream_bits
        );
        let expected_bits =
            compute_shannon_entropy(&symbols[..=i], (i + 1) as i32, max_symbol, None);
        assert!(
            (expected_bits - stream_bits).abs() <= 2,
            "expected_bits {} stream_bits {}",
            expected_bits,
            stream_bits
        );
    }

    let mut tracker2 = ShannonEntropyTracker::new();
    tracker2.push(symbols.as_slice(), symbols.len() as i32);
    assert_eq!(
        tracker.current_number_of_data_bits(),
        tracker2.current_number_of_data_bits()
    );

    let _ = tracker2.peek(&symbols[0..1], 1);
    assert_eq!(
        tracker.current_number_of_data_bits(),
        tracker2.current_number_of_data_bits()
    );
}

// --- Ported from ref symbol_coding_test.cc ---

#[test]
fn symbol_coding_test_large_numbers() {
    use draco_bitstream::compression::config::compression_shared::K_DRACO_MESH_BITSTREAM_VERSION;
    use draco_bitstream::compression::entropy::symbol_decoding::decode_symbols;
    use draco_bitstream::compression::entropy::symbol_encoding::encode_symbols;
    let input: [u32; 4] = [12_345_678, 1_223_333, 111, 5];
    let num_values = input.len() as i32;
    let mut enc = EncoderBuffer::new();
    assert!(encode_symbols(&input, num_values, 1, None, &mut enc));

    let mut out = vec![0u32; input.len()];
    let mut dec_buf = DecoderBuffer::new();
    dec_buf.init(enc.data());
    dec_buf.set_bitstream_version(K_DRACO_MESH_BITSTREAM_VERSION);
    assert!(decode_symbols(
        input.len() as u32,
        1,
        &mut dec_buf,
        &mut out
    ));
    assert_eq!(&input[..], &out[..]);
}

#[test]
fn symbol_coding_test_empty() {
    use draco_bitstream::compression::config::compression_shared::K_DRACO_MESH_BITSTREAM_VERSION;
    use draco_bitstream::compression::entropy::symbol_decoding::decode_symbols;
    use draco_bitstream::compression::entropy::symbol_encoding::encode_symbols;
    let mut enc = EncoderBuffer::new();
    assert!(encode_symbols(&[], 0, 1, None, &mut enc));
    let mut dec_buf = DecoderBuffer::new();
    dec_buf.init(enc.data());
    dec_buf.set_bitstream_version(K_DRACO_MESH_BITSTREAM_VERSION);
    assert!(decode_symbols(0, 1, &mut dec_buf, &mut []));
}

#[test]
fn symbol_coding_test_one_symbol() {
    use draco_bitstream::compression::config::compression_shared::K_DRACO_MESH_BITSTREAM_VERSION;
    use draco_bitstream::compression::entropy::symbol_decoding::decode_symbols;
    use draco_bitstream::compression::entropy::symbol_encoding::encode_symbols;
    let input: Vec<u32> = vec![0; 1200];
    let mut enc = EncoderBuffer::new();
    assert!(encode_symbols(
        input.as_slice(),
        input.len() as i32,
        1,
        None,
        &mut enc
    ));
    let mut out = vec![0u32; input.len()];
    let mut dec_buf = DecoderBuffer::new();
    dec_buf.init(enc.data());
    dec_buf.set_bitstream_version(K_DRACO_MESH_BITSTREAM_VERSION);
    assert!(decode_symbols(
        input.len() as u32,
        1,
        &mut dec_buf,
        &mut out
    ));
    assert_eq!(input, out);
}

#[test]
fn symbol_coding_test_conversion_full_range_i8() {
    use draco_core::core::bit_utils::{convert_signed_int_to_symbol, convert_symbol_to_signed_int};
    fn roundtrip(x: i8) {
        let sym = convert_signed_int_to_symbol(x);
        let y = convert_symbol_to_signed_int(sym);
        assert_eq!(x, y);
    }
    roundtrip(-128);
    roundtrip(-127);
    roundtrip(-1);
    roundtrip(0);
    roundtrip(1);
    roundtrip(127);
}

#[test]
fn symbol_coding_test_many_numbers() {
    use draco_bitstream::compression::config::compression_shared::{
        SymbolCodingMethod, K_DRACO_MESH_BITSTREAM_VERSION,
    };
    use draco_bitstream::compression::entropy::symbol_decoding::decode_symbols;
    use draco_bitstream::compression::entropy::symbol_encoding::{
        encode_symbols, set_symbol_encoding_method,
    };
    use draco_core::core::options::Options;
    let pairs: [(u32, u32); 5] = [(12, 1500), (1025, 31000), (7, 1), (9, 5), (0, 6432)];
    let in_values: Vec<u32> = pairs
        .iter()
        .flat_map(|(val, count)| std::iter::repeat(*val).take(*count as usize))
        .collect();
    let methods = [
        SymbolCodingMethod::SymbolCodingTagged,
        SymbolCodingMethod::SymbolCodingRaw,
    ];
    for method in methods {
        let mut options = Options::new();
        set_symbol_encoding_method(&mut options, method);
        let mut enc = EncoderBuffer::new();
        assert!(
            encode_symbols(
                in_values.as_slice(),
                in_values.len() as i32,
                1,
                Some(&options),
                &mut enc
            ),
            "method {:?}",
            method
        );
        let mut out = vec![0u32; in_values.len()];
        let mut dec_buf = DecoderBuffer::new();
        dec_buf.init(enc.data());
        dec_buf.set_bitstream_version(K_DRACO_MESH_BITSTREAM_VERSION);
        assert!(decode_symbols(
            in_values.len() as u32,
            1,
            &mut dec_buf,
            &mut out
        ));
        assert_eq!(in_values, out);
    }
}

#[test]
fn symbol_coding_test_bit_lengths() {
    use draco_bitstream::compression::config::compression_shared::K_DRACO_MESH_BITSTREAM_VERSION;
    use draco_bitstream::compression::entropy::symbol_decoding::decode_symbols;
    use draco_bitstream::compression::entropy::symbol_encoding::encode_symbols;
    const BIT_LENGTHS: i32 = 18;
    let in_vec: Vec<u32> = (0..BIT_LENGTHS).map(|i| 1 << i).collect();
    let mut out = vec![0u32; in_vec.len()];
    let mut enc = EncoderBuffer::new();
    for i in 0..BIT_LENGTHS {
        enc.clear();
        assert!(encode_symbols(
            &in_vec[..(i + 1) as usize],
            i + 1,
            1,
            None,
            &mut enc
        ));
        let mut dec_buf = DecoderBuffer::new();
        dec_buf.init(enc.data());
        dec_buf.set_bitstream_version(K_DRACO_MESH_BITSTREAM_VERSION);
        assert!(decode_symbols(
            (i + 1) as u32,
            1,
            &mut dec_buf,
            &mut out[..(i + 1) as usize]
        ));
        for j in 0..=i {
            assert_eq!(in_vec[j as usize], out[j as usize]);
        }
    }
}

#[test]
fn symbol_coding_test_large_number_condition() {
    use draco_bitstream::compression::config::compression_shared::K_DRACO_MESH_BITSTREAM_VERSION;
    use draco_bitstream::compression::entropy::symbol_decoding::decode_symbols;
    use draco_bitstream::compression::entropy::symbol_encoding::encode_symbols;
    const NUM_SYMBOLS: usize = 1_000_000;
    const VAL: u32 = 1 << 18;
    let input = vec![VAL; NUM_SYMBOLS];
    let mut enc = EncoderBuffer::new();
    assert!(encode_symbols(
        input.as_slice(),
        input.len() as i32,
        1,
        None,
        &mut enc
    ));
    let mut out = vec![0u32; input.len()];
    let mut dec_buf = DecoderBuffer::new();
    dec_buf.init(enc.data());
    dec_buf.set_bitstream_version(K_DRACO_MESH_BITSTREAM_VERSION);
    assert!(decode_symbols(
        input.len() as u32,
        1,
        &mut dec_buf,
        &mut out
    ));
    assert_eq!(input, out);
}

// --- Ported from ref point_d_vector_test.cc (size + copy) ---

#[test]
fn point_d_vector_test_size_and_copy() {
    use draco_bitstream::compression::attributes::point_d_vector::PointDVector;
    for n_items in 0..=10 {
        for dimensionality in 1..=10 {
            let v = PointDVector::<u32>::new(n_items, dimensionality);
            assert_eq!(v.size(), n_items);
            assert_eq!(v.data().len(), n_items * dimensionality);
        }
    }

    let n_items = 10usize;
    let dimensionality = 5usize;
    let att_dim = 3usize;
    let offset = 1usize;
    let mut var = PointDVector::<u32>::new(n_items, dimensionality);
    let att: Vec<u32> = (0..(n_items * att_dim))
        .map(|i| (i / att_dim) as u32)
        .collect();
    var.copy_attribute_buffer(att_dim, offset, &att);
    for val in 0..n_items {
        for d in 0..att_dim {
            assert_eq!(var.point(val)[offset + d], val as u32);
        }
    }

    let mut dest = PointDVector::<u32>::new(n_items, dimensionality);
    for item in 0..n_items {
        dest.copy_item(&var, item, item);
    }
    for val in 0..n_items {
        for d in 0..att_dim {
            assert_eq!(dest.point(val)[offset + d], val as u32);
        }
    }
}

// --- Ported from ref prediction_scheme_normal_octahedron_transform_test.cc (interface) ---

#[test]
fn prediction_scheme_normal_octahedron_transform_test_interface() {
    use draco_bitstream::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
    use draco_bitstream::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_encoding_transform::PredictionSchemeNormalOctahedronEncodingTransform;
    let transform = PredictionSchemeNormalOctahedronEncodingTransform::with_max_quantized_value(15);
    assert!(transform.are_corrections_positive());
    assert_eq!(transform.max_quantized_value(), 15);
    assert_eq!(transform.center_value(), 7);
    assert_eq!(transform.quantization_bits(), 4);
}

// --- Ported from ref prediction_scheme_normal_octahedron_canonicalized_transform_test (interface) ---

#[test]
fn prediction_scheme_normal_octahedron_canonicalized_transform_test_interface() {
    use draco_bitstream::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
    use draco_bitstream::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_canonicalized_encoding_transform::PredictionSchemeNormalOctahedronCanonicalizedEncodingTransform;
    let transform =
        PredictionSchemeNormalOctahedronCanonicalizedEncodingTransform::with_max_quantized_value(
            15,
        );
    assert!(transform.are_corrections_positive());
    assert_eq!(transform.max_quantized_value(), 15);
    assert_eq!(transform.center_value(), 7);
    assert_eq!(transform.quantization_bits(), 4);
}
