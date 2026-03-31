//! Mesh attribute corner table implementation.
//! Reference: `_ref/draco/src/draco/mesh/mesh_attribute_corner_table.h` + `.cc`.

use crate::attributes::geometry_indices::{
    AttributeValueIndex, CornerIndex, FaceIndex, VertexIndex, INVALID_CORNER_INDEX,
    INVALID_VERTEX_INDEX,
};
use crate::attributes::point_attribute::PointAttribute;
use crate::mesh::corner_table::CornerTable;
use crate::mesh::corner_table_iterators::{CornerTableTraversal, VertexRingIterator};
use crate::mesh::mesh::Mesh;
use crate::mesh::valence_cache::{ValenceCache, ValenceCacheTable};
use crate::{draco_dcheck, draco_dcheck_lt};

pub struct MeshAttributeCornerTable<'a> {
    is_edge_on_seam: Vec<bool>,
    is_vertex_on_seam: Vec<bool>,
    no_interior_seams: bool,
    corner_to_vertex_map: Vec<VertexIndex>,
    vertex_to_left_most_corner_map: Vec<CornerIndex>,
    vertex_to_attribute_entry_id_map: Vec<AttributeValueIndex>,
    corner_table: Option<&'a CornerTable>,
    valence_cache: ValenceCache<MeshAttributeCornerTable<'a>>,
}

impl<'a> MeshAttributeCornerTable<'a> {
    pub fn new() -> Self {
        Self {
            is_edge_on_seam: Vec::new(),
            is_vertex_on_seam: Vec::new(),
            no_interior_seams: true,
            corner_to_vertex_map: Vec::new(),
            vertex_to_left_most_corner_map: Vec::new(),
            vertex_to_attribute_entry_id_map: Vec::new(),
            corner_table: None,
            valence_cache: ValenceCache::new(),
        }
    }

    pub fn init_empty(&mut self, table: &'a CornerTable) -> bool {
        self.valence_cache.clear_valence_cache();
        self.valence_cache.clear_valence_cache_inaccurate();
        self.is_edge_on_seam = vec![false; table.num_corners()];
        self.is_vertex_on_seam = vec![false; table.num_vertices()];
        self.corner_to_vertex_map = vec![INVALID_VERTEX_INDEX; table.num_corners()];
        self.vertex_to_attribute_entry_id_map.clear();
        self.vertex_to_attribute_entry_id_map
            .reserve(table.num_vertices());
        self.vertex_to_left_most_corner_map.clear();
        self.vertex_to_left_most_corner_map
            .reserve(table.num_vertices());
        self.corner_table = Some(table);
        self.no_interior_seams = true;
        true
    }

    pub fn init_from_attribute(
        &mut self,
        mesh: &Mesh,
        table: &'a CornerTable,
        att: &PointAttribute,
    ) -> bool {
        if !self.init_empty(table) {
            return false;
        }
        self.valence_cache.clear_valence_cache();
        self.valence_cache.clear_valence_cache_inaccurate();

        let num_corners = self.corner_table().num_corners();
        for c in 0..num_corners {
            let corner = CornerIndex::from(c as u32);
            let face = self.corner_table().face(corner);
            if self.corner_table().is_degenerated(face) {
                continue;
            }
            let opp_corner = self.corner_table().opposite(corner);
            if opp_corner == INVALID_CORNER_INDEX {
                self.is_edge_on_seam[corner.value() as usize] = true;
                let v0 = self.corner_table().vertex(self.corner_table().next(corner));
                self.is_vertex_on_seam[v0.value() as usize] = true;
                let v1 = self
                    .corner_table()
                    .vertex(self.corner_table().previous(corner));
                self.is_vertex_on_seam[v1.value() as usize] = true;
                continue;
            }
            if opp_corner < corner {
                continue;
            }
            let mut act_c = corner;
            let mut act_sibling_c = opp_corner;
            for _ in 0..2 {
                act_c = self.corner_table().next(act_c);
                act_sibling_c = self.corner_table().previous(act_sibling_c);
                let point_id = mesh.corner_to_point_id(act_c);
                let sibling_point_id = mesh.corner_to_point_id(act_sibling_c);
                if att.mapped_index(point_id) != att.mapped_index(sibling_point_id) {
                    self.no_interior_seams = false;
                    self.is_edge_on_seam[corner.value() as usize] = true;
                    self.is_edge_on_seam[opp_corner.value() as usize] = true;
                    let v0 = self.corner_table().vertex(self.corner_table().next(corner));
                    let v1 = self
                        .corner_table()
                        .vertex(self.corner_table().previous(corner));
                    let v2 = self
                        .corner_table()
                        .vertex(self.corner_table().next(opp_corner));
                    let v3 = self
                        .corner_table()
                        .vertex(self.corner_table().previous(opp_corner));
                    self.is_vertex_on_seam[v0.value() as usize] = true;
                    self.is_vertex_on_seam[v1.value() as usize] = true;
                    self.is_vertex_on_seam[v2.value() as usize] = true;
                    self.is_vertex_on_seam[v3.value() as usize] = true;
                    break;
                }
            }
        }
        self.recompute_vertices(Some(mesh), Some(att));
        true
    }

    pub fn add_seam_edge(&mut self, corner: CornerIndex) {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.is_edge_on_seam[corner.value() as usize] = true;
        let v0 = self
            .corner_table()
            .vertex(self.corner_table().next(corner))
            .value() as usize;
        let v1 = self
            .corner_table()
            .vertex(self.corner_table().previous(corner))
            .value() as usize;
        self.is_vertex_on_seam[v0] = true;
        self.is_vertex_on_seam[v1] = true;

        let opp_corner = self.corner_table().opposite(corner);
        if opp_corner != INVALID_CORNER_INDEX {
            self.no_interior_seams = false;
            self.is_edge_on_seam[opp_corner.value() as usize] = true;
            let v2 = self
                .corner_table()
                .vertex(self.corner_table().next(opp_corner))
                .value() as usize;
            let v3 = self
                .corner_table()
                .vertex(self.corner_table().previous(opp_corner))
                .value() as usize;
            self.is_vertex_on_seam[v2] = true;
            self.is_vertex_on_seam[v3] = true;
        }
    }

    pub fn recompute_vertices(
        &mut self,
        mesh: Option<&Mesh>,
        att: Option<&PointAttribute>,
    ) -> bool {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        if let (Some(mesh), Some(att)) = (mesh, att) {
            self.recompute_vertices_internal(mesh, att, true)
        } else {
            self.recompute_vertices_internal_dummy()
        }
    }

    pub fn is_corner_opposite_to_seam_edge(&self, corner: CornerIndex) -> bool {
        if corner == INVALID_CORNER_INDEX {
            return false;
        }
        self.is_edge_on_seam[corner.value() as usize]
    }

    pub fn opposite(&self, corner: CornerIndex) -> CornerIndex {
        if corner == INVALID_CORNER_INDEX || self.is_corner_opposite_to_seam_edge(corner) {
            return INVALID_CORNER_INDEX;
        }
        self.corner_table().opposite(corner)
    }

    pub fn next(&self, corner: CornerIndex) -> CornerIndex {
        self.corner_table().next(corner)
    }

    pub fn previous(&self, corner: CornerIndex) -> CornerIndex {
        self.corner_table().previous(corner)
    }

    pub fn is_corner_on_seam(&self, corner: CornerIndex) -> bool {
        let v = self.corner_table().vertex(corner);
        self.is_vertex_on_seam[v.value() as usize]
    }

    pub fn get_left_corner(&self, corner: CornerIndex) -> CornerIndex {
        self.opposite(self.previous(corner))
    }

    pub fn get_right_corner(&self, corner: CornerIndex) -> CornerIndex {
        self.opposite(self.next(corner))
    }

    pub fn swing_right(&self, corner: CornerIndex) -> CornerIndex {
        self.previous(self.opposite(self.previous(corner)))
    }

    pub fn swing_left(&self, corner: CornerIndex) -> CornerIndex {
        self.next(self.opposite(self.next(corner)))
    }

    pub fn num_vertices(&self) -> usize {
        self.vertex_to_attribute_entry_id_map.len()
    }

    pub fn num_faces(&self) -> usize {
        self.corner_table().num_faces()
    }

    pub fn num_corners(&self) -> usize {
        self.corner_table().num_corners()
    }

    pub fn vertex(&self, corner: CornerIndex) -> VertexIndex {
        draco_dcheck_lt!(corner.value() as usize, self.corner_to_vertex_map.len());
        self.confident_vertex(corner)
    }

    pub fn confident_vertex(&self, corner: CornerIndex) -> VertexIndex {
        self.corner_to_vertex_map[corner.value() as usize]
    }

    pub fn vertex_parent(&self, vert: VertexIndex) -> VertexIndex {
        VertexIndex::from(self.vertex_to_attribute_entry_id_map[vert.value() as usize].value())
    }

    pub fn left_most_corner(&self, vert: VertexIndex) -> CornerIndex {
        self.vertex_to_left_most_corner_map[vert.value() as usize]
    }

    pub fn face(&self, corner: CornerIndex) -> FaceIndex {
        self.corner_table().face(corner)
    }

    pub fn first_corner(&self, face: FaceIndex) -> CornerIndex {
        self.corner_table().first_corner(face)
    }

    pub fn all_corners(&self, face: FaceIndex) -> [CornerIndex; 3] {
        self.corner_table().all_corners(face)
    }

    pub fn is_on_boundary(&self, vert: VertexIndex) -> bool {
        let corner = self.left_most_corner(vert);
        if corner == INVALID_CORNER_INDEX {
            return true;
        }
        if self.swing_left(corner) == INVALID_CORNER_INDEX {
            return true;
        }
        false
    }

    pub fn is_degenerated(&self, face: FaceIndex) -> bool {
        self.corner_table().is_degenerated(face)
    }

    pub fn no_interior_seams(&self) -> bool {
        self.no_interior_seams
    }

    pub fn corner_table(&self) -> &CornerTable {
        self.corner_table.expect("Corner table missing")
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
        draco_dcheck_lt!(c.value() as usize, self.corner_table().num_corners());
        if c == INVALID_CORNER_INDEX {
            return -1;
        }
        self.confident_valence_for_corner(c)
    }

    pub fn confident_valence_for_corner(&self, c: CornerIndex) -> i32 {
        draco_dcheck_lt!(c.value() as usize, self.corner_table().num_corners());
        self.confident_valence(self.vertex(c))
    }

    pub fn get_valence_cache(&self) -> &ValenceCache<MeshAttributeCornerTable<'a>> {
        &self.valence_cache
    }

    /// Returns mutable reference to valence cache.
    /// C++ uses `mutable` on cache fields; Rust needs explicit &mut accessor.
    pub fn get_valence_cache_mut(&mut self) -> &mut ValenceCache<MeshAttributeCornerTable<'a>> {
        &mut self.valence_cache
    }

    fn recompute_vertices_internal(
        &mut self,
        mesh: &Mesh,
        att: &PointAttribute,
        init_vertex_to_attribute_entry_map: bool,
    ) -> bool {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.vertex_to_attribute_entry_id_map.clear();
        self.vertex_to_left_most_corner_map.clear();
        let mut num_new_vertices = 0u32;
        let num_vertices = self.corner_table().num_vertices();
        for v in 0..num_vertices {
            let vert = VertexIndex::from(v as u32);
            let c = self.corner_table().left_most_corner(vert);
            if c == INVALID_CORNER_INDEX {
                continue;
            }
            let mut first_vert_id = AttributeValueIndex::from(num_new_vertices);
            num_new_vertices += 1;
            if init_vertex_to_attribute_entry_map {
                let point_id = mesh.corner_to_point_id(c);
                self.vertex_to_attribute_entry_id_map
                    .push(att.mapped_index(point_id));
            } else {
                self.vertex_to_attribute_entry_id_map.push(first_vert_id);
            }
            let mut first_c = c;
            let mut act_c;
            if self.is_vertex_on_seam[vert.value() as usize] {
                act_c = self.swing_left(first_c);
                while act_c != INVALID_CORNER_INDEX {
                    first_c = act_c;
                    act_c = self.swing_left(act_c);
                    if act_c == c {
                        return false;
                    }
                }
            }
            self.corner_to_vertex_map[first_c.value() as usize] =
                VertexIndex::from(first_vert_id.value());
            self.vertex_to_left_most_corner_map.push(first_c);
            act_c = self.corner_table().swing_right(first_c);
            while act_c != INVALID_CORNER_INDEX && act_c != first_c {
                if self.is_corner_opposite_to_seam_edge(self.corner_table().next(act_c)) {
                    first_vert_id = AttributeValueIndex::from(num_new_vertices);
                    num_new_vertices += 1;
                    if init_vertex_to_attribute_entry_map {
                        let point_id = mesh.corner_to_point_id(act_c);
                        self.vertex_to_attribute_entry_id_map
                            .push(att.mapped_index(point_id));
                    } else {
                        self.vertex_to_attribute_entry_id_map.push(first_vert_id);
                    }
                    self.vertex_to_left_most_corner_map.push(act_c);
                }
                self.corner_to_vertex_map[act_c.value() as usize] =
                    VertexIndex::from(first_vert_id.value());
                act_c = self.corner_table().swing_right(act_c);
            }
        }
        true
    }

    fn recompute_vertices_internal_dummy(&mut self) -> bool {
        draco_dcheck!(self.valence_cache.is_cache_empty());
        self.vertex_to_attribute_entry_id_map.clear();
        self.vertex_to_left_most_corner_map.clear();
        let mut num_new_vertices = 0u32;
        let num_vertices = self.corner_table().num_vertices();
        for v in 0..num_vertices {
            let vert = VertexIndex::from(v as u32);
            let c = self.corner_table().left_most_corner(vert);
            if c == INVALID_CORNER_INDEX {
                continue;
            }
            let mut first_vert_id = AttributeValueIndex::from(num_new_vertices);
            num_new_vertices += 1;
            self.vertex_to_attribute_entry_id_map.push(first_vert_id);
            let mut first_c = c;
            let mut act_c;
            if self.is_vertex_on_seam[vert.value() as usize] {
                act_c = self.swing_left(first_c);
                while act_c != INVALID_CORNER_INDEX {
                    first_c = act_c;
                    act_c = self.swing_left(act_c);
                    if act_c == c {
                        return false;
                    }
                }
            }
            self.corner_to_vertex_map[first_c.value() as usize] =
                VertexIndex::from(first_vert_id.value());
            self.vertex_to_left_most_corner_map.push(first_c);
            act_c = self.corner_table().swing_right(first_c);
            while act_c != INVALID_CORNER_INDEX && act_c != first_c {
                if self.is_corner_opposite_to_seam_edge(self.corner_table().next(act_c)) {
                    first_vert_id = AttributeValueIndex::from(num_new_vertices);
                    num_new_vertices += 1;
                    self.vertex_to_attribute_entry_id_map.push(first_vert_id);
                    self.vertex_to_left_most_corner_map.push(act_c);
                }
                self.corner_to_vertex_map[act_c.value() as usize] =
                    VertexIndex::from(first_vert_id.value());
                act_c = self.corner_table().swing_right(act_c);
            }
        }
        true
    }
}

impl<'a> Default for MeshAttributeCornerTable<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> CornerTableTraversal for MeshAttributeCornerTable<'a> {
    fn left_most_corner(&self, v: VertexIndex) -> CornerIndex {
        MeshAttributeCornerTable::left_most_corner(self, v)
    }
    fn swing_left(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::swing_left(self, c)
    }
    fn swing_right(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::swing_right(self, c)
    }
    fn previous(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::previous(self, c)
    }
    fn next(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::next(self, c)
    }
    fn opposite(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::opposite(self, c)
    }
    fn face(&self, c: CornerIndex) -> FaceIndex {
        MeshAttributeCornerTable::face(self, c)
    }
    fn first_corner(&self, f: FaceIndex) -> CornerIndex {
        MeshAttributeCornerTable::first_corner(self, f)
    }
    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        MeshAttributeCornerTable::vertex(self, c)
    }
}

impl<'a> ValenceCacheTable for MeshAttributeCornerTable<'a> {
    fn num_vertices(&self) -> usize {
        MeshAttributeCornerTable::num_vertices(self)
    }
    fn valence(&self, v: VertexIndex) -> i32 {
        MeshAttributeCornerTable::valence(self, v)
    }
    fn confident_vertex(&self, c: CornerIndex) -> VertexIndex {
        MeshAttributeCornerTable::confident_vertex(self, c)
    }
    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        MeshAttributeCornerTable::vertex(self, c)
    }
}
