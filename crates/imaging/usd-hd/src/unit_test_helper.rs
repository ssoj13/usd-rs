
//! HdUnitTestHelper - Unit test driver for core engine.
//!
//! Corresponds to pxr/imaging/hd/unitTestHelper.h.
//! Minimal test driver that exercises render index and engine.

use crate::prim::HdReprSelector;
use crate::render::{HdRenderPassSharedPtr, HdRenderPassStateSharedPtr, HdRprimCollection};
use std::sync::Arc;
use usd_tf::Token;

/// Unit test driver - exercises render index and engine.
///
/// Corresponds to C++ `Hd_TestDriver`.
pub struct HdUnitTestHelper {
    _repr_selector: HdReprSelector,
    _render_pass: Option<HdRenderPassSharedPtr>,
    _render_pass_state: HdRenderPassStateSharedPtr,
    _collection: HdRprimCollection,
}

impl HdUnitTestHelper {
    /// Create new test driver.
    pub fn new() -> Self {
        Self::with_repr(HdReprSelector::default())
    }

    /// Create with specific repr selector.
    pub fn with_repr(repr_selector: HdReprSelector) -> Self {
        let collection = HdRprimCollection::new(Token::new("test"));
        let state = Arc::new(crate::render_pass_state::HdRenderPassStateBase::default());
        Self {
            _repr_selector: repr_selector,
            _render_pass: None,
            _render_pass_state: state,
            _collection: collection,
        }
    }

    /// Get render pass state.
    pub fn get_render_pass_state(&self) -> &HdRenderPassStateSharedPtr {
        &self._render_pass_state
    }

    /// Get collection.
    pub fn get_collection(&self) -> &HdRprimCollection {
        &self._collection
    }
}

impl Default for HdUnitTestHelper {
    fn default() -> Self {
        Self::new()
    }
}
