//! Scene IO helpers.
//!
//! What: Reads and writes Draco scenes to external file formats.
//! Why: Provides scene-level IO parity with the Draco C++ transcoder API.
//! How: Dispatches by file extension, using glTF encoder/decoder and mesh IO.
//! Where used: `scene_io_test` and user-facing scene import/export utilities.

use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::core::options::Options;
use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::io::file_utils::{lowercase_file_extension, split_path};
use crate::io::gltf_decoder::GltfDecoder;
use crate::io::gltf_encoder::GltfEncoder;
use crate::io::obj_encoder::ObjEncoder;
use crate::io::ply_encoder::PlyEncoder;
use crate::scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SceneFileFormat {
    Unknown,
    Gltf,
    Usd,
    Ply,
    Obj,
}

fn get_scene_file_format(file_name: &str) -> SceneFileFormat {
    let extension = lowercase_file_extension(file_name);
    if extension == "gltf" || extension == "glb" {
        return SceneFileFormat::Gltf;
    }
    if extension == "usd" || extension == "usda" || extension == "usdc" || extension == "usdz" {
        return SceneFileFormat::Usd;
    }
    if extension == "obj" {
        return SceneFileFormat::Obj;
    }
    if extension == "ply" {
        return SceneFileFormat::Ply;
    }
    SceneFileFormat::Unknown
}

/// Reads a scene from a file. Currently only glTF 2.0 scene files are supported.
pub fn read_scene_from_file(file_name: &str) -> StatusOr<Box<Scene>> {
    read_scene_from_file_with_files(file_name, None)
}

/// Reads a scene from a file and returns associated files in |scene_files|.
pub fn read_scene_from_file_with_files(
    file_name: &str,
    mut scene_files: Option<&mut Vec<String>>,
) -> StatusOr<Box<Scene>> {
    match get_scene_file_format(file_name) {
        SceneFileFormat::Gltf => {
            let mut decoder = GltfDecoder::new();
            decoder.decode_from_file_to_scene(file_name, scene_files.as_deref_mut())
        }
        SceneFileFormat::Usd => StatusOr::new_status(Status::new(
            StatusCode::DracoError,
            "USD is not supported yet.",
        )),
        _ => StatusOr::new_status(Status::new(
            StatusCode::DracoError,
            "Unknown input file format.",
        )),
    }
}

/// Writes a scene into a file with default options.
pub fn write_scene_to_file(file_name: &str, scene: &Scene) -> Status {
    let options = Options::new();
    write_scene_to_file_with_options(file_name, scene, &options)
}

/// Writes a scene into a file, configurable with |options|.
pub fn write_scene_to_file_with_options(
    file_name: &str,
    scene: &Scene,
    _options: &Options,
) -> Status {
    let mut folder_path = String::new();
    let mut out_file_name = String::new();
    split_path(file_name, &mut folder_path, &mut out_file_name);
    let format = get_scene_file_format(file_name);
    match format {
        SceneFileFormat::Gltf => {
            let mut encoder = GltfEncoder::new();
            if !encoder.encode_to_file(scene, file_name, &folder_path) {
                return Status::new(StatusCode::DracoError, "Failed to encode the scene.");
            }
            ok_status()
        }
        SceneFileFormat::Usd => Status::new(StatusCode::DracoError, "USD is not supported yet."),
        SceneFileFormat::Ply | SceneFileFormat::Obj => {
            // Convert the scene to mesh via GLB and re-encode to target format.
            let mut gltf_encoder = GltfEncoder::new();
            let mut buffer = EncoderBuffer::new();
            let status = gltf_encoder.encode_to_buffer(scene, &mut buffer);
            if !status.is_ok() {
                return status;
            }
            let mut gltf_decoder = GltfDecoder::new();
            let mut dec_buffer = DecoderBuffer::new();
            dec_buffer.init(buffer.data());
            let mesh_or = gltf_decoder.decode_from_buffer(&dec_buffer);
            if !mesh_or.is_ok() {
                return mesh_or.status().clone();
            }
            let mesh = mesh_or.into_value();
            if format == SceneFileFormat::Ply {
                let mut ply_encoder = PlyEncoder::new();
                if !ply_encoder.encode_to_file_mesh(&mesh, file_name) {
                    return Status::new(
                        StatusCode::DracoError,
                        "Failed to encode the scene as PLY.",
                    );
                }
            } else {
                let mut obj_encoder = ObjEncoder::new();
                if !obj_encoder.encode_to_file_mesh(&mesh, file_name) {
                    return Status::new(
                        StatusCode::DracoError,
                        "Failed to encode the scene as OBJ.",
                    );
                }
            }
            ok_status()
        }
        SceneFileFormat::Unknown => {
            Status::new(StatusCode::DracoError, "Unknown output file format.")
        }
    }
}
