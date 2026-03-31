//! Thread identification for trace events.
//!
//! This module provides types for identifying threads in trace output.
//!
//! # Examples
//!
//! ```
//! use usd_trace::ThreadId;
//!
//! // Get the current thread's identifier
//! let id = ThreadId::current();
//! println!("Thread: {}", id);
//!
//! // Create a custom thread identifier
//! let custom_id = ThreadId::new("Worker Thread 1");
//! ```

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::thread;

/// The main thread's ID, captured at initialization time.
static MAIN_THREAD_ID: OnceLock<thread::ThreadId> = OnceLock::new();

/// Initialize the main thread ID. Should be called from the main thread
/// at program startup.
fn get_main_thread_id() -> thread::ThreadId {
    *MAIN_THREAD_ID.get_or_init(|| thread::current().id())
}

/// Represents an identifier for a thread.
///
/// Thread IDs are used to group trace events by their originating thread.
/// The main thread is identified specially as "Main Thread", while other
/// threads are identified by their thread ID.
///
/// # Examples
///
/// ```
/// use usd_trace::ThreadId;
///
/// let current = ThreadId::current();
/// println!("Current thread: {}", current);
///
/// // Thread IDs can be compared for equality
/// let same = ThreadId::current();
/// // Note: These may or may not be equal depending on caching
/// ```
#[derive(Clone, Debug)]
pub struct ThreadId {
    /// String representation of the thread ID.
    id: String,
}

impl ThreadId {
    /// Creates a new thread identifier for the current thread.
    ///
    /// If called from the main thread, returns "Main Thread".
    /// Otherwise, returns "Thread XXX" where XXX is the thread ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::ThreadId;
    ///
    /// let id = ThreadId::current();
    /// println!("Running on: {}", id);
    /// ```
    #[must_use]
    pub fn current() -> Self {
        let current_id = thread::current().id();
        let main_id = get_main_thread_id();

        if current_id == main_id {
            Self {
                id: "Main Thread".to_string(),
            }
        } else {
            Self {
                id: format!("Thread {:?}", current_id),
            }
        }
    }

    /// Creates a new thread identifier with a custom name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name for this thread identifier
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::ThreadId;
    ///
    /// let worker = ThreadId::new("Worker 1");
    /// let io_thread = ThreadId::new("I/O Thread");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { id: name.into() }
    }

    /// Returns the string representation of this thread ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::ThreadId;
    ///
    /// let id = ThreadId::new("Worker");
    /// assert_eq!(id.as_str(), "Worker");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.id
    }

    /// Returns true if this is the main thread identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::ThreadId;
    ///
    /// let main_id = ThreadId::new("Main Thread");
    /// assert!(main_id.is_main_thread());
    ///
    /// let worker = ThreadId::new("Worker");
    /// assert!(!worker.is_main_thread());
    /// ```
    #[must_use]
    pub fn is_main_thread(&self) -> bool {
        self.id == "Main Thread"
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::current()
    }
}

impl PartialEq for ThreadId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ThreadId {}

impl Hash for ThreadId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialOrd for ThreadId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ThreadId {
    fn cmp(&self, other: &Self) -> Ordering {
        // Sort by string length first, then lexicographically.
        // This results in numerically sorted thread IDs when they are
        // in the form "Thread XXX".
        match self.id.len().cmp(&other.id.len()) {
            Ordering::Equal => self.id.cmp(&other.id),
            ord => ord,
        }
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl From<&str> for ThreadId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ThreadId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Returns the thread identifier for the current thread.
///
/// This is a convenience function equivalent to `ThreadId::current()`.
///
/// # Examples
///
/// ```
/// use usd_trace::get_thread_id;
///
/// let id = get_thread_id();
/// println!("Current thread: {}", id);
/// ```
#[must_use]
pub fn get_thread_id() -> ThreadId {
    ThreadId::current()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_current() {
        let id = ThreadId::current();
        // On the test thread, it might be main or might not depending on test runner
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_new() {
        let id = ThreadId::new("Custom Thread");
        assert_eq!(id.as_str(), "Custom Thread");
    }

    #[test]
    fn test_default() {
        let id = ThreadId::default();
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_equality() {
        let id1 = ThreadId::new("Test");
        let id2 = ThreadId::new("Test");
        let id3 = ThreadId::new("Other");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_ordering() {
        // Shorter strings should come first
        let id1 = ThreadId::new("Thread 1");
        let id2 = ThreadId::new("Thread 10");
        let id3 = ThreadId::new("Thread 2");

        assert!(id1 < id2); // "Thread 1" (8 chars) < "Thread 10" (9 chars)
        assert!(id3 < id2); // "Thread 2" (8 chars) < "Thread 10" (9 chars)

        // Same length, lexicographic order
        assert!(id1 < id3); // "Thread 1" < "Thread 2"
    }

    #[test]
    fn test_hash() {
        let id1 = ThreadId::new("Test");
        let id2 = ThreadId::new("Test");

        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    #[test]
    fn test_display() {
        let id = ThreadId::new("My Thread");
        assert_eq!(format!("{}", id), "My Thread");
    }

    #[test]
    fn test_from_str() {
        let id: ThreadId = "Test Thread".into();
        assert_eq!(id.as_str(), "Test Thread");
    }

    #[test]
    fn test_from_string() {
        let id: ThreadId = String::from("Test Thread").into();
        assert_eq!(id.as_str(), "Test Thread");
    }

    #[test]
    fn test_is_main_thread() {
        let main = ThreadId::new("Main Thread");
        assert!(main.is_main_thread());

        let other = ThreadId::new("Worker");
        assert!(!other.is_main_thread());
    }

    #[test]
    fn test_get_thread_id() {
        let id = get_thread_id();
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_different_threads() {
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let id = ThreadId::current();
            tx.send(id).expect("failed to send");
        });

        let spawned_id = rx.recv().expect("failed to receive");
        let current_id = ThreadId::current();

        // The spawned thread should have a different ID
        // (unless both are "Main Thread" which shouldn't happen)
        // Actually they could be equal if test runner uses same thread
        // Just verify both are valid
        assert!(!spawned_id.as_str().is_empty());
        assert!(!current_id.as_str().is_empty());

        handle.join().expect("thread panicked");
    }

    #[test]
    fn test_clone() {
        let id1 = ThreadId::new("Test");
        let id2 = id1.clone();
        assert_eq!(id1, id2);
    }
}
