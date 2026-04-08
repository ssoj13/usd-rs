//! HdUnitTestNullRenderDelegate - Null render delegate for unit tests.
//!
//! Corresponds to pxr/imaging/hd/unitTestNullRenderDelegate.h.
//! Implements HdRenderDelegate with no-op rendering for tests.

use crate::change_tracker::HdChangeTracker;
use crate::command::{HdCommandArgs, HdCommandDescriptors};
use crate::render::render_delegate::HdResourceRegistry;
use crate::render::render_delegate::{HdRenderDelegate, HdRenderParamSharedPtr, TfTokenVector};
use crate::render::render_index::HdRenderIndex;
use crate::render::{
    HdRenderPass, HdRenderPassSharedPtr, HdRenderPassStateSharedPtr, HdResourceRegistrySharedPtr,
};
use crate::{HdRprimCollection, HdSceneDelegate};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

static SUPPORTED_RPRIM: Lazy<TfTokenVector> = Lazy::new(|| {
    vec![
        Token::new("mesh"),
        Token::new("basisCurves"),
        Token::new("points"),
    ]
});
static SUPPORTED_SPRIM: Lazy<TfTokenVector> = Lazy::new(|| {
    vec![
        Token::new("camera"),
        Token::new("light"),
        Token::new("material"),
    ]
});
static SUPPORTED_BPRIM: Lazy<TfTokenVector> = Lazy::new(|| vec![Token::new("renderBuffer")]);

/// Null resource registry for unit tests.
struct NullResourceRegistry;

impl HdResourceRegistry for NullResourceRegistry {}

/// Null render delegate for unit tests - no actual rendering.
pub struct HdUnitTestNullRenderDelegate;

impl HdUnitTestNullRenderDelegate {
    /// Create new null render delegate.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HdUnitTestNullRenderDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl HdRenderDelegate for HdUnitTestNullRenderDelegate {
    fn get_supported_rprim_types(&self) -> &TfTokenVector {
        &SUPPORTED_RPRIM
    }

    fn get_supported_sprim_types(&self) -> &TfTokenVector {
        &SUPPORTED_SPRIM
    }

    fn get_supported_bprim_types(&self) -> &TfTokenVector {
        &SUPPORTED_BPRIM
    }

    fn create_rprim(
        &mut self,
        _type_id: &Token,
        _id: SdfPath,
    ) -> Option<crate::render::HdPrimHandle> {
        None
    }

    fn create_sprim(
        &mut self,
        _type_id: &Token,
        _id: SdfPath,
    ) -> Option<crate::render::HdPrimHandle> {
        None
    }

    fn create_bprim(
        &mut self,
        _type_id: &Token,
        _id: SdfPath,
    ) -> Option<crate::render::HdPrimHandle> {
        None
    }

    fn create_instancer(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _id: SdfPath,
    ) -> Option<Box<dyn crate::render::render_delegate::HdInstancer>> {
        None
    }

    fn destroy_instancer(
        &mut self,
        _instancer: Box<dyn crate::render::render_delegate::HdInstancer>,
    ) {
    }

    fn create_fallback_sprim(&mut self, _type_id: &Token) -> Option<crate::render::HdPrimHandle> {
        None
    }

    fn create_fallback_bprim(&mut self, _type_id: &Token) -> Option<crate::render::HdPrimHandle> {
        None
    }

    fn create_render_pass(
        &mut self,
        _index: &HdRenderIndex,
        collection: &HdRprimCollection,
    ) -> Option<HdRenderPassSharedPtr> {
        Some(Arc::new(HdUnitTestNullRenderPass::new(collection.clone())))
    }

    fn commit_resources(&mut self, _tracker: &mut HdChangeTracker) {
        // No-op for null delegate
    }

    fn get_resource_registry(&self) -> HdResourceRegistrySharedPtr {
        Arc::new(NullResourceRegistry)
    }

    fn get_render_param(&self) -> Option<HdRenderParamSharedPtr> {
        None
    }

    fn get_command_descriptors(&self) -> HdCommandDescriptors {
        Vec::new()
    }

    fn invoke_command(&mut self, _command: &Token, _args: &HdCommandArgs) -> bool {
        false
    }
}

/// Null render pass - sync only, no draw.
pub struct HdUnitTestNullRenderPass {
    collection: HdRprimCollection,
}

impl HdUnitTestNullRenderPass {
    /// Create new null render pass with the given collection.
    pub fn new(collection: HdRprimCollection) -> Self {
        Self { collection }
    }
}

impl HdRenderPass for HdUnitTestNullRenderPass {
    fn get_rprim_collection(&self) -> &HdRprimCollection {
        &self.collection
    }

    fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
        self.collection = collection;
    }

    fn sync(&mut self) {
        // No-op for unit tests
    }

    fn execute(&mut self, _state: &HdRenderPassStateSharedPtr, _render_tags: &TfTokenVector) {
        // No-op for unit tests
    }
}
