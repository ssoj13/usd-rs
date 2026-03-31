//! Trace event container.
//!
//! Port of pxr/base/trace/eventContainer.h
//!
//! This module provides a container for TraceEvent instances that supports
//! efficient appending and bidirectional iteration.

// SAFETY: This module uses extensive unsafe for custom memory management
// (allocating node+events blocks, pointer arithmetic, manual drop).
// All unsafe operations are necessary for the C++ parity implementation.
#![allow(unsafe_code)]

use super::event::Event;
use std::alloc::{Layout, alloc, dealloc};
use std::ptr::NonNull;

// ============================================================================
// Constants
// ============================================================================

/// Default block size in bytes.
const DEFAULT_BLOCK_SIZE: usize = 4096;

/// Minimum number of events per block.
const MIN_EVENTS_PER_BLOCK: usize = 16;

// ============================================================================
// Event Container
// ============================================================================

/// A container for TraceEvent instances.
///
/// This container only allows appending events at the end and supports
/// both forward and reverse iteration. Events are stored in contiguous
/// blocks for cache efficiency.
pub struct EventContainer {
    /// First node in the list.
    front: *mut Node,
    /// Last node in the list.
    back: *mut Node,
    /// Pointer to where the next event should be constructed.
    next_event: *mut Event,
    /// Block size in bytes.
    block_size_bytes: usize,
}

/// A node in the event container's linked list.
#[repr(C)]
struct Node {
    /// End of valid events in this node.
    end: *mut Event,
    /// Sentinel marking the capacity limit.
    sentinel: *mut Event,
    /// Previous node in the list.
    prev: *mut Node,
    /// Next node in the list.
    next: *mut Node,
}

impl Node {
    /// Creates a new node with the given capacity.
    fn new(capacity: usize) -> Option<NonNull<Self>> {
        let event_size = std::mem::size_of::<Event>();
        let event_align = std::mem::align_of::<Event>();
        let node_size = std::mem::size_of::<Node>();

        // Ensure node is aligned for events
        let node_aligned_size = (node_size + event_align - 1) & !(event_align - 1);
        let total_size = node_aligned_size + capacity * event_size;

        let layout = Layout::from_size_align(total_size, event_align.max(8)).ok()?;
        // SAFETY: Allocating memory for node + events
        #[allow(unsafe_code)]
        let ptr = unsafe { alloc(layout) };

        if ptr.is_null() {
            return None;
        }

        let node_ptr = ptr as *mut Node;
        #[allow(unsafe_code)]
        let events_start = unsafe { ptr.add(node_aligned_size) as *mut Event };
        #[allow(unsafe_code)]
        let events_end = unsafe { events_start.add(capacity) };

        // SAFETY: Initializing allocated node
        #[allow(unsafe_code)]
        unsafe {
            (*node_ptr).end = events_start;
            (*node_ptr).sentinel = events_end;
            (*node_ptr).prev = std::ptr::null_mut();
            (*node_ptr).next = std::ptr::null_mut();
        }

        NonNull::new(node_ptr)
    }

    /// Returns true if the node cannot hold any more events.
    fn is_full(&self) -> bool {
        self.end == self.sentinel
    }

    /// Returns iterator to the beginning of events.
    fn begin(&self) -> *const Event {
        let node_ptr = self as *const Node as *const u8;
        let event_align = std::mem::align_of::<Event>();
        let node_size = std::mem::size_of::<Node>();
        let node_aligned_size = (node_size + event_align - 1) & !(event_align - 1);
        unsafe { node_ptr.add(node_aligned_size) as *const Event }
    }

    /// Returns iterator to the end of events.
    fn end(&self) -> *const Event {
        self.end as *const Event
    }

    /// Claims an event entry.
    fn claim_entry(&mut self) {
        self.end = unsafe { self.end.add(1) };
    }

    /// Returns the capacity of this node.
    fn capacity(&self) -> usize {
        let begin = self.begin();
        unsafe { self.sentinel.offset_from(begin as *mut Event) as usize }
    }

    /// Returns the number of events in this node.
    fn len(&self) -> usize {
        let begin = self.begin();
        unsafe { (self.end as *const Event).offset_from(begin) as usize }
    }
}

impl EventContainer {
    /// Creates a new empty event container.
    pub fn new() -> Self {
        Self::with_block_size(DEFAULT_BLOCK_SIZE)
    }

    /// Creates a new event container with the specified block size.
    pub fn with_block_size(block_size: usize) -> Self {
        Self {
            front: std::ptr::null_mut(),
            back: std::ptr::null_mut(),
            next_event: std::ptr::null_mut(),
            block_size_bytes: block_size.max(256),
        }
    }

    /// Returns true if the container is empty.
    pub fn is_empty(&self) -> bool {
        self.front.is_null() || self.begin() == self.end()
    }

    /// Appends a new event to the container.
    pub fn push(&mut self, event: Event) {
        if self.back.is_null() || unsafe { (*self.back).is_full() } {
            self.allocate();
        }

        // SAFETY: Initializing allocated node
        #[allow(unsafe_code)]
        unsafe {
            self.next_event.write(event);
            (*self.back).claim_entry();
            self.next_event = self.next_event.add(1);
        }
    }

    /// Returns an iterator to the beginning of the container.
    pub fn begin(&self) -> ConstIterator {
        if self.front.is_null() {
            ConstIterator {
                node: std::ptr::null(),
                event: std::ptr::null(),
            }
        } else {
            // SAFETY: Initializing allocated node
            #[allow(unsafe_code)]
            unsafe {
                ConstIterator {
                    node: self.front,
                    event: (*self.front).begin(),
                }
            }
        }
    }

    /// Returns an iterator to the end of the container.
    pub fn end(&self) -> ConstIterator {
        if self.back.is_null() {
            ConstIterator {
                node: std::ptr::null(),
                event: std::ptr::null(),
            }
        } else {
            // SAFETY: Initializing allocated node
            #[allow(unsafe_code)]
            unsafe {
                ConstIterator {
                    node: self.back,
                    event: (*self.back).end(),
                }
            }
        }
    }

    /// Returns an iterator over all events.
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        EventIter {
            current: self.begin(),
            end: self.end(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns a reverse iterator.
    pub fn iter_rev(&self) -> impl Iterator<Item = &Event> {
        EventRevIter {
            current: self.end(),
            begin: self.begin(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Appends another container to this one, taking ownership of its events.
    pub fn append(&mut self, mut other: EventContainer) {
        if other.is_empty() {
            return;
        }

        if self.is_empty() {
            *self = other;
            return;
        }

        // Join the lists
        // SAFETY: Initializing allocated node
        #[allow(unsafe_code)]
        unsafe {
            (*self.back).next = other.front;
            if !other.front.is_null() {
                (*other.front).prev = self.back;
            }
            self.back = other.back;
            self.next_event = other.next_event;
        }

        // Prevent other from deallocating nodes
        other.front = std::ptr::null_mut();
        other.back = std::ptr::null_mut();
    }

    /// Returns the total number of events.
    pub fn len(&self) -> usize {
        let mut count = 0;
        let mut current = self.front;
        while !current.is_null() {
            // SAFETY: Initializing allocated node
            #[allow(unsafe_code)]
            unsafe {
                count += (*current).len();
                current = (*current).next;
            }
        }
        count
    }

    /// Allocates a new block of memory for events.
    fn allocate(&mut self) {
        let event_size = std::mem::size_of::<Event>();
        let capacity = (self.block_size_bytes / event_size).max(MIN_EVENTS_PER_BLOCK);

        let node = Node::new(capacity).expect("Failed to allocate event node");
        let node_ptr = node.as_ptr();

        // SAFETY: Initializing allocated node
        #[allow(unsafe_code)]
        unsafe {
            if self.back.is_null() {
                self.front = node_ptr;
            } else {
                (*self.back).next = node_ptr;
                (*node_ptr).prev = self.back;
            }
            self.back = node_ptr;
            self.next_event = (*node_ptr).begin() as *mut Event;
        }
    }
}

impl Default for EventContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EventContainer {
    fn drop(&mut self) {
        let mut current = self.front;
        while !current.is_null() {
            // SAFETY: Initializing allocated node
            #[allow(unsafe_code)]
            unsafe {
                let node = current;
                current = (*node).next;

                // Drop all events in this node
                let begin = (*node).begin() as *mut Event;
                let end = (*node).end as *mut Event;
                let mut event_ptr = begin;
                while event_ptr != end {
                    std::ptr::drop_in_place(event_ptr);
                    event_ptr = event_ptr.add(1);
                }

                // Deallocate the node
                let event_align = std::mem::align_of::<Event>();
                let node_size = std::mem::size_of::<Node>();
                let node_aligned_size = (node_size + event_align - 1) & !(event_align - 1);
                let capacity = (*node).capacity();
                let total_size = node_aligned_size + capacity * std::mem::size_of::<Event>();
                let layout =
                    Layout::from_size_align(total_size, event_align.max(8)).expect("valid layout");
                dealloc(node as *mut u8, layout);
            }
        }
    }
}

// EventContainer is Send but not Sync
// SAFETY: EventContainer is Send-safe as it owns all nodes exclusively
#[allow(unsafe_code)]
unsafe impl Send for EventContainer {}

// ============================================================================
// Const Iterator
// ============================================================================

/// Bidirectional iterator for TraceEvents.
#[derive(Clone, Copy)]
pub struct ConstIterator {
    node: *const Node,
    event: *const Event,
}

impl ConstIterator {
    /// Advances the iterator to the next event.
    pub fn advance(&mut self) {
        if self.event.is_null() {
            return;
        }

        // SAFETY: Initializing allocated node
        #[allow(unsafe_code)]
        unsafe {
            self.event = self.event.add(1);
            if self.event == (*self.node).end() {
                let next = (*self.node).next;
                if !next.is_null() {
                    self.node = next;
                    self.event = (*self.node).begin();
                }
            }
        }
    }

    /// Moves the iterator to the previous event.
    pub fn reverse(&mut self) {
        if self.event.is_null() {
            return;
        }

        // SAFETY: Initializing allocated node
        #[allow(unsafe_code)]
        unsafe {
            if self.event == (*self.node).begin() {
                let prev = (*self.node).prev;
                if !prev.is_null() {
                    self.node = prev;
                    self.event = (*self.node).end();
                }
            }
            self.event = self.event.sub(1);
        }
    }

    /// Returns a reference to the current event.
    pub fn get(&self) -> Option<&Event> {
        if self.event.is_null() {
            None
        } else {
            unsafe { Some(&*self.event) }
        }
    }
}

impl PartialEq for ConstIterator {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event
    }
}

impl Eq for ConstIterator {}

// ============================================================================
// Event Iterator
// ============================================================================

/// Forward iterator over events.
struct EventIter<'a> {
    current: ConstIterator,
    end: ConstIterator,
    _marker: std::marker::PhantomData<&'a Event>,
}

impl<'a> Iterator for EventIter<'a> {
    type Item = &'a Event;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            return None;
        }

        // Get pointer before advancing
        let event_ptr = self.current.event;
        self.current.advance();
        // SAFETY: Events live as long as the container
        if !event_ptr.is_null() {
            Some(unsafe { &*event_ptr })
        } else {
            None
        }
    }
}

/// Reverse iterator over events.
struct EventRevIter<'a> {
    current: ConstIterator,
    begin: ConstIterator,
    _marker: std::marker::PhantomData<&'a Event>,
}

impl<'a> Iterator for EventRevIter<'a> {
    type Item = &'a Event;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.begin {
            return None;
        }

        self.current.reverse();
        let event = self.current.get()?;
        // SAFETY: Events live as long as the container
        Some(unsafe { &*(event as *const Event) })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventType;

    fn make_event(ts: u64) -> Event {
        Event::new("test", EventType::Begin, ts)
    }

    #[test]
    fn test_empty_container() {
        let container = EventContainer::new();
        assert!(container.is_empty());
        assert_eq!(container.len(), 0);
    }

    #[test]
    fn test_push_single() {
        let mut container = EventContainer::new();
        container.push(make_event(100));

        assert!(!container.is_empty());
        assert_eq!(container.len(), 1);
    }

    #[test]
    fn test_push_many() {
        let mut container = EventContainer::with_block_size(256);

        for i in 0..100 {
            container.push(make_event(i));
        }

        assert_eq!(container.len(), 100);
    }

    #[test]
    fn test_iteration() {
        let mut container = EventContainer::new();

        for i in 0..10 {
            container.push(make_event(i as u64));
        }

        let timestamps: Vec<_> = container.iter().map(|e| e.timestamp()).collect();
        assert_eq!(timestamps, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn test_reverse_iteration() {
        let mut container = EventContainer::new();

        for i in 0..10 {
            container.push(make_event(i as u64));
        }

        let timestamps: Vec<_> = container.iter_rev().map(|e| e.timestamp()).collect();
        assert_eq!(timestamps, (0..10).rev().collect::<Vec<_>>());
    }

    #[test]
    fn test_append() {
        let mut container1 = EventContainer::new();
        let mut container2 = EventContainer::new();

        for i in 0..5 {
            container1.push(make_event(i as u64));
        }
        for i in 5..10 {
            container2.push(make_event(i as u64));
        }

        container1.append(container2);

        assert_eq!(container1.len(), 10);
        let timestamps: Vec<_> = container1.iter().map(|e| e.timestamp()).collect();
        assert_eq!(timestamps, (0..10).collect::<Vec<_>>());
    }
}
