//! Edgebreaker encoder implementation interface.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_encoder_impl_interface.h`.

use draco_core::attributes::geometry_indices::FaceIndex;
use draco_core::core::status::Status;
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use crate::compression::mesh::mesh_edgebreaker_encoder::MeshEdgebreakerEncoder;

pub trait MeshEdgebreakerEncoderImplInterface {
    fn init(&mut self, encoder: *mut MeshEdgebreakerEncoder) -> bool;

    fn get_attribute_corner_table(&self, att_id: i32) -> Option<&MeshAttributeCornerTable<'_>>;
    fn get_attribute_encoding_data(&self, att_id: i32) -> &MeshAttributeIndicesEncodingData;

    fn generate_attributes_encoder(&mut self, att_id: i32) -> bool;
    fn encode_attributes_encoder_identifier(&mut self, att_encoder_id: i32) -> bool;
    fn encode_connectivity(&mut self) -> Status;

    fn get_corner_table(&self) -> Option<&CornerTable>;
    fn is_face_encoded(&self, face_id: FaceIndex) -> bool;

    fn get_encoder(&self) -> &MeshEdgebreakerEncoder;

    /// Returns the traversal symbol sequence for Standard encoder (parity debugging).
    fn get_traversal_symbols(&self) -> Option<Vec<EdgebreakerTopologyBitPattern>> {
        None
    }
}
