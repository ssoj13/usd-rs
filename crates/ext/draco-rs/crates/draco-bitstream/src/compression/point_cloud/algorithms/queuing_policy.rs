//! Queueing policy helpers for kd-tree encoders/decoders.
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/queuing_policy.h`.
//!
//! Provides a small uniform interface over queue/stack/priority queue choices.

use std::collections::{BinaryHeap, VecDeque};

/// FIFO queue policy.
pub struct Queue<T> {
    q: VecDeque<T>,
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        Self { q: VecDeque::new() }
    }

    pub fn empty(&self) -> bool {
        self.q.is_empty()
    }

    pub fn size(&self) -> usize {
        self.q.len()
    }

    pub fn clear(&mut self) {
        self.q.clear();
    }

    pub fn push(&mut self, value: T) {
        self.q.push_back(value);
    }

    pub fn pop(&mut self) {
        let _ = self.q.pop_front();
    }

    pub fn front(&self) -> &T {
        self.q.front().expect("queue front")
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// LIFO stack policy.
pub struct Stack<T> {
    s: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Self { s: Vec::new() }
    }

    pub fn empty(&self) -> bool {
        self.s.is_empty()
    }

    pub fn size(&self) -> usize {
        self.s.len()
    }

    pub fn clear(&mut self) {
        self.s.clear();
    }

    pub fn push(&mut self, value: T) {
        self.s.push(value);
    }

    pub fn pop(&mut self) {
        let _ = self.s.pop();
    }

    pub fn front(&self) -> &T {
        self.s.last().expect("stack front")
    }
}

impl<T> Default for Stack<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Priority queue policy (max-heap).
pub struct PriorityQueue<T: Ord> {
    s: BinaryHeap<T>,
}

impl<T: Ord> PriorityQueue<T> {
    pub fn new() -> Self {
        Self {
            s: BinaryHeap::new(),
        }
    }

    pub fn empty(&self) -> bool {
        self.s.is_empty()
    }

    pub fn size(&self) -> usize {
        self.s.len()
    }

    pub fn clear(&mut self) {
        self.s.clear();
    }

    pub fn push(&mut self, value: T) {
        self.s.push(value);
    }

    pub fn pop(&mut self) {
        let _ = self.s.pop();
    }

    pub fn front(&self) -> &T {
        self.s.peek().expect("priority queue front")
    }
}

impl<T: Ord> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}
