//! Decoder bindings for JavaScript.
//!
//! Wraps `draco_bitstream::compression::decode::Decoder` and mirrors the
//! Emscripten WebIDL API for decoding geometry and accessing attributes.

use std::cmp::min;
use std::ops::Deref;

use crate::arrays::{
    DracoFloat32Array, DracoInt16Array, DracoInt32Array, DracoInt8Array, DracoUInt16Array,
    DracoUInt32Array, DracoUInt8Array,
};
use crate::buffer::DecoderBuffer;
use crate::geometry::{
    attribute_from_mesh, attribute_from_point_cloud, attribute_value_index, Mesh, PointAttribute,
    PointCloud,
};
use crate::metadata::Metadata;
use crate::status::Status;
use crate::types::{data_type_from_i32, encoded_geometry_to_i32, geometry_attribute_from_i32};
use draco_bitstream::compression::config::compression_shared::EncodedGeometryType;
use draco_bitstream::compression::decode as bitstream_decode;
use draco_core::attributes::geometry_indices::{FaceIndex, PointIndex};
use draco_core::core::decoder_buffer::DecoderBuffer as CoreDecoderBuffer;
use draco_core::core::draco_types::DataType;
use draco_core::mesh::mesh_stripifier::MeshStripifier;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Decoder {
    inner: bitstream_decode::Decoder,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Decoder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: bitstream_decode::Decoder::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = DecodeArrayToPointCloud))]
    pub fn decode_array_to_point_cloud(
        &mut self,
        data: &[u8],
        data_size: usize,
        out_point_cloud: &PointCloud,
    ) -> Status {
        let count = min(data_size, data.len());
        let slice = &data[..count];
        let mut buffer = CoreDecoderBuffer::new();
        buffer.init(slice);
        let status = self
            .inner
            .decode_buffer_to_geometry(&mut buffer, &mut out_point_cloud.inner().borrow_mut());
        Status::from_status(status)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = DecodeArrayToMesh))]
    pub fn decode_array_to_mesh(
        &mut self,
        data: &[u8],
        data_size: usize,
        out_mesh: &Mesh,
    ) -> Status {
        let count = min(data_size, data.len());
        let slice = &data[..count];
        let mut buffer = CoreDecoderBuffer::new();
        buffer.init(slice);
        let status = self
            .inner
            .decode_buffer_to_geometry_mesh(&mut buffer, &mut out_mesh.inner().borrow_mut());
        Status::from_status(status)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = DecodeBufferToPointCloud))]
    pub fn decode_buffer_to_point_cloud(
        &mut self,
        in_buffer: &mut DecoderBuffer,
        out_point_cloud: &PointCloud,
    ) -> Status {
        let status = in_buffer.with_core_buffer(|buffer| {
            self.inner
                .decode_buffer_to_geometry(buffer, &mut out_point_cloud.inner().borrow_mut())
        });
        Status::from_status(status)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = DecodeBufferToMesh))]
    pub fn decode_buffer_to_mesh(
        &mut self,
        in_buffer: &mut DecoderBuffer,
        out_mesh: &Mesh,
    ) -> Status {
        let status = in_buffer.with_core_buffer(|buffer| {
            self.inner
                .decode_buffer_to_geometry_mesh(buffer, &mut out_mesh.inner().borrow_mut())
        });
        Status::from_status(status)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetEncodedGeometryType_Deprecated))]
    pub fn get_encoded_geometry_type_deprecated(&mut self, in_buffer: &mut DecoderBuffer) -> i32 {
        let status_or = in_buffer.with_core_buffer(|buffer| {
            bitstream_decode::Decoder::get_encoded_geometry_type(buffer)
        });
        if !status_or.is_ok() {
            return EncodedGeometryType::InvalidGeometryType as i32;
        }
        encoded_geometry_to_i32(status_or.into_value())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeId))]
    pub fn get_attribute_id_point_cloud(&self, pc: &PointCloud, att_type: i32) -> i32 {
        let att_type = geometry_attribute_from_i32(att_type);
        pc.inner().borrow().get_named_attribute_id(att_type)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeId_Mesh))]
    pub fn get_attribute_id_mesh(&self, mesh: &Mesh, att_type: i32) -> i32 {
        let att_type = geometry_attribute_from_i32(att_type);
        mesh.inner().borrow().get_named_attribute_id(att_type)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeIdByName))]
    pub fn get_attribute_id_by_name_point_cloud(&self, pc: &PointCloud, name: &str) -> i32 {
        pc.inner()
            .borrow()
            .get_attribute_id_by_metadata_entry("name", name)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeIdByName_Mesh))]
    pub fn get_attribute_id_by_name_mesh(&self, mesh: &Mesh, name: &str) -> i32 {
        mesh.inner()
            .borrow()
            .get_attribute_id_by_metadata_entry("name", name)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeIdByMetadataEntry))]
    pub fn get_attribute_id_by_metadata_entry_point_cloud(
        &self,
        pc: &PointCloud,
        name: &str,
        value: &str,
    ) -> i32 {
        pc.inner()
            .borrow()
            .get_attribute_id_by_metadata_entry(name, value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeIdByMetadataEntry_Mesh))]
    pub fn get_attribute_id_by_metadata_entry_mesh(
        &self,
        mesh: &Mesh,
        name: &str,
        value: &str,
    ) -> i32 {
        mesh.inner()
            .borrow()
            .get_attribute_id_by_metadata_entry(name, value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttribute))]
    pub fn get_attribute_point_cloud(
        &self,
        pc: &PointCloud,
        att_id: i32,
    ) -> Option<PointAttribute> {
        if pc.inner().borrow().attribute(att_id).is_none() {
            return None;
        }
        Some(attribute_from_point_cloud(pc, att_id))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttribute_Mesh))]
    pub fn get_attribute_mesh(&self, mesh: &Mesh, att_id: i32) -> Option<PointAttribute> {
        if mesh.inner().borrow().attribute(att_id).is_none() {
            return None;
        }
        Some(attribute_from_mesh(mesh, att_id))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeByUniqueId))]
    pub fn get_attribute_by_unique_id_point_cloud(
        &self,
        pc: &PointCloud,
        unique_id: i32,
    ) -> Option<PointAttribute> {
        let att_id = pc
            .inner()
            .borrow()
            .get_attribute_id_by_unique_id(unique_id as u32);
        if att_id < 0 {
            return None;
        }
        Some(attribute_from_point_cloud(pc, att_id))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeByUniqueId_Mesh))]
    pub fn get_attribute_by_unique_id_mesh(
        &self,
        mesh: &Mesh,
        unique_id: i32,
    ) -> Option<PointAttribute> {
        let att_id = mesh
            .inner()
            .borrow()
            .get_attribute_id_by_unique_id(unique_id as u32);
        if att_id < 0 {
            return None;
        }
        Some(attribute_from_mesh(mesh, att_id))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetMetadata))]
    pub fn get_metadata_point_cloud(&self, pc: &PointCloud) -> Option<Metadata> {
        pc.inner()
            .borrow()
            .get_metadata()
            .map(|meta| Metadata::from_core(meta.deref().clone()))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetMetadata_Mesh))]
    pub fn get_metadata_mesh(&self, mesh: &Mesh) -> Option<Metadata> {
        mesh.inner()
            .borrow()
            .get_metadata()
            .map(|meta| Metadata::from_core(meta.deref().clone()))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeMetadata))]
    pub fn get_attribute_metadata_point_cloud(
        &self,
        pc: &PointCloud,
        att_id: i32,
    ) -> Option<Metadata> {
        pc.inner()
            .borrow()
            .get_attribute_metadata_by_attribute_id(att_id)
            .map(|att_meta| Metadata::from_core(att_meta.deref().clone()))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeMetadata_Mesh))]
    pub fn get_attribute_metadata_mesh(&self, mesh: &Mesh, att_id: i32) -> Option<Metadata> {
        mesh.inner()
            .borrow()
            .get_attribute_metadata_by_attribute_id(att_id)
            .map(|att_meta| Metadata::from_core(att_meta.deref().clone()))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetFaceFromMesh))]
    pub fn get_face_from_mesh(
        &self,
        mesh: &Mesh,
        face_id: i32,
        out_values: &mut DracoInt32Array,
    ) -> bool {
        if face_id < 0 {
            return false;
        }
        let face_id = FaceIndex::from(face_id as u32);
        let mesh_rc = mesh.inner();
        let mesh_ref = mesh_rc.borrow();
        if face_id.value() >= mesh_ref.num_faces() {
            return false;
        }
        let face = mesh_ref.face(face_id);
        let values = vec![
            face[0].value() as i32,
            face[1].value() as i32,
            face[2].value() as i32,
        ];
        out_values.move_data(values);
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetTriangleStripsFromMesh))]
    pub fn get_triangle_strips_from_mesh(
        &self,
        mesh: &Mesh,
        strip_values: &mut DracoInt32Array,
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mesh_ref = mesh_rc.borrow();
        let mut stripifier = MeshStripifier::new();
        let mut strips: Vec<u32> = Vec::new();
        if !stripifier.generate_triangle_strips_with_degenerate_triangles(&mesh_ref, &mut strips) {
            return 0;
        }
        let strips_i32: Vec<i32> = strips.iter().map(|v| *v as i32).collect();
        strip_values.move_data(strips_i32);
        stripifier.num_strips()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetTrianglesUInt16Array))]
    pub fn get_triangles_uint16_array(&self, mesh: &Mesh, out_size: i32, out_values: u32) -> bool {
        if out_size < 0 {
            return false;
        }
        let mesh_rc = mesh.inner();
        let mesh_ref = mesh_rc.borrow();
        if mesh_ref.num_points() > u16::MAX as u32 {
            return false;
        }
        let num_faces = mesh_ref.num_faces() as usize;
        let expected = num_faces * 3 * std::mem::size_of::<u16>();
        if expected != out_size as usize {
            return false;
        }
        let out_ptr = out_values as *mut u16;
        if out_ptr.is_null() {
            return false;
        }
        unsafe {
            let out_slice = std::slice::from_raw_parts_mut(out_ptr, num_faces * 3);
            for face_id in 0..num_faces {
                let face = mesh_ref.face(FaceIndex::from(face_id as u32));
                out_slice[face_id * 3] = face[0].value() as u16;
                out_slice[face_id * 3 + 1] = face[1].value() as u16;
                out_slice[face_id * 3 + 2] = face[2].value() as u16;
            }
        }
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetTrianglesUInt32Array))]
    pub fn get_triangles_uint32_array(&self, mesh: &Mesh, out_size: i32, out_values: u32) -> bool {
        if out_size < 0 {
            return false;
        }
        let mesh_rc = mesh.inner();
        let mesh_ref = mesh_rc.borrow();
        let num_faces = mesh_ref.num_faces() as usize;
        let expected = num_faces * 3 * std::mem::size_of::<u32>();
        if expected != out_size as usize {
            return false;
        }
        let out_ptr = out_values as *mut u32;
        if out_ptr.is_null() {
            return false;
        }
        unsafe {
            let out_slice = std::slice::from_raw_parts_mut(out_ptr, num_faces * 3);
            for face_id in 0..num_faces {
                let face = mesh_ref.face(FaceIndex::from(face_id as u32));
                out_slice[face_id * 3] = face[0].value();
                out_slice[face_id * 3 + 1] = face[1].value();
                out_slice[face_id * 3 + 2] = face[2].value();
            }
        }
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeFloat))]
    pub fn get_attribute_float(
        &self,
        pa: &PointAttribute,
        att_index: i32,
        out_values: &mut DracoFloat32Array,
    ) -> bool {
        let att_index = match attribute_value_index(att_index) {
            Some(index) => index,
            None => return false,
        };
        let mut values = Vec::<f32>::new();
        let ok = pa
            .with_attribute(|att| {
                let components = att.num_components() as usize;
                if components == 0 {
                    return false;
                }
                values.resize(components, 0.0);
                att.convert_value(att_index, components as i8, &mut values)
            })
            .unwrap_or(false);
        if !ok {
            return false;
        }
        out_values.move_data(values);
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeFloatForAllPoints))]
    pub fn get_attribute_float_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoFloat32Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeFloatForAllPoints_Mesh))]
    pub fn get_attribute_float_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoFloat32Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeIntForAllPoints))]
    pub fn get_attribute_int_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoInt32Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeIntForAllPoints_Mesh))]
    pub fn get_attribute_int_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoInt32Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeInt8ForAllPoints))]
    pub fn get_attribute_int8_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoInt8Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeInt8ForAllPoints_Mesh))]
    pub fn get_attribute_int8_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoInt8Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeUInt8ForAllPoints))]
    pub fn get_attribute_uint8_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoUInt8Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeUInt8ForAllPoints_Mesh))]
    pub fn get_attribute_uint8_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoUInt8Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeInt16ForAllPoints))]
    pub fn get_attribute_int16_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoInt16Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeInt16ForAllPoints_Mesh))]
    pub fn get_attribute_int16_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoInt16Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeUInt16ForAllPoints))]
    pub fn get_attribute_uint16_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoUInt16Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeUInt16ForAllPoints_Mesh))]
    pub fn get_attribute_uint16_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoUInt16Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeInt32ForAllPoints))]
    pub fn get_attribute_int32_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoInt32Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeInt32ForAllPoints_Mesh))]
    pub fn get_attribute_int32_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoInt32Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeUInt32ForAllPoints))]
    pub fn get_attribute_uint32_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        out_values: &mut DracoUInt32Array,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeUInt32ForAllPoints_Mesh))]
    pub fn get_attribute_uint32_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        out_values: &mut DracoUInt32Array,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.fill_attribute_for_all_points_count(num_points, pa, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeDataArrayForAllPoints))]
    pub fn get_attribute_data_array_for_all_points(
        &self,
        pc: &PointCloud,
        pa: &PointAttribute,
        data_type: i32,
        out_size: i32,
        out_values: u32,
    ) -> bool {
        let num_points = pc.inner().borrow().num_points() as usize;
        self.copy_attribute_array_for_points(num_points, pa, data_type, out_size, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeDataArrayForAllPoints_Mesh))]
    pub fn get_attribute_data_array_for_all_points_mesh(
        &self,
        mesh: &Mesh,
        pa: &PointAttribute,
        data_type: i32,
        out_size: i32,
        out_values: u32,
    ) -> bool {
        let num_points = mesh.inner().borrow().num_points() as usize;
        self.copy_attribute_array_for_points(num_points, pa, data_type, out_size, out_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SkipAttributeTransform))]
    pub fn skip_attribute_transform(&mut self, att_type: i32) {
        let att_type = geometry_attribute_from_i32(att_type);
        self.inner.set_skip_attribute_transform(att_type);
    }
}

impl Decoder {
    fn fill_attribute_for_all_points_count<
        T: Copy + Default + draco_core::attributes::draco_numeric::DracoNumeric,
        A: DracoArrayAdapter<T>,
    >(
        &self,
        num_points: usize,
        pa: &PointAttribute,
        out_values: &mut A,
    ) -> bool {
        let components = pa
            .with_attribute(|att| att.num_components() as usize)
            .unwrap_or(0);
        if components == 0 {
            return false;
        }
        let mut out = Vec::<T>::with_capacity(num_points * components);
        let mut tmp = vec![T::default(); components];
        for i in 0..num_points {
            let point_index = PointIndex::from(i as u32);
            let ok = pa
                .with_attribute(|att| {
                    let att_index = att.mapped_index(point_index);
                    att.convert_value(att_index, components as i8, &mut tmp)
                })
                .unwrap_or(false);
            if !ok {
                return false;
            }
            out.extend_from_slice(&tmp);
        }
        out_values.move_data(out);
        true
    }

    fn copy_attribute_array_for_points(
        &self,
        num_points: usize,
        pa: &PointAttribute,
        data_type: i32,
        out_size: i32,
        out_values: u32,
    ) -> bool {
        if out_size < 0 {
            return false;
        }
        let out_ptr = out_values as *mut u8;
        if out_ptr.is_null() {
            return false;
        }
        let data_type = data_type_from_i32(data_type);
        match data_type {
            DataType::Int8 => {
                self.copy_attribute_array_count::<i8>(num_points, pa, out_size, out_ptr)
            }
            DataType::Uint8 => {
                self.copy_attribute_array_count::<u8>(num_points, pa, out_size, out_ptr)
            }
            DataType::Int16 => {
                self.copy_attribute_array_count::<i16>(num_points, pa, out_size, out_ptr)
            }
            DataType::Uint16 => {
                self.copy_attribute_array_count::<u16>(num_points, pa, out_size, out_ptr)
            }
            DataType::Int32 => {
                self.copy_attribute_array_count::<i32>(num_points, pa, out_size, out_ptr)
            }
            DataType::Uint32 => {
                self.copy_attribute_array_count::<u32>(num_points, pa, out_size, out_ptr)
            }
            DataType::Float32 => {
                self.copy_attribute_array_count::<f32>(num_points, pa, out_size, out_ptr)
            }
            DataType::Bool => false,
            _ => false,
        }
    }

    fn copy_attribute_array_count<
        T: Copy + Default + draco_core::attributes::draco_numeric::DracoNumeric,
    >(
        &self,
        num_points: usize,
        pa: &PointAttribute,
        out_size: i32,
        out_ptr: *mut u8,
    ) -> bool {
        let components = pa
            .with_attribute(|att| att.num_components() as usize)
            .unwrap_or(0);
        if components == 0 {
            return false;
        }
        let total = num_points * components;
        let expected = total * std::mem::size_of::<T>();
        if expected != out_size as usize {
            return false;
        }
        let mut out = vec![T::default(); total];
        let mut tmp = vec![T::default(); components];
        for i in 0..num_points {
            let point_index = PointIndex::from(i as u32);
            let ok = pa
                .with_attribute(|att| {
                    let att_index = att.mapped_index(point_index);
                    att.convert_value(att_index, components as i8, &mut tmp)
                })
                .unwrap_or(false);
            if !ok {
                return false;
            }
            out[(i * components)..(i * components + components)].copy_from_slice(&tmp);
        }
        unsafe {
            let dst = std::slice::from_raw_parts_mut(out_ptr as *mut u8, expected);
            let src = std::slice::from_raw_parts(out.as_ptr() as *const u8, expected);
            dst.copy_from_slice(src);
        }
        true
    }
}

// Adapter trait to allow reusing fill logic across Draco array wrapper types.
pub(crate) trait DracoArrayAdapter<T> {
    fn move_data(&mut self, values: Vec<T>);
}

impl DracoArrayAdapter<f32> for DracoFloat32Array {
    fn move_data(&mut self, values: Vec<f32>) {
        DracoFloat32Array::move_data(self, values);
    }
}
impl DracoArrayAdapter<i8> for DracoInt8Array {
    fn move_data(&mut self, values: Vec<i8>) {
        DracoInt8Array::move_data(self, values);
    }
}
impl DracoArrayAdapter<u8> for DracoUInt8Array {
    fn move_data(&mut self, values: Vec<u8>) {
        DracoUInt8Array::move_data(self, values);
    }
}
impl DracoArrayAdapter<i16> for DracoInt16Array {
    fn move_data(&mut self, values: Vec<i16>) {
        DracoInt16Array::move_data(self, values);
    }
}
impl DracoArrayAdapter<u16> for DracoUInt16Array {
    fn move_data(&mut self, values: Vec<u16>) {
        DracoUInt16Array::move_data(self, values);
    }
}
impl DracoArrayAdapter<i32> for DracoInt32Array {
    fn move_data(&mut self, values: Vec<i32>) {
        DracoInt32Array::move_data(self, values);
    }
}
impl DracoArrayAdapter<u32> for DracoUInt32Array {
    fn move_data(&mut self, values: Vec<u32>) {
        DracoUInt32Array::move_data(self, values);
    }
}
