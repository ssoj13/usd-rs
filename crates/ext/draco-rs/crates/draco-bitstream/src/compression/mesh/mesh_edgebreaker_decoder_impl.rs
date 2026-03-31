//! Edgebreaker decoder implementation.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_decoder_impl.h|cc`.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// When DRACO_PARITY_DUMP_SYMBOLS_DECODE=1, symbols decoded during connectivity
    /// decode are collected here for parity comparison.
    static DECODED_TRAVERSAL_SYMBOLS: RefCell<Option<Vec<u32>>> = RefCell::new(None);
}

/// Takes decoded traversal symbols collected during the last EdgeBreaker decode
/// (when DRACO_PARITY_DUMP_SYMBOLS_DECODE=1). Returns None if no decode ran or
/// symbols were not captured.
pub fn take_decoded_traversal_symbols_for_parity() -> Option<Vec<u32>> {
    DECODED_TRAVERSAL_SYMBOLS.with(|cell| cell.borrow_mut().take())
}

use draco_core::attributes::geometry_indices::{
    CornerIndex, FaceIndex, PointIndex, VertexIndex, INVALID_CORNER_INDEX,
};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::varint_decoding::decode_varint;
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::corner_table_iterators::VertexCornersIterator;
use draco_core::mesh::mesh::Mesh;
use draco_core::mesh::mesh::MeshAttributeElementType;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;
use draco_core::point_cloud::point_cloud::PointCloud;

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::attributes::points_sequencer::PointsSequencer;
use crate::compression::attributes::sequential_attribute_decoders_controller::SequentialAttributeDecodersController;
use crate::compression::config::compression_shared::{bitstream_version, MeshTraversalMethod};
use crate::compression::mesh::edgebreaker_shared::{
    EdgeFaceName, EdgebreakerTopologyBitPattern, HoleEventData, TopologySplitEventData,
};
use crate::compression::mesh::mesh_edgebreaker_decoder::MeshEdgebreakerDecoder;
use crate::compression::mesh::mesh_edgebreaker_decoder_impl_interface::MeshEdgebreakerDecoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_decoder::EdgebreakerTraversalDecoder;
use crate::compression::mesh::traverser::{
    DepthFirstTraverser, MaxPredictionDegreeTraverser, MeshAttributeIndicesEncodingObserver,
    MeshTraversalSequencer,
};
use crate::compression::mesh::MeshDecoder;
use crate::compression::point_cloud::PointCloudDecoder;

struct AttributeData {
    decoder_id: i32,
    connectivity_data: MeshAttributeCornerTable<'static>,
    is_connectivity_used: bool,
    encoding_data: MeshAttributeIndicesEncodingData,
    attribute_seam_corners: Vec<i32>,
}

impl AttributeData {
    fn new() -> Self {
        Self {
            decoder_id: -1,
            connectivity_data: MeshAttributeCornerTable::new(),
            is_connectivity_used: true,
            encoding_data: MeshAttributeIndicesEncodingData::new(),
            attribute_seam_corners: Vec::new(),
        }
    }
}

pub struct MeshEdgebreakerDecoderImpl<TraversalDecoderT>
where
    TraversalDecoderT: EdgebreakerTraversalDecoder + Default + 'static,
{
    decoder: *mut MeshEdgebreakerDecoder,
    corner_table: Option<Box<CornerTable>>,
    // Parity: C++ keeps a traversal stack for alternative decode paths.
    #[allow(dead_code)]
    corner_traversal_stack: Vec<CornerIndex>,
    vertex_traversal_length: Vec<i32>,
    topology_split_data: Vec<TopologySplitEventData>,
    hole_event_data: Vec<HoleEventData>,
    init_face_configurations: Vec<bool>,
    init_corners: Vec<CornerIndex>,
    last_symbol_id: i32,
    last_vert_id: i32,
    last_face_id: i32,
    // Parity: visited arrays exist in the reference for optional face/vertex tracking.
    #[allow(dead_code)]
    visited_faces: Vec<bool>,
    // Parity: visited arrays exist in the reference for optional face/vertex tracking.
    #[allow(dead_code)]
    visited_verts: Vec<bool>,
    is_vert_hole: Vec<bool>,
    num_new_vertices: i32,
    new_to_parent_vertex_map: HashMap<i32, i32>,
    num_encoded_vertices: i32,
    processed_corner_ids: Vec<i32>,
    /// Parity: C++ has this but never populates or uses it; decoder uses default corner order.
    processed_connectivity_corners: Vec<CornerIndex>,
    pos_encoding_data: MeshAttributeIndicesEncodingData,
    pos_data_decoder_id: i32,
    attribute_data: Vec<AttributeData>,
    traversal_decoder: TraversalDecoderT,
}

impl<TraversalDecoderT> MeshEdgebreakerDecoderImpl<TraversalDecoderT>
where
    TraversalDecoderT: EdgebreakerTraversalDecoder + Default + 'static,
{
    pub fn new() -> Self {
        Self {
            decoder: std::ptr::null_mut(),
            corner_table: None,
            corner_traversal_stack: Vec::new(),
            vertex_traversal_length: Vec::new(),
            topology_split_data: Vec::new(),
            hole_event_data: Vec::new(),
            init_face_configurations: Vec::new(),
            init_corners: Vec::new(),
            last_symbol_id: -1,
            last_vert_id: -1,
            last_face_id: -1,
            visited_faces: Vec::new(),
            visited_verts: Vec::new(),
            is_vert_hole: Vec::new(),
            num_new_vertices: 0,
            new_to_parent_vertex_map: HashMap::new(),
            num_encoded_vertices: 0,
            processed_corner_ids: Vec::new(),
            processed_connectivity_corners: Vec::new(),
            pos_encoding_data: MeshAttributeIndicesEncodingData::new(),
            pos_data_decoder_id: -1,
            attribute_data: Vec::new(),
            traversal_decoder: TraversalDecoderT::default(),
        }
    }

    fn decoder(&self) -> &MeshEdgebreakerDecoder {
        unsafe { &*self.decoder }
    }

    fn decoder_mut(&mut self) -> &mut MeshEdgebreakerDecoder {
        unsafe { &mut *self.decoder }
    }

    fn corner_table_ptr(&mut self) -> *mut CornerTable {
        match self.corner_table.as_mut() {
            Some(ct) => ct.as_mut() as *mut CornerTable,
            None => std::ptr::null_mut(),
        }
    }

    fn corner_table_ref(&self) -> Option<&CornerTable> {
        self.corner_table.as_deref()
    }

    fn corner_table_static(&self) -> &'static CornerTable {
        unsafe { std::mem::transmute(self.corner_table.as_ref().unwrap().as_ref()) }
    }

    fn create_vertex_traversal_sequencer<TraverserT, CornerTableT>(
        &self,
        encoding_data: *mut MeshAttributeIndicesEncodingData,
        corner_table: *const CornerTableT,
    ) -> Option<Box<dyn PointsSequencer>>
    where
        TraverserT: crate::compression::mesh::traverser::MeshTraverser<
                CornerTable = CornerTableT,
                Observer = MeshAttributeIndicesEncodingObserver<CornerTableT>,
            > + Default
            + 'static,
        CornerTableT: crate::compression::mesh::traverser::TraversalCornerTable + 'static,
    {
        let mesh = self.decoder().mesh()?;
        let mut sequencer = MeshTraversalSequencer::<TraverserT, CornerTableT>::new(
            mesh as *const Mesh,
            encoding_data,
            corner_table,
        );
        sequencer.set_traverser(TraverserT::default());
        // C++ decoder never calls SetCornerOrder; uses default [3*0, 3*1, ...] (mesh_traversal_sequencer.h:89)
        Some(Box::new(sequencer))
    }

    fn is_topology_split(&mut self, encoder_symbol_id: i32) -> Option<(EdgeFaceName, i32)> {
        let last = self.topology_split_data.last()?;
        if last.source_symbol_id as i32 > encoder_symbol_id {
            return Some((EdgeFaceName::LeftFaceEdge, -1));
        }
        if last.source_symbol_id as i32 != encoder_symbol_id {
            return None;
        }
        let edge = if last.source_edge == 0 {
            EdgeFaceName::LeftFaceEdge
        } else {
            EdgeFaceName::RightFaceEdge
        };
        let split_id = last.split_symbol_id as i32;
        self.topology_split_data.pop();
        Some((edge, split_id))
    }

    fn decode_hole_and_topology_split_events(&mut self, decoder_buffer: &mut DecoderBuffer) -> i32 {
        let mut num_topology_splits: u32 = 0;
        if self.decoder().bitstream_version() < bitstream_version(2, 0) {
            if !decoder_buffer.decode(&mut num_topology_splits) {
                return -1;
            }
        } else if !decode_varint(&mut num_topology_splits, decoder_buffer) {
            return -1;
        }
        if num_topology_splits > 0 {
            if let Some(corner_table) = self.corner_table_ref() {
                if num_topology_splits > corner_table.num_faces() as u32 {
                    return -1;
                }
            }
            if self.decoder().bitstream_version() < bitstream_version(1, 2) {
                for _ in 0..num_topology_splits {
                    let mut event = TopologySplitEventData::default();
                    if !decoder_buffer.decode(&mut event.split_symbol_id) {
                        return -1;
                    }
                    if !decoder_buffer.decode(&mut event.source_symbol_id) {
                        return -1;
                    }
                    let mut edge_data: u8 = 0;
                    if !decoder_buffer.decode(&mut edge_data) {
                        return -1;
                    }
                    event.source_edge = (edge_data & 1) as u32;
                    self.topology_split_data.push(event);
                }
            } else {
                let mut last_source_symbol_id = 0u32;
                for _ in 0..num_topology_splits {
                    let mut event = TopologySplitEventData::default();
                    let mut delta: u32 = 0;
                    if !decode_varint(&mut delta, decoder_buffer) {
                        return -1;
                    }
                    event.source_symbol_id = delta + last_source_symbol_id;
                    if !decode_varint(&mut delta, decoder_buffer) {
                        return -1;
                    }
                    if delta > event.source_symbol_id {
                        return -1;
                    }
                    event.split_symbol_id = event.source_symbol_id - delta;
                    last_source_symbol_id = event.source_symbol_id;
                    self.topology_split_data.push(event);
                }
                let mut dummy: u64 = 0;
                if !decoder_buffer.start_bit_decoding(false, &mut dummy) {
                    return -1;
                }
                for i in 0..num_topology_splits as usize {
                    let mut edge_data: u32 = 0;
                    if self.decoder().bitstream_version() < bitstream_version(2, 2) {
                        if !decoder_buffer.decode_least_significant_bits32(2, &mut edge_data) {
                            return -1;
                        }
                    } else if !decoder_buffer.decode_least_significant_bits32(1, &mut edge_data) {
                        return -1;
                    }
                    if let Some(event) = self.topology_split_data.get_mut(i) {
                        event.source_edge = edge_data & 1;
                    }
                }
                decoder_buffer.end_bit_decoding();
            }
        }

        let mut num_hole_events: u32 = 0;
        if self.decoder().bitstream_version() < bitstream_version(2, 0) {
            if !decoder_buffer.decode(&mut num_hole_events) {
                return -1;
            }
        } else if self.decoder().bitstream_version() < bitstream_version(2, 1) {
            if !decode_varint(&mut num_hole_events, decoder_buffer) {
                return -1;
            }
        }

        if num_hole_events > 0 {
            if self.decoder().bitstream_version() < bitstream_version(1, 2) {
                for _ in 0..num_hole_events {
                    let mut event = HoleEventData::default();
                    if !decoder_buffer.decode(&mut event) {
                        return -1;
                    }
                    self.hole_event_data.push(event);
                }
            } else {
                let mut last_symbol_id: i32 = 0;
                for _ in 0..num_hole_events {
                    let mut delta: u32 = 0;
                    if !decode_varint(&mut delta, decoder_buffer) {
                        return -1;
                    }
                    let symbol_id = delta as i32 + last_symbol_id;
                    last_symbol_id = symbol_id;
                    self.hole_event_data.push(HoleEventData::new(symbol_id));
                }
            }
        }

        decoder_buffer.decoded_size() as i32
    }

    fn decode_attribute_connectivities_on_face_legacy(&mut self, corner: CornerIndex) -> bool {
        let corners = {
            let corner_table = match self.corner_table_ref() {
                Some(ct) => ct,
                None => return false,
            };
            [
                corner,
                corner_table.next(corner),
                corner_table.previous(corner),
            ]
        };
        for c in corners {
            let opp_corner = {
                let corner_table = match self.corner_table_ref() {
                    Some(ct) => ct,
                    None => return false,
                };
                corner_table.opposite(c)
            };
            if opp_corner == INVALID_CORNER_INDEX {
                for data in &mut self.attribute_data {
                    data.attribute_seam_corners.push(c.value() as i32);
                }
                continue;
            }
            let mut seams = vec![false; self.attribute_data.len()];
            for i in 0..seams.len() {
                seams[i] = self.traversal_decoder.decode_attribute_seam(i);
            }
            for (i, is_seam) in seams.iter().enumerate() {
                if *is_seam {
                    self.attribute_data[i]
                        .attribute_seam_corners
                        .push(c.value() as i32);
                }
            }
        }
        true
    }

    fn decode_attribute_connectivities_on_face(&mut self, corner: CornerIndex) -> bool {
        let corners = {
            let corner_table = match self.corner_table_ref() {
                Some(ct) => ct,
                None => return false,
            };
            [
                corner,
                corner_table.next(corner),
                corner_table.previous(corner),
            ]
        };
        let src_face_id = {
            let corner_table = match self.corner_table_ref() {
                Some(ct) => ct,
                None => return false,
            };
            corner_table.face(corner)
        };
        for c in corners {
            let opp_corner = {
                let corner_table = match self.corner_table_ref() {
                    Some(ct) => ct,
                    None => return false,
                };
                corner_table.opposite(c)
            };
            if opp_corner == INVALID_CORNER_INDEX {
                for data in &mut self.attribute_data {
                    data.attribute_seam_corners.push(c.value() as i32);
                }
                continue;
            }
            let opp_face_id = {
                let corner_table = match self.corner_table_ref() {
                    Some(ct) => ct,
                    None => return false,
                };
                corner_table.face(opp_corner)
            };
            if opp_face_id < src_face_id {
                continue;
            }
            let mut seams = vec![false; self.attribute_data.len()];
            for i in 0..seams.len() {
                seams[i] = self.traversal_decoder.decode_attribute_seam(i);
            }
            for (i, is_seam) in seams.iter().enumerate() {
                if *is_seam {
                    self.attribute_data[i]
                        .attribute_seam_corners
                        .push(c.value() as i32);
                }
            }
        }
        true
    }

    fn assign_points_to_corners(&mut self, num_connectivity_verts: i32) -> bool {
        let mesh_ptr = {
            let decoder = self.decoder_mut();
            match decoder.mesh_mut() {
                Some(mesh) => mesh as *mut Mesh,
                None => return false,
            }
        };
        if mesh_ptr.is_null() {
            return false;
        }
        let mesh = unsafe { &mut *mesh_ptr };
        let corner_table_ptr = self.corner_table_ptr();
        if corner_table_ptr.is_null() {
            return false;
        }
        let corner_table = unsafe { &*corner_table_ptr };

        mesh.set_num_faces(corner_table.num_faces());

        if self.attribute_data.is_empty() {
            for f in 0..mesh.num_faces() {
                let face_id = FaceIndex::from(f);
                let start_corner = CornerIndex::from(3 * face_id.value());
                let mut face = [PointIndex::from(0); 3];
                for c in 0..3 {
                    let vert_id = corner_table.vertex(start_corner + c as u32).value();
                    face[c] = PointIndex::from(vert_id);
                }
                mesh.set_face(face_id, face);
            }
            if num_connectivity_verts < 0 {
                return false;
            }
            let pc_ptr: *mut PointCloud = {
                let decoder = self.decoder_mut();
                match decoder.point_cloud_mut() {
                    Some(pc) => pc as *mut PointCloud,
                    None => return false,
                }
            };
            if pc_ptr.is_null() {
                return false;
            }
            unsafe { &mut *pc_ptr }.set_num_points(num_connectivity_verts as u32);
            return true;
        }

        let mut point_to_corner_map: Vec<i32> = Vec::new();
        let mut corner_to_point_map: Vec<u32> = vec![0; corner_table.num_corners()];

        for v in 0..corner_table.num_vertices() {
            let vert = VertexIndex::from(v as u32);
            let mut c = corner_table.left_most_corner(vert);
            if c == INVALID_CORNER_INDEX {
                continue;
            }
            let mut dedup_first_corner = c;
            let is_hole = self.is_vert_hole.get(v).copied().unwrap_or(false);
            if !is_hole {
                for data in &self.attribute_data {
                    if !data.connectivity_data.is_corner_on_seam(c) {
                        continue;
                    }
                    let vert_id = data.connectivity_data.vertex(c);
                    let mut act_c = corner_table.swing_right(c);
                    let mut seam_found = false;
                    while act_c != c {
                        if act_c == INVALID_CORNER_INDEX {
                            return false;
                        }
                        if data.connectivity_data.vertex(act_c) != vert_id {
                            dedup_first_corner = act_c;
                            seam_found = true;
                            break;
                        }
                        act_c = corner_table.swing_right(act_c);
                    }
                    if seam_found {
                        break;
                    }
                }
            }

            c = dedup_first_corner;
            corner_to_point_map[c.value() as usize] = point_to_corner_map.len() as u32;
            point_to_corner_map.push(c.value() as i32);
            let mut prev_c = c;
            c = corner_table.swing_right(c);
            while c != INVALID_CORNER_INDEX && c != dedup_first_corner {
                let mut attribute_seam = false;
                for data in &self.attribute_data {
                    if data.connectivity_data.vertex(c) != data.connectivity_data.vertex(prev_c) {
                        attribute_seam = true;
                        break;
                    }
                }
                if attribute_seam {
                    corner_to_point_map[c.value() as usize] = point_to_corner_map.len() as u32;
                    point_to_corner_map.push(c.value() as i32);
                } else {
                    corner_to_point_map[c.value() as usize] =
                        corner_to_point_map[prev_c.value() as usize];
                }
                prev_c = c;
                c = corner_table.swing_right(c);
            }
        }

        for f in 0..mesh.num_faces() {
            let face_id = FaceIndex::from(f);
            let mut face = [PointIndex::from(0); 3];
            for c in 0..3 {
                let idx = (3 * face_id.value() + c as u32) as usize;
                face[c] = PointIndex::from(corner_to_point_map[idx]);
            }
            mesh.set_face(face_id, face);
        }

        let pc_ptr: *mut PointCloud = {
            let decoder = self.decoder_mut();
            match decoder.point_cloud_mut() {
                Some(pc) => pc as *mut PointCloud,
                None => return false,
            }
        };
        if pc_ptr.is_null() {
            return false;
        }
        unsafe { &mut *pc_ptr }.set_num_points(point_to_corner_map.len() as u32);
        true
    }

    fn decode_connectivity_internal(&mut self, num_symbols: i32) -> i32 {
        if std::env::var("DRACO_PARITY_DUMP_SYMBOLS_DECODE")
            .ok()
            .as_deref()
            == Some("1")
        {
            DECODED_TRAVERSAL_SYMBOLS.with(|cell| *cell.borrow_mut() = Some(Vec::new()));
        }
        let mut active_corner_stack: Vec<CornerIndex> = Vec::new();
        let mut topology_split_active_corners: HashMap<i32, CornerIndex> = HashMap::new();
        let mut invalid_vertices: Vec<VertexIndex> = Vec::new();
        let remove_invalid_vertices = self.attribute_data.is_empty();

        let max_num_vertices = self.is_vert_hole.len() as i32;
        let mut num_faces: i32 = 0;
        let corner_table_ptr = self.corner_table_ptr();
        if corner_table_ptr.is_null() {
            return -1;
        }

        for symbol_id in 0..num_symbols {
            let face = FaceIndex::from(num_faces as u32);
            num_faces += 1;
            let mut check_topology_split = false;
            let symbol = self.traversal_decoder.decode_symbol();
            DECODED_TRAVERSAL_SYMBOLS.with(|cell| {
                if let Some(ref mut v) = *cell.borrow_mut() {
                    v.push(symbol);
                }
            });
            if symbol == EdgebreakerTopologyBitPattern::TopologyC as u32 {
                if active_corner_stack.is_empty() {
                    return -1;
                }
                let corner_a = *active_corner_stack.last().unwrap();
                let (vertex_x, corner_b) = unsafe {
                    let ct = &*corner_table_ptr;
                    let vertex_x = ct.vertex(ct.next(corner_a));
                    let corner_b = ct.next(ct.left_most_corner(vertex_x));
                    (vertex_x, corner_b)
                };
                if corner_a == corner_b {
                    return -1;
                }
                unsafe {
                    let ct = &mut *corner_table_ptr;
                    if ct.opposite(corner_a) != INVALID_CORNER_INDEX
                        || ct.opposite(corner_b) != INVALID_CORNER_INDEX
                    {
                        return -1;
                    }
                    let corner = CornerIndex::from(3 * face.value());
                    ct.set_opposite_corners(corner_a, corner + 1);
                    ct.set_opposite_corners(corner_b, corner + 2);
                    let vert_a_prev = ct.vertex(ct.previous(corner_a));
                    let vert_b_next = ct.vertex(ct.next(corner_b));
                    if vertex_x == vert_a_prev || vertex_x == vert_b_next {
                        return -1;
                    }
                    ct.map_corner_to_vertex(corner, vertex_x);
                    ct.map_corner_to_vertex(corner + 1, vert_b_next);
                    ct.map_corner_to_vertex(corner + 2, vert_a_prev);
                    ct.set_left_most_corner(vert_a_prev, corner + 2);
                }
                if (vertex_x.value() as usize) < self.is_vert_hole.len() {
                    self.is_vert_hole[vertex_x.value() as usize] = false;
                }
                active_corner_stack.pop();
                active_corner_stack.push(CornerIndex::from(3 * face.value()));
            } else if symbol == EdgebreakerTopologyBitPattern::TopologyR as u32
                || symbol == EdgebreakerTopologyBitPattern::TopologyL as u32
            {
                if active_corner_stack.is_empty() {
                    return -1;
                }
                let corner_a = *active_corner_stack.last().unwrap();
                unsafe {
                    let ct = &mut *corner_table_ptr;
                    if ct.opposite(corner_a) != INVALID_CORNER_INDEX {
                        return -1;
                    }
                    let corner = CornerIndex::from(3 * face.value());
                    let (opp_corner, corner_l, corner_r) =
                        if symbol == EdgebreakerTopologyBitPattern::TopologyR as u32 {
                            (corner + 2, corner + 1, corner)
                        } else {
                            (corner + 1, corner, corner + 2)
                        };
                    ct.set_opposite_corners(opp_corner, corner_a);
                    let new_vert_index = ct.add_new_vertex();
                    if ct.num_vertices() as i32 > max_num_vertices {
                        return -1;
                    }
                    ct.map_corner_to_vertex(opp_corner, new_vert_index);
                    ct.set_left_most_corner(new_vert_index, opp_corner);
                    let vertex_r = ct.vertex(ct.previous(corner_a));
                    ct.map_corner_to_vertex(corner_r, vertex_r);
                    ct.set_left_most_corner(vertex_r, corner_r);
                    ct.map_corner_to_vertex(corner_l, ct.vertex(ct.next(corner_a)));
                    active_corner_stack.pop();
                    active_corner_stack.push(corner);
                }
                check_topology_split = true;
            } else if symbol == EdgebreakerTopologyBitPattern::TopologyS as u32 {
                if active_corner_stack.is_empty() {
                    return -1;
                }
                let corner_b = active_corner_stack.pop().unwrap();
                if let Some(split_corner) = topology_split_active_corners.get(&symbol_id).copied() {
                    active_corner_stack.push(split_corner);
                }
                if active_corner_stack.is_empty() {
                    return -1;
                }
                let corner_a = *active_corner_stack.last().unwrap();
                if corner_a == corner_b {
                    return -1;
                }
                unsafe {
                    let ct = &mut *corner_table_ptr;
                    if ct.opposite(corner_a) != INVALID_CORNER_INDEX
                        || ct.opposite(corner_b) != INVALID_CORNER_INDEX
                    {
                        return -1;
                    }
                    let corner = CornerIndex::from(3 * face.value());
                    ct.set_opposite_corners(corner_a, corner + 2);
                    ct.set_opposite_corners(corner_b, corner + 1);
                    let vertex_p = ct.vertex(ct.previous(corner_a));
                    ct.map_corner_to_vertex(corner, vertex_p);
                    let vertex_a_next = ct.vertex(ct.next(corner_a));
                    ct.map_corner_to_vertex(corner + 1, vertex_a_next);
                    let vert_b_prev = ct.vertex(ct.previous(corner_b));
                    ct.map_corner_to_vertex(corner + 2, vert_b_prev);
                    ct.set_left_most_corner(vert_b_prev, corner + 2);
                    let mut corner_n = ct.next(corner_b);
                    let vertex_n = ct.vertex(corner_n);
                    self.traversal_decoder.merge_vertices(vertex_p, vertex_n);
                    let left_most_n = ct.left_most_corner(vertex_n);
                    ct.set_left_most_corner(vertex_p, left_most_n);
                    let first_corner = corner_n;
                    while corner_n != INVALID_CORNER_INDEX {
                        ct.map_corner_to_vertex(corner_n, vertex_p);
                        corner_n = ct.swing_left(corner_n);
                        if corner_n == first_corner {
                            return -1;
                        }
                    }
                    ct.make_vertex_isolated(vertex_n);
                    if remove_invalid_vertices {
                        invalid_vertices.push(vertex_n);
                    }
                    active_corner_stack.pop();
                    active_corner_stack.push(corner);
                }
            } else if symbol == EdgebreakerTopologyBitPattern::TopologyE as u32 {
                unsafe {
                    let ct = &mut *corner_table_ptr;
                    let corner = CornerIndex::from(3 * face.value());
                    let first_vert_index = ct.add_new_vertex();
                    ct.map_corner_to_vertex(corner, first_vert_index);
                    let second_vert_index = ct.add_new_vertex();
                    ct.map_corner_to_vertex(corner + 1, second_vert_index);
                    let third_vert_index = ct.add_new_vertex();
                    ct.map_corner_to_vertex(corner + 2, third_vert_index);
                    if ct.num_vertices() as i32 > max_num_vertices {
                        return -1;
                    }
                    ct.set_left_most_corner(first_vert_index, corner);
                    ct.set_left_most_corner(first_vert_index + 1, corner + 1);
                    ct.set_left_most_corner(first_vert_index + 2, corner + 2);
                    active_corner_stack.push(corner);
                }
                check_topology_split = true;
            } else {
                return -1;
            }

            if let Some(active_corner) = active_corner_stack.last().copied() {
                self.processed_connectivity_corners.push(active_corner);
                self.traversal_decoder
                    .new_active_corner_reached(active_corner);
            } else {
                return -1;
            }

            if check_topology_split {
                let encoder_symbol_id = num_symbols - symbol_id - 1;
                while let Some((split_edge, encoder_split_symbol_id)) =
                    self.is_topology_split(encoder_symbol_id)
                {
                    if encoder_split_symbol_id < 0 {
                        return -1;
                    }
                    let act_top_corner = *active_corner_stack.last().unwrap();
                    let new_active_corner = unsafe {
                        let ct = &*corner_table_ptr;
                        if split_edge == EdgeFaceName::RightFaceEdge {
                            ct.next(act_top_corner)
                        } else {
                            ct.previous(act_top_corner)
                        }
                    };
                    let decoder_split_symbol_id = num_symbols - encoder_split_symbol_id - 1;
                    topology_split_active_corners
                        .insert(decoder_split_symbol_id, new_active_corner);
                }
            }
        }

        unsafe {
            let ct = &*corner_table_ptr;
            if ct.num_vertices() as i32 > max_num_vertices {
                return -1;
            }
        }

        while let Some(corner) = active_corner_stack.pop() {
            let interior_face = self.traversal_decoder.decode_start_face_configuration();
            if interior_face {
                let ct = unsafe { &mut *corner_table_ptr };
                if num_faces as usize >= ct.num_faces() {
                    return -1;
                }
                let corner_a = corner;
                let vert_n = ct.vertex(ct.next(corner_a));
                let corner_b = ct.next(ct.left_most_corner(vert_n));
                let vert_x = ct.vertex(ct.next(corner_b));
                let corner_c = ct.next(ct.left_most_corner(vert_x));

                if corner == corner_b || corner == corner_c || corner_b == corner_c {
                    return -1;
                }
                if ct.opposite(corner) != INVALID_CORNER_INDEX
                    || ct.opposite(corner_b) != INVALID_CORNER_INDEX
                    || ct.opposite(corner_c) != INVALID_CORNER_INDEX
                {
                    return -1;
                }

                let vert_p = ct.vertex(ct.next(corner_c));
                let face = FaceIndex::from(num_faces as u32);
                num_faces += 1;
                let new_corner = CornerIndex::from(3 * face.value());
                ct.set_opposite_corners(new_corner, corner);
                ct.set_opposite_corners(new_corner + 1, corner_b);
                ct.set_opposite_corners(new_corner + 2, corner_c);

                ct.map_corner_to_vertex(new_corner, vert_x);
                ct.map_corner_to_vertex(new_corner + 1, vert_p);
                ct.map_corner_to_vertex(new_corner + 2, vert_n);

                for ci in 0..3 {
                    let v = ct.vertex(new_corner + ci as u32).value() as usize;
                    if v < self.is_vert_hole.len() {
                        self.is_vert_hole[v] = false;
                    }
                }

                self.init_face_configurations.push(true);
                self.init_corners.push(new_corner);
            } else {
                self.init_face_configurations.push(false);
                self.init_corners.push(corner);
            }
        }

        for (i, &c) in self.init_corners.iter().enumerate() {
            if self.init_face_configurations.get(i) == Some(&true) {
                self.processed_connectivity_corners.push(c);
            }
        }

        let expected_faces = unsafe { &*corner_table_ptr }.num_faces();
        if num_faces as usize != expected_faces {
            return -1;
        }

        let mut num_vertices = unsafe { &*corner_table_ptr }.num_vertices() as i32;
        let ct_ptr = corner_table_ptr;
        for invalid_vert in invalid_vertices {
            let mut src_vert = VertexIndex::from((num_vertices - 1) as u32);
            while unsafe { &*ct_ptr }.left_most_corner(src_vert) == INVALID_CORNER_INDEX {
                num_vertices -= 1;
                if num_vertices <= 0 {
                    return -1;
                }
                src_vert = VertexIndex::from((num_vertices - 1) as u32);
            }
            if src_vert < invalid_vert {
                continue;
            }

            let mut corner_list = Vec::new();
            {
                let ct_ref = unsafe { &*ct_ptr };
                let mut vcit = VertexCornersIterator::from_vertex(ct_ref, src_vert);
                while !vcit.end() {
                    corner_list.push(vcit.corner());
                    vcit.next();
                }
            }

            let ct_mut = unsafe { &mut *ct_ptr };
            for cid in corner_list {
                if ct_mut.vertex(cid) != src_vert {
                    return -1;
                }
                ct_mut.map_corner_to_vertex(cid, invalid_vert);
            }
            let left_corner = ct_mut.left_most_corner(src_vert);
            ct_mut.set_left_most_corner(invalid_vert, left_corner);
            ct_mut.make_vertex_isolated(src_vert);
            let invalid_idx = invalid_vert.value() as usize;
            let src_idx = src_vert.value() as usize;
            if invalid_idx < self.is_vert_hole.len() && src_idx < self.is_vert_hole.len() {
                self.is_vert_hole[invalid_idx] = self.is_vert_hole[src_idx];
                self.is_vert_hole[src_idx] = false;
            }
            num_vertices -= 1;
        }
        num_vertices
    }
}

impl<TraversalDecoderT> Default for MeshEdgebreakerDecoderImpl<TraversalDecoderT>
where
    TraversalDecoderT: EdgebreakerTraversalDecoder + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<TraversalDecoderT> MeshEdgebreakerDecoderImplInterface
    for MeshEdgebreakerDecoderImpl<TraversalDecoderT>
where
    TraversalDecoderT: EdgebreakerTraversalDecoder + Default,
{
    fn init(&mut self, decoder: *mut MeshEdgebreakerDecoder) -> bool {
        self.decoder = decoder;
        true
    }

    fn get_attribute_corner_table(&self, att_id: i32) -> Option<&MeshAttributeCornerTable<'_>> {
        let decoder = self.decoder();
        for data in &self.attribute_data {
            let decoder_id = data.decoder_id;
            if decoder_id < 0 || decoder_id >= decoder.num_attributes_decoders() {
                continue;
            }
            let dec = match decoder.attributes_decoder(decoder_id) {
                Some(dec) => dec,
                None => continue,
            };
            for j in 0..dec.get_num_attributes() {
                if dec.get_attribute_id(j) == att_id {
                    if data.is_connectivity_used {
                        return Some(&data.connectivity_data);
                    }
                    return None;
                }
            }
        }
        None
    }

    fn get_attribute_encoding_data(&self, att_id: i32) -> &MeshAttributeIndicesEncodingData {
        let decoder = self.decoder();
        for data in &self.attribute_data {
            let decoder_id = data.decoder_id;
            if decoder_id < 0 || decoder_id >= decoder.num_attributes_decoders() {
                continue;
            }
            let dec = match decoder.attributes_decoder(decoder_id) {
                Some(dec) => dec,
                None => continue,
            };
            for j in 0..dec.get_num_attributes() {
                if dec.get_attribute_id(j) == att_id {
                    return &data.encoding_data;
                }
            }
        }
        &self.pos_encoding_data
    }

    fn create_attributes_decoder(&mut self, att_decoder_id: i32) -> bool {
        let mut att_data_id: i8 = 0;
        let mut decoder_type: u8 = 0;
        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if !buffer.decode(&mut att_data_id) {
                return false;
            }
            if !buffer.decode(&mut decoder_type) {
                return false;
            }
        }

        if att_data_id >= 0 {
            let data_id = att_data_id as usize;
            if data_id >= self.attribute_data.len() {
                return false;
            }
            if self.attribute_data[data_id].decoder_id >= 0 {
                return false;
            }
            self.attribute_data[data_id].decoder_id = att_decoder_id;
        } else {
            if self.pos_data_decoder_id >= 0 {
                return false;
            }
            self.pos_data_decoder_id = att_decoder_id;
        }

        let mut traversal_method = MeshTraversalMethod::MeshTraversalDepthFirst;
        if self.decoder().bitstream_version() >= bitstream_version(1, 2) {
            let mut traversal_method_encoded: u8 = 0;
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if !buffer.decode(&mut traversal_method_encoded) {
                return false;
            }
            if traversal_method_encoded >= MeshTraversalMethod::NumTraversalMethods as u8 {
                return false;
            }
            traversal_method = if traversal_method_encoded
                == MeshTraversalMethod::MeshTraversalPredictionDegree as u8
            {
                MeshTraversalMethod::MeshTraversalPredictionDegree
            } else {
                MeshTraversalMethod::MeshTraversalDepthFirst
            };
        }

        let mesh = match self.decoder().mesh() {
            Some(mesh) => mesh,
            None => return false,
        };
        let mesh_ptr = mesh as *const Mesh;

        let sequencer: Option<Box<dyn PointsSequencer>> =
            if decoder_type == MeshAttributeElementType::MeshVertexAttribute as u8 {
                let encoding_data_ptr: *mut MeshAttributeIndicesEncodingData;
                if att_data_id < 0 {
                    encoding_data_ptr = &mut self.pos_encoding_data;
                } else {
                    let data_id = att_data_id as usize;
                    encoding_data_ptr = &mut self.attribute_data[data_id].encoding_data;
                    self.attribute_data[data_id].is_connectivity_used = false;
                }

                let corner_table_ptr = match self.corner_table_ref() {
                    Some(ct) => ct as *const CornerTable,
                    None => return false,
                };

                if traversal_method == MeshTraversalMethod::MeshTraversalPredictionDegree {
                    type AttObserver = MeshAttributeIndicesEncodingObserver<CornerTable>;
                    type AttTraverser = MaxPredictionDegreeTraverser<CornerTable, AttObserver>;
                    self.create_vertex_traversal_sequencer::<AttTraverser, CornerTable>(
                        encoding_data_ptr,
                        corner_table_ptr,
                    )
                } else if traversal_method == MeshTraversalMethod::MeshTraversalDepthFirst {
                    type AttObserver = MeshAttributeIndicesEncodingObserver<CornerTable>;
                    type AttTraverser = DepthFirstTraverser<CornerTable, AttObserver>;
                    self.create_vertex_traversal_sequencer::<AttTraverser, CornerTable>(
                        encoding_data_ptr,
                        corner_table_ptr,
                    )
                } else {
                    return false;
                }
            } else {
                if traversal_method != MeshTraversalMethod::MeshTraversalDepthFirst {
                    return false;
                }
                if att_data_id < 0 {
                    return false;
                }
                let data_id = att_data_id as usize;
                type AttObserver =
                    MeshAttributeIndicesEncodingObserver<MeshAttributeCornerTable<'static>>;
                type AttTraverser =
                    DepthFirstTraverser<MeshAttributeCornerTable<'static>, AttObserver>;

                let encoding_data_ptr = &mut self.attribute_data[data_id].encoding_data as *mut _;
                let corner_table_ptr = &self.attribute_data[data_id].connectivity_data as *const _;
                let mut traversal_sequencer = MeshTraversalSequencer::<
                AttTraverser,
                <AttTraverser as crate::compression::mesh::traverser::MeshTraverser>::CornerTable,
            >::new(mesh_ptr, encoding_data_ptr, corner_table_ptr);
                traversal_sequencer.set_traverser(AttTraverser::default());
                // C++ per-corner path also never calls SetCornerOrder (mesh_edgebreaker_decoder_impl.cc:222-231)
                Some(Box::new(traversal_sequencer))
            };

        let sequencer = match sequencer {
            Some(seq) => seq,
            None => return false,
        };
        let controller = SequentialAttributeDecodersController::new(sequencer);
        self.decoder_mut()
            .set_attributes_decoder(att_decoder_id, Box::new(controller))
    }

    fn decode_connectivity(&mut self) -> bool {
        self.num_new_vertices = 0;
        self.new_to_parent_vertex_map.clear();

        let decoder_bitstream_version = self.decoder().bitstream_version();
        if decoder_bitstream_version < bitstream_version(2, 2) {
            let mut num_new_verts: u32 = 0;
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if decoder_bitstream_version < bitstream_version(2, 0) {
                if !buffer.decode(&mut num_new_verts) {
                    return false;
                }
            } else if !decode_varint(&mut num_new_verts, buffer) {
                return false;
            }
            self.num_new_vertices = num_new_verts as i32;
        }

        let mut num_encoded_vertices: u32 = 0;
        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if decoder_bitstream_version < bitstream_version(2, 0) {
                if !buffer.decode(&mut num_encoded_vertices) {
                    return false;
                }
            } else if !decode_varint(&mut num_encoded_vertices, buffer) {
                return false;
            }
        }
        self.num_encoded_vertices = num_encoded_vertices as i32;

        let mut num_faces: u32 = 0;
        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if decoder_bitstream_version < bitstream_version(2, 0) {
                if !buffer.decode(&mut num_faces) {
                    return false;
                }
            } else if !decode_varint(&mut num_faces, buffer) {
                return false;
            }
        }

        if num_faces > (u32::MAX / 3) {
            return false;
        }
        if num_encoded_vertices > num_faces * 3 {
            return false;
        }

        let min_num_face_edges = (3 * num_faces) / 2;
        let num_encoded_vertices_64 = num_encoded_vertices as u64;
        let max_num_vertex_edges = num_encoded_vertices_64 * (num_encoded_vertices_64 - 1) / 2;
        if max_num_vertex_edges < min_num_face_edges as u64 {
            return false;
        }

        let mut num_attribute_data: u8 = 0;
        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if !buffer.decode(&mut num_attribute_data) {
                return false;
            }
        }

        let mut num_encoded_symbols: u32 = 0;
        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if decoder_bitstream_version < bitstream_version(2, 0) {
                if !buffer.decode(&mut num_encoded_symbols) {
                    return false;
                }
            } else if !decode_varint(&mut num_encoded_symbols, buffer) {
                return false;
            }
        }
        if num_faces < num_encoded_symbols {
            return false;
        }
        let max_encoded_faces = num_encoded_symbols + (num_encoded_symbols / 3);
        if num_faces > max_encoded_faces {
            return false;
        }

        let mut num_encoded_split_symbols: u32 = 0;
        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if decoder_bitstream_version < bitstream_version(2, 0) {
                if !buffer.decode(&mut num_encoded_split_symbols) {
                    return false;
                }
            } else if !decode_varint(&mut num_encoded_split_symbols, buffer) {
                return false;
            }
        }
        if num_encoded_split_symbols > num_encoded_symbols {
            return false;
        }

        self.vertex_traversal_length.clear();
        self.corner_table = Some(Box::new(CornerTable::new()));
        if self.corner_table.is_none() {
            return false;
        }
        self.processed_corner_ids.clear();
        self.processed_corner_ids.reserve(num_faces as usize);
        self.processed_connectivity_corners.clear();
        self.processed_connectivity_corners
            .reserve(num_faces as usize);
        self.topology_split_data.clear();
        self.hole_event_data.clear();
        self.init_face_configurations.clear();
        self.init_corners.clear();

        self.last_symbol_id = -1;
        self.last_face_id = -1;
        self.last_vert_id = -1;

        self.attribute_data.clear();
        self.attribute_data
            .resize_with(num_attribute_data as usize, AttributeData::new);

        {
            let ct = self.corner_table.as_mut().unwrap();
            if !ct.reset_with_vertices(
                num_faces as i32,
                (num_encoded_vertices + num_encoded_split_symbols) as i32,
            ) {
                return false;
            }
        }

        self.is_vert_hole = vec![true; (num_encoded_vertices + num_encoded_split_symbols) as usize];

        let mut topology_split_decoded_bytes = -1;
        if decoder_bitstream_version < bitstream_version(2, 2) {
            let mut encoded_connectivity_size: u32 = 0;
            let (data_head, bitstream_version) = {
                let buffer = match self.decoder_mut().buffer_mut() {
                    Some(buf) => buf,
                    None => return false,
                };
                if decoder_bitstream_version < bitstream_version(2, 0) {
                    if !buffer.decode(&mut encoded_connectivity_size) {
                        return false;
                    }
                } else if !decode_varint(&mut encoded_connectivity_size, buffer) {
                    return false;
                }
                if encoded_connectivity_size == 0
                    || encoded_connectivity_size as i64 > buffer.remaining_size()
                {
                    return false;
                }
                let data_head = buffer.data_head().to_vec();
                (data_head, buffer.bitstream_version())
            };
            let start = encoded_connectivity_size as usize;
            if start > data_head.len() {
                return false;
            }
            let mut event_buffer = DecoderBuffer::new();
            event_buffer.init_with_version(&data_head[start..], bitstream_version);
            topology_split_decoded_bytes =
                self.decode_hole_and_topology_split_events(&mut event_buffer);
            if topology_split_decoded_bytes == -1 {
                return false;
            }
        } else {
            let buffer_ptr = unsafe {
                match (*self.decoder).buffer_mut() {
                    Some(buf) => buf as *mut DecoderBuffer,
                    None => return false,
                }
            };
            if unsafe { self.decode_hole_and_topology_split_events(&mut *buffer_ptr) } == -1 {
                return false;
            }
        }

        let trait_ptr: *const dyn MeshEdgebreakerDecoderImplInterface = self;
        self.traversal_decoder.init(trait_ptr);
        self.traversal_decoder
            .set_num_encoded_vertices((num_encoded_vertices + num_encoded_split_symbols) as i32);
        self.traversal_decoder
            .set_num_attribute_data(num_attribute_data as usize);

        let mut traversal_end_buffer = DecoderBuffer::new();
        if !self.traversal_decoder.start(&mut traversal_end_buffer) {
            return false;
        }

        let num_connectivity_verts = self.decode_connectivity_internal(num_encoded_symbols as i32);
        if num_connectivity_verts == -1 {
            return false;
        }

        {
            let buffer = match self.decoder_mut().buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            buffer.init_with_version(
                traversal_end_buffer.data_head(),
                traversal_end_buffer.bitstream_version(),
            );
            if decoder_bitstream_version < bitstream_version(2, 2) {
                buffer.advance(topology_split_decoded_bytes as i64);
            }
        }

        if !self.attribute_data.is_empty() {
            if decoder_bitstream_version < bitstream_version(2, 1) {
                let num_corners = match self.corner_table_ref() {
                    Some(ct) => ct.num_corners(),
                    None => return false,
                };
                for ci in (0..num_corners).step_by(3) {
                    if !self.decode_attribute_connectivities_on_face_legacy(CornerIndex::from(
                        ci as u32,
                    )) {
                        return false;
                    }
                }
            } else {
                let num_corners = match self.corner_table_ref() {
                    Some(ct) => ct.num_corners(),
                    None => return false,
                };
                for ci in (0..num_corners).step_by(3) {
                    if !self.decode_attribute_connectivities_on_face(CornerIndex::from(ci as u32)) {
                        return false;
                    }
                }
            }
        }
        self.traversal_decoder.done();

        let corner_table = self.corner_table_static();
        for data in &mut self.attribute_data {
            data.connectivity_data.init_empty(corner_table);
            for &c in &data.attribute_seam_corners {
                data.connectivity_data
                    .add_seam_edge(CornerIndex::from(c as u32));
            }
            if !data.connectivity_data.recompute_vertices(None, None) {
                return false;
            }
        }

        let num_vertices = match self.corner_table_ref() {
            Some(ct) => ct.num_vertices(),
            None => return false,
        };
        self.pos_encoding_data.init(num_vertices);
        for data in &mut self.attribute_data {
            let mut att_connectivity_verts = data.connectivity_data.num_vertices();
            if att_connectivity_verts < num_vertices {
                att_connectivity_verts = num_vertices;
            }
            data.encoding_data.init(att_connectivity_verts);
        }

        if !self.assign_points_to_corners(num_connectivity_verts) {
            return false;
        }
        true
    }

    fn on_attributes_decoded(&mut self) -> bool {
        true
    }

    fn get_decoder(&self) -> &MeshEdgebreakerDecoder {
        self.decoder()
    }

    fn get_corner_table(&self) -> Option<&CornerTable> {
        self.corner_table_ref()
    }
}
