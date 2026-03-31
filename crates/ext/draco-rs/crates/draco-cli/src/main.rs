//! Combined Draco CLI (encoder/decoder/transcoder) for draco-rs.
//!
//! What: Single binary that exposes Draco encoder, decoder, and transcoder tools.
//! Why: Matches C++ `draco_encoder`, `draco_decoder`, and `draco_transcoder` with one executable.
//! How: Parses subcommands and routes to Rust ports built on draco-rs/draco-bitstream.
//! Where used: Developer tooling, CI pipelines, and parity verification vs `_ref/draco`.

use std::env;
use std::process;

use draco_bitstream::compression::config::compression_shared::{
    EncodedGeometryType, PredictionSchemeMethod,
};
use draco_bitstream::compression::decode::{DecodedPointCloud, Decoder};
use draco_bitstream::compression::encode::Encoder;
use draco_bitstream::compression::expert_encode::ExpertEncoder;
use draco_rs::attributes::geometry_attribute::GeometryAttributeType;
use draco_rs::core::cycle_timer::CycleTimer;
use draco_rs::core::decoder_buffer::DecoderBuffer;
use draco_rs::core::encoder_buffer::EncoderBuffer;
use draco_rs::core::options::Options as DracoOptions;
use draco_rs::core::status::{Status, StatusCode};
use draco_rs::io::file_utils;
use draco_rs::io::mesh_io;
use draco_rs::io::obj_encoder::ObjEncoder;
use draco_rs::io::parser_utils;
use draco_rs::io::ply_encoder::PlyEncoder;
use draco_rs::io::point_cloud_io;
use draco_rs::io::stl_encoder::StlEncoder;
use draco_rs::mesh::Mesh;
use draco_rs::point_cloud::PointCloud;
use draco_rs::tools::transcoder_lib::{DracoTranscoder, DracoTranscodingOptions, FileOptions};

#[derive(Clone, Debug)]
struct EncoderOptions {
    is_point_cloud: bool,
    pos_quantization_bits: i32,
    tex_coords_quantization_bits: i32,
    tex_coords_deleted: bool,
    normals_quantization_bits: i32,
    normals_deleted: bool,
    generic_quantization_bits: i32,
    generic_deleted: bool,
    compression_level: i32,
    preserve_polygons: bool,
    use_metadata: bool,
    input: String,
    output: String,
}

impl Default for EncoderOptions {
    fn default() -> Self {
        Self {
            is_point_cloud: false,
            pos_quantization_bits: 11,
            tex_coords_quantization_bits: 10,
            tex_coords_deleted: false,
            normals_quantization_bits: 8,
            normals_deleted: false,
            generic_quantization_bits: 8,
            generic_deleted: false,
            compression_level: 7,
            preserve_polygons: false,
            use_metadata: false,
            input: String::new(),
            output: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DecodeOptions {
    input: String,
    output: String,
}

fn usage() {
    println!("Usage: draco <command> [options] -i input");
    println!("");
    println!("Commands:");
    println!("  encoder       encode a mesh/point cloud to .drc");
    println!("  decoder       decode .drc to .obj/.ply/.stl");
    println!("  transcoder    transcode glTF scenes with Draco compression");
    println!("");
    println!("Use 'draco <command> -h' for command-specific options.");
}

fn usage_encoder() {
    println!("Usage: draco encoder [options] -i input");
    println!("");
    println!("Main options:");
    println!("  -h | -?               show help.");
    println!("  -i <input>            input file name.");
    println!("  -o <output>           output file name.");
    println!("  -point_cloud          forces the input to be encoded as a point cloud.");
    println!("  -qp <value>           quantization bits for the position attribute, default=11.");
    println!("  -qt <value>           quantization bits for the texture coordinate attribute, default=10.");
    println!(
        "  -qn <value>           quantization bits for the normal vector attribute, default=8."
    );
    println!("  -qg <value>           quantization bits for any generic attribute, default=8.");
    println!("  -cl <value>           compression level [0-10], most=10, least=0, default=7.");
    println!("  --skip ATTRIBUTE_NAME skip a given attribute (NORMAL, TEX_COORD, GENERIC)");
    println!("  --metadata            use metadata to encode extra information in mesh files.");
    println!("  -preserve_polygons    encode polygon info as an attribute.");
    println!("");
    println!("Use negative quantization values to skip the specified attribute");
}

fn usage_decoder() {
    println!("Usage: draco decoder [options] -i input");
    println!("");
    println!("Main options:");
    println!("  -h | -?               show help.");
    println!("  -o <output>           output file name.");
}

fn usage_transcoder() {
    println!("Usage: draco transcoder [options] -i input -o output\n");
    println!("Main options:");
    println!("  -h | -?         show help.");
    println!("  -i <input>      input file name.");
    println!("  -o <output>     output file name.");
    println!("  -qp <value>     quantization bits for the position attribute, default=11.");
    println!(
        "  -qt <value>     quantization bits for the texture coordinate attribute, default=10."
    );
    println!("  -qn <value>     quantization bits for the normal vector attribute, default=8.");
    println!("  -qc <value>     quantization bits for the color attribute, default=8.");
    println!("  -qtg <value>    quantization bits for the tangent attribute, default=8.");
    println!("  -qw <value>     quantization bits for the weight attribute, default=8.");
    println!("  -qg <value>     quantization bits for any generic attribute, default=8.");
    println!("");
    println!("Boolean options may be negated by prefixing 'no'.");
}

fn string_to_i32(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(0)
}

fn print_encoder_options(pc: &PointCloud, options: &EncoderOptions) {
    println!("Encoder options:");
    println!("  Compression level = {}", options.compression_level);
    if options.pos_quantization_bits == 0 {
        println!("  Positions: No quantization");
    } else {
        println!(
            "  Positions: Quantization = {} bits",
            options.pos_quantization_bits
        );
    }

    if pc.get_named_attribute_id(GeometryAttributeType::TexCoord) >= 0 {
        if options.tex_coords_quantization_bits == 0 {
            println!("  Texture coordinates: No quantization");
        } else {
            println!(
                "  Texture coordinates: Quantization = {} bits",
                options.tex_coords_quantization_bits
            );
        }
    } else if options.tex_coords_deleted {
        println!("  Texture coordinates: Skipped");
    }

    if pc.get_named_attribute_id(GeometryAttributeType::Normal) >= 0 {
        if options.normals_quantization_bits == 0 {
            println!("  Normals: No quantization");
        } else {
            println!(
                "  Normals: Quantization = {} bits",
                options.normals_quantization_bits
            );
        }
    } else if options.normals_deleted {
        println!("  Normals: Skipped");
    }

    if pc.get_named_attribute_id(GeometryAttributeType::Generic) >= 0 {
        if options.generic_quantization_bits == 0 {
            println!("  Generic: No quantization");
        } else {
            println!(
                "  Generic: Quantization = {} bits",
                options.generic_quantization_bits
            );
        }
    } else if options.generic_deleted {
        println!("  Generic: Skipped");
    }
    println!("");
}

fn encode_point_cloud_to_file(
    _pc: &PointCloud,
    file: &str,
    encoder: &mut ExpertEncoder<'_>,
) -> i32 {
    let mut timer = CycleTimer::new();
    let mut buffer = EncoderBuffer::new();
    timer.start();
    let status = encoder.encode_to_buffer(&mut buffer);
    if !status.is_ok() {
        println!("Failed to encode the point cloud.");
        println!("{}", status.error_msg());
        return -1;
    }
    timer.stop();
    if !file_utils::write_buffer_to_file(buffer.data(), file) {
        println!("Failed to write the output file.");
        return -1;
    }
    println!(
        "Encoded point cloud saved to {} ({} ms to encode).",
        file,
        timer.get_in_ms()
    );
    println!("\nEncoded size = {} bytes\n", buffer.size());
    0
}

fn encode_mesh_to_file(_mesh: &Mesh, file: &str, encoder: &mut ExpertEncoder<'_>) -> i32 {
    let mut timer = CycleTimer::new();
    let mut buffer = EncoderBuffer::new();
    timer.start();
    let status = encoder.encode_to_buffer(&mut buffer);
    if !status.is_ok() {
        println!("Failed to encode the mesh.");
        println!("{}", status.error_msg());
        return -1;
    }
    timer.stop();
    if !file_utils::write_buffer_to_file(buffer.data(), file) {
        println!("Failed to create the output file.");
        return -1;
    }
    println!(
        "Encoded mesh saved to {} ({} ms to encode).",
        file,
        timer.get_in_ms()
    );
    println!("\nEncoded size = {} bytes\n", buffer.size());
    0
}

fn delete_named_attributes(pc: &mut PointCloud, attr_type: GeometryAttributeType) -> bool {
    let mut deleted = false;
    if pc.num_named_attributes(attr_type) > 0 {
        deleted = true;
    }
    while pc.num_named_attributes(attr_type) > 0 {
        let att_id = pc.get_named_attribute_id_by_index(attr_type, 0);
        pc.delete_attribute(att_id);
    }
    deleted
}

fn setup_expert_encoder<'a>(
    pc: &'a PointCloud,
    mesh: Option<&'a Mesh>,
    options: &EncoderOptions,
) -> ExpertEncoder<'a> {
    // Convert compression level to speed (0 = slowest, 10 = fastest).
    let speed = 10 - options.compression_level;

    let mut encoder = Encoder::new();
    if options.pos_quantization_bits > 0 {
        encoder.set_attribute_quantization(
            GeometryAttributeType::Position,
            options.pos_quantization_bits,
        );
    }
    if options.tex_coords_quantization_bits > 0 {
        encoder.set_attribute_quantization(
            GeometryAttributeType::TexCoord,
            options.tex_coords_quantization_bits,
        );
    }
    if options.normals_quantization_bits > 0 {
        encoder.set_attribute_quantization(
            GeometryAttributeType::Normal,
            options.normals_quantization_bits,
        );
    }
    if options.generic_quantization_bits > 0 {
        encoder.set_attribute_quantization(
            GeometryAttributeType::Generic,
            options.generic_quantization_bits,
        );
    }
    encoder.set_speed_options(speed, speed);

    let mut expert = match mesh {
        Some(mesh) => ExpertEncoder::new_mesh(mesh),
        None => ExpertEncoder::new_point_cloud(pc),
    };
    expert.reset(encoder.create_expert_encoder_options(pc));

    // If there is an attribute that stores polygon edges, disable prediction.
    let poly_att_id = pc.get_attribute_id_by_metadata_entry("name", "added_edges");
    if poly_att_id != -1 {
        let _ = expert.set_attribute_prediction_scheme(
            poly_att_id,
            PredictionSchemeMethod::PredictionNone as i32,
        );
    }
    expert
}

fn run_encoder(args: &[String]) -> i32 {
    // CLI parity with draco_encoder.cc: manual argv parsing and defaults.
    let mut options = EncoderOptions::default();
    let argc_check = args.len().saturating_sub(1);

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-h" || arg == "-?" {
            usage_encoder();
            return 0;
        } else if arg == "-i" && i < argc_check {
            options.input = args[i + 1].clone();
            i += 1;
        } else if arg == "-o" && i < argc_check {
            options.output = args[i + 1].clone();
            i += 1;
        } else if arg == "-point_cloud" {
            options.is_point_cloud = true;
        } else if arg == "-qp" && i < argc_check {
            options.pos_quantization_bits = string_to_i32(&args[i + 1]);
            if options.pos_quantization_bits > 30 {
                println!(
                    "Error: The maximum number of quantization bits for the position attribute is 30."
                );
                return -1;
            }
            i += 1;
        } else if arg == "-qt" && i < argc_check {
            options.tex_coords_quantization_bits = string_to_i32(&args[i + 1]);
            if options.tex_coords_quantization_bits > 30 {
                println!(
                    "Error: The maximum number of quantization bits for the texture coordinate attribute is 30."
                );
                return -1;
            }
            i += 1;
        } else if arg == "-qn" && i < argc_check {
            options.normals_quantization_bits = string_to_i32(&args[i + 1]);
            if options.normals_quantization_bits > 30 {
                println!(
                    "Error: The maximum number of quantization bits for the normal attribute is 30."
                );
                return -1;
            }
            i += 1;
        } else if arg == "-qg" && i < argc_check {
            options.generic_quantization_bits = string_to_i32(&args[i + 1]);
            if options.generic_quantization_bits > 30 {
                println!(
                    "Error: The maximum number of quantization bits for generic attributes is 30."
                );
                return -1;
            }
            i += 1;
        } else if arg == "-cl" && i < argc_check {
            options.compression_level = string_to_i32(&args[i + 1]);
            i += 1;
        } else if arg == "--skip" && i < argc_check {
            let name = args[i + 1].as_str();
            if name == "NORMAL" {
                options.normals_quantization_bits = -1;
            } else if name == "TEX_COORD" {
                options.tex_coords_quantization_bits = -1;
            } else if name == "GENERIC" {
                options.generic_quantization_bits = -1;
            } else {
                println!("Error: Invalid attribute name after --skip");
                return -1;
            }
            i += 1;
        } else if arg == "--metadata" {
            options.use_metadata = true;
        } else if arg == "-preserve_polygons" {
            options.preserve_polygons = true;
        }
        i += 1;
    }

    if args.is_empty() || options.input.is_empty() {
        usage_encoder();
        return -1;
    }

    if options.pos_quantization_bits < 0 {
        println!("Error: Position attribute cannot be skipped.");
        return -1;
    }

    if options.output.is_empty() {
        options.output = format!("{}.drc", options.input);
    }

    if options.is_point_cloud {
        let pc_or = point_cloud_io::read_point_cloud_from_file(&options.input);
        if !pc_or.is_ok() {
            println!(
                "Failed loading the input point cloud: {}.",
                pc_or.status().error_msg()
            );
            return -1;
        }
        let mut pc = pc_or.into_value();
        // Parity: delete only when quantization bits < 0 (--skip), like C++ draco_encoder.cc.
        if options.tex_coords_quantization_bits < 0 {
            options.tex_coords_deleted =
                delete_named_attributes(&mut pc, GeometryAttributeType::TexCoord);
        }
        if options.normals_quantization_bits < 0 {
            options.normals_deleted =
                delete_named_attributes(&mut pc, GeometryAttributeType::Normal);
        }
        if options.generic_quantization_bits < 0 {
            options.generic_deleted =
                delete_named_attributes(&mut pc, GeometryAttributeType::Generic);
        }
        if options.tex_coords_deleted || options.normals_deleted || options.generic_deleted {
            pc.deduplicate_point_ids();
        }

        print_encoder_options(pc.as_ref(), &options);
        let mut expert = setup_expert_encoder(pc.as_ref(), None, &options);
        let ret = encode_point_cloud_to_file(pc.as_ref(), &options.output, &mut expert);
        if ret != -1 && options.compression_level < 10 {
            println!("For better compression, increase the compression level up to '-cl 10'.\n");
        }
        return ret;
    }

    let mut load_options = DracoOptions::new();
    load_options.set_bool("use_metadata", options.use_metadata);
    load_options.set_bool("preserve_polygons", options.preserve_polygons);
    let mesh_or = mesh_io::read_mesh_from_file(&options.input, Some(&load_options), None);
    if !mesh_or.is_ok() {
        println!(
            "Failed loading the input mesh: {}.",
            mesh_or.status().error_msg()
        );
        return -1;
    }
    let mut mesh = mesh_or.into_value();
    let input_is_mesh = mesh.num_faces() > 0;

    {
        {
            let pc: &mut PointCloud = mesh.as_mut();
            // Parity: delete only when quantization bits < 0 (--skip), like C++ draco_encoder.cc.
            if options.tex_coords_quantization_bits < 0 {
                options.tex_coords_deleted =
                    delete_named_attributes(pc, GeometryAttributeType::TexCoord);
            }
            if options.normals_quantization_bits < 0 {
                options.normals_deleted =
                    delete_named_attributes(pc, GeometryAttributeType::Normal);
            }
            if options.generic_quantization_bits < 0 {
                options.generic_deleted =
                    delete_named_attributes(pc, GeometryAttributeType::Generic);
            }
        }
        if options.tex_coords_deleted || options.normals_deleted || options.generic_deleted {
            // Must use Mesh::deduplicate_point_ids when we have faces so that face indices are remapped (ref: draco_encoder.cc calls pc->DeduplicatePointIds() which for Mesh* uses Mesh::ApplyPointIdDeduplication).
            if input_is_mesh {
                mesh.deduplicate_point_ids();
            } else {
                mesh.as_mut().deduplicate_point_ids();
            }
        }

        print_encoder_options(mesh.as_ref(), &options);
    }

    let pc_ref: &PointCloud = mesh.as_ref();
    let mut expert = setup_expert_encoder(
        pc_ref,
        if input_is_mesh {
            Some(mesh.as_ref())
        } else {
            None
        },
        &options,
    );

    let ret = if input_is_mesh {
        encode_mesh_to_file(mesh.as_ref(), &options.output, &mut expert)
    } else {
        encode_point_cloud_to_file(pc_ref, &options.output, &mut expert)
    };

    if ret != -1 && options.compression_level < 10 {
        println!("For better compression, increase the compression level up to '-cl 10'.\n");
    }

    ret
}

fn return_decode_error(status: &Status) -> i32 {
    println!("Failed to decode the input file {}", status.error_msg());
    -1
}

fn decode_output_extension(output: &str) -> String {
    if output.len() >= 4 {
        parser_utils::to_lower(&output[output.len() - 4..])
    } else {
        parser_utils::to_lower(output)
    }
}

fn run_decoder(args: &[String]) -> i32 {
    // CLI parity with draco_decoder.cc: decode and dispatch by extension.
    let mut options = DecodeOptions::default();
    let argc_check = args.len().saturating_sub(1);

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-h" || arg == "-?" {
            usage_decoder();
            return 0;
        } else if arg == "-i" && i < argc_check {
            options.input = args[i + 1].clone();
            i += 1;
        } else if arg == "-o" && i < argc_check {
            options.output = args[i + 1].clone();
            i += 1;
        }
        i += 1;
    }

    if args.is_empty() || options.input.is_empty() {
        usage_decoder();
        return -1;
    }

    let mut data: Vec<u8> = Vec::new();
    if !file_utils::read_file_to_buffer(&options.input, &mut data) {
        println!("Failed opening the input file.");
        return -1;
    }
    if data.is_empty() {
        println!("Empty input file.");
        return -1;
    }

    let mut buffer = DecoderBuffer::new();
    buffer.init(&data);

    let mut timer = CycleTimer::new();
    let geom_type_or = Decoder::get_encoded_geometry_type(&mut buffer);
    if !geom_type_or.is_ok() {
        return return_decode_error(geom_type_or.status());
    }
    let geom_type = geom_type_or.into_value();

    enum DecodedGeometry {
        Mesh(Box<Mesh>),
        PointCloud(Box<PointCloud>),
    }

    let decoded = if geom_type == EncodedGeometryType::TriangularMesh {
        timer.start();
        let mut decoder = Decoder::new();
        let status_or = decoder.decode_mesh_from_buffer(&mut buffer);
        if !status_or.is_ok() {
            return return_decode_error(status_or.status());
        }
        let mesh = status_or.into_value();
        timer.stop();
        DecodedGeometry::Mesh(mesh)
    } else if geom_type == EncodedGeometryType::PointCloud {
        timer.start();
        let mut decoder = Decoder::new();
        let status_or = decoder.decode_point_cloud_from_buffer(&mut buffer);
        if !status_or.is_ok() {
            return return_decode_error(status_or.status());
        }
        let decoded = status_or.into_value();
        timer.stop();
        match decoded {
            DecodedPointCloud::PointCloud(pc) => DecodedGeometry::PointCloud(pc),
            DecodedPointCloud::Mesh(mesh) => DecodedGeometry::Mesh(mesh),
        }
    } else {
        println!("Failed to decode the input file.");
        return -1;
    };

    if options.output.is_empty() {
        options.output = format!("{}.ply", options.input);
    }

    let extension = decode_output_extension(&options.output);

    if extension == ".obj" {
        let mut obj_encoder = ObjEncoder::new();
        match decoded {
            DecodedGeometry::Mesh(mesh) => {
                if !obj_encoder.encode_to_file_mesh(mesh.as_ref(), &options.output) {
                    println!("Failed to store the decoded mesh as OBJ.");
                    return -1;
                }
            }
            DecodedGeometry::PointCloud(pc) => {
                if !obj_encoder.encode_to_file_point_cloud(pc.as_ref(), &options.output) {
                    println!("Failed to store the decoded point cloud as OBJ.");
                    return -1;
                }
            }
        }
    } else if extension == ".ply" {
        let mut ply_encoder = PlyEncoder::new();
        match decoded {
            DecodedGeometry::Mesh(mesh) => {
                if !ply_encoder.encode_to_file_mesh(mesh.as_ref(), &options.output) {
                    println!("Failed to store the decoded mesh as PLY.");
                    return -1;
                }
            }
            DecodedGeometry::PointCloud(pc) => {
                if !ply_encoder.encode_to_file_point_cloud(pc.as_ref(), &options.output) {
                    println!("Failed to store the decoded point cloud as PLY.");
                    return -1;
                }
            }
        }
    } else if extension == ".stl" {
        match decoded {
            DecodedGeometry::Mesh(mesh) => {
                let mut stl_encoder = StlEncoder::new();
                let status = stl_encoder.encode_to_file(mesh.as_ref(), &options.output);
                if status.code() != StatusCode::Ok {
                    println!("Failed to store the decoded mesh as STL.");
                    return -1;
                }
            }
            DecodedGeometry::PointCloud(_) => {
                println!("Can't store a point cloud as STL.");
                return -1;
            }
        }
    } else {
        println!("Invalid output file extension. Use .obj .ply or .stl.");
        return -1;
    }

    println!(
        "Decoded geometry saved to {} ({} ms to decode)",
        options.output,
        timer.get_in_ms()
    );
    0
}

fn transcode_file(
    file_options: &FileOptions,
    transcode_options: &DracoTranscodingOptions,
) -> Status {
    let mut timer = CycleTimer::new();
    timer.start();
    let dt_or = DracoTranscoder::create(transcode_options);
    if !dt_or.is_ok() {
        return dt_or.status().clone();
    }
    let mut dt = dt_or.into_value();
    let status = dt.transcode(file_options);
    timer.stop();
    if status.is_ok() {
        println!(
            "Transcode\t{}\t{}",
            file_options.input_filename,
            timer.get_in_ms()
        );
    }
    status
}

fn run_transcoder(args: &[String]) -> i32 {
    // CLI parity with draco_transcoder.cc: options map to DracoCompressionOptions.
    let mut file_options = FileOptions::default();
    let mut transcode_options = DracoTranscodingOptions::default();
    let argc_check = args.len().saturating_sub(1);

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-h" || arg == "-?" {
            usage_transcoder();
            return 0;
        } else if arg == "-i" && i < argc_check {
            file_options.input_filename = args[i + 1].clone();
            i += 1;
        } else if arg == "-o" && i < argc_check {
            file_options.output_filename = args[i + 1].clone();
            i += 1;
        } else if arg == "-qp" && i < argc_check {
            transcode_options
                .geometry
                .quantization_position
                .set_quantization_bits(string_to_i32(&args[i + 1]));
            i += 1;
        } else if arg == "-qt" && i < argc_check {
            transcode_options.geometry.quantization_bits_tex_coord = string_to_i32(&args[i + 1]);
            i += 1;
        } else if arg == "-qn" && i < argc_check {
            transcode_options.geometry.quantization_bits_normal = string_to_i32(&args[i + 1]);
            i += 1;
        } else if arg == "-qc" && i < argc_check {
            transcode_options.geometry.quantization_bits_color = string_to_i32(&args[i + 1]);
            i += 1;
        } else if arg == "-qtg" && i < argc_check {
            transcode_options.geometry.quantization_bits_tangent = string_to_i32(&args[i + 1]);
            i += 1;
        } else if arg == "-qw" && i < argc_check {
            transcode_options.geometry.quantization_bits_weight = string_to_i32(&args[i + 1]);
            i += 1;
        } else if arg == "-qg" && i < argc_check {
            transcode_options.geometry.quantization_bits_generic = string_to_i32(&args[i + 1]);
            i += 1;
        }
        i += 1;
    }

    if args.is_empty()
        || file_options.input_filename.is_empty()
        || file_options.output_filename.is_empty()
    {
        usage_transcoder();
        return -1;
    }

    let status = transcode_file(&file_options, &transcode_options);
    if !status.is_ok() {
        println!(
            "Failed\t{}\t{}",
            file_options.input_filename,
            status.error_msg()
        );
        return -1;
    }
    0
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
        process::exit(-1);
    }

    let command = args[1].as_str();
    let sub_args = &args[2..];
    let code = match command {
        "encoder" | "encode" => run_encoder(sub_args),
        "decoder" | "decode" => run_decoder(sub_args),
        "transcoder" | "transcode" => run_transcoder(sub_args),
        "-h" | "-?" | "--help" | "help" => {
            usage();
            0
        }
        _ => {
            usage();
            -1
        }
    };

    if code == 0 {
        process::exit(0);
    }
    process::exit(1);
}
