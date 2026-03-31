//! Unity C-ABI bridge for the Draco Rust port.
//!
//! This crate mirrors `_ref/draco/src/draco/unity/draco_unity_plugin.*` and
//! exposes the same C ABI so Unity can consume `.drc` meshes without a C++
//! dependency. The exported functions and structs are layout-compatible with
//! the reference plugin.

use std::ffi::{c_char, c_void};
use std::ptr;
use std::slice;

use draco_bitstream::compression::config::compression_shared::EncodedGeometryType;
use draco_bitstream::compression::decode::Decoder as DracoDecoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{FaceIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::DataType as DracoDataType;
use draco_core::mesh::mesh::Mesh;

/// C ABI data type enum (matches draco::DataType values).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataTypeC {
    Invalid = 0,
    Int8 = 1,
    Uint8 = 2,
    Int16 = 3,
    Uint16 = 4,
    Int32 = 5,
    Uint32 = 6,
    Int64 = 7,
    Uint64 = 8,
    Float32 = 9,
    Float64 = 10,
    Bool = 11,
    TypesCount = 12,
}

impl DataTypeC {
    fn from_draco(dt: DracoDataType) -> Self {
        match dt {
            DracoDataType::Invalid => DataTypeC::Invalid,
            DracoDataType::Int8 => DataTypeC::Int8,
            DracoDataType::Uint8 => DataTypeC::Uint8,
            DracoDataType::Int16 => DataTypeC::Int16,
            DracoDataType::Uint16 => DataTypeC::Uint16,
            DracoDataType::Int32 => DataTypeC::Int32,
            DracoDataType::Uint32 => DataTypeC::Uint32,
            DracoDataType::Int64 => DataTypeC::Int64,
            DracoDataType::Uint64 => DataTypeC::Uint64,
            DracoDataType::Float32 => DataTypeC::Float32,
            DracoDataType::Float64 => DataTypeC::Float64,
            DracoDataType::Bool => DataTypeC::Bool,
            DracoDataType::TypesCount => DataTypeC::TypesCount,
        }
    }
}

/// C ABI geometry attribute enum (matches draco::GeometryAttribute::Type).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryAttributeTypeC {
    Invalid = -1,
    Position = 0,
    Normal = 1,
    Color = 2,
    TexCoord = 3,
    Generic = 4,
    Tangent = 5,
    Material = 6,
    Joints = 7,
    Weights = 8,
    NamedAttributesCount = 9,
}

fn geom_type_from_i32(value: i32) -> Option<GeometryAttributeType> {
    match value {
        -1 => Some(GeometryAttributeType::Invalid),
        0 => Some(GeometryAttributeType::Position),
        1 => Some(GeometryAttributeType::Normal),
        2 => Some(GeometryAttributeType::Color),
        3 => Some(GeometryAttributeType::TexCoord),
        4 => Some(GeometryAttributeType::Generic),
        5 => Some(GeometryAttributeType::Tangent),
        6 => Some(GeometryAttributeType::Material),
        7 => Some(GeometryAttributeType::Joints),
        8 => Some(GeometryAttributeType::Weights),
        9 => Some(GeometryAttributeType::NamedAttributesCount),
        _ => None,
    }
}

fn geom_type_to_c(value: GeometryAttributeType) -> GeometryAttributeTypeC {
    match value {
        GeometryAttributeType::Invalid => GeometryAttributeTypeC::Invalid,
        GeometryAttributeType::Position => GeometryAttributeTypeC::Position,
        GeometryAttributeType::Normal => GeometryAttributeTypeC::Normal,
        GeometryAttributeType::Color => GeometryAttributeTypeC::Color,
        GeometryAttributeType::TexCoord => GeometryAttributeTypeC::TexCoord,
        GeometryAttributeType::Generic => GeometryAttributeTypeC::Generic,
        GeometryAttributeType::Tangent => GeometryAttributeTypeC::Tangent,
        GeometryAttributeType::Material => GeometryAttributeTypeC::Material,
        GeometryAttributeType::Joints => GeometryAttributeTypeC::Joints,
        GeometryAttributeType::Weights => GeometryAttributeTypeC::Weights,
        GeometryAttributeType::NamedAttributesCount => GeometryAttributeTypeC::NamedAttributesCount,
    }
}

/// Struct representing Draco attribute data within Unity.
#[repr(C)]
pub struct DracoData {
    pub data_type: DataTypeC,
    pub data: *mut c_void,
}

impl Default for DracoData {
    fn default() -> Self {
        Self {
            data_type: DataTypeC::Invalid,
            data: ptr::null_mut(),
        }
    }
}

/// Struct representing a Draco attribute within Unity.
///
/// `private_attribute` points to an internal heap-owned snapshot so the handle
/// stays valid even after the source mesh has been released.
#[repr(C)]
pub struct DracoAttribute {
    pub attribute_type: GeometryAttributeTypeC,
    pub data_type: DataTypeC,
    pub num_components: i32,
    pub unique_id: i32,
    pub private_attribute: *const c_void,
}

impl Default for DracoAttribute {
    fn default() -> Self {
        Self {
            attribute_type: GeometryAttributeTypeC::Invalid,
            data_type: DataTypeC::Invalid,
            num_components: 0,
            unique_id: 0,
            private_attribute: ptr::null(),
        }
    }
}

struct DracoAttributeHandle {
    attribute: PointAttribute,
    num_points: usize,
}

/// Struct representing a Draco mesh within Unity.
#[repr(C)]
pub struct DracoMesh {
    pub num_faces: i32,
    pub num_vertices: i32,
    pub num_attributes: i32,
    pub private_mesh: *mut c_void,
}

impl Default for DracoMesh {
    fn default() -> Self {
        Self {
            num_faces: 0,
            num_vertices: 0,
            num_attributes: 0,
            private_mesh: ptr::null_mut(),
        }
    }
}

/// Deprecated mesh payload used by legacy Unity API.
#[repr(C)]
pub struct DracoToUnityMesh {
    pub num_faces: i32,
    pub indices: *mut i32,
    pub num_vertices: i32,
    pub position: *mut f32,
    pub has_normal: bool,
    pub normal: *mut f32,
    pub has_texcoord: bool,
    pub texcoord: *mut f32,
    pub has_color: bool,
    pub color: *mut f32,
}

impl Default for DracoToUnityMesh {
    fn default() -> Self {
        Self {
            num_faces: 0,
            indices: ptr::null_mut(),
            num_vertices: 0,
            position: ptr::null_mut(),
            has_normal: false,
            normal: ptr::null_mut(),
            has_texcoord: false,
            texcoord: ptr::null_mut(),
            has_color: false,
            color: ptr::null_mut(),
        }
    }
}

// --- Internal helpers -----------------------------------------------------

fn create_draco_attribute(mesh: &Mesh, attr: &PointAttribute) -> *mut DracoAttribute {
    let mut attribute_copy = PointAttribute::new();
    attribute_copy.copy_from(attr);
    let handle = Box::new(DracoAttributeHandle {
        attribute: attribute_copy,
        num_points: mesh.num_points() as usize,
    });
    let mut attribute = Box::new(DracoAttribute::default());
    attribute.attribute_type = geom_type_to_c(attr.attribute_type());
    attribute.data_type = DataTypeC::from_draco(attr.data_type());
    attribute.num_components = attr.num_components() as i32;
    attribute.unique_id = attr.unique_id() as i32;
    attribute.private_attribute = Box::into_raw(handle) as *const c_void;
    Box::into_raw(attribute)
}

fn alloc_array<T>(len: usize) -> *mut T {
    let alloc_len = if len == 0 { 1 } else { len };
    let bytes = alloc_len.saturating_mul(std::mem::size_of::<T>());
    let ptr = unsafe { libc::malloc(bytes) as *mut T };
    if ptr.is_null() {
        return ptr::null_mut();
    }
    ptr
}

unsafe fn free_array(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    libc::free(ptr);
}

fn copy_attribute_data<T: draco_core::attributes::draco_numeric::DracoNumeric + Default>(
    num_points: usize,
    attr: &PointAttribute,
) -> *mut T {
    let num_components = attr.num_components() as usize;
    if !(1..=4).contains(&num_components) {
        return ptr::null_mut();
    }
    let total = num_points.saturating_mul(num_components);
    let data_ptr = alloc_array::<T>(total);
    if data_ptr.is_null() {
        return ptr::null_mut();
    }
    let data = unsafe { slice::from_raw_parts_mut(data_ptr, total) };
    for i in 0..num_points {
        let val_index = attr.mapped_index(PointIndex::from(i as u32));
        let base = i * num_components;
        let out_slice = &mut data[base..base + num_components];
        if !attr.convert_value(val_index, num_components as i8, out_slice) {
            unsafe { free_array(data_ptr as *mut c_void) };
            return ptr::null_mut();
        }
    }
    data_ptr
}

fn convert_attribute_data(num_points: usize, attr: &PointAttribute) -> *mut c_void {
    match attr.data_type() {
        DracoDataType::Int8 => copy_attribute_data::<i8>(num_points, attr) as *mut c_void,
        DracoDataType::Uint8 => copy_attribute_data::<u8>(num_points, attr) as *mut c_void,
        DracoDataType::Int16 => copy_attribute_data::<i16>(num_points, attr) as *mut c_void,
        DracoDataType::Uint16 => copy_attribute_data::<u16>(num_points, attr) as *mut c_void,
        DracoDataType::Int32 => copy_attribute_data::<i32>(num_points, attr) as *mut c_void,
        DracoDataType::Uint32 => copy_attribute_data::<u32>(num_points, attr) as *mut c_void,
        DracoDataType::Float32 => copy_attribute_data::<f32>(num_points, attr) as *mut c_void,
        _ => ptr::null_mut(),
    }
}

fn vec_into_raw<T>(mut data: Vec<T>) -> *mut T {
    let ptr = data.as_mut_ptr();
    std::mem::forget(data);
    ptr
}

unsafe fn free_vec<T>(ptr: *mut T, len: usize) {
    if ptr.is_null() {
        return;
    }
    drop(Vec::from_raw_parts(ptr, len, len));
}

// --- C ABI ----------------------------------------------------------------

/// Release data associated with DracoMesh.
///
/// Exported `DracoAttribute` handles remain valid until `ReleaseDracoAttribute()`
/// because they own an internal attribute snapshot.
#[no_mangle]
pub unsafe extern "C" fn ReleaseDracoMesh(mesh_ptr: *mut *mut DracoMesh) {
    if mesh_ptr.is_null() {
        return;
    }
    let mesh = *mesh_ptr;
    if mesh.is_null() {
        return;
    }
    let mesh_box = Box::from_raw(mesh);
    let inner_mesh = mesh_box.private_mesh as *mut Mesh;
    if !inner_mesh.is_null() {
        drop(Box::from_raw(inner_mesh));
    }
    *mesh_ptr = ptr::null_mut();
}

/// Release data associated with DracoAttribute.
#[no_mangle]
pub unsafe extern "C" fn ReleaseDracoAttribute(attr_ptr: *mut *mut DracoAttribute) {
    if attr_ptr.is_null() {
        return;
    }
    let attr = *attr_ptr;
    if attr.is_null() {
        return;
    }
    let attr_box = Box::from_raw(attr);
    let handle = attr_box.private_attribute as *mut DracoAttributeHandle;
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
    drop(attr_box);
    *attr_ptr = ptr::null_mut();
}

/// Release attribute data.
#[no_mangle]
pub unsafe extern "C" fn ReleaseDracoData(data_ptr: *mut *mut DracoData) {
    if data_ptr.is_null() {
        return;
    }
    let data = *data_ptr;
    if data.is_null() {
        return;
    }
    let data_ref = &*data;
    match data_ref.data_type {
        DataTypeC::Int8
        | DataTypeC::Uint8
        | DataTypeC::Int16
        | DataTypeC::Uint16
        | DataTypeC::Int32
        | DataTypeC::Uint32
        | DataTypeC::Float32 => {
            free_array(data_ref.data);
        }
        _ => {}
    }
    drop(Box::from_raw(data));
    *data_ptr = ptr::null_mut();
}

/// Decodes compressed Draco mesh in |data| and returns |mesh|.
#[no_mangle]
pub unsafe extern "C" fn DecodeDracoMesh(
    data: *mut c_char,
    length: u32,
    mesh: *mut *mut DracoMesh,
) -> i32 {
    if mesh.is_null() || !(*mesh).is_null() {
        return -1;
    }
    let len = length as usize;
    if data.is_null() && len > 0 {
        return -2;
    }
    let data_slice = if len == 0 {
        &[]
    } else {
        slice::from_raw_parts(data as *const u8, len)
    };

    let mut buffer = DecoderBuffer::new();
    buffer.init(data_slice);
    let type_status = DracoDecoder::get_encoded_geometry_type(&mut buffer);
    if !type_status.is_ok() {
        return -2;
    }
    let geom_type = type_status.into_value();
    if geom_type != EncodedGeometryType::TriangularMesh {
        return -3;
    }

    let mut decoder = DracoDecoder::new();
    let statusor = decoder.decode_mesh_from_buffer(&mut buffer);
    if !statusor.is_ok() {
        return -4;
    }
    let mesh_box = statusor.into_value();

    let mut out_mesh = Box::new(DracoMesh::default());
    out_mesh.num_faces = mesh_box.num_faces() as i32;
    out_mesh.num_vertices = mesh_box.num_points() as i32;
    out_mesh.num_attributes = mesh_box.num_attributes();
    out_mesh.private_mesh = Box::into_raw(mesh_box) as *mut c_void;

    let num_faces = out_mesh.num_faces;
    *mesh = Box::into_raw(out_mesh);
    num_faces
}

/// Returns attribute at |index| in |mesh|.
#[no_mangle]
pub unsafe extern "C" fn GetAttribute(
    mesh: *const DracoMesh,
    index: i32,
    attribute: *mut *mut DracoAttribute,
) -> bool {
    if mesh.is_null() || attribute.is_null() || !(*attribute).is_null() {
        return false;
    }
    if index < 0 {
        return false;
    }
    let mesh_ref = &*(mesh as *const DracoMesh);
    let mesh_ptr = mesh_ref.private_mesh as *const Mesh;
    if mesh_ptr.is_null() {
        return false;
    }
    let m = &*mesh_ptr;
    let attr = match m.attribute(index) {
        Some(attr) => attr,
        None => return false,
    };
    *attribute = create_draco_attribute(m, attr);
    true
}

/// Returns attribute of |type| at |index| in |mesh|.
#[no_mangle]
pub unsafe extern "C" fn GetAttributeByType(
    mesh: *const DracoMesh,
    attr_type: i32,
    index: i32,
    attribute: *mut *mut DracoAttribute,
) -> bool {
    if mesh.is_null() || attribute.is_null() || !(*attribute).is_null() {
        return false;
    }
    if index < 0 {
        return false;
    }
    let mesh_ref = &*(mesh as *const DracoMesh);
    let mesh_ptr = mesh_ref.private_mesh as *const Mesh;
    if mesh_ptr.is_null() {
        return false;
    }
    let m = &*mesh_ptr;
    let geom_type = match geom_type_from_i32(attr_type) {
        Some(t) => t,
        None => return false,
    };
    let attr = match m.get_named_attribute_by_index(geom_type, index) {
        Some(attr) => attr,
        None => return false,
    };
    *attribute = create_draco_attribute(m, attr);
    true
}

/// Returns attribute with |unique_id| in |mesh|.
#[no_mangle]
pub unsafe extern "C" fn GetAttributeByUniqueId(
    mesh: *const DracoMesh,
    unique_id: i32,
    attribute: *mut *mut DracoAttribute,
) -> bool {
    if mesh.is_null() || attribute.is_null() || !(*attribute).is_null() {
        return false;
    }
    if unique_id < 0 {
        return false;
    }
    let mesh_ref = &*(mesh as *const DracoMesh);
    let mesh_ptr = mesh_ref.private_mesh as *const Mesh;
    if mesh_ptr.is_null() {
        return false;
    }
    let m = &*mesh_ptr;
    let attr = match m.get_attribute_by_unique_id(unique_id as u32) {
        Some(attr) => attr,
        None => return false,
    };
    *attribute = create_draco_attribute(m, attr);
    true
}

/// Returns mesh indices and data type.
#[no_mangle]
pub unsafe extern "C" fn GetMeshIndices(
    mesh: *const DracoMesh,
    indices: *mut *mut DracoData,
) -> bool {
    if mesh.is_null() || indices.is_null() || !(*indices).is_null() {
        return false;
    }
    let mesh_ref = &*(mesh as *const DracoMesh);
    let mesh_ptr = mesh_ref.private_mesh as *const Mesh;
    if mesh_ptr.is_null() {
        return false;
    }
    let m = &*mesh_ptr;
    let num_faces = m.num_faces() as usize;
    let total = num_faces * 3;
    let data_ptr = alloc_array::<i32>(total);
    if data_ptr.is_null() {
        return false;
    }
    let out = slice::from_raw_parts_mut(data_ptr, total);
    for i in 0..num_faces {
        let face = m.face(FaceIndex::from(i as u32));
        let base = i * 3;
        out[base] = face[0].value() as i32;
        out[base + 1] = face[1].value() as i32;
        out[base + 2] = face[2].value() as i32;
    }
    let mut draco_data = Box::new(DracoData::default());
    draco_data.data = data_ptr as *mut c_void;
    draco_data.data_type = DataTypeC::Int32;
    *indices = Box::into_raw(draco_data);
    true
}

/// Returns attribute data and data type.
///
/// The attribute payload is read from the attribute-owned snapshot rather than
/// from the mesh, so the caller may release the mesh before materializing the
/// attribute data.
#[no_mangle]
pub unsafe extern "C" fn GetAttributeData(
    _mesh: *const DracoMesh,
    attribute: *const DracoAttribute,
    data: *mut *mut DracoData,
) -> bool {
    if attribute.is_null() || data.is_null() || !(*data).is_null() {
        return false;
    }
    let handle_ptr = (*attribute).private_attribute as *const DracoAttributeHandle;
    if handle_ptr.is_null() {
        return false;
    }
    let handle = &*handle_ptr;
    let temp_data = convert_attribute_data(handle.num_points, &handle.attribute);
    if temp_data.is_null() {
        return false;
    }
    let mut draco_data = Box::new(DracoData::default());
    draco_data.data = temp_data;
    draco_data.data_type = DataTypeC::from_draco(handle.attribute.data_type());
    *data = Box::into_raw(draco_data);
    true
}

/// Release data associated with DracoToUnityMesh (deprecated).
#[no_mangle]
pub unsafe extern "C" fn ReleaseUnityMesh(mesh_ptr: *mut *mut DracoToUnityMesh) {
    if mesh_ptr.is_null() {
        return;
    }
    let mesh = *mesh_ptr;
    if mesh.is_null() {
        return;
    }
    let mut mesh_box = Box::from_raw(mesh);

    if !mesh_box.indices.is_null() {
        let len = (mesh_box.num_faces as usize).saturating_mul(3);
        free_vec(mesh_box.indices, len);
        mesh_box.indices = ptr::null_mut();
    }
    if !mesh_box.position.is_null() {
        let len = (mesh_box.num_vertices as usize).saturating_mul(3);
        free_vec(mesh_box.position, len);
        mesh_box.position = ptr::null_mut();
    }
    if mesh_box.has_normal && !mesh_box.normal.is_null() {
        let len = (mesh_box.num_vertices as usize).saturating_mul(3);
        free_vec(mesh_box.normal, len);
        mesh_box.normal = ptr::null_mut();
        mesh_box.has_normal = false;
    }
    if mesh_box.has_texcoord && !mesh_box.texcoord.is_null() {
        let len = (mesh_box.num_vertices as usize).saturating_mul(2);
        free_vec(mesh_box.texcoord, len);
        mesh_box.texcoord = ptr::null_mut();
        mesh_box.has_texcoord = false;
    }
    if mesh_box.has_color && !mesh_box.color.is_null() {
        let len = (mesh_box.num_vertices as usize).saturating_mul(4);
        free_vec(mesh_box.color, len);
        mesh_box.color = ptr::null_mut();
        mesh_box.has_color = false;
    }

    *mesh_ptr = ptr::null_mut();
}

/// Deprecated Unity decode API (kept for parity).
#[no_mangle]
pub unsafe extern "C" fn DecodeMeshForUnity(
    data: *mut c_char,
    length: u32,
    tmp_mesh: *mut *mut DracoToUnityMesh,
) -> i32 {
    let len = length as usize;
    if data.is_null() && len > 0 {
        return -1;
    }
    let data_slice = if len == 0 {
        &[]
    } else {
        slice::from_raw_parts(data as *const u8, len)
    };

    let mut buffer = DecoderBuffer::new();
    buffer.init(data_slice);
    let type_status = DracoDecoder::get_encoded_geometry_type(&mut buffer);
    if !type_status.is_ok() {
        return -1;
    }
    let geom_type = type_status.into_value();
    if geom_type != EncodedGeometryType::TriangularMesh {
        return -2;
    }

    let mut decoder = DracoDecoder::new();
    let statusor = decoder.decode_mesh_from_buffer(&mut buffer);
    if !statusor.is_ok() {
        return -3;
    }
    let mesh_box = statusor.into_value();

    let mut unity_mesh = Box::new(DracoToUnityMesh::default());
    unity_mesh.num_faces = mesh_box.num_faces() as i32;
    unity_mesh.num_vertices = mesh_box.num_points() as i32;

    let num_faces = mesh_box.num_faces() as usize;
    let num_points = mesh_box.num_points() as usize;

    let mut indices = vec![0i32; num_faces * 3];
    for i in 0..num_faces {
        let face = mesh_box.face(FaceIndex::from(i as u32));
        let base = i * 3;
        indices[base] = face[0].value() as i32;
        indices[base + 1] = face[1].value() as i32;
        indices[base + 2] = face[2].value() as i32;
    }
    unity_mesh.indices = vec_into_raw(indices);

    let pos_att = match mesh_box.get_named_attribute(GeometryAttributeType::Position) {
        Some(att) => att,
        None => {
            let mut raw = Box::into_raw(unity_mesh);
            ReleaseUnityMesh(&mut raw);
            return -8;
        }
    };
    let mut positions = vec![0.0f32; num_points * 3];
    for i in 0..num_points {
        let val_index = pos_att.mapped_index(PointIndex::from(i as u32));
        let base = i * 3;
        if !pos_att.convert_value(val_index, 3, &mut positions[base..base + 3]) {
            let mut raw = Box::into_raw(unity_mesh);
            ReleaseUnityMesh(&mut raw);
            return -8;
        }
    }
    unity_mesh.position = vec_into_raw(positions);

    if let Some(normal_att) = mesh_box.get_named_attribute(GeometryAttributeType::Normal) {
        let mut normals = vec![0.0f32; num_points * 3];
        for i in 0..num_points {
            let val_index = normal_att.mapped_index(PointIndex::from(i as u32));
            let base = i * 3;
            if !normal_att.convert_value(val_index, 3, &mut normals[base..base + 3]) {
                let mut raw = Box::into_raw(unity_mesh);
                ReleaseUnityMesh(&mut raw);
                return -8;
            }
        }
        unity_mesh.normal = vec_into_raw(normals);
        unity_mesh.has_normal = true;
    }

    if let Some(color_att) = mesh_box.get_named_attribute(GeometryAttributeType::Color) {
        let mut colors = vec![0.0f32; num_points * 4];
        let num_components = color_att.num_components() as usize;
        for i in 0..num_points {
            let val_index = color_att.mapped_index(PointIndex::from(i as u32));
            let base = i * 4;
            if !color_att.convert_value(
                val_index,
                num_components as i8,
                &mut colors[base..base + num_components],
            ) {
                let mut raw = Box::into_raw(unity_mesh);
                ReleaseUnityMesh(&mut raw);
                return -8;
            }
            if num_components < 4 {
                colors[base + 3] = 1.0;
            }
        }
        unity_mesh.color = vec_into_raw(colors);
        unity_mesh.has_color = true;
    }

    if let Some(texcoord_att) = mesh_box.get_named_attribute(GeometryAttributeType::TexCoord) {
        let mut texcoords = vec![0.0f32; num_points * 2];
        for i in 0..num_points {
            let val_index = texcoord_att.mapped_index(PointIndex::from(i as u32));
            let base = i * 2;
            if !texcoord_att.convert_value(val_index, 2, &mut texcoords[base..base + 2]) {
                let mut raw = Box::into_raw(unity_mesh);
                ReleaseUnityMesh(&mut raw);
                return -8;
            }
        }
        unity_mesh.texcoord = vec_into_raw(texcoords);
        unity_mesh.has_texcoord = true;
    }

    let out_ptr = Box::into_raw(unity_mesh);
    *tmp_mesh = out_ptr;
    num_faces as i32
}
