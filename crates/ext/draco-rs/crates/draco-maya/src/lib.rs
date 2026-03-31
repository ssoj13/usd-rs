//! Maya C-ABI bridge for the Draco Rust port.
//!
//! This crate mirrors `_ref/draco/src/draco/maya/draco_maya_plugin.*` and exposes
//! the same C ABI so Maya/Python tooling can consume `.drc` meshes without a C++
//! dependency. The functions here are intended to be compiled as a shared
//! library and loaded by Maya (or Python ctypes) exactly like the reference.

use std::ffi::{c_char, CStr};
use std::fs::File;
use std::io::Write;
use std::ptr;
use std::slice;

use draco_bitstream::compression::config::compression_shared::EncodedGeometryType;
use draco_bitstream::compression::decode::Decoder as DracoDecoder;
use draco_bitstream::compression::encode::Encoder as DracoEncoder;
use draco_core::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use draco_core::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::mesh::mesh::Mesh;

/// Result codes for Maya encode API (matches C++ enum values).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeResult {
    Ok = 0,
    KoWrongInput = -1,
    KoMeshEncoding = -2,
    KoFileCreation = -3,
}

/// Result codes for Maya decode API (matches C++ enum values).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeResult {
    Ok = 0,
    KoGeometryTypeInvalid = -1,
    KoTriangularMeshNotFound = -2,
    KoMeshDecoding = -3,
}

/// C ABI mesh payload used by the Maya/Python plugin interface.
#[repr(C)]
pub struct Drc2PyMesh {
    pub faces_num: i32,
    pub faces: *mut i32,
    pub vertices_num: i32,
    pub vertices: *mut f32,
    pub normals_num: i32,
    pub normals: *mut f32,
    pub uvs_num: i32,
    pub uvs_real_num: i32,
    pub uvs: *mut f32,
}

impl Default for Drc2PyMesh {
    fn default() -> Self {
        Self {
            faces_num: 0,
            faces: ptr::null_mut(),
            vertices_num: 0,
            vertices: ptr::null_mut(),
            normals_num: 0,
            normals: ptr::null_mut(),
            uvs_num: 0,
            uvs_real_num: 0,
            uvs: ptr::null_mut(),
        }
    }
}

// --- Internal helpers -----------------------------------------------------

/// Leaks a Vec into a raw pointer so it can cross the C ABI boundary.
fn vec_into_raw<T>(mut data: Vec<T>) -> *mut T {
    let ptr = data.as_mut_ptr();
    std::mem::forget(data);
    ptr
}

/// Reclaims a raw pointer that was produced by `vec_into_raw`.
unsafe fn free_raw_array<T>(ptr: *mut T, len: usize) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer/len pair must match a Vec previously leaked via vec_into_raw.
    drop(Vec::from_raw_parts(ptr, len, len));
}

fn decode_faces(mesh: &Mesh) -> Vec<i32> {
    let num_faces = mesh.num_faces() as usize;
    let mut faces = vec![0i32; num_faces * 3];
    for i in 0..num_faces {
        let face = mesh.face(FaceIndex::from(i as u32));
        let base = i * 3;
        faces[base] = face[0].value() as i32;
        faces[base + 1] = face[1].value() as i32;
        faces[base + 2] = face[2].value() as i32;
    }
    faces
}

fn decode_attribute_f32_3(mesh: &Mesh, attr_type: GeometryAttributeType) -> (Vec<f32>, i32) {
    let att = match mesh.get_named_attribute(attr_type) {
        Some(att) => att,
        None => return (Vec::new(), 0),
    };
    let num_points = mesh.num_points() as usize;
    let mut values = vec![0.0f32; num_points * 3];
    for i in 0..num_points {
        let point = PointIndex::from(i as u32);
        let val_index = att.mapped_index(point);
        let mut tmp = [0.0f32; 3];
        if !att.convert_value(val_index, 3, &mut tmp) {
            // Mirror reference behavior: abort filling but keep the partially filled buffer.
            return (values, num_points as i32);
        }
        let base = i * 3;
        values[base] = tmp[0];
        values[base + 1] = tmp[1];
        values[base + 2] = tmp[2];
    }
    (values, num_points as i32)
}

fn decode_attribute_f32_2(mesh: &Mesh, attr_type: GeometryAttributeType) -> (Vec<f32>, i32, i32) {
    let att = match mesh.get_named_attribute(attr_type) {
        Some(att) => att,
        None => return (Vec::new(), 0, 0),
    };
    let num_points = mesh.num_points() as usize;
    let mut values = vec![0.0f32; num_points * 2];
    for i in 0..num_points {
        let point = PointIndex::from(i as u32);
        let val_index = att.mapped_index(point);
        let mut tmp = [0.0f32; 2];
        if !att.convert_value(val_index, 2, &mut tmp) {
            // Mirror reference behavior: abort filling but keep the partially filled buffer.
            return (values, num_points as i32, att.size() as i32);
        }
        let base = i * 2;
        values[base] = tmp[0];
        values[base + 1] = tmp[1];
    }
    (values, num_points as i32, att.size() as i32)
}

fn checked_len(count: i32, stride: usize) -> Option<usize> {
    if count <= 0 {
        return Some(0);
    }
    let count_u = count as usize;
    count_u.checked_mul(stride)
}

// --- C ABI ----------------------------------------------------------------

/// Releases memory allocated by `drc2py_decode`.
#[no_mangle]
pub unsafe extern "C" fn drc2py_free(mesh_ptr: *mut *mut Drc2PyMesh) {
    if mesh_ptr.is_null() {
        return;
    }
    let mesh = *mesh_ptr;
    if mesh.is_null() {
        return;
    }
    let mut mesh_box = Box::from_raw(mesh);

    if let Some(len) = checked_len(mesh_box.faces_num, 3) {
        free_raw_array(mesh_box.faces, len);
    }
    if let Some(len) = checked_len(mesh_box.vertices_num, 3) {
        free_raw_array(mesh_box.vertices, len);
    }
    if let Some(len) = checked_len(mesh_box.normals_num, 3) {
        free_raw_array(mesh_box.normals, len);
    }
    if let Some(len) = checked_len(mesh_box.uvs_num, 2) {
        free_raw_array(mesh_box.uvs, len);
    }

    // Clear pointers to mirror the reference cleanup behavior.
    mesh_box.faces = ptr::null_mut();
    mesh_box.vertices = ptr::null_mut();
    mesh_box.normals = ptr::null_mut();
    mesh_box.uvs = ptr::null_mut();
    mesh_box.faces_num = 0;
    mesh_box.vertices_num = 0;
    mesh_box.normals_num = 0;
    mesh_box.uvs_num = 0;
    mesh_box.uvs_real_num = 0;

    *mesh_ptr = ptr::null_mut();
}

/// Decodes a Draco mesh buffer into raw arrays consumable by Maya/Python.
#[no_mangle]
pub unsafe extern "C" fn drc2py_decode(
    data: *mut c_char,
    length: u32,
    res_mesh: *mut *mut Drc2PyMesh,
) -> DecodeResult {
    if res_mesh.is_null() {
        return DecodeResult::KoMeshDecoding;
    }
    *res_mesh = ptr::null_mut();

    let len = length as usize;
    if data.is_null() && len > 0 {
        return DecodeResult::KoGeometryTypeInvalid;
    }
    let data_slice = if len == 0 {
        &[]
    } else {
        // Safety: caller guarantees a valid buffer of `length` bytes.
        slice::from_raw_parts(data as *const u8, len)
    };

    let mut buffer = DecoderBuffer::new();
    buffer.init(data_slice);

    let geom_type_status = DracoDecoder::get_encoded_geometry_type(&mut buffer);
    if !geom_type_status.is_ok() {
        return DecodeResult::KoGeometryTypeInvalid;
    }
    let geom_type = geom_type_status.into_value();
    if geom_type != EncodedGeometryType::TriangularMesh {
        return DecodeResult::KoTriangularMeshNotFound;
    }

    let mut decoder = DracoDecoder::new();
    let mesh_status = decoder.decode_mesh_from_buffer(&mut buffer);
    if !mesh_status.is_ok() {
        return DecodeResult::KoMeshDecoding;
    }
    let mesh = mesh_status.into_value();

    // Marshall the mesh into flat arrays matching the C ABI contract.
    let faces = decode_faces(&mesh);
    let (vertices, vertices_num) = decode_attribute_f32_3(&mesh, GeometryAttributeType::Position);
    let (normals, normals_num) = decode_attribute_f32_3(&mesh, GeometryAttributeType::Normal);
    let (uvs, uvs_num, uvs_real_num) =
        decode_attribute_f32_2(&mesh, GeometryAttributeType::TexCoord);

    let mut out_mesh = Box::new(Drc2PyMesh::default());
    out_mesh.faces_num = mesh.num_faces() as i32;
    out_mesh.faces = vec_into_raw(faces);
    out_mesh.vertices_num = vertices_num;
    out_mesh.vertices = vec_into_raw(vertices);
    out_mesh.normals_num = normals_num;
    out_mesh.normals = vec_into_raw(normals);
    out_mesh.uvs_num = uvs_num;
    out_mesh.uvs_real_num = uvs_real_num;
    out_mesh.uvs = vec_into_raw(uvs);

    *res_mesh = Box::into_raw(out_mesh);
    DecodeResult::Ok
}

/// Encodes a Maya/Python mesh payload into a `.drc` file.
#[no_mangle]
pub unsafe extern "C" fn drc2py_encode(
    in_mesh: *const Drc2PyMesh,
    file_path: *const c_char,
) -> EncodeResult {
    if in_mesh.is_null() || file_path.is_null() {
        return EncodeResult::KoWrongInput;
    }
    let mesh = &*in_mesh;

    let faces_num = mesh.faces_num;
    let vertices_num = mesh.vertices_num;
    if faces_num <= 0 || vertices_num <= 0 {
        return EncodeResult::KoWrongInput;
    }

    let faces_len = match checked_len(faces_num, 3) {
        Some(len) => len,
        None => return EncodeResult::KoWrongInput,
    };
    let vertices_len = match checked_len(vertices_num, 3) {
        Some(len) => len,
        None => return EncodeResult::KoWrongInput,
    };

    let faces = if faces_len == 0 {
        &[]
    } else {
        if mesh.faces.is_null() {
            return EncodeResult::KoWrongInput;
        }
        slice::from_raw_parts(mesh.faces as *const i32, faces_len)
    };
    let vertices = if vertices_len == 0 {
        &[]
    } else {
        if mesh.vertices.is_null() {
            return EncodeResult::KoWrongInput;
        }
        slice::from_raw_parts(mesh.vertices as *const f32, vertices_len)
    };

    let normals_len = match checked_len(mesh.normals_num, 3) {
        Some(len) => len,
        None => return EncodeResult::KoWrongInput,
    };
    let normals = if normals_len == 0 {
        &[]
    } else {
        if mesh.normals.is_null() {
            return EncodeResult::KoWrongInput;
        }
        slice::from_raw_parts(mesh.normals as *const f32, normals_len)
    };

    let uvs_len = match checked_len(mesh.uvs_num, 2) {
        Some(len) => len,
        None => return EncodeResult::KoWrongInput,
    };
    let uvs = if uvs_len == 0 {
        &[]
    } else {
        if mesh.uvs.is_null() {
            return EncodeResult::KoWrongInput;
        }
        slice::from_raw_parts(mesh.uvs as *const f32, uvs_len)
    };

    // Build a Draco mesh from the flat Maya arrays.
    let mut drc_mesh = Mesh::new();
    drc_mesh.set_num_faces(faces_num as usize);
    drc_mesh.set_num_points(vertices_num as u32);

    for i in 0..faces_num as usize {
        let base = i * 3;
        let i0 = faces[base];
        let i1 = faces[base + 1];
        let i2 = faces[base + 2];
        if i0 < 0 || i1 < 0 || i2 < 0 {
            return EncodeResult::KoWrongInput;
        }
        let face = [
            PointIndex::from(i0 as u32),
            PointIndex::from(i1 as u32),
            PointIndex::from(i2 as u32),
        ];
        drc_mesh.set_face(FaceIndex::from(i as u32), face);
    }

    let mut pos_attr = GeometryAttribute::new();
    pos_attr.init(
        GeometryAttributeType::Position,
        None,
        3,
        DataType::Float32,
        false,
        (std::mem::size_of::<f32>() * 3) as i64,
        0,
    );
    let pos_att_id = drc_mesh.add_attribute_from_geometry(&pos_attr, true, vertices_num as u32);
    if pos_att_id < 0 {
        return EncodeResult::KoMeshEncoding;
    }
    if let Some(att) = drc_mesh.attribute(pos_att_id) {
        for i in 0..vertices_num as usize {
            let base = i * 3;
            let point = [vertices[base], vertices[base + 1], vertices[base + 2]];
            att.set_attribute_value_array(AttributeValueIndex::from(i as u32), &point);
        }
    }

    if mesh.normals_num > 0 {
        let mut norm_attr = GeometryAttribute::new();
        norm_attr.init(
            GeometryAttributeType::Normal,
            None,
            3,
            DataType::Float32,
            false,
            (std::mem::size_of::<f32>() * 3) as i64,
            0,
        );
        let norm_att_id =
            drc_mesh.add_attribute_from_geometry(&norm_attr, true, mesh.normals_num as u32);
        if norm_att_id < 0 {
            return EncodeResult::KoMeshEncoding;
        }
        if let Some(att) = drc_mesh.attribute(norm_att_id) {
            for i in 0..mesh.normals_num as usize {
                let base = i * 3;
                let normal = [normals[base], normals[base + 1], normals[base + 2]];
                att.set_attribute_value_array(AttributeValueIndex::from(i as u32), &normal);
            }
        }
    }

    if mesh.uvs_num > 0 {
        let mut uv_attr = GeometryAttribute::new();
        uv_attr.init(
            GeometryAttributeType::TexCoord,
            None,
            2,
            DataType::Float32,
            false,
            (std::mem::size_of::<f32>() * 2) as i64,
            0,
        );
        let uv_att_id = drc_mesh.add_attribute_from_geometry(&uv_attr, true, mesh.uvs_num as u32);
        if uv_att_id < 0 {
            return EncodeResult::KoMeshEncoding;
        }
        if let Some(att) = drc_mesh.attribute(uv_att_id) {
            for i in 0..mesh.uvs_num as usize {
                let base = i * 2;
                let uv = [uvs[base], uvs[base + 1]];
                att.set_attribute_value_array(AttributeValueIndex::from(i as u32), &uv);
            }
        }
    }

    // Deduplicate attributes and points (reference behavior behind build flags).
    let _ = drc_mesh.deduplicate_attribute_values();
    drc_mesh.deduplicate_point_ids();

    // Encode mesh into a Draco buffer using default settings.
    let mut encoder = DracoEncoder::new();
    let mut buffer = EncoderBuffer::new();
    let status = encoder.encode_mesh_to_buffer(&drc_mesh, &mut buffer);
    if !status.is_ok() {
        return EncodeResult::KoMeshEncoding;
    }

    // Write buffer to disk.
    let path = CStr::from_ptr(file_path).to_string_lossy();
    let mut file = match File::create(path.as_ref()) {
        Ok(file) => file,
        Err(_) => return EncodeResult::KoFileCreation,
    };
    if file.write_all(buffer.data()).is_err() {
        return EncodeResult::KoFileCreation;
    }

    EncodeResult::Ok
}
