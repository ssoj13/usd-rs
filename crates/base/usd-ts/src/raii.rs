//! RAII helpers for spline authoring.
//!
//! Port of pxr/base/ts/raii.h
//!
//! Provides thread-local stacked selectors for controlling
//! authoring behavior during interactive editing.

use super::types::AntiRegressionMode;
use std::cell::RefCell;

thread_local! {
    /// Stack of anti-regression authoring mode selectors.
    static ANTI_REGRESSION_STACK: RefCell<Vec<AntiRegressionMode>> = const { RefCell::new(Vec::new()) };

    /// Stack of edit behavior blocks.
    static EDIT_BLOCK_STACK: RefCell<usize> = const { RefCell::new(0) };
}

/// RAII helper that locally sets the anti-regression authoring mode.
///
/// The effect lasts as long as the object exists.
/// The effect is limited to the calling thread.
/// Multiple instances on the same thread will stack.
///
/// # Examples
///
/// ```
/// use usd_ts::{AntiRegressionAuthoringSelector, AntiRegressionMode};
///
/// // Set mode for this scope
/// {
///     let _selector = AntiRegressionAuthoringSelector::new(AntiRegressionMode::Contain);
///     // Mode is now Contain...
/// }
/// // Mode reverts to previous
/// ```
pub struct AntiRegressionAuthoringSelector {
    mode: AntiRegressionMode,
}

impl AntiRegressionAuthoringSelector {
    /// Creates a new selector with the given mode.
    pub fn new(mode: AntiRegressionMode) -> Self {
        ANTI_REGRESSION_STACK.with(|stack| {
            stack.borrow_mut().push(mode);
        });

        Self { mode }
    }

    /// Returns the mode this selector is using.
    pub fn mode(&self) -> AntiRegressionMode {
        self.mode
    }

    /// Returns the current effective anti-regression mode.
    ///
    /// Returns the top of the stack, or None if no selector is active.
    pub fn current() -> Option<AntiRegressionMode> {
        ANTI_REGRESSION_STACK.with(|stack| stack.borrow().last().copied())
    }

    /// Returns the current effective anti-regression mode,
    /// or the default if no selector is active.
    pub fn current_or_default() -> AntiRegressionMode {
        Self::current().unwrap_or(AntiRegressionMode::None)
    }
}

impl Drop for AntiRegressionAuthoringSelector {
    fn drop(&mut self) {
        ANTI_REGRESSION_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// RAII helper that temporarily blocks automatic editing behaviors.
///
/// Currently this includes anti-regression. While active,
/// automatic anti-regression adjustments are disabled.
///
/// # Examples
///
/// ```
/// use usd_ts::EditBehaviorBlock;
///
/// // Block auto-behaviors for this scope
/// {
///     let _block = EditBehaviorBlock::new();
///     // Auto-behaviors disabled...
/// }
/// // Auto-behaviors restored
/// ```
pub struct EditBehaviorBlock;

impl EditBehaviorBlock {
    /// Creates a new edit behavior block.
    pub fn new() -> Self {
        EDIT_BLOCK_STACK.with(|count| {
            *count.borrow_mut() += 1;
        });

        Self
    }

    /// Returns true if edit behaviors are currently blocked.
    pub fn is_blocked() -> bool {
        EDIT_BLOCK_STACK.with(|count| *count.borrow() > 0)
    }

    /// Returns the current block depth.
    pub fn depth() -> usize {
        EDIT_BLOCK_STACK.with(|count| *count.borrow())
    }
}

impl Default for EditBehaviorBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EditBehaviorBlock {
    fn drop(&mut self) {
        EDIT_BLOCK_STACK.with(|count| {
            let mut c = count.borrow_mut();
            if *c > 0 {
                *c -= 1;
            }
        });
    }
}

/// Returns the current anti-regression authoring mode.
///
/// If an EditBehaviorBlock is active, returns None.
/// Otherwise returns the current AntiRegressionAuthoringSelector mode.
pub fn get_anti_regression_mode() -> Option<AntiRegressionMode> {
    if EditBehaviorBlock::is_blocked() {
        return None;
    }

    AntiRegressionAuthoringSelector::current()
}

/// Returns the current anti-regression mode or default.
///
/// If an EditBehaviorBlock is active, returns None.
/// Otherwise returns the current mode or Contain as default.
pub fn get_anti_regression_mode_or_default() -> AntiRegressionMode {
    if EditBehaviorBlock::is_blocked() {
        return AntiRegressionMode::None;
    }

    AntiRegressionAuthoringSelector::current_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anti_regression_selector() {
        assert!(AntiRegressionAuthoringSelector::current().is_none());

        {
            let _s1 = AntiRegressionAuthoringSelector::new(AntiRegressionMode::Contain);
            assert_eq!(
                AntiRegressionAuthoringSelector::current(),
                Some(AntiRegressionMode::Contain)
            );

            {
                let _s2 = AntiRegressionAuthoringSelector::new(AntiRegressionMode::KeepRatio);
                assert_eq!(
                    AntiRegressionAuthoringSelector::current(),
                    Some(AntiRegressionMode::KeepRatio)
                );
            }

            // Back to Contain
            assert_eq!(
                AntiRegressionAuthoringSelector::current(),
                Some(AntiRegressionMode::Contain)
            );
        }

        // Back to None
        assert!(AntiRegressionAuthoringSelector::current().is_none());
    }

    #[test]
    fn test_edit_behavior_block() {
        assert!(!EditBehaviorBlock::is_blocked());
        assert_eq!(EditBehaviorBlock::depth(), 0);

        {
            let _b1 = EditBehaviorBlock::new();
            assert!(EditBehaviorBlock::is_blocked());
            assert_eq!(EditBehaviorBlock::depth(), 1);

            {
                let _b2 = EditBehaviorBlock::new();
                assert!(EditBehaviorBlock::is_blocked());
                assert_eq!(EditBehaviorBlock::depth(), 2);
            }

            assert_eq!(EditBehaviorBlock::depth(), 1);
        }

        assert!(!EditBehaviorBlock::is_blocked());
        assert_eq!(EditBehaviorBlock::depth(), 0);
    }

    #[test]
    fn test_block_overrides_selector() {
        let _s = AntiRegressionAuthoringSelector::new(AntiRegressionMode::Contain);

        assert_eq!(
            get_anti_regression_mode(),
            Some(AntiRegressionMode::Contain)
        );

        {
            let _b = EditBehaviorBlock::new();
            assert!(get_anti_regression_mode().is_none());
            assert_eq!(
                get_anti_regression_mode_or_default(),
                AntiRegressionMode::None
            );
        }

        // Back to Contain
        assert_eq!(
            get_anti_regression_mode(),
            Some(AntiRegressionMode::Contain)
        );
    }
}
