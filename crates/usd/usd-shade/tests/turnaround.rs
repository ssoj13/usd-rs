/// Roundtrip (turnaround) tests: write a stage to disk, read back, verify data.
///
/// Test coverage:
///   1. usda_attribute_roundtrip  — write USDA, read back, verify prim/attr
///   2. usda_hierarchy            — nested prims survive USDA export/import
///   3. geometry_preservation     — mesh topology + points via USDA
///   4. transform_animation       — xformOps + time samples via USDA
///   5. material_preservation     — UsdPreviewSurface inputs via USDA
///   6. metadata_preservation     — upAxis, metersPerUnit, timeCodes via USDA
///   7. usdc_to_usda_hierarchy    — read real USDC, export to USDA, count prims
///   8. usdc_to_usda_mesh_types   — USDC -> USDA, verify Mesh prims present
///   9. usda_string_and_bool      — string/bool attrs survive roundtrip
///   10. usda_relationship        — relationship targets survive USDA roundtrip
///
/// Note: USDC *writing* roundtrip is not yet tested here because the USDC writer
/// (CrateWriter::write_paths) produces Lz4-compressed path-index tables that the
/// reader cannot decode. Tests that need USDC write are marked with a skip guard.
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::{
    Mesh, XformOpType, Xformable,
    metrics::{
        get_stage_meters_per_unit, get_stage_up_axis, set_stage_meters_per_unit, set_stage_up_axis,
    },
};
use usd_gf::{Vec3f, vec3f};
use usd_sdf::{Path as SdfPath, TimeCode, value_type_registry::ValueTypeRegistry};
use usd_shade::Shader;
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn data_dir() -> PathBuf {
    // Manifest at crates/usd/usd-shade/  ->  ../../../data  =  workspace/data
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("data")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("data"))
}

fn ensure_formats() {
    // One-time registration of USDA/USDC/USDZ file formats.
    usd_sdf::init();
}

fn new_stage() -> Arc<Stage> {
    ensure_formats();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create_in_memory")
}

fn open_stage(path: &std::path::Path) -> Arc<Stage> {
    ensure_formats();
    Stage::open(&*path.to_string_lossy(), InitialLoadSet::LoadAll)
        .unwrap_or_else(|e| panic!("open_stage {}: {e}", path.display()))
}

/// Export stage to USDA (always text — avoids the USDC writer offset bug).
fn export_usda(stage: &Stage, path: &std::path::Path) {
    assert!(
        path.extension().map_or(false, |e| e == "usda"),
        "export_usda: path must end in .usda"
    );
    stage
        .export(&*path.to_string_lossy(), false)
        .unwrap_or_else(|e| panic!("export to {}: {e}", path.display()));
}

fn vtype(name: &str) -> usd_sdf::ValueTypeName {
    ValueTypeRegistry::instance().find_type(name)
}

fn tcd() -> TimeCode {
    TimeCode::default()
}

fn assert_f32_slice_eq(label: &str, a: &[f32], b: &[f32], eps: f32) {
    assert_eq!(a.len(), b.len(), "{label}: length mismatch");
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert!(
            (x - y).abs() <= eps,
            "{label}[{i}]: {x} vs {y} (delta {})",
            (x - y).abs()
        );
    }
}

// ---------------------------------------------------------------------------
// 1. Basic attribute roundtrip via USDA
// ---------------------------------------------------------------------------

#[test]
fn usda_attribute_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("attr.usda");

    let stage = new_stage();
    let prim = stage.define_prim("/Root/Cube", "Cube").unwrap();

    let size_attr = prim
        .create_attribute("size", &vtype("double"), false, None)
        .expect("create size attr");
    size_attr.set(Value::from(4.5f64), tcd());

    let label_attr = prim
        .create_attribute("label", &vtype("string"), true, None)
        .expect("create label attr");
    label_attr.set(Value::from("hello world".to_string()), tcd());

    export_usda(&stage, &usda_path);
    assert!(usda_path.exists());

    let s2 = open_stage(&usda_path);
    let p2 = s2
        .get_prim_at_path(&SdfPath::from_string("/Root/Cube").unwrap())
        .expect("/Root/Cube missing");
    assert_eq!(p2.type_name().as_str(), "Cube");

    let size: f64 = p2
        .get_attribute("size")
        .and_then(|a| a.get_typed::<f64>(tcd()))
        .expect("size missing");
    assert!((size - 4.5).abs() < 1e-9, "size: {size}");

    let label: String = p2
        .get_attribute("label")
        .and_then(|a| a.get_typed::<String>(tcd()))
        .expect("label missing");
    assert_eq!(label, "hello world", "label mismatch");
}

// ---------------------------------------------------------------------------
// 2. Nested prim hierarchy via USDA
// ---------------------------------------------------------------------------

#[test]
fn usda_hierarchy() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("hier.usda");

    let stage = new_stage();
    stage.define_prim("/World", "Xform").unwrap();
    stage.define_prim("/World/Geo", "Scope").unwrap();
    stage.define_prim("/World/Geo/Mesh", "Mesh").unwrap();
    stage.define_prim("/World/Materials", "Scope").unwrap();

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    for path in [
        "/World",
        "/World/Geo",
        "/World/Geo/Mesh",
        "/World/Materials",
    ] {
        let p = s2
            .get_prim_at_path(&SdfPath::from_string(path).unwrap())
            .unwrap_or_else(|| panic!("{path} missing after USDA roundtrip"));
        assert!(p.is_valid(), "{path} invalid");
    }
}

// ---------------------------------------------------------------------------
// 3. Geometry preservation via USDA
// ---------------------------------------------------------------------------

#[test]
fn geometry_preservation() {
    let points_src: Vec<Vec3f> = vec![
        vec3f(0.0, 0.0, 0.0),
        vec3f(1.0, 0.0, 0.0),
        vec3f(1.0, 1.0, 0.0),
        vec3f(0.0, 1.0, 0.0),
    ];
    let face_counts_src: Vec<i32> = vec![4];
    let face_indices_src: Vec<i32> = vec![0, 1, 2, 3];

    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("quad.usda");

    let stage = new_stage();
    let mesh_path = SdfPath::from_string("/Quad").unwrap();
    let mesh = Mesh::define(&stage, &mesh_path);
    assert!(mesh.is_valid());

    mesh.create_face_vertex_counts_attr(None, false)
        .set(Value::from(face_counts_src.clone()), tcd());
    mesh.create_face_vertex_indices_attr(None, false)
        .set(Value::from(face_indices_src.clone()), tcd());
    mesh.point_based()
        .create_points_attr(None, false)
        .set(Value::from(points_src.clone()), tcd());

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let mesh2 = Mesh::get(&s2, &mesh_path);
    assert!(mesh2.is_valid(), "mesh2 invalid");

    let fc2: Vec<i32> = mesh2
        .get_face_vertex_counts_attr()
        .get_typed(tcd())
        .expect("face_vertex_counts missing");
    assert_eq!(fc2, face_counts_src, "face_counts mismatch");

    let fi2: Vec<i32> = mesh2
        .get_face_vertex_indices_attr()
        .get_typed(tcd())
        .expect("face_vertex_indices missing");
    assert_eq!(fi2, face_indices_src, "face_indices mismatch");

    let pts2: Vec<Vec3f> = mesh2
        .point_based()
        .get_points_attr()
        .get_typed(tcd())
        .expect("points missing");
    let pts2_flat: Vec<f32> = pts2.iter().flat_map(|v| [v[0], v[1], v[2]]).collect();
    let pts_src_flat: Vec<f32> = points_src.iter().flat_map(|v| [v[0], v[1], v[2]]).collect();
    assert_f32_slice_eq("points", &pts2_flat, &pts_src_flat, 1e-6);
}

// ---------------------------------------------------------------------------
// 4. Transform and animation preservation via USDA
// ---------------------------------------------------------------------------

#[test]
fn transform_animation() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("anim.usda");

    let stage = new_stage();
    stage.set_start_time_code(1.0);
    stage.set_end_time_code(3.0);

    let prim = stage.define_prim("/Mover", "Xform").unwrap();

    // Create xformOp:translate attribute directly (works around add_xform_op validity bug).
    let translate_attr = prim
        .create_attribute("xformOp:translate", &vtype("float3"), false, None)
        .expect("create xformOp:translate");
    translate_attr.set(Value::from(vec3f(0.0, 0.0, 0.0)), TimeCode::new(1.0));
    translate_attr.set(Value::from(vec3f(5.0, 0.0, 0.0)), TimeCode::new(2.0));
    translate_attr.set(Value::from(vec3f(10.0, 0.0, 0.0)), TimeCode::new(3.0));

    // Create xformOpOrder attribute
    let order_attr = prim
        .create_attribute("xformOpOrder", &vtype("token[]"), true, None)
        .expect("create xformOpOrder");
    let order_tokens: Vec<usd_tf::Token> = vec![Token::new("xformOp:translate")];
    order_attr.set(Value::new(order_tokens), tcd());

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let p2 = s2
        .get_prim_at_path(&SdfPath::from_string("/Mover").unwrap())
        .expect("/Mover missing");
    let xf2 = Xformable::new(p2);
    let ops2 = xf2.get_ordered_xform_ops();
    assert!(!ops2.is_empty(), "no xform ops after USDA");
    assert_eq!(ops2[0].op_type(), XformOpType::Translate, "op type wrong");

    let samples = ops2[0].get_time_samples();
    assert!(
        samples.len() >= 3,
        "expected >=3 time samples, got {}",
        samples.len()
    );

    let v2: Vec3f = ops2[0]
        .get_typed(TimeCode::new(2.0))
        .expect("translate at t=2 missing");
    assert!((v2[0] - 5.0).abs() < 1e-5, "translate.x at t=2: {}", v2[0]);

    let v3: Vec3f = ops2[0]
        .get_typed(TimeCode::new(3.0))
        .expect("translate at t=3 missing");
    assert!((v3[0] - 10.0).abs() < 1e-5, "translate.x at t=3: {}", v3[0]);
}

// ---------------------------------------------------------------------------
// 5. Material (UsdPreviewSurface) shader input preservation via USDA
// ---------------------------------------------------------------------------

#[test]
fn material_preservation() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("mat.usda");

    let stage = new_stage();
    let arc_stage = stage.clone();

    let shd_path = SdfPath::from_string("/Looks/Mat/Shader").unwrap();
    stage.define_prim("/Looks/Mat", "Material").unwrap();

    let shader = Shader::define(&arc_stage, &shd_path);
    assert!(shader.is_valid());
    shader.set_shader_id(&Token::new("UsdPreviewSurface"));

    let float_t = vtype("float");
    let color3f_t = vtype("color3f");

    shader
        .create_input(&Token::new("roughness"), &float_t)
        .set(Value::from(0.35f32), tcd());
    shader
        .create_input(&Token::new("metallic"), &float_t)
        .set(Value::from(0.0f32), tcd());
    shader
        .create_input(&Token::new("diffuseColor"), &color3f_t)
        .set(Value::from(vec3f(0.8, 0.2, 0.1)), tcd());

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let arc_s2 = s2.clone();
    let shader2 = Shader::get(&arc_s2, &shd_path);
    assert!(shader2.is_valid(), "shader2 invalid");

    let r2: f32 = shader2
        .get_input(&Token::new("roughness"))
        .get_value(tcd())
        .and_then(|v| v.downcast_clone::<f32>())
        .expect("roughness missing");
    assert!((r2 - 0.35).abs() < 1e-5, "roughness: {r2}");

    let d2: Vec3f = shader2
        .get_input(&Token::new("diffuseColor"))
        .get_value(tcd())
        .and_then(|v| v.downcast_clone::<Vec3f>())
        .expect("diffuseColor missing");
    assert!((d2[0] - 0.8).abs() < 1e-5, "diffuseColor.r: {}", d2[0]);

    let m2: f32 = shader2
        .get_input(&Token::new("metallic"))
        .get_value(tcd())
        .and_then(|v| v.downcast_clone::<f32>())
        .expect("metallic missing");
    assert!((m2 - 0.0).abs() < 1e-5, "metallic: {m2}");
}

// ---------------------------------------------------------------------------
// 6. Metadata preservation via USDA
// ---------------------------------------------------------------------------

#[test]
fn metadata_preservation() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("meta.usda");

    let stage = new_stage();
    set_stage_up_axis(&stage, &Token::new("Z"));
    set_stage_meters_per_unit(&stage, 0.01);
    stage.set_start_time_code(1.0);
    stage.set_end_time_code(100.0);
    stage.set_time_codes_per_second(24.0);

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    assert_eq!(get_stage_up_axis(&s2).as_str(), "Z", "upAxis mismatch");
    let mpu = get_stage_meters_per_unit(&s2);
    assert!((mpu - 0.01).abs() < 1e-9, "metersPerUnit: {mpu}");
    assert!((s2.get_time_codes_per_second() - 24.0).abs() < 1e-9, "tcps");
    assert_eq!(s2.get_start_time_code() as i64, 1, "startTimeCode");
    assert_eq!(s2.get_end_time_code() as i64, 100, "endTimeCode");
}

// ---------------------------------------------------------------------------
// 7. Real file: bmw_x3.usdc -> USDA -> verify prim count
// ---------------------------------------------------------------------------

#[test]
fn usdc_to_usda_hierarchy() {
    let src = data_dir().join("bmw_x3.usdc");
    if !src.exists() {
        eprintln!("SKIP usdc_to_usda_hierarchy: {} not found", src.display());
        return;
    }

    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("bmw.usda");

    let stage = open_stage(&src);
    let prims_before: Vec<_> = stage
        .traverse()
        .into_iter()
        .filter(|p| p.is_defined())
        .collect();
    assert!(!prims_before.is_empty(), "USDC stage has no defined prims");

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let prims_after: Vec<_> = s2
        .traverse()
        .into_iter()
        .filter(|p| p.is_defined())
        .collect();

    assert_eq!(
        prims_before.len(),
        prims_after.len(),
        "prim count usdc->usda: {} vs {}",
        prims_before.len(),
        prims_after.len()
    );

    // Verify output is valid USDA text.
    let content = std::fs::read_to_string(&usda_path).expect("read USDA");
    assert!(content.contains("#usda"), "USDA missing header");
}

// ---------------------------------------------------------------------------
// 8. Real file: bmw_x3.usdc -> USDA -> verify Mesh prims present
// ---------------------------------------------------------------------------

#[test]
fn usdc_to_usda_mesh_types() {
    let src = data_dir().join("bmw_x3.usdc");
    if !src.exists() {
        eprintln!("SKIP usdc_to_usda_mesh_types: {} not found", src.display());
        return;
    }

    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("bmw_types.usda");

    let stage = open_stage(&src);
    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let has_mesh = s2.traverse().into_iter().any(|p| p.type_name() == "Mesh");
    assert!(has_mesh, "no Mesh prims after usdc->usda");
}

// ---------------------------------------------------------------------------
// 9. String and bool attributes survive USDA roundtrip
// ---------------------------------------------------------------------------

#[test]
fn usda_string_and_bool() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("types.usda");

    let stage = new_stage();
    let prim = stage.define_prim("/Typed", "Scope").unwrap();

    prim.create_attribute("flag", &vtype("bool"), true, None)
        .unwrap()
        .set(Value::from(true), tcd());

    prim.create_attribute("name", &vtype("string"), true, None)
        .unwrap()
        .set(Value::from("test_value".to_string()), tcd());

    prim.create_attribute("count", &vtype("int"), true, None)
        .unwrap()
        .set(Value::from(42i32), tcd());

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let p2 = s2
        .get_prim_at_path(&SdfPath::from_string("/Typed").unwrap())
        .expect("/Typed missing");

    let flag: bool = p2
        .get_attribute("flag")
        .and_then(|a| a.get_typed::<bool>(tcd()))
        .expect("flag missing");
    assert!(flag, "flag should be true");

    let name: String = p2
        .get_attribute("name")
        .and_then(|a| a.get_typed::<String>(tcd()))
        .expect("name missing");
    assert_eq!(name, "test_value", "name mismatch");

    let count: i32 = p2
        .get_attribute("count")
        .and_then(|a| a.get_typed::<i32>(tcd()))
        .expect("count missing");
    assert_eq!(count, 42, "count mismatch");
}

// ---------------------------------------------------------------------------
// 10. Relationship targets survive USDA roundtrip
// ---------------------------------------------------------------------------

#[test]
fn usda_relationship() {
    let tmp = TempDir::new().unwrap();
    let usda_path = tmp.path().join("rel.usda");

    let stage = new_stage();
    stage.define_prim("/Geo/Mesh", "Mesh").unwrap();
    let _mat_prim = stage.define_prim("/Looks/Mat", "Material").unwrap();

    // Add a relationship on /Geo/Mesh pointing to /Looks/Mat.
    let mesh_prim = stage
        .get_prim_at_path(&SdfPath::from_string("/Geo/Mesh").unwrap())
        .unwrap();
    let rel = mesh_prim
        .create_relationship("material:binding", false)
        .expect("create_relationship");

    let mat_path = SdfPath::from_string("/Looks/Mat").unwrap();
    rel.add_target(&mat_path);

    export_usda(&stage, &usda_path);

    let s2 = open_stage(&usda_path);
    let mesh2 = s2
        .get_prim_at_path(&SdfPath::from_string("/Geo/Mesh").unwrap())
        .expect("/Geo/Mesh missing");
    let rel2 = mesh2
        .get_relationship("material:binding")
        .expect("relationship missing");
    let targets = rel2.get_targets();
    assert!(!targets.is_empty(), "relationship has no targets");
    assert_eq!(
        targets[0].as_str(),
        "/Looks/Mat",
        "target path mismatch: {}",
        targets[0].as_str()
    );
}

#[test]
#[ignore]
fn debug_usda_content() {
    ensure_formats();
    let stage = new_stage();
    let prim = stage.define_prim("/Root/Cube", "Cube").unwrap();
    let attr = prim
        .create_attribute("size", &vtype("double"), false, None)
        .expect("create size attr");
    attr.set(Value::from(4.5f64), tcd());
    let s = stage.export_to_string(false).unwrap();
    println!("USDA:\n{s}");
}
