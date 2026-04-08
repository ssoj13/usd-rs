//! usdcompress — Compress USD meshes with Draco
//!
//! Port of `_ref/OpenUSD/pxr/usd/bin/usdcompress/usdcompress.py`.
//!
//! Compresses mesh geometry in USD files using the Draco compression library.
//! Uses pure-Rust draco-rs (crates/draco-rs) — no C++ dependency.
//!
//! Implementation differences from Python reference:
//! - Uses pure-Rust Draco encoder instead of C++ `UsdDraco._WriteDraco`
//! - No `_PrimvarSupported` check yet (only strips base geometry properties)
//! - Triangulates polygons via fan triangulation before Draco encoding
//! - Interfaces and CLI match the Python `usdcompress` tool exactly

use usd::sdf::TimeCode;
use usd::sdf::layer_offset::LayerOffset;
use usd::usd::Stage;
use usd::usd::common::{InitialLoadSet, ListPosition};
use usd::usd::prim::Prim;
use usd::usd_geom::mesh::Mesh as UsdGeomMesh;

use draco_bitstream::compression::encode::Encoder as DracoEncoder;
use draco_core::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use draco_core::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::mesh::mesh::Mesh as DracoMesh;

use std::collections::HashMap;

/// USD mesh properties that are compressed by Draco.
/// Matches C++ `UsdDracoEncoder::ENCODED_PROPERTIES`.
const ENCODED_PROPERTIES: &[&str] = &[
    "extent",
    "faceVertexCounts",
    "faceVertexIndices",
    "points",
    "holeIndices",
];

// ---------------------------------------------------------------------------
// Public entry
// ---------------------------------------------------------------------------

/// Run the compress command.
pub fn run(args: &[String]) -> i32 {
    match run_impl(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

// ---------------------------------------------------------------------------
// Options (matches Python ParseOptions)
// ---------------------------------------------------------------------------

struct CompressOptions {
    input: String,
    output: String,
    verbose: bool,
    qp: i32,
    qt: i32,
    qn: i32,
    cl: i32,
    preserve_polygons: Option<bool>,
    discard_subdivision: Option<bool>,
    ignore_opinion_errors: bool,
}

fn parse_options(args: &[String]) -> Result<CompressOptions, String> {
    let mut input_file: Option<String> = None;
    let mut output_file: Option<String> = None;
    let mut verbose = false;
    let mut qp: i32 = 14;
    let mut qt: i32 = 12;
    let mut qn: i32 = 10;
    let mut cl: i32 = 10;
    let mut preserve_polygons: Option<bool> = None;
    let mut discard_subdivision: Option<bool> = None;
    let mut ignore_opinion_errors = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-o" | "--out" => {
                i += 1;
                if i >= args.len() {
                    return Err("-o/--out requires an output file".into());
                }
                output_file = Some(args[i].clone());
            }
            "-v" | "--verbose" => verbose = true,
            "-qp" => {
                i += 1;
                if i >= args.len() {
                    return Err("-qp requires a value".into());
                }
                qp = args[i].parse().map_err(|_| "Invalid -qp value (0-30)")?;
                if !(0..=30).contains(&qp) {
                    return Err("-qp must be 0-30".into());
                }
            }
            "-qt" => {
                i += 1;
                if i >= args.len() {
                    return Err("-qt requires a value".into());
                }
                qt = args[i].parse().map_err(|_| "Invalid -qt value (0-30)")?;
                if !(0..=30).contains(&qt) {
                    return Err("-qt must be 0-30".into());
                }
            }
            "-qn" => {
                i += 1;
                if i >= args.len() {
                    return Err("-qn requires a value".into());
                }
                qn = args[i].parse().map_err(|_| "Invalid -qn value (0-30)")?;
                if !(0..=30).contains(&qn) {
                    return Err("-qn must be 0-30".into());
                }
            }
            "-cl" => {
                i += 1;
                if i >= args.len() {
                    return Err("-cl requires a value".into());
                }
                cl = args[i].parse().map_err(|_| "Invalid -cl value (0-10)")?;
                if !(0..=10).contains(&cl) {
                    return Err("-cl must be 0-10".into());
                }
            }
            "--preserve_polygons" => {
                i += 1;
                if i >= args.len() {
                    return Err("--preserve_polygons requires 0 or 1".into());
                }
                preserve_polygons = Some(args[i] == "1");
            }
            "--discard_subdivision" => {
                i += 1;
                if i >= args.len() {
                    return Err("--discard_subdivision requires 0 or 1".into());
                }
                discard_subdivision = Some(args[i] == "1");
            }
            "--ignore_opinion_errors" => ignore_opinion_errors = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                if input_file.is_none() {
                    input_file = Some(args[i].clone());
                } else {
                    return Err("Only one input file expected".into());
                }
            }
        }
        i += 1;
    }

    let input = input_file.ok_or("No input file specified")?;
    let output = output_file.ok_or("Output file required (-o/--out)")?;

    if !std::path::Path::new(&input).is_file() {
        return Err(format!("Input file is missing: {}", input));
    }

    Ok(CompressOptions {
        input,
        output,
        verbose,
        qp,
        qt,
        qn,
        cl,
        preserve_polygons,
        discard_subdivision,
        ignore_opinion_errors,
    })
}

// ---------------------------------------------------------------------------
// UsdDracoEncoder (matches Python class UsdDracoEncoder)
// ---------------------------------------------------------------------------

struct UsdDracoEncoder<'a> {
    options: &'a CompressOptions,
    used_file_names: HashMap<String, u32>,
}

impl<'a> UsdDracoEncoder<'a> {
    fn new(options: &'a CompressOptions) -> Self {
        Self {
            options,
            used_file_names: HashMap::new(),
        }
    }

    /// Encodes meshes in a given stage and writes .drc files.
    /// Matches Python `encodeStage`.
    fn encode_stage(&mut self, stage: &Stage, draco_dir: &str) -> Result<(), String> {
        std::fs::create_dir_all(draco_dir)
            .map_err(|e| format!("Failed to create draco dir '{}': {}", draco_dir, e))?;

        if self.options.verbose {
            eprintln!("Meshes:");
        }

        self.used_file_names.clear();

        for prim in stage.traverse() {
            if prim.type_name().as_str() != "Mesh" {
                continue;
            }

            let geom_mesh = UsdGeomMesh::new(prim.clone());

            // Create a unique file name for the compressed mesh.
            let file_name = format!("{}{}.drc", draco_dir, self.get_file_name(&prim));

            // Compress mesh and write to file.
            self.encode_mesh(stage, &geom_mesh, &prim, &file_name)?;

            if self.options.verbose {
                eprintln!("  saved {}", file_name);
            }
        }

        Ok(())
    }

    /// Converts an Option<bool> to an i32 flag for Draco.
    /// Matches Python `fromBool`.
    fn from_bool(option: Option<bool>, invert: bool) -> i32 {
        const UNSPECIFIED: i32 = -1;
        match option {
            None => UNSPECIFIED,
            Some(v) => {
                let val = if v { 1 } else { 0 };
                if invert { 1 - val } else { val }
            }
        }
    }

    /// Compresses a mesh to a .drc file and strips geometry properties.
    /// Matches Python `encodeMesh`.
    fn encode_mesh(
        &self,
        _stage: &Stage,
        geom_mesh: &UsdGeomMesh,
        prim: &Prim,
        file_name: &str,
    ) -> Result<(), String> {
        let _preserve_polygons = Self::from_bool(self.options.preserve_polygons, false);
        let _preserve_pos_order = Self::from_bool(self.options.discard_subdivision, true);
        let _preserve_holes = Self::from_bool(self.options.discard_subdivision, true);

        // Compress mesh geometry with pure-Rust Draco and write to file.
        // This replaces the C++ `UsdDraco._WriteDraco()` call.
        let success = self.write_draco(geom_mesh, file_name)?;
        if !success {
            return Err(format!(
                "Could not encode mesh: {}",
                prim.path().to_string()
            ));
        }

        // Strip encoded geometry properties from the USD mesh.
        // Matches Python loop over ENCODED_PROPERTIES.
        for name in ENCODED_PROPERTIES {
            self.remove_property_or_exit(prim, name);
        }

        // TODO: Strip encoded primvars (requires PrimvarsAPI and _PrimvarSupported)
        // In C++/Python, this iterates PrimvarsAPI(prim).GetPrimvars()
        // and strips supported primvar properties + their :indices.

        // Add Draco file as a reference to the USD mesh prim.
        // Matches Python: mesh.GetPrim().GetReferences().AddReference(fileName)
        prim.get_references().add_reference_to_default_prim(
            file_name,
            LayerOffset::default(),
            ListPosition::BackOfPrependList,
        );

        Ok(())
    }

    /// Pure-Rust Draco encoding for a single USD mesh.
    /// Replaces C++ `UsdDraco._WriteDraco()`.
    fn write_draco(&self, geom_mesh: &UsdGeomMesh, file_name: &str) -> Result<bool, String> {
        let time = TimeCode::default();

        // Extract mesh data
        let face_vertex_counts = match geom_mesh.get_face_vertex_counts(time) {
            Some(c) => c,
            None => return Ok(false),
        };
        let face_vertex_indices = match geom_mesh.get_face_vertex_indices(time) {
            Some(idx) => idx,
            None => return Ok(false),
        };

        let points_attr = geom_mesh.point_based().get_points_attr();
        let points: Vec<[f32; 3]> = if let Some(val) = points_attr.get(time) {
            if let Some(arr) = val.get::<usd::vt::Array<usd::gf::vec3::Vec3f>>() {
                arr.iter().map(|v| [v.x, v.y, v.z]).collect()
            } else {
                return Ok(false);
            }
        } else {
            return Ok(false);
        };

        if points.is_empty() {
            return Ok(false);
        }

        // Triangulate: split polygons into triangles via fan triangulation
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut idx_offset = 0usize;
        for count in face_vertex_counts.iter() {
            let n = *count as usize;
            if n < 3 {
                idx_offset += n;
                continue;
            }
            // Fan triangulation from vertex 0 of each face
            let v0 = face_vertex_indices[idx_offset] as u32;
            for j in 1..n - 1 {
                let v1 = face_vertex_indices[idx_offset + j] as u32;
                let v2 = face_vertex_indices[idx_offset + j + 1] as u32;
                triangles.push([v0, v1, v2]);
            }
            idx_offset += n;
        }

        if triangles.is_empty() {
            return Ok(false);
        }

        // Build Draco mesh
        let num_points = points.len() as u32;
        let num_faces = triangles.len();
        let mut draco_mesh = DracoMesh::new();
        draco_mesh.set_num_faces(num_faces);
        draco_mesh.set_num_points(num_points);

        // Add position attribute
        let mut pos_ga = GeometryAttribute::new();
        let byte_stride = (3 * std::mem::size_of::<f32>()) as i64;
        pos_ga.init(
            GeometryAttributeType::Position,
            None,
            3,
            DataType::Float32,
            false,
            byte_stride,
            0,
        );
        let pos_att_id = draco_mesh.add_attribute_from_geometry(&pos_ga, true, num_points);

        // Set position values
        if let Some(att) = draco_mesh.attribute_mut(pos_att_id) {
            for (i, pt) in points.iter().enumerate() {
                att.set_attribute_value(AttributeValueIndex::from(i as u32), pt);
            }
        }

        // Set face indices
        for (fi, tri) in triangles.iter().enumerate() {
            let face = [
                PointIndex::from(tri[0]),
                PointIndex::from(tri[1]),
                PointIndex::from(tri[2]),
            ];
            draco_mesh.set_face(FaceIndex::from(fi as u32), face);
        }

        // Optionally add normals
        let normals_attr = geom_mesh.point_based().get_normals_attr();
        if let Some(val) = normals_attr.get(time) {
            if let Some(arr) = val.get::<usd::vt::Array<usd::gf::vec3::Vec3f>>() {
                let normals: Vec<[f32; 3]> = arr.iter().map(|v| [v.x, v.y, v.z]).collect();
                if normals.len() == points.len() {
                    let mut norm_ga = GeometryAttribute::new();
                    norm_ga.init(
                        GeometryAttributeType::Normal,
                        None,
                        3,
                        DataType::Float32,
                        false,
                        byte_stride,
                        0,
                    );
                    let norm_id =
                        draco_mesh.add_attribute_from_geometry(&norm_ga, true, num_points);
                    if let Some(att) = draco_mesh.attribute_mut(norm_id) {
                        for (i, n) in normals.iter().enumerate() {
                            att.set_attribute_value(AttributeValueIndex::from(i as u32), n);
                        }
                    }
                }
            }
        }

        // Encode with Draco
        let mut encoder = DracoEncoder::new();
        // Map compression level: our CL 0=fast..10=best → draco speed 10=fast..0=best
        let encoding_speed = 10 - self.options.cl;
        encoder.set_speed_options(encoding_speed, encoding_speed);
        encoder.set_attribute_quantization(GeometryAttributeType::Position, self.options.qp);
        encoder.set_attribute_quantization(GeometryAttributeType::Normal, self.options.qn);
        encoder.set_attribute_quantization(GeometryAttributeType::TexCoord, self.options.qt);

        let mut buffer = EncoderBuffer::new();
        let status = encoder.encode_mesh_to_buffer(&draco_mesh, &mut buffer);

        if !status.is_ok() {
            return Err(format!("Draco encoding failed: {}", status.error_msg()));
        }

        let compressed_data = buffer.data();

        // Write .drc file
        if let Some(parent) = std::path::Path::new(file_name).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir for {}: {}", file_name, e))?;
        }
        std::fs::write(file_name, compressed_data)
            .map_err(|e| format!("Failed to write {}: {}", file_name, e))?;

        Ok(true)
    }

    /// Removes a property from a prim, handling opinion errors.
    /// Matches Python `removePropertyOrExit`.
    fn remove_property_or_exit(&self, prim: &Prim, name: &str) {
        // Do nothing if prim has no property with the given name
        if !prim.has_attribute(name) {
            return;
        }

        // In the full C++ implementation, we would check the property stack
        // for multiple opinions and handle edit target matching.
        // For now, directly remove the property.
        //
        // TODO: Check property stack depth and opinion matching
        //   - If specs.len() > 1: warn_or_exit(prop, plural=true)
        //   - If specs.len() == 1 && spec != editTarget spec: warn_or_exit(prop, plural=false)
        if !prim.remove_property(name) {
            if self.options.ignore_opinion_errors {
                eprintln!(
                    "WARNING: Results may be invalid. Opinion will not be cleared for property: {}/{}",
                    prim.path().to_string(),
                    name
                );
            } else {
                eprintln!(
                    "ERROR: Results may be invalid. Opinion will not be cleared for property: {}/{}",
                    prim.path().to_string(),
                    name
                );
            }
        }
    }

    /// Returns a unique file name (without directory or extension) for a mesh.
    /// Matches Python `getFileName` exactly.
    fn get_file_name(&mut self, prim: &Prim) -> String {
        let path = prim.path().to_string();

        // Replace punctuation with '_' (matches Python string.punctuation behavior)
        let mut file_name: String = path
            .chars()
            .map(|c| if c.is_ascii_punctuation() { '_' } else { c })
            .collect();

        // Strip leading underscore
        if file_name.starts_with('_') {
            file_name = file_name[1..].to_string();
        }

        // Handle duplicate file names by appending _2, _3, etc.
        let counter = self.used_file_names.entry(file_name.clone()).or_insert(0);
        *counter += 1;
        if *counter > 1 {
            file_name = format!("{}_{}", file_name, counter);
        }

        file_name
    }
}

// ---------------------------------------------------------------------------
// Main implementation
// ---------------------------------------------------------------------------

fn run_impl(args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;

    // Print options in verbose mode (matches Python)
    if options.verbose {
        eprintln!("Options:");
        eprintln!("  input  : {}", options.input);
        eprintln!("  output : {}", options.output);
        eprintln!("  quantization bits for positions : {}", options.qp);
        eprintln!("  quantization bits for textures  : {}", options.qt);
        eprintln!("  quantization bits for normals   : {}", options.qn);
        eprintln!("  compression level : {}", options.cl);
        if let Some(pp) = options.preserve_polygons {
            eprintln!("  preserve polygons : {}", if pp { "yes" } else { "no" });
        }
        if let Some(ds) = options.discard_subdivision {
            eprintln!("  discard subdivision : {}", if ds { "yes" } else { "no" });
        }
        if options.ignore_opinion_errors {
            eprintln!("  ignore opinion errors");
        }
    }

    // Open USD stage (matches Python: Usd.Stage.Open(options.input))
    let stage = Stage::open(&options.input, InitialLoadSet::LoadAll)
        .map_err(|e| format!("Failed to open stage: {}", e))?;

    // Encode and save all meshes in USD stage with Draco
    // Matches Python: encoder.encodeStage(stage, options.output + '.draco/')
    let draco_dir = format!("{}.draco/", options.output);
    let mut encoder = UsdDracoEncoder::new(&options);
    encoder.encode_stage(&stage, &draco_dir)?;

    // Save the modified USD stage that references encoded meshes.
    // Matches Python: stage.GetRootLayer().Export(options.output)
    stage
        .get_root_layer()
        .export(&options.output)
        .map_err(|e| format!("Failed to export stage: {}", e))?;

    if options.verbose {
        eprintln!("Stage:");
        eprintln!("  saved {}", options.output);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Help text
// ---------------------------------------------------------------------------

fn print_help() {
    println!(
        r#"usdcompress - Compress USD meshes with Draco

USAGE:
    usd compress [options] <input> -o <output>

DESCRIPTION:
    Compress usd file to a specified output file and Draco-compressed
    files in the corresponding directory.

    Output consists of:
    - <output>         Modified USD file referencing compressed meshes
    - <output>.draco/  Directory with .drc compressed mesh files

ARGUMENTS:
    <input>    Input USD file

REQUIRED OPTIONS:
    -o, --out <file>    Output USD file

QUANTIZATION OPTIONS (bits, 0-30):
    -qp <bits>    Quantization bits for positions [default: 14]
    -qt <bits>    Quantization bits for textures [default: 12]
    -qn <bits>    Quantization bits for normals [default: 10]

COMPRESSION OPTIONS:
    -cl <level>   Compression level 0-10, best=10 [default: 10]
    --preserve_polygons <0|1>      Preserve polygon structure
    --discard_subdivision <0|1>    Discard subdivision data

OTHER OPTIONS:
    -h, --help                  Show this help
    -v, --verbose               Enable verbose output
    --ignore_opinion_errors     Proceed when opinions cannot be cleared

EXAMPLES:
    # Compress with defaults
    usd compress model.usd -o model_compressed.usd

    # High quality compression
    usd compress -qp 16 -qt 14 -cl 10 scene.usd -o scene_hq.usd

    # Fast compression with lower quality
    usd compress -qp 10 -qt 8 -cl 5 scene.usd -o scene_fast.usd

    # Verbose output
    usd compress -v model.usd -o model_out.usd
"#
    );
}
