//! Edgebreaker decoder implementation interface.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_decoder_impl_interface.h`.

use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::mesh::mesh_edgebreaker_decoder::MeshEdgebreakerDecoder;

pub trait MeshEdgebreakerDecoderImplInterface {
    fn init(&mut self, decoder: *mut MeshEdgebreakerDecoder) -> bool;

    fn get_attribute_corner_table(&self, att_id: i32) -> Option<&MeshAttributeCornerTable<'_>>;
    fn get_attribute_encoding_data(&self, att_id: i32) -> &MeshAttributeIndicesEncodingData;

    fn create_attributes_decoder(&mut self, att_decoder_id: i32) -> bool;
    fn decode_connectivity(&mut self) -> bool;
    fn on_attributes_decoded(&mut self) -> bool;

    fn get_decoder(&self) -> &MeshEdgebreakerDecoder;
    fn get_corner_table(&self) -> Option<&CornerTable>;
}
