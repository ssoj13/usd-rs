//! Corner table mesh connectivity.
//! Reference: `_ref/draco/src/draco/mesh/corner_table.h` + `.cc`.

use smallvec::SmallVec;

use crate::attributes::geometry_indices::{
    CornerIndex, FaceIndex, VertexIndex, INVALID_CORNER_INDEX, INVALID_FACE_INDEX,
    INVALID_VERTEX_INDEX,
};
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::mesh::corner_table_iterators::{CornerTableTraversal, VertexRingIterator};
use crate::mesh::valence_cache::{ValenceCache, ValenceCacheTable};
use crate::{draco_dcheck, draco_dcheck_lt};

pub type FaceType = [VertexIndex; 3];

pub const INVALID_FACE: FaceType = [
    INVALID_VERTEX_INDEX,
    INVALID_VERTEX_INDEX,
    INVALID_VERTEX_INDEX,
];

pub struct CornerTable {
    corner_to_vertex_map: IndexTypeVector<CornerIndex, VertexIndex>,
    opposite_corners: IndexTypeVector<CornerIndex, CornerIndex>,
    vertex_corners: IndexTypeVector<VertexIndex, CornerIndex>,
    num_original_vertices: usize,
    num_degenerated_faces: i32,
    num_isolated_vertices: i32,
    non_manifold_vertex_parents: IndexTypeVector<VertexIndex, VertexIndex>,
    valence_cache: ValenceCache<CornerTable>,
}

impl CornerTable {
    pub fn new() -> Self {
        Self {
            corner_to_vertex_map: IndexTypeVector::new(),
            opposite_corners: IndexTypeVector::new(),
            vertex_corners: IndexTypeVector::new(),
            num_original_vertices: 0,
            num_degenerated_faces: 0,
            num_isolated_vertices: 0,
            non_manifold_vertex_parents: IndexTypeVector::new(),
            valence_cache: ValenceCache::new(),
        }
    }

    pub fn create(faces: &IndexTypeVector<FaceIndex, FaceType>) -> Option<Self> {
        let mut ct = Self::new();
        if !ct.init(faces) {
            return None;
        }
        Some(ct)
    }

    pub fn init(&mut self, faces: &IndexTypeVector<FaceIndex, FaceType>) -> bool {
        self.valence_cache.clear_valence_cache();
        self.valence_cache.clear_valence_cache_inaccurate();
        self.corner_to_vertex_map.resize(faces.size() * 3);
        for fi in 0..faces.size() {
            let face = FaceIndex::new(fi as u32);
            let first_corner = self.first_corner(face);
            for i in 0..3 {
                self.corner_to_vertex_map[first_corner + i as u32] = faces[face][i];
            }
        }
        let mut num_vertices: i32 = -1;
        if !self.compute_opposite_corners(&mut num_vertices) {
            return false;
        }
        if !self.break_non_manifold_edges() {
            return false;
        }
        if !self.compute_vertex_corners(num_vertices) {
            return false;
        }
        true
    }

    pub fn reset(&mut self, num_faces: i32) -> bool {
        self.reset_with_vertices(num_faces, num_faces * 3)
    }

    pub fn reset_with_vertices(&mut self, num_faces: i32, num_vertices: i32) -> bool {
        if num_faces < 0 || num_vertices < 0 {
            return false;
        }
        let num_faces_unsigned = num_faces as u64;
        if num_faces_unsigned > (u32::MAX as u64) / 3 {
            return false;
        }
        let num_corners = (num_faces_unsigned * 3) as usize;
        self.corner_to_vertex_map
            .assign(num_corners, INVALID_VERTEX_INDEX);
        self.opposite_corners
            .assign(num_corners, INVALID_CORNER_INDEX);
        self.vertex_corners.reserve(num_vertices as usize);
        self.valence_cache.clear_valence_cache();
        self.valence_cache.clear_valence_cache_inaccurate();
        true
    }

    pub fn num_vertices(&self) -> usize {
        self.vertex_corners.size()
    }

    pub fn num_corners(&self) -> usize {
        self.corner_to_vertex_map.size()
    }

    pub fn num_faces(&self) -> usize {
        self.corner_to_vertex_map.size() / 3
    }

    pub fn opposite(&self, corner: CornerIndex) -> CornerIndex {
        if corner == INVALID_CORNER_INDEX {
            return corner;
        }
        self.opposite_corners[corner]
    }

    pub fn next(&self, corner: CornerIndex) -> CornerIndex {
        if corner == INVALID_CORNER_INDEX {
            return corner;
        }
        let next_corner = CornerIndex::new(corner.value() + 1);
        if self.local_index(next_corner) != 0 {
            next_corner
        } else {
            CornerIndex::new(next_corner.value() - 3)
        }
    }

    pub fn previous(&self, corner: CornerIndex) -> CornerIndex {
        if corner == INVALID_CORNER_INDEX {
            return corner;
        }
        if self.local_index(corner) != 0 {
            CornerIndex::new(corner.value() - 1)
        } else {
            CornerIndex::new(corner.value() + 2)
        }
    }

    pub fn vertex(&self, corner: CornerIndex) -> VertexIndex {
        if corner == INVALID_CORNER_INDEX {
            return INVALID_VERTEX_INDEX;
        }
        self.confident_vertex(corner)
    }

    pub fn confident_vertex(&self, corner: CornerIndex) -> VertexIndex {
        draco_dcheck_lt!(corner.value() as usize, self.num_corners());
        self.corner_to_vertex_map[corner]
    }

    pub fn face(&self, corner: CornerIndex) -> FaceIndex {
        if corner == INVALID_CORNER_INDEX {
            return INVALID_FACE_INDEX;
        }
        FaceIndex::new(corner.value() / 3)
    }

    pub fn first_corner(&self, face: FaceIndex) -> CornerIndex {
        if face == INVALID_FACE_INDEX {
            return INVALID_CORNER_INDEX;
        }
        CornerIndex::new(face.value() * 3)
    }

    pub fn all_corners(&self, face: FaceIndex) -> [CornerIndex; 3] {
        let ci = CornerIndex::new(face.value() * 3);
        [ci, ci + 1, ci + 2]
    }

    pub fn local_index(&self, corner: CornerIndex) -> u32 {
        corner.value() % 3
    }

    pub fn face_data(&self, face: FaceIndex) -> FaceType {
        let first_corner = self.first_corner(face);
        let mut face_data = [INVALID_VERTEX_INDEX; 3];
        for i in 0..3 {
            face_data[i] = self.corner_to_vertex_map[first_corner + i as u32];
        }
        face_data
    }

    pub fn set_face_data(&mut self, face: FaceIndex, data: FaceType) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        let first_corner = self.first_corner(face);
        for i in 0..3 {
            self.corner_to_vertex_map[first_corner + i as u32] = data[i];
        }
    }

    pub fn left_most_corner(&self, v: VertexIndex) -> CornerIndex {
        self.vertex_corners[v]
    }

    pub fn vertex_parent(&self, vertex: VertexIndex) -> VertexIndex {
        if (vertex.value() as usize) < self.num_original_vertices {
            return vertex;
        }
        let offset = (vertex.value() as usize) - self.num_original_vertices;
        self.non_manifold_vertex_parents[VertexIndex::new(offset as u32)]
    }

    pub fn is_valid(&self, c: CornerIndex) -> bool {
        self.vertex(c) != INVALID_VERTEX_INDEX
    }

    pub fn valence(&self, v: VertexIndex) -> i32 {
        if v == INVALID_VERTEX_INDEX {
            return -1;
        }
        self.confident_valence(v)
    }

    pub fn confident_valence(&self, v: VertexIndex) -> i32 {
        draco_dcheck_lt!(v.value() as usize, self.num_vertices());
        let mut it = VertexRingIterator::from_vertex(self, v);
        let mut valence = 0;
        while !it.end() {
            valence += 1;
            it.next();
        }
        valence
    }

    pub fn valence_for_corner(&self, c: CornerIndex) -> i32 {
        if c == INVALID_CORNER_INDEX {
            return -1;
        }
        self.confident_valence_for_corner(c)
    }

    pub fn confident_valence_for_corner(&self, c: CornerIndex) -> i32 {
        draco_dcheck_lt!(c.value() as usize, self.num_corners());
        self.confident_valence(self.confident_vertex(c))
    }

    pub fn is_on_boundary(&self, vert: VertexIndex) -> bool {
        let corner = self.left_most_corner(vert);
        if self.swing_left(corner) == INVALID_CORNER_INDEX {
            return true;
        }
        false
    }

    pub fn swing_right(&self, corner: CornerIndex) -> CornerIndex {
        self.previous(self.opposite(self.previous(corner)))
    }

    pub fn swing_left(&self, corner: CornerIndex) -> CornerIndex {
        self.next(self.opposite(self.next(corner)))
    }

    pub fn get_left_corner(&self, corner_id: CornerIndex) -> CornerIndex {
        if corner_id == INVALID_CORNER_INDEX {
            return INVALID_CORNER_INDEX;
        }
        self.opposite(self.previous(corner_id))
    }

    pub fn get_right_corner(&self, corner_id: CornerIndex) -> CornerIndex {
        if corner_id == INVALID_CORNER_INDEX {
            return INVALID_CORNER_INDEX;
        }
        self.opposite(self.next(corner_id))
    }

    pub fn num_new_vertices(&self) -> usize {
        self.num_vertices() - self.num_original_vertices
    }

    pub fn num_original_vertices(&self) -> usize {
        self.num_original_vertices
    }

    pub fn num_degenerated_faces(&self) -> i32 {
        self.num_degenerated_faces
    }

    pub fn num_isolated_vertices(&self) -> i32 {
        self.num_isolated_vertices
    }

    pub fn is_degenerated(&self, face: FaceIndex) -> bool {
        if face == INVALID_FACE_INDEX {
            return true;
        }
        let first_face_corner = self.first_corner(face);
        let v0 = self.vertex(first_face_corner);
        let v1 = self.vertex(self.next(first_face_corner));
        let v2 = self.vertex(self.previous(first_face_corner));
        v0 == v1 || v0 == v2 || v1 == v2
    }

    pub fn set_opposite_corner(&mut self, corner_id: CornerIndex, opp_corner_id: CornerIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.opposite_corners[corner_id] = opp_corner_id;
    }

    pub fn set_opposite_corners(&mut self, corner_0: CornerIndex, corner_1: CornerIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        if corner_0 != INVALID_CORNER_INDEX {
            self.set_opposite_corner(corner_0, corner_1);
        }
        if corner_1 != INVALID_CORNER_INDEX {
            self.set_opposite_corner(corner_1, corner_0);
        }
    }

    pub fn map_corner_to_vertex(&mut self, corner_id: CornerIndex, vert_id: VertexIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.corner_to_vertex_map[corner_id] = vert_id;
    }

    pub fn add_new_vertex(&mut self) -> VertexIndex {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.vertex_corners.push_back(INVALID_CORNER_INDEX);
        VertexIndex::new((self.vertex_corners.size() - 1) as u32)
    }

    pub fn add_new_face(&mut self, vertices: FaceType) -> FaceIndex {
        let new_face_index = FaceIndex::new(self.num_faces() as u32);
        for i in 0..3 {
            self.corner_to_vertex_map.push_back(vertices[i]);
            self.set_left_most_corner(
                vertices[i],
                CornerIndex::new((self.corner_to_vertex_map.size() - 1) as u32),
            );
        }
        self.opposite_corners
            .resize_with_value(self.corner_to_vertex_map.size(), INVALID_CORNER_INDEX);
        new_face_index
    }

    pub fn set_left_most_corner(&mut self, vert: VertexIndex, corner: CornerIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        if vert != INVALID_VERTEX_INDEX {
            self.vertex_corners[vert] = corner;
        }
    }

    pub fn update_vertex_to_corner_map(&mut self, vert: VertexIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        let first_c = self.vertex_corners[vert];
        if first_c == INVALID_CORNER_INDEX {
            return;
        }
        let mut act_c = self.swing_left(first_c);
        let mut c = first_c;
        while act_c != INVALID_CORNER_INDEX && act_c != first_c {
            c = act_c;
            act_c = self.swing_left(act_c);
        }
        if act_c != first_c {
            self.vertex_corners[vert] = c;
        }
    }

    pub fn set_num_vertices(&mut self, num_vertices: i32) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.vertex_corners
            .resize_with_value(num_vertices as usize, INVALID_CORNER_INDEX);
    }

    pub fn make_vertex_isolated(&mut self, vert: VertexIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.vertex_corners[vert] = INVALID_CORNER_INDEX;
    }

    pub fn is_vertex_isolated(&self, v: VertexIndex) -> bool {
        self.left_most_corner(v) == INVALID_CORNER_INDEX
    }

    pub fn make_face_invalid(&mut self, face: FaceIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        if face != INVALID_FACE_INDEX {
            let first_corner = self.first_corner(face);
            for i in 0..3 {
                self.corner_to_vertex_map[first_corner + i as u32] = INVALID_VERTEX_INDEX;
            }
        }
    }

    pub fn update_face_to_vertex_map(&mut self, vertex: VertexIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        // Inline VertexCornersIterator logic to avoid borrow conflict:
        // the iterator borrows &self while we need &mut self.corner_to_vertex_map.
        let start_corner = self.vertex_corners[vertex];
        if start_corner == INVALID_CORNER_INDEX {
            return;
        }
        let mut corner = start_corner;
        let mut left_traversal = true;
        while corner != INVALID_CORNER_INDEX {
            self.corner_to_vertex_map[corner] = vertex;
            if left_traversal {
                corner = self.swing_left(corner);
                if corner == INVALID_CORNER_INDEX {
                    corner = self.swing_right(start_corner);
                    left_traversal = false;
                } else if corner == start_corner {
                    break;
                }
            } else {
                corner = self.swing_right(corner);
            }
        }
    }

    pub fn valence_cache(&self) -> &ValenceCache<CornerTable> {
        &self.valence_cache
    }

    pub fn valence_cache_mut(&mut self) -> &mut ValenceCache<CornerTable> {
        &mut self.valence_cache
    }

    fn compute_opposite_corners(&mut self, num_vertices: &mut i32) -> bool {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.opposite_corners
            .resize_with_value(self.num_corners(), INVALID_CORNER_INDEX);

        let mut num_corners_on_vertices: Vec<usize> = Vec::new();
        num_corners_on_vertices.reserve(self.num_corners());
        for c in 0..self.num_corners() {
            let corner = CornerIndex::new(c as u32);
            let v1 = self.vertex(corner);
            let v1_value = v1.value() as usize;
            if v1_value >= num_corners_on_vertices.len() {
                num_corners_on_vertices.resize(v1_value + 1, 0);
            }
            num_corners_on_vertices[v1_value] += 1;
        }

        #[derive(Clone, Copy)]
        struct VertexEdgePair {
            sink_vert: VertexIndex,
            edge_corner: CornerIndex,
        }

        impl VertexEdgePair {
            fn new() -> Self {
                Self {
                    sink_vert: INVALID_VERTEX_INDEX,
                    edge_corner: INVALID_CORNER_INDEX,
                }
            }
        }

        let mut vertex_edges: Vec<VertexEdgePair> = vec![VertexEdgePair::new(); self.num_corners()];

        let mut vertex_offset: Vec<usize> = vec![0; num_corners_on_vertices.len()];
        let mut offset = 0usize;
        for i in 0..num_corners_on_vertices.len() {
            vertex_offset[i] = offset;
            offset += num_corners_on_vertices[i];
        }

        let mut c: u32 = 0;
        while (c as usize) < self.num_corners() {
            let corner = CornerIndex::new(c);
            let tip_v = self.vertex(corner);
            let source_v = self.vertex(self.next(corner));
            let sink_v = self.vertex(self.previous(corner));

            let face_index = self.face(corner);
            if corner == self.first_corner(face_index) {
                let v0 = self.vertex(corner);
                if v0 == source_v || v0 == sink_v || source_v == sink_v {
                    self.num_degenerated_faces += 1;
                    c += 3;
                    continue;
                }
            }

            let mut opposite_c = INVALID_CORNER_INDEX;
            let num_corners_on_vert = num_corners_on_vertices[sink_v.value() as usize];
            offset = vertex_offset[sink_v.value() as usize];
            for i in 0..num_corners_on_vert {
                let other_v = vertex_edges[offset].sink_vert;
                if other_v == INVALID_VERTEX_INDEX {
                    break;
                }
                if other_v == source_v {
                    if tip_v == self.vertex(vertex_edges[offset].edge_corner) {
                        offset += 1;
                        continue;
                    }
                    opposite_c = vertex_edges[offset].edge_corner;
                    let mut j = i + 1;
                    let mut shift_offset = offset;
                    while j < num_corners_on_vert {
                        vertex_edges[shift_offset] = vertex_edges[shift_offset + 1];
                        if vertex_edges[shift_offset].sink_vert == INVALID_VERTEX_INDEX {
                            break;
                        }
                        j += 1;
                        shift_offset += 1;
                    }
                    vertex_edges[shift_offset].sink_vert = INVALID_VERTEX_INDEX;
                    break;
                }
                offset += 1;
            }
            if opposite_c == INVALID_CORNER_INDEX {
                let num_corners_on_source_vert = num_corners_on_vertices[source_v.value() as usize];
                offset = vertex_offset[source_v.value() as usize];
                for _ in 0..num_corners_on_source_vert {
                    if vertex_edges[offset].sink_vert == INVALID_VERTEX_INDEX {
                        vertex_edges[offset].sink_vert = sink_v;
                        vertex_edges[offset].edge_corner = corner;
                        break;
                    }
                    offset += 1;
                }
            } else {
                self.opposite_corners[corner] = opposite_c;
                self.opposite_corners[opposite_c] = corner;
            }
            c += 1;
        }

        *num_vertices = num_corners_on_vertices.len() as i32;
        true
    }

    fn break_non_manifold_edges(&mut self) -> bool {
        let mut visited_corners = vec![false; self.num_corners()];
        // SmallVec avoids heap allocation for typical vertex valence (4–12).
        let mut sink_vertices: SmallVec<[(VertexIndex, CornerIndex); 12]> = SmallVec::new();
        loop {
            let mut mesh_connectivity_updated = false;
            for c_idx in 0..self.num_corners() {
                if visited_corners[c_idx] {
                    continue;
                }
                sink_vertices.clear();

                let mut first_c = CornerIndex::new(c_idx as u32);
                let mut current_c = first_c;
                loop {
                    let next_c = self.swing_left(current_c);
                    if next_c == first_c
                        || next_c == INVALID_CORNER_INDEX
                        || visited_corners[next_c.value() as usize]
                    {
                        break;
                    }
                    current_c = next_c;
                }

                first_c = current_c;

                loop {
                    visited_corners[current_c.value() as usize] = true;
                    let sink_c = self.next(current_c);
                    let sink_v = self.corner_to_vertex_map[sink_c];
                    let edge_corner = self.previous(current_c);
                    let mut vertex_connectivity_updated = false;

                    for attached in &sink_vertices {
                        if attached.0 == sink_v {
                            let other_edge_corner = attached.1;
                            let opp_edge_corner = self.opposite(edge_corner);
                            if opp_edge_corner == other_edge_corner {
                                continue;
                            }
                            let opp_other_edge_corner = self.opposite(other_edge_corner);
                            if opp_edge_corner != INVALID_CORNER_INDEX {
                                self.set_opposite_corner(opp_edge_corner, INVALID_CORNER_INDEX);
                            }
                            if opp_other_edge_corner != INVALID_CORNER_INDEX {
                                self.set_opposite_corner(
                                    opp_other_edge_corner,
                                    INVALID_CORNER_INDEX,
                                );
                            }
                            self.set_opposite_corner(edge_corner, INVALID_CORNER_INDEX);
                            self.set_opposite_corner(other_edge_corner, INVALID_CORNER_INDEX);

                            vertex_connectivity_updated = true;
                            break;
                        }
                    }

                    if vertex_connectivity_updated {
                        mesh_connectivity_updated = true;
                        break;
                    }

                    let new_sink_vert =
                        (self.corner_to_vertex_map[self.previous(current_c)], sink_c);
                    sink_vertices.push(new_sink_vert);

                    current_c = self.swing_right(current_c);
                    if current_c == first_c || current_c == INVALID_CORNER_INDEX {
                        break;
                    }
                }
            }

            if !mesh_connectivity_updated {
                break;
            }
        }
        true
    }

    fn compute_vertex_corners(&mut self, mut num_vertices: i32) -> bool {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.num_original_vertices = num_vertices as usize;
        self.vertex_corners
            .resize_with_value(num_vertices as usize, INVALID_CORNER_INDEX);

        let mut visited_vertices = vec![false; num_vertices as usize];
        let mut visited_corners = vec![false; self.num_corners()];

        for f in 0..self.num_faces() {
            let face = FaceIndex::new(f as u32);
            let first_face_corner = self.first_corner(face);
            if self.is_degenerated(face) {
                continue;
            }
            for k in 0..3 {
                let c = first_face_corner + k as u32;
                if visited_corners[c.value() as usize] {
                    continue;
                }
                let mut v = self.corner_to_vertex_map[c];
                let mut is_non_manifold_vertex = false;
                if visited_vertices[v.value() as usize] {
                    self.vertex_corners.push_back(INVALID_CORNER_INDEX);
                    self.non_manifold_vertex_parents.push_back(v);
                    visited_vertices.push(false);
                    v = VertexIndex::new(num_vertices as u32);
                    num_vertices += 1;
                    is_non_manifold_vertex = true;
                }
                visited_vertices[v.value() as usize] = true;

                let mut act_c = c;
                while act_c != INVALID_CORNER_INDEX {
                    visited_corners[act_c.value() as usize] = true;
                    self.vertex_corners[v] = act_c;
                    if is_non_manifold_vertex {
                        self.corner_to_vertex_map[act_c] = v;
                    }
                    act_c = self.swing_left(act_c);
                    if act_c == c {
                        break;
                    }
                }
                if act_c == INVALID_CORNER_INDEX {
                    act_c = self.swing_right(c);
                    while act_c != INVALID_CORNER_INDEX {
                        visited_corners[act_c.value() as usize] = true;
                        if is_non_manifold_vertex {
                            self.corner_to_vertex_map[act_c] = v;
                        }
                        act_c = self.swing_right(act_c);
                    }
                }
            }
        }

        self.num_isolated_vertices = 0;
        for visited in &visited_vertices {
            if !*visited {
                self.num_isolated_vertices += 1;
            }
        }
        true
    }
}

impl Default for CornerTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ValenceCacheTable for CornerTable {
    fn num_vertices(&self) -> usize {
        CornerTable::num_vertices(self)
    }

    fn valence(&self, v: VertexIndex) -> i32 {
        CornerTable::valence(self, v)
    }

    fn confident_vertex(&self, c: CornerIndex) -> VertexIndex {
        CornerTable::confident_vertex(self, c)
    }

    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        CornerTable::vertex(self, c)
    }
}

impl CornerTableTraversal for CornerTable {
    fn left_most_corner(&self, v: VertexIndex) -> CornerIndex {
        CornerTable::left_most_corner(self, v)
    }

    fn swing_left(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::swing_left(self, c)
    }

    fn swing_right(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::swing_right(self, c)
    }

    fn previous(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::previous(self, c)
    }

    fn next(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::next(self, c)
    }

    fn opposite(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::opposite(self, c)
    }

    fn face(&self, c: CornerIndex) -> FaceIndex {
        CornerTable::face(self, c)
    }

    fn first_corner(&self, f: FaceIndex) -> CornerIndex {
        CornerTable::first_corner(self, f)
    }

    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        CornerTable::vertex(self, c)
    }
}
