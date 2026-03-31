//! Edgebreaker traversal encoder (standard).
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_traversal_encoder.h`.

use draco_core::core::encoder_buffer::EncoderBuffer;

use crate::compression::bit_coders::rans_bit_encoder::RAnsBitEncoder;
use crate::compression::mesh::edgebreaker_shared::{
    EdgebreakerTopologyBitPattern, EDGE_BREAKER_TOPOLOGY_BIT_PATTERN_LENGTH,
};
use crate::compression::mesh::mesh_edgebreaker_encoder_impl_interface::MeshEdgebreakerEncoderImplInterface;
use crate::compression::mesh::MeshEncoder;

pub trait EdgebreakerTraversalEncoder {
    fn init(&mut self, encoder: *const dyn MeshEdgebreakerEncoderImplInterface) -> bool;
    fn set_num_attribute_data(&mut self, num_data: usize);
    fn start(&mut self);
    fn encode_start_face_configuration(&mut self, interior: bool);
    fn new_corner_reached(
        &mut self,
        _corner: draco_core::attributes::geometry_indices::CornerIndex,
    );
    fn encode_symbol(&mut self, symbol: EdgebreakerTopologyBitPattern);
    fn encode_attribute_seam(&mut self, attribute: usize, is_seam: bool);
    fn done(&mut self);
    fn num_encoded_symbols(&self) -> usize;
    fn buffer(&self) -> &EncoderBuffer;
    /// Returns the raw symbol sequence for Standard encoder (for parity debugging).
    fn standard_symbols(&self) -> Option<&[EdgebreakerTopologyBitPattern]> {
        None
    }
}

pub struct MeshEdgebreakerTraversalEncoder {
    start_face_encoder: RAnsBitEncoder,
    traversal_buffer: EncoderBuffer,
    encoder_impl: Option<*const dyn MeshEdgebreakerEncoderImplInterface>,
    symbols: Vec<EdgebreakerTopologyBitPattern>,
    attribute_connectivity_encoders: Vec<RAnsBitEncoder>,
    num_attribute_data: usize,
}

impl MeshEdgebreakerTraversalEncoder {
    pub fn new() -> Self {
        Self {
            start_face_encoder: RAnsBitEncoder::new(),
            traversal_buffer: EncoderBuffer::new(),
            encoder_impl: None,
            symbols: Vec::new(),
            attribute_connectivity_encoders: Vec::new(),
            num_attribute_data: 0,
        }
    }

    pub(crate) fn encode_traversal_symbols(&mut self) {
        let encoder_ptr = match self.encoder_impl {
            Some(ptr) => ptr,
            None => return,
        };
        let encoder_impl = unsafe { &*encoder_ptr };
        let num_faces = encoder_impl
            .get_encoder()
            .mesh()
            .map(|m| m.num_faces() as i64)
            .unwrap_or(0);
        if num_faces <= 0 {
            return;
        }
        self.traversal_buffer
            .start_bit_encoding(num_faces * 3, true);
        for symbol in self.symbols.iter().rev() {
            let symbol_id = *symbol as u32;
            let nbits = EDGE_BREAKER_TOPOLOGY_BIT_PATTERN_LENGTH[symbol_id as usize];
            if nbits > 0 {
                self.traversal_buffer
                    .encode_least_significant_bits32(nbits, symbol_id);
            }
        }
        self.traversal_buffer.end_bit_encoding();
    }

    pub(crate) fn encode_start_faces(&mut self) {
        self.start_face_encoder
            .end_encoding(&mut self.traversal_buffer);
    }

    pub(crate) fn encode_attribute_seams(&mut self) {
        for encoder in &mut self.attribute_connectivity_encoders {
            encoder.end_encoding(&mut self.traversal_buffer);
        }
    }

    pub(crate) fn buffer_mut(&mut self) -> &mut EncoderBuffer {
        &mut self.traversal_buffer
    }
}

impl Default for MeshEdgebreakerTraversalEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgebreakerTraversalEncoder for MeshEdgebreakerTraversalEncoder {
    fn init(&mut self, encoder: *const dyn MeshEdgebreakerEncoderImplInterface) -> bool {
        self.encoder_impl = Some(encoder);
        true
    }

    fn set_num_attribute_data(&mut self, num_data: usize) {
        self.num_attribute_data = num_data;
    }

    fn start(&mut self) {
        self.start_face_encoder.start_encoding();
        if self.num_attribute_data > 0 {
            self.attribute_connectivity_encoders = (0..self.num_attribute_data)
                .map(|_| RAnsBitEncoder::new())
                .collect();
            for enc in &mut self.attribute_connectivity_encoders {
                enc.start_encoding();
            }
        }
    }

    fn encode_start_face_configuration(&mut self, interior: bool) {
        self.start_face_encoder.encode_bit(interior);
    }

    fn new_corner_reached(
        &mut self,
        _corner: draco_core::attributes::geometry_indices::CornerIndex,
    ) {
    }

    fn encode_symbol(&mut self, symbol: EdgebreakerTopologyBitPattern) {
        self.symbols.push(symbol);
    }

    fn encode_attribute_seam(&mut self, attribute: usize, is_seam: bool) {
        self.attribute_connectivity_encoders[attribute].encode_bit(is_seam);
    }

    fn done(&mut self) {
        self.encode_traversal_symbols();
        self.encode_start_faces();
        self.encode_attribute_seams();
    }

    fn num_encoded_symbols(&self) -> usize {
        self.symbols.len()
    }

    fn buffer(&self) -> &EncoderBuffer {
        &self.traversal_buffer
    }

    fn standard_symbols(&self) -> Option<&[EdgebreakerTopologyBitPattern]> {
        Some(&self.symbols)
    }
}
