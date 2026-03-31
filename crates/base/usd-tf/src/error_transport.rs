//! Error transport for moving errors between threads.
//!
//! This module provides a facility for transporting errors from one thread to
//! another. This is useful when work is done in a child thread but errors
//! should be reported in the parent thread.
//!
//! # Usage
//!
//! Typical use is to create an [`ErrorMark`] in the thread that is the error
//! source (e.g., the child thread), then call [`ErrorMark::transport()`] to
//! lift generated errors out into an [`ErrorTransport`] object. Later the
//! thread that wants to sink those errors (e.g., the parent thread) invokes
//! [`ErrorTransport::post()`] to post all contained errors to its own
//! thread's error list.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{ErrorTransport, ErrorMark, tf_error};
//! use std::thread;
//!
//! // Create transport to move errors from child to parent
//! let transport = thread::spawn(|| {
//!     let mark = ErrorMark::new();
//!     tf_error!("Error in child thread");
//!     mark.transport()
//! }).join().unwrap();
//!
//! // Post errors to parent thread
//! transport.post();
//! ```

use super::diagnostic_mgr::DiagnosticMgr;
use super::error::TfError;

/// A facility for transporting errors from thread to thread.
///
/// `ErrorTransport` holds a list of errors that can be moved between threads.
/// Use [`ErrorMark::transport()`] to create one, then call [`post()`] in the
/// destination thread to inject the errors there.
///
/// [`ErrorMark::transport()`]: super::ErrorMark::transport
/// [`post()`]: ErrorTransport::post
#[derive(Debug, Default)]
pub struct ErrorTransport {
    /// The transported error list.
    errors: Vec<TfError>,
}

impl ErrorTransport {
    /// Construct an empty `ErrorTransport`.
    #[must_use]
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Construct from a list of errors.
    #[must_use]
    pub(crate) fn from_errors(errors: Vec<TfError>) -> Self {
        Self { errors }
    }

    /// Post all contained errors to the current thread's error list.
    ///
    /// This method consumes the transport. After calling, the errors
    /// are moved to the current thread's error list.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::ErrorTransport;
    ///
    /// let transport = ErrorTransport::new();
    /// assert!(transport.is_empty()); // Check before posting
    /// transport.post(); // No-op for empty transport
    /// ```
    pub fn post(self) {
        if !self.is_empty() {
            self.post_impl();
        }
    }

    /// Internal implementation of post.
    fn post_impl(self) {
        DiagnosticMgr::instance().splice_errors(self.errors);
    }

    /// Return true if this `ErrorTransport` contains no errors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Return the number of errors in this transport.
    #[must_use]
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Swap this `ErrorTransport`'s content with `other`.
    ///
    /// This provides a lightweight way to move the contents of one
    /// `ErrorTransport` to another.
    pub fn swap(&mut self, other: &mut ErrorTransport) {
        std::mem::swap(&mut self.errors, &mut other.errors);
    }

    /// Take all errors out of this transport, leaving it empty.
    #[must_use]
    pub fn take(&mut self) -> Vec<TfError> {
        std::mem::take(&mut self.errors)
    }

    /// Get a reference to the contained errors.
    #[must_use]
    pub fn errors(&self) -> &[TfError] {
        &self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let transport = ErrorTransport::new();
        assert!(transport.is_empty());
        assert_eq!(transport.len(), 0);
    }

    #[test]
    fn test_default() {
        let transport: ErrorTransport = Default::default();
        assert!(transport.is_empty());
    }

    #[test]
    fn test_swap() {
        let mut t1 = ErrorTransport::new();
        let mut t2 = ErrorTransport::new();

        // Both empty initially
        assert!(t1.is_empty());
        assert!(t2.is_empty());

        // Swap should work with empty transports
        t1.swap(&mut t2);
        assert!(t1.is_empty());
        assert!(t2.is_empty());
    }

    #[test]
    fn test_take() {
        let mut transport = ErrorTransport::new();
        let errors = transport.take();
        assert!(errors.is_empty());
        assert!(transport.is_empty());
    }

    #[test]
    fn test_post_empty() {
        // Posting an empty transport should be a no-op
        let transport = ErrorTransport::new();
        transport.post();
        // No panic = success
    }

    #[test]
    fn test_errors() {
        let transport = ErrorTransport::new();
        assert!(transport.errors().is_empty());
    }
}
