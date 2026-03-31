//! Edgebreaker predictive traversal decoder.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_traversal_predictive_decoder.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, VertexIndex};
use draco_core::core::decoder_buffer::DecoderBuffer;

use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use crate::compression::mesh::mesh_edgebreaker_decoder_impl_interface::MeshEdgebreakerDecoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_decoder::{
    EdgebreakerTraversalDecoder, MeshEdgebreakerTraversalDecoder,
};

pub struct MeshEdgebreakerTraversalPredictiveDecoder {
    base: MeshEdgebreakerTraversalDecoder,
    corner_table: *const draco_core::mesh::corner_table::CornerTable,
    num_vertices: i32,
    vertex_valences: Vec<i32>,
    prediction_decoder: RAnsBitDecoder,
    last_symbol: i32,
    predicted_symbol: i32,
}

impl MeshEdgebreakerTraversalPredictiveDecoder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MeshEdgebreakerTraversalPredictiveDecoder {
    fn default() -> Self {
        Self {
            base: MeshEdgebreakerTraversalDecoder::new(),
            corner_table: std::ptr::null(),
            num_vertices: 0,
            vertex_valences: Vec::new(),
            prediction_decoder: RAnsBitDecoder::new(),
            last_symbol: -1,
            predicted_symbol: -1,
        }
    }
}

impl EdgebreakerTraversalDecoder for MeshEdgebreakerTraversalPredictiveDecoder {
    fn init(&mut self, decoder: *const dyn MeshEdgebreakerDecoderImplInterface) {
        self.base.init(decoder);
        let decoder_impl = unsafe { &*decoder };
        if let Some(ct) = decoder_impl.get_corner_table() {
            self.corner_table = ct as *const _;
        }
    }

    fn bitstream_version(&self) -> u16 {
        self.base.bitstream_version()
    }

    fn set_num_encoded_vertices(&mut self, num_vertices: i32) {
        self.num_vertices = num_vertices;
    }

    fn set_num_attribute_data(&mut self, num_data: usize) {
        self.base.set_num_attribute_data(num_data);
    }

    fn start(&mut self, out_buffer: &mut DecoderBuffer) -> bool {
        if !self.base.start(out_buffer) {
            return false;
        }
        let mut num_split_symbols: i32 = 0;
        if !out_buffer.decode(&mut num_split_symbols) || num_split_symbols < 0 {
            return false;
        }
        if num_split_symbols >= self.num_vertices {
            return false;
        }
        if self.num_vertices < 0 {
            return false;
        }
        self.vertex_valences.resize(self.num_vertices as usize, 0);
        if !self.prediction_decoder.start_decoding(out_buffer) {
            return false;
        }
        true
    }

    fn decode_start_face_configuration(&mut self) -> bool {
        self.base.decode_start_face_configuration()
    }

    fn decode_symbol(&mut self) -> u32 {
        if self.predicted_symbol != -1 {
            if self.prediction_decoder.decode_next_bit() {
                self.last_symbol = self.predicted_symbol;
                return self.predicted_symbol as u32;
            }
        }
        let decoded = self.base.decode_symbol();
        self.last_symbol = decoded as i32;
        decoded
    }

    fn new_active_corner_reached(&mut self, corner: CornerIndex) {
        let corner_table = unsafe { &*self.corner_table };
        let next = corner_table.next(corner);
        let prev = corner_table.previous(corner);
        match self.last_symbol as u32 {
            x if x == EdgebreakerTopologyBitPattern::TopologyC as u32
                || x == EdgebreakerTopologyBitPattern::TopologyS as u32 =>
            {
                let v_next = corner_table.vertex(next).value() as usize;
                let v_prev = corner_table.vertex(prev).value() as usize;
                self.vertex_valences[v_next] += 1;
                self.vertex_valences[v_prev] += 1;
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyR as u32 => {
                let v_corner = corner_table.vertex(corner).value() as usize;
                let v_next = corner_table.vertex(next).value() as usize;
                let v_prev = corner_table.vertex(prev).value() as usize;
                self.vertex_valences[v_corner] += 1;
                self.vertex_valences[v_next] += 1;
                self.vertex_valences[v_prev] += 2;
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyL as u32 => {
                let v_corner = corner_table.vertex(corner).value() as usize;
                let v_next = corner_table.vertex(next).value() as usize;
                let v_prev = corner_table.vertex(prev).value() as usize;
                self.vertex_valences[v_corner] += 1;
                self.vertex_valences[v_next] += 2;
                self.vertex_valences[v_prev] += 1;
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyE as u32 => {
                let v_corner = corner_table.vertex(corner).value() as usize;
                let v_next = corner_table.vertex(next).value() as usize;
                let v_prev = corner_table.vertex(prev).value() as usize;
                self.vertex_valences[v_corner] += 2;
                self.vertex_valences[v_next] += 2;
                self.vertex_valences[v_prev] += 2;
            }
            _ => {}
        }

        if self.last_symbol == EdgebreakerTopologyBitPattern::TopologyC as i32
            || self.last_symbol == EdgebreakerTopologyBitPattern::TopologyR as i32
        {
            let pivot = corner_table.vertex(corner_table.next(corner));
            if self.vertex_valences[pivot.value() as usize] < 6 {
                self.predicted_symbol = EdgebreakerTopologyBitPattern::TopologyR as i32;
            } else {
                self.predicted_symbol = EdgebreakerTopologyBitPattern::TopologyC as i32;
            }
        } else {
            self.predicted_symbol = -1;
        }
    }

    fn merge_vertices(&mut self, dest: VertexIndex, source: VertexIndex) {
        let d = dest.value() as usize;
        let s = source.value() as usize;
        self.vertex_valences[d] += self.vertex_valences[s];
    }

    fn decode_attribute_seam(&mut self, attribute: usize) -> bool {
        self.base.decode_attribute_seam(attribute)
    }

    fn done(&mut self) {
        self.base.done();
    }
}
