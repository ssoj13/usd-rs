//! IO tests (factory, stdio, utils) parity with `_ref/draco/src/draco/io/*_test.cc`.
//!
//! What: Validates file factories, buffer helpers, and format decoders against
//! reference testdata.
//! Why: Ensures IO behavior matches Draco C++ expectations before adding more
//! formats.
//! Where used: Runs under `draco-rs` tests using `crates/draco-rs/test`.

use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Once;

use crate::io::test_utils::{
    get_test_file_full_path, get_test_temp_dir, get_test_temp_file_full_path,
    read_mesh_from_test_file, read_point_cloud_from_test_file, read_scene_from_test_file,
};

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use crate::compression::DracoCompressionOptions;
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::io::file_reader_factory::FileReaderFactory;
use crate::io::file_reader_interface::FileReaderInterface;
use crate::io::file_utils;
use crate::io::file_writer_factory::FileWriterFactory;
use crate::io::file_writer_interface::FileWriterInterface;
use crate::io::file_writer_utils;
use crate::io::mesh_io;
use crate::io::obj_decoder::ObjDecoder;
use crate::io::obj_encoder::ObjEncoder;
use crate::io::ply_decoder::PlyDecoder;
use crate::io::ply_encoder::PlyEncoder;
use crate::io::ply_property_reader::PlyPropertyReader;
use crate::io::ply_reader::PlyReader;
use crate::io::point_cloud_io;
use crate::io::scene_io;
use crate::io::stdio_file_reader::StdioFileReader;
use crate::io::stdio_file_writer::StdioFileWriter;
use crate::io::stl_decoder::StlDecoder;
use crate::io::stl_encoder::StlEncoder;
use crate::io::texture_io;
use crate::mesh::Mesh;
use crate::point_cloud::PointCloud;
use crate::scene::{Scene, SceneUtils};
use draco_bitstream::compression::config::compression_shared::PointCloudEncodingMethod;
use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_core::metadata::metadata::MetadataString;

const FILE_SIZE_CAR_DRC: usize = 69892;
const FILE_SIZE_CUBE_PC_DRC: usize = 224;

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

/// Assigns the value from a StatusOr-like result or panics (test-only helper).
macro_rules! draco_assign_or_assert {
    ($lhs:expr, $expression:expr) => {{
        let _statusor = $expression;
        assert!(
            _statusor.is_ok(),
            "{}",
            _statusor.status().error_msg_string()
        );
        $lhs = _statusor.into_value();
    }};
}

fn attribute_type_from_index(index: i32) -> GeometryAttributeType {
    match index {
        0 => GeometryAttributeType::Position,
        1 => GeometryAttributeType::Normal,
        2 => GeometryAttributeType::Color,
        3 => GeometryAttributeType::TexCoord,
        4 => GeometryAttributeType::Generic,
        5 => GeometryAttributeType::Tangent,
        6 => GeometryAttributeType::Material,
        7 => GeometryAttributeType::Joints,
        8 => GeometryAttributeType::Weights,
        9 => GeometryAttributeType::NamedAttributesCount,
        _ => GeometryAttributeType::Invalid,
    }
}

struct AlwaysFailFileReader;

impl AlwaysFailFileReader {
    fn open(_file_name: &str) -> Option<Box<dyn FileReaderInterface>> {
        None
    }
}

impl FileReaderInterface for AlwaysFailFileReader {
    fn read_file_to_buffer(&mut self, _buffer: &mut Vec<u8>) -> bool {
        false
    }

    fn get_file_size(&mut self) -> usize {
        0
    }
}

struct AlwaysOkFileReader;

impl AlwaysOkFileReader {
    fn open(_file_name: &str) -> Option<Box<dyn FileReaderInterface>> {
        Some(Box::new(Self))
    }
}

impl FileReaderInterface for AlwaysOkFileReader {
    fn read_file_to_buffer(&mut self, _buffer: &mut Vec<u8>) -> bool {
        true
    }

    fn get_file_size(&mut self) -> usize {
        0
    }
}

struct AlwaysFailFileWriter;

impl AlwaysFailFileWriter {
    fn open(_file_name: &str) -> Option<Box<dyn FileWriterInterface>> {
        None
    }
}

impl FileWriterInterface for AlwaysFailFileWriter {
    fn write(&mut self, _buffer: &[u8]) -> bool {
        false
    }
}

struct AlwaysOkFileWriter;

impl AlwaysOkFileWriter {
    fn open(_file_name: &str) -> Option<Box<dyn FileWriterInterface>> {
        Some(Box::new(Self))
    }
}

impl FileWriterInterface for AlwaysOkFileWriter {
    fn write(&mut self, _buffer: &[u8]) -> bool {
        true
    }
}

static REGISTER_READERS: Once = Once::new();
static REGISTER_WRITERS: Once = Once::new();

fn register_readers_once() {
    REGISTER_READERS.call_once(|| {
        assert!(!FileReaderFactory::register_reader(None));
        assert!(FileReaderFactory::register_reader(Some(
            AlwaysFailFileReader::open
        )));
        assert!(FileReaderFactory::register_reader(Some(
            AlwaysOkFileReader::open
        )));
    });
}

fn register_writers_once() {
    REGISTER_WRITERS.call_once(|| {
        assert!(!FileWriterFactory::register_writer(None));
        assert!(FileWriterFactory::register_writer(Some(
            AlwaysFailFileWriter::open
        )));
        assert!(FileWriterFactory::register_writer(Some(
            AlwaysOkFileWriter::open
        )));
    });
}

#[test]
fn file_reader_factory_registration_fail() {
    // Assert registration of None is rejected.
    assert!(!FileReaderFactory::register_reader(None));
}

#[test]
fn file_reader_factory_open_reader() {
    register_readers_once();
    let path = get_test_file_full_path("cube_att.drc");
    let mut reader = FileReaderFactory::open_reader(&path);
    assert!(reader.is_some(), "open_reader failed for {}", path);
    let mut buffer: Vec<u8> = Vec::new();
    assert!(
        reader.as_mut().unwrap().read_file_to_buffer(&mut buffer),
        "read_file_to_buffer failed"
    );
    assert!(!buffer.is_empty());
}

#[test]
fn file_writer_factory_registration_fail() {
    assert!(!FileWriterFactory::register_writer(None));
}

#[test]
fn file_writer_factory_open_writer() {
    register_writers_once();
    let mut writer = FileWriterFactory::open_writer("fake file");
    assert!(writer.is_some());
    assert!(writer.as_mut().unwrap().write(&[]));
}

#[test]
fn file_utils_splits_path() {
    let mut folder = String::new();
    let mut file = String::new();
    file_utils::split_path("file.x", &mut folder, &mut file);
    assert_eq!(folder, ".");
    assert_eq!(file, "file.x");

    file_utils::split_path("a/b/file.y", &mut folder, &mut file);
    assert_eq!(folder, "a/b");
    assert_eq!(file, "file.y");

    file_utils::split_path("//a/b/c/d/file.z", &mut folder, &mut file);
    assert_eq!(folder, "//a/b/c/d");
    assert_eq!(file, "file.z");
}

#[test]
fn file_utils_replace_extension() {
    assert_eq!(file_utils::replace_file_extension("a.abc", "x"), "a.x");
    assert_eq!(file_utils::replace_file_extension("abc", "x"), "abc.x");
    assert_eq!(
        file_utils::replace_file_extension("a/b/c.d", "xyz"),
        "a/b/c.xyz"
    );
}

#[test]
fn file_utils_lowercase_extension() {
    assert_eq!(file_utils::lowercase_file_extension("image.jpeg"), "jpeg");
    assert_eq!(file_utils::lowercase_file_extension("image.JPEG"), "jpeg");
    assert_eq!(file_utils::lowercase_file_extension("image.png"), "png");
    assert_eq!(file_utils::lowercase_file_extension("image.pNg"), "png");
    assert_eq!(file_utils::lowercase_file_extension("FILE.glb"), "glb");
    assert_eq!(file_utils::lowercase_file_extension(".file.gltf"), "gltf");
    assert_eq!(
        file_utils::lowercase_file_extension("the.file.gltf"),
        "gltf"
    );
    assert_eq!(file_utils::lowercase_file_extension("FILE_glb"), "");
    assert_eq!(file_utils::lowercase_file_extension(""), "");
    assert_eq!(file_utils::lowercase_file_extension("image."), "");
}

#[test]
fn file_utils_get_full_path() {
    assert_eq!(
        file_utils::get_full_path("xo.png", "/d/i/r/xo.gltf"),
        "/d/i/r/xo.png"
    );
    assert_eq!(
        file_utils::get_full_path("buf/01.bin", "dir/xo.gltf"),
        "dir/buf/01.bin"
    );
    assert_eq!(file_utils::get_full_path("xo.mtl", "/xo.obj"), "/xo.mtl");
    assert_eq!(file_utils::get_full_path("xo.mtl", "xo.obj"), "xo.mtl");
}

#[test]
fn file_writer_utils_split_path_private_non_windows() {
    let test_path = "/path/to/file";
    let mut directory = String::new();
    let mut file = String::new();
    file_writer_utils::split_path_private(test_path, &mut directory, &mut file);
    assert_eq!(directory, "/path/to");
    assert_eq!(file, "file");
}

#[test]
fn file_writer_utils_split_path_private_windows() {
    let test_path = "C:\\path\\to\\file";
    let mut directory = String::new();
    let mut file = String::new();
    file_writer_utils::split_path_private(test_path, &mut directory, &mut file);
    assert_eq!(directory, "C:\\path\\to");
    assert_eq!(file, "file");
}

#[test]
fn file_writer_utils_directory_exists() {
    assert!(file_writer_utils::directory_exists(&get_test_temp_dir()));
    assert!(!file_writer_utils::directory_exists("fake/test/subdir"));
}

#[test]
fn file_writer_utils_check_and_create_path_for_file() {
    let fake_file = "fake.file";
    let fake_file_subdir = "a/few/dirs/down";
    let test_temp_dir = get_test_temp_dir();
    let fake_file_directory = format!("{}/{}", test_temp_dir, fake_file_subdir);
    let fake_full_path = format!("{}/{}", fake_file_directory, fake_file);
    assert!(file_writer_utils::check_and_create_path_for_file(
        &fake_full_path
    ));
    assert!(file_writer_utils::directory_exists(&fake_file_directory));
    let _ = fs::remove_dir_all(&fake_file_directory);
}

#[test]
fn stdio_file_reader_fail_open() {
    assert!(StdioFileReader::open("").is_none());
    assert!(StdioFileReader::open("stdio reader fake file").is_none());
}

#[test]
fn stdio_file_reader_open() {
    let car_path = get_test_file_full_path("car.drc");
    let cube_path = get_test_file_full_path("cube_pc.drc");
    assert!(StdioFileReader::open(&car_path).is_some());
    assert!(StdioFileReader::open(&cube_path).is_some());
}

#[test]
fn stdio_file_reader_fail_read_empty_file() {
    let temp_file = get_test_temp_file_full_path("empty.drc");
    if let Some(parent) = Path::new(&temp_file).parent() {
        fs::create_dir_all(parent).expect("create temp dir");
    }
    fs::write(&temp_file, &[]).expect("write empty temp file");
    let mut reader = StdioFileReader::open(&temp_file).expect("open temp file");
    let mut buffer: Vec<u8> = Vec::new();
    assert!(!reader.read_file_to_buffer(&mut buffer));
    let _ = fs::remove_file(&temp_file);
}

#[test]
fn stdio_file_reader_read_file() {
    let mut buffer: Vec<u8> = Vec::new();

    let car_path = get_test_file_full_path("car.drc");
    let mut reader = StdioFileReader::open(&car_path).expect("open car.drc");
    assert!(reader.read_file_to_buffer(&mut buffer));
    assert_eq!(buffer.len(), FILE_SIZE_CAR_DRC);

    let cube_path = get_test_file_full_path("cube_pc.drc");
    let mut reader = StdioFileReader::open(&cube_path).expect("open cube_pc.drc");
    assert!(reader.read_file_to_buffer(&mut buffer));
    assert_eq!(buffer.len(), FILE_SIZE_CUBE_PC_DRC);
}

#[test]
fn stdio_file_reader_get_file_size() {
    let car_path = get_test_file_full_path("car.drc");
    let mut reader = StdioFileReader::open(&car_path).expect("open car.drc");
    assert_eq!(reader.get_file_size(), FILE_SIZE_CAR_DRC);

    let cube_path = get_test_file_full_path("cube_pc.drc");
    let mut reader = StdioFileReader::open(&cube_path).expect("open cube_pc.drc");
    assert_eq!(reader.get_file_size(), FILE_SIZE_CUBE_PC_DRC);
}

#[test]
fn stdio_file_writer_fail_open() {
    assert!(StdioFileWriter::open("").is_none());
}

#[test]
fn stdio_file_writer_basic_write() {
    let data = "Hello".as_bytes();
    let temp_file = get_test_temp_file_full_path("hello");
    let mut writer = StdioFileWriter::open(&temp_file).expect("open writer");
    assert!(writer.write(data));
    drop(writer);
    let read_back = fs::read(&temp_file).expect("read back temp file");
    assert_eq!(read_back, data);
    let _ = fs::remove_file(&temp_file);
}

fn read_ply_file(file_name: &str) -> Vec<u8> {
    let path = get_test_file_full_path(file_name);
    let mut data: Vec<u8> = Vec::new();
    assert!(file_utils::read_file_to_buffer(&path, &mut data));
    data
}

#[test]
fn ply_reader_test_reader() {
    let data = read_ply_file("test_pos_color.ply");
    let mut buf = DecoderBuffer::new();
    buf.init(&data);
    let mut reader = PlyReader::new();
    let status = reader.read(&mut buf);
    draco_assert_ok!(status);
    assert_eq!(reader.num_elements(), 2);
    assert_eq!(reader.element(0).num_properties(), 7);
    assert_eq!(reader.element(1).num_properties(), 1);
    assert!(reader.element(1).property(0).is_list());

    let prop = reader
        .element(0)
        .get_property_by_name("red")
        .expect("missing red");
    let reader_uint8 = PlyPropertyReader::<u8>::new(prop);
    let reader_uint32 = PlyPropertyReader::<u32>::new(prop);
    let reader_float = PlyPropertyReader::<f32>::new(prop);
    for i in 0..reader.element(0).num_entries() {
        let v_u8 = reader_uint8.read_value(i);
        assert_eq!(v_u8 as u32, reader_uint32.read_value(i));
        assert_eq!(v_u8 as f32, reader_float.read_value(i));
    }
}

#[test]
fn ply_reader_test_reader_ascii() {
    let data = read_ply_file("test_pos_color.ply");
    assert!(!data.is_empty());
    let mut buf = DecoderBuffer::new();
    buf.init(&data);
    let mut reader = PlyReader::new();
    let status = reader.read(&mut buf);
    draco_assert_ok!(status);

    let data_ascii = read_ply_file("test_pos_color_ascii.ply");
    buf.init(&data_ascii);
    let mut reader_ascii = PlyReader::new();
    let status = reader_ascii.read(&mut buf);
    draco_assert_ok!(status);

    assert_eq!(reader.num_elements(), reader_ascii.num_elements());
    assert_eq!(
        reader.element(0).num_properties(),
        reader_ascii.element(0).num_properties()
    );

    let prop = reader
        .element(0)
        .get_property_by_name("x")
        .expect("missing x");
    let prop_ascii = reader_ascii
        .element(0)
        .get_property_by_name("x")
        .expect("missing x");
    let reader_float = PlyPropertyReader::<f32>::new(prop);
    let reader_float_ascii = PlyPropertyReader::<f32>::new(prop_ascii);
    for i in 0..reader.element(0).num_entries() {
        let a = reader_float.read_value(i);
        let b = reader_float_ascii.read_value(i);
        assert!((a - b).abs() <= 1e-4);
    }
}

#[test]
fn ply_reader_test_extra_whitespace() {
    let data = read_ply_file("test_extra_whitespace.ply");
    assert!(!data.is_empty());
    let mut buf = DecoderBuffer::new();
    buf.init(&data);
    let mut reader = PlyReader::new();
    let status = reader.read(&mut buf);
    draco_assert_ok!(status);

    assert_eq!(reader.num_elements(), 2);
    assert_eq!(reader.element(0).num_properties(), 7);
    assert_eq!(reader.element(1).num_properties(), 1);
    assert!(reader.element(1).property(0).is_list());

    let prop = reader
        .element(0)
        .get_property_by_name("red")
        .expect("missing red");
    let reader_uint8 = PlyPropertyReader::<u8>::new(prop);
    let reader_uint32 = PlyPropertyReader::<u32>::new(prop);
    let reader_float = PlyPropertyReader::<f32>::new(prop);
    for i in 0..reader.element(0).num_entries() {
        let v_u8 = reader_uint8.read_value(i);
        assert_eq!(v_u8 as u32, reader_uint32.read_value(i));
        assert_eq!(v_u8 as f32, reader_float.read_value(i));
    }
}

#[test]
fn ply_reader_test_more_datatypes() {
    let data = read_ply_file("test_more_datatypes.ply");
    assert!(!data.is_empty());
    let mut buf = DecoderBuffer::new();
    buf.init(&data);
    let mut reader = PlyReader::new();
    let status = reader.read(&mut buf);
    draco_assert_ok!(status);

    assert_eq!(reader.num_elements(), 2);
    assert_eq!(reader.element(0).num_properties(), 7);
    assert_eq!(reader.element(1).num_properties(), 1);
    assert!(reader.element(1).property(0).is_list());

    let prop = reader
        .element(0)
        .get_property_by_name("red")
        .expect("missing red");
    let reader_uint8 = PlyPropertyReader::<u8>::new(prop);
    let reader_uint32 = PlyPropertyReader::<u32>::new(prop);
    let reader_float = PlyPropertyReader::<f32>::new(prop);
    for i in 0..reader.element(0).num_entries() {
        let v_u8 = reader_uint8.read_value(i);
        assert_eq!(v_u8 as u32, reader_uint32.read_value(i));
        assert_eq!(v_u8 as f32, reader_float.read_value(i));
    }
}

fn decode_ply_mesh(file_name: &str) -> Option<Mesh> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = PlyDecoder::new();
    let mut mesh = Mesh::new();
    let status = decoder.decode_from_file_mesh(&path, &mut mesh);
    if !status.is_ok() {
        return None;
    }
    Some(mesh)
}

fn decode_ply_point_cloud(file_name: &str) -> Option<PointCloud> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = PlyDecoder::new();
    let mut pc = PointCloud::new();
    let status = decoder.decode_from_file_point_cloud(&path, &mut pc);
    if !status.is_ok() {
        return None;
    }
    Some(pc)
}

fn test_ply_decoding(
    file_name: &str,
    num_faces: u32,
    num_points: u32,
    out_mesh: Option<&mut Mesh>,
) {
    let mesh = decode_ply_mesh(file_name);
    assert!(mesh.is_some(), "Failed to load test model {}", file_name);
    let mut mesh = mesh.unwrap();
    assert_eq!(mesh.num_faces(), num_faces);
    if let Some(out) = out_mesh {
        *out = std::mem::take(&mut mesh);
    }

    let pc = decode_ply_point_cloud(file_name);
    assert!(pc.is_some(), "Failed to load test model {}", file_name);
    let pc = pc.unwrap();
    assert_eq!(pc.num_points(), num_points);
}

fn test_ply_decoding_any(file_name: &str) {
    let mesh = decode_ply_mesh(file_name);
    assert!(mesh.is_some(), "Failed to load test model {}", file_name);
    assert!(mesh.unwrap().num_faces() > 0);

    let pc = decode_ply_point_cloud(file_name);
    assert!(pc.is_some(), "Failed to load test model {}", file_name);
    assert!(pc.unwrap().num_points() > 0);
}

#[test]
fn ply_decoder_test_decoding() {
    test_ply_decoding("test_pos_color.ply", 224u32, 114, None);
}

#[test]
fn ply_decoder_test_normals() {
    let mut mesh = Mesh::new();
    test_ply_decoding("cube_att.ply", 12u32, 3 * 8, Some(&mut mesh));
    let att_id = mesh.get_named_attribute_id(GeometryAttributeType::Normal);
    assert!(att_id >= 0);
    let att = mesh.attribute(att_id).expect("normal attribute");
    assert_eq!(att.size(), 6);
}

#[test]
fn ply_decoder_test_decoding_all() {
    test_ply_decoding_any("bun_zipper.ply");
    test_ply_decoding_any("test_extra_whitespace.ply");
    test_ply_decoding_any("test_more_datatypes.ply");
    test_ply_decoding_any("test_pos_color_ascii.ply");
    test_ply_decoding("int_point_cloud.ply", 0u32, 16, None);
    test_ply_decoding_any("cube_quads.ply");
    test_ply_decoding_any("Box.ply");
    test_ply_decoding_any("delim_test.ply");
}

fn decode_obj_mesh(file_name: &str, deduplicate_input_values: bool) -> Option<Mesh> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = ObjDecoder::new();
    decoder.set_deduplicate_input_values(deduplicate_input_values);
    let mut mesh = Mesh::new();
    let status = decoder.decode_from_file_mesh(&path, &mut mesh);
    if !status.is_ok() {
        return None;
    }
    Some(mesh)
}

fn decode_obj_point_cloud(file_name: &str, deduplicate_input_values: bool) -> Option<PointCloud> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = ObjDecoder::new();
    decoder.set_deduplicate_input_values(deduplicate_input_values);
    let mut pc = PointCloud::new();
    let status = decoder.decode_from_file_point_cloud(&path, &mut pc);
    if !status.is_ok() {
        return None;
    }
    Some(pc)
}

fn decode_obj_with_metadata(file_name: &str) -> Option<Mesh> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = ObjDecoder::new();
    decoder.set_use_metadata(true);
    let mut mesh = Mesh::new();
    let status = decoder.decode_from_file_mesh(&path, &mut mesh);
    if !status.is_ok() {
        return None;
    }
    Some(mesh)
}

fn decode_obj_with_polygons(
    file_name: &str,
    _regularize_quads: bool,
    _store_added_edges_per_vertex: bool,
) -> Option<Mesh> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = ObjDecoder::new();
    decoder.set_preserve_polygons(true);
    let mut mesh = Mesh::new();
    let status = decoder.decode_from_file_mesh(&path, &mut mesh);
    if !status.is_ok() {
        return None;
    }
    Some(mesh)
}

fn test_obj_decoding(file_name: &str) {
    println!("obj decode {}", file_name);
    let mesh = decode_obj_mesh(file_name, false);
    assert!(mesh.is_some(), "Failed to load test model {}", file_name);
    assert!(mesh.unwrap().num_faces() > 0);

    let pc = decode_obj_point_cloud(file_name, false);
    assert!(pc.is_some(), "Failed to load test model {}", file_name);
    assert!(pc.unwrap().num_points() > 0);
}

#[test]
fn obj_decoder_extra_vertex_obj() {
    test_obj_decoding("extra_vertex.obj");
}

#[test]
fn obj_decoder_partial_attributes_obj() {
    test_obj_decoding("cube_att_partial.obj");
}

#[test]
fn obj_decoder_sub_objects() {
    let mesh = decode_obj_mesh("cube_att_sub_o.obj", false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert!(mesh.num_faces() > 0);
    assert_eq!(mesh.num_attributes(), 4);
    let att = mesh.attribute(3).expect("sub obj attribute");
    assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
    assert_eq!(att.size(), 3);
    assert_eq!(att.unique_id(), 3);
}

#[test]
fn obj_decoder_sub_objects_with_metadata() {
    let mesh = decode_obj_with_metadata("cube_att_sub_o.obj");
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert!(mesh.num_faces() > 0);
    assert_eq!(mesh.num_attributes(), 4);
    let att = mesh.attribute(3).expect("sub obj attribute");
    assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
    assert_eq!(att.size(), 3);

    let metadata = mesh
        .get_attribute_metadata_by_attribute_id(3)
        .expect("metadata");
    let mut sub_obj_id: i32 = 0;
    assert!(metadata.get_entry_int("obj2", &mut sub_obj_id));
    assert_eq!(sub_obj_id, 2);
}

#[test]
fn obj_decoder_quad_triangulate_obj() {
    let mesh = decode_obj_mesh("cube_quads.obj", false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.num_faces(), 12);
    assert_eq!(mesh.num_attributes(), 3);
    assert_eq!(mesh.num_points(), 4 * 6);
}

#[test]
fn obj_decoder_quad_preserve_obj() {
    let mesh = decode_obj_with_polygons("cube_quads.obj", false, false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.num_faces(), 12);
    assert_eq!(mesh.num_attributes(), 4);
    assert_eq!(mesh.num_points(), 4 * 6);

    let att = mesh.attribute(3).expect("added edges attribute");
    assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
    assert_eq!(att.size(), 2);
    let new_edge_value = att.get_value_array::<u8, 1>(AttributeValueIndex::from(0))[0];
    let old_edge_value = att.get_value_array::<u8, 1>(AttributeValueIndex::from(1))[0];
    assert_eq!(new_edge_value, 0);
    assert_eq!(old_edge_value, 1);

    for i in 0..6 {
        assert_eq!(att.mapped_index(PointIndex::from(4 * i + 0)).value(), 0);
        assert_eq!(att.mapped_index(PointIndex::from(4 * i + 1)).value(), 1);
        assert_eq!(att.mapped_index(PointIndex::from(4 * i + 2)).value(), 0);
        assert_eq!(att.mapped_index(PointIndex::from(4 * i + 3)).value(), 0);
    }

    let metadata = mesh
        .get_attribute_metadata_by_attribute_id(3)
        .expect("metadata");
    assert!(metadata.sub_metadatas().is_empty());
    assert_eq!(metadata.entries().len(), 1);
    let mut name = MetadataString::default();
    assert!(metadata.get_entry_string("name", &mut name));
    assert_eq!(name.as_bytes(), b"added_edges");
}

#[test]
fn obj_decoder_octagon_triangulated_obj() {
    let mesh = decode_obj_mesh("octagon.obj", false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.num_attributes(), 1);
    assert_eq!(mesh.num_points(), 8);
    let att = mesh.attribute(0).expect("pos");
    assert_eq!(att.attribute_type(), GeometryAttributeType::Position);
    assert_eq!(att.size(), 8);
}

#[test]
fn obj_decoder_octagon_preserved_obj() {
    let mesh = decode_obj_with_polygons("octagon.obj", false, false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();

    assert_eq!(mesh.num_attributes(), 2);
    let pos = mesh.attribute(0).expect("pos");
    assert_eq!(pos.attribute_type(), GeometryAttributeType::Position);
    assert_eq!(pos.size(), 8);

    let att = mesh.attribute(1).expect("added edges attribute");
    assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
    assert_eq!(mesh.num_points(), 8 + 4);

    assert_eq!(att.size(), 2);
    let new_edge_value = att.get_value_array::<u8, 1>(AttributeValueIndex::from(0))[0];
    let old_edge_value = att.get_value_array::<u8, 1>(AttributeValueIndex::from(1))[0];
    assert_eq!(new_edge_value, 0);
    assert_eq!(old_edge_value, 1);

    assert_eq!(att.mapped_index(PointIndex::from(0)).value(), 0);
    assert_eq!(att.mapped_index(PointIndex::from(1)).value(), 1);
    assert_eq!(att.mapped_index(PointIndex::from(2)).value(), 0);
    assert_eq!(att.mapped_index(PointIndex::from(3)).value(), 1);
    assert_eq!(att.mapped_index(PointIndex::from(4)).value(), 0);
    assert_eq!(att.mapped_index(PointIndex::from(5)).value(), 1);
    assert_eq!(att.mapped_index(PointIndex::from(6)).value(), 0);
    assert_eq!(att.mapped_index(PointIndex::from(7)).value(), 1);
    assert_eq!(att.mapped_index(PointIndex::from(8)).value(), 0);
    assert_eq!(att.mapped_index(PointIndex::from(9)).value(), 1);
    assert_eq!(att.mapped_index(PointIndex::from(10)).value(), 0);
    assert_eq!(att.mapped_index(PointIndex::from(11)).value(), 0);

    let metadata = mesh
        .get_attribute_metadata_by_attribute_id(1)
        .expect("metadata");
    assert!(metadata.sub_metadatas().is_empty());
    assert_eq!(metadata.entries().len(), 1);
    let mut name = MetadataString::default();
    assert!(metadata.get_entry_string("name", &mut name));
    assert_eq!(name.as_bytes(), b"added_edges");
}

#[test]
fn obj_decoder_empty_name_obj() {
    let mesh = decode_obj_mesh("empty_name.obj", false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.num_attributes(), 1);
    let att = mesh.attribute(0).expect("attr");
    assert_eq!(att.size(), 3);
}

#[test]
fn obj_decoder_point_cloud_obj() {
    let mesh = decode_obj_mesh("test_lines.obj", false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.num_faces(), 0);
    assert_eq!(mesh.num_attributes(), 1);
    let att = mesh.attribute(0).expect("attr");
    assert_eq!(att.size(), 484);
}

#[test]
fn obj_decoder_wrong_attribute_mapping() {
    let mesh = decode_obj_mesh("test_wrong_attribute_mapping.obj", false);
    assert!(mesh.is_some());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.num_attributes(), 1);
    let att = mesh.attribute(0).expect("attr");
    assert_eq!(att.size(), 3);
}

#[test]
fn obj_decoder_test_decoding_all() {
    test_obj_decoding("bunny_norm.obj");
    test_obj_decoding("cube_att.obj");
    test_obj_decoding("cube_att_partial.obj");
    test_obj_decoding("cube_att_sub_o.obj");
    test_obj_decoding("cube_quads.obj");
    test_obj_decoding("cube_subd.obj");
    test_obj_decoding("eof_test.obj");
    test_obj_decoding("extra_vertex.obj");
    test_obj_decoding("mat_test.obj");
    test_obj_decoding("one_face_123.obj");
    test_obj_decoding("one_face_312.obj");
    test_obj_decoding("one_face_321.obj");
    test_obj_decoding("sphere.obj");
    test_obj_decoding("test_nm.obj");
    test_obj_decoding("test_nm_trans.obj");
    test_obj_decoding("test_sphere.obj");
    test_obj_decoding("three_faces_123.obj");
    test_obj_decoding("three_faces_312.obj");
    test_obj_decoding("two_faces_123.obj");
    test_obj_decoding("two_faces_312.obj");
    test_obj_decoding("inf_nan.obj");
}

fn test_stl_decoding(file_name: &str) {
    let path = get_test_file_full_path(file_name);
    let mut decoder = StlDecoder::new();
    let mesh: Box<Mesh>;
    draco_assign_or_assert!(mesh, decoder.decode_from_file(&path));
    assert!(mesh.num_faces() > 0);
    assert!(mesh.num_points() > 0);
}

fn test_stl_decoding_should_fail(file_name: &str) {
    let path = get_test_file_full_path(file_name);
    let mut decoder = StlDecoder::new();
    let status_or = decoder.decode_from_file(&path);
    assert!(!status_or.is_ok());
}

#[test]
fn stl_decoder_test_decoding() {
    test_stl_decoding("STL/bunny.stl");
    test_stl_decoding("STL/test_sphere.stl");
    test_stl_decoding_should_fail("STL/test_sphere_ascii.stl");
}

fn compare_stl_meshes(mesh0: &Mesh, mesh1: &Mesh) {
    assert_eq!(mesh0.num_faces(), mesh1.num_faces());
    assert_eq!(mesh0.num_attributes(), mesh1.num_attributes());
    for att_id in 0..mesh0.num_attributes() {
        let att0 = mesh0.attribute(att_id).expect("mesh0 attribute");
        let att1 = mesh1.attribute(att_id).expect("mesh1 attribute");
        assert_eq!(att0.attribute_type(), att1.attribute_type());
        // Normals are recomputed during STL encoding and may differ.
        if att0.attribute_type() != GeometryAttributeType::Normal {
            assert_eq!(att0.size(), att1.size());
        }
    }
}

fn encode_and_decode_stl_mesh(mesh: &Mesh) -> Option<Mesh> {
    let mut encoder_buffer = EncoderBuffer::new();
    let mut encoder = StlEncoder::new();
    let status = encoder.encode_to_buffer(mesh, &mut encoder_buffer);
    if !status.is_ok() {
        return None;
    }

    let mut decoder_buffer = DecoderBuffer::new();
    decoder_buffer.init(encoder_buffer.data());
    let mut decoder = StlDecoder::new();
    let status_or_mesh = decoder.decode_from_buffer(&mut decoder_buffer);
    if !status_or_mesh.is_ok() {
        return None;
    }
    Some(*status_or_mesh.into_value())
}

fn test_stl_encoding(file_name: &str) {
    let mesh: Box<Mesh>;
    draco_assign_or_assert!(mesh, read_mesh_from_test_file(file_name));
    assert!(mesh.num_faces() > 0);

    let decoded_mesh = encode_and_decode_stl_mesh(&mesh).expect("decoded mesh");
    compare_stl_meshes(&mesh, &decoded_mesh);
}

#[test]
fn stl_encoder_test_encoding() {
    test_stl_encoding("STL/bunny.stl");
    test_stl_encoding("STL/test_sphere.stl");
}

#[test]
fn scene_io_test_scene_io() {
    let file_name = get_test_file_full_path("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");
    let scene: Box<crate::scene::Scene>;
    draco_assign_or_assert!(scene, scene_io::read_scene_from_file(&file_name));
    assert!(scene.num_nodes() > 0);

    let out_file_name = get_test_temp_file_full_path("out_scene.gltf");
    draco_assert_ok!(scene_io::write_scene_to_file(&out_file_name, &scene));

    assert!(file_utils::get_file_size(&out_file_name) > 0);
    assert!(file_utils::get_file_size(&get_test_temp_file_full_path("CesiumMilkTruck.png")) > 0);
    assert!(file_utils::get_file_size(&get_test_temp_file_full_path("buffer0.bin")) > 0);
}

#[test]
fn scene_io_test_save_to_ply() {
    let file_name = get_test_file_full_path("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");
    let scene: Box<crate::scene::Scene>;
    draco_assign_or_assert!(scene, scene_io::read_scene_from_file(&file_name));

    let out_file_name = get_test_temp_file_full_path("out_scene.ply");
    draco_assert_ok!(scene_io::write_scene_to_file(&out_file_name, &scene));

    let mesh: Box<Mesh>;
    draco_assign_or_assert!(
        mesh,
        mesh_io::read_mesh_from_file(&out_file_name, None, None)
    );
    assert!(mesh.num_faces() > 0);
}

#[test]
fn scene_io_test_save_to_obj() {
    let file_name = get_test_file_full_path("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");
    let scene: Box<crate::scene::Scene>;
    draco_assign_or_assert!(scene, scene_io::read_scene_from_file(&file_name));

    let out_file_name = get_test_temp_file_full_path("out_scene.obj");
    draco_assert_ok!(scene_io::write_scene_to_file(&out_file_name, &scene));

    let mesh: Box<Mesh>;
    draco_assign_or_assert!(
        mesh,
        mesh_io::read_mesh_from_file(&out_file_name, None, None)
    );
    assert!(mesh.num_faces() > 0);
}

fn read_mesh_from_test_file_with_metadata(file_name: &str) -> Box<Mesh> {
    let path = get_test_file_full_path(file_name);
    let status_or = mesh_io::read_mesh_from_file_with_metadata(&path, true);
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    status_or.into_value()
}

fn compare_meshes(mesh0: &Mesh, mesh1: &Mesh) {
    assert_eq!(mesh0.num_faces(), mesh1.num_faces());
    assert_eq!(mesh0.num_attributes(), mesh1.num_attributes());
    for att_id in 0..mesh0.num_attributes() {
        assert_eq!(
            mesh0.attribute(att_id).unwrap().size(),
            mesh1.attribute(att_id).unwrap().size()
        );
    }
}

fn encode_and_decode_mesh(mesh: &Mesh) -> Box<Mesh> {
    let mut encoder_buffer = EncoderBuffer::new();
    let mut encoder = ObjEncoder::new();
    assert!(encoder.encode_to_buffer_mesh(mesh, &mut encoder_buffer));

    let mut decoder_buffer = DecoderBuffer::new();
    decoder_buffer.init(encoder_buffer.data());
    let mut decoded_mesh = Box::new(Mesh::new());
    let mut decoder = ObjDecoder::new();
    decoder.set_use_metadata(true);
    let status = decoder.decode_from_buffer_mesh(&mut decoder_buffer, decoded_mesh.as_mut());
    assert!(status.is_ok(), "{}", status.error_msg_string());
    decoded_mesh
}

fn test_obj_encoding(file_name: &str) {
    let mesh = read_mesh_from_test_file_with_metadata(file_name);
    assert!(
        mesh.num_faces() > 0,
        "Failed to load test model {file_name}"
    );

    let decoded_mesh = encode_and_decode_mesh(&mesh);
    compare_meshes(&mesh, &decoded_mesh);
}

#[test]
fn obj_encoder_has_sub_object() {
    test_obj_encoding("cube_att_sub_o.obj");
}

#[test]
fn obj_encoder_has_material() {
    let mesh0 = read_mesh_from_test_file_with_metadata("mat_test.obj");
    let mesh1 = encode_and_decode_mesh(&mesh0);

    assert_eq!(mesh0.num_faces(), mesh1.num_faces());
    assert_eq!(mesh0.num_attributes(), mesh1.num_attributes());

    let mat_att_id0 = mesh0.get_named_attribute_id(GeometryAttributeType::Material);
    let mat_att_id1 = mesh1.get_named_attribute_id(GeometryAttributeType::Material);
    assert!(mat_att_id0 >= 0);
    assert!(mat_att_id1 >= 0);

    assert_eq!(
        mesh0.attribute(0).unwrap().size(),
        mesh1.attribute(0).unwrap().size()
    );
    assert_eq!(mesh0.attribute(mat_att_id0).unwrap().size(), 29);
    assert_eq!(mesh1.attribute(mat_att_id1).unwrap().size(), 7);
}

#[test]
fn obj_encoder_test_encoding_all() {
    test_obj_encoding("bunny_norm.obj");
    test_obj_encoding("cube_att.obj");
    test_obj_encoding("cube_att_partial.obj");
    test_obj_encoding("cube_quads.obj");
    test_obj_encoding("cube_subd.obj");
    test_obj_encoding("extra_vertex.obj");
    test_obj_encoding("multiple_isolated_triangles.obj");
    test_obj_encoding("multiple_tetrahedrons.obj");
    test_obj_encoding("one_face_123.obj");
    test_obj_encoding("one_face_312.obj");
    test_obj_encoding("one_face_321.obj");
    test_obj_encoding("sphere.obj");
    test_obj_encoding("test_nm.obj");
    test_obj_encoding("test_nm_trans.obj");
    test_obj_encoding("test_sphere.obj");
    test_obj_encoding("three_faces_123.obj");
    test_obj_encoding("three_faces_312.obj");
    test_obj_encoding("two_faces_123.obj");
    test_obj_encoding("two_faces_312.obj");
}

#[test]
fn obj_encoder_test_octagon_preserved() {
    let mesh: Box<Mesh>;
    draco_assign_or_assert!(mesh, read_mesh_from_test_file("octagon_preserved.drc"));
    assert_eq!(mesh.num_faces(), 6);
    assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Generic), 1);
    assert!(mesh
        .get_attribute_metadata_by_string_entry("name", "added_edges")
        .is_some());

    let encoded_path = get_test_temp_file_full_path("encoded.obj");
    let mut encoder = ObjEncoder::new();
    assert!(encoder.encode_to_file_mesh(&mesh, &encoded_path));

    let mut data_encoded: Vec<u8> = Vec::new();
    let mut data_golden: Vec<u8> = Vec::new();
    assert!(file_utils::read_file_to_buffer(
        &encoded_path,
        &mut data_encoded
    ));
    assert!(file_utils::read_file_to_buffer(
        &get_test_file_full_path("octagon_preserved.obj"),
        &mut data_golden
    ));

    assert_eq!(data_encoded.len(), data_golden.len());
    assert_eq!(data_encoded, data_golden);
}

#[test]
fn texture_io_test_load_from_buffer() {
    let file_name = get_test_file_full_path("test.png");
    let mut image_data: Vec<u8> = Vec::new();
    assert!(file_utils::read_file_to_buffer(&file_name, &mut image_data));

    let texture: Box<crate::texture::Texture>;
    draco_assign_or_assert!(
        texture,
        texture_io::read_texture_from_buffer_with_mime(&image_data, "image/png")
    );
    assert_eq!(texture.source_image().mime_type(), "image/png");

    let mut encoded_buffer: Vec<u8> = Vec::new();
    draco_assert_ok!(texture_io::write_texture_to_buffer(
        &texture,
        &mut encoded_buffer
    ));

    assert_eq!(image_data.len(), encoded_buffer.len());
    for i in 0..encoded_buffer.len() {
        assert_eq!(image_data[i], encoded_buffer[i]);
    }
}

#[test]
fn texture_io_test_wrong_extension() {
    let file_name = get_test_file_full_path("this_is_png.jpg");
    let texture: Box<crate::texture::Texture>;
    draco_assign_or_assert!(texture, texture_io::read_texture_from_file(&file_name));
    assert_eq!(texture.source_image().mime_type(), "image/png");
}

#[test]
fn texture_io_test_trailing_jpeg_bytes() {
    let file_name = get_test_file_full_path("trailing_zero.jpg");
    let texture: Box<crate::texture::Texture>;
    draco_assign_or_assert!(texture, texture_io::read_texture_from_file(&file_name));
    assert!(texture.source_image().mime_type().starts_with("image/"));
}

fn test_point_cloud_compression_method(
    method: PointCloudEncodingMethod,
    expected_num_attributes: i32,
    file_name: &str,
) {
    let status_or = read_point_cloud_from_test_file(file_name);
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let decoded = status_or.into_value();
    let pc = decoded.as_ref();
    assert!(
        pc.num_attributes() >= expected_num_attributes,
        "Failed to load test model: {} wrong number of attributes",
        file_name
    );

    let mut options = EncoderOptions::create_default_options();
    for att_id in 0..pc.num_attributes() {
        options.set_attribute_int(&att_id, "quantization_bits", 14);
    }

    let mut stream = Cursor::new(Vec::new());
    draco_assert_ok!(point_cloud_io::write_point_cloud_into_writer_with_options(
        pc,
        &mut stream,
        method,
        &options
    ));
    let data = stream.into_inner();
    let mut read_stream = Cursor::new(data);
    let status_or_decoded = point_cloud_io::read_point_cloud_from_reader(&mut read_stream);
    assert!(
        status_or_decoded.is_ok(),
        "{}",
        status_or_decoded.status().error_msg_string()
    );
    let decoded_pc = status_or_decoded.into_value();
    let decoded_pc = decoded_pc.as_ref();

    for i in 0..GeometryAttributeType::NamedAttributesCount as i32 {
        let att_type = attribute_type_from_index(i);
        assert_eq!(
            pc.num_named_attributes(att_type),
            decoded_pc.num_named_attributes(att_type)
        );
    }

    assert_eq!(pc.num_points(), decoded_pc.num_points());
}

#[test]
fn point_cloud_io_encode_sequential_nm_obj() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudSequentialEncoding,
        2,
        "test_nm.obj",
    );
}

#[test]
fn point_cloud_io_encode_sequential_pos_obj() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudSequentialEncoding,
        1,
        "point_cloud_test_pos.obj",
    );
}

#[test]
fn point_cloud_io_encode_sequential_pos_ply() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudSequentialEncoding,
        1,
        "point_cloud_test_pos.ply",
    );
}

#[test]
fn point_cloud_io_encode_sequential_pos_norm_obj() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudSequentialEncoding,
        2,
        "point_cloud_test_pos_norm.obj",
    );
}

#[test]
fn point_cloud_io_encode_sequential_pos_norm_ply() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudSequentialEncoding,
        2,
        "point_cloud_test_pos_norm.ply",
    );
}

#[test]
fn point_cloud_io_encode_kd_tree_pos_obj() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudKdTreeEncoding,
        1,
        "point_cloud_test_pos.obj",
    );
}

#[test]
fn point_cloud_io_encode_kd_tree_pos_ply() {
    test_point_cloud_compression_method(
        PointCloudEncodingMethod::PointCloudKdTreeEncoding,
        1,
        "point_cloud_test_pos.ply",
    );
}

#[test]
fn point_cloud_io_obj_file_input() {
    let status_or = read_point_cloud_from_test_file("test_nm.obj");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let decoded = status_or.into_value();
    let pc = decoded.as_ref();
    assert_eq!(pc.num_points(), 97);
}

/// Verifies extension parsing uses lowercase_file_extension (case-insensitive .OBJ/.PLY).
#[test]
fn point_cloud_io_extension_case_insensitive() {
    use std::io::Write;
    let src_path = get_test_file_full_path("test_nm.obj");
    let data = std::fs::read(&src_path).expect("read test_nm.obj");
    let out_upper = get_test_temp_file_full_path("point_cloud_upper.OBJ");
    let mut f = std::fs::File::create(&out_upper).expect("create temp .OBJ");
    f.write_all(&data).expect("write");
    f.sync_all().ok();
    drop(f);
    let status_or = point_cloud_io::read_point_cloud_from_file(&out_upper);
    assert!(
        status_or.is_ok(),
        "Uppercase .OBJ extension should be recognized: {}",
        status_or.status().error_msg_string()
    );
    let decoded = status_or.into_value();
    assert_eq!(decoded.as_ref().num_points(), 97);
}

#[test]
fn point_cloud_io_wrong_file_obj() {
    let status_or = read_point_cloud_from_test_file("wrong_file_name.obj");
    assert!(!status_or.is_ok());
}

#[test]
fn point_cloud_io_wrong_file_ply() {
    let status_or = read_point_cloud_from_test_file("wrong_file_name.ply");
    assert!(!status_or.is_ok());
}

#[test]
fn point_cloud_io_wrong_file() {
    let status_or = read_point_cloud_from_test_file("wrong_file_name");
    assert!(!status_or.is_ok());
}

fn assert_mesh_roundtrip_counts(source: &Mesh, roundtrip: &Mesh) {
    assert_eq!(source.num_faces(), roundtrip.num_faces());
    assert_eq!(source.num_points(), roundtrip.num_points());
    assert_eq!(source.num_attributes(), roundtrip.num_attributes());
    for i in 0..GeometryAttributeType::NamedAttributesCount as i32 {
        let att_type = attribute_type_from_index(i);
        assert_eq!(
            source.num_named_attributes(att_type),
            roundtrip.num_named_attributes(att_type)
        );
    }
}

fn assert_scene_roundtrip_counts(source: &Scene, roundtrip: &Scene) {
    assert_eq!(source.num_meshes(), roundtrip.num_meshes());
    assert_eq!(source.num_mesh_groups(), roundtrip.num_mesh_groups());
    assert_eq!(source.num_nodes(), roundtrip.num_nodes());
    assert_eq!(source.num_root_nodes(), roundtrip.num_root_nodes());
    assert_eq!(source.num_animations(), roundtrip.num_animations());
    assert_eq!(source.num_skins(), roundtrip.num_skins());
    assert_eq!(source.num_lights(), roundtrip.num_lights());
    assert_eq!(
        source.num_instance_arrays(),
        roundtrip.num_instance_arrays()
    );
}

#[test]
fn io_roundtrip_obj_mesh() {
    let source = read_mesh_from_test_file("cube_att.obj").into_value();
    let out_path = get_test_temp_file_full_path("roundtrip_cube_att.obj");
    let mut encoder = ObjEncoder::new();
    assert!(encoder.encode_to_file_mesh(&source, &out_path));
    let roundtrip = mesh_io::read_mesh_from_file(&out_path, None, None).into_value();
    assert_mesh_roundtrip_counts(&source, &roundtrip);
}

#[test]
fn io_roundtrip_ply_mesh() {
    let source = read_mesh_from_test_file("cube_att.ply").into_value();
    let out_path = get_test_temp_file_full_path("roundtrip_cube_att.ply");
    let mut encoder = PlyEncoder::new();
    assert!(encoder.encode_to_file_mesh(&source, &out_path));
    let roundtrip = mesh_io::read_mesh_from_file(&out_path, None, None).into_value();
    assert_mesh_roundtrip_counts(&source, &roundtrip);
}

#[test]
fn io_roundtrip_stl_mesh() {
    let source = read_mesh_from_test_file("STL/test_sphere.stl").into_value();
    let out_path = get_test_temp_file_full_path("roundtrip_test_sphere.stl");
    let mut encoder = StlEncoder::new();
    draco_assert_ok!(encoder.encode_to_file(&source, &out_path));
    let roundtrip = mesh_io::read_mesh_from_file(&out_path, None, None).into_value();
    assert_mesh_roundtrip_counts(&source, &roundtrip);
}

#[test]
fn io_roundtrip_draco_mesh() {
    let source = read_mesh_from_test_file("cube_att.obj.edgebreaker.cl4.2.2.drc").into_value();
    let mut stream = Cursor::new(Vec::new());
    draco_assert_ok!(mesh_io::write_mesh_into_writer(&source, &mut stream));
    let data = stream.into_inner();
    let out_path = get_test_temp_file_full_path("roundtrip_cube_att.drc");
    assert!(file_utils::write_buffer_to_file(&data, &out_path));
    let roundtrip = mesh_io::read_mesh_from_file(&out_path, None, None).into_value();
    assert_mesh_roundtrip_counts(&source, &roundtrip);
}

/// Encodes a mesh loaded from OBJ to Draco and roundtrips. Catches encoder bugs
/// that only appear when encoding OBJ-decoded meshes (e.g. face index out of range).
/// Reference: _ref/draco OBJ decoder + DeduplicatePointIds; io_roundtrip_draco_mesh
/// uses .drc input so it does not exercise OBJ→encode path.
#[test]
fn io_roundtrip_obj_encode_draco_mesh() {
    let path = get_test_file_full_path("cube_att.obj");
    let source = mesh_io::read_mesh_from_file(&path, None, None).into_value();
    let num_points = source.num_points();
    for fi in 0..source.num_faces() {
        let face = source.face(FaceIndex::from(fi as u32));
        for c in 0..3 {
            assert!(
                face[c].value() < num_points,
                "OBJ mesh invalid: face {} corner {} has point index {} >= num_points {}",
                fi,
                c,
                face[c].value(),
                num_points
            );
        }
    }
    let mut stream = Cursor::new(Vec::new());
    let status = mesh_io::write_mesh_into_writer(&source, &mut stream);
    assert!(status.is_ok(), "OBJ→Draco encode failed: {:?}", status);
    let data = stream.into_inner();
    let out_path = get_test_temp_file_full_path("roundtrip_cube_att_obj.drc");
    assert!(file_utils::write_buffer_to_file(&data, &out_path));
    let roundtrip = mesh_io::read_mesh_from_file(&out_path, None, None).into_value();
    assert_mesh_roundtrip_counts(&source, &roundtrip);
}

#[test]
fn io_roundtrip_gltf_scene() {
    let source = read_scene_from_test_file("Box/glTF/Box.gltf").into_value();
    let out_path = get_test_temp_file_full_path("roundtrip_box.gltf");
    draco_assert_ok!(scene_io::write_scene_to_file(&out_path, &source));
    let roundtrip = scene_io::read_scene_from_file(&out_path).into_value();
    assert_scene_roundtrip_counts(&source, &roundtrip);
}

/// Roundtrip glTF scene as .glb with Draco compression: read → encode with KHR_draco → decode → compare.
#[test]
fn io_roundtrip_gltf_glb_draco() {
    let mut source = read_scene_from_test_file("Box/glTF/Box.gltf").into_value();
    let options = DracoCompressionOptions::default();
    SceneUtils::set_draco_compression_options(Some(&options), source.as_mut());
    let out_path = get_test_temp_file_full_path("roundtrip_box_draco.glb");
    draco_assert_ok!(scene_io::write_scene_to_file(&out_path, &source));
    let roundtrip = scene_io::read_scene_from_file(&out_path).into_value();
    assert_scene_roundtrip_counts(&source, &roundtrip);
}
