//! Traverser base utilities for mesh traversal.
//! Reference: `_ref/draco/src/draco/compression/mesh/traverser/traverser_base.h`.

use draco_core::attributes::geometry_indices::{
    CornerIndex, FaceIndex, VertexIndex, INVALID_CORNER_INDEX, INVALID_FACE_INDEX,
};
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

/// Observer interface for mesh traversal events.
pub trait TraversalObserver {
    fn on_new_face_visited(&mut self, face: FaceIndex);
    fn on_new_vertex_visited(&mut self, vert: VertexIndex, corner: CornerIndex);
}

/// Corner table interface required by traversal algorithms.
pub trait TraversalCornerTable {
    fn num_faces(&self) -> usize;
    fn num_vertices(&self) -> usize;
    fn next(&self, c: CornerIndex) -> CornerIndex;
    fn previous(&self, c: CornerIndex) -> CornerIndex;
    fn vertex(&self, c: CornerIndex) -> VertexIndex;
    fn is_on_boundary(&self, v: VertexIndex) -> bool;
    fn get_right_corner(&self, c: CornerIndex) -> CornerIndex;
    fn get_left_corner(&self, c: CornerIndex) -> CornerIndex;
    fn left_most_corner(&self, v: VertexIndex) -> CornerIndex;
}

impl TraversalCornerTable for CornerTable {
    fn num_faces(&self) -> usize {
        CornerTable::num_faces(self)
    }
    fn num_vertices(&self) -> usize {
        CornerTable::num_vertices(self)
    }
    fn next(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::next(self, c)
    }
    fn previous(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::previous(self, c)
    }
    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        CornerTable::vertex(self, c)
    }
    fn is_on_boundary(&self, v: VertexIndex) -> bool {
        CornerTable::is_on_boundary(self, v)
    }
    fn get_right_corner(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::get_right_corner(self, c)
    }
    fn get_left_corner(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::get_left_corner(self, c)
    }
    fn left_most_corner(&self, v: VertexIndex) -> CornerIndex {
        CornerTable::left_most_corner(self, v)
    }
}

impl<'a> TraversalCornerTable for MeshAttributeCornerTable<'a> {
    fn num_faces(&self) -> usize {
        MeshAttributeCornerTable::num_faces(self)
    }
    fn num_vertices(&self) -> usize {
        MeshAttributeCornerTable::num_vertices(self)
    }
    fn next(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::next(self, c)
    }
    fn previous(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::previous(self, c)
    }
    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        MeshAttributeCornerTable::vertex(self, c)
    }
    fn is_on_boundary(&self, v: VertexIndex) -> bool {
        MeshAttributeCornerTable::is_on_boundary(self, v)
    }
    fn get_right_corner(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::get_right_corner(self, c)
    }
    fn get_left_corner(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::get_left_corner(self, c)
    }
    fn left_most_corner(&self, v: VertexIndex) -> CornerIndex {
        MeshAttributeCornerTable::left_most_corner(self, v)
    }
}

/// Base class that tracks visited faces/vertices and owns the observer.
pub struct TraverserBase<CornerTableT: TraversalCornerTable, TraversalObserverT: TraversalObserver>
{
    corner_table: *const CornerTableT,
    traversal_observer: TraversalObserverT,
    is_face_visited: Vec<bool>,
    is_vertex_visited: Vec<bool>,
}

impl<CornerTableT, TraversalObserverT> TraverserBase<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    pub fn new() -> Self {
        Self {
            corner_table: std::ptr::null(),
            traversal_observer: TraversalObserverT::default(),
            is_face_visited: Vec::new(),
            is_vertex_visited: Vec::new(),
        }
    }
}

impl<CornerTableT, TraversalObserverT> TraverserBase<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver,
{
    pub fn init(&mut self, corner_table: &CornerTableT, traversal_observer: TraversalObserverT) {
        self.corner_table = corner_table as *const CornerTableT;
        self.is_face_visited = vec![false; corner_table.num_faces()];
        self.is_vertex_visited = vec![false; corner_table.num_vertices()];
        self.traversal_observer = traversal_observer;
    }

    pub fn corner_table(&self) -> &CornerTableT {
        unsafe { &*self.corner_table }
    }

    pub fn is_face_visited(&self, face_id: FaceIndex) -> bool {
        if face_id == INVALID_FACE_INDEX {
            return true;
        }
        self.is_face_visited[face_id.value() as usize]
    }

    pub fn is_face_visited_corner(&self, corner_id: CornerIndex) -> bool {
        if corner_id == INVALID_CORNER_INDEX {
            return true;
        }
        self.is_face_visited[(corner_id.value() / 3) as usize]
    }

    pub fn mark_face_visited(&mut self, face_id: FaceIndex) {
        self.is_face_visited[face_id.value() as usize] = true;
    }

    pub fn is_vertex_visited(&self, vert_id: VertexIndex) -> bool {
        self.is_vertex_visited[vert_id.value() as usize]
    }

    pub fn mark_vertex_visited(&mut self, vert_id: VertexIndex) {
        self.is_vertex_visited[vert_id.value() as usize] = true;
    }

    pub fn traversal_observer(&mut self) -> &mut TraversalObserverT {
        &mut self.traversal_observer
    }
}

/// Common interface for mesh traversers used by the sequencer.
pub trait MeshTraverser {
    type CornerTable: TraversalCornerTable;
    type Observer: TraversalObserver;

    fn init(&mut self, corner_table: &Self::CornerTable, observer: Self::Observer);
    fn on_traversal_start(&mut self);
    fn on_traversal_end(&mut self);
    fn traverse_from_corner(&mut self, corner_id: CornerIndex) -> bool;
    fn corner_table(&self) -> &Self::CornerTable;
}
