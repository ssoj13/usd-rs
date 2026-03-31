//! Thread-safe concurrent list.
//!
//! Port of pxr/base/trace/concurrentList.h
//!
//! This module provides a thread-safe singly-linked list that supports
//! concurrent insertion and iteration.

use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

// ============================================================================
// Concurrent List
// ============================================================================

/// A thread-safe singly-linked list that supports concurrent insertion.
///
/// This list only allows appending items at the head and forward iteration.
/// Items cannot be removed once added.
pub struct ConcurrentList<T: Default> {
    /// Head of the linked list.
    head: AtomicPtr<Node<T>>,
}

/// A node in the concurrent list.
/// Cache-line aligned to prevent false sharing.
#[repr(C, align(128))]
struct Node<T> {
    /// The stored value.
    value: T,
    /// Pointer to the next node.
    next: *mut Node<T>,
}

impl<T: Default> ConcurrentList<T> {
    /// Creates a new empty concurrent list.
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Inserts a new default-initialized item at the head of the list.
    ///
    /// Returns a mutable reference to the newly inserted item.
    pub fn insert(&self) -> &mut T {
        // Allocate and initialize a new node
        let new_node = Box::into_raw(Box::new(Node {
            value: T::default(),
            next: ptr::null_mut(),
        }));

        // Atomically insert at the head
        loop {
            let current_head = self.head.load(Ordering::Relaxed);
            #[allow(unsafe_code)] // SAFETY: new_node is valid pointer from Box::into_raw
            unsafe {
                (*new_node).next = current_head;
            }

            if self
                .head
                .compare_exchange_weak(current_head, new_node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // Return reference to the value
        #[allow(unsafe_code)] // SAFETY: new_node was inserted and is valid
        unsafe {
            &mut (*new_node).value
        }
    }

    /// Inserts an item with the given value at the head of the list.
    ///
    /// Returns a reference to the newly inserted item.
    pub fn insert_value(&self, value: T) -> &T {
        let new_node = Box::into_raw(Box::new(Node {
            value,
            next: ptr::null_mut(),
        }));

        loop {
            let current_head = self.head.load(Ordering::Relaxed);
            #[allow(unsafe_code)] // SAFETY: new_node valid from Box::into_raw
            unsafe {
                (*new_node).next = current_head;
            }

            if self
                .head
                .compare_exchange_weak(current_head, new_node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        #[allow(unsafe_code)] // SAFETY: new_node inserted successfully
        unsafe {
            &(*new_node).value
        }
    }

    /// Returns an iterator over the list.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            current: self.head.load(Ordering::Acquire),
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire).is_null()
    }
}

impl<T: Default> Default for ConcurrentList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default> Drop for ConcurrentList<T> {
    fn drop(&mut self) {
        // Safe because we have exclusive access during drop
        let mut current = *self.head.get_mut();
        #[allow(unsafe_code)] // SAFETY: Deallocating all nodes via Box::from_raw
        while !current.is_null() {
            let node_to_delete = current;
            unsafe {
                current = (*current).next;
                let _ = Box::from_raw(node_to_delete);
            }
        }
    }
}

// SAFETY: ConcurrentList uses atomic operations for thread-safe insertion.
// Send/Sync is safe if T is Send (values are moved into nodes).
#[allow(unsafe_code)]
unsafe impl<T: Default + Send> Send for ConcurrentList<T> {}
#[allow(unsafe_code)]
unsafe impl<T: Default + Send> Sync for ConcurrentList<T> {}

// ============================================================================
// Iterator
// ============================================================================

/// Forward iterator over the concurrent list.
pub struct Iter<'a, T> {
    current: *mut Node<T>,
    _marker: std::marker::PhantomData<&'a T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            return None;
        }

        #[allow(unsafe_code)] // SAFETY: current is valid node from list
        let node = unsafe { &*self.current };
        self.current = node.next;
        Some(&node.value)
    }
}

// ============================================================================
// Mutable Iterator
// ============================================================================

/// Forward iterator that provides mutable access.
pub struct IterMut<'a, T> {
    current: *mut Node<T>,
    _marker: std::marker::PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            return None;
        }

        #[allow(unsafe_code)] // SAFETY: current is valid node, exclusive access
        let node = unsafe { &mut *self.current };
        self.current = node.next;
        Some(&mut node.value)
    }
}

// ============================================================================
// IntoIter
// ============================================================================

/// Consuming iterator over the concurrent list.
pub struct IntoIter<T: Default> {
    /// Ownership anchor - keeps list alive during iteration.
    _list: ConcurrentList<T>,
    current: *mut Node<T>,
}

impl<T: Default> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            return None;
        }

        #[allow(unsafe_code)] // SAFETY: Taking ownership of node via Box::from_raw
        let node = unsafe { Box::from_raw(self.current) };
        self.current = node.next;
        Some(node.value)
    }
}

impl<T: Default> IntoIterator for ConcurrentList<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(mut self) -> Self::IntoIter {
        let head = *self.head.get_mut();
        // Prevent the destructor from running on the nodes
        *self.head.get_mut() = ptr::null_mut();

        IntoIter {
            _list: self,
            current: head,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_empty_list() {
        let list: ConcurrentList<i32> = ConcurrentList::new();
        assert!(list.is_empty());
        assert_eq!(list.iter().count(), 0);
    }

    #[test]
    fn test_insert_single() {
        let list: ConcurrentList<i32> = ConcurrentList::new();

        let item = list.insert();
        *item = 42;

        assert!(!list.is_empty());
        assert_eq!(list.iter().count(), 1);
        assert_eq!(*list.iter().next().unwrap(), 42);
    }

    #[test]
    fn test_insert_multiple() {
        let list: ConcurrentList<i32> = ConcurrentList::new();

        for i in 0..10 {
            let item = list.insert();
            *item = i;
        }

        assert_eq!(list.iter().count(), 10);

        // Items are in reverse order (inserted at head)
        let values: Vec<_> = list.iter().copied().collect();
        assert_eq!(values, (0..10).rev().collect::<Vec<_>>());
    }

    #[test]
    fn test_insert_value() {
        let list: ConcurrentList<String> = ConcurrentList::new();

        list.insert_value("Hello".to_string());
        list.insert_value("World".to_string());

        let values: Vec<_> = list.iter().map(|s| s.as_str()).collect();
        assert_eq!(values, vec!["World", "Hello"]);
    }

    #[test]
    fn test_concurrent_insert() {
        let list = Arc::new(ConcurrentList::<i32>::new());
        let num_threads = 4;
        let items_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let list = Arc::clone(&list);
                thread::spawn(move || {
                    for i in 0..items_per_thread {
                        let item = list.insert();
                        *item = t * items_per_thread + i;
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread join failed");
        }

        assert_eq!(
            list.iter().count(),
            (num_threads * items_per_thread) as usize
        );
    }

    #[test]
    fn test_into_iter() {
        let list: ConcurrentList<i32> = ConcurrentList::new();

        for i in 0..5 {
            let item = list.insert();
            *item = i;
        }

        let values: Vec<_> = list.into_iter().collect();
        assert_eq!(values.len(), 5);
    }
}
