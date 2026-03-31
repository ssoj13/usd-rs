//! USD meshdump command - targeted composed-prim diagnostics.
//!
//! This exists because large production assets such as `bmw_x3` are too heavy
//! for the generic `usd dump` traversal when we only need to inspect one
//! composed prim. Keeping this as a focused command lets us compare what the
//! stage reader actually produced for `usda`, `usdc`, and `usdz` without
//! paying a whole-stage walk on every investigation.

use usd::sdf::{Path, TimeCode};
use usd::usd::{InitialLoadSet, Stage};
use usd::usd_geom::bbox_cache::BBoxCache;
use usd::usd_geom::boundable::Boundable as UsdGeomBoundable;
use usd::usd_geom::mesh::Mesh as UsdGeomMesh;
use usd::usd_geom::primvars_api::PrimvarsAPI as UsdGeomPrimvarsAPI;
use usd::usd_geom::tokens::usd_geom_tokens;
use usd::usd_geom::xformable::Xformable as UsdGeomXformable;
use usd_gf::vec2::Vec2f;
use usd_gf::vec3::Vec3d;
use usd_gf::vec3::Vec3f;
use usd_tf::Token;

/// Run the meshdump command with given arguments.
pub fn run(args: &[String]) -> i32 {
    match parse_args(args) {
        Ok(options) => match run_meshdump(&options) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("Failed to process '{}' - {}", options.input, error);
                1
            }
        },
        Err(ParseOutcome::Help) => {
            print_help();
            0
        }
        Err(ParseOutcome::Error(error)) => {
            eprintln!("error: {}", error);
            eprintln!();
            print_usage();
            1
        }
    }
}

struct MeshDumpOptions {
    input: String,
    prim_path: String,
    time: TimeCode,
}

enum ParseOutcome {
    Help,
    Error(String),
}

fn parse_args(args: &[String]) -> Result<MeshDumpOptions, ParseOutcome> {
    if args.len() < 2 {
        return Err(ParseOutcome::Error("no input file specified".into()));
    }

    let mut input: Option<String> = None;
    let mut prim_path: Option<String> = None;
    let mut time = TimeCode::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "-t" | "--time" => {
                i += 1;
                if i >= args.len() {
                    return Err(ParseOutcome::Error(
                        "--time requires a numeric argument".into(),
                    ));
                }
                let parsed = args[i]
                    .parse::<f64>()
                    .map_err(|_| ParseOutcome::Error(format!("invalid time '{}'", args[i])))?;
                time = TimeCode::new(parsed);
            }
            arg if arg.starts_with('-') => {
                return Err(ParseOutcome::Error(format!("unknown option: {}", arg)));
            }
            _ => {
                if input.is_none() {
                    input = Some(args[i].clone());
                } else if prim_path.is_none() {
                    prim_path = Some(args[i].clone());
                } else {
                    return Err(ParseOutcome::Error(
                        "expected exactly one input file and one prim path".into(),
                    ));
                }
            }
        }
        i += 1;
    }

    let input = input.ok_or_else(|| ParseOutcome::Error("no input file specified".into()))?;
    let prim_path =
        prim_path.ok_or_else(|| ParseOutcome::Error("no prim path specified".into()))?;

    Ok(MeshDumpOptions {
        input,
        prim_path,
        time,
    })
}

fn print_usage() {
    eprintln!("Usage: usd meshdump [options] <inputFile> <primPath>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -t, --time <value>  Sample time (default: default time)");
    eprintln!("  -h, --help          Show this help");
}

fn print_help() {
    println!("usd meshdump - Dump one composed prim/mesh with xform and bounds details");
    println!();
    print_usage();
}

fn run_meshdump(options: &MeshDumpOptions) -> Result<(), String> {
    let path = Path::from_string(&options.prim_path)
        .ok_or_else(|| format!("invalid prim path '{}'", options.prim_path))?;
    let stage = Stage::open(&options.input, InitialLoadSet::LoadAll)
        .map_err(|error| format!("failed to open stage: {}", error))?;
    let prim = stage
        .get_prim_at_path(&path)
        .ok_or_else(|| format!("prim not found at '{}'", options.prim_path))?;
    let default_prim = stage.get_default_prim();

    println!("file: {}", options.input);
    println!("requestedPath: {}", options.prim_path);
    println!("resolvedPath: {}", prim.path());
    println!("type: {}", prim.type_name());
    println!("time: {}", format_time_code(options.time));
    println!(
        "defaultPrim: {}",
        if default_prim.is_valid() {
            default_prim.path().to_string()
        } else {
            "<none>".to_string()
        }
    );
    println!("parent: {}", prim.path().get_parent_path());
    println!("children: {}", prim.get_all_children().len());
    for child in prim.get_all_children() {
        println!("  child: {} [{}]", child.path(), child.type_name());
    }

    dump_xformable(&prim, options.time);
    dump_boundable(&prim, options.time);
    dump_composed_bounds(&stage, &prim, options.time);
    dump_mesh(&prim, options.time);

    println!("attributes:");
    for attr_name in prim.get_attribute_names() {
        if let Some(attr) = prim.get_attribute(attr_name.as_str()) {
            println!(
                "  {} valid={} authored={} type={}",
                attr_name,
                attr.is_valid(),
                attr.has_authored_value(),
                attr.type_name()
            );
        }
    }

    Ok(())
}

/// Dump the local transform contract exactly as the composed stage sees it.
///
/// The BMW binary-format issue is currently suspected to be hierarchy or xform
/// related, so we print both `xformOpOrder` and the evaluated local matrix
/// instead of relying on viewer-side behavior.
fn dump_xformable(prim: &usd::usd::Prim, time: TimeCode) {
    let xformable = UsdGeomXformable::new(prim.clone());
    if !xformable.is_valid() {
        return;
    }

    let order_attr = xformable.get_xform_op_order_attr();
    let order = order_attr
        .get_typed_vec::<Token>(TimeCode::default())
        .unwrap_or_default();
    let mut resets_stack = false;
    let ops = xformable.get_ordered_xform_ops_with_reset(&mut resets_stack);
    let local = xformable.get_local_transformation(time);

    println!("xform:");
    println!("  xformOpOrder: {}", format_token_list(&order));
    println!("  resetsXformStack: {}", resets_stack);
    println!("  orderedOps: {}", ops.len());
    for op in ops {
        println!(
            "    op: {} inverse={} valueType={}",
            op.op_name(),
            op.is_inverse_op(),
            op.attr().type_name()
        );
    }
    println!("  localMatrix: {:?}", local);
}

fn dump_boundable(prim: &usd::usd::Prim, time: TimeCode) {
    let boundable = UsdGeomBoundable::new(prim.clone());
    if !boundable.is_valid() {
        return;
    }

    let extent_attr = boundable.get_extent_attr();
    let authored_extent = extent_attr.get_typed_vec::<Vec3f>(time).unwrap_or_default();
    let computed_extent = boundable.compute_extent(time).unwrap_or_default();

    println!("bounds:");
    println!(
        "  authoredExtent: {}",
        format_vec3_slice(&authored_extent, authored_extent.len())
    );
    println!(
        "  computedExtent: {}",
        format_vec3_slice(&computed_extent, computed_extent.len())
    );
}

/// Dump the composed `BBoxCache` bounds the viewer should use for framing.
///
/// Wrapper `Xform` assets often have no authored `extent`, so `Boundable`
/// output alone is not enough. Printing `BBoxCache` world/local/untransformed
/// bounds lets us see whether BMW-style package roots are being composed
/// incorrectly before the camera ever looks at them.
fn dump_composed_bounds(stage: &std::sync::Arc<Stage>, prim: &usd::usd::Prim, time: TimeCode) {
    let purposes = {
        let t = usd_geom_tokens();
        vec![t.default_.clone(), t.proxy.clone()]
    };
    let mut cache = BBoxCache::new(time, purposes, true, false);
    let world = cache.compute_world_bound(prim);
    let local = cache.compute_local_bound(prim);
    let untransformed = cache.compute_untransformed_bound(prim);

    println!("composedBounds:");
    dump_bbox("  world", world);
    dump_bbox("  local", local);
    dump_bbox("  untransformed", untransformed);

    let default_prim = stage.get_default_prim();
    if default_prim.is_valid() && default_prim.path() != prim.path() {
        let default_world = cache.compute_world_bound(&default_prim);
        println!(
            "  stageDefaultPrim: {} [{}]",
            default_prim.path(),
            default_prim.type_name()
        );
        dump_bbox("  defaultPrimWorld", default_world);
    }
}

fn dump_bbox(label: &str, bbox: usd_gf::BBox3d) {
    let range = bbox.compute_aligned_range();
    if range.is_empty() {
        println!("{label}: <empty>");
        return;
    }

    println!(
        "{label}: min={:?} max={:?} matrix={:?}",
        vec3d_tuple(range.min()),
        vec3d_tuple(range.max()),
        bbox.matrix()
    );
}

fn dump_mesh(prim: &usd::usd::Prim, time: TimeCode) {
    let mesh = UsdGeomMesh::new(prim.clone());
    if !mesh.is_valid() || prim.type_name().as_str() != "Mesh" {
        return;
    }

    let point_based = mesh.point_based();
    let gprim = point_based.gprim();

    let points = point_based
        .get_points_attr()
        .get_typed_vec::<Vec3f>(time)
        .unwrap_or_default();
    let normals = point_based
        .get_normals_attr()
        .get_typed_vec::<Vec3f>(time)
        .unwrap_or_default();
    let face_vertex_counts = mesh
        .get_face_vertex_counts_attr()
        .get_typed_vec::<i32>(time)
        .unwrap_or_default();
    let face_vertex_indices = mesh
        .get_face_vertex_indices_attr()
        .get_typed_vec::<i32>(time)
        .unwrap_or_default();
    let subdivision_scheme = mesh
        .get_subdivision_scheme_attr()
        .get_typed::<Token>(time)
        .unwrap_or_else(|| Token::new(""));
    let orientation = gprim
        .get_orientation_attr()
        .get_typed::<Token>(time)
        .unwrap_or_else(|| Token::new(""));
    let double_sided = gprim
        .get_double_sided_attr()
        .get_typed::<bool>(time)
        .unwrap_or(false);
    let display_color = gprim
        .get_display_color_primvar()
        .get_attr()
        .get_typed_vec::<Vec3f>(time)
        .unwrap_or_default();
    let display_color_interp = gprim.get_display_color_primvar().get_interpolation();
    let st_primvar = UsdGeomPrimvarsAPI::new(prim.clone()).get_primvar(&Token::new("st"));
    let st = st_primvar
        .get_attr()
        .get_typed_vec::<Vec2f>(time)
        .unwrap_or_default();
    let st_interp = if st_primvar.is_valid() {
        st_primvar.get_interpolation()
    } else {
        Token::new("")
    };

    println!("mesh:");
    println!(
        "  points: count={} sample={}",
        points.len(),
        format_vec3_slice(&points, 2)
    );
    println!(
        "  normals: count={} interpolation={} sample={}",
        normals.len(),
        point_based.get_normals_interpolation(),
        format_vec3_slice(&normals, 2)
    );
    println!(
        "  faceVertexCounts: count={} sample={}",
        face_vertex_counts.len(),
        format_i32_slice(&face_vertex_counts, 12)
    );
    println!(
        "  faceVertexIndices: count={} sample={}",
        face_vertex_indices.len(),
        format_i32_slice(&face_vertex_indices, 18)
    );
    println!("  subdivisionScheme: {}", subdivision_scheme);
    println!("  orientation: {}", orientation);
    println!("  doubleSided: {}", double_sided);
    println!(
        "  primvars:displayColor: count={} interpolation={} sample={}",
        display_color.len(),
        display_color_interp,
        format_vec3_slice(&display_color, 2)
    );
    println!(
        "  primvars:st: count={} interpolation={} sample={}",
        st.len(),
        st_interp,
        format_vec2_slice(&st, 2)
    );
}

fn format_time_code(time: TimeCode) -> String {
    if time.is_default() {
        "default".to_string()
    } else {
        time.value().to_string()
    }
}

fn format_token_list(tokens: &[Token]) -> String {
    let values: Vec<&str> = tokens.iter().map(Token::as_str).collect();
    format!("{:?}", values)
}

fn format_i32_slice(values: &[i32], limit: usize) -> String {
    let truncated: Vec<i32> = values.iter().take(limit).copied().collect();
    if values.len() > limit {
        format!("{:?} ...", truncated)
    } else {
        format!("{:?}", truncated)
    }
}

fn format_vec3_slice(values: &[Vec3f], limit: usize) -> String {
    let truncated: Vec<(f32, f32, f32)> = values
        .iter()
        .take(limit)
        .map(|value| (value.x, value.y, value.z))
        .collect();
    if values.len() > limit {
        format!("{:?} ...", truncated)
    } else {
        format!("{:?}", truncated)
    }
}

fn format_vec2_slice(values: &[Vec2f], limit: usize) -> String {
    let truncated: Vec<(f32, f32)> = values
        .iter()
        .take(limit)
        .map(|value| (value.x, value.y))
        .collect();
    if values.len() > limit {
        format!("{:?} ...", truncated)
    } else {
        format!("{:?}", truncated)
    }
}

fn vec3d_tuple(value: &Vec3d) -> (f64, f64, f64) {
    (value.x, value.y, value.z)
}
