//! Edgebreaker predictive traversal encoder.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_traversal_predictive_encoder.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, VertexIndex};
use draco_core::core::encoder_buffer::EncoderBuffer;

use crate::compression::bit_coders::rans_bit_encoder::RAnsBitEncoder;
use crate::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use crate::compression::mesh::mesh_edgebreaker_encoder_impl_interface::MeshEdgebreakerEncoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_encoder::{
    EdgebreakerTraversalEncoder, MeshEdgebreakerTraversalEncoder,
};

pub struct MeshEdgebreakerTraversalPredictiveEncoder {
    base: MeshEdgebreakerTraversalEncoder,
    corner_table: *const draco_core::mesh::corner_table::CornerTable,
    vertex_valences: Vec<i32>,
    predictions: Vec<bool>,
    prev_symbol: i32,
    num_split_symbols: i32,
    last_corner: CornerIndex,
    num_symbols: i32,
}

impl MeshEdgebreakerTraversalPredictiveEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    fn compute_predicted_symbol(&self, pivot: VertexIndex) -> i32 {
        let valence = self.vertex_valences[pivot.value() as usize];
        if valence < 0 {
            return EdgebreakerTopologyBitPattern::TopologyInvalid as i32;
        }
        if valence < 6 {
            return EdgebreakerTopologyBitPattern::TopologyR as i32;
        }
        EdgebreakerTopologyBitPattern::TopologyC as i32
    }

    fn output_buffer_mut(&mut self) -> &mut EncoderBuffer {
        self.base.buffer_mut()
    }

    fn pattern_from_i32(&self, value: i32) -> EdgebreakerTopologyBitPattern {
        match value as u32 {
            x if x == EdgebreakerTopologyBitPattern::TopologyC as u32 => {
                EdgebreakerTopologyBitPattern::TopologyC
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyS as u32 => {
                EdgebreakerTopologyBitPattern::TopologyS
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyL as u32 => {
                EdgebreakerTopologyBitPattern::TopologyL
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyR as u32 => {
                EdgebreakerTopologyBitPattern::TopologyR
            }
            x if x == EdgebreakerTopologyBitPattern::TopologyE as u32 => {
                EdgebreakerTopologyBitPattern::TopologyE
            }
            _ => EdgebreakerTopologyBitPattern::TopologyInvalid,
        }
    }
}

impl Default for MeshEdgebreakerTraversalPredictiveEncoder {
    fn default() -> Self {
        Self {
            base: MeshEdgebreakerTraversalEncoder::new(),
            corner_table: std::ptr::null(),
            vertex_valences: Vec::new(),
            predictions: Vec::new(),
            prev_symbol: -1,
            num_split_symbols: 0,
            last_corner: draco_core::attributes::geometry_indices::INVALID_CORNER_INDEX,
            num_symbols: 0,
        }
    }
}

impl EdgebreakerTraversalEncoder for MeshEdgebreakerTraversalPredictiveEncoder {
    fn init(&mut self, encoder: *const dyn MeshEdgebreakerEncoderImplInterface) -> bool {
        if !self.base.init(encoder) {
            return false;
        }
        let encoder_impl = unsafe { &*encoder };
        if let Some(ct) = encoder_impl.get_corner_table() {
            self.corner_table = ct as *const _;
            let num_vertices = ct.num_vertices();
            self.vertex_valences.resize(num_vertices, 0);
            for i in 0..num_vertices {
                self.vertex_valences[i] = ct.valence(VertexIndex::from(i as u32));
            }
        }
        true
    }

    fn set_num_attribute_data(&mut self, num_data: usize) {
        self.base.set_num_attribute_data(num_data);
    }

    fn start(&mut self) {
        self.base.start();
    }

    fn encode_start_face_configuration(&mut self, interior: bool) {
        self.base.encode_start_face_configuration(interior);
    }

    fn new_corner_reached(&mut self, corner: CornerIndex) {
        self.last_corner = corner;
    }

    fn encode_symbol(&mut self, symbol: EdgebreakerTopologyBitPattern) {
        self.num_symbols += 1;
        let corner_table = unsafe { &*self.corner_table };
        let next = corner_table.next(self.last_corner);
        let prev = corner_table.previous(self.last_corner);

        let mut predicted_symbol = -1;
        match symbol {
            EdgebreakerTopologyBitPattern::TopologyC => {
                predicted_symbol = self.compute_predicted_symbol(corner_table.vertex(next));
                self.vertex_valences[corner_table.vertex(next).value() as usize] -= 1;
                self.vertex_valences[corner_table.vertex(prev).value() as usize] -= 1;
            }
            EdgebreakerTopologyBitPattern::TopologyS => {
                self.vertex_valences[corner_table.vertex(next).value() as usize] -= 1;
                self.vertex_valences[corner_table.vertex(prev).value() as usize] -= 1;
                self.vertex_valences[corner_table.vertex(self.last_corner).value() as usize] = -1;
                self.num_split_symbols += 1;
            }
            EdgebreakerTopologyBitPattern::TopologyR => {
                predicted_symbol = self.compute_predicted_symbol(corner_table.vertex(next));
                self.vertex_valences[corner_table.vertex(self.last_corner).value() as usize] -= 1;
                self.vertex_valences[corner_table.vertex(next).value() as usize] -= 1;
                self.vertex_valences[corner_table.vertex(prev).value() as usize] -= 2;
            }
            EdgebreakerTopologyBitPattern::TopologyL => {
                self.vertex_valences[corner_table.vertex(self.last_corner).value() as usize] -= 1;
                self.vertex_valences[corner_table.vertex(next).value() as usize] -= 2;
                self.vertex_valences[corner_table.vertex(prev).value() as usize] -= 1;
            }
            EdgebreakerTopologyBitPattern::TopologyE => {
                self.vertex_valences[corner_table.vertex(self.last_corner).value() as usize] -= 2;
                self.vertex_valences[corner_table.vertex(next).value() as usize] -= 2;
                self.vertex_valences[corner_table.vertex(prev).value() as usize] -= 2;
            }
            _ => {}
        }

        let mut store_prev_symbol = true;
        if predicted_symbol != -1 {
            if predicted_symbol == self.prev_symbol {
                self.predictions.push(true);
                store_prev_symbol = false;
            } else if self.prev_symbol != -1 {
                self.predictions.push(false);
            }
        }
        if store_prev_symbol && self.prev_symbol != -1 {
            self.base
                .encode_symbol(self.pattern_from_i32(self.prev_symbol));
        }
        self.prev_symbol = symbol as i32;
    }

    fn encode_attribute_seam(&mut self, attribute: usize, is_seam: bool) {
        self.base.encode_attribute_seam(attribute, is_seam);
    }

    fn done(&mut self) {
        if self.prev_symbol != -1 {
            self.base
                .encode_symbol(self.pattern_from_i32(self.prev_symbol));
        }
        self.base.done();
        let num_split_symbols = self.num_split_symbols;
        self.output_buffer_mut().encode(num_split_symbols as i32);
        let mut prediction_encoder = RAnsBitEncoder::new();
        prediction_encoder.start_encoding();
        for &p in self.predictions.iter().rev() {
            prediction_encoder.encode_bit(p);
        }
        prediction_encoder.end_encoding(self.output_buffer_mut());
    }

    fn num_encoded_symbols(&self) -> usize {
        self.num_symbols as usize
    }

    fn buffer(&self) -> &EncoderBuffer {
        self.base.buffer()
    }
}
