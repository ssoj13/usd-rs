//! Alembic data writer implementation.
//!
//! Port of `pxr/usd/plugin/usdAbc/alembicWriter.{cpp,h}`.
//!
//! Walks the SdfAbstractData layer, maps each USD prim type to the
//! corresponding Alembic output schema (OPolyMesh, OXform, OSubD, …),
//! writes time-sampled properties and xform ops, then serialises the
//! archive via `alembic-rs` / Ogawa.

use std::collections::HashMap;
use std::sync::Arc;

use alembic::abc::OArchive;
use alembic::abc_core::TimeSampling;
use alembic::geom::{
    BasisType, CameraSample, CurvePeriodicity, CurveType, OCurves, OCurvesSample, OPoints,
    OPointsSample, OPolyMesh, OPolyMeshSample, OSubD, OSubDSample, OXform, OXformSample,
};
use alembic::ogawa::writer::OObject;
use alembic::ogawa::writer::schema::{OCamera, OFaceSet, OFaceSetSample};
use alembic::util::Mat4;
use usd_gf::matrix4::Matrix4;
use usd_gf::vec2::Vec2f;
use usd_gf::vec3::Vec3f;
use usd_tf::Token;
use usd_vt::Value;

use super::abstract_data::AbstractData;
use super::path::Path;

// ============================================================================
// AlembicDataWriter
// ============================================================================

/// Alembic writer matching C++ `UsdAbc_AlembicDataWriter`.
pub struct AlembicDataWriter {
    file_path: Option<String>,
    errors: Vec<String>,
    flags: HashMap<Token, bool>,
    is_valid: bool,
    /// Opened OArchive — held until close().
    archive: Option<OArchive>,
}

impl AlembicDataWriter {
    pub fn new() -> Self {
        Self {
            file_path: None,
            errors: Vec::new(),
            flags: HashMap::new(),
            is_valid: false,
            archive: None,
        }
    }

    /// Open an Alembic file for writing. Matches C++ `Open()`.
    pub fn open(&mut self, file_path: &str, _comment: &str) -> bool {
        self.errors.clear();
        self.file_path = Some(file_path.to_string());

        // Create parent directories if needed.
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    self.errors.push(format!("Cannot create directory: {e}"));
                    return false;
                }
            }
        }

        match OArchive::create(file_path) {
            Ok(archive) => {
                self.archive = Some(archive);
                self.is_valid = true;
                true
            }
            Err(e) => {
                self.errors
                    .push(format!("Cannot open Alembic archive: {e}"));
                false
            }
        }
    }

    /// Write an AbstractData layer into the archive. Matches C++ `Write()`.
    pub fn write(&mut self, data: &Arc<dyn AbstractData>) -> bool {
        if !self.is_valid {
            self.errors.push("Writer not opened".to_string());
            return false;
        }
        let Some(archive) = self.archive.as_mut() else {
            return false;
        };

        // Register acyclic time sampling if there are animated properties.
        let all_times: Vec<f64> = data.list_all_time_samples_vec();
        let ts_index: u32 = if all_times.is_empty() {
            0 // static / identity
        } else {
            archive.addTimeSampling(TimeSampling::acyclic(all_times))
        };

        // Build the root OObject tree, then serialise.
        let mut root = OObject::new("");
        write_prim(data.as_ref(), &Path::absolute_root(), &mut root, ts_index);

        match archive.write_archive(&root) {
            Ok(()) => true,
            Err(e) => {
                self.errors.push(format!("Archive write failed: {e}"));
                false
            }
        }
    }

    /// Close the archive. Matches C++ `Close()`.
    pub fn close(&mut self) -> bool {
        // Dropping OArchive flushes all data to disk.
        self.archive = None;
        let was_valid = self.is_valid;
        self.is_valid = false;
        self.file_path = None;
        was_valid
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn get_errors(&self) -> Option<String> {
        if self.errors.is_empty() {
            None
        } else {
            Some(self.errors.join("; "))
        }
    }

    pub fn set_flag(&mut self, flag: Token, set: bool) {
        self.flags.insert(flag, set);
    }
}

impl Default for AlembicDataWriter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SpecVisitor that collects prim paths (used to validate hierarchy)
// ============================================================================

// ============================================================================
// Recursive prim writer
// ============================================================================

fn tok(s: &str) -> Token {
    Token::new(s)
}

/// Recursively write a USD prim into an Alembic OObject parent.
fn write_prim(data: &dyn AbstractData, path: &Path, parent: &mut OObject, ts_index: u32) {
    // Pseudo-root: only descend into children.
    if path.is_absolute_root_path() {
        for child_path in prim_children(data, path) {
            write_prim(data, &child_path, parent, ts_index);
        }
        return;
    }

    let type_name = get_token_field(data, path, "typeName")
        .map(|t| t.to_string())
        .unwrap_or_default();

    let obj = match type_name.as_str() {
        "Mesh" => {
            // subdivisionScheme == "none" → PolyMesh, else SubD.
            let scheme = get_prop_default_token(data, path, "subdivisionScheme");
            if scheme.as_deref() == Some("none") {
                build_poly_mesh(data, path, ts_index)
            } else {
                build_subd(data, path, ts_index)
            }
        }
        "BasisCurves" | "NurbsCurves" => build_curves(data, path, ts_index),
        "Points" => build_points(data, path, ts_index),
        "Camera" => build_camera(data, path, ts_index),
        "GeomSubset" => build_faceset(data, path, ts_index),
        // Xform, Scope, unknown types — emit as OXform to preserve hierarchy.
        _ => build_xform(data, path, ts_index),
    };

    // Add to parent; then write children into the newly-added child.
    let added = parent.add_child(obj);
    for child_path in prim_children(data, path) {
        write_prim(data, &child_path, added, ts_index);
    }
}

// ============================================================================
// Field helpers
// ============================================================================

fn get_token_field(data: &dyn AbstractData, path: &Path, field: &str) -> Option<Token> {
    data.get_field(path, &tok(field))
        .and_then(|v| v.downcast_clone::<Token>())
}

/// Get a default-value token from a USD property spec.
fn get_prop_default_token(
    data: &dyn AbstractData,
    prim_path: &Path,
    prop_name: &str,
) -> Option<String> {
    let prop_path = prim_path.append_property(prop_name)?;
    data.get_field(&prop_path, &tok("default"))
        .and_then(|v| v.downcast_clone::<Token>())
        .map(|t| t.to_string())
}

/// List ordered child prim paths.
fn prim_children(data: &dyn AbstractData, path: &Path) -> Vec<Path> {
    let Some(val) = data.get_field(path, &tok("primChildren")) else {
        return Vec::new();
    };
    if let Some(arr) = val.as_vec_clone::<Token>() {
        arr.iter()
            .filter_map(|name| path.append_child(name.as_str()))
            .collect()
    } else {
        Vec::new()
    }
}

/// Get the static default value for a USD property, or first time sample.
fn get_prop_value(data: &dyn AbstractData, prim_path: &Path, prop_name: &str) -> Option<Value> {
    let prop_path = prim_path.append_property(prop_name)?;
    // Prefer time samples, fall back to default.
    let times = data.list_time_samples_for_path_vec(&prop_path);
    if let Some(&t) = times.first() {
        return data.query_time_sample(&prop_path, t);
    }
    data.get_field(&prop_path, &tok("default"))
}

/// Get all time samples for a property as (time, value) pairs.
fn get_all_time_samples(
    data: &dyn AbstractData,
    prim_path: &Path,
    prop_name: &str,
) -> Vec<(f64, Value)> {
    let Some(prop_path) = prim_path.append_property(prop_name) else {
        return Vec::new();
    };
    data.list_time_samples_for_path_vec(&prop_path)
        .into_iter()
        .filter_map(|t| data.query_time_sample(&prop_path, t).map(|v| (t, v)))
        .collect()
}

// ============================================================================
// Type conversion helpers
// ============================================================================

/// Downcast Value to Vec<glam::Vec3> (via Array<Vec3f>).
fn value_to_vec3(val: &Value) -> Vec<alembic::util::Vec3> {
    if let Some(arr) = val.as_vec_clone::<Vec3f>() {
        return arr
            .iter()
            .map(|v| alembic::util::Vec3::new(v.x, v.y, v.z))
            .collect();
    }
    Vec::new()
}

/// Downcast Value to Vec<i32> (via Array<i32>).
fn value_to_i32_arr(val: &Value) -> Vec<i32> {
    val.as_vec_clone::<i32>().unwrap_or_default()
}

/// Downcast Value to Vec<i64> (via Array<i64> or Array<i32>).
fn value_to_i64_arr(val: &Value) -> Vec<i64> {
    if let Some(arr) = val.as_vec_clone::<i64>() {
        return arr;
    }
    if let Some(arr) = val.as_vec_clone::<i32>() {
        return arr.iter().map(|&x| x as i64).collect();
    }
    Vec::new()
}

/// Downcast Value to Vec<glam::Vec2> (via Array<Vec2f>).
fn value_to_vec2(val: &Value) -> Vec<alembic::util::Vec2> {
    if let Some(arr) = val.as_vec_clone::<Vec2f>() {
        return arr
            .iter()
            .map(|v| alembic::util::Vec2::new(v.x, v.y))
            .collect();
    }
    Vec::new()
}

/// Downcast Value to Vec<f32> (via Array<f32>).
fn value_to_f32_arr(val: &Value) -> Vec<f32> {
    val.as_vec_clone::<f32>().unwrap_or_default()
}

/// Convert a Matrix4d Value to alembic::util::Mat4.
fn value_to_mat4(val: &Value) -> Option<Mat4> {
    let m = val.downcast::<Matrix4<f64>>()?;
    // Matrix4 uses row-vector convention; rows = m.row(i).
    // alembic Mat4 is glam::Mat4 (column-major).
    let r0 = m.row(0);
    let r1 = m.row(1);
    let r2 = m.row(2);
    let r3 = m.row(3);
    Some(Mat4::from_cols_array(&[
        r0.x as f32,
        r0.y as f32,
        r0.z as f32,
        r0.w as f32,
        r1.x as f32,
        r1.y as f32,
        r1.z as f32,
        r1.w as f32,
        r2.x as f32,
        r2.y as f32,
        r2.z as f32,
        r2.w as f32,
        r3.x as f32,
        r3.y as f32,
        r3.z as f32,
        r3.w as f32,
    ]))
}

/// Read an f64 property (with f32 fallback).
fn get_f64_prop(data: &dyn AbstractData, prim_path: &Path, prop_name: &str) -> f64 {
    get_prop_value(data, prim_path, prop_name)
        .and_then(|v| {
            v.downcast_clone::<f64>()
                .or_else(|| v.downcast_clone::<f32>().map(|f| f as f64))
        })
        .unwrap_or(0.0)
}

// ============================================================================
// Schema builders
// ============================================================================

/// Build an OXform from a USD Xform/Scope/unknown prim.
fn build_xform(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut oxform = OXform::new(&name);
    oxform.set_time_sampling(ts_index);

    let mat_samples = get_all_time_samples(data, path, "xformOp:transform");
    if mat_samples.is_empty() {
        // Static or no xformOp: write one sample (identity or authored).
        let mat = get_prop_value(data, path, "xformOp:transform")
            .and_then(|v| value_to_mat4(&v))
            .unwrap_or(Mat4::IDENTITY);
        oxform.add_sample(OXformSample::from_matrix(mat, true));
    } else {
        for (_t, val) in &mat_samples {
            let mat = value_to_mat4(val).unwrap_or(Mat4::IDENTITY);
            oxform.add_sample(OXformSample::from_matrix(mat, true));
        }
    }

    oxform.build()
}

/// Build an OPolyMesh from a USD Mesh (subdivisionScheme == "none").
fn build_poly_mesh(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut omesh = OPolyMesh::new(&name);
    omesh.set_time_sampling(ts_index);

    let fc = value_to_i32_arr(&get_prop_value(data, path, "faceVertexCounts").unwrap_or_default());
    let fi = value_to_i32_arr(&get_prop_value(data, path, "faceVertexIndices").unwrap_or_default());
    let normals_val = get_prop_value(data, path, "normals");
    let uv_val = get_prop_value(data, path, "primvars:st")
        .or_else(|| get_prop_value(data, path, "primvars:uv"));

    let pt_samples = get_all_time_samples(data, path, "points");

    let build_sample = |positions: Vec<alembic::util::Vec3>| {
        let mut s = OPolyMeshSample::new(positions, fc.clone(), fi.clone());
        s.normals = normals_val.as_ref().map(|v| value_to_vec3(v));
        s.uvs = uv_val.as_ref().map(|v| value_to_vec2(v));
        s
    };

    if pt_samples.is_empty() {
        let pts = value_to_vec3(&get_prop_value(data, path, "points").unwrap_or_default());
        omesh.add_sample(&build_sample(pts));
    } else {
        for (_t, pt_val) in &pt_samples {
            omesh.add_sample(&build_sample(value_to_vec3(pt_val)));
        }
    }

    omesh.build()
}

/// Build an OSubD from a USD Mesh with a subdivision scheme.
fn build_subd(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut osubd = OSubD::new(&name);
    osubd.set_time_sampling(ts_index);

    let scheme = get_prop_default_token(data, path, "subdivisionScheme")
        .unwrap_or_else(|| "catmull-clark".to_string());

    let fc = value_to_i32_arr(&get_prop_value(data, path, "faceVertexCounts").unwrap_or_default());
    let fi = value_to_i32_arr(&get_prop_value(data, path, "faceVertexIndices").unwrap_or_default());
    let uv_val = get_prop_value(data, path, "primvars:st")
        .or_else(|| get_prop_value(data, path, "primvars:uv"));

    let crease_indices = get_prop_value(data, path, "creaseIndices").map(|v| value_to_i32_arr(&v));
    let crease_lengths = get_prop_value(data, path, "creaseLengths").map(|v| value_to_i32_arr(&v));
    let crease_sharp =
        get_prop_value(data, path, "creaseSharpnesses").map(|v| value_to_f32_arr(&v));
    let corner_indices = get_prop_value(data, path, "cornerIndices").map(|v| value_to_i32_arr(&v));
    let corner_sharp =
        get_prop_value(data, path, "cornerSharpnesses").map(|v| value_to_f32_arr(&v));
    let holes = get_prop_value(data, path, "holeIndices").map(|v| value_to_i32_arr(&v));

    let pt_samples = get_all_time_samples(data, path, "points");

    let build_sample = |positions: Vec<alembic::util::Vec3>| {
        let mut s = OSubDSample::new(positions, fc.clone(), fi.clone()).with_scheme(&scheme);
        s.uvs = uv_val.as_ref().map(|v| value_to_vec2(v));
        s.crease_indices = crease_indices.clone();
        s.crease_lengths = crease_lengths.clone();
        s.crease_sharpnesses = crease_sharp.clone();
        s.corner_indices = corner_indices.clone();
        s.corner_sharpnesses = corner_sharp.clone();
        s.holes = holes.clone();
        s
    };

    if pt_samples.is_empty() {
        let pts = value_to_vec3(&get_prop_value(data, path, "points").unwrap_or_default());
        osubd.add_sample(&build_sample(pts));
    } else {
        for (_t, pt_val) in &pt_samples {
            osubd.add_sample(&build_sample(value_to_vec3(pt_val)));
        }
    }

    osubd.build()
}

/// Build OCurves from a USD BasisCurves/NurbsCurves prim.
fn build_curves(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut ocurves = OCurves::new(&name);
    ocurves.set_time_sampling(ts_index);

    let type_name = get_token_field(data, path, "typeName")
        .map(|t| t.to_string())
        .unwrap_or_default();

    // USD BasisCurves are always cubic; NurbsCurves use variable-order (map to Cubic).
    let curve_type = if type_name == "NurbsCurves" {
        CurveType::Linear // NurbsCurves mapped to linear; Alembic handles knots separately
    } else {
        CurveType::Cubic
    };

    let basis_str =
        get_prop_default_token(data, path, "basis").unwrap_or_else(|| "bezier".to_string());
    let basis = match basis_str.as_str() {
        "bspline" => BasisType::Bspline,
        "catmullRom" => BasisType::CatmullRom,
        "hermite" => BasisType::Hermite,
        "power" => BasisType::Power,
        _ => BasisType::Bezier,
    };

    let wrap_str =
        get_prop_default_token(data, path, "wrap").unwrap_or_else(|| "nonperiodic".to_string());
    let wrap = if wrap_str == "periodic" {
        CurvePeriodicity::Periodic
    } else {
        CurvePeriodicity::NonPeriodic
    };

    let num_verts =
        value_to_i32_arr(&get_prop_value(data, path, "curveVertexCounts").unwrap_or_default());
    let normals_val = get_prop_value(data, path, "normals");

    let pt_samples = get_all_time_samples(data, path, "points");

    let build_sample = |positions: Vec<alembic::util::Vec3>| OCurvesSample {
        positions,
        num_vertices: num_verts.clone(),
        curve_type,
        basis,
        wrap,
        velocities: None,
        widths: None,
        normals: normals_val.as_ref().map(|v| value_to_vec3(v)),
        uvs: None,
        knots: None,
        orders: None,
    };

    if pt_samples.is_empty() {
        let pts = value_to_vec3(&get_prop_value(data, path, "points").unwrap_or_default());
        ocurves.add_sample(&build_sample(pts));
    } else {
        for (_t, pt_val) in &pt_samples {
            ocurves.add_sample(&build_sample(value_to_vec3(&pt_val)));
        }
    }

    ocurves.build()
}

/// Build OPoints from a USD Points prim.
fn build_points(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut opoints = OPoints::new(&name);
    opoints.set_time_sampling(ts_index);

    let ids = value_to_i64_arr(&get_prop_value(data, path, "ids").unwrap_or_default());
    let pt_samples = get_all_time_samples(data, path, "points");

    let build_sample = |positions: Vec<alembic::util::Vec3>| {
        let sample_ids = if ids.is_empty() {
            (0..positions.len() as i64).collect()
        } else {
            ids.clone()
        };
        OPointsSample::new(positions, sample_ids)
    };

    if pt_samples.is_empty() {
        let pts = value_to_vec3(&get_prop_value(data, path, "points").unwrap_or_default());
        opoints.add_sample(&build_sample(pts));
    } else {
        for (_t, pt_val) in &pt_samples {
            opoints.add_sample(&build_sample(value_to_vec3(&pt_val)));
        }
    }

    opoints.build()
}

/// Build OCamera from a USD Camera prim.
fn build_camera(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut ocam = OCamera::new(&name);
    ocam.set_time_sampling(ts_index);

    let mut sample = CameraSample::default();

    // USD stores lengths in mm; Alembic uses cm → divide by 10.
    let fl = get_f64_prop(data, path, "focalLength");
    if fl > 0.0 {
        sample.focal_length = fl / 10.0;
    }

    let ha = get_f64_prop(data, path, "horizontalAperture");
    if ha > 0.0 {
        sample.horizontal_aperture = ha / 10.0;
    }

    let va = get_f64_prop(data, path, "verticalAperture");
    if va > 0.0 {
        sample.vertical_aperture = va / 10.0;
    }

    let ho = get_f64_prop(data, path, "horizontalApertureOffset");
    if ho != 0.0 {
        sample.horizontal_film_offset = ho / 10.0;
    }

    let vo = get_f64_prop(data, path, "verticalApertureOffset");
    if vo != 0.0 {
        sample.vertical_film_offset = vo / 10.0;
    }

    let fs = get_f64_prop(data, path, "fStop");
    if fs > 0.0 {
        sample.f_stop = fs;
    }

    let fd = get_f64_prop(data, path, "focusDistance");
    if fd > 0.0 {
        sample.focus_distance = fd;
    }

    // clippingRange is stored as GfVec2f; read x/y separately.
    let near = get_f64_prop(data, path, "clippingRange.x");
    let far = get_f64_prop(data, path, "clippingRange.y");
    if near > 0.0 {
        sample.near_clipping_plane = near;
    }
    if far > 0.0 {
        sample.far_clipping_plane = far;
    }

    let so = get_f64_prop(data, path, "shutterOpen");
    let sc = get_f64_prop(data, path, "shutterClose");
    if sc > 0.0 {
        sample.shutter_open = so;
        sample.shutter_close = sc;
    }

    ocam.add_sample(sample);
    ocam.build()
}

/// Build OFaceSet from a USD GeomSubset prim.
fn build_faceset(data: &dyn AbstractData, path: &Path, ts_index: u32) -> OObject {
    let name = path.get_name().to_string();
    let mut ofs = OFaceSet::new(&name);
    ofs.set_time_sampling(ts_index);

    let faces = value_to_i32_arr(&get_prop_value(data, path, "indices").unwrap_or_default());
    ofs.add_sample(&OFaceSetSample { faces });

    ofs.build()
}
