//! UsdEditContext - RAII utility for temporarily modifying a stage's edit target.
//!
//! Port of pxr/usd/usd/editContext.h/cpp
//!
//! A utility class to temporarily modify a stage's current EditTarget during
//! an execution scope.

use crate::edit_target::EditTarget;
use crate::stage::Stage;
use std::sync::Arc;

/// A utility class to temporarily modify a stage's current EditTarget during
/// an execution scope.
///
/// Matches C++ `UsdEditContext`.
///
/// This is an "RAII"-like object meant to be used as an automatic local
/// variable. Upon construction, it sets a given stage's EditTarget, and upon
/// destruction it restores the stage's EditTarget to what it was previously.
///
/// # Example
///
/// ```rust,ignore
/// use usd_core::{Stage, EditContext};
///
/// fn set_vis_state(prim: &Prim, vis: bool) {
///     let stage = prim.get_stage().unwrap();
///     let _ctx = EditContext::new_with_target(
///         stage.clone(),
///         EditTarget::for_layer(stage.get_session_layer().unwrap())
///     );
///     prim.get_attribute("visible").unwrap().set(vis, TimeCode::default());
///     // EditTarget is restored when _ctx goes out of scope
/// }
/// ```
///
/// # Threading Note
///
/// When one thread is mutating a UsdStage, it is unsafe for any other thread
/// to either query or mutate it. Using this class with a stage in such a way
/// that it modifies the stage's EditTarget constitutes a mutation.
pub struct EditContext {
    stage: Option<Arc<Stage>>,
    original_edit_target: EditTarget,
}

impl EditContext {
    /// Construct without modifying stage's current EditTarget. Save
    /// stage's current EditTarget to restore on destruction.
    ///
    /// Matches C++ `UsdEditContext(const UsdStagePtr &stage)`.
    ///
    /// If stage is invalid, this class takes no action.
    pub fn new(stage: Arc<Stage>) -> Self {
        let original_edit_target = stage.get_edit_target();
        Self {
            stage: Some(stage),
            original_edit_target,
        }
    }

    /// Construct and save stage's current EditTarget to restore on
    /// destruction, then invoke stage->SetEditTarget(editTarget).
    ///
    /// Matches C++ `UsdEditContext(const UsdStagePtr &stage, const UsdEditTarget &editTarget)`.
    ///
    /// If stage is invalid, this class takes no action.
    /// If editTarget is invalid, the stage will issue an error and its
    /// EditTarget will not be modified.
    pub fn new_with_target(stage: Arc<Stage>, edit_target: EditTarget) -> Self {
        let original_edit_target = stage.get_edit_target();
        stage.set_edit_target(edit_target);
        Self {
            stage: Some(stage),
            original_edit_target,
        }
    }

    /// Construct from a (stage, edit_target) pair.
    ///
    /// Matches C++ `UsdEditContext(const std::pair<UsdStagePtr, UsdEditTarget> &stageTarget)`.
    ///
    /// This is handy to construct an edit context from the return value of
    /// another function (e.g., GetVariantEditContext).
    pub fn from_pair(stage_target: (Arc<Stage>, EditTarget)) -> Self {
        Self::new_with_target(stage_target.0, stage_target.1)
    }

    /// Returns the stage this context is bound to.
    pub fn stage(&self) -> Option<&Arc<Stage>> {
        self.stage.as_ref()
    }

    /// Returns the original edit target that will be restored on drop.
    pub fn original_edit_target(&self) -> &EditTarget {
        &self.original_edit_target
    }
}

impl Drop for EditContext {
    /// Restore the stage's original EditTarget if this context's stage is valid.
    ///
    /// Matches C++ `~UsdEditContext()`.
    fn drop(&mut self) {
        if let Some(stage) = &self.stage {
            if self.original_edit_target.is_valid() {
                stage.set_edit_target(self.original_edit_target.clone());
            }
        }
    }
}

/// A guard that provides scoped edit target modification using Rust's ownership system.
///
/// This is a more idiomatic Rust alternative to EditContext that leverages
/// the borrow checker to ensure the edit target is restored.
///
/// # Example
///
/// ```rust,ignore
/// fn edit_in_session_layer(stage: &Stage) {
///     let guard = stage.with_edit_target(session_layer_target);
///     // All edits here go to session layer
///     // Original target is restored when guard drops
/// }
/// ```
pub struct EditTargetGuard<'a> {
    stage: &'a Stage,
    original_edit_target: EditTarget,
}

impl<'a> EditTargetGuard<'a> {
    /// Create a new edit target guard.
    pub fn new(stage: &'a Stage, new_target: EditTarget) -> Self {
        let original_edit_target = stage.get_edit_target();
        stage.set_edit_target(new_target);
        Self {
            stage,
            original_edit_target,
        }
    }
}

impl<'a> Drop for EditTargetGuard<'a> {
    fn drop(&mut self) {
        self.stage
            .set_edit_target(self.original_edit_target.clone());
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_edit_context_creation() {
        // Basic test - full tests require a Stage
    }
}
