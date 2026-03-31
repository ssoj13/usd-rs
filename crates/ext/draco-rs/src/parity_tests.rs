//! Parity tests: encoder determinism, mesh/CornerTable dumps for C++ comparison.
//!
//! What: Ensures Rust encoder is deterministic and helps pinpoint divergence from C++.
//! Why: Traversal order and bitstream must match reference; no differences allowed.

use std::io::Cursor;

use crate::attributes::geometry_indices::FaceIndex;
use crate::io::mesh_io;
use crate::io::test_utils::{get_test_file_full_path, get_test_temp_file_full_path};
use crate::mesh::mesh_misc_functions::create_corner_table_from_position_attribute;
use draco_bitstream::compression::config::compression_shared::MeshEncoderMethod;
use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_bitstream::compression::encode::Encoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;

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

/// Builds encoder options matching CLI defaults (speed=3, qp=11, qt=10, qn=8).
fn cli_matching_encoder_options(mesh: &crate::mesh::Mesh) -> EncoderOptions {
    let mut enc = Encoder::new();
    enc.set_speed_options(3, 3);
    enc.set_attribute_quantization(GeometryAttributeType::Position, 11);
    enc.set_attribute_quantization(GeometryAttributeType::TexCoord, 10);
    enc.set_attribute_quantization(GeometryAttributeType::Normal, 8);
    enc.create_expert_encoder_options(mesh)
}

/// Encoder must produce byte-identical output when encoding the same mesh twice
/// with the same options. Catches non-determinism (e.g. HashMap iteration order).
#[test]
fn parity_encoder_determinism() {
    let path = get_test_file_full_path("cube_att.obj");
    let mesh = mesh_io::read_mesh_from_file(&path, None, None).into_value();
    let options = cli_matching_encoder_options(mesh.as_ref());

    let mut buf1 = Cursor::new(Vec::new());
    draco_assert_ok!(mesh_io::write_mesh_into_writer_with_options(
        mesh.as_ref(),
        &mut buf1,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
        &options,
    ));
    let data1 = buf1.into_inner();

    let mut buf2 = Cursor::new(Vec::new());
    draco_assert_ok!(mesh_io::write_mesh_into_writer_with_options(
        mesh.as_ref(),
        &mut buf2,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
        &options,
    ));
    let data2 = buf2.into_inner();

    assert_eq!(
        data1.len(),
        data2.len(),
        "Encoder must be deterministic: two encodes of same mesh gave different lengths"
    );
    assert_eq!(
        data1, data2,
        "Encoder must be deterministic: two encodes of same mesh gave different bytes"
    );
}

/// Encodes the mesh decoded from ref .drc. If the result still differs from ref .drc size,
/// the divergence is in the encoder. If it matches, the divergence was in OBJ decode/dedup.
#[test]
fn parity_encode_decoded_ref_mesh() {
    let ref_drc_path = get_test_file_full_path("cube_att_ref_out.drc");
    if !std::path::Path::new(&ref_drc_path).exists() {
        // ref .drc not yet generated; run ref_rust_roundtrip first
        return;
    }
    let ref_data = std::fs::read(&ref_drc_path).expect("read ref drc");
    let mesh = mesh_io::decode_mesh_from_bytes(&ref_data).into_value();
    let options = cli_matching_encoder_options(mesh.as_ref());
    let mut buf = Cursor::new(Vec::new());
    draco_assert_ok!(mesh_io::write_mesh_into_writer_with_options(
        mesh.as_ref(),
        &mut buf,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
        &options,
    ));
    let rust_from_ref_mesh = buf.into_inner();
    let ref_size = ref_data.len();
    let rust_size = rust_from_ref_mesh.len();
    // Log for debugging; exact match would indicate encoder parity when mesh is identical
    assert!(
        rust_size > 0,
        "Rust encode of ref-decoded mesh produced empty output"
    );
    // Document: if rust_size == ref_size and bytes match, encoder has full parity for this mesh
    eprintln!(
        "parity_encode_decoded_ref_mesh: ref={} bytes, rust_from_ref_mesh={} bytes",
        ref_size, rust_size
    );
}

/// Finds and reports the first byte index where ref and rust .drc differ.
/// Uses ref-decoded mesh encoded by Rust (parity_encode_decoded_ref_mesh) for apples-to-apples:
/// same mesh, different encoder output.
#[test]
fn parity_drc_byte_diff() {
    let ref_path = get_test_file_full_path("cube_att_ref_out.drc");
    if !std::path::Path::new(&ref_path).exists() {
        return;
    }
    let ref_data = std::fs::read(&ref_path).expect("read ref");
    let mesh = mesh_io::decode_mesh_from_bytes(&ref_data).into_value();
    let options = cli_matching_encoder_options(mesh.as_ref());
    let mut buf = Cursor::new(Vec::new());
    draco_assert_ok!(mesh_io::write_mesh_into_writer_with_options(
        mesh.as_ref(),
        &mut buf,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
        &options,
    ));
    let rust_data = buf.into_inner();
    let mut first_diff = None;
    for (i, (a, b)) in ref_data.iter().zip(rust_data.iter()).enumerate() {
        if a != b {
            first_diff = Some((i, *a, *b));
            break;
        }
    }
    if let Some((idx, ref_byte, rust_byte)) = first_diff {
        eprintln!(
            "parity_drc_byte_diff: first diff at byte {}: ref=0x{:02x} rust=0x{:02x}",
            idx, ref_byte, rust_byte
        );
        // Draco mesh bitstream layout (approx): header, encoder_method, num_vertices (varint),
        // num_faces (varint), num_attribute_data, attribute_ids, num_symbols (varint),
        // num_split_symbols (varint), split_data, traversal_buffer (start_faces + symbols + seams)
        if idx < 20 {
            eprintln!("  -> likely header/encoder_method/varint section");
        } else if idx < 50 {
            eprintln!("  -> likely connectivity/metadata section");
        } else {
            eprintln!("  -> likely traversal or attribute data section");
        }
    } else if ref_data.len() != rust_data.len() {
        eprintln!(
            "parity_drc_byte_diff: same up to min len {}; ref len={} rust len={}",
            ref_data.len().min(rust_data.len()),
            ref_data.len(),
            rust_data.len()
        );
    } else {
        eprintln!(
            "parity_drc_byte_diff: files are byte-identical ({} bytes)",
            ref_data.len()
        );
    }
}

/// Compares ref-decode vs rust-encode symbol sequences (bitstream order).
/// Encoder writes symbols in reverse traversal order, so we compare ref_decode
/// with reversed(rust_encode). Run dump test first to generate the files.
#[test]
fn parity_symbol_compare() {
    let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test");
    let ref_path = test_dir.join("parity_symbols_ref_decode.txt");
    let rust_path = test_dir.join("parity_symbols_rust_encode.txt");
    if !ref_path.exists() || !rust_path.exists() {
        return;
    }
    let ref_content = std::fs::read_to_string(&ref_path).expect("read ref symbols");
    let rust_content = std::fs::read_to_string(&rust_path).expect("read rust symbols");
    let ref_symbols: Vec<&str> = ref_content
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    let mut rust_symbols: Vec<&str> = rust_content
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    rust_symbols.reverse(); // Encoder writes in reverse traversal order
    let mut first_diff = None;
    for (i, (rs, rust_s)) in ref_symbols.iter().zip(rust_symbols.iter()).enumerate() {
        if *rs != *rust_s {
            first_diff = Some((i, *rs, *rust_s));
            break;
        }
    }
    if let Some((idx, ref_s, rust_s)) = first_diff {
        eprintln!(
            "parity_symbol_compare: first diff at index {}: ref={} rust={}",
            idx, ref_s, rust_s
        );
    } else if ref_symbols.len() != rust_symbols.len() {
        eprintln!(
            "parity_symbol_compare: same up to min len; ref={} rust={} symbols",
            ref_symbols.len(),
            rust_symbols.len()
        );
    } else {
        eprintln!(
            "parity_symbol_compare: symbol sequences match ({} symbols)",
            ref_symbols.len()
        );
    }
}

/// Decodes ref .drc and encodes mesh; with DRACO_PARITY_DUMP_SYMBOLS=1 and
/// DRACO_PARITY_DUMP_SYMBOLS_DECODE=1 writes:
///   - parity_symbols_ref_decode.txt (symbols from decoder when reading ref .drc)
///   - parity_symbols_rust_encode.txt (symbols from encoder when encoding the mesh)
/// Run: DRACO_PARITY_DUMP_SYMBOLS=1 DRACO_PARITY_DUMP_SYMBOLS_DECODE=1 cargo test -p draco-rs parity_symbol_roundtrip_dump --nocapture
#[test]
fn parity_symbol_roundtrip_dump() {
    let ref_path = get_test_file_full_path("cube_att_ref_out.drc");
    if !std::path::Path::new(&ref_path).exists() {
        return;
    }
    let ref_data = std::fs::read(&ref_path).expect("read ref drc");
    // Decode: with DRACO_PARITY_DUMP_SYMBOLS_DECODE=1 writes parity_symbols_ref_decode.txt
    let mesh = mesh_io::decode_mesh_from_bytes(&ref_data).into_value();
    let options = cli_matching_encoder_options(mesh.as_ref());
    let mut buf = Cursor::new(Vec::new());
    // Encode: with DRACO_PARITY_DUMP_SYMBOLS=1 writes parity_symbols_rust_encode.txt
    draco_assert_ok!(mesh_io::write_mesh_into_writer_with_options(
        mesh.as_ref(),
        &mut buf,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
        &options,
    ));
    let rust_data = buf.into_inner();
    eprintln!(
        "parity_symbol_roundtrip_dump: ref={} bytes, rust_encode={} bytes",
        ref_data.len(),
        rust_data.len()
    );
    assert!(rust_data.len() > 0);
}

/// Dumps mesh and CornerTable state for comparison with C++ reference.
/// Run with: DRACO_PARITY_DUMP=1 cargo test -p draco-rs parity_dump_mesh_and_corner_table --nocapture
#[test]
fn parity_dump_mesh_and_corner_table() {
    if std::env::var("DRACO_PARITY_DUMP").ok().as_deref() != Some("1") {
        return;
    }
    let path = get_test_file_full_path("cube_att.obj");
    let mesh = mesh_io::read_mesh_from_file(&path, None, None).into_value();
    let ct = create_corner_table_from_position_attribute(mesh.as_ref())
        .expect("CornerTable creation failed");

    let mut out = String::new();
    out.push_str(&format!("num_points={}\n", mesh.num_points()));
    out.push_str(&format!("num_faces={}\n", mesh.num_faces()));
    out.push_str("faces:\n");
    for fi in 0..mesh.num_faces() {
        let face = mesh.face(FaceIndex::from(fi as u32));
        out.push_str(&format!("  {}: {:?}\n", fi, face));
    }
    out.push_str(&format!("ct.num_vertices={}\n", ct.num_vertices()));
    out.push_str(&format!("ct.num_corners={}\n", ct.num_corners()));
    out.push_str("corner_to_vertex (first 20):\n");
    for c in 0..std::cmp::min(20, ct.num_corners()) {
        let ci = draco_core::attributes::geometry_indices::CornerIndex::new(c as u32);
        let v = ct.vertex(ci);
        let opp = ct.opposite(ci);
        out.push_str(&format!("  c{} -> v{} opp={}\n", c, v.value(), opp.value()));
    }
    let dump_path = get_test_temp_file_full_path("parity_dump_rust.txt");
    std::fs::write(&dump_path, &out).expect("write dump");
    eprintln!("Parity dump written to {}", dump_path);
}
