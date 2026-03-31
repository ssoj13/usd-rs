//! Edgebreaker valence traversal decoder.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_traversal_valence_decoder.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, VertexIndex};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::varint_decoding::decode_varint;

use crate::compression::config::compression_shared::bitstream_version;
use crate::compression::entropy::symbol_decoding::decode_symbols;
use crate::compression::mesh::edgebreaker_shared::{
    EdgebreakerTopologyBitPattern, EDGE_BREAKER_SYMBOL_TO_TOPOLOGY_ID,
};
use crate::compression::mesh::mesh_edgebreaker_decoder_impl_interface::MeshEdgebreakerDecoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_decoder::{
    EdgebreakerTraversalDecoder, MeshEdgebreakerTraversalDecoder,
};

pub struct MeshEdgebreakerTraversalValenceDecoder {
    base: MeshEdgebreakerTraversalDecoder,
    corner_table: *const draco_core::mesh::corner_table::CornerTable,
    num_vertices: i32,
    vertex_valences: Vec<i32>,
    last_symbol: i32,
    active_context: i32,
    min_valence: i32,
    max_valence: i32,
    context_symbols: Vec<Vec<u32>>,
    context_counters: Vec<i32>,
}

impl MeshEdgebreakerTraversalValenceDecoder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MeshEdgebreakerTraversalValenceDecoder {
    fn default() -> Self {
        Self {
            base: MeshEdgebreakerTraversalDecoder::new(),
            corner_table: std::ptr::null(),
            num_vertices: 0,
            vertex_valences: Vec::new(),
            last_symbol: -1,
            active_context: -1,
            min_valence: 2,
            max_valence: 7,
            context_symbols: Vec::new(),
            context_counters: Vec::new(),
        }
    }
}

impl EdgebreakerTraversalDecoder for MeshEdgebreakerTraversalValenceDecoder {
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
        if self.bitstream_version() < bitstream_version(2, 2) {
            if !self.base.decode_traversal_symbols() {
                return false;
            }
        }
        if !self.base.decode_start_faces() {
            return false;
        }
        if !self.base.decode_attribute_seams() {
            return false;
        }
        *out_buffer = MeshEdgebreakerTraversalDecoder::clone_buffer(&self.base.buffer);

        if self.bitstream_version() < bitstream_version(2, 2) {
            let mut num_split_symbols: u32 = 0;
            if self.bitstream_version() < bitstream_version(2, 0) {
                if !out_buffer.decode(&mut num_split_symbols) {
                    return false;
                }
            } else if !decode_varint(&mut num_split_symbols, out_buffer) {
                return false;
            }
            if num_split_symbols as i32 >= self.num_vertices {
                return false;
            }
            let mut mode: i8 = 0;
            if !out_buffer.decode(&mut mode) {
                return false;
            }
            if mode == 0 {
                self.min_valence = 2;
                self.max_valence = 7;
            } else {
                return false;
            }
        } else {
            self.min_valence = 2;
            self.max_valence = 7;
        }

        if self.num_vertices < 0 {
            return false;
        }
        self.vertex_valences.resize(self.num_vertices as usize, 0);

        let num_unique_valences = (self.max_valence - self.min_valence + 1) as usize;
        self.context_symbols = vec![Vec::new(); num_unique_valences];
        self.context_counters = vec![0; num_unique_valences];
        for i in 0..num_unique_valences {
            let mut num_symbols: u32 = 0;
            if !decode_varint(&mut num_symbols, out_buffer) {
                return false;
            }
            let corner_table = unsafe { &*self.corner_table };
            if num_symbols as usize > corner_table.num_faces() {
                return false;
            }
            if num_symbols > 0 {
                self.context_symbols[i].resize(num_symbols as usize, 0);
                if !decode_symbols(num_symbols, 1, out_buffer, &mut self.context_symbols[i]) {
                    return false;
                }
                self.context_counters[i] = num_symbols as i32;
            }
        }
        true
    }

    fn decode_start_face_configuration(&mut self) -> bool {
        self.base.decode_start_face_configuration()
    }

    fn decode_symbol(&mut self) -> u32 {
        if self.active_context != -1 {
            let idx = self.active_context as usize;
            self.context_counters[idx] -= 1;
            if self.context_counters[idx] < 0 {
                return EdgebreakerTopologyBitPattern::TopologyInvalid as u32;
            }
            let symbol_id = self.context_symbols[idx][self.context_counters[idx] as usize];
            if symbol_id > 4 {
                return EdgebreakerTopologyBitPattern::TopologyInvalid as u32;
            }
            self.last_symbol = EDGE_BREAKER_SYMBOL_TO_TOPOLOGY_ID[symbol_id as usize] as i32;
        } else {
            if self.bitstream_version() < bitstream_version(2, 2) {
                self.last_symbol = self.base.decode_symbol() as i32;
            } else {
                self.last_symbol = EdgebreakerTopologyBitPattern::TopologyE as i32;
            }
        }
        self.last_symbol as u32
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
        let active_valence = self.vertex_valences[corner_table.vertex(next).value() as usize];
        let mut clamped = active_valence;
        if clamped < self.min_valence {
            clamped = self.min_valence;
        } else if clamped > self.max_valence {
            clamped = self.max_valence;
        }
        self.active_context = clamped - self.min_valence;
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
