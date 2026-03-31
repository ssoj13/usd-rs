//! SdfChangeBlock - Batching changes for efficient notification.
//!
//! Port of pxr/usd/sdf/changeBlock.h
//!
//! SdfChangeBlock provides a way to group related changes to scene description
//! for more efficient processing. Normally, Sdf sends notification immediately
//! as changes are made. With a change block, notifications are delayed until
//! the outermost block exits.
//!
//! # Warning
//!
//! It is NOT safe to use Usd or other downstream API while a changeblock is open!
//! Derived representations will not have had a chance to update.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

// Global counter for active change blocks per thread.
thread_local! {
    static BLOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Global change block key counter.
static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

/// Pending changes that are queued while a change block is open.
#[derive(Debug, Default)]
pub struct PendingChanges {
    /// Number of changes queued.
    pub count: usize,
}

/// RAII guard for batching scene description changes.
///
/// Opening a changeblock tells Sdf to delay sending notification about
/// changes until the outermost changeblock is exited.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::ChangeBlock;
///
/// {
///     let _block = ChangeBlock::new();
///     // Make multiple changes here...
///     // Notifications are delayed until _block is dropped
/// }
/// // Notifications are sent when the block exits
/// ```
///
/// # Safety
///
/// Do NOT use Usd or downstream APIs while a change block is open.
/// Those APIs may have stale views of the scene.
pub struct ChangeBlock {
    /// Unique key for this block.
    key: u64,
    /// Whether this is the outermost block.
    is_outermost: bool,
}

impl ChangeBlock {
    /// Opens a new change block.
    ///
    /// Changes made while this block is active will be batched.
    /// Notifications are delayed until the outermost block exits.
    #[must_use]
    pub fn new() -> Self {
        let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);

        let depth = BLOCK_DEPTH.with(|d| {
            let current = d.get();
            d.set(current + 1);
            current
        });

        let is_outermost = depth == 0;

        // Notify ChangeManager if this is the outermost block
        if is_outermost {
            use crate::change_manager::ChangeManager;
            ChangeManager::instance().open_change_block();
        }

        Self { key, is_outermost }
    }

    /// Returns whether this is the outermost change block.
    #[inline]
    pub fn is_outermost(&self) -> bool {
        self.is_outermost
    }

    /// Returns the unique key for this change block.
    #[inline]
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Returns the current nesting depth of change blocks.
    pub fn depth() -> u32 {
        BLOCK_DEPTH.with(|d| d.get())
    }

    /// Returns true if any change block is currently open.
    #[inline]
    pub fn is_open() -> bool {
        Self::depth() > 0
    }
}

impl Default for ChangeBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ChangeBlock {
    fn drop(&mut self) {
        let was_outermost = self.is_outermost;

        BLOCK_DEPTH.with(|d| {
            let current = d.get();
            debug_assert!(current > 0, "ChangeBlock depth underflow");
            d.set(current.saturating_sub(1));
        });

        // If this was the outermost block, notify ChangeManager to send notices
        if was_outermost {
            use crate::change_manager::ChangeManager;
            ChangeManager::instance().close_change_block();
        }
    }
}

// Note: ChangeBlock uses thread-local state, so it should only be used
// within a single thread. The key and is_outermost fields are derived
// from thread-local BLOCK_DEPTH counter.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_block_depth() {
        assert_eq!(ChangeBlock::depth(), 0);
        assert!(!ChangeBlock::is_open());

        {
            let block1 = ChangeBlock::new();
            assert!(block1.is_outermost());
            assert_eq!(ChangeBlock::depth(), 1);
            assert!(ChangeBlock::is_open());

            {
                let block2 = ChangeBlock::new();
                assert!(!block2.is_outermost());
                assert_eq!(ChangeBlock::depth(), 2);
            }

            assert_eq!(ChangeBlock::depth(), 1);
        }

        assert_eq!(ChangeBlock::depth(), 0);
        assert!(!ChangeBlock::is_open());
    }

    #[test]
    fn test_change_block_key() {
        let block1 = ChangeBlock::new();
        let block2 = ChangeBlock::new();
        assert_ne!(block1.key(), block2.key());
    }
}
