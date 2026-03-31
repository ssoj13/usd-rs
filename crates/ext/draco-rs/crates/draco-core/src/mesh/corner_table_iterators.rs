//! Corner table iterator utilities.
//! Reference: `_ref/draco/src/draco/mesh/corner_table_iterators.h`.

use crate::attributes::geometry_indices::{
    CornerIndex, FaceIndex, VertexIndex, INVALID_CORNER_INDEX,
};

pub trait CornerTableTraversal {
    fn left_most_corner(&self, v: VertexIndex) -> CornerIndex;
    fn swing_left(&self, c: CornerIndex) -> CornerIndex;
    fn swing_right(&self, c: CornerIndex) -> CornerIndex;
    fn previous(&self, c: CornerIndex) -> CornerIndex;
    fn next(&self, c: CornerIndex) -> CornerIndex;
    fn opposite(&self, c: CornerIndex) -> CornerIndex;
    fn face(&self, c: CornerIndex) -> FaceIndex;
    fn first_corner(&self, f: FaceIndex) -> CornerIndex;
    fn vertex(&self, c: CornerIndex) -> VertexIndex;
}

// Iterator over vertices in the 1-ring around a specified vertex.
#[derive(Clone, Copy, Debug)]
pub struct VertexRingIterator<'a, T: CornerTableTraversal> {
    corner_table: Option<&'a T>,
    start_corner: CornerIndex,
    corner: CornerIndex,
    left_traversal: bool,
}

impl<'a, T: CornerTableTraversal> VertexRingIterator<'a, T> {
    pub fn new() -> Self {
        Self {
            corner_table: None,
            start_corner: INVALID_CORNER_INDEX,
            corner: INVALID_CORNER_INDEX,
            left_traversal: true,
        }
    }

    pub fn from_vertex(table: &'a T, vert_id: VertexIndex) -> Self {
        let start_corner = table.left_most_corner(vert_id);
        Self {
            corner_table: Some(table),
            start_corner,
            corner: start_corner,
            left_traversal: true,
        }
    }

    pub fn vertex(&self) -> VertexIndex {
        let table = self.corner_table.expect("Corner table missing");
        let ring_corner = if self.left_traversal {
            table.previous(self.corner)
        } else {
            table.next(self.corner)
        };
        table.vertex(ring_corner)
    }

    pub fn edge_corner(&self) -> CornerIndex {
        let table = self.corner_table.expect("Corner table missing");
        if self.left_traversal {
            table.next(self.corner)
        } else {
            table.previous(self.corner)
        }
    }

    pub fn end(&self) -> bool {
        self.corner == INVALID_CORNER_INDEX
    }

    pub fn next(&mut self) {
        let table = self.corner_table.expect("Corner table missing");
        if self.left_traversal {
            self.corner = table.swing_left(self.corner);
            if self.corner == INVALID_CORNER_INDEX {
                self.corner = self.start_corner;
                self.left_traversal = false;
            } else if self.corner == self.start_corner {
                self.corner = INVALID_CORNER_INDEX;
            }
        } else {
            self.corner = table.swing_right(self.corner);
        }
    }

    pub fn end_iterator(other: VertexRingIterator<'a, T>) -> VertexRingIterator<'a, T> {
        let mut ret = other;
        ret.corner = INVALID_CORNER_INDEX;
        ret
    }
}

impl<'a, T: CornerTableTraversal> Iterator for VertexRingIterator<'a, T> {
    type Item = VertexIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end() {
            return None;
        }
        let current = self.vertex();
        self.next();
        Some(current)
    }
}

// Iterator over faces adjacent to the specified face.
#[derive(Clone, Copy, Debug)]
pub struct FaceAdjacencyIterator<'a, T: CornerTableTraversal> {
    corner_table: Option<&'a T>,
    start_corner: CornerIndex,
    corner: CornerIndex,
}

impl<'a, T: CornerTableTraversal> FaceAdjacencyIterator<'a, T> {
    pub fn new() -> Self {
        Self {
            corner_table: None,
            start_corner: INVALID_CORNER_INDEX,
            corner: INVALID_CORNER_INDEX,
        }
    }

    pub fn from_face(table: &'a T, face_id: FaceIndex) -> Self {
        let start_corner = table.first_corner(face_id);
        let mut iter = Self {
            corner_table: Some(table),
            start_corner,
            corner: start_corner,
        };
        if table.opposite(iter.corner) == INVALID_CORNER_INDEX {
            iter.find_next_face_neighbor();
        }
        iter
    }

    pub fn face(&self) -> FaceIndex {
        let table = self.corner_table.expect("Corner table missing");
        table.face(table.opposite(self.corner))
    }

    pub fn end(&self) -> bool {
        self.corner == INVALID_CORNER_INDEX
    }

    pub fn next(&mut self) {
        self.find_next_face_neighbor();
    }

    pub fn end_iterator(other: FaceAdjacencyIterator<'a, T>) -> FaceAdjacencyIterator<'a, T> {
        let mut ret = other;
        ret.corner = INVALID_CORNER_INDEX;
        ret
    }

    fn find_next_face_neighbor(&mut self) {
        let table = self.corner_table.expect("Corner table missing");
        while self.corner != INVALID_CORNER_INDEX {
            self.corner = table.next(self.corner);
            if self.corner == self.start_corner {
                self.corner = INVALID_CORNER_INDEX;
                return;
            }
            if table.opposite(self.corner) != INVALID_CORNER_INDEX {
                return;
            }
        }
    }
}

impl<'a, T: CornerTableTraversal> Iterator for FaceAdjacencyIterator<'a, T> {
    type Item = FaceIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end() {
            return None;
        }
        let current = self.face();
        self.next();
        Some(current)
    }
}

// Iterator over corners attached to a specified vertex.
#[derive(Clone, Copy, Debug)]
pub struct VertexCornersIterator<'a, T: CornerTableTraversal> {
    corner_table: Option<&'a T>,
    start_corner: CornerIndex,
    corner: CornerIndex,
    left_traversal: bool,
}

impl<'a, T: CornerTableTraversal> VertexCornersIterator<'a, T> {
    pub fn new() -> Self {
        Self {
            corner_table: None,
            start_corner: INVALID_CORNER_INDEX,
            corner: INVALID_CORNER_INDEX,
            left_traversal: true,
        }
    }

    pub fn from_vertex(table: &'a T, vert_id: VertexIndex) -> Self {
        let start_corner = table.left_most_corner(vert_id);
        Self {
            corner_table: Some(table),
            start_corner,
            corner: start_corner,
            left_traversal: true,
        }
    }

    pub fn from_corner(table: &'a T, corner_id: CornerIndex) -> Self {
        Self {
            corner_table: Some(table),
            start_corner: corner_id,
            corner: corner_id,
            left_traversal: true,
        }
    }

    pub fn corner(&self) -> CornerIndex {
        self.corner
    }

    pub fn end(&self) -> bool {
        self.corner == INVALID_CORNER_INDEX
    }

    pub fn next(&mut self) {
        let table = self.corner_table.expect("Corner table missing");
        if self.left_traversal {
            self.corner = table.swing_left(self.corner);
            if self.corner == INVALID_CORNER_INDEX {
                self.corner = table.swing_right(self.start_corner);
                self.left_traversal = false;
            } else if self.corner == self.start_corner {
                self.corner = INVALID_CORNER_INDEX;
            }
        } else {
            self.corner = table.swing_right(self.corner);
        }
    }

    pub fn end_iterator(other: VertexCornersIterator<'a, T>) -> VertexCornersIterator<'a, T> {
        let mut ret = other;
        ret.corner = INVALID_CORNER_INDEX;
        ret
    }
}

impl<'a, T: CornerTableTraversal> Iterator for VertexCornersIterator<'a, T> {
    type Item = CornerIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end() {
            return None;
        }
        let current = self.corner();
        self.next();
        Some(current)
    }
}
