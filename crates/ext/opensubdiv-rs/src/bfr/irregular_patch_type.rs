//! IrregularPatchType — type alias for the single irregular patch representation.
//!
//! In C++ this header forward-declares `PatchTree` and creates a typedef
//! `IrregularPatchType = PatchTree`.  We mirror the same relationship here.

use std::sync::Arc;

use super::patch_tree::PatchTree;

/// The concrete type that backs irregular patches.
///
/// Only `PatchTree` exists as an implementation — the C++ comment explicitly
/// notes that a potential abstraction layer was deferred.
pub type IrregularPatchType = PatchTree;

/// Shared (ref-counted, immutable) pointer to an irregular patch.
pub type IrregularPatchSharedPtr = Arc<IrregularPatchType>;
