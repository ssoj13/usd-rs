//! TraceCollection - Collection of events per thread.
//!
//! Port of pxr/base/trace/collection.h

use super::category::CategoryId;
use super::event::Event;
use super::event_list::EventList;
use super::threads::ThreadId;
use std::collections::BTreeMap;

/// Collection of events organized by thread.
///
/// This class owns lists of TraceEvent instances per thread, and allows
/// read access to them.
#[derive(Debug, Default)]
pub struct Collection {
    /// Events per thread (BTreeMap for ordered iteration).
    events_per_thread: BTreeMap<ThreadId, EventList>,
}

/// Visitor trait for iterating over collection events.
pub trait Visitor {
    /// Called at the beginning of iteration.
    fn on_begin_collection(&mut self);

    /// Called at the end of iteration.
    fn on_end_collection(&mut self);

    /// Called before events from a thread.
    fn on_begin_thread(&mut self, thread_id: ThreadId);

    /// Called after events from a thread.
    fn on_end_thread(&mut self, thread_id: ThreadId);

    /// Returns true if the visitor accepts events with this category.
    fn accepts_category(&self, category_id: CategoryId) -> bool;

    /// Called for each event.
    fn on_event(&mut self, thread_id: ThreadId, key: &str, event: &Event);
}

impl Collection {
    /// Creates a new empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the collection is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events_per_thread.is_empty()
    }

    /// Returns the number of threads with events.
    #[inline]
    pub fn thread_count(&self) -> usize {
        self.events_per_thread.len()
    }

    /// Returns total number of events across all threads.
    pub fn event_count(&self) -> usize {
        self.events_per_thread.values().map(|el| el.len()).sum()
    }

    /// Adds events to the collection, taking ownership.
    pub fn add_to_collection(&mut self, thread_id: ThreadId, events: EventList) {
        if let Some(existing) = self.events_per_thread.get_mut(&thread_id) {
            existing.append(events);
        } else {
            self.events_per_thread.insert(thread_id, events);
        }
    }

    /// Forward iterates over events and calls visitor callbacks.
    pub fn iterate<V: Visitor>(&self, visitor: &mut V) {
        self.iterate_impl(visitor, false);
    }

    /// Reverse iterates over events and calls visitor callbacks.
    pub fn reverse_iterate<V: Visitor>(&self, visitor: &mut V) {
        self.iterate_impl(visitor, true);
    }

    /// Merges another collection into this one.
    pub fn merge(&mut self, other: Collection) {
        for (thread_id, events) in other.events_per_thread {
            self.add_to_collection(thread_id, events);
        }
    }

    /// Clears all events.
    pub fn clear(&mut self) {
        self.events_per_thread.clear();
    }

    /// Returns an iterator over (thread_id, event_list) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&ThreadId, &EventList)> {
        self.events_per_thread.iter()
    }

    /// Returns the events for a specific thread.
    pub fn get_thread_events(&self, thread_id: &ThreadId) -> Option<&EventList> {
        self.events_per_thread.get(thread_id)
    }

    /// Returns all thread IDs that have events.
    pub fn thread_ids(&self) -> impl Iterator<Item = &ThreadId> {
        self.events_per_thread.keys()
    }

    fn iterate_impl<V: Visitor>(&self, visitor: &mut V, reverse: bool) {
        visitor.on_begin_collection();

        for (thread_id, event_list) in &self.events_per_thread {
            visitor.on_begin_thread(thread_id.clone());

            if reverse {
                for event in event_list.iter_rev() {
                    // C++ checks AcceptsCategory before calling OnEvent
                    if !visitor.accepts_category(event.category()) {
                        continue;
                    }
                    visitor.on_event(thread_id.clone(), event.key(), event);
                }
            } else {
                for event in event_list.iter() {
                    if !visitor.accepts_category(event.category()) {
                        continue;
                    }
                    visitor.on_event(thread_id.clone(), event.key(), event);
                }
            }

            visitor.on_end_thread(thread_id.clone());
        }

        visitor.on_end_collection();
    }
}

/// Simple visitor that collects all events.
#[derive(Debug, Default)]
pub struct CollectingVisitor {
    /// All collected events with their thread IDs.
    pub events: Vec<(ThreadId, Event)>,
}

impl Visitor for CollectingVisitor {
    fn on_begin_collection(&mut self) {
        self.events.clear();
    }

    fn on_end_collection(&mut self) {}

    fn on_begin_thread(&mut self, _thread_id: ThreadId) {}

    fn on_end_thread(&mut self, _thread_id: ThreadId) {}

    fn accepts_category(&self, _category_id: CategoryId) -> bool {
        true
    }

    fn on_event(&mut self, thread_id: ThreadId, _key: &str, event: &Event) {
        self.events.push((thread_id, event.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Event;

    #[test]
    fn test_collection_basic() {
        let mut collection = Collection::new();
        assert!(collection.is_empty());

        let mut events = EventList::new();
        events.push(Event::begin("test", 100));
        events.push(Event::end("test", 200));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        assert!(!collection.is_empty());
        assert_eq!(collection.thread_count(), 1);
        assert_eq!(collection.event_count(), 2);
    }

    #[test]
    fn test_collection_visitor() {
        let mut collection = Collection::new();

        let mut events = EventList::new();
        events.push(Event::begin("test1", 100));
        events.push(Event::end("test1", 200));
        collection.add_to_collection(ThreadId::new("thread1"), events);

        let mut visitor = CollectingVisitor::default();
        collection.iterate(&mut visitor);

        assert_eq!(visitor.events.len(), 2);
    }

    #[test]
    fn test_collection_merge() {
        let mut c1 = Collection::new();
        let mut c2 = Collection::new();

        let mut e1 = EventList::new();
        e1.push(Event::begin("a", 100));
        c1.add_to_collection(ThreadId::new("thread1"), e1);

        let mut e2 = EventList::new();
        e2.push(Event::begin("b", 200));
        c2.add_to_collection(ThreadId::new("thread2"), e2);

        c1.merge(c2);
        assert_eq!(c1.thread_count(), 2);
        assert_eq!(c1.event_count(), 2);
    }
}
