//! Mesh compression codecs.
//! Reference: `_ref/draco/src/draco/compression/mesh`.
//!
//! Provides mesh encoder/decoder interfaces and sequential mesh codecs.

pub mod edgebreaker_shared;
pub mod mesh_edgebreaker_decoder;
pub mod mesh_edgebreaker_decoder_impl;
pub mod mesh_edgebreaker_decoder_impl_interface;
pub mod mesh_edgebreaker_encoder;
pub mod mesh_edgebreaker_encoder_impl;
pub mod mesh_edgebreaker_encoder_impl_interface;
pub mod mesh_edgebreaker_traversal_decoder;
pub mod mesh_edgebreaker_traversal_encoder;
pub mod mesh_edgebreaker_traversal_predictive_decoder;
pub mod mesh_edgebreaker_traversal_predictive_encoder;
pub mod mesh_edgebreaker_traversal_valence_decoder;
pub mod mesh_edgebreaker_traversal_valence_encoder;
pub mod traverser;

pub use mesh_edgebreaker_decoder::MeshEdgebreakerDecoder;
pub use mesh_edgebreaker_decoder_impl::take_decoded_traversal_symbols_for_parity;
pub use mesh_edgebreaker_encoder::MeshEdgebreakerEncoder;

use crate::compression::attributes::linear_sequencer::LinearSequencer;
use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::attributes::sequential_attribute_decoders_controller::SequentialAttributeDecodersController;
use crate::compression::attributes::sequential_attribute_encoders_controller::SequentialAttributeEncodersController;
use crate::compression::config::compression_shared::{
    bitstream_version, EncodedGeometryType, MeshEncoderMethod,
};
use crate::compression::entropy::symbol_decoding::decode_symbols;
use crate::compression::entropy::symbol_encoding::encode_symbols;
use crate::compression::point_cloud::{
    PointCloudDecoder, PointCloudDecoderBase, PointCloudEncoder, PointCloudEncoderBase,
};
use draco_core::attributes::geometry_indices::{FaceIndex, PointIndex};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::core::varint_decoding::decode_varint;
use draco_core::core::varint_encoding::encode_varint;
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh::Mesh;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

pub struct MeshEncoderBase {
    pub(crate) pc_base: PointCloudEncoderBase,
    pub(crate) mesh: *const Mesh,
    pub(crate) num_encoded_faces: usize,
}

impl MeshEncoderBase {
    pub fn new() -> Self {
        Self {
            pc_base: PointCloudEncoderBase::new(),
            mesh: std::ptr::null(),
            num_encoded_faces: 0,
        }
    }

    pub fn set_mesh(&mut self, mesh: &Mesh) {
        self.mesh = mesh as *const Mesh;
        self.pc_base.point_cloud =
            mesh as *const _ as *const draco_core::point_cloud::point_cloud::PointCloud;
    }

    pub fn mesh(&self) -> Option<&Mesh> {
        unsafe { self.mesh.as_ref() }
    }
}

impl Default for MeshEncoderBase {
    fn default() -> Self {
        Self::new()
    }
}

pub trait MeshEncoder: PointCloudEncoder {
    fn mesh_base(&self) -> &MeshEncoderBase;
    fn mesh_base_mut(&mut self) -> &mut MeshEncoderBase;

    fn set_mesh(&mut self, mesh: &Mesh) {
        self.mesh_base_mut().set_mesh(mesh);
    }

    fn mesh(&self) -> Option<&Mesh> {
        self.mesh_base().mesh()
    }

    fn num_encoded_faces(&self) -> usize {
        self.mesh_base().num_encoded_faces
    }

    fn set_num_encoded_faces(&mut self, num: usize) {
        self.mesh_base_mut().num_encoded_faces = num;
    }

    fn encode_connectivity(&mut self) -> Status;
    fn compute_number_of_encoded_faces(&mut self);
}

pub struct MeshSequentialEncoder {
    base: MeshEncoderBase,
}

impl MeshSequentialEncoder {
    pub fn new() -> Self {
        Self {
            base: MeshEncoderBase::new(),
        }
    }

    fn compress_and_encode_indices(&mut self) -> bool {
        let mesh = match self.mesh() {
            Some(mesh) => mesh,
            None => return false,
        };
        let mut indices_buffer = Vec::with_capacity(mesh.num_faces() as usize * 3);
        let mut last_index_value: i32 = 0;
        for i in 0..mesh.num_faces() {
            let face = mesh.face(FaceIndex::from(i));
            for j in 0..3 {
                let index_value = face[j].value() as i32;
                let index_diff = index_value - last_index_value;
                let encoded_val =
                    ((index_diff.abs() as u32) << 1) | if index_diff < 0 { 1 } else { 0 };
                indices_buffer.push(encoded_val);
                last_index_value = index_value;
            }
        }
        let buffer = match self.buffer() {
            Some(buf) => buf,
            None => return false,
        };
        encode_symbols(
            &indices_buffer,
            indices_buffer.len() as i32,
            1,
            None,
            buffer,
        )
    }
}

impl Default for MeshSequentialEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudEncoder for MeshSequentialEncoder {
    fn base(&self) -> &PointCloudEncoderBase {
        &self.base.pc_base
    }

    fn base_mut(&mut self) -> &mut PointCloudEncoderBase {
        &mut self.base.pc_base
    }

    fn get_geometry_type(&self) -> EncodedGeometryType {
        EncodedGeometryType::TriangularMesh
    }

    fn get_encoding_method(&self) -> u8 {
        MeshEncoderMethod::MeshSequentialEncoding as u8
    }

    fn encode_geometry_data(&mut self) -> Status {
        let status = self.encode_connectivity();
        if !status.is_ok() {
            return status;
        }
        if self
            .options()
            .get_global_bool("store_number_of_encoded_faces", false)
        {
            self.compute_number_of_encoded_faces();
        }
        ok_status()
    }

    fn generate_attributes_encoder(&mut self, att_id: i32) -> bool {
        if att_id == 0 {
            let num_points = self
                .point_cloud()
                .map(|pc| pc.num_points() as i32)
                .unwrap_or(0);
            let sequencer = Box::new(LinearSequencer::new(num_points));
            let enc = SequentialAttributeEncodersController::with_attribute_id(sequencer, att_id);
            self.add_attributes_encoder(Box::new(enc));
        } else if let Some(enc) = self.attributes_encoder_mut(0) {
            enc.add_attribute_id(att_id);
        }
        true
    }

    fn compute_number_of_encoded_points(&mut self) {
        let num_points = self.mesh().map(|m| m.num_points() as usize).unwrap_or(0);
        self.set_num_encoded_points(num_points);
    }
}

impl MeshEncoder for MeshSequentialEncoder {
    fn mesh_base(&self) -> &MeshEncoderBase {
        &self.base
    }

    fn mesh_base_mut(&mut self) -> &mut MeshEncoderBase {
        &mut self.base
    }

    fn encode_connectivity(&mut self) -> Status {
        let mesh = match self.mesh() {
            Some(mesh) => mesh,
            None => return Status::new(StatusCode::DracoError, "Missing mesh."),
        };
        let buffer = match self.buffer() {
            Some(buf) => buf,
            None => return Status::new(StatusCode::DracoError, "Missing output buffer."),
        };
        let num_faces = mesh.num_faces();
        encode_varint(num_faces, buffer);
        encode_varint(mesh.num_points(), buffer);

        if self
            .options()
            .get_global_bool("compress_connectivity", false)
        {
            if !buffer.encode(0u8) {
                return Status::new(StatusCode::DracoError, "Failed to encode connectivity.");
            }
            if !self.compress_and_encode_indices() {
                return Status::new(StatusCode::DracoError, "Failed to compress connectivity.");
            }
        } else {
            if !buffer.encode(1u8) {
                return Status::new(StatusCode::DracoError, "Failed to encode connectivity.");
            }
            let num_points = mesh.num_points();
            if num_points < 256 {
                for i in 0..num_faces {
                    let face = mesh.face(FaceIndex::from(i));
                    for j in 0..3 {
                        if !buffer.encode(face[j].value() as u8) {
                            return Status::new(
                                StatusCode::DracoError,
                                "Failed to encode indices.",
                            );
                        }
                    }
                }
            } else if num_points < (1 << 16) {
                for i in 0..num_faces {
                    let face = mesh.face(FaceIndex::from(i));
                    for j in 0..3 {
                        if !buffer.encode(face[j].value() as u16) {
                            return Status::new(
                                StatusCode::DracoError,
                                "Failed to encode indices.",
                            );
                        }
                    }
                }
            } else if num_points < (1 << 21) {
                for i in 0..num_faces {
                    let face = mesh.face(FaceIndex::from(i));
                    for j in 0..3 {
                        encode_varint(face[j].value(), buffer);
                    }
                }
            } else {
                for i in 0..num_faces {
                    let face = mesh.face(FaceIndex::from(i));
                    for j in 0..3 {
                        if !buffer.encode(face[j].value()) {
                            return Status::new(
                                StatusCode::DracoError,
                                "Failed to encode indices.",
                            );
                        }
                    }
                }
            }
        }
        ok_status()
    }

    fn compute_number_of_encoded_faces(&mut self) {
        let num_faces = self.mesh().map(|m| m.num_faces() as usize).unwrap_or(0);
        self.set_num_encoded_faces(num_faces);
    }
}

pub trait MeshDecoder: PointCloudDecoder {
    fn set_mesh(&mut self, mesh: &mut Mesh);
    fn mesh(&self) -> Option<&Mesh>;

    fn get_corner_table(&self) -> Option<&CornerTable> {
        None
    }

    fn get_attribute_corner_table(&self, _att_id: i32) -> Option<&MeshAttributeCornerTable<'_>> {
        None
    }

    fn get_attribute_encoding_data(
        &self,
        _att_id: i32,
    ) -> Option<&MeshAttributeIndicesEncodingData> {
        None
    }

    fn decode(
        &mut self,
        options: &crate::compression::config::decoder_options::DecoderOptions,
        in_buffer: &mut DecoderBuffer,
        out_mesh: &mut Mesh,
    ) -> Status
    where
        Self: Sized,
    {
        self.set_mesh(out_mesh);
        PointCloudDecoder::decode(self, options, in_buffer, out_mesh)
    }
}

pub struct MeshSequentialDecoder {
    base: PointCloudDecoderBase,
    mesh: *mut Mesh,
}

impl MeshSequentialDecoder {
    pub fn new() -> Self {
        Self {
            base: PointCloudDecoderBase::new(),
            mesh: std::ptr::null_mut(),
        }
    }

    fn mesh_mut(&mut self) -> Option<&mut Mesh> {
        unsafe { self.mesh.as_mut() }
    }

    fn decode_and_decompress_indices(&mut self, num_faces: u32) -> bool {
        let mut indices_buffer = vec![0u32; num_faces as usize * 3];
        {
            let buffer = match self.buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if !decode_symbols(num_faces * 3, 1, buffer, &mut indices_buffer) {
                return false;
            }
        }
        let mesh = match self.mesh_mut() {
            Some(mesh) => mesh,
            None => return false,
        };
        let mut last_index_value: i32 = 0;
        let mut vertex_index = 0usize;
        for _ in 0..num_faces {
            let mut face = [PointIndex::from(0u32); 3];
            for j in 0..3 {
                let encoded_val = indices_buffer[vertex_index];
                vertex_index += 1;
                let mut index_diff = (encoded_val >> 1) as i32;
                if (encoded_val & 1) != 0 {
                    if index_diff > last_index_value {
                        return false;
                    }
                    index_diff = -index_diff;
                } else if index_diff > (i32::MAX - last_index_value) {
                    return false;
                }
                let index_value = index_diff + last_index_value;
                face[j] = PointIndex::from(index_value as u32);
                last_index_value = index_value;
            }
            mesh.add_face(face);
        }
        true
    }

    fn decode_connectivity(&mut self) -> bool {
        let decoder_bitstream_version = self.bitstream_version();
        let mut num_faces: u32 = 0;
        let mut num_points: u32 = 0;
        let mut connectivity_method: u8 = 0;
        let mut faces: Vec<[PointIndex; 3]> = Vec::new();
        {
            let buffer = match self.buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if decoder_bitstream_version < bitstream_version(2, 2) {
                if !buffer.decode(&mut num_faces) || !buffer.decode(&mut num_points) {
                    return false;
                }
            } else {
                if !decode_varint(&mut num_faces, buffer) || !decode_varint(&mut num_points, buffer)
                {
                    return false;
                }
            }
            let faces_64 = num_faces as u64;
            if faces_64 > 0xffffffff / 3 {
                return false;
            }
            if faces_64 > (buffer.remaining_size() as u64) / 3 {
                return false;
            }
            if !buffer.decode(&mut connectivity_method) {
                return false;
            }
            if connectivity_method != 0 {
                faces.reserve(num_faces as usize);
                if num_points < 256 {
                    for _ in 0..num_faces {
                        let mut face = [PointIndex::from(0u32); 3];
                        for j in 0..3 {
                            let mut val: u8 = 0;
                            if !buffer.decode(&mut val) {
                                return false;
                            }
                            face[j] = PointIndex::from(val as u32);
                        }
                        faces.push(face);
                    }
                } else if num_points < (1 << 16) {
                    for _ in 0..num_faces {
                        let mut face = [PointIndex::from(0u32); 3];
                        for j in 0..3 {
                            let mut val: u16 = 0;
                            if !buffer.decode(&mut val) {
                                return false;
                            }
                            face[j] = PointIndex::from(val as u32);
                        }
                        faces.push(face);
                    }
                } else if num_points < (1 << 21)
                    && decoder_bitstream_version >= bitstream_version(2, 2)
                {
                    for _ in 0..num_faces {
                        let mut face = [PointIndex::from(0u32); 3];
                        for j in 0..3 {
                            let mut val: u32 = 0;
                            if !decode_varint(&mut val, buffer) {
                                return false;
                            }
                            face[j] = PointIndex::from(val);
                        }
                        faces.push(face);
                    }
                } else {
                    for _ in 0..num_faces {
                        let mut face = [PointIndex::from(0u32); 3];
                        for j in 0..3 {
                            let mut val: u32 = 0;
                            if !buffer.decode(&mut val) {
                                return false;
                            }
                            face[j] = PointIndex::from(val);
                        }
                        faces.push(face);
                    }
                }
            }
        }
        if connectivity_method == 0 {
            if !self.decode_and_decompress_indices(num_faces) {
                return false;
            }
        } else {
            let mesh = match self.mesh_mut() {
                Some(mesh) => mesh,
                None => return false,
            };
            for face in faces {
                mesh.add_face(face);
            }
        }
        if let Some(mesh) = self.mesh_mut() {
            mesh.set_num_points(num_points);
        }
        true
    }
}

impl Default for MeshSequentialDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudDecoder for MeshSequentialDecoder {
    fn base(&self) -> &PointCloudDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PointCloudDecoderBase {
        &mut self.base
    }

    fn as_mesh_decoder(&self) -> Option<&dyn MeshDecoder> {
        Some(self)
    }

    fn get_geometry_type(&self) -> EncodedGeometryType {
        EncodedGeometryType::TriangularMesh
    }

    fn decode_geometry_data(&mut self) -> bool {
        self.decode_connectivity()
    }

    fn create_attributes_decoder(&mut self, att_decoder_id: i32) -> bool {
        let num_points = self
            .point_cloud()
            .map(|pc| pc.num_points() as i32)
            .unwrap_or(0);
        let sequencer = Box::new(LinearSequencer::new(num_points));
        let dec = SequentialAttributeDecodersController::new(sequencer);
        self.set_attributes_decoder(att_decoder_id, Box::new(dec))
    }
}

impl MeshDecoder for MeshSequentialDecoder {
    fn set_mesh(&mut self, mesh: &mut Mesh) {
        self.mesh = mesh as *mut Mesh;
        self.base.point_cloud =
            mesh as *mut _ as *mut draco_core::point_cloud::point_cloud::PointCloud;
    }

    fn mesh(&self) -> Option<&Mesh> {
        unsafe { self.mesh.as_ref() }
    }
}
