//! TraceEventList - Ordered collection of trace events.
//!
//! Port of pxr/base/trace/eventList.h

use super::event::Event;
use super::key::DynamicKey;
use std::collections::HashSet;

/// Ordered collection of TraceEvents and associated keys/data.
///
/// This class represents an ordered collection of TraceEvents and the
/// TraceDynamicKeys and data that the events reference.
#[derive(Debug, Default)]
pub struct EventList {
    /// Events in this list.
    events: Vec<Event>,
    /// Cached keys (to keep references valid).
    key_cache: HashSet<String>,
}

impl EventList {
    /// Creates a new empty event list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the list is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the number of events.
    #[inline]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Adds an event to the end of the list.
    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Constructs an event at the end of the list.
    pub fn emplace_back(&mut self, event: Event) -> &Event {
        self.events.push(event);
        self.events.last().expect("just pushed")
    }

    /// Caches a key and returns a reference that remains valid for the lifetime of the list.
    pub fn cache_key(&mut self, key: &str) -> DynamicKey {
        self.key_cache.insert(key.to_string());
        DynamicKey::new(key)
    }

    /// Appends another list to this one, taking ownership of its events.
    pub fn append(&mut self, mut other: EventList) {
        self.events.append(&mut other.events);
        self.key_cache.extend(other.key_cache);
    }

    /// Clears all events from the list.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Returns an iterator over the events.
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.events.iter()
    }

    /// Returns a reverse iterator over the events.
    pub fn iter_rev(&self) -> impl Iterator<Item = &Event> {
        self.events.iter().rev()
    }

    /// Returns a slice of all events.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Takes ownership of all events, leaving the list empty.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

impl IntoIterator for EventList {
    type Item = Event;
    type IntoIter = std::vec::IntoIter<Event>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

impl<'a> IntoIterator for &'a EventList {
    type Item = &'a Event;
    type IntoIter = std::slice::Iter<'a, Event>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}

impl FromIterator<Event> for EventList {
    fn from_iter<I: IntoIterator<Item = Event>>(iter: I) -> Self {
        let events: Vec<Event> = iter.into_iter().collect();
        Self {
            events,
            key_cache: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::EventType;

    #[test]
    fn test_event_list_basic() {
        let mut list = EventList::new();
        assert!(list.is_empty());

        list.push(Event::begin("test1", 100));
        list.push(Event::end("test1", 200));

        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
    }

    #[test]
    fn test_event_list_append() {
        let mut list1 = EventList::new();
        list1.push(Event::begin("test1", 100));

        let mut list2 = EventList::new();
        list2.push(Event::end("test1", 200));

        list1.append(list2);
        assert_eq!(list1.len(), 2);
    }

    #[test]
    fn test_event_list_iter() {
        let mut list = EventList::new();
        list.push(Event::begin("a", 100));
        list.push(Event::begin("b", 200));
        list.push(Event::begin("c", 300));

        let keys: Vec<&str> = list.iter().map(|e| e.key()).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }
}
