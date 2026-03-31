//! Edgebreaker valence traversal encoder.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_traversal_valence_encoder.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, VertexIndex, INVALID_CORNER_INDEX};
use draco_core::core::draco_index_type_vector::IndexTypeVector;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::varint_encoding::encode_varint;

use crate::compression::entropy::symbol_encoding::encode_symbols;
use crate::compression::mesh::edgebreaker_shared::{
    EdgebreakerTopologyBitPattern, EDGE_BREAKER_TOPOLOGY_TO_SYMBOL_ID,
};
use crate::compression::mesh::mesh_edgebreaker_encoder_impl_interface::MeshEdgebreakerEncoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_encoder::{
    EdgebreakerTraversalEncoder, MeshEdgebreakerTraversalEncoder,
};

pub struct MeshEdgebreakerTraversalValenceEncoder {
    base: MeshEdgebreakerTraversalEncoder,
    encoder_impl: Option<*const dyn MeshEdgebreakerEncoderImplInterface>,
    corner_table: *const draco_core::mesh::corner_table::CornerTable,
    corner_to_vertex_map: IndexTypeVector<CornerIndex, VertexIndex>,
    vertex_valences: IndexTypeVector<VertexIndex, i32>,
    prev_symbol: i32,
    last_corner: CornerIndex,
    num_symbols: i32,
    min_valence: i32,
    max_valence: i32,
    context_symbols: Vec<Vec<u32>>,
}

impl MeshEdgebreakerTraversalValenceEncoder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MeshEdgebreakerTraversalValenceEncoder {
    fn default() -> Self {
        Self {
            base: MeshEdgebreakerTraversalEncoder::new(),
            encoder_impl: None,
            corner_table: std::ptr::null(),
            corner_to_vertex_map: IndexTypeVector::new(),
            vertex_valences: IndexTypeVector::new(),
            prev_symbol: -1,
            last_corner: draco_core::attributes::geometry_indices::INVALID_CORNER_INDEX,
            num_symbols: 0,
            min_valence: 2,
            max_valence: 7,
            context_symbols: Vec::new(),
        }
    }
}

impl EdgebreakerTraversalEncoder for MeshEdgebreakerTraversalValenceEncoder {
    fn init(&mut self, encoder: *const dyn MeshEdgebreakerEncoderImplInterface) -> bool {
        if !self.base.init(encoder) {
            return false;
        }
        self.encoder_impl = Some(encoder);
        let encoder_impl = unsafe { &*encoder };
        if let Some(ct) = encoder_impl.get_corner_table() {
            self.corner_table = ct as *const _;
            self.min_valence = 2;
            self.max_valence = 7;
            let num_vertices = ct.num_vertices();
            self.vertex_valences = IndexTypeVector::with_size_value(num_vertices, 0);
            for i in 0..num_vertices {
                self.vertex_valences[VertexIndex::from(i as u32)] =
                    ct.valence(VertexIndex::from(i as u32));
            }
            let num_corners = ct.num_corners();
            self.corner_to_vertex_map =
                IndexTypeVector::with_size_value(num_corners, VertexIndex::from(0u32));
            for i in 0..num_corners {
                let c = CornerIndex::from(i as u32);
                self.corner_to_vertex_map[c] = ct.vertex(c);
            }
            let num_unique_valences = (self.max_valence - self.min_valence + 1) as usize;
            self.context_symbols = vec![Vec::new(); num_unique_valences];
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
        let active_valence = self.vertex_valences[self.corner_to_vertex_map[next]];

        match symbol {
            EdgebreakerTopologyBitPattern::TopologyC | EdgebreakerTopologyBitPattern::TopologyS => {
                self.vertex_valences[self.corner_to_vertex_map[next]] -= 1;
                self.vertex_valences[self.corner_to_vertex_map[prev]] -= 1;
                if symbol == EdgebreakerTopologyBitPattern::TopologyS {
                    let mut num_left_faces = 0;
                    let mut act_c = corner_table.opposite(prev);
                    while act_c != INVALID_CORNER_INDEX {
                        let encoder_ptr = match self.encoder_impl {
                            Some(ptr) => ptr,
                            None => return,
                        };
                        let encoder_impl = unsafe { &*encoder_ptr };
                        if encoder_impl.is_face_encoded(corner_table.face(act_c)) {
                            break;
                        }
                        num_left_faces += 1;
                        act_c = corner_table.opposite(corner_table.next(act_c));
                    }
                    self.vertex_valences[self.corner_to_vertex_map[self.last_corner]] =
                        num_left_faces + 1;

                    let new_vert_id = self.vertex_valences.size() as i32;
                    let mut num_right_faces = 0;
                    act_c = corner_table.opposite(next);
                    while act_c != INVALID_CORNER_INDEX {
                        let encoder_ptr = match self.encoder_impl {
                            Some(ptr) => ptr,
                            None => return,
                        };
                        let encoder_impl = unsafe { &*encoder_ptr };
                        if encoder_impl.is_face_encoded(corner_table.face(act_c)) {
                            break;
                        }
                        num_right_faces += 1;
                        self.corner_to_vertex_map[corner_table.next(act_c)] =
                            VertexIndex::from(new_vert_id as u32);
                        act_c = corner_table.opposite(corner_table.previous(act_c));
                    }
                    self.vertex_valences.push_back(num_right_faces + 1);
                }
            }
            EdgebreakerTopologyBitPattern::TopologyR => {
                self.vertex_valences[self.corner_to_vertex_map[self.last_corner]] -= 1;
                self.vertex_valences[self.corner_to_vertex_map[next]] -= 1;
                self.vertex_valences[self.corner_to_vertex_map[prev]] -= 2;
            }
            EdgebreakerTopologyBitPattern::TopologyL => {
                self.vertex_valences[self.corner_to_vertex_map[self.last_corner]] -= 1;
                self.vertex_valences[self.corner_to_vertex_map[next]] -= 2;
                self.vertex_valences[self.corner_to_vertex_map[prev]] -= 1;
            }
            EdgebreakerTopologyBitPattern::TopologyE => {
                self.vertex_valences[self.corner_to_vertex_map[self.last_corner]] -= 2;
                self.vertex_valences[self.corner_to_vertex_map[next]] -= 2;
                self.vertex_valences[self.corner_to_vertex_map[prev]] -= 2;
            }
            _ => {}
        }

        if self.prev_symbol != -1 {
            let mut clamped_valence = active_valence;
            if clamped_valence < self.min_valence {
                clamped_valence = self.min_valence;
            } else if clamped_valence > self.max_valence {
                clamped_valence = self.max_valence;
            }
            let context = (clamped_valence - self.min_valence) as usize;
            let symbol_id = EDGE_BREAKER_TOPOLOGY_TO_SYMBOL_ID[self.prev_symbol as usize] as u32;
            self.context_symbols[context].push(symbol_id);
        }
        self.prev_symbol = symbol as i32;
    }

    fn encode_attribute_seam(&mut self, attribute: usize, is_seam: bool) {
        self.base.encode_attribute_seam(attribute, is_seam);
    }

    fn done(&mut self) {
        // Encode start faces and attribute seams first.
        self.base.encode_start_faces();
        self.base.encode_attribute_seams();

        // Encode context symbols.
        for symbols in &self.context_symbols {
            encode_varint(symbols.len() as u32, self.base.buffer_mut());
            if !symbols.is_empty() {
                encode_symbols(
                    symbols,
                    symbols.len() as i32,
                    1,
                    None,
                    self.base.buffer_mut(),
                );
            }
        }
    }

    fn num_encoded_symbols(&self) -> usize {
        self.num_symbols as usize
    }

    fn buffer(&self) -> &EncoderBuffer {
        self.base.buffer()
    }
}
