//! Internal notice registry scaffolding.
//!
//! Port of pxr/base/tf/noticeRegistry.h
//!
//! In C++, Tf_NoticeRegistry is an internal singleton that manages
//! notice delivery, listener registration, and probe management.
//! In Rust, this functionality is integrated directly into `NoticeRegistry`
//! in the `notice` module.
//!
//! The types here (`NoticeRegistryState`, `DeliveryStats`, `RegistryUseGuard`,
//! `NoticeBlockGuard`) are not currently wired into the live notice system.
//! They are retained as scaffolding matching C++ internals for future use
//! when thread-safe delivery tracking is needed.
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Internal registry state for notice delivery tracking.
///
/// Tracks active deliveries to ensure safe removal of listeners
/// (listeners cannot be removed while another thread is iterating
/// the delivery list).
pub struct NoticeRegistryState {
    /// Number of callers currently using the registry.
    user_count: AtomicUsize,
    /// Whether probe-based introspection is active.
    do_probing: AtomicBool,
    /// Global block count for notice suppression.
    global_block_count: AtomicUsize,
}

impl NoticeRegistryState {
    /// Create new registry state.
    pub fn new() -> Self {
        Self {
            user_count: AtomicUsize::new(0),
            do_probing: AtomicBool::new(false),
            global_block_count: AtomicUsize::new(0),
        }
    }

    /// Increment the user count (called when entering registry operations).
    pub fn begin_use(&self) {
        self.user_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrement the user count (called when leaving registry operations).
    pub fn end_use(&self) {
        self.user_count.fetch_sub(1, Ordering::AcqRel);
    }

    /// Returns true if only one caller is using the registry (safe to remove entries).
    pub fn is_sole_user(&self) -> bool {
        self.user_count.load(Ordering::Acquire) <= 1
    }

    /// Returns current user count.
    pub fn user_count(&self) -> usize {
        self.user_count.load(Ordering::Acquire)
    }

    /// Check if probing is active.
    pub fn is_probing(&self) -> bool {
        self.do_probing.load(Ordering::Acquire)
    }

    /// Set probing state.
    pub fn set_probing(&self, probing: bool) {
        self.do_probing.store(probing, Ordering::Release);
    }

    /// Increment global block count (suppress notice delivery).
    pub fn increment_block_count(&self) {
        self.global_block_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrement global block count (resume notice delivery).
    pub fn decrement_block_count(&self) {
        self.global_block_count.fetch_sub(1, Ordering::AcqRel);
    }

    /// Returns true if notice delivery is globally blocked.
    pub fn is_blocked(&self) -> bool {
        self.global_block_count.load(Ordering::Acquire) > 0
    }
}

impl Default for NoticeRegistryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Delivery statistics for notice send operations.
#[derive(Debug, Clone, Default)]
pub struct DeliveryStats {
    /// Number of listeners that received the notice.
    pub listeners_notified: usize,
    /// Number of listeners that were skipped (expired weak refs, etc.).
    pub listeners_skipped: usize,
}

/// Scope guard for tracking registry usage.
///
/// Increments user count on creation, decrements on drop.
/// This prevents unsafe removal of deliverer entries while
/// iteration is in progress.
pub struct RegistryUseGuard<'a> {
    state: &'a NoticeRegistryState,
}

impl<'a> RegistryUseGuard<'a> {
    /// Begin using the registry.
    pub fn new(state: &'a NoticeRegistryState) -> Self {
        state.begin_use();
        Self { state }
    }
}

impl Drop for RegistryUseGuard<'_> {
    fn drop(&mut self) {
        self.state.end_use();
    }
}

/// Scope guard for blocking notice delivery.
///
/// Increments block count on creation, decrements on drop.
pub struct NoticeBlockGuard<'a> {
    state: &'a NoticeRegistryState,
}

impl<'a> NoticeBlockGuard<'a> {
    /// Block notice delivery.
    pub fn new(state: &'a NoticeRegistryState) -> Self {
        state.increment_block_count();
        Self { state }
    }
}

impl Drop for NoticeBlockGuard<'_> {
    fn drop(&mut self) {
        self.state.decrement_block_count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_state() {
        let state = NoticeRegistryState::new();
        assert_eq!(state.user_count(), 0);
        assert!(state.is_sole_user());

        state.begin_use();
        assert_eq!(state.user_count(), 1);
        assert!(state.is_sole_user());

        state.begin_use();
        assert_eq!(state.user_count(), 2);
        assert!(!state.is_sole_user());

        state.end_use();
        assert!(state.is_sole_user());
    }

    #[test]
    fn test_block_count() {
        let state = NoticeRegistryState::new();
        assert!(!state.is_blocked());

        state.increment_block_count();
        assert!(state.is_blocked());

        state.decrement_block_count();
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_probing() {
        let state = NoticeRegistryState::new();
        assert!(!state.is_probing());

        state.set_probing(true);
        assert!(state.is_probing());
    }

    #[test]
    fn test_use_guard() {
        let state = NoticeRegistryState::new();
        assert_eq!(state.user_count(), 0);

        {
            let _guard = RegistryUseGuard::new(&state);
            assert_eq!(state.user_count(), 1);

            {
                let _guard2 = RegistryUseGuard::new(&state);
                assert_eq!(state.user_count(), 2);
            }

            assert_eq!(state.user_count(), 1);
        }

        assert_eq!(state.user_count(), 0);
    }

    #[test]
    fn test_block_guard() {
        let state = NoticeRegistryState::new();
        assert!(!state.is_blocked());

        {
            let _guard = NoticeBlockGuard::new(&state);
            assert!(state.is_blocked());
        }

        assert!(!state.is_blocked());
    }
}
