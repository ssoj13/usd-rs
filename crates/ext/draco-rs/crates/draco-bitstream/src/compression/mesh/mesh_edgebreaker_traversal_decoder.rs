//! Edgebreaker traversal decoder (standard).
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_traversal_decoder.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, VertexIndex};
use draco_core::core::decoder_buffer::DecoderBuffer;

use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::config::compression_shared::bitstream_version;
use crate::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use crate::compression::mesh::mesh_edgebreaker_decoder_impl_interface::MeshEdgebreakerDecoderImplInterface;
use crate::compression::point_cloud::PointCloudDecoder;

pub trait EdgebreakerTraversalDecoder {
    fn init(&mut self, decoder: *const dyn MeshEdgebreakerDecoderImplInterface);
    fn bitstream_version(&self) -> u16;
    fn set_num_encoded_vertices(&mut self, num_vertices: i32);
    fn set_num_attribute_data(&mut self, num_data: usize);
    fn start(&mut self, out_buffer: &mut DecoderBuffer) -> bool;
    fn decode_start_face_configuration(&mut self) -> bool;
    fn decode_symbol(&mut self) -> u32;
    fn new_active_corner_reached(&mut self, _corner: CornerIndex);
    fn merge_vertices(&mut self, _dest: VertexIndex, _source: VertexIndex);
    fn decode_attribute_seam(&mut self, attribute: usize) -> bool;
    fn done(&mut self);
}

pub struct MeshEdgebreakerTraversalDecoder {
    pub(crate) buffer: DecoderBuffer<'static>,
    symbol_buffer: DecoderBuffer<'static>,
    start_face_decoder: RAnsBitDecoder,
    start_face_buffer: DecoderBuffer<'static>,
    attribute_connectivity_decoders: Vec<RAnsBitDecoder>,
    num_attribute_data: usize,
    decoder_impl: Option<*const dyn MeshEdgebreakerDecoderImplInterface>,
}

impl MeshEdgebreakerTraversalDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clone_buffer(src: &DecoderBuffer<'static>) -> DecoderBuffer<'static> {
        let mut out = DecoderBuffer::new();
        out.init_with_version(src.data(), src.bitstream_version());
        out.start_decoding_from(src.position() as i64);
        out
    }

    fn init_from_decoder(&mut self) {
        let decoder_ptr = match self.decoder_impl {
            Some(ptr) => ptr,
            None => return,
        };
        let decoder_impl = unsafe { &*decoder_ptr };
        let decoder = decoder_impl.get_decoder();
        let buf = match decoder.buffer() {
            Some(buf) => buf,
            None => return,
        };
        let data_head = buf.data_head();
        let version = buf.bitstream_version();
        // Copy remaining data into an owned buffer and leak to obtain &'static [u8].
        // The leaked memory is intentional: traversal decoder output (data_head) is used to
        // re-init the main decode buffer, which outlives the traversal decoder. To avoid UB
        // from transmuting lifetimes, we copy and leak. One-shot decode typically processes
        // one asset; the leak is bounded by the connectivity payload size.
        let owned: Box<[u8]> = data_head.to_vec().into_boxed_slice();
        let data: &'static [u8] = Box::leak(owned);
        self.buffer.init_with_version(data, version);
    }

    pub(crate) fn decode_traversal_symbols(&mut self) -> bool {
        let mut traversal_size: u64 = 0;
        self.symbol_buffer = Self::clone_buffer(&self.buffer);
        if !self
            .symbol_buffer
            .start_bit_decoding(true, &mut traversal_size)
        {
            return false;
        }
        self.buffer = Self::clone_buffer(&self.symbol_buffer);
        if traversal_size as i64 > self.buffer.remaining_size() {
            return false;
        }
        self.buffer.advance(traversal_size as i64);
        true
    }

    pub(crate) fn decode_start_faces(&mut self) -> bool {
        if self.buffer.bitstream_version() < bitstream_version(2, 2) {
            self.start_face_buffer = Self::clone_buffer(&self.buffer);
            let mut traversal_size: u64 = 0;
            if !self
                .start_face_buffer
                .start_bit_decoding(true, &mut traversal_size)
            {
                return false;
            }
            self.buffer = Self::clone_buffer(&self.start_face_buffer);
            if traversal_size as i64 > self.buffer.remaining_size() {
                return false;
            }
            self.buffer.advance(traversal_size as i64);
            return true;
        }
        self.start_face_decoder.start_decoding(&mut self.buffer)
    }

    pub(crate) fn decode_attribute_seams(&mut self) -> bool {
        if self.num_attribute_data == 0 {
            return true;
        }
        self.attribute_connectivity_decoders = (0..self.num_attribute_data)
            .map(|_| RAnsBitDecoder::new())
            .collect();
        for decoder in &mut self.attribute_connectivity_decoders {
            if !decoder.start_decoding(&mut self.buffer) {
                return false;
            }
        }
        true
    }
}

impl EdgebreakerTraversalDecoder for MeshEdgebreakerTraversalDecoder {
    fn init(&mut self, decoder: *const dyn MeshEdgebreakerDecoderImplInterface) {
        self.decoder_impl = Some(decoder);
        self.init_from_decoder();
    }

    fn bitstream_version(&self) -> u16 {
        self.buffer.bitstream_version()
    }

    fn set_num_encoded_vertices(&mut self, _num_vertices: i32) {}

    fn set_num_attribute_data(&mut self, num_data: usize) {
        self.num_attribute_data = num_data;
    }

    fn start(&mut self, out_buffer: &mut DecoderBuffer) -> bool {
        if !self.decode_traversal_symbols() {
            return false;
        }
        if !self.decode_start_faces() {
            return false;
        }
        if !self.decode_attribute_seams() {
            return false;
        }
        *out_buffer = Self::clone_buffer(&self.buffer);
        true
    }

    fn decode_start_face_configuration(&mut self) -> bool {
        let mut face_configuration: u32 = 0;
        if self.buffer.bitstream_version() < bitstream_version(2, 2) {
            self.start_face_buffer
                .decode_least_significant_bits32(1, &mut face_configuration);
        } else {
            face_configuration = if self.start_face_decoder.decode_next_bit() {
                1
            } else {
                0
            };
        }
        face_configuration != 0
    }

    fn decode_symbol(&mut self) -> u32 {
        let mut symbol: u32 = 0;
        self.symbol_buffer
            .decode_least_significant_bits32(1, &mut symbol);
        if symbol == EdgebreakerTopologyBitPattern::TopologyC as u32 {
            return symbol;
        }
        let mut symbol_suffix: u32 = 0;
        self.symbol_buffer
            .decode_least_significant_bits32(2, &mut symbol_suffix);
        symbol |= symbol_suffix << 1;
        symbol
    }

    fn new_active_corner_reached(&mut self, _corner: CornerIndex) {}

    fn merge_vertices(&mut self, _dest: VertexIndex, _source: VertexIndex) {}

    fn decode_attribute_seam(&mut self, attribute: usize) -> bool {
        self.attribute_connectivity_decoders[attribute].decode_next_bit()
    }

    fn done(&mut self) {
        if self.symbol_buffer.bit_decoder_active() {
            self.symbol_buffer.end_bit_decoding();
        }
        if self.buffer.bitstream_version() < bitstream_version(2, 2) {
            self.start_face_buffer.end_bit_decoding();
        } else {
            self.start_face_decoder.clear();
        }
    }
}

impl Default for MeshEdgebreakerTraversalDecoder {
    fn default() -> Self {
        Self {
            buffer: DecoderBuffer::new(),
            symbol_buffer: DecoderBuffer::new(),
            start_face_decoder: RAnsBitDecoder::new(),
            start_face_buffer: DecoderBuffer::new(),
            attribute_connectivity_decoders: Vec::new(),
            num_attribute_data: 0,
            decoder_impl: None,
        }
    }
}
