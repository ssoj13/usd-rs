//! Depth-first mesh traverser.
//! Reference: `_ref/draco/src/draco/compression/mesh/traverser/depth_first_traverser.h`.

use draco_core::attributes::geometry_indices::{
    CornerIndex, FaceIndex, INVALID_CORNER_INDEX, INVALID_FACE_INDEX, INVALID_VERTEX_INDEX,
};

use crate::compression::mesh::traverser::traverser_base::{
    MeshTraverser, TraversalCornerTable, TraversalObserver, TraverserBase,
};

pub struct DepthFirstTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver,
{
    base: TraverserBase<CornerTableT, TraversalObserverT>,
    corner_traversal_stack: Vec<CornerIndex>,
}

impl<CornerTableT, TraversalObserverT> DepthFirstTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    pub fn new() -> Self {
        Self {
            base: TraverserBase::new(),
            corner_traversal_stack: Vec::new(),
        }
    }
}

impl<CornerTableT, TraversalObserverT> Default
    for DepthFirstTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<CornerTableT, TraversalObserverT> MeshTraverser
    for DepthFirstTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    type CornerTable = CornerTableT;
    type Observer = TraversalObserverT;

    fn init(&mut self, corner_table: &Self::CornerTable, observer: Self::Observer) {
        self.base.init(corner_table, observer);
    }

    fn on_traversal_start(&mut self) {}

    fn on_traversal_end(&mut self) {}

    fn traverse_from_corner(&mut self, mut corner_id: CornerIndex) -> bool {
        if self.base.is_face_visited_corner(corner_id) {
            return true;
        }
        self.corner_traversal_stack.clear();
        self.corner_traversal_stack.push(corner_id);

        let corner_table_ptr = self.base.corner_table() as *const CornerTableT;
        let corner_table = unsafe { &*corner_table_ptr };
        let next_vert = corner_table.vertex(corner_table.next(corner_id));
        let prev_vert = corner_table.vertex(corner_table.previous(corner_id));
        if next_vert == INVALID_VERTEX_INDEX || prev_vert == INVALID_VERTEX_INDEX {
            return false;
        }
        if !self.base.is_vertex_visited(next_vert) {
            self.base.mark_vertex_visited(next_vert);
            self.base
                .traversal_observer()
                .on_new_vertex_visited(next_vert, corner_table.next(corner_id));
        }
        if !self.base.is_vertex_visited(prev_vert) {
            self.base.mark_vertex_visited(prev_vert);
            self.base
                .traversal_observer()
                .on_new_vertex_visited(prev_vert, corner_table.previous(corner_id));
        }

        while let Some(&stack_corner) = self.corner_traversal_stack.last() {
            corner_id = stack_corner;
            let mut face_id = FaceIndex::from(corner_id.value() / 3);
            if corner_id == INVALID_CORNER_INDEX || self.base.is_face_visited(face_id) {
                self.corner_traversal_stack.pop();
                continue;
            }
            loop {
                self.base.mark_face_visited(face_id);
                self.base.traversal_observer().on_new_face_visited(face_id);
                let vert_id = corner_table.vertex(corner_id);
                if vert_id == INVALID_VERTEX_INDEX {
                    return false;
                }
                if !self.base.is_vertex_visited(vert_id) {
                    let on_boundary = corner_table.is_on_boundary(vert_id);
                    self.base.mark_vertex_visited(vert_id);
                    self.base
                        .traversal_observer()
                        .on_new_vertex_visited(vert_id, corner_id);
                    if !on_boundary {
                        corner_id = corner_table.get_right_corner(corner_id);
                        face_id = FaceIndex::from(corner_id.value() / 3);
                        continue;
                    }
                }

                let right_corner_id = corner_table.get_right_corner(corner_id);
                let left_corner_id = corner_table.get_left_corner(corner_id);
                let right_face_id = if right_corner_id == INVALID_CORNER_INDEX {
                    INVALID_FACE_INDEX
                } else {
                    FaceIndex::from(right_corner_id.value() / 3)
                };
                let left_face_id = if left_corner_id == INVALID_CORNER_INDEX {
                    INVALID_FACE_INDEX
                } else {
                    FaceIndex::from(left_corner_id.value() / 3)
                };
                if self.base.is_face_visited(right_face_id) {
                    if self.base.is_face_visited(left_face_id) {
                        self.corner_traversal_stack.pop();
                        break;
                    } else {
                        corner_id = left_corner_id;
                        face_id = left_face_id;
                    }
                } else {
                    if self.base.is_face_visited(left_face_id) {
                        corner_id = right_corner_id;
                        face_id = right_face_id;
                    } else {
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

    fn corner_table(&self) -> &Self::CornerTable {
        self.base.corner_table()
    }
}
