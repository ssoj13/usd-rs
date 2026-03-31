//! Edgebreaker encoder implementation.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_encoder_impl.h|cc`.

use std::collections::HashMap;

use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{
    CornerIndex, FaceIndex, INVALID_CORNER_INDEX, INVALID_FACE_INDEX,
};
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::core::varint_encoding::encode_varint;
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh::Mesh;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;
use draco_core::mesh::mesh_misc_functions::{
    create_corner_table_from_all_attributes, create_corner_table_from_position_attribute,
};

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::attributes::points_sequencer::PointsSequencer;
use crate::compression::attributes::sequential_attribute_encoders_controller::SequentialAttributeEncodersController;
use crate::compression::config::compression_shared::MeshTraversalMethod;
use crate::compression::mesh::edgebreaker_shared::{
    EdgeFaceName, EdgebreakerTopologyBitPattern, TopologySplitEventData,
};
use crate::compression::mesh::mesh_edgebreaker_encoder::MeshEdgebreakerEncoder;
use crate::compression::mesh::mesh_edgebreaker_encoder_impl_interface::MeshEdgebreakerEncoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_encoder::EdgebreakerTraversalEncoder;
use crate::compression::mesh::traverser::{
    DepthFirstTraverser, MaxPredictionDegreeTraverser, MeshAttributeIndicesEncodingObserver,
    MeshTraversalSequencer,
};
use crate::compression::mesh::MeshEncoder;
use crate::compression::point_cloud::PointCloudEncoder;

pub struct MeshEdgebreakerEncoderImpl<TraversalEncoderT>
where
    TraversalEncoderT: EdgebreakerTraversalEncoder + Default + 'static,
{
    encoder: *mut MeshEdgebreakerEncoder,
    mesh: *const Mesh,
    corner_table: Option<Box<CornerTable>>,
    corner_traversal_stack: Vec<CornerIndex>,
    visited_faces: Vec<bool>,
    pos_encoding_data: MeshAttributeIndicesEncodingData,
    pos_traversal_method: MeshTraversalMethod,
    processed_connectivity_corners: Vec<CornerIndex>,
    visited_vertex_ids: Vec<bool>,
    vertex_traversal_length: Vec<i32>,
    topology_split_event_data: Vec<TopologySplitEventData>,
    face_to_split_symbol_map: HashMap<i32, i32>,
    visited_holes: Vec<bool>,
    vertex_hole_id: Vec<i32>,
    last_encoded_symbol_id: i32,
    num_split_symbols: u32,
    attribute_data: Vec<AttributeData>,
    attribute_encoder_to_data_id_map: Vec<i32>,
    traversal_encoder: TraversalEncoderT,
    use_single_connectivity: bool,
}

struct AttributeData {
    attribute_index: i32,
    connectivity_data: MeshAttributeCornerTable<'static>,
    is_connectivity_used: bool,
    encoding_data: MeshAttributeIndicesEncodingData,
    traversal_method: MeshTraversalMethod,
}

impl AttributeData {
    fn new() -> Self {
        Self {
            attribute_index: -1,
            connectivity_data: MeshAttributeCornerTable::new(),
            is_connectivity_used: true,
            encoding_data: MeshAttributeIndicesEncodingData::new(),
            traversal_method: MeshTraversalMethod::MeshTraversalDepthFirst,
        }
    }
}

impl<TraversalEncoderT> MeshEdgebreakerEncoderImpl<TraversalEncoderT>
where
    TraversalEncoderT: EdgebreakerTraversalEncoder + Default + 'static,
{
    pub fn new() -> Self {
        Self {
            encoder: std::ptr::null_mut(),
            mesh: std::ptr::null(),
            corner_table: None,
            corner_traversal_stack: Vec::new(),
            visited_faces: Vec::new(),
            pos_encoding_data: MeshAttributeIndicesEncodingData::new(),
            pos_traversal_method: MeshTraversalMethod::MeshTraversalDepthFirst,
            processed_connectivity_corners: Vec::new(),
            visited_vertex_ids: Vec::new(),
            vertex_traversal_length: Vec::new(),
            topology_split_event_data: Vec::new(),
            face_to_split_symbol_map: HashMap::new(),
            visited_holes: Vec::new(),
            vertex_hole_id: Vec::new(),
            last_encoded_symbol_id: -1,
            num_split_symbols: 0,
            attribute_data: Vec::new(),
            attribute_encoder_to_data_id_map: Vec::new(),
            traversal_encoder: TraversalEncoderT::default(),
            use_single_connectivity: false,
        }
    }

    fn encoder(&self) -> &MeshEdgebreakerEncoder {
        unsafe { &*self.encoder }
    }

    fn encoder_mut(&mut self) -> &mut MeshEdgebreakerEncoder {
        unsafe { &mut *self.encoder }
    }

    fn mesh(&self) -> &Mesh {
        unsafe { &*self.mesh }
    }

    fn corner_table(&self) -> &CornerTable {
        self.corner_table.as_ref().expect("Corner table missing")
    }

    fn corner_table_static(&self) -> &'static CornerTable {
        unsafe { std::mem::transmute(self.corner_table()) }
    }

    fn init_attribute_data(&mut self) -> bool {
        if self.use_single_connectivity {
            return true;
        }
        let num_attributes = self.mesh().num_attributes();
        if num_attributes <= 1 {
            return true;
        }
        self.attribute_data.clear();
        self.attribute_data
            .resize_with((num_attributes - 1) as usize, AttributeData::new);
        let mesh_ptr = self.mesh as *const Mesh;
        let num_corners = self.corner_table().num_corners();
        let ct = self.corner_table_static();
        let mut data_index = 0usize;
        for i in 0..num_attributes {
            let att_index = i;
            let att = match unsafe { &*mesh_ptr }.attribute(att_index) {
                Some(att) => att,
                None => continue,
            };
            if att.attribute_type() == GeometryAttributeType::Position {
                continue;
            }
            let data = &mut self.attribute_data[data_index];
            data.attribute_index = att_index;
            data.encoding_data
                .encoded_attribute_value_index_to_corner_map
                .clear();
            data.encoding_data
                .encoded_attribute_value_index_to_corner_map
                .reserve(num_corners);
            data.encoding_data.num_values = 0;
            data.connectivity_data
                .init_from_attribute(unsafe { &*mesh_ptr }, ct, att);
            data_index += 1;
        }
        true
    }

    fn create_vertex_traversal_sequencer<TraverserT, CornerTableT>(
        &self,
        encoding_data: *mut MeshAttributeIndicesEncodingData,
        corner_table: *const CornerTableT,
    ) -> Box<dyn PointsSequencer>
    where
        TraverserT: crate::compression::mesh::traverser::MeshTraverser<
                CornerTable = CornerTableT,
                Observer = MeshAttributeIndicesEncodingObserver<CornerTableT>,
            > + Default
            + 'static,
        CornerTableT: crate::compression::mesh::traverser::TraversalCornerTable + 'static,
    {
        let mut sequencer = MeshTraversalSequencer::<TraverserT, CornerTableT>::new(
            self.mesh as *const Mesh,
            encoding_data,
            corner_table,
        );
        sequencer.set_corner_order(&self.processed_connectivity_corners);
        sequencer.set_traverser(TraverserT::default());
        Box::new(sequencer)
    }

    fn find_init_face_configuration(&self, face_id: FaceIndex) -> (bool, CornerIndex) {
        let mut corner_index = CornerIndex::from(3 * face_id.value());
        for _ in 0..3 {
            if self.corner_table().opposite(corner_index) == INVALID_CORNER_INDEX {
                return (false, corner_index);
            }
            let vert_id = self.corner_table().vertex(corner_index);
            if self.vertex_hole_id[vert_id.value() as usize] != -1 {
                let mut right_corner = corner_index;
                while right_corner != INVALID_CORNER_INDEX {
                    corner_index = right_corner;
                    right_corner = self.corner_table().swing_right(right_corner);
                }
                let out_corner = self.corner_table().previous(corner_index);
                return (false, out_corner);
            }
            corner_index = self.corner_table().next(corner_index);
        }
        (true, corner_index)
    }

    fn get_right_corner(&self, corner_id: CornerIndex) -> CornerIndex {
        let next_corner_id = self.corner_table().next(corner_id);
        self.corner_table().opposite(next_corner_id)
    }

    fn get_left_corner(&self, corner_id: CornerIndex) -> CornerIndex {
        let prev_corner_id = self.corner_table().previous(corner_id);
        self.corner_table().opposite(prev_corner_id)
    }

    fn is_right_face_visited(&self, corner_id: CornerIndex) -> bool {
        let next_corner_id = self.corner_table().next(corner_id);
        let opp_corner_id = self.corner_table().opposite(next_corner_id);
        if opp_corner_id != INVALID_CORNER_INDEX {
            return self.visited_faces[self.corner_table().face(opp_corner_id).value() as usize];
        }
        true
    }

    fn is_left_face_visited(&self, corner_id: CornerIndex) -> bool {
        let prev_corner_id = self.corner_table().previous(corner_id);
        let opp_corner_id = self.corner_table().opposite(prev_corner_id);
        if opp_corner_id != INVALID_CORNER_INDEX {
            return self.visited_faces[self.corner_table().face(opp_corner_id).value() as usize];
        }
        true
    }

    fn encode_hole(&mut self, start_corner_id: CornerIndex, encode_first_vertex: bool) -> i32 {
        let mut corner_id = self.corner_table().previous(start_corner_id);
        while self.corner_table().opposite(corner_id) != INVALID_CORNER_INDEX {
            corner_id = self.corner_table().opposite(corner_id);
            corner_id = self.corner_table().next(corner_id);
        }
        let start_vertex_id = self.corner_table().vertex(start_corner_id);
        let mut num_encoded_hole_verts = 0;
        if encode_first_vertex {
            self.visited_vertex_ids[start_vertex_id.value() as usize] = true;
            num_encoded_hole_verts += 1;
        }
        let hole_id = self.vertex_hole_id[start_vertex_id.value() as usize] as usize;
        self.visited_holes[hole_id] = true;
        let mut start_vert_id = self
            .corner_table()
            .vertex(self.corner_table().next(corner_id));
        let mut act_vertex_id = self
            .corner_table()
            .vertex(self.corner_table().previous(corner_id));
        while act_vertex_id != start_vertex_id {
            start_vert_id = act_vertex_id;
            self.visited_vertex_ids[act_vertex_id.value() as usize] = true;
            num_encoded_hole_verts += 1;
            corner_id = self.corner_table().next(corner_id);
            while self.corner_table().opposite(corner_id) != INVALID_CORNER_INDEX {
                corner_id = self.corner_table().opposite(corner_id);
                corner_id = self.corner_table().next(corner_id);
            }
            act_vertex_id = self
                .corner_table()
                .vertex(self.corner_table().previous(corner_id));
        }
        let _ = start_vert_id;
        num_encoded_hole_verts
    }

    fn get_split_symbol_id_on_face(&self, face_id: i32) -> i32 {
        self.face_to_split_symbol_map
            .get(&face_id)
            .copied()
            .unwrap_or(-1)
    }

    fn check_and_store_topology_split_event(
        &mut self,
        src_symbol_id: i32,
        src_edge: EdgeFaceName,
        neighbor_face_id: i32,
    ) {
        let symbol_id = self.get_split_symbol_id_on_face(neighbor_face_id);
        if symbol_id == -1 {
            return;
        }
        let event_data = TopologySplitEventData {
            split_symbol_id: symbol_id as u32,
            source_symbol_id: src_symbol_id as u32,
            source_edge: src_edge as u32,
        };
        self.topology_split_event_data.push(event_data);
    }

    fn encode_attribute_connectivities_on_face(&mut self, corner: CornerIndex) -> bool {
        let corners = [
            corner,
            self.corner_table().next(corner),
            self.corner_table().previous(corner),
        ];
        let src_face_id = self.corner_table().face(corner);
        self.visited_faces[src_face_id.value() as usize] = true;
        for c in 0..3 {
            let opp_corner = self.corner_table().opposite(corners[c]);
            if opp_corner == INVALID_CORNER_INDEX {
                continue;
            }
            let opp_face_id = self.corner_table().face(opp_corner);
            if self.visited_faces[opp_face_id.value() as usize] {
                continue;
            }
            for i in 0..self.attribute_data.len() {
                if self.attribute_data[i]
                    .connectivity_data
                    .is_corner_opposite_to_seam_edge(corners[c])
                {
                    self.traversal_encoder.encode_attribute_seam(i, true);
                } else {
                    self.traversal_encoder.encode_attribute_seam(i, false);
                }
            }
        }
        true
    }

    fn find_holes(&mut self) -> bool {
        let num_corners = self.corner_table().num_corners();
        for i in 0..num_corners {
            let corner = CornerIndex::from(i as u32);
            if self
                .corner_table()
                .is_degenerated(self.corner_table().face(corner))
            {
                continue;
            }
            if self.corner_table().opposite(corner) == INVALID_CORNER_INDEX {
                let mut boundary_vert_id =
                    self.corner_table().vertex(self.corner_table().next(corner));
                if self.vertex_hole_id[boundary_vert_id.value() as usize] != -1 {
                    continue;
                }
                let boundary_id = self.visited_holes.len() as i32;
                self.visited_holes.push(false);
                let mut corner_id = corner;
                while self.vertex_hole_id[boundary_vert_id.value() as usize] == -1 {
                    self.vertex_hole_id[boundary_vert_id.value() as usize] = boundary_id;
                    corner_id = self.corner_table().next(corner_id);
                    while self.corner_table().opposite(corner_id) != INVALID_CORNER_INDEX {
                        corner_id = self.corner_table().opposite(corner_id);
                        corner_id = self.corner_table().next(corner_id);
                    }
                    boundary_vert_id = self
                        .corner_table()
                        .vertex(self.corner_table().next(corner_id));
                }
            }
        }
        true
    }

    fn encode_connectivity_from_corner(&mut self, mut corner_id: CornerIndex) -> bool {
        let debug_traversal = std::env::var("DRACO_DEBUG_TRAVERSAL").ok().as_deref() == Some("1");
        let mut debug_symbol_count = 0u32;
        self.corner_traversal_stack.clear();
        self.corner_traversal_stack.push(corner_id);
        let num_faces = self.mesh().num_faces() as i32;
        while !self.corner_traversal_stack.is_empty() {
            corner_id = *self.corner_traversal_stack.last().unwrap();
            if corner_id == INVALID_CORNER_INDEX
                || self.visited_faces[self.corner_table().face(corner_id).value() as usize]
            {
                self.corner_traversal_stack.pop();
                continue;
            }
            let mut num_visited_faces = 0;
            while num_visited_faces < num_faces {
                num_visited_faces += 1;
                self.last_encoded_symbol_id += 1;
                let face_id = self.corner_table().face(corner_id);
                self.visited_faces[face_id.value() as usize] = true;
                self.processed_connectivity_corners.push(corner_id);
                self.traversal_encoder.new_corner_reached(corner_id);
                let vert_id = self.corner_table().vertex(corner_id);
                let on_boundary = self.vertex_hole_id[vert_id.value() as usize] != -1;
                if !self.visited_vertex_ids[vert_id.value() as usize] {
                    self.visited_vertex_ids[vert_id.value() as usize] = true;
                    if !on_boundary {
                        if debug_traversal && debug_symbol_count < 12 {
                            eprintln!(
                                "[sym {}] C corner={} face={}",
                                debug_symbol_count,
                                corner_id.value(),
                                self.corner_table().face(corner_id).value()
                            );
                            debug_symbol_count += 1;
                        }
                        self.traversal_encoder
                            .encode_symbol(EdgebreakerTopologyBitPattern::TopologyC);
                        corner_id = self.get_right_corner(corner_id);
                        continue;
                    }
                }
                let right_corner_id = self.get_right_corner(corner_id);
                let left_corner_id = self.get_left_corner(corner_id);
                let right_face_id = self.corner_table().face(right_corner_id);
                let left_face_id = self.corner_table().face(left_corner_id);
                let right_visited = self.is_right_face_visited(corner_id);
                let left_visited = self.is_left_face_visited(corner_id);
                if right_visited {
                    if right_face_id != INVALID_FACE_INDEX {
                        self.check_and_store_topology_split_event(
                            self.last_encoded_symbol_id,
                            EdgeFaceName::RightFaceEdge,
                            right_face_id.value() as i32,
                        );
                    }
                    if left_visited {
                        if left_face_id != INVALID_FACE_INDEX {
                            self.check_and_store_topology_split_event(
                                self.last_encoded_symbol_id,
                                EdgeFaceName::LeftFaceEdge,
                                left_face_id.value() as i32,
                            );
                        }
                        if debug_traversal && debug_symbol_count < 12 {
                            eprintln!(
                                "[sym {}] E corner={} R_vis={} L_vis={}",
                                debug_symbol_count,
                                corner_id.value(),
                                right_visited,
                                left_visited
                            );
                            debug_symbol_count += 1;
                        }
                        self.traversal_encoder
                            .encode_symbol(EdgebreakerTopologyBitPattern::TopologyE);
                        self.corner_traversal_stack.pop();
                        break;
                    } else {
                        if debug_traversal && debug_symbol_count < 12 {
                            eprintln!(
                                "[sym {}] R corner={} R_vis={} L_vis={} right_face={} left_face={}",
                                debug_symbol_count,
                                corner_id.value(),
                                right_visited,
                                left_visited,
                                right_face_id.value(),
                                left_face_id.value()
                            );
                            debug_symbol_count += 1;
                        }
                        self.traversal_encoder
                            .encode_symbol(EdgebreakerTopologyBitPattern::TopologyR);
                        corner_id = left_corner_id;
                    }
                } else {
                    if left_visited {
                        if left_face_id != INVALID_FACE_INDEX {
                            self.check_and_store_topology_split_event(
                                self.last_encoded_symbol_id,
                                EdgeFaceName::LeftFaceEdge,
                                left_face_id.value() as i32,
                            );
                        }
                        if debug_traversal && debug_symbol_count < 12 {
                            eprintln!(
                                "[sym {}] L corner={} R_vis={} L_vis={} right_face={} left_face={}",
                                debug_symbol_count,
                                corner_id.value(),
                                right_visited,
                                left_visited,
                                right_face_id.value(),
                                left_face_id.value()
                            );
                            debug_symbol_count += 1;
                        }
                        self.traversal_encoder
                            .encode_symbol(EdgebreakerTopologyBitPattern::TopologyL);
                        corner_id = right_corner_id;
                    } else {
                        if debug_traversal && debug_symbol_count < 12 {
                            eprintln!(
                                "[sym {}] S corner={} R_vis={} L_vis={}",
                                debug_symbol_count,
                                corner_id.value(),
                                right_visited,
                                left_visited
                            );
                            debug_symbol_count += 1;
                        }
                        self.traversal_encoder
                            .encode_symbol(EdgebreakerTopologyBitPattern::TopologyS);
                        self.num_split_symbols += 1;
                        if on_boundary {
                            let hole_id = self.vertex_hole_id[vert_id.value() as usize] as usize;
                            if !self.visited_holes[hole_id] {
                                self.encode_hole(corner_id, false);
                            }
                        }
                        self.face_to_split_symbol_map
                            .insert(face_id.value() as i32, self.last_encoded_symbol_id);
                        if let Some(last) = self.corner_traversal_stack.last_mut() {
                            *last = left_corner_id;
                        }
                        self.corner_traversal_stack.push(right_corner_id);
                        break;
                    }
                }
            }
        }
        true
    }

    fn encode_split_data(&mut self) -> bool {
        let num_events = self.topology_split_event_data.len() as u32;
        let buffer_ptr = match self.encoder_mut().buffer() {
            Some(buf) => buf as *mut draco_core::core::encoder_buffer::EncoderBuffer,
            None => return false,
        };
        let buffer = unsafe { &mut *buffer_ptr };
        encode_varint(num_events, buffer);
        if num_events > 0 {
            let mut last_source_symbol_id: u32 = 0;
            for event in &self.topology_split_event_data {
                let source_symbol_id = event.source_symbol_id;
                encode_varint(source_symbol_id - last_source_symbol_id, buffer);
                encode_varint(source_symbol_id - event.split_symbol_id, buffer);
                last_source_symbol_id = source_symbol_id;
            }
            buffer.start_bit_encoding(num_events as i64, false);
            for event in &self.topology_split_event_data {
                buffer.encode_least_significant_bits32(1, event.source_edge);
            }
            buffer.end_bit_encoding();
        }
        true
    }
}

impl<TraversalEncoderT> Default for MeshEdgebreakerEncoderImpl<TraversalEncoderT>
where
    TraversalEncoderT: EdgebreakerTraversalEncoder + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<TraversalEncoderT> MeshEdgebreakerEncoderImplInterface
    for MeshEdgebreakerEncoderImpl<TraversalEncoderT>
where
    TraversalEncoderT: EdgebreakerTraversalEncoder + Default,
{
    fn init(&mut self, encoder: *mut MeshEdgebreakerEncoder) -> bool {
        self.encoder = encoder;
        let mesh = self.encoder().mesh();
        if let Some(mesh) = mesh {
            self.mesh = mesh as *const Mesh;
        }
        self.attribute_encoder_to_data_id_map.clear();
        let options = self.encoder().options();
        if options.is_global_option_set("split_mesh_on_seams") {
            self.use_single_connectivity = options.get_global_bool("split_mesh_on_seams", false);
        } else if options.get_speed() >= 6 {
            self.use_single_connectivity = true;
        } else {
            self.use_single_connectivity = false;
        }
        true
    }

    fn get_attribute_corner_table(&self, att_id: i32) -> Option<&MeshAttributeCornerTable<'_>> {
        for data in &self.attribute_data {
            if data.attribute_index == att_id {
                if data.is_connectivity_used {
                    return Some(&data.connectivity_data);
                }
                return None;
            }
        }
        None
    }

    fn get_attribute_encoding_data(&self, att_id: i32) -> &MeshAttributeIndicesEncodingData {
        for data in &self.attribute_data {
            if data.attribute_index == att_id {
                return &data.encoding_data;
            }
        }
        &self.pos_encoding_data
    }

    fn generate_attributes_encoder(&mut self, att_id: i32) -> bool {
        if self.use_single_connectivity && !self.encoder().base().attributes_encoders.is_empty() {
            if let Some(enc) = self.encoder_mut().attributes_encoder_mut(0) {
                enc.add_attribute_id(att_id);
                return true;
            }
        }
        let mesh_ptr = self.mesh as *const Mesh;
        let mesh = unsafe { &*mesh_ptr };
        let element_type = mesh.get_attribute_element_type(att_id);
        let att = match mesh.attribute(att_id) {
            Some(att) => att,
            None => return false,
        };
        let mut att_data_id: i32 = -1;
        for (i, data) in self.attribute_data.iter().enumerate() {
            if data.attribute_index == att_id {
                att_data_id = i as i32;
                break;
            }
        }

        let mut traversal_method = MeshTraversalMethod::MeshTraversalDepthFirst;
        let num_vertices = self.corner_table().num_vertices();
        let no_interior_seams = if att_data_id >= 0 {
            self.attribute_data[att_data_id as usize]
                .connectivity_data
                .no_interior_seams()
        } else {
            false
        };
        let speed = self.encoder().options().get_speed();
        let num_attributes = mesh.num_attributes();
        let sequencer: Option<Box<dyn PointsSequencer>> = if self.use_single_connectivity
            || att.attribute_type() == GeometryAttributeType::Position
            || element_type == draco_core::mesh::mesh::MeshAttributeElementType::MeshVertexAttribute
            || (element_type
                == draco_core::mesh::mesh::MeshAttributeElementType::MeshCornerAttribute
                && att_data_id >= 0
                && no_interior_seams)
        {
            let encoding_data: *mut MeshAttributeIndicesEncodingData;
            if self.use_single_connectivity
                || att.attribute_type() == GeometryAttributeType::Position
            {
                encoding_data = &mut self.pos_encoding_data;
            } else {
                let data = &mut self.attribute_data[att_data_id as usize];
                data.encoding_data
                    .assign_vertex_to_encoded_map(num_vertices, -1);
                data.is_connectivity_used = false;
                encoding_data = &mut data.encoding_data;
            }
            if speed == 0 && att.attribute_type() == GeometryAttributeType::Position {
                traversal_method = MeshTraversalMethod::MeshTraversalPredictionDegree;
                if self.use_single_connectivity && num_attributes > 1 {
                    traversal_method = MeshTraversalMethod::MeshTraversalDepthFirst;
                }
            }
            if traversal_method == MeshTraversalMethod::MeshTraversalPredictionDegree {
                type AttObserver = MeshAttributeIndicesEncodingObserver<CornerTable>;
                type AttTraverser = MaxPredictionDegreeTraverser<CornerTable, AttObserver>;
                let corner_table = self.corner_table() as *const CornerTable;
                Some(
                    self.create_vertex_traversal_sequencer::<AttTraverser, CornerTable>(
                        encoding_data,
                        corner_table,
                    ),
                )
            } else {
                type AttObserver = MeshAttributeIndicesEncodingObserver<CornerTable>;
                type AttTraverser = DepthFirstTraverser<CornerTable, AttObserver>;
                let corner_table = self.corner_table() as *const CornerTable;
                Some(
                    self.create_vertex_traversal_sequencer::<AttTraverser, CornerTable>(
                        encoding_data,
                        corner_table,
                    ),
                )
            }
        } else {
            if att_data_id < 0 {
                return false;
            }
            type AttObserver =
                MeshAttributeIndicesEncodingObserver<MeshAttributeCornerTable<'static>>;
            type AttTraverser = DepthFirstTraverser<MeshAttributeCornerTable<'static>, AttObserver>;
            let data = &mut self.attribute_data[att_data_id as usize];
            let encoding_data = &mut data.encoding_data as *mut MeshAttributeIndicesEncodingData;
            let corner_table = &data.connectivity_data as *const MeshAttributeCornerTable<'static>;
            data.encoding_data
                .assign_vertex_to_encoded_map(data.connectivity_data.num_vertices(), -1);
            let seq = self.create_vertex_traversal_sequencer::<
                AttTraverser,
                MeshAttributeCornerTable<'static>,
            >(encoding_data, corner_table);
            Some(seq)
        };

        let sequencer = match sequencer {
            Some(seq) => seq,
            None => return false,
        };

        if att_data_id == -1 {
            self.pos_traversal_method = traversal_method;
        } else {
            self.attribute_data[att_data_id as usize].traversal_method = traversal_method;
        }

        let controller =
            SequentialAttributeEncodersController::with_attribute_id(sequencer, att_id);
        self.attribute_encoder_to_data_id_map.push(att_data_id);
        self.encoder_mut()
            .add_attributes_encoder(Box::new(controller));
        true
    }

    fn encode_attributes_encoder_identifier(&mut self, att_encoder_id: i32) -> bool {
        let att_data_id = self.attribute_encoder_to_data_id_map[att_encoder_id as usize] as i8;
        {
            let buffer_ptr = match self.encoder_mut().buffer() {
                Some(buf) => buf as *mut draco_core::core::encoder_buffer::EncoderBuffer,
                None => return false,
            };
            let buffer = unsafe { &mut *buffer_ptr };
            buffer.encode(att_data_id);
        }

        let mut element_type =
            draco_core::mesh::mesh::MeshAttributeElementType::MeshVertexAttribute;
        let traversal_method = if att_data_id >= 0 {
            let att_id = self.attribute_data[att_data_id as usize].attribute_index;
            element_type = self.mesh().get_attribute_element_type(att_id);
            self.attribute_data[att_data_id as usize].traversal_method
        } else {
            self.pos_traversal_method
        };
        if element_type == draco_core::mesh::mesh::MeshAttributeElementType::MeshVertexAttribute
            || (element_type
                == draco_core::mesh::mesh::MeshAttributeElementType::MeshCornerAttribute
                && att_data_id >= 0
                && self.attribute_data[att_data_id as usize]
                    .connectivity_data
                    .no_interior_seams())
        {
            let buffer_ptr = match self.encoder_mut().buffer() {
                Some(buf) => buf as *mut draco_core::core::encoder_buffer::EncoderBuffer,
                None => return false,
            };
            let buffer = unsafe { &mut *buffer_ptr };
            buffer.encode(
                draco_core::mesh::mesh::MeshAttributeElementType::MeshVertexAttribute as u8,
            );
            buffer.encode(traversal_method as u8);
        } else {
            let buffer_ptr = match self.encoder_mut().buffer() {
                Some(buf) => buf as *mut draco_core::core::encoder_buffer::EncoderBuffer,
                None => return false,
            };
            let buffer = unsafe { &mut *buffer_ptr };
            buffer.encode(
                draco_core::mesh::mesh::MeshAttributeElementType::MeshCornerAttribute as u8,
            );
            buffer.encode(traversal_method as u8);
        }
        true
    }

    fn encode_connectivity(&mut self) -> Status {
        if self.use_single_connectivity {
            self.corner_table = create_corner_table_from_all_attributes(self.mesh()).map(Box::new);
        } else {
            self.corner_table =
                create_corner_table_from_position_attribute(self.mesh()).map(Box::new);
        }
        if self.corner_table.is_none()
            || self.corner_table().num_faces()
                == self.corner_table().num_degenerated_faces() as usize
        {
            return Status::new(StatusCode::DracoError, "All triangles are degenerate.");
        }

        let trait_ptr: *const dyn MeshEdgebreakerEncoderImplInterface = self;
        self.traversal_encoder.init(trait_ptr);

        let num_vertices_to_encode =
            self.corner_table().num_vertices() as i32 - self.corner_table().num_isolated_vertices();
        encode_varint(
            num_vertices_to_encode as u32,
            self.encoder_mut().buffer().unwrap(),
        );

        let num_faces =
            self.corner_table().num_faces() as i32 - self.corner_table().num_degenerated_faces();
        encode_varint(num_faces as u32, self.encoder_mut().buffer().unwrap());

        self.visited_faces = vec![false; self.mesh().num_faces() as usize];
        self.pos_encoding_data
            .assign_vertex_to_encoded_map(self.corner_table().num_vertices(), -1);
        self.pos_encoding_data
            .encoded_attribute_value_index_to_corner_map
            .clear();
        self.pos_encoding_data
            .encoded_attribute_value_index_to_corner_map
            .reserve(self.corner_table().num_faces() * 3);
        self.visited_vertex_ids = vec![false; self.corner_table().num_vertices()];
        self.vertex_traversal_length.clear();
        self.last_encoded_symbol_id = -1;
        self.num_split_symbols = 0;
        self.topology_split_event_data.clear();
        self.face_to_split_symbol_map.clear();
        self.visited_holes.clear();
        self.vertex_hole_id = vec![-1; self.corner_table().num_vertices()];
        self.processed_connectivity_corners.clear();
        self.processed_connectivity_corners
            .reserve(self.corner_table().num_faces());
        self.pos_encoding_data.num_values = 0;

        if !self.find_holes() {
            return Status::new(StatusCode::DracoError, "Failed to process mesh holes.");
        }
        if !self.init_attribute_data() {
            return Status::new(
                StatusCode::DracoError,
                "Failed to initialize attribute data.",
            );
        }
        let num_attribute_data = self.attribute_data.len() as u8;
        self.encoder_mut()
            .buffer()
            .unwrap()
            .encode(num_attribute_data);
        self.traversal_encoder
            .set_num_attribute_data(num_attribute_data as usize);

        let num_corners = self.corner_table().num_corners();
        let corner_table_ptr = self.corner_table() as *const CornerTable;
        let corner_table = unsafe { &*corner_table_ptr };
        self.traversal_encoder.start();

        let debug_traversal = std::env::var("DRACO_DEBUG_TRAVERSAL").ok().as_deref() == Some("1");
        let mut init_face_connectivity_corners: Vec<CornerIndex> = Vec::new();
        for c_id in 0..num_corners {
            let corner_index = CornerIndex::from(c_id as u32);
            let face_id = corner_table.face(corner_index);
            if self.visited_faces[face_id.value() as usize] {
                continue;
            }
            if corner_table.is_degenerated(face_id) {
                continue;
            }
            let (interior_config, start_corner) = self.find_init_face_configuration(face_id);
            if debug_traversal {
                eprintln!(
                    "[init] c_id={} face_id={} interior={} start_corner={}",
                    c_id,
                    face_id.value(),
                    interior_config,
                    start_corner.value()
                );
            }
            self.traversal_encoder
                .encode_start_face_configuration(interior_config);
            if interior_config {
                let corner_index = start_corner;
                let vert_id = corner_table.vertex(corner_index);
                let next_vert_id = corner_table.vertex(corner_table.next(corner_index));
                let prev_vert_id = corner_table.vertex(corner_table.previous(corner_index));
                self.visited_vertex_ids[vert_id.value() as usize] = true;
                self.visited_vertex_ids[next_vert_id.value() as usize] = true;
                self.visited_vertex_ids[prev_vert_id.value() as usize] = true;
                self.vertex_traversal_length.push(1);
                self.visited_faces[face_id.value() as usize] = true;
                init_face_connectivity_corners.push(corner_table.next(corner_index));
                let opp_id = corner_table.opposite(corner_table.next(corner_index));
                let opp_face_id = corner_table.face(opp_id);
                if debug_traversal {
                    eprintln!(
                        "[init] opp_id={} opp_face_id={} next(corner)={}",
                        opp_id.value(),
                        opp_face_id.value(),
                        corner_table.next(corner_index).value()
                    );
                    // Dump opposite for first 12 corners
                    for i in 0..12.min(corner_table.num_corners()) {
                        let ci = CornerIndex::from(i as u32);
                        let opp = corner_table.opposite(ci);
                        eprintln!("  opp[{}]={}", i, opp.value());
                    }
                }
                if opp_face_id != INVALID_FACE_INDEX
                    && !self.visited_faces[opp_face_id.value() as usize]
                {
                    if !self.encode_connectivity_from_corner(opp_id) {
                        return Status::new(
                            StatusCode::DracoError,
                            "Failed to encode mesh component.",
                        );
                    }
                }
            } else {
                self.encode_hole(corner_table.next(start_corner), true);
                if !self.encode_connectivity_from_corner(start_corner) {
                    return Status::new(StatusCode::DracoError, "Failed to encode mesh component.");
                }
            }
        }
        // C++: reverse traversal corners, then append init (mesh_edgebreaker_encoder_impl.cc:401-408)
        self.processed_connectivity_corners.reverse();
        self.processed_connectivity_corners
            .extend(init_face_connectivity_corners.iter());
        if !self.attribute_data.is_empty() {
            self.visited_faces = vec![false; self.mesh().num_faces() as usize];
            let corners = self.processed_connectivity_corners.clone();
            for ci in corners {
                self.encode_attribute_connectivities_on_face(ci);
            }
        }
        self.traversal_encoder.done();
        let num_encoded_symbols = self.traversal_encoder.num_encoded_symbols() as u32;
        encode_varint(num_encoded_symbols, self.encoder_mut().buffer().unwrap());
        encode_varint(self.num_split_symbols, self.encoder_mut().buffer().unwrap());
        if !self.encode_split_data() {
            return Status::new(StatusCode::DracoError, "Failed to encode split data.");
        }
        let traversal_data = self.traversal_encoder.buffer().data().to_vec();
        self.encoder_mut()
            .buffer()
            .unwrap()
            .encode_bytes(&traversal_data);
        ok_status()
    }

    fn get_corner_table(&self) -> Option<&CornerTable> {
        self.corner_table.as_ref().map(|ct| ct.as_ref())
    }

    fn is_face_encoded(&self, face_id: FaceIndex) -> bool {
        self.visited_faces[face_id.value() as usize]
    }

    fn get_encoder(&self) -> &MeshEdgebreakerEncoder {
        self.encoder()
    }

    fn get_traversal_symbols(&self) -> Option<Vec<EdgebreakerTopologyBitPattern>> {
        self.traversal_encoder
            .standard_symbols()
            .map(|s| s.to_vec())
    }
}
