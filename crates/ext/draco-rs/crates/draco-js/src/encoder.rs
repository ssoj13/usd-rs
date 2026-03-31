//! Encoder bindings for JavaScript.
//!
//! Exposes builders plus basic and expert encoders compatible with the
//! Emscripten WebIDL interface used by draco3d.

use std::cmp::min;

use crate::arrays::DracoInt8Array;
use crate::geometry::{Mesh, PointCloud};
use crate::metadata::Metadata;
use crate::types::geometry_attribute_from_i32;
use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_bitstream::compression::encode as bitstream_encode;
use draco_bitstream::compression::expert_encode as bitstream_expert_encode;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute as CorePointAttribute;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::mesh::mesh::Mesh as CoreMesh;
use draco_core::metadata::geometry_metadata::{AttributeMetadata, GeometryMetadata};
use draco_core::point_cloud::point_cloud::PointCloud as CorePointCloud;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

trait GeometryAccess {
    fn add_attribute(&mut self, att: CorePointAttribute) -> i32;
    fn attribute(&self, att_id: i32) -> Option<&CorePointAttribute>;
    fn attribute_mut(&mut self, att_id: i32) -> Option<&mut CorePointAttribute>;
    fn num_points(&self) -> u32;
    fn set_num_points(&mut self, num_points: u32);
    fn get_metadata(&self) -> Option<&GeometryMetadata>;
    fn add_metadata(&mut self, metadata: GeometryMetadata);
    fn metadata_mut(&mut self) -> Option<&mut GeometryMetadata>;
}

impl GeometryAccess for CorePointCloud {
    fn add_attribute(&mut self, att: CorePointAttribute) -> i32 {
        CorePointCloud::add_attribute(self, att)
    }

    fn attribute(&self, att_id: i32) -> Option<&CorePointAttribute> {
        CorePointCloud::attribute(self, att_id)
    }

    fn attribute_mut(&mut self, att_id: i32) -> Option<&mut CorePointAttribute> {
        CorePointCloud::attribute_mut(self, att_id)
    }

    fn num_points(&self) -> u32 {
        CorePointCloud::num_points(self)
    }

    fn set_num_points(&mut self, num_points: u32) {
        CorePointCloud::set_num_points(self, num_points);
    }

    fn get_metadata(&self) -> Option<&GeometryMetadata> {
        CorePointCloud::get_metadata(self)
    }

    fn add_metadata(&mut self, metadata: GeometryMetadata) {
        CorePointCloud::add_metadata(self, metadata);
    }

    fn metadata_mut(&mut self) -> Option<&mut GeometryMetadata> {
        CorePointCloud::metadata_mut(self)
    }
}

impl GeometryAccess for CoreMesh {
    fn add_attribute(&mut self, att: CorePointAttribute) -> i32 {
        std::ops::DerefMut::deref_mut(self).add_attribute(att)
    }

    fn attribute(&self, att_id: i32) -> Option<&CorePointAttribute> {
        std::ops::Deref::deref(self).attribute(att_id)
    }

    fn attribute_mut(&mut self, att_id: i32) -> Option<&mut CorePointAttribute> {
        std::ops::DerefMut::deref_mut(self).attribute_mut(att_id)
    }

    fn num_points(&self) -> u32 {
        std::ops::Deref::deref(self).num_points()
    }

    fn set_num_points(&mut self, num_points: u32) {
        std::ops::DerefMut::deref_mut(self).set_num_points(num_points);
    }

    fn get_metadata(&self) -> Option<&GeometryMetadata> {
        std::ops::Deref::deref(self).get_metadata()
    }

    fn add_metadata(&mut self, metadata: GeometryMetadata) {
        std::ops::DerefMut::deref_mut(self).add_metadata(metadata);
    }

    fn metadata_mut(&mut self) -> Option<&mut GeometryMetadata> {
        std::ops::DerefMut::deref_mut(self).metadata_mut()
    }
}

fn add_attribute_impl<T: Copy, G: GeometryAccess>(
    geometry: &mut G,
    att_type: i32,
    num_vertices: i32,
    num_components: i32,
    att_values: &[T],
    data_type: DataType,
) -> i32 {
    if num_vertices < 0 || num_components < 0 {
        return -1;
    }
    let num_vertices = num_vertices as usize;
    let num_components = num_components as usize;
    let expected = num_vertices * num_components;
    if att_values.len() < expected {
        return -1;
    }

    let mut att = CorePointAttribute::new();
    att.init(
        geometry_attribute_from_i32(att_type),
        num_components as i8,
        data_type,
        false,
        num_vertices,
    );
    let att_id = geometry.add_attribute(att);
    let att_ref = geometry.attribute_mut(att_id).expect("Attribute missing");

    for i in 0..num_vertices {
        let start = i * num_components;
        let end = start + num_components;
        let slice = &att_values[start..end];
        let bytes = unsafe {
            std::slice::from_raw_parts(
                slice.as_ptr() as *const u8,
                std::mem::size_of::<T>() * num_components,
            )
        };
        att_ref.set_attribute_value_bytes(AttributeValueIndex::from(i as u32), bytes);
    }

    if geometry.num_points() == 0 {
        geometry.set_num_points(num_vertices as u32);
    } else if geometry.num_points() as usize != num_vertices {
        return -1;
    }
    att_id
}

fn add_metadata_impl<G: GeometryAccess>(geometry: &mut G, metadata: &Metadata) -> bool {
    if geometry.get_metadata().is_some() {
        return false;
    }
    let geo_metadata = GeometryMetadata::from_metadata(metadata.inner().clone());
    geometry.add_metadata(geo_metadata);
    true
}

fn set_metadata_for_attribute_impl<G: GeometryAccess>(
    geometry: &mut G,
    attribute_id: i32,
    metadata: &Metadata,
) -> bool {
    if attribute_id < 0 {
        return false;
    }
    if geometry.attribute(attribute_id).is_none() {
        return false;
    }
    if geometry.get_metadata().is_none() {
        geometry.add_metadata(GeometryMetadata::new());
    }
    let unique_id = geometry
        .attribute(attribute_id)
        .expect("Attribute missing")
        .unique_id();
    let mut att_metadata = AttributeMetadata::from_metadata(metadata.inner().clone());
    att_metadata.set_att_unique_id(unique_id);
    if let Some(meta) = geometry.metadata_mut() {
        return meta.add_attribute_metadata(Some(Box::new(att_metadata)));
    }
    false
}

fn set_normalized_flag_for_attribute_impl<G: GeometryAccess>(
    geometry: &mut G,
    attribute_id: i32,
    normalized: bool,
) -> bool {
    if let Some(att) = geometry.attribute_mut(attribute_id) {
        att.set_normalized(normalized);
        return true;
    }
    false
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct PointCloudBuilder;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PointCloudBuilder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        PointCloudBuilder
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddFloatAttribute))]
    pub fn add_float_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[f32],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Float32,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt8Attribute))]
    pub fn add_int8_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i8],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Int8,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddUInt8Attribute))]
    pub fn add_uint8_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[u8],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Uint8,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt16Attribute))]
    pub fn add_int16_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i16],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Int16,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddUInt16Attribute))]
    pub fn add_uint16_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[u16],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Uint16,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt32Attribute))]
    pub fn add_int32_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i32],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Int32,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddUInt32Attribute))]
    pub fn add_uint32_attribute(
        &self,
        pc: &PointCloud,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[u32],
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_attribute_impl(
            &mut *pc_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Uint32,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddMetadata))]
    pub fn add_metadata(&self, pc: &PointCloud, metadata: &Metadata) -> bool {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        add_metadata_impl(&mut *pc_ref, metadata)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetMetadataForAttribute))]
    pub fn set_metadata_for_attribute(
        &self,
        pc: &PointCloud,
        attribute_id: i32,
        metadata: &Metadata,
    ) -> bool {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        set_metadata_for_attribute_impl(&mut *pc_ref, attribute_id, metadata)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetNormalizedFlagForAttribute))]
    pub fn set_normalized_flag_for_attribute(
        &self,
        pc: &PointCloud,
        attribute_id: i32,
        normalized: bool,
    ) -> bool {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        set_normalized_flag_for_attribute_impl(&mut *pc_ref, attribute_id, normalized)
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct MeshBuilder;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl MeshBuilder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        MeshBuilder
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddFacesToMesh))]
    pub fn add_faces_to_mesh(&self, mesh: &Mesh, num_faces: i32, faces: &[i32]) -> bool {
        if num_faces < 0 {
            return false;
        }
        let num_faces = num_faces as usize;
        if faces.len() < num_faces * 3 {
            return false;
        }
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        mesh_ref.set_num_faces(num_faces);
        for i in 0..num_faces {
            let idx = i * 3;
            let face = [
                PointIndex::from(faces[idx] as u32),
                PointIndex::from(faces[idx + 1] as u32),
                PointIndex::from(faces[idx + 2] as u32),
            ];
            mesh_ref.set_face(FaceIndex::from(i as u32), face);
        }
        true
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddFloatAttributeToMesh))]
    pub fn add_float_attribute_to_mesh(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[f32],
    ) -> i32 {
        self.add_float_attribute(mesh, att_type, num_vertices, num_components, att_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt32AttributeToMesh))]
    pub fn add_int32_attribute_to_mesh(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i32],
    ) -> i32 {
        self.add_int32_attribute(mesh, att_type, num_vertices, num_components, att_values)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddMetadataToMesh))]
    pub fn add_metadata_to_mesh(&self, mesh: &Mesh, metadata: &Metadata) -> bool {
        self.add_metadata(mesh, metadata)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddFloatAttribute))]
    pub fn add_float_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[f32],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Float32,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt8Attribute))]
    pub fn add_int8_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i8],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Int8,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddUInt8Attribute))]
    pub fn add_uint8_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[u8],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Uint8,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt16Attribute))]
    pub fn add_int16_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i16],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Int16,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddUInt16Attribute))]
    pub fn add_uint16_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[u16],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Uint16,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddInt32Attribute))]
    pub fn add_int32_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[i32],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Int32,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddUInt32Attribute))]
    pub fn add_uint32_attribute(
        &self,
        mesh: &Mesh,
        att_type: i32,
        num_vertices: i32,
        num_components: i32,
        att_values: &[u32],
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_attribute_impl(
            &mut *mesh_ref,
            att_type,
            num_vertices,
            num_components,
            att_values,
            DataType::Uint32,
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = AddMetadata))]
    pub fn add_metadata(&self, mesh: &Mesh, metadata: &Metadata) -> bool {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        add_metadata_impl(&mut *mesh_ref, metadata)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetMetadataForAttribute))]
    pub fn set_metadata_for_attribute(
        &self,
        mesh: &Mesh,
        attribute_id: i32,
        metadata: &Metadata,
    ) -> bool {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        set_metadata_for_attribute_impl(&mut *mesh_ref, attribute_id, metadata)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetNormalizedFlagForAttribute))]
    pub fn set_normalized_flag_for_attribute(
        &self,
        mesh: &Mesh,
        attribute_id: i32,
        normalized: bool,
    ) -> bool {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        set_normalized_flag_for_attribute_impl(&mut *mesh_ref, attribute_id, normalized)
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Encoder {
    inner: bitstream_encode::Encoder,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Encoder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: bitstream_encode::Encoder::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetEncodingMethod))]
    pub fn set_encoding_method(&mut self, method: i32) {
        self.inner.set_encoding_method(method);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetAttributeQuantization))]
    pub fn set_attribute_quantization(&mut self, att_type: i32, quantization_bits: i32) {
        let att_type = geometry_attribute_from_i32(att_type);
        self.inner
            .set_attribute_quantization(att_type, quantization_bits);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetAttributeExplicitQuantization))]
    pub fn set_attribute_explicit_quantization(
        &mut self,
        att_type: i32,
        quantization_bits: i32,
        num_components: i32,
        origin: &[f32],
        range: f32,
    ) {
        let att_type = geometry_attribute_from_i32(att_type);
        let num_components = num_components.max(0) as usize;
        let count = min(origin.len(), num_components);
        self.inner.set_attribute_explicit_quantization(
            att_type,
            quantization_bits,
            num_components as i32,
            &origin[..count],
            range,
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetSpeedOptions))]
    pub fn set_speed_options(&mut self, encoding_speed: i32, decoding_speed: i32) {
        self.inner.set_speed_options(encoding_speed, decoding_speed);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetTrackEncodedProperties))]
    pub fn set_track_encoded_properties(&mut self, flag: bool) {
        self.inner.set_track_encoded_properties(flag);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = EncodeMeshToDracoBuffer))]
    pub fn encode_mesh_to_draco_buffer(
        &mut self,
        mesh: &Mesh,
        draco_buffer: &mut DracoInt8Array,
    ) -> i32 {
        let mesh_rc = mesh.inner();
        let mut mesh_ref = mesh_rc.borrow_mut();
        if mesh_ref.get_named_attribute_id(geometry_attribute_from_i32(0)) == -1 {
            return 0;
        }
        if !mesh_ref.deduplicate_attribute_values() {
            return 0;
        }
        mesh_ref.deduplicate_point_ids();
        let mut buffer = EncoderBuffer::new();
        if !self
            .inner
            .encode_mesh_to_buffer(&mesh_ref, &mut buffer)
            .is_ok()
        {
            return 0;
        }
        let data = buffer.data();
        let values: Vec<i8> = data.iter().map(|v| *v as i8).collect();
        draco_buffer.set_values(&values);
        buffer.size() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = EncodePointCloudToDracoBuffer))]
    pub fn encode_point_cloud_to_draco_buffer(
        &mut self,
        pc: &PointCloud,
        deduplicate_values: bool,
        draco_buffer: &mut DracoInt8Array,
    ) -> i32 {
        let pc_rc = pc.inner();
        let mut pc_ref = pc_rc.borrow_mut();
        if pc_ref.get_named_attribute_id(geometry_attribute_from_i32(0)) == -1 {
            return 0;
        }
        if deduplicate_values {
            if !pc_ref.deduplicate_attribute_values() {
                return 0;
            }
            pc_ref.deduplicate_point_ids();
        }
        let mut buffer = EncoderBuffer::new();
        if !self
            .inner
            .encode_point_cloud_to_buffer(&pc_ref, &mut buffer)
            .is_ok()
        {
            return 0;
        }
        let data = buffer.data();
        let values: Vec<i8> = data.iter().map(|v| *v as i8).collect();
        draco_buffer.set_values(&values);
        buffer.size() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetNumberOfEncodedPoints))]
    pub fn get_number_of_encoded_points(&self) -> i32 {
        self.inner.num_encoded_points() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetNumberOfEncodedFaces))]
    pub fn get_number_of_encoded_faces(&self) -> i32 {
        self.inner.num_encoded_faces() as i32
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct ExpertEncoder {
    options: EncoderOptions,
    geometry: ExpertGeometry,
    num_encoded_points: i32,
    num_encoded_faces: i32,
}

enum ExpertGeometry {
    PointCloud(std::rc::Rc<std::cell::RefCell<CorePointCloud>>),
    Mesh(std::rc::Rc<std::cell::RefCell<CoreMesh>>),
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl ExpertEncoder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(pc: &PointCloud) -> Self {
        Self {
            options: EncoderOptions::create_default_options(),
            geometry: ExpertGeometry::PointCloud(pc.inner()),
            num_encoded_points: 0,
            num_encoded_faces: 0,
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fromMesh))]
    pub fn from_mesh(mesh: &Mesh) -> Self {
        Self {
            options: EncoderOptions::create_default_options(),
            geometry: ExpertGeometry::Mesh(mesh.inner()),
            num_encoded_points: 0,
            num_encoded_faces: 0,
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetEncodingMethod))]
    pub fn set_encoding_method(&mut self, method: i32) {
        self.options.set_global_int("encoding_method", method);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetAttributeQuantization))]
    pub fn set_attribute_quantization(&mut self, att_id: i32, quantization_bits: i32) {
        self.options
            .set_attribute_int(&att_id, "quantization_bits", quantization_bits);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetAttributeExplicitQuantization))]
    pub fn set_attribute_explicit_quantization(
        &mut self,
        att_id: i32,
        quantization_bits: i32,
        num_components: i32,
        origin: &[f32],
        range: f32,
    ) {
        let num_components = num_components.max(0) as usize;
        let count = min(origin.len(), num_components);
        self.options
            .set_attribute_int(&att_id, "quantization_bits", quantization_bits);
        self.options.set_attribute_vector(
            &att_id,
            "quantization_origin",
            num_components as i32,
            &origin[..count],
        );
        self.options
            .set_attribute_float(&att_id, "quantization_range", range);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetSpeedOptions))]
    pub fn set_speed_options(&mut self, encoding_speed: i32, decoding_speed: i32) {
        self.options.set_speed(encoding_speed, decoding_speed);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = SetTrackEncodedProperties))]
    pub fn set_track_encoded_properties(&mut self, flag: bool) {
        self.options
            .set_global_bool("store_number_of_encoded_points", flag);
        self.options
            .set_global_bool("store_number_of_encoded_faces", flag);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = EncodeToDracoBuffer))]
    pub fn encode_to_draco_buffer(
        &mut self,
        deduplicate_values: bool,
        draco_buffer: &mut DracoInt8Array,
    ) -> i32 {
        if deduplicate_values {
            match &self.geometry {
                ExpertGeometry::PointCloud(pc) => {
                    let mut pc_ref = pc.borrow_mut();
                    if !pc_ref.deduplicate_attribute_values() {
                        return 0;
                    }
                    pc_ref.deduplicate_point_ids();
                }
                ExpertGeometry::Mesh(mesh) => {
                    let mut mesh_ref = mesh.borrow_mut();
                    if !mesh_ref.deduplicate_attribute_values() {
                        return 0;
                    }
                    mesh_ref.deduplicate_point_ids();
                }
            }
        }
        let mut buffer = EncoderBuffer::new();
        let status = match &self.geometry {
            ExpertGeometry::PointCloud(pc) => {
                let pc_ref = pc.borrow();
                let mut encoder = bitstream_expert_encode::ExpertEncoder::new_point_cloud(&pc_ref);
                encoder.reset(self.options.clone());
                let status = encoder.encode_to_buffer(&mut buffer);
                self.num_encoded_points = encoder.num_encoded_points() as i32;
                self.num_encoded_faces = encoder.num_encoded_faces() as i32;
                status
            }
            ExpertGeometry::Mesh(mesh) => {
                let mesh_ref = mesh.borrow();
                let mut encoder = bitstream_expert_encode::ExpertEncoder::new_mesh(&mesh_ref);
                encoder.reset(self.options.clone());
                let status = encoder.encode_to_buffer(&mut buffer);
                self.num_encoded_points = encoder.num_encoded_points() as i32;
                self.num_encoded_faces = encoder.num_encoded_faces() as i32;
                status
            }
        };
        if !status.is_ok() {
            return 0;
        }
        let data = buffer.data();
        let values: Vec<i8> = data.iter().map(|v| *v as i8).collect();
        draco_buffer.set_values(&values);
        buffer.size() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetNumberOfEncodedPoints))]
    pub fn get_number_of_encoded_points(&self) -> i32 {
        self.num_encoded_points
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetNumberOfEncodedFaces))]
    pub fn get_number_of_encoded_faces(&self) -> i32 {
        self.num_encoded_faces
    }
}
