//! Max prediction degree mesh traverser.
//! Reference: `_ref/draco/src/draco/compression/mesh/traverser/max_prediction_degree_traverser.h`.

use draco_core::attributes::geometry_indices::{
    CornerIndex, FaceIndex, VertexIndex, INVALID_CORNER_INDEX, INVALID_FACE_INDEX,
};
use draco_core::core::draco_index_type_vector::IndexTypeVector;

use crate::compression::mesh::traverser::traverser_base::{
    MeshTraverser, TraversalCornerTable, TraversalObserver, TraverserBase,
};

pub struct MaxPredictionDegreeTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver,
{
    base: TraverserBase<CornerTableT, TraversalObserverT>,
    traversal_stacks: Vec<Vec<CornerIndex>>,
    best_priority: usize,
    prediction_degree: IndexTypeVector<VertexIndex, i32>,
}

impl<CornerTableT, TraversalObserverT>
    MaxPredictionDegreeTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    pub fn new() -> Self {
        Self {
            base: TraverserBase::new(),
            traversal_stacks: vec![Vec::new(); Self::max_priority()],
            best_priority: 0,
            prediction_degree: IndexTypeVector::new(),
        }
    }

    #[inline]
    fn max_priority() -> usize {
        3
    }

    fn pop_next_corner_to_traverse(&mut self) -> CornerIndex {
        for i in self.best_priority..Self::max_priority() {
            if let Some(ci) = self.traversal_stacks[i].pop() {
                self.best_priority = i;
                return ci;
            }
        }
        INVALID_CORNER_INDEX
    }

    fn add_corner_to_traversal_stack(&mut self, ci: CornerIndex, priority: usize) {
        let priority = priority.min(Self::max_priority() - 1);
        self.traversal_stacks[priority].push(ci);
        if priority < self.best_priority {
            self.best_priority = priority;
        }
    }

    fn compute_priority(&mut self, corner_id: CornerIndex) -> usize {
        let v_tip = self.base.corner_table().vertex(corner_id);
        let mut priority = 0usize;
        if !self.base.is_vertex_visited(v_tip) {
            let degree = {
                let current = self.prediction_degree[v_tip];
                self.prediction_degree[v_tip] = current + 1;
                self.prediction_degree[v_tip]
            };
            priority = if degree > 1 { 1 } else { 2 };
        }
        priority.min(Self::max_priority() - 1)
    }
}

impl<CornerTableT, TraversalObserverT> Default
    for MaxPredictionDegreeTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<CornerTableT, TraversalObserverT> MeshTraverser
    for MaxPredictionDegreeTraverser<CornerTableT, TraversalObserverT>
where
    CornerTableT: TraversalCornerTable,
    TraversalObserverT: TraversalObserver + Default,
{
    type CornerTable = CornerTableT;
    type Observer = TraversalObserverT;

    fn init(&mut self, corner_table: &Self::CornerTable, observer: Self::Observer) {
        self.base.init(corner_table, observer);
    }

    fn on_traversal_start(&mut self) {
        let num_vertices = self.base.corner_table().num_vertices();
        self.prediction_degree = IndexTypeVector::with_size_value(num_vertices, 0);
        self.traversal_stacks = vec![Vec::new(); Self::max_priority()];
        self.best_priority = 0;
    }

    fn on_traversal_end(&mut self) {}

    fn traverse_from_corner(&mut self, corner_id: CornerIndex) -> bool {
        if self.prediction_degree.size() == 0 {
            return true;
        }

        self.traversal_stacks[0].push(corner_id);
        self.best_priority = 0;

        let corner_table_ptr = self.base.corner_table() as *const CornerTableT;
        let corner_table = unsafe { &*corner_table_ptr };
        let next_vert = corner_table.vertex(corner_table.next(corner_id));
        let prev_vert = corner_table.vertex(corner_table.previous(corner_id));
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
        let tip_vertex = corner_table.vertex(corner_id);
        if !self.base.is_vertex_visited(tip_vertex) {
            self.base.mark_vertex_visited(tip_vertex);
            self.base
                .traversal_observer()
                .on_new_vertex_visited(tip_vertex, corner_id);
        }

        let mut current_corner = self.pop_next_corner_to_traverse();
        while current_corner != INVALID_CORNER_INDEX {
            let mut face_id = FaceIndex::from(current_corner.value() / 3);
            if self.base.is_face_visited(face_id) {
                current_corner = self.pop_next_corner_to_traverse();
                continue;
            }
            loop {
                face_id = FaceIndex::from(current_corner.value() / 3);
                self.base.mark_face_visited(face_id);
                self.base.traversal_observer().on_new_face_visited(face_id);

                let vert_id = corner_table.vertex(current_corner);
                if !self.base.is_vertex_visited(vert_id) {
                    self.base.mark_vertex_visited(vert_id);
                    self.base
                        .traversal_observer()
                        .on_new_vertex_visited(vert_id, current_corner);
                }

                let right_corner_id = corner_table.get_right_corner(current_corner);
                let left_corner_id = corner_table.get_left_corner(current_corner);
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
                let is_right_face_visited = self.base.is_face_visited(right_face_id);
                let is_left_face_visited = self.base.is_face_visited(left_face_id);

                if !is_left_face_visited {
                    let priority = self.compute_priority(left_corner_id);
                    if is_right_face_visited && priority <= self.best_priority {
                        current_corner = left_corner_id;
                        continue;
                    } else {
                        self.add_corner_to_traversal_stack(left_corner_id, priority);
                    }
                }
                if !is_right_face_visited {
                    let priority = self.compute_priority(right_corner_id);
                    if priority <= self.best_priority {
                        current_corner = right_corner_id;
                        continue;
                    } else {
                        self.add_corner_to_traversal_stack(right_corner_id, priority);
                    }
                }
                break;
            }
            current_corner = self.pop_next_corner_to_traverse();
        }
        true
    }

    fn corner_table(&self) -> &Self::CornerTable {
        self.base.corner_table()
    }
}
