//! Mesh IO helpers.
//! Reference: `_ref/draco/src/draco/io/mesh_io.h` + `.cc`.

use std::path::Path;

use crate::core::decoder_buffer::DecoderBuffer;

/// Path for parity debug dumps (DRACO_PARITY_DUMP_SYMBOLS, DRACO_PARITY_DUMP_SYMBOLS_DECODE).
fn parity_dump_path(file_name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test")
        .join(file_name)
        .to_string_lossy()
        .into_owned()
}
use crate::core::encoder_buffer::EncoderBuffer;
use crate::core::options::Options;
use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::io::file_utils::{lowercase_file_extension, read_file_to_buffer};
use crate::io::gltf_decoder::GltfDecoder;
use crate::io::obj_decoder::ObjDecoder;
use crate::io::ply_decoder::PlyDecoder;
use crate::io::stl_decoder::StlDecoder;
use crate::mesh::Mesh;
use draco_bitstream::compression::config::compression_shared::MeshEncoderMethod;
use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_bitstream::compression::decode::Decoder;
use draco_bitstream::compression::expert_encode::ExpertEncoder;
use draco_bitstream::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use draco_bitstream::compression::mesh::take_decoded_traversal_symbols_for_parity;
use std::io::{Read, Write};

/// Decodes a Draco mesh from raw bytes.
/// When `DRACO_PARITY_DUMP_SYMBOLS_DECODE=1` is set, writes decoded traversal symbols
/// to `test/parity_symbols_ref_decode.txt` for parity comparison.
pub fn decode_mesh_from_bytes(data: &[u8]) -> StatusOr<Box<Mesh>> {
    let mut buffer = DecoderBuffer::new();
    buffer.init(data);
    let mut decoder = Decoder::new();
    let status_or = decoder.decode_mesh_from_buffer(&mut buffer);
    if !status_or.is_ok() {
        return StatusOr::new_status(Status::new(StatusCode::DracoError, "Error decoding input."));
    }
    if std::env::var("DRACO_PARITY_DUMP_SYMBOLS_DECODE")
        .ok()
        .as_deref()
        == Some("1")
    {
        if let Some(symbols) = take_decoded_traversal_symbols_for_parity() {
            let path = parity_dump_path("parity_symbols_ref_decode.txt");
            let mut s = String::new();
            for (i, sym) in symbols.iter().enumerate() {
                let name = match *sym {
                    0 => "C",
                    1 => "S",
                    3 => "L",
                    5 => "R",
                    7 => "E",
                    _ => "?",
                };
                s.push_str(&format!("{} {}\n", i, name));
            }
            let _ = std::fs::write(&path, &s);
            eprintln!(
                "DRACO_PARITY_DUMP_SYMBOLS_DECODE: wrote {} symbols to {}",
                symbols.len(),
                path
            );
        }
    }
    status_or
}

/// Reads a mesh from a file. Optional |options| configure metadata usage and
/// polygon preservation. Optional |mesh_files| collects input file references.
pub fn read_mesh_from_file(
    file_name: &str,
    options: Option<&Options>,
    mut mesh_files: Option<&mut Vec<String>>,
) -> StatusOr<Box<Mesh>> {
    let mut mesh = Box::new(Mesh::new());
    let extension = lowercase_file_extension(file_name);
    let default_options = Options::new();
    let options = options.unwrap_or(&default_options);

    // C++ pushes file_name into mesh_files BEFORE extension dispatch (for non-OBJ/non-glTF)
    if extension != "gltf" && extension != "obj" {
        if let Some(files) = mesh_files.as_deref_mut() {
            files.push(file_name.to_string());
        }
    }

    if extension == "obj" {
        let mut obj_decoder = ObjDecoder::new();
        obj_decoder.set_use_metadata(options.get_bool_or("use_metadata", false));
        obj_decoder.set_preserve_polygons(options.get_bool_or("preserve_polygons", false));
        let status = obj_decoder.decode_from_file_mesh_with_files(file_name, &mut mesh, mesh_files);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        return StatusOr::new_value(mesh);
    }
    if extension == "ply" {
        let mut ply_decoder = PlyDecoder::new();
        let status = ply_decoder.decode_from_file_mesh(file_name, &mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        return StatusOr::new_value(mesh);
    }
    if extension == "stl" {
        let mut stl_decoder = StlDecoder::new();
        return stl_decoder.decode_from_file(file_name);
    }
    if extension == "gltf" || extension == "glb" {
        let mut decoder = GltfDecoder::new();
        return decoder.decode_from_file(file_name, mesh_files);
    }

    let mut file_data: Vec<u8> = Vec::new();
    if !read_file_to_buffer(file_name, &mut file_data) {
        return StatusOr::new_status(Status::new(
            StatusCode::IoError,
            "Unable to read input file.",
        ));
    }
    decode_mesh_from_bytes(&file_data)
}

/// Reads a mesh from a file (default options).
pub fn read_mesh_from_file_simple(file_name: &str) -> StatusOr<Box<Mesh>> {
    read_mesh_from_file(file_name, None, None)
}

/// Reads a mesh from a file with metadata toggle (obj-specific).
pub fn read_mesh_from_file_with_metadata(
    file_name: &str,
    use_metadata: bool,
) -> StatusOr<Box<Mesh>> {
    let mut options = Options::new();
    options.set_bool("use_metadata", use_metadata);
    read_mesh_from_file(file_name, Some(&options), None)
}

/// Reads a mesh from a file with explicit options.
pub fn read_mesh_from_file_with_options(file_name: &str, options: &Options) -> StatusOr<Box<Mesh>> {
    read_mesh_from_file(file_name, Some(options), None)
}

/// Writes a mesh into a byte stream using explicit encoding options.
/// When `DRACO_PARITY_DUMP_SYMBOLS=1` is set, also writes the traversal symbols to
/// `test/parity_symbols_rust_encode.txt` for debugging.
pub fn write_mesh_into_writer_with_options<W: Write>(
    mesh: &Mesh,
    writer: &mut W,
    method: MeshEncoderMethod,
    options: &EncoderOptions,
) -> Status {
    let mut buffer = EncoderBuffer::new();
    let local_options = options.clone();
    let mut encoder = ExpertEncoder::new_mesh(mesh);
    encoder.reset(local_options);
    encoder.set_encoding_method(method as i32);
    let mut out_symbols: Vec<EdgebreakerTopologyBitPattern> = Vec::new();
    let status = if std::env::var("DRACO_PARITY_DUMP_SYMBOLS").ok().as_deref() == Some("1") {
        encoder.encode_to_buffer_with_symbols(&mut buffer, Some(&mut out_symbols))
    } else {
        encoder.encode_to_buffer(&mut buffer)
    };
    if !status.is_ok() {
        return status;
    }
    if !out_symbols.is_empty() {
        let path = parity_dump_path("parity_symbols_rust_encode.txt");
        let mut s = String::new();
        for (i, sym) in out_symbols.iter().enumerate() {
            let name = match *sym as u32 {
                0 => "C",
                1 => "S",
                3 => "L",
                5 => "R",
                7 => "E",
                _ => "?",
            };
            s.push_str(&format!("{} {}\n", i, name));
        }
        let _ = std::fs::write(&path, &s);
        eprintln!(
            "DRACO_PARITY_DUMP_SYMBOLS: wrote {} symbols to {}",
            out_symbols.len(),
            path
        );
    }
    if let Err(err) = writer.write_all(buffer.data()) {
        return Status::new(
            StatusCode::IoError,
            &format!("Stream write failed: {}", err),
        );
    }
    ok_status()
}

/// Writes a mesh into a byte stream using default encoder options.
pub fn write_mesh_into_writer_with_method<W: Write>(
    mesh: &Mesh,
    writer: &mut W,
    method: MeshEncoderMethod,
) -> Status {
    let options = EncoderOptions::create_default_options();
    write_mesh_into_writer_with_options(mesh, writer, method, &options)
}

/// Writes a mesh into a byte stream using the default encoding method.
pub fn write_mesh_into_writer<W: Write>(mesh: &Mesh, writer: &mut W) -> Status {
    write_mesh_into_writer_with_method(mesh, writer, MeshEncoderMethod::MeshEdgebreakerEncoding)
}

/// Reads a mesh from a byte stream encoded in Draco bitstream format.
pub fn read_mesh_from_reader<R: Read>(reader: &mut R) -> StatusOr<Box<Mesh>> {
    let mut data: Vec<u8> = Vec::new();
    if let Err(err) = reader.read_to_end(&mut data) {
        return StatusOr::new_status(Status::new(
            StatusCode::IoError,
            &format!("Stream read failed: {}", err),
        ));
    }
    decode_mesh_from_bytes(&data)
}
